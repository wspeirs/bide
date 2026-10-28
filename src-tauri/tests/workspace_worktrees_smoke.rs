use std::path::{Path, PathBuf};
use bide_lib::{
    git,
    projects::ProjectsState,
    workspace,
};

fn run_git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("git execution failed");
    assert!(status.success(), "git {:?} in {:?} failed", args, dir);
}

fn has_omp() -> bool {
    std::process::Command::new("omp")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn smoke_test_workspace_config_and_worktrees() {
    if !has_omp() {
        eprintln!("omp binary not found in PATH; skipping smoke test");
        return;
    }
    let tmp = tempfile::tempdir().expect("create temp dir");
    let repo_dir = tmp.path().join("wt-smoke");
    std::fs::create_dir_all(&repo_dir).expect("create repo dir");

    run_git(&repo_dir, &["init", "-q", "-b", "main"]);
    run_git(&repo_dir, &["config", "user.email", "test@example.com"]);
    run_git(&repo_dir, &["config", "user.name", "Test User"]);
    run_git(&repo_dir, &["commit", "-q", "--allow-empty", "-m", "init"]);

    let canonical_repo = std::fs::canonicalize(&repo_dir).expect("canonicalize repo");
    let state = ProjectsState::new();

    // 1. Open workspace
    let info = state.open(&canonical_repo.to_string_lossy()).await.expect("open repo");
    assert_eq!(info.dir, canonical_repo.to_string_lossy());

    // 2. Initial config check
    let (key, is_git) = workspace::repo_key(&canonical_repo).await;
    assert!(is_git);
    let store_dir = workspace::store_dir();
    let store_file = workspace::store_path(&store_dir, &key);
    // Cleanup any pre-existing store file for this test key
    let _ = std::fs::remove_file(&store_file);

    // 3. Set config: "defaultThinkingLevel: low"
    let norm_text = workspace::normalize("defaultThinkingLevel: low").expect("normalize");
    std::fs::create_dir_all(&store_dir).expect("create store dir");
    std::fs::write(&store_file, &norm_text).expect("write store file");

    // Sync to workspace
    let warning = workspace::sync(&canonical_repo, &store_dir).await.expect("sync");
    assert!(warning.is_none());

    // Verify .omp/config.yml exists with content
    let ws_omp_cfg = canonical_repo.join(".omp").join("config.yml");
    assert!(ws_omp_cfg.exists());
    let cfg_content = std::fs::read_to_string(&ws_omp_cfg).expect("read ws cfg");
    assert_eq!(cfg_content, "defaultThinkingLevel: low\n");

    // Verify git ignores .omp/config.yml
    let ignore_status = std::process::Command::new("git")
        .args(["check-ignore", "-q", ".omp/config.yml"])
        .current_dir(&canonical_repo)
        .status()
        .expect("git check-ignore");
    assert!(ignore_status.success(), ".omp/config.yml should be ignored");

    // Verify git status is clean
    let git_st = git::git_status(&canonical_repo).await.expect("git status");
    assert!(git_st.untracked.is_empty(), "No untracked files expected");

    // 4. Worktree operations: list, add, open
    let wts = git::git_worktree_list(&canonical_repo).await.expect("list wts");
    assert_eq!(wts.len(), 1);
    assert!(wts[0].is_main);
    assert!(wts[0].is_current);

    // Add worktree for feat/a
    let wt_path_str = git::git_worktree_add(&canonical_repo, "feat/a", true, None)
        .await
        .expect("add worktree");
    let wt_path = PathBuf::from(&wt_path_str);
    assert!(wt_path.to_string_lossy().ends_with("wt-smoke.worktrees/feat-a"));

    // Open worktree in bide ProjectsState
    let wt_info = state.open(&wt_path_str).await.expect("open worktree in bide");
    assert_eq!(wt_info.dir, wt_path_str);

    // Verify .omp/config.yml was synced into the worktree
    let wt_omp_cfg = wt_path.join(".omp").join("config.yml");
    assert!(wt_omp_cfg.exists(), "Worktree must have synced .omp/config.yml");
    let wt_cfg_content = std::fs::read_to_string(&wt_omp_cfg).expect("read wt cfg");
    assert_eq!(wt_cfg_content, "defaultThinkingLevel: low\n");

    // Verify worktree git status is clean
    let wt_git_st = git::git_status(&wt_path).await.expect("wt git status");
    assert!(wt_git_st.untracked.is_empty());

    // 5. Hand-edit mtime test:
    // Update main workspace .omp/config.yml to defaultThinkingLevel: high
    // with modified time set newer than store
    std::fs::write(&ws_omp_cfg, "defaultThinkingLevel: high\n").expect("write high");
    let now = std::time::SystemTime::now();
    let later = now + std::time::Duration::from_secs(5);
    let ws_f = std::fs::File::open(&ws_omp_cfg).expect("open ws cfg");
    ws_f.set_modified(later).expect("set modified");

    // Trigger sync
    workspace::sync(&canonical_repo, &store_dir).await.expect("sync after edit");

    // Store should now have updated content
    let store_content = std::fs::read_to_string(&store_file).expect("read store");
    assert_eq!(store_content, "defaultThinkingLevel: high\n");

    // 6. Worktree remove
    // Close the project tab for wt
    state.close(&wt_info.id).await.expect("close wt tab");
    state.close(&info.id).await.expect("close main tab");
    state.shutdown_all().await;

    // Remove worktree
    git::git_worktree_remove(&canonical_repo, &wt_path_str, false)
        .await
        .expect("remove worktree");
    assert!(!wt_path.exists(), "Worktree directory should be deleted");

    // Verify branch feat/a still exists
    let branches = git::git_branches(&canonical_repo).await.expect("list branches");
    assert!(branches.local.contains(&"feat/a".to_string()));

    // Clean up store file
    let _ = std::fs::remove_file(&store_file);
    if let Some(parent) = wt_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
