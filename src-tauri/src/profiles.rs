use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{persist, roles, workspace};

const PROFILE_THINKING_LEVELS: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh", "max", "auto"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelProfile {
    pub model_roles: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thinking_level: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_fallbacks: HashMap<String, String>,
}

type Profiles = BTreeMap<String, ModelProfile>;

#[derive(Debug, Serialize)]
pub struct ModelProfileScopes {
    pub global: BTreeMap<String, ModelProfile>,
    pub local: BTreeMap<String, ModelProfile>,
}

impl ModelProfileScopes {
    pub fn resolve(&self, name: &str) -> Option<&ModelProfile> {
        self.local.get(name).or_else(|| self.global.get(name))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelProfileScope {
    Global,
    Local,
}

static STORE_LOCK: Mutex<()> = Mutex::new(());

fn global_path() -> PathBuf {
    persist::config_path().with_file_name("model-profiles.json")
}

async fn local_path(dir: &Path) -> PathBuf {
    let (key, _) = workspace::repo_key(dir).await;
    workspace::store_path(&workspace::store_dir(), &key).with_extension("model-profiles.json")
}

pub async fn get(dir: Option<&Path>) -> Result<ModelProfileScopes, String> {
    let local = match dir {
        Some(dir) => Some(local_path(dir).await),
        None => None,
    };
    read_scopes(&global_path(), local.as_deref())
}

pub async fn save(
    dir: Option<&Path>,
    scope: ModelProfileScope,
    name: &str,
    profile: Option<ModelProfile>,
) -> Result<(), String> {
    let path = match scope {
        ModelProfileScope::Global => global_path(),
        ModelProfileScope::Local => {
            local_path(dir.ok_or("Local model profiles require a workspace")?).await
        }
    };
    save_to(&path, name, profile)
}

fn read_scopes(global: &Path, local: Option<&Path>) -> Result<ModelProfileScopes, String> {
    Ok(ModelProfileScopes {
        global: read(global)?,
        local: match local {
            Some(path) => read(path)?,
            None => Profiles::new(),
        },
    })
}

fn read(path: &Path) -> Result<Profiles, String> {
    let error = |e: io::Error| format!("Failed to read model profiles at {}: {e}", path.display());
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => match fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Profiles::new()),
            _ => return Err(error(e)),
        },
        Err(e) => return Err(error(e)),
    };
    let invalid = |e| format!("Invalid model profiles at {}: {e}", path.display());
    let profiles: Profiles = serde_json::from_slice(&data).map_err(|e| invalid(e.to_string()))?;
    for (name, profile) in &profiles {
        validate_name(name).map_err(&invalid)?;
        validate_profile(profile).map_err(|e| invalid(format!("{name:?}: {e}")))?;
    }
    Ok(profiles)
}

fn save_to(path: &Path, name: &str, profile: Option<ModelProfile>) -> Result<(), String> {
    validate_name(name)?;
    if let Some(profile) = &profile {
        validate_profile(profile)?;
    }
    let _lock = STORE_LOCK.lock().map_err(|e| e.to_string())?;
    let mut profiles = read(path)?;
    match profile {
        Some(profile) => {
            profiles.insert(name.to_owned(), profile);
        }
        None if profiles.remove(name).is_some() => {}
        None => return Ok(()),
    }
    let mut data = serde_json::to_vec_pretty(&profiles).map_err(|e| e.to_string())?;
    data.push(b'\n');
    write_atomic(path, &data)
        .map_err(|e| format!("Failed to write model profiles at {}: {e}", path.display()))
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    // Atomic replacement can bypass the old file's write permissions. Respect them.
    match OpenOptions::new().write(true).open(path) {
        Ok(file) if file.metadata()?.permissions().readonly() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "store is read-only",
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let parent = path.parent().expect("profile store path has a parent");
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".model-profiles-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    let result = (|| {
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
        return Err("Profile name must be nonempty, trimmed, and free of control characters".into());
    }
    if name.eq_ignore_ascii_case(roles::CUSTOM_ROLE) {
        return Err("Custom is a reserved profile name".into());
    }
    Ok(())
}

fn valid_token(value: &str, extra: &[u8]) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric() || extra.contains(&b))
}

fn valid_selector(selector: &str) -> bool {
    let Some((provider, model)) = selector.split_once('/') else {
        return false;
    };
    // Colons also occur in real model tags (e.g. llama3:8b).
    let model = match model.rsplit_once(':') {
        Some((id, level)) if PROFILE_THINKING_LEVELS.contains(&level) => id,
        _ => model,
    };
    valid_token(provider, b"-_")
        && model.split('/').all(|part| match part.split_once(':') {
            Some((id, tag)) => valid_token(id, b"-_.@+") && valid_token(tag, b"-_.@+"),
            None => valid_token(part, b"-_.@+"),
        })
}

fn validate_profile(profile: &ModelProfile) -> Result<(), String> {
    if !profile.model_roles.contains_key("default") {
        return Err("Model profiles require a default role".into());
    }
    for (role, selector) in &profile.model_roles {
        if !valid_token(role, b"-_") || !role.bytes().any(|b| b.is_ascii_alphanumeric()) {
            return Err(format!("Invalid model role name {role:?}"));
        }
        if !valid_selector(selector) {
            return Err(format!("Invalid model selector for {role:?}: expected provider/model[:thinking]"));
        }
    }
    for (role, selector) in &profile.model_fallbacks {
        if !profile.model_roles.contains_key(role) {
            return Err(format!("Fallback for {role:?} requires a primary model"));
        }
        if !valid_selector(selector) {
            return Err(format!("Invalid fallback model selector for {role:?}: expected provider/model[:thinking]"));
        }
    }
    if let Some(level) = &profile.default_thinking_level {
        if !PROFILE_THINKING_LEVELS.contains(&level.as_str()) {
            return Err(format!("Invalid default thinking level {level:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_profile_shadows_whole_global_profile_until_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let global = dir.path().join("model-profiles.json");
        let local = workspace::store_path(dir.path(), "repo").with_extension("model-profiles.json");
        let global_profile = ModelProfile {
            model_roles: HashMap::from([
                ("default".into(), "ollama/llama3:8b:high".into()),
                ("advisor".into(), "anthropic/claude-sonnet-4-6".into()),
            ]),
            default_thinking_level: Some("high".into()),
            model_fallbacks: HashMap::from([("default".into(), "openrouter/org/model:free:low".into())]),
        };
        let local_profile = ModelProfile {
            model_roles: HashMap::from([("default".into(), "openrouter/org/model:free".into())]),
            default_thinking_level: None,
            model_fallbacks: HashMap::new(),
        };
        save_to(&global, "Work", Some(global_profile.clone())).unwrap();
        save_to(&local, "Work", Some(local_profile.clone())).unwrap();
        let scopes = read_scopes(&global, Some(&local)).unwrap();
        assert_eq!(scopes.resolve("Work"), Some(&local_profile));
        save_to(&local, "Work", None).unwrap();
        let scopes = read_scopes(&global, Some(&local)).unwrap();
        assert_eq!(scopes.resolve("Work"), Some(&global_profile));

        let corrupt = br#"{"Work":{"modelRoles":{}}}"#;
        fs::write(&local, corrupt).unwrap();
        assert!(save_to(&local, "Work", None).is_err());
        assert_eq!(fs::read(&local).unwrap().as_slice(), corrupt);
    }
}
