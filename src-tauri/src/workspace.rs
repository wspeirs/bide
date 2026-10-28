use std::path::{Path, PathBuf};
use serde::Serialize;

pub const REL: &str = ".omp/config.yml";
pub const TRACKED_WARNING: &str =
    ".omp/config.yml is tracked by git in this repo; bide will not read or write it.";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub text: String,
    pub source: &'static str, // "workspace" | "global" | "none"
    pub warning: Option<String>,
}

pub fn store_dir() -> PathBuf {
    crate::persist::config_path().with_file_name("workspaces")
}

pub async fn repo_key(dir: &Path) -> (String, bool) {
    match crate::git::common_dir(dir).await {
        Ok(p) => (p.to_string_lossy().into_owned(), true),
        Err(_) => (
            std::fs::canonicalize(dir)
                .unwrap_or_else(|_| dir.to_path_buf())
                .to_string_lossy()
                .into_owned(),
            false,
        ),
    }
}

pub fn store_path(store_dir: &Path, key: &str) -> PathBuf {
    store_dir.join(format!(
        "{}.yml",
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, key.as_bytes())
    ))
}

pub fn sync_files(ws: &Path, store: &Path) -> std::io::Result<()> {
    let ws_exists = ws.exists();
    let store_exists = store.exists();

    match (ws_exists, store_exists) {
        (false, false) => Ok(()),
        (true, false) => {
            if let Some(p) = store.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(ws, store)?;
            Ok(())
        }
        (false, true) => {
            if let Some(p) = ws.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(store, ws)?;
            Ok(())
        }
        (true, true) => {
            let ws_bytes = std::fs::read(ws)?;
            let store_bytes = std::fs::read(store)?;
            if ws_bytes == store_bytes {
                return Ok(());
            }

            let ws_mtime = std::fs::metadata(ws)?.modified()?;
            let store_mtime = std::fs::metadata(store)?.modified()?;

            if ws_mtime > store_mtime {
                std::fs::copy(ws, store)?;
            } else {
                std::fs::copy(store, ws)?;
            }
            Ok(())
        }
    }
}

pub async fn sync(dir: &Path, store_dir: &Path) -> Result<Option<String>, String> {
    let (key, is_git) = repo_key(dir).await;
    if is_git && crate::git::is_tracked(dir, REL).await {
        return Ok(Some(TRACKED_WARNING.to_string()));
    }
    let ws_file = dir.join(REL);
    let s_file = store_path(store_dir, &key);
    sync_files(&ws_file, &s_file).map_err(|e| e.to_string())?;
    if is_git && ws_file.exists() {
        crate::git::ensure_excluded(dir, REL)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(None)
}

pub fn normalize(text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok("{}\n".to_string());
    }
    let val = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(text)
        .map_err(|e| format!("Invalid YAML: {e}"))?;
    if !val.is_mapping() {
        return Err("Config must be a YAML mapping (key: value pairs) at the top level".to_string());
    }
    if text.ends_with('\n') {
        Ok(text.to_string())
    } else {
        Ok(format!("{text}\n"))
    }
}

fn try_read_config_file(agent_dir: &Path) -> Option<String> {
    let yml = agent_dir.join("config.yml");
    if let Ok(content) = std::fs::read_to_string(&yml) {
        return Some(content);
    }
    let yaml = agent_dir.join("config.yaml");
    if let Ok(content) = std::fs::read_to_string(&yaml) {
        return Some(content);
    }
    None
}

pub async fn read_global() -> Option<String> {
    let out = tokio::process::Command::new("omp")
        .args(["config", "path"])
        .stdin(std::process::Stdio::null())
        .output()
        .await;

    if let Ok(out) = out {
        if out.status.success() {
            let dir_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !dir_str.is_empty() {
                let agent_dir = PathBuf::from(dir_str);
                if let Some(content) = try_read_config_file(&agent_dir) {
                    return Some(content);
                }
            }
        }
    }

    if let Some(agent_dir) = dirs::home_dir().map(|h| h.join(".omp").join("agent")) {
        if let Some(content) = try_read_config_file(&agent_dir) {
            return Some(content);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_normalize_empty_and_whitespace() {
        assert_eq!(normalize("").unwrap(), "{}\n");
        assert_eq!(normalize("   \n\t\n  ").unwrap(), "{}\n");
    }

    #[test]
    fn test_normalize_invalid_yaml() {
        let err = normalize("foo: [bar").unwrap_err();
        assert!(err.starts_with("Invalid YAML:"));
    }

    #[test]
    fn test_normalize_non_mapping() {
        let err = normalize("- item1\n- item2").unwrap_err();
        assert_eq!(
            err,
            "Config must be a YAML mapping (key: value pairs) at the top level"
        );

        let err2 = normalize("just a string").unwrap_err();
        assert_eq!(
            err2,
            "Config must be a YAML mapping (key: value pairs) at the top level"
        );
    }

    #[test]
    fn test_normalize_valid_mapping() {
        let yml = "model: gpt-4\nauto_compact: true";
        assert_eq!(normalize(yml).unwrap(), format!("{yml}\n"));

        let yml_newline = "model: gpt-4\nauto_compact: true\n";
        assert_eq!(normalize(yml_newline).unwrap(), yml_newline);
    }

    #[test]
    fn test_store_dir_and_path() {
        let dir = store_dir();
        assert!(dir.to_string_lossy().ends_with(".config/bide/workspaces"));

        let path = store_path(&dir, "some/key");
        assert!(path.to_string_lossy().ends_with(".yml"));
        let path2 = store_path(&dir, "some/key");
        assert_eq!(path, path2);
    }

    #[test]
    fn test_sync_files() {
        let dir = tempdir().unwrap();
        let ws = dir.path().join("ws").join(".omp").join("config.yml");
        let store = dir.path().join("store").join("workspaces").join("key.yml");

        // Neither exists
        sync_files(&ws, &store).unwrap();
        assert!(!ws.exists());
        assert!(!store.exists());

        // Only ws exists
        std::fs::create_dir_all(ws.parent().unwrap()).unwrap();
        std::fs::write(&ws, "ws content\n").unwrap();
        sync_files(&ws, &store).unwrap();
        assert!(store.exists());
        assert_eq!(std::fs::read_to_string(&store).unwrap(), "ws content\n");

        // Equal bytes
        let ws_meta = std::fs::metadata(&ws).unwrap();
        let store_meta = std::fs::metadata(&store).unwrap();
        sync_files(&ws, &store).unwrap();
        assert_eq!(
            std::fs::metadata(&ws).unwrap().modified().unwrap(),
            ws_meta.modified().unwrap()
        );
        assert_eq!(
            std::fs::metadata(&store).unwrap().modified().unwrap(),
            store_meta.modified().unwrap()
        );

        // Store newer
        std::thread::sleep(std::time::Duration::from_millis(15));
        std::fs::write(&store, "store content newer\n").unwrap();
        sync_files(&ws, &store).unwrap();
        assert_eq!(
            std::fs::read_to_string(&ws).unwrap(),
            "store content newer\n"
        );

        // ws newer
        std::thread::sleep(std::time::Duration::from_millis(15));
        std::fs::write(&ws, "ws content newer\n").unwrap();
        sync_files(&ws, &store).unwrap();
        assert_eq!(
            std::fs::read_to_string(&store).unwrap(),
            "ws content newer\n"
        );
    }

    #[test]
    fn normalize_validates() {
        assert_eq!(normalize("  \n").unwrap(), "{}\n");
        assert_eq!(
            normalize("- a\n").unwrap_err(),
            "Config must be a YAML mapping (key: value pairs) at the top level"
        );
        assert!(normalize("a: [\n").unwrap_err().starts_with("Invalid YAML"));
        assert_eq!(normalize("a: 1").unwrap(), "a: 1\n");
    }

    #[test]
    fn sync_files_last_writer_wins() {
        let dir = tempdir().unwrap();
        let ws = dir.path().join("ws").join(".omp").join("config.yml");
        let store = dir.path().join("store").join("workspaces").join("key.yml");

        // only ws -> store created with ws bytes
        std::fs::create_dir_all(ws.parent().unwrap()).unwrap();
        std::fs::write(&ws, "ws content").unwrap();
        sync_files(&ws, &store).unwrap();
        assert_eq!(std::fs::read_to_string(&store).unwrap(), "ws content");

        // only store -> ws created
        let dir2 = tempdir().unwrap();
        let ws2 = dir2.path().join("ws2").join(".omp").join("config.yml");
        let store2 = dir2.path().join("store2").join("workspaces").join("key.yml");
        std::fs::create_dir_all(store2.parent().unwrap()).unwrap();
        std::fs::write(&store2, "store content").unwrap();
        sync_files(&ws2, &store2).unwrap();
        assert_eq!(std::fs::read_to_string(&ws2).unwrap(), "store content");

        // both differ with ws newer -> store gets ws bytes
        std::fs::write(&ws2, "ws2 newer").unwrap();
        let now = std::time::SystemTime::now();
        let later = now + std::time::Duration::from_secs(10);
        let file_ws = std::fs::File::open(&ws2).unwrap();
        file_ws.set_modified(later).unwrap();
        sync_files(&ws2, &store2).unwrap();
        assert_eq!(std::fs::read_to_string(&store2).unwrap(), "ws2 newer");

        // both differ with store newer -> ws gets store bytes
        std::fs::write(&store2, "store2 even newer").unwrap();
        let much_later = later + std::time::Duration::from_secs(10);
        let file_store = std::fs::File::open(&store2).unwrap();
        file_store.set_modified(much_later).unwrap();
        sync_files(&ws2, &store2).unwrap();
        assert_eq!(std::fs::read_to_string(&ws2).unwrap(), "store2 even newer");

        // equal bytes -> ws mtime unchanged
        let ws_mtime_before = std::fs::metadata(&ws2).unwrap().modified().unwrap();
        sync_files(&ws2, &store2).unwrap();
        let ws_mtime_after = std::fs::metadata(&ws2).unwrap().modified().unwrap();
        assert_eq!(ws_mtime_before, ws_mtime_after);
    }

    #[tokio::test]
    async fn sync_git_lifecycle_and_worktree_sharing() {
        let tmp = tempdir().unwrap();
        let repo_dir = tmp.path().join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        let store_dir = tmp.path().join("store");
        std::fs::create_dir_all(&store_dir).unwrap();

        let run_cmd = |dir: &Path, args: &[&str]| {
            let st = std::process::Command::new("git")
                .args(args)
                .current_dir(dir)
                .status()
                .unwrap();
            assert!(st.success());
        };

        run_cmd(&repo_dir, &["init", "-q", "-b", "main"]);
        run_cmd(&repo_dir, &["config", "user.email", "a@b.com"]);
        run_cmd(&repo_dir, &["config", "user.name", "Test"]);
        std::fs::write(repo_dir.join("a"), "hello").unwrap();
        run_cmd(&repo_dir, &["add", "a"]);
        run_cmd(&repo_dir, &["commit", "-m", "init"]);

        // 1. Untracked .omp/config.yml: sync adds exclude entry and writes store
        let ws_cfg = repo_dir.join(REL);
        std::fs::create_dir_all(ws_cfg.parent().unwrap()).unwrap();
        std::fs::write(&ws_cfg, "foo: bar\n").unwrap();

        let warn = sync(&repo_dir, &store_dir).await.unwrap();
        assert!(warn.is_none());

        let (key, is_git) = repo_key(&repo_dir).await;
        assert!(is_git);
        let s_path = store_path(&store_dir, &key);
        assert!(s_path.exists());
        assert_eq!(std::fs::read_to_string(&s_path).unwrap(), "foo: bar\n");

        // Verify git check-ignore returns success (it is excluded)
        let check_ignore = std::process::Command::new("git")
            .args(["check-ignore", "-q", REL])
            .current_dir(&repo_dir)
            .status()
            .unwrap();
        assert!(check_ignore.success());

        // 2. A worktree shares the same repo key, so syncing worktree gets the store config
        let wt_path = crate::git::git_worktree_add(&repo_dir, "wt-branch", true, None)
            .await
            .unwrap();
        let wt_p = PathBuf::from(&wt_path);

        let (wt_key, wt_is_git) = repo_key(&wt_p).await;
        assert!(wt_is_git);
        assert_eq!(key, wt_key); // Shared repo key!

        // Sync worktree -> gets "foo: bar\n" from store
        let wt_warn = sync(&wt_p, &store_dir).await.unwrap();
        assert!(wt_warn.is_none());
        assert!(wt_p.join(REL).exists());
        assert_eq!(
            std::fs::read_to_string(wt_p.join(REL)).unwrap(),
            "foo: bar\n"
        );

        // 3. Tracked .omp/config.yml returns TRACKED_WARNING and does not touch or overwrite
        let repo_tracked = tmp.path().join("repo-tracked");
        std::fs::create_dir_all(&repo_tracked).unwrap();
        run_cmd(&repo_tracked, &["init", "-q", "-b", "main"]);
        run_cmd(&repo_tracked, &["config", "user.email", "a@b.com"]);
        run_cmd(&repo_tracked, &["config", "user.name", "Test"]);

        let tracked_cfg = repo_tracked.join(REL);
        std::fs::create_dir_all(tracked_cfg.parent().unwrap()).unwrap();
        std::fs::write(&tracked_cfg, "original: tracked\n").unwrap();
        run_cmd(&repo_tracked, &["add", REL]);
        run_cmd(&repo_tracked, &["commit", "-m", "track omp cfg"]);

        let (t_key, _) = repo_key(&repo_tracked).await;
        let t_store_file = store_path(&store_dir, &t_key);
        std::fs::write(&t_store_file, "store: should_not_overwrite\n").unwrap();

        let t_warn = sync(&repo_tracked, &store_dir).await.unwrap();
        assert_eq!(t_warn.as_deref(), Some(TRACKED_WARNING));
        // Verify tracked file was not overwritten
        assert_eq!(
            std::fs::read_to_string(&tracked_cfg).unwrap(),
            "original: tracked\n"
        );
        // Verify store was not overwritten
        assert_eq!(
            std::fs::read_to_string(&t_store_file).unwrap(),
            "store: should_not_overwrite\n"
        );

        // Clean up worktree sibling
        let _ = crate::git::git_worktree_remove(&repo_dir, &wt_path, true).await;
        if let Some(parent) = wt_p.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}
