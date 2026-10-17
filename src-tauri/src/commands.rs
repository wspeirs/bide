use serde_json::Value;
use std::sync::Arc;
use tauri::State;

use crate::{
    git::{self, GitStatus},
    persist,
    projects::{ProjectInfo, ProjectsState},
    roles::{self, RoleInfo},
    update::{self, UpdateReport},
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
pub async fn omp_ui_respond(
    state: State<'_, ProjectsState>,
    id: String,
    response: Value,
) -> Result<(), String> {
    let project = state.get_project(&id).await?;
    project.session.send_ui_response(response).await
}

#[tauri::command]
pub async fn omp_list_roles(
    state: State<'_, ProjectsState>,
    id: String,
) -> Result<Vec<RoleInfo>, String> {
    let project = state.get_project(&id).await?;
    let model_roles = roles::get_model_roles(&project.dir).await?;
    Ok(roles::get_chat_roles(&model_roles))
}

#[tauri::command]
pub async fn omp_set_role(
    state: State<'_, ProjectsState>,
    id: String,
    role: String,
) -> Result<Value, String> {
    state.set_role(&id, &role).await
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
