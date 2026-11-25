use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::omp::OmpSession;
use crate::profiles::ModelProfile;

/// Trimmed `get_available_models` entry; the raw catalog is several MiB.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub thinking_levels: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProfileModelOptions {
    roles: Vec<String>,
    models: Vec<ProfileCatalogModel>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileCatalogModel {
    provider: String,
    id: String,
    name: String,
    kind: String,
    #[serde(skip_serializing)]
    reasoning: bool,
    #[serde(rename(deserialize = "thinking"), default, deserialize_with = "deserialize_efforts")]
    thinking_levels: Vec<String>,
}

fn deserialize_efforts<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    Ok(Option::<Vec<String>>::deserialize(deserializer)?.unwrap_or_default())
}

pub async fn profile_model_options(
    cwd: &Path,
    env: &BTreeMap<String, String>,
) -> Result<ProfileModelOptions, String> {
    #[derive(Deserialize)]
    struct Catalog {
        models: Vec<ProfileCatalogModel>,
    }

    // The Custom RPC catalog is chat-only and needs an open session. The CLI also
    // lists specialized models and can serve global profiles with no workspace.
    // Use the same workspace environment as the running agent, including provider keys.
    let catalog = Command::new("omp")
        .args(["models", "ls", "--kind", "all", "--json"])
        .current_dir(cwd)
        .envs(env)
        .output();
    let mut config = Command::new("omp");
    config.current_dir(cwd).envs(env);
    let (output, configured) = tokio::join!(catalog, read_model_roles(&mut config));
    let output = output.map_err(|e| format!("Failed to list omp models: {e}"))?;
    if !output.status.success() {
        return Err(format!("Failed to list omp models: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let Catalog { mut models } = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse omp model catalog: {e}"))?;
    for model in &mut models {
        if model.reasoning {
            model.thinking_levels.insert(0, "off".to_owned());
        }
    }
    // Built-in role IDs from omp's model browser, plus configured custom roles.
    let mut roles: Vec<_> = [
        "default", "advisor", "smol", "slow", "vision", "plan", "commit", "tiny",
        "memory", "task", "widget", "image", "web", "speech", "dictation", "judge",
    ].into_iter().map(str::to_owned).collect();
    roles.extend(configured?.into_keys());
    roles.sort_unstable();
    roles.dedup();
    Ok(ProfileModelOptions { roles, models })
}

/// `role` value marking a tab whose model was picked directly instead of via a role.
pub const CUSTOM_ROLE: &str = "custom";

pub const VALID_THINKING_LEVELS: &[&str] = &[
    "off", "minimal", "low", "medium", "high", "xhigh", "max", "adaptive",
];

/// Splits candidate by `/` into `(provider, rest)`.
/// Checks if `rest` ends with `:<level>` where level is in valid thinking levels.
/// If yes: strips `:<level>`, returns `(provider, model_id, Some(level))`.
/// If no: does NOT strip colon (e.g. `llama3:8b` preserves `:8b`), returns `(provider, model_id, None)`.
pub fn parse_candidate(candidate: &str) -> (String, String, Option<String>) {
    let candidate = candidate.trim();
    let (provider, rest) = match candidate.split_once('/') {
        Some((p, r)) => (p.trim(), r.trim()),
        None => ("", candidate),
    };

    if let Some((model, possible_level)) = rest.rsplit_once(':') {
        let level_trimmed = possible_level.trim();
        if VALID_THINKING_LEVELS.contains(&level_trimmed) {
            return (
                provider.to_string(),
                model.trim().to_string(),
                Some(level_trimmed.to_string()),
            );
        }
    }

    (provider.to_string(), rest.to_string(), None)
}

/// Splits comma-separated candidates: `cand1, cand2, ...`.
pub fn split_candidates(selector: &str) -> Vec<&str> {
    selector
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Follows `@<alias>` references with cycle detection guard (e.g. `HashSet<String>`).
pub fn resolve_role_alias(
    roles: &HashMap<String, String>,
    role_name: &str,
) -> Result<String, String> {
    let mut current = role_name.strip_prefix('@').unwrap_or(role_name).trim().to_string();
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(current.clone()) {
            return Err(format!("Cycle detected in role alias: {current}"));
        }

        let selector = roles
            .get(&current)
            .ok_or_else(|| format!("Role not found: {current}"))?
            .trim();

        if let Some(alias) = selector.strip_prefix('@') {
            current = alias.trim().to_string();
        } else {
            return Ok(selector.to_string());
        }
    }
}

/// Runs `omp config get modelRoles --json` with `cwd` = project dir to get model roles map.
pub async fn get_model_roles(cwd: &Path) -> Result<HashMap<String, String>, String> {
    read_model_roles(Command::new("omp").current_dir(cwd)).await
}

async fn read_model_roles(command: &mut Command) -> Result<HashMap<String, String>, String> {
    let output = command
        .args(["config", "get", "modelRoles", "--json"])
        .output()
        .await
        .map_err(|e| format!("Failed to execute 'omp config get modelRoles --json': {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("'omp config get modelRoles --json' failed: {stderr}"));
    }

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse omp config JSON output: {e}"))?;

    let roles_val = if let Some(val) = parsed.get("value") {
        val
    } else {
        &parsed
    };

    serde_json::from_value(roles_val.clone())
        .map_err(|e| format!("Failed to deserialize modelRoles map: {e}"))
}

async fn send(session: &OmpSession, cmd: serde_json::Value) -> Result<serde_json::Value, String> {
    match cmd {
        serde_json::Value::Object(map) => session.request(map).await,
        _ => Err("RPC command must be a JSON object".to_string()),
    }
}

pub async fn apply_profile(session: &OmpSession, profile: &ModelProfile) -> Result<(), String> {
    send_runtime_settings(session, profile).await
}

pub(crate) async fn clear_profile(
    session: &OmpSession,
    advisor_model: Option<&str>,
    advisor_thinking: Option<&str>,
) -> Result<(), String> {
    send_runtime_settings(
        session,
        serde_json::json!({
            "clearProfile": true,
            "advisorModel": advisor_model,
            "advisorThinking": advisor_thinking,
        }),
    )
    .await
}

pub(crate) async fn set_advisor(
    session: &OmpSession,
    model: Option<&str>,
    level: Option<&str>,
) -> Result<(), String> {
    send_runtime_settings(
        session,
        serde_json::json!({ "advisorModel": model, "advisorThinking": level }),
    )
    .await
}

/// Omp has no role-mutation RPC. The Bide extension applies runtime-only settings and
/// acknowledges this ticket over rpc-ui, without changing either omp config file.
async fn send_runtime_settings(session: &OmpSession, payload: impl Serialize) -> Result<(), String> {
    const COMMAND: &str = "bide-model-profile";
    let available = send(session, serde_json::json!({ "type": "get_available_commands" })).await?;
    if !available["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| command["name"] == COMMAND && command["source"] == "extension")
    }) {
        return Err("The Bide model profile command is unavailable in this omp session".into());
    }

    let ticket = uuid::Uuid::new_v4();
    let status_key = format!("{COMMAND}:{ticket}");
    let encoded = BASE64_STANDARD.encode(serde_json::to_vec(&payload).map_err(|e| e.to_string())?);
    let mut events = session.subscribe_broadcast();
    session.command(&format!("/{COMMAND} {ticket} {encoded}")).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.recv().await {
                Ok(frame) if frame["statusKey"] == status_key => {
                    let result: serde_json::Value = serde_json::from_str(
                        frame["statusText"].as_str().ok_or("Missing model profile result")?,
                    )
                    .map_err(|e| format!("Invalid model profile result: {e}"))?;
                    return match result["error"].as_str() {
                        Some(error) => Err(error.to_owned()),
                        None => Ok(()),
                    };
                }
                Ok(frame) if frame["type"] == "process_exited" => {
                    return Err("omp stopped before applying the model profile".into());
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => return Err("omp closed before applying the model profile".into()),
            }
        }
    })
    .await
    .map_err(|_| "omp did not confirm the model profile change".to_string())?
}

/// `model` is `provider/modelId`; the model id itself may contain `/`.
pub async fn set_model(session: &OmpSession, model: &str) -> Result<serde_json::Value, String> {
    let (provider, model_id) = model
        .split_once('/')
        .ok_or_else(|| format!("Model must be 'provider/modelId', got '{model}'"))?;
    send(session, serde_json::json!({ "type": "set_model", "provider": provider, "modelId": model_id })).await
}

pub async fn set_thinking(session: &OmpSession, level: &str) -> Result<serde_json::Value, String> {
    send(session, serde_json::json!({ "type": "set_thinking_level", "level": level })).await
}

#[derive(Deserialize)]
struct CatalogModel {
    provider: String,
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    reasoning: bool,
    thinking: Option<CatalogThinking>,
}

#[derive(Deserialize)]
struct CatalogThinking {
    #[serde(default)]
    efforts: Vec<String>,
}

impl From<CatalogModel> for ModelInfo {
    fn from(model: CatalogModel) -> Self {
        let thinking_levels = if model.reasoning {
            let mut efforts = model.thinking.map_or_else(Vec::new, |thinking| thinking.efforts);
            efforts.insert(0, "off".to_owned());
            efforts
        } else {
            Vec::new()
        };
        Self { provider: model.provider, id: model.id, name: model.name, thinking_levels }
    }
}

async fn catalog_models(session: &OmpSession) -> Result<Vec<CatalogModel>, String> {
    #[derive(Deserialize)]
    struct Res {
        models: Vec<CatalogModel>,
    }
    let res = send(session, serde_json::json!({ "type": "get_available_models" })).await?;
    serde_json::from_value::<Res>(res)
        .map(|r| r.models)
        .map_err(|e| format!("Failed to parse available models: {e}"))
}

pub async fn list_models(session: &OmpSession) -> Result<Vec<ModelInfo>, String> {
    Ok(catalog_models(session).await?.into_iter().map(ModelInfo::from).collect())
}

pub(crate) async fn validate_model_thinking(
    session: &OmpSession,
    model: &str,
    level: &str,
) -> Result<(), String> {
    let (provider, id) = model
        .split_once('/')
        .ok_or_else(|| format!("Model must be 'provider/modelId', got '{model}'"))?;
    let entry = catalog_models(session)
        .await?
        .into_iter()
        .find(|entry| entry.provider == provider && entry.id == id)
        .ok_or_else(|| format!("Model '{model}' is unavailable"))?;
    let supported = entry.reasoning
        && (level == "off" || entry.thinking.as_ref().is_some_and(|thinking| {
            thinking.efforts.iter().any(|effort| effort == level)
        }));
    if !supported {
        return Err(format!("Model '{model}' does not support thinking level '{level}'"));
    }
    Ok(())
}

pub async fn list_thinking_levels(session: &OmpSession) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Res {
        levels: Vec<String>,
    }
    let res = send(session, serde_json::json!({ "type": "get_available_thinking_levels" })).await?;
    serde_json::from_value::<Res>(res)
        .map(|r| r.levels)
        .map_err(|e| format!("Failed to parse thinking levels: {e}"))
}

/// Resolves role selector and applies model and optional thinking level to the session.
pub async fn set_role(
    session: &OmpSession,
    cwd: &Path,
    role_name: &str,
) -> Result<serde_json::Value, String> {
    let model_roles = get_model_roles(cwd).await?;
    let selector = resolve_role_alias(&model_roles, role_name)?;
    let candidates = split_candidates(&selector);
    if candidates.is_empty() {
        return Err(format!("No candidate models specified for role '{role_name}'"));
    }

    let mut last_err = String::new();
    for candidate in candidates {
        let (provider, model_id, thinking_level) = parse_candidate(candidate);
        let cmd = serde_json::json!({ "type": "set_model", "provider": provider, "modelId": model_id });
        match send(session, cmd).await {
            Ok(model_data) => {
                if let Some(level) = thinking_level {
                    if let Err(e) = set_thinking(session, &level).await {
                        return Err(format!("Model set but failed to set thinking level: {e}"));
                    }
                }
                return Ok(model_data);
            }
            Err(e) => {
                last_err = e;
            }
        }
    }

    Err(format!(
        "All candidate models failed for role '{role_name}'. Last error: {last_err}"
    ))
}

/// Re-applies a tab's persisted selection to a freshly spawned session (best effort).
pub async fn apply_selection(
    session: &OmpSession,
    cwd: &Path,
    role: &str,
    model: Option<&str>,
    thinking: Option<&str>,
    advisor_model: Option<&str>,
    advisor_thinking: Option<&str>,
) {
    match model {
        Some(m) if role == CUSTOM_ROLE => {
            let _ = set_model(session, m).await;
        }
        _ if role != "default" => {
            let _ = set_role(session, cwd, role).await;
        }
        _ => {}
    }
    if let Some(level) = thinking {
        let _ = set_thinking(session, level).await;
    }
    if let Some(model) = advisor_model {
        if let Err(e) = set_advisor(session, Some(model), advisor_thinking).await {
            eprintln!("Failed to restore Custom advisor model {model:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_catalog_exposes_only_supported_thinking_levels() {
        for (reasoning, thinking, expected) in [
            (true, serde_json::json!({ "efforts": ["low", "high"] }), vec!["off", "low", "high"]),
            (true, serde_json::Value::Null, vec!["off"]),
            (false, serde_json::json!({ "efforts": ["low"] }), vec![]),
        ] {
            let raw = serde_json::json!({
                "provider": "test", "id": "model", "reasoning": reasoning, "thinking": thinking,
            });
            let model = ModelInfo::from(serde_json::from_value::<CatalogModel>(raw).unwrap());
            assert_eq!(serde_json::to_value(model).unwrap()["thinkingLevels"], serde_json::json!(expected));
        }
    }

    #[test]
    fn test_parse_candidate_thinking_level_stripping_vs_model_tag() {
        // anthropic/claude-3-7-sonnet:high -> level high
        assert_eq!(
            parse_candidate("anthropic/claude-3-7-sonnet:high"),
            (
                "anthropic".to_string(),
                "claude-3-7-sonnet".to_string(),
                Some("high".to_string())
            )
        );

        // ollama/llama3:8b -> model llama3:8b, level None
        assert_eq!(
            parse_candidate("ollama/llama3:8b"),
            ("ollama".to_string(), "llama3:8b".to_string(), None)
        );

        // ollama/llama3:8b:high -> model llama3:8b, level high
        assert_eq!(
            parse_candidate("ollama/llama3:8b:high"),
            (
                "ollama".to_string(),
                "llama3:8b".to_string(),
                Some("high".to_string())
            )
        );

        // Plain provider/model with no colon
        assert_eq!(
            parse_candidate("openai/gpt-4o"),
            ("openai".to_string(), "gpt-4o".to_string(), None)
        );

        // All valid thinking levels recognized
        for level in VALID_THINKING_LEVELS {
            let input = format!("provider/model:{level}");
            let parsed = parse_candidate(&input);
            assert_eq!(
                parsed,
                (
                    "provider".to_string(),
                    "model".to_string(),
                    Some((*level).to_string())
                )
            );
        }

        // Custom tags that are not thinking levels should not be stripped
        assert_eq!(
            parse_candidate("provider/model:latest"),
            ("provider".to_string(), "model:latest".to_string(), None)
        );
        assert_eq!(
            parse_candidate("provider/model:v1"),
            ("provider".to_string(), "model:v1".to_string(), None)
        );

        // Whitespace trimming
        assert_eq!(
            parse_candidate("  google-antigravity/gemini-3.8-flash:high  "),
            (
                "google-antigravity".to_string(),
                "gemini-3.8-flash".to_string(),
                Some("high".to_string())
            )
        );

        // No provider slash
        assert_eq!(
            parse_candidate("claude-sonnet:low"),
            ("".to_string(), "claude-sonnet".to_string(), Some("low".to_string()))
        );
    }

    #[test]
    fn test_split_candidates() {
        assert_eq!(
            split_candidates("cand1, cand2, cand3"),
            vec!["cand1", "cand2", "cand3"]
        );
        assert_eq!(
            split_candidates("  cand1 ,   cand2  "),
            vec!["cand1", "cand2"]
        );
        assert_eq!(split_candidates("single"), vec!["single"]);
        assert_eq!(split_candidates(""), Vec::<&str>::new());
        assert_eq!(split_candidates("   "), Vec::<&str>::new());
        assert_eq!(
            split_candidates("cand1, , cand2"),
            vec!["cand1", "cand2"]
        );
    }

    #[test]
    fn test_resolve_role_alias_direct_and_hops() {
        let mut roles = HashMap::new();
        roles.insert("default".to_string(), "anthropic/claude-3-5-sonnet".to_string());
        roles.insert("slow".to_string(), "anthropic/claude-opus-5-5:medium".to_string());
        roles.insert("plan".to_string(), "@slow".to_string());
        roles.insert("task".to_string(), "@plan".to_string());

        // Direct
        assert_eq!(
            resolve_role_alias(&roles, "default").unwrap(),
            "anthropic/claude-3-5-sonnet"
        );
        // Single hop
        assert_eq!(
            resolve_role_alias(&roles, "plan").unwrap(),
            "anthropic/claude-opus-5-5:medium"
        );
        // Multi hop
        assert_eq!(
            resolve_role_alias(&roles, "task").unwrap(),
            "anthropic/claude-opus-5-5:medium"
        );
        // Prefixed with @ in query
        assert_eq!(
            resolve_role_alias(&roles, "@plan").unwrap(),
            "anthropic/claude-opus-5-5:medium"
        );
    }

    #[test]
    fn test_resolve_role_alias_cycle_detection() {
        let mut roles = HashMap::new();
        roles.insert("a".to_string(), "@a".to_string());
        assert!(resolve_role_alias(&roles, "a").is_err());

        let mut cycle2 = HashMap::new();
        cycle2.insert("a".to_string(), "@b".to_string());
        cycle2.insert("b".to_string(), "@a".to_string());
        assert!(resolve_role_alias(&cycle2, "a").is_err());

        let mut cycle3 = HashMap::new();
        cycle3.insert("a".to_string(), "@b".to_string());
        cycle3.insert("b".to_string(), "@c".to_string());
        cycle3.insert("c".to_string(), "@a".to_string());
        let err = resolve_role_alias(&cycle3, "a").unwrap_err();
        assert!(err.contains("Cycle detected"));
    }

    #[test]
    fn test_resolve_role_alias_missing() {
        let mut roles = HashMap::new();
        roles.insert("default".to_string(), "model".to_string());
        roles.insert("broken".to_string(), "@nonexistent".to_string());

        assert!(resolve_role_alias(&roles, "nonexistent").is_err());
        let err = resolve_role_alias(&roles, "broken").unwrap_err();
        assert!(err.contains("Role not found"));
    }
}
