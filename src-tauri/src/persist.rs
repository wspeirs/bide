use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaneSizesConfig {
    pub left: Vec<f64>,
    pub git: f64,
}

impl Default for PaneSizesConfig {
    fn default() -> Self {
        Self {
            left: vec![15.0, 60.0, 25.0],
            git: 22.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TabConfig {
    pub id: String,
    pub dir: String,
    pub session_file: Option<String>,
    #[serde(default)]
    pub orchestrate: bool,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub advisor: bool,
}

fn default_role() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    pub active_tab: Option<String>,
    #[serde(default)]
    pub pane_sizes: PaneSizesConfig,
    #[serde(default)]
    pub tabs: Vec<TabConfig>,
}

fn default_version() -> u32 {
    1
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            version: 1,
            active_tab: None,
            pane_sizes: PaneSizesConfig::default(),
            tabs: Vec::new(),
        }
    }
}

/// Returns ~/.config/bide/session.json
pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".config").join("bide").join("session.json"))
        .unwrap_or_else(|| PathBuf::from(".config/bide/session.json"))
}

fn backup_path(path: &Path) -> PathBuf {
    match path.file_name() {
        Some(name) => {
            let mut bak_name = name.to_os_string();
            bak_name.push(".bak");
            path.with_file_name(bak_name)
        }
        None => path.with_extension("bak"),
    }
}

/// Writes config to target path atomically via a `.tmp` file and rename.
pub fn save_to(path: &Path, config: &SessionConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let tmp_path = match path.file_name() {
        Some(name) => {
            let mut tmp_name = name.to_os_string();
            tmp_name.push(".tmp");
            path.with_file_name(tmp_name)
        }
        None => path.with_extension("tmp"),
    };

    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&tmp_path, json).map_err(|e| e.to_string())?;

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.to_string());
    }

    Ok(())
}

/// Writes to session.json atomically via `.tmp` file and rename.
pub fn save(config: &SessionConfig) -> Result<(), String> {
    save_to(&config_path(), config)
}

/// Loads session configuration from specified path.
/// Missing file -> returns default.
/// Corrupt file -> backs up to `<path>.bak` and returns default.
pub fn load_from(path: &Path) -> SessionConfig {
    if !path.exists() {
        return SessionConfig::default();
    }

    let data = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => {
            let bak = backup_path(path);
            let _ = fs::copy(path, bak);
            return SessionConfig::default();
        }
    };

    match serde_json::from_str::<SessionConfig>(&data) {
        Ok(cfg) => cfg,
        Err(_) => {
            let bak = backup_path(path);
            let _ = fs::copy(path, bak);
            SessionConfig::default()
        }
    }
}

/// Loads from config_path(). Returns default if missing or corrupt (after backup).
pub fn load() -> SessionConfig {
    load_from(&config_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_round_trip_save_and_load() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("session.json");

        let config = SessionConfig {
            version: 1,
            active_tab: Some("tab-abc".to_string()),
            pane_sizes: PaneSizesConfig {
                left: vec![20.0, 50.0, 30.0],
                git: 25.5,
            },
            tabs: vec![
                TabConfig {
                    id: "tab-abc".to_string(),
                    dir: "/workspace/proj".to_string(),
                    session_file: Some("/workspace/.omp/sessions/s1.jsonl".to_string()),
                    orchestrate: true,
                    role: "planner".to_string(),
                    advisor: true,
                },
                TabConfig {
                    id: "tab-def".to_string(),
                    dir: "/workspace/proj2".to_string(),
                    session_file: None,
                    orchestrate: false,
                    role: "default".to_string(),
                    advisor: false,
                },
            ],
        };

        save_to(&path, &config).expect("save_to should succeed");
        assert!(path.exists());

        let loaded = load_from(&path);
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_missing_file_returns_default() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("nonexistent.json");

        assert!(!path.exists());
        let loaded = load_from(&path);
        assert_eq!(loaded, SessionConfig::default());
        assert!(!dir.path().join("nonexistent.json.bak").exists());
    }

    #[test]
    fn test_corrupt_file_copies_to_bak_and_returns_default() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("session.json");
        let bak_path = dir.path().join("session.json.bak");

        let corrupt_data = "{ \"version\": \"not_a_number\", broken json }";
        fs::write(&path, corrupt_data).expect("write corrupt file");

        let loaded = load_from(&path);
        assert_eq!(loaded, SessionConfig::default());

        assert!(bak_path.exists(), "backup file must exist");
        let bak_content = fs::read_to_string(&bak_path).expect("read bak");
        assert_eq!(bak_content, corrupt_data);
    }

    #[test]
    fn test_save_atomic_creates_dir_and_no_leftover_tmp() {
        let dir = tempdir().expect("tempdir");
        let nested_path = dir.path().join("nested").join("deep").join("session.json");
        let tmp_path = dir.path().join("nested").join("deep").join("session.json.tmp");

        let config = SessionConfig::default();
        save_to(&nested_path, &config).expect("save_to in non-existent directory");

        assert!(nested_path.exists());
        assert!(!tmp_path.exists(), "tmp file must be cleaned up by rename");
    }

    #[test]
    fn test_default_deserialization() {
        let raw = r#"{
            "activeTab": "tab-1",
            "tabs": [
                { "id": "tab-1", "dir": "/tmp" }
            ]
        }"#;

        let cfg: SessionConfig = serde_json::from_str(raw).expect("deserialize partial");
        assert_eq!(cfg.version, 1);
        assert_eq!(cfg.active_tab.as_deref(), Some("tab-1"));
        assert_eq!(cfg.pane_sizes, PaneSizesConfig::default());
        assert_eq!(cfg.tabs.len(), 1);
        assert_eq!(cfg.tabs[0].id, "tab-1");
        assert_eq!(cfg.tabs[0].dir, "/tmp");
        assert_eq!(cfg.tabs[0].session_file, None);
        assert!(!cfg.tabs[0].orchestrate);
        assert_eq!(cfg.tabs[0].role, "default");
        assert!(!cfg.tabs[0].advisor);
    }

    #[test]
    fn test_config_path() {
        let path = config_path();
        assert!(path.to_string_lossy().ends_with(".config/bide/session.json"));
    }
}
