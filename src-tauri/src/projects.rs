use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::{
    omp::OmpSession,
    persist::{self, TabConfig},
    profiles,
    roles,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub id: String,
    pub dir: String,
    pub session_file: Option<String>,
    pub orchestrate: bool,
    pub role: String,
    pub profile: Option<String>,
    pub advisor_model_key: Option<String>,
    pub advisor_thinking: Option<String>,
}

#[derive(Clone)]
pub struct Project {
    pub id: String,
    pub dir: PathBuf,
    pub session: Arc<OmpSession>,
    pub session_file: Option<String>,
    pub orchestrate: bool,
    pub role: String,
    pub profile: Option<String>,
    /// Saved Custom advisor choice, independent of the active model profile.
    pub advisor_model_key: Option<String>,
    pub advisor_thinking: Option<String>,
    /// `provider/modelId` when `role == roles::CUSTOM_ROLE`.
    pub model: Option<String>,
    /// Thinking level override on top of the role/model.
    pub thinking: Option<String>,
    pub restart_pending: Arc<AtomicBool>,
    pub channels: Arc<Mutex<Vec<tauri::ipc::Channel<serde_json::Value>>>>,
}

impl From<&Project> for ProjectInfo {
    fn from(p: &Project) -> Self {
        Self {
            id: p.id.clone(),
            dir: p.dir.to_string_lossy().to_string(),
            session_file: p.session.session_file().or_else(|| p.session_file.clone()),
            orchestrate: p.orchestrate,
            role: p.role.clone(),
            profile: p.profile.clone(),
            advisor_model_key: p.advisor_model_key.clone(),
            advisor_thinking: p.advisor_thinking.clone(),
        }
    }
}

impl From<Arc<Project>> for ProjectInfo {
    fn from(p: Arc<Project>) -> Self {
        Self::from(p.as_ref())
    }
}

#[derive(Clone, Default)]
pub struct ProjectsState {
    pub projects: Arc<tokio::sync::Mutex<HashMap<String, Arc<Project>>>>,
    pub(crate) profile_changes: Arc<tokio::sync::Mutex<()>>,
}

impl ProjectsState {
    pub fn new() -> Self {
        Self {
            projects: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            profile_changes: Arc::default(),
        }
    }

    pub async fn open(&self, dir: &str) -> Result<ProjectInfo, String> {
        let dir_path = std::fs::canonicalize(dir).map_err(|e| format!("Cannot open '{dir}': {e}"))?;
        match crate::workspace::sync(&dir_path, &crate::workspace::store_dir()).await {
            Ok(Some(warning)) => eprintln!("workspace sync warning: {warning}"),
            Err(e) => eprintln!("workspace sync error: {e}"),
            Ok(None) => {}
        }
        let env_vars = crate::workspace::read_env(&dir_path).await;
        let session = OmpSession::spawn_with_env(&dir_path, None, &env_vars).await?;
        let session_file = session.session_file();
        let id = uuid::Uuid::new_v4().to_string();

        let project = Arc::new(Project {
            id: id.clone(),
            dir: dir_path.clone(),
            session,
            session_file: session_file.clone(),
            orchestrate: false,
            role: "default".to_string(),
            profile: None,
            advisor_model_key: None,
            advisor_thinking: None,
            model: None,
            thinking: None,
            restart_pending: Arc::new(AtomicBool::new(false)),
            channels: Arc::new(Mutex::new(Vec::new())),
        });

        self.projects
            .lock()
            .await
            .insert(id.clone(), project.clone());

        let mut config = persist::load();
        config.tabs.push(TabConfig {
            id: id.clone(),
            dir: dir_path.to_string_lossy().into_owned(),
            session_file,
            orchestrate: false,
            role: "default".to_string(),
            profile: None,
            advisor_model_key: None,
            advisor_thinking: None,
            model: None,
            thinking: None,
        });
        config.active_tab = Some(id);
        let _ = persist::save(&config);

        Ok(ProjectInfo::from(&*project))
    }

    pub async fn close(&self, id: &str) -> Result<(), String> {
        let project = self.projects.lock().await.remove(id);
        let mut config = persist::load();
        let had_tab = config.tabs.iter().any(|t| t.id == id);

        if project.is_none() && !had_tab {
            return Err(format!("Project '{id}' not found"));
        }

        if let Some(project) = project {
            let _ = project.session.shutdown().await;
        }

        if had_tab {
            config.tabs.retain(|t| t.id != id);
            if config.active_tab.as_deref() == Some(id) {
                config.active_tab = config.tabs.first().map(|t| t.id.clone());
            }
            let _ = persist::save(&config);
        }

        Ok(())
    }

    pub async fn restore(&self) -> Result<Vec<ProjectInfo>, String> {
        let mut config = persist::load();
        let mut restored = Vec::new();
        let mut kept_tabs = Vec::with_capacity(config.tabs.len());

        for mut tab in config.tabs {
            // Already running (e.g. frontend reloaded): reuse the live session.
            if let Some(p) = self.projects.lock().await.get(&tab.id) {
                restored.push(ProjectInfo::from(&**p));
                kept_tabs.push(tab);
                continue;
            }
            let dir_path = PathBuf::from(&tab.dir);
            if !dir_path.is_dir() {
                continue;
            }
            match crate::workspace::sync(&dir_path, &crate::workspace::store_dir()).await {
                Ok(Some(warning)) => eprintln!("workspace sync warning: {warning}"),
                Err(e) => eprintln!("workspace sync error: {e}"),
                Ok(None) => {}
            }
            let resume_path = tab
                .session_file
                .as_ref()
                .filter(|sf| Path::new(sf).exists())
                .map(Path::new);

            let env_vars = crate::workspace::read_env(&dir_path).await;
            let session_res = match resume_path {
                Some(rf) => match OmpSession::spawn_with_env(&dir_path, Some(rf), &env_vars).await {
                    Ok(s) => Ok(s),
                    Err(_) => OmpSession::spawn_with_env(&dir_path, None, &env_vars).await,
                },
                None => OmpSession::spawn_with_env(&dir_path, None, &env_vars).await,
            };

            let session = match session_res {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to restore tab {}: {e}", tab.id);
                    continue;
                }
            };

            if let Err(e) = restore_profile(&session, &dir_path, tab.profile.as_deref()).await {
                eprintln!("Failed to restore model profile for tab {}: {e}", tab.id);
                tab.profile = None;
            }
            if tab.profile.is_none() {
                roles::apply_selection(
                    &session,
                    &dir_path,
                    &tab.role,
                    tab.model.as_deref(),
                    tab.thinking.as_deref(),
                    tab.advisor_model_key.as_deref(),
                    tab.advisor_thinking.as_deref(),
                )
                .await;
            }

            let session_file = session.session_file().or_else(|| tab.session_file.clone());
            tab.session_file = session_file.clone();

            let project = Arc::new(Project {
                id: tab.id.clone(),
                dir: dir_path,
                session,
                session_file,
                orchestrate: tab.orchestrate,
                role: tab.role.clone(),
                profile: tab.profile.clone(),
                advisor_model_key: tab.advisor_model_key.clone(),
                advisor_thinking: tab.advisor_thinking.clone(),
                model: tab.model.clone(),
                thinking: tab.thinking.clone(),
                restart_pending: Arc::new(AtomicBool::new(false)),
                channels: Arc::new(Mutex::new(Vec::new())),
            });

            self.projects
                .lock()
                .await
                .insert(tab.id.clone(), project.clone());
            restored.push(ProjectInfo::from(&*project));
            kept_tabs.push(tab);
        }

        config.tabs = kept_tabs;
        if let Some(active_id) = &config.active_tab {
            if !config.tabs.iter().any(|t| &t.id == active_id) {
                config.active_tab = config.tabs.first().map(|t| t.id.clone());
            }
        }
        let _ = persist::save(&config);
        Ok(restored)
    }

    pub async fn restart(&self, id: &str) -> Result<ProjectInfo, String> {
        let _change = self.profile_changes.lock().await;
        let existing = self.get_project(id).await?;
        let _ = existing.session.shutdown().await;

        let resume_candidate = existing
            .session
            .session_file()
            .or_else(|| existing.session_file.clone());

        let resume_path = resume_candidate
            .as_ref()
            .filter(|sf| Path::new(sf).exists())
            .map(Path::new);

        let env_vars = crate::workspace::read_env(&existing.dir).await;
        let new_session = match resume_path {
            Some(rf) => match OmpSession::spawn_with_env(&existing.dir, Some(rf), &env_vars).await {
                Ok(s) => s,
                Err(_) => OmpSession::spawn_with_env(&existing.dir, None, &env_vars).await?,
            },
            None => OmpSession::spawn_with_env(&existing.dir, None, &env_vars).await?,
        };

        {
            let chs = existing.channels.lock().unwrap();
            for ch in chs.iter() {
                new_session.subscribe(ch.clone());
            }
        }

        let mut profile = existing.profile.clone();
        if let Err(e) = restore_profile(&new_session, &existing.dir, profile.as_deref()).await {
            eprintln!("Failed to restore model profile for tab {id}: {e}");
            profile = None;
        }
        if profile.is_none() {
            roles::apply_selection(
                &new_session,
                &existing.dir,
                &existing.role,
                existing.model.as_deref(),
                existing.thinking.as_deref(),
                existing.advisor_model_key.as_deref(),
                existing.advisor_thinking.as_deref(),
            )
            .await;
        }

        let new_session_file = new_session
            .session_file()
            .or_else(|| existing.session_file.clone());

        let updated_project = Arc::new(Project {
            id: id.to_string(),
            dir: existing.dir.clone(),
            session: new_session,
            session_file: new_session_file.clone(),
            orchestrate: existing.orchestrate,
            role: existing.role.clone(),
            profile,
            advisor_model_key: existing.advisor_model_key.clone(),
            advisor_thinking: existing.advisor_thinking.clone(),
            model: existing.model.clone(),
            thinking: existing.thinking.clone(),
            restart_pending: existing.restart_pending.clone(),
            channels: existing.channels.clone(),
        });

        self.projects
            .lock()
            .await
            .insert(id.to_string(), updated_project.clone());

        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.session_file = new_session_file;
            tab.profile = updated_project.profile.clone();
            let _ = persist::save(&config);
        }

        Ok(ProjectInfo::from(&*updated_project))
    }

    /// Restart now if settled (Ok(true)); otherwise mark restart_pending and restart on the
    /// next `session_settled` frame (Ok(false)). Err = immediate restart failed.
    pub async fn restart_when_settled(&self, p: Arc<Project>) -> Result<bool, String> {
        if p.session.is_settled() {
            self.restart(&p.id).await.map(|_| true)
        } else {
            p.restart_pending.store(true, std::sync::atomic::Ordering::SeqCst);
            let state = self.clone();
            let mut rx = p.session.subscribe_broadcast();
            tokio::spawn(async move {
                while let Ok(frame) = rx.recv().await {
                    if frame.get("type").and_then(|v| v.as_str()) == Some("session_settled") {
                        if p.restart_pending.swap(false, std::sync::atomic::Ordering::SeqCst) {
                            let _ = state.restart(&p.id).await;
                        }
                        break;
                    }
                }
            });
            Ok(false)
        }
    }

    pub async fn get_project(&self, id: &str) -> Result<Arc<Project>, String> {
        self.projects
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Project '{id}' not found"))
    }

    pub async fn shutdown_all(&self) {
        let projects: Vec<Arc<Project>> = {
            let lock = self.projects.lock().await;
            lock.values().cloned().collect()
        };
        for project in projects {
            let _ = project.session.shutdown().await;
        }
    }

    pub async fn set_orchestrate(&self, id: &str, orchestrate: bool) -> Result<(), String> {
        {
            let mut lock = self.projects.lock().await;
            let proj = lock
                .get_mut(id)
                .ok_or_else(|| format!("Project '{id}' not found"))?;
            let mut updated = (**proj).clone();
            updated.orchestrate = orchestrate;
            *proj = Arc::new(updated);
        }
        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.orchestrate = orchestrate;
            let _ = persist::save(&config);
        }
        Ok(())
    }

    pub async fn set_advisor(
        &self,
        id: &str,
        model: Option<String>,
        level: Option<String>,
    ) -> Result<(), String> {
        let _change = self.profile_changes.lock().await;
        let project = self.get_project(id).await?;
        if project.profile.is_some() {
            return Err("Select Custom to pick an advisor model manually".into());
        }
        if !project.session.is_settled() {
            return Err("Wait for omp to finish before changing advisor models".into());
        }
        let level = if model.is_some() { level } else { None };
        if let (Some(model), Some(level)) = (model.as_deref(), level.as_deref()) {
            roles::validate_model_thinking(&project.session, model, level).await?;
        }
        roles::set_advisor(&project.session, model.as_deref(), level.as_deref()).await?;
        drop(project);

        let mut lock = self.projects.lock().await;
        let project = lock.get_mut(id).ok_or_else(|| format!("Project '{id}' not found"))?;
        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.advisor_model_key.clone_from(&model);
            tab.advisor_thinking.clone_from(&level);
            persist::save(&config)?;
        }
        let project = Arc::make_mut(project);
        project.advisor_model_key = model;
        project.advisor_thinking = level;
        Ok(())
    }

    pub async fn set_profile(&self, id: &str, name: Option<&str>) -> Result<ProjectInfo, String> {
        let _change = self.profile_changes.lock().await;
        let project = self.get_project(id).await?;
        let scopes;
        let selection = if let Some(name) = name {
            scopes = profiles::get(Some(&project.dir)).await?;
            let profile = scopes.resolve(name).ok_or_else(|| format!("Model profile {name:?} not found"))?;
            Some((name, profile))
        } else {
            None
        };
        self.apply_project_profile(&project, selection).await
    }

    pub(crate) async fn apply_project_profile(
        &self,
        project: &Project,
        selection: Option<(&str, &profiles::ModelProfile)>,
    ) -> Result<ProjectInfo, String> {
        if !project.session.is_settled() {
            return Err("Wait for omp to finish before changing model profiles".into());
        }
        let (name, model, thinking) = match selection {
            Some((name, profile)) => {
                roles::apply_profile(&project.session, profile).await?;
                (Some(name.to_owned()), None, None)
            }
            None if project.profile.is_some() => {
                let state = project.session.get_state().await?;
                let model = state["model"]["provider"].as_str().zip(state["model"]["id"].as_str())
                    .map(|(provider, id)| format!("{provider}/{id}"));
                let thinking = state["thinkingLevel"].as_str().map(str::to_owned);
                roles::clear_profile(
                    &project.session,
                    project.advisor_model_key.as_deref(),
                    project.advisor_thinking.as_deref(),
                )
                .await?;
                (None, model, thinking)
            }
            None => return Ok(ProjectInfo::from(project)),
        };
        self.save_selection(&project.id, roles::CUSTOM_ROLE, name, model, thinking).await
    }

    /// Switches to `roles::CUSTOM_ROLE`; keeps the thinking override unless explicitly changed.
    pub async fn set_model(
        &self,
        id: &str,
        model: &str,
        level: Option<&str>,
    ) -> Result<serde_json::Value, String> {
        let _change = self.profile_changes.lock().await;
        let project = self.get_project(id).await?;
        if project.profile.is_some() {
            return Err("Select Custom to pick a model manually".into());
        }
        if let Some(level) = level {
            roles::validate_model_thinking(&project.session, model, level).await?;
        }
        let res = roles::set_model(&project.session, model).await?;
        let thinking = if let Some(level) = level {
            roles::set_thinking(&project.session, level).await?;
            Some(level.to_owned())
        } else {
            project.thinking.clone()
        };
        self.save_selection(id, roles::CUSTOM_ROLE, None, Some(model.to_owned()), thinking)
            .await?;
        Ok(res)
    }

    pub async fn set_thinking(&self, id: &str, level: &str) -> Result<serde_json::Value, String> {
        let _change = self.profile_changes.lock().await;
        let project = self.get_project(id).await?;
        if project.profile.is_some() {
            return Err("Select Custom to change thinking manually".into());
        }
        let res = roles::set_thinking(&project.session, level).await?;
        self.save_selection(id, &project.role, None, project.model.clone(), Some(level.to_owned()))
            .await?;
        Ok(res)
    }

    async fn save_selection(
        &self,
        id: &str,
        role: &str,
        profile: Option<String>,
        model: Option<String>,
        thinking: Option<String>,
    ) -> Result<ProjectInfo, String> {
        let mut lock = self.projects.lock().await;
        let project = lock.get_mut(id).ok_or_else(|| format!("Project '{id}' not found"))?;
        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.role = role.to_owned();
            tab.profile.clone_from(&profile);
            tab.model.clone_from(&model);
            tab.thinking.clone_from(&thinking);
            persist::save(&config)?;
        }
        let mut updated = (**project).clone();
        updated.role = role.to_owned();
        updated.profile = profile;
        updated.model = model;
        updated.thinking = thinking;
        let info = ProjectInfo::from(&updated);
        *project = Arc::new(updated);
        Ok(info)
    }
}

async fn restore_profile(session: &OmpSession, dir: &Path, name: Option<&str>) -> Result<(), String> {
    if let Some(name) = name {
        let scopes = profiles::get(Some(dir)).await?;
        let profile = scopes.resolve(name).ok_or_else(|| format!("Model profile {name:?} not found"))?;
        roles::apply_profile(session, profile).await?;
    }
    Ok(())
}
