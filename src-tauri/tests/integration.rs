use serde_json::Value;
use tokio::sync::broadcast;
use std::{
    path::Path,
    sync::Arc,
    time::Duration,
};

use bide_lib::{
    omp::OmpSession,
    projects::ProjectsState,
    roles,
};

fn has_omp() -> bool {
    std::process::Command::new("omp")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn test_multi_session_concurrency_and_resume() {
    if !has_omp() {
        eprintln!("omp binary not found in PATH; skipping multi-session integration test");
        return;
    }

    // 1. Create two temp directories
    let dir1 = tempfile::tempdir().expect("failed to create tempdir 1");
    let dir2 = tempfile::tempdir().expect("failed to create tempdir 2");

    // 2. Spawn two OmpSession instances concurrently
    let (s1_res, s2_res) = tokio::join!(
        OmpSession::spawn(dir1.path(), None),
        OmpSession::spawn(dir2.path(), None)
    );

    let session1 = s1_res.expect("spawn session 1 failed");
    let session2 = s2_res.expect("spawn session 2 failed");

    // Subscribe to broadcast on both sessions to receive events
    let rx1 = session1.subscribe_broadcast();
    let rx2 = session2.subscribe_broadcast();

    // 3. Prompt both sessions concurrently
    let p1 = {
        let s1 = Arc::clone(&session1);
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(60), s1.prompt("echo test1", None)).await
        })
    };

    let p2 = {
        let s2 = Arc::clone(&session2);
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(60), s2.prompt("echo test2", None)).await
        })
    };

    let (res1, res2) = tokio::join!(p1, p2);
    let prompt_res_1 = res1.expect("task 1 join failed");
    let prompt_res_2 = res2.expect("task 2 join failed");

    // Verify responses if prompt succeeded (handle network timeouts gracefully)
    if let Ok(Ok(resp1)) = prompt_res_1 {
        assert!(!resp1.is_null());
    }
    if let Ok(Ok(resp2)) = prompt_res_2 {
        assert!(!resp2.is_null());
    }

    // Wait for session_settled frames
    let wait_for_settled = |mut rx: broadcast::Receiver<Value>, id: &'static str| async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            if let Ok(Ok(frame)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                let ftype = frame.get("type").and_then(|t| t.as_str()).unwrap_or("");
                eprintln!("[{id}] frame: {ftype}");
                if ftype == "session_settled" {
                    return true;
                }
            }
        }
        false
    };

    let (s1_settled, s2_settled) = tokio::join!(
        wait_for_settled(rx1, "s1"),
        wait_for_settled(rx2, "s2"),
    );
    eprintln!("s1_settled: {s1_settled}, s2_settled: {s2_settled}");
    assert!(s1_settled, "session 1 should settle");
    assert!(s2_settled, "session 2 should settle");

    let state1 = session1.get_state().await.expect("get_state session 1 failed");
    // 4. Test session shutdown for session 2
    session2.shutdown().await.expect("session 2 shutdown failed");

    // 5. Test resume: take session_file from session 1, spawn new OmpSession with --resume
    let session_file_1 = session1
        .session_file()
        .expect("session 1 session_file should be present");
    assert!(!session_file_1.is_empty(), "session_file must not be empty");
    let sf_path = Path::new(&session_file_1);
    eprintln!("session_file_1 = {:?}", session_file_1);
    assert!(sf_path.exists(), "session file must exist on disk");

    let resumed_session = OmpSession::spawn(dir1.path(), Some(sf_path))
        .await
        .expect("resumed session spawn failed");

    let resumed_state = resumed_session
        .get_state()
        .await
        .expect("get_state on resumed session failed");

    // Verify resumed session points to the same session file
    let resumed_sf = resumed_session.session_file();
    assert_eq!(
        resumed_sf.as_deref(),
        Some(session_file_1.as_str()),
        "Resumed session file must match original session file"
    );

    // Verify sessionId matches between state1 and resumed_state if present
    let extract_session_id = |state: &serde_json::Value| -> Option<String> {
        state
            .get("sessionId")
            .or_else(|| state.get("data").and_then(|d| d.get("sessionId")))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    if let (Some(id1), Some(id_resumed)) =
        (extract_session_id(&state1), extract_session_id(&resumed_state))
    {
        assert_eq!(id1, id_resumed, "Session ID should match on resume");
    }

    // 6. Shutdown all sessions cleanly
    session1.shutdown().await.expect("session 1 shutdown failed");
    resumed_session
        .shutdown()
        .await
        .expect("resumed session shutdown failed");
}

#[tokio::test]
async fn test_role_resolution_against_live_omp_config() {
    if !has_omp() {
        eprintln!("omp binary not found in PATH; skipping role resolution test");
        return;
    }

    let temp_dir = tempfile::tempdir().expect("failed to create tempdir");

    // 1. Query model roles using live omp binary via get_model_roles
    let model_roles = roles::get_model_roles(temp_dir.path())
        .await
        .expect("get_model_roles failed");

    assert!(
        model_roles.contains_key("default"),
        "modelRoles should contain 'default' role: {:?}",
        model_roles
    );

    // 2. Filter to chat roles
    let chat_roles = roles::get_chat_roles(&model_roles);
    assert!(!chat_roles.is_empty(), "chat_roles should not be empty");

    // Verify default role is in chat_roles
    let default_role = chat_roles.iter().find(|r| r.name == "default");
    assert!(default_role.is_some(), "chat_roles must include 'default'");
    let default_role = default_role.unwrap();
    assert!(!default_role.selector.is_empty(), "default role must specify a selector");

    // Verify hidden/background roles are excluded
    for hidden in roles::HIDDEN_ROLES {
        assert!(
            !chat_roles.iter().any(|r| r.name == *hidden),
            "hidden role '{}' should not be in chat_roles",
            hidden
        );
    }

    // 3. Test resolve_role_alias
    let resolved = roles::resolve_role_alias(&model_roles, "default")
        .expect("resolve_role_alias for 'default' failed");
    assert!(
        !resolved.is_empty(),
        "resolved candidate string should not be empty"
    );

    // 4. Test parse_candidate on the resolved string
    let (provider, model_id, thinking_level) = roles::parse_candidate(&resolved);
    assert!(!provider.is_empty(), "provider should not be empty");
    assert!(!model_id.is_empty(), "model_id should not be empty");
    if let Some(level) = thinking_level {
        assert!(
            roles::VALID_THINKING_LEVELS.contains(&level.as_str()),
            "thinking level '{}' should be valid",
            level
        );
    }
}

#[tokio::test]
async fn test_projects_state_lifecycle() {
    if !has_omp() {
        eprintln!("omp binary not found in PATH; skipping ProjectsState lifecycle test");
        return;
    }

    let temp_dir = tempfile::tempdir().expect("failed to create tempdir");
    let dir_str = temp_dir.path().to_str().expect("valid dir string");

    let state = ProjectsState::new();

    // 1. Open project
    let info = state.open(dir_str).await.expect("open project failed");
    assert_eq!(info.dir, dir_str);
    assert!(!info.id.is_empty());
    assert!(info.session_file.is_some());

    // 2. List / get project state
    let project = state
        .get_project(&info.id)
        .await
        .expect("get_project failed");
    assert_eq!(project.id, info.id);
    assert_eq!(project.dir, temp_dir.path());

    // Verify session state from project.session
    let session_state = project
        .session
        .get_state()
        .await
        .expect("project session get_state failed");
    assert!(!session_state.is_null());

    // 3. New session
    let new_sess_res = project
        .session
        .new_session()
        .await
        .expect("project session new_session failed");
    assert!(!new_sess_res.is_null());

    // Verify get_state reflects active session
    let updated_state = project
        .session
        .get_state()
        .await
        .expect("get_state after new_session failed");
    assert!(!updated_state.is_null());

    // 4. Close project
    state.close(&info.id).await.expect("close project failed");

    // Verify project no longer in state
    let get_after_close = state.get_project(&info.id).await;
    assert!(
        get_after_close.is_err(),
        "Project should be removed after close"
    );

    // 5. Clean shutdown
    state.shutdown_all().await;
}

#[tokio::test]
async fn test_phase4_full_smoke_multi_tab_workflow() {
    if !has_omp() {
        eprintln!("omp binary not found in PATH; skipping Phase 4 smoke test");
        return;
    }

    // 1. Setup scratch repo directory with git initialized
    let temp_root = tempfile::tempdir().expect("failed to create tempdir");
    let root_path = temp_root.path();

    // Initialize a git repository with an initial commit
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(root_path)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.name", "Smoke Test"])
        .current_dir(root_path)
        .output()
        .expect("git config user.name failed");
    std::process::Command::new("git")
        .args(["config", "user.email", "smoke@test.local"])
        .current_dir(root_path)
        .output()
        .expect("git config user.email failed");

    let readme_path = root_path.join("README.md");
    std::fs::write(&readme_path, "# Smoke Test Repo\n").expect("write README");
    std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(root_path)
        .output()
        .expect("git add failed");
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(root_path)
        .output()
        .expect("git commit failed");

    // Create tab 2 and tab 3 dirs
    let dir2 = tempfile::tempdir().expect("failed to create tempdir 2");
    let dir3 = tempfile::tempdir().expect("failed to create tempdir 3");

    let state = ProjectsState::new();

    // 2. Multi-tab creation: open 3 tabs
    let tab1 = state.open(root_path.to_str().unwrap()).await.expect("open tab 1");
    let tab2 = state.open(dir2.path().to_str().unwrap()).await.expect("open tab 2");
    let tab3 = state.open(dir3.path().to_str().unwrap()).await.expect("open tab 3");

    assert!(!tab1.id.is_empty());
    assert!(!tab2.id.is_empty());
    assert!(!tab3.id.is_empty());

    // 3. Role listing on repository
    let model_roles = roles::get_model_roles(root_path).await.expect("get_model_roles");
    let chat_roles = roles::get_chat_roles(&model_roles);
    assert!(!chat_roles.is_empty(), "Chat roles list should not be empty");

    // 4. Git status on repository
    let git_st = bide_lib::git::git_status(root_path).await.expect("git_status failed");
    assert!(git_st.staged.is_empty());
    assert!(git_st.unstaged.is_empty());
    assert!(git_st.untracked.is_empty());

    // Create a new untracked file and verify git_status picks it up
    let new_file = root_path.join("new_file.txt");
    std::fs::write(&new_file, "smoke content\n").expect("write new_file");
    let git_st_after = bide_lib::git::git_status(root_path).await.expect("git_status after write");
    assert!(git_st_after.untracked.iter().any(|f| f == "new_file.txt"));

    // Stage new file and commit
    bide_lib::git::git_stage(root_path, "new_file.txt").await.expect("stage new file");
    bide_lib::git::git_commit(root_path, "Add new_file.txt").await.expect("commit new file");
    let git_st_clean = bide_lib::git::git_status(root_path).await.expect("git_status clean");
    assert!(git_st_clean.staged.is_empty());
    assert!(git_st_clean.unstaged.is_empty());

    // 5. Concurrent prompts across sessions
    let p1 = state.get_project(&tab1.id).await.expect("get tab 1");
    let p2 = state.get_project(&tab2.id).await.expect("get tab 2");
    let p3 = state.get_project(&tab3.id).await.expect("get tab 3");

    let rx1 = p1.session.subscribe_broadcast();
    let rx2 = p2.session.subscribe_broadcast();
    let rx3 = p3.session.subscribe_broadcast();

    let prompt1 = {
        let s = p1.session.clone();
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(60), s.prompt("echo tab1", None)).await
        })
    };
    let prompt2 = {
        let s = p2.session.clone();
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(60), s.prompt("echo tab2", None)).await
        })
    };
    let prompt3 = {
        let s = p3.session.clone();
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(60), s.prompt("echo tab3", None)).await
        })
    };

    let (r1, r2, r3) = tokio::join!(prompt1, prompt2, prompt3);
    assert!(r1.is_ok());
    assert!(r2.is_ok());
    assert!(r3.is_ok());

    // Wait for settlement
    let wait_settle = |mut rx: broadcast::Receiver<Value>| async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < deadline {
            if let Ok(Ok(f)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
                if f.get("type").and_then(|t| t.as_str()) == Some("session_settled") {
                    return true;
                }
            }
        }
        false
    };

    let (s1, s2, s3) = tokio::join!(wait_settle(rx1), wait_settle(rx2), wait_settle(rx3));
    assert!(s1 && s2 && s3, "All three tabs should settle");

    // 6. Test project restart
    let restarted = state.restart(&tab1.id).await.expect("restart tab 1");
    assert_eq!(restarted.id, tab1.id);

    // 7. Clean shutdown of all tabs
    state.shutdown_all().await;
}
