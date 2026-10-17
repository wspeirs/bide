use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::omp::OmpSession;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub name: String,
    pub selector: String,
}

pub const VALID_THINKING_LEVELS: &[&str] = &[
    "off", "minimal", "low", "medium", "high", "xhigh", "max", "adaptive",
];

pub const HIDDEN_ROLES: &[&str] = &[
    "image", "web", "speech", "dictation", "judge", "tiny", "memory",
];

pub const CHAT_ROLE_ORDER: &[&str] = &[
    "default", "smol", "slow", "plan", "task", "commit", "vision", "advisor",
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

/// Filters out kind roles (`image`, `web`, `speech`, `dictation`, `judge`)
/// and background roles (`tiny`, `memory`).
/// Returns list of chat roles (e.g. `default`, `smol`, `slow`, `plan`, `task`, `commit`, `vision`, `advisor` and any custom role).
pub fn get_chat_roles(model_roles: &HashMap<String, String>) -> Vec<RoleInfo> {
    let mut roles: Vec<RoleInfo> = model_roles
        .iter()
        .filter(|(name, _)| !HIDDEN_ROLES.contains(&name.as_str()))
        .map(|(name, selector)| RoleInfo {
            name: name.clone(),
            selector: selector.clone(),
        })
        .collect();

    roles.sort_by(|a, b| {
        let pos_a = CHAT_ROLE_ORDER.iter().position(|&r| r == a.name);
        let pos_b = CHAT_ROLE_ORDER.iter().position(|&r| r == b.name);
        match (pos_a, pos_b) {
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });

    roles
}

/// Runs `omp config get modelRoles --json` with `cwd` = project dir to get model roles map.
pub async fn get_model_roles(cwd: &Path) -> Result<HashMap<String, String>, String> {
    let output = Command::new("omp")
        .args(["config", "get", "modelRoles", "--json"])
        .current_dir(cwd)
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
        let mut model_cmd = serde_json::Map::new();
        model_cmd.insert(
            "type".to_string(),
            serde_json::Value::String("set_model".to_string()),
        );
        model_cmd.insert(
            "provider".to_string(),
            serde_json::Value::String(provider),
        );
        model_cmd.insert(
            "modelId".to_string(),
            serde_json::Value::String(model_id),
        );

        match session.request(model_cmd).await {
            Ok(model_data) => {
                if let Some(level) = thinking_level {
                    let mut think_cmd = serde_json::Map::new();
                    think_cmd.insert(
                        "type".to_string(),
                        serde_json::Value::String("set_thinking_level".to_string()),
                    );
                    think_cmd.insert(
                        "level".to_string(),
                        serde_json::Value::String(level),
                    );
                    if let Err(e) = session.request(think_cmd).await {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_get_chat_roles_filtering_and_order() {
        let mut model_roles = HashMap::new();
        // Chat roles
        model_roles.insert("default".to_string(), "anthropic/claude-3-5:high".to_string());
        model_roles.insert("smol".to_string(), "anthropic/claude-haiku".to_string());
        model_roles.insert("slow".to_string(), "anthropic/claude-opus".to_string());
        model_roles.insert("plan".to_string(), "@slow".to_string());
        model_roles.insert("task".to_string(), "google/gemini".to_string());
        model_roles.insert("commit".to_string(), "google/gemini".to_string());
        model_roles.insert("vision".to_string(), "google/gemini-vision".to_string());
        model_roles.insert("advisor".to_string(), "anthropic/claude".to_string());
        model_roles.insert("custom_role".to_string(), "custom/model".to_string());

        // Kind roles (should be filtered out)
        model_roles.insert("image".to_string(), "openai/dall-e-3".to_string());
        model_roles.insert("web".to_string(), "google/gemini".to_string());
        model_roles.insert("speech".to_string(), "openai/tts".to_string());
        model_roles.insert("dictation".to_string(), "openai/whisper".to_string());
        model_roles.insert("judge".to_string(), "anthropic/claude".to_string());

        // Background roles (should be filtered out)
        model_roles.insert("tiny".to_string(), "anthropic/haiku".to_string());
        model_roles.insert("memory".to_string(), "anthropic/haiku".to_string());

        let chat_roles = get_chat_roles(&model_roles);

        let names: Vec<String> = chat_roles.iter().map(|r| r.name.clone()).collect();

        // Hidden roles must not be present
        for hidden in HIDDEN_ROLES {
            assert!(!names.contains(&hidden.to_string()), "Hidden role '{hidden}' should not be present");
        }

        // Chat roles must be present
        for expected in &[
            "default", "smol", "slow", "plan", "task", "commit", "vision", "advisor", "custom_role",
        ] {
            assert!(names.contains(&expected.to_string()), "Chat role '{expected}' should be present");
        }

        // Check preferred ordering
        let expected_order = vec![
            "default", "smol", "slow", "plan", "task", "commit", "vision", "advisor", "custom_role",
        ];
        assert_eq!(names, expected_order);
    }

    #[test]
    fn test_role_info_serde() {
        let info = RoleInfo {
            name: "default".to_string(),
            selector: "anthropic/claude-3-5:high".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"default\""));
        assert!(json.contains("\"selector\":\"anthropic/claude-3-5:high\""));

        let deserialized: RoleInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, info);
    }
}
