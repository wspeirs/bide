use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tauri::State;

use crate::{
    git::{self, Branches, GitStatus, Worktree},
    persist,
    profiles::{self, ModelProfile, ModelProfileScope, ModelProfileScopes},
    projects::{ProjectInfo, ProjectsState},
    roles::{self, ModelInfo, ProfileModelOptions},
    update::{self, UpdateReport},
    workspace::{self, WorkspaceConfig},
};

#[tauri::command]
pub async fn project_open(
    state: State<'_, ProjectsState>,
    dir: String,
) -> Result<ProjectInfo, String> {
    state.open(&dir).await
}

#[tauri::command]
pub async fn project_close(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    state.close(&id).await
}
#[tauri::command]
pub async fn project_restart(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<ProjectInfo, String> {
    state.restart(&id).await
}


#[tauri::command]
pub async fn projects_restore(state: State<'_, ProjectsState>) -> Result<Vec<ProjectInfo>, String> {
    state.restore().await
}

#[tauri::command]
pub async fn omp_subscribe(
    state: State<'_, ProjectsState>,
    id: String,
    channel: tauri::ipc::Channel<Value>,
) -> Result<(), String> {
    let project = state.get_project(&id).await?;
    project.channels.lock().unwrap().push(channel.clone());
    project.session.subscribe(channel);
    Ok(())
}

#[tauri::command]
pub async fn omp_prompt(
    state: State<'_, ProjectsState>,
    id: String,
    text: String,
    streaming_behavior: Option<String>,
) -> Result<(), String> {
    let project = state.get_project(&id).await?;
    project
        .session
        .prompt(&text, streaming_behavior.as_deref())
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn omp_abort(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    let project = state.get_project(&id).await?;
    project.session.abort().await.map(|_| ())
}

#[tauri::command]
pub async fn omp_get_state(state: State<'_, ProjectsState>, id: String) -> Result<Value, String> {
    let project = state.get_project(&id).await?;
    project.session.get_state().await
}

#[tauri::command]
pub async fn omp_get_stderr(state: State<'_, ProjectsState>, id: String) -> Result<Vec<String>, String> {
    let project = state.get_project(&id).await?;
    Ok(project.session.stderr_tail())
}

#[tauri::command]
pub async fn omp_new_session(state: State<'_, ProjectsState>, id: String) -> Result<Value, String> {
    let project = state.get_project(&id).await?;
    let res = project.session.new_session().await?;
    if let Some(sf) = project.session.session_file() {
        let mut lock = state.projects.lock().await;
        if let Some(p) = lock.get_mut(&id) {
            let mut updated = (**p).clone();
            updated.session_file = Some(sf.clone());
            *p = Arc::new(updated);
        }
        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.session_file = Some(sf);
            let _ = persist::save(&config);
        }
    }
    Ok(res)
}

#[tauri::command]
pub async fn omp_command(
    state: State<'_, ProjectsState>,
    id: String,
    slash: String,
) -> Result<Value, String> {
    let project = state.get_project(&id).await?;
    project.session.command(&slash).await
}

#[tauri::command]
pub async fn omp_ext_statuses(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<HashMap<String, String>, String> {
    let project = state.get_project(&id).await?;
    Ok(project.session.ext_statuses())
}

#[tauri::command]
pub async fn omp_get_messages(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<Value, String> {
    let project = state.get_project(&id).await?;
    project.session.get_messages().await
}

#[tauri::command]
pub async fn omp_ui_respond(
    state: State<'_, ProjectsState>,
    id: String,
    response: Value,
) -> Result<(), String> {
    let project = state.get_project(&id).await?;
    project.session.send_ui_response(response).await
}

#[tauri::command]
pub async fn model_profiles_get(
    state: State<'_, ProjectsState>,
    id: Option<String>,
) -> Result<ModelProfileScopes, String> {
    let project = match id {
        Some(id) => Some(state.get_project(&id).await?),
        None => None,
    };
    profiles::get(project.as_ref().map(|p| p.dir.as_path())).await
}

#[tauri::command]
pub async fn model_profile_options(
    state: State<'_, ProjectsState>,
    id: Option<String>,
) -> Result<ProfileModelOptions, String> {
    if let Some(id) = id {
        let project = state.get_project(&id).await?;
        let env = workspace::read_env(&project.dir).await;
        roles::profile_model_options(&project.dir, &env).await
    } else {
        roles::profile_model_options(std::path::Path::new("/"), &BTreeMap::new()).await
    }
}

#[derive(serde::Serialize)]
pub struct ModelProfileSaveResult {
    updated: Vec<ProjectInfo>,
    warning: Option<String>,
}

#[tauri::command]
pub async fn model_profile_save(
    state: State<'_, ProjectsState>,
    id: Option<String>,
    scope: ModelProfileScope,
    name: String,
    profile: Option<ModelProfile>,
) -> Result<ModelProfileSaveResult, String> {
    let _change = state.profile_changes.lock().await;
    let project = match scope {
        ModelProfileScope::Global => None,
        ModelProfileScope::Local => {
            let id = id.as_deref().ok_or("Local model profiles require a workspace")?;
            Some(state.get_project(id).await?)
        }
    };
    let dir = project.as_ref().map(|p| p.dir.as_path());
    let local_key = match dir {
        Some(dir) => Some(workspace::repo_key(dir).await.0),
        None => None,
    };
    let selected: Vec<_> = state.projects.lock().await.values()
        .filter(|p| p.profile.as_deref() == Some(name.as_str()))
        .cloned()
        .collect();
    let mut affected = Vec::new();
    for p in selected {
        if let Some(key) = &local_key {
            if &workspace::repo_key(&p.dir).await.0 != key {
                continue;
            }
        }
        let scopes = profiles::get(Some(&p.dir)).await?;
        let next = match scope {
            ModelProfileScope::Global => scopes.local.get(&name).or(profile.as_ref()),
            ModelProfileScope::Local => profile.as_ref().or_else(|| scopes.global.get(&name)),
        };
        if next != scopes.resolve(&name) {
            if !p.session.is_settled() {
                return Err(format!("Wait for omp in {} to finish before editing its active profile", p.dir.display()));
            }
            affected.push((p, next.cloned()));
        }
    }
    profiles::save(dir, scope, &name, profile).await?;

    let mut updated = Vec::new();
    let mut failures = Vec::new();
    for (p, next) in affected {
        let selection = next.as_ref().map(|profile| (name.as_str(), profile));
        match state.apply_project_profile(&p, selection).await {
            Ok(info) => updated.push(info),
            Err(e) => failures.push(format!("{}: {e}", p.dir.display())),
        }
    }
    let warning = (!failures.is_empty()).then(|| {
        format!("Saved, but could not update the active profile in {}", failures.join("; "))
    });
    Ok(ModelProfileSaveResult { updated, warning })
}

#[tauri::command]
pub async fn omp_set_profile(
    state: State<'_, ProjectsState>,
    id: String,
    name: Option<String>,
) -> Result<ProjectInfo, String> {
    state.set_profile(&id, name.as_deref()).await
}


#[tauri::command]
pub async fn omp_set_advisor(
    state: State<'_, ProjectsState>,
    id: String,
    model: Option<String>,
    level: Option<String>,
) -> Result<(), String> {
    state.set_advisor(&id, model, level).await
}
#[tauri::command]
pub async fn omp_list_models(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<Vec<ModelInfo>, String> {
    let project = state.get_project(&id).await?;
    roles::list_models(&project.session).await
}

#[tauri::command]
pub async fn omp_list_thinking_levels(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<Vec<String>, String> {
    let project = state.get_project(&id).await?;
    roles::list_thinking_levels(&project.session).await
}

#[tauri::command]
pub async fn omp_set_model(
    state: State<'_, ProjectsState>,
    id: String,
    model: String,
    level: Option<String>,
) -> Result<Value, String> {
    state.set_model(&id, &model, level.as_deref()).await
}

#[tauri::command]
pub async fn omp_set_thinking(
    state: State<'_, ProjectsState>,
    id: String,
    level: String,
) -> Result<Value, String> {
    state.set_thinking(&id, &level).await
}

#[tauri::command]
pub async fn omp_collab_link(state: State<'_, ProjectsState>, id: String) -> Result<String, String> {
    let project = state.get_project(&id).await?;
    let pid = project
        .session
        .pid()
        .ok_or_else(|| "omp is not running for this tab".to_string())?;
    crate::omp::collab_link(pid).await
}


#[tauri::command]
pub async fn git_status(state: State<'_, ProjectsState>, id: String) -> Result<GitStatus, String> {
    let p = state.get_project(&id).await?;
    git::git_status(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_stage(
    state: State<'_, ProjectsState>,
    id: String,
    path: String,
) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_stage(&p.dir, &path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_unstage(
    state: State<'_, ProjectsState>,
    id: String,
    path: String,
) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_unstage(&p.dir, &path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_add_all(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_add_all(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_commit(
    state: State<'_, ProjectsState>,
    id: String,
    msg: String,
) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_commit(&p.dir, &msg)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_push(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_push(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_pull(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_pull(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_fetch(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_fetch(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_init(state: State<'_, ProjectsState>, id: String) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_init(&p.dir).await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct DirEntry {
    name: String,
    is_dir: bool,
}

/// List one directory of the project (`rel` is relative to the project root, "" for root).
/// Directories first, then files, case-insensitive; `.git` is hidden.
#[tauri::command]
pub async fn fs_list_dir(
    state: State<'_, ProjectsState>,
    id: String,
    rel: String,
) -> Result<Vec<DirEntry>, String> {
    use std::path::Component;
    let rel_path = std::path::Path::new(&rel);
    if !rel_path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err("invalid path".to_string());
    }
    let p = state.get_project(&id).await?;
    let mut rd = tokio::fs::read_dir(p.dir.join(rel_path))
        .await
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let Some(e) = rd.next_entry().await.map_err(|e| e.to_string())? {
        let name = e.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        // metadata() follows symlinks, so a symlink to a dir counts as a dir
        let is_dir = tokio::fs::metadata(e.path()).await.map(|m| m.is_dir()).unwrap_or(false);
        out.push(DirEntry { name, is_dir });
    }
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

#[tauri::command]
pub async fn git_status_text(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<String, String> {
    let p = state.get_project(&id).await?;
    git::git_status_text(&p.dir)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn app_update(state: State<'_, ProjectsState>) -> Result<UpdateReport, String> {
    Ok(update::app_update(&state).await)
}

#[tauri::command]
pub async fn workspace_config_get(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<WorkspaceConfig, String> {
    let p = state.get_project(&id).await?;
    let store_dir = workspace::store_dir();
    let warning = workspace::sync(&p.dir, &store_dir).await?;
    if let Some(w) = warning {
        Ok(WorkspaceConfig {
            text: String::new(),
            source: "none",
            warning: Some(w),
        })
    } else {
        let (key, _) = workspace::repo_key(&p.dir).await;
        let path = workspace::store_path(&store_dir, &key);
        if path.exists() {
            let text = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to read workspace config: {e}"))?;
            Ok(WorkspaceConfig {
                text,
                source: "workspace",
                warning: None,
            })
        } else if let Some(t) = workspace::read_global().await {
            Ok(WorkspaceConfig {
                text: t,
                source: "global",
                warning: None,
            })
        } else {
            Ok(WorkspaceConfig {
                text: String::new(),
                source: "none",
                warning: None,
            })
        }
    }
}

/// Saves config.yml and/or the repo env, then restarts each tab of the repo once so it picks up both.
#[tauri::command]
pub async fn workspace_save(
    state: State<'_, ProjectsState>,
    id: String,
    text: Option<String>,
    env: Option<BTreeMap<String, String>>,
) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    let (key, is_git) = workspace::repo_key(&p.dir).await;
    let text = match text {
        Some(t) => {
            if is_git && git::is_tracked(&p.dir, workspace::REL).await {
                return Err(workspace::TRACKED_WARNING.to_string());
            }
            Some(workspace::normalize(&t)?)
        }
        None => None,
    };
    let store_dir = workspace::store_dir();
    // Env is validated before anything is written, so a rejected env leaves config untouched too.
    if let Some(env) = &env {
        workspace::write_env_to_store(&store_dir, &key, env)?;
    }
    if let Some(text) = &text {
        tokio::fs::create_dir_all(&store_dir)
            .await
            .map_err(|e| format!("Failed to create store directory: {e}"))?;
        tokio::fs::write(workspace::store_path(&store_dir, &key), text)
            .await
            .map_err(|e| format!("Failed to write store config: {e}"))?;
    }

    let projects: Vec<_> = state.projects.lock().await.values().cloned().collect();
    let mut failed = Vec::new();
    for q in projects {
        let (q_key, _) = workspace::repo_key(&q.dir).await;
        if q_key != key {
            continue;
        }
        if text.is_some() {
            if let Err(e) = workspace::sync(&q.dir, &store_dir).await {
                eprintln!("Failed to sync workspace config for tab {}: {e}", q.id);
            }
        }
        let dir = q.dir.display().to_string();
        if let Err(e) = state.restart_when_settled(q).await {
            failed.push(format!("{dir}: {e}"));
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("Saved, but failed to restart omp for {}", failed.join("; ")))
    }
}

#[tauri::command]
pub async fn workspace_env_get(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<BTreeMap<String, String>, String> {
    let p = state.get_project(&id).await?;
    let (key, _) = workspace::repo_key(&p.dir).await;
    workspace::read_env_from_store(&workspace::store_dir(), &key)
}

#[tauri::command]
pub async fn git_worktree_list(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<Vec<Worktree>, String> {
    let p = state.get_project(&id).await?;
    git::git_worktree_list(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_branches(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<Branches, String> {
    let p = state.get_project(&id).await?;
    git::git_branches(&p.dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_worktree_add(
    state: State<'_, ProjectsState>,
    id: String,
    branch: String,
    new_branch: bool,
    base: Option<String>,
) -> Result<String, String> {
    let p = state.get_project(&id).await?;
    git::git_worktree_add(&p.dir, &branch, new_branch, base.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_worktree_remove(
    state: State<'_, ProjectsState>,
    id: String,
    path: String,
    force: bool,
) -> Result<(), String> {
    let p = state.get_project(&id).await?;
    git::git_worktree_remove(&p.dir, &path, force).await.map_err(|e| e.to_string())
}
