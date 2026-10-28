use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::{
    omp::OmpSession,
    persist::{self, TabConfig},
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
    pub advisor: bool,
}

#[derive(Clone)]
pub struct Project {
    pub id: String,
    pub dir: PathBuf,
    pub session: Arc<OmpSession>,
    pub session_file: Option<String>,
    pub orchestrate: bool,
    pub role: String,
    pub advisor: bool,
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
            advisor: p.advisor,
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
}

impl ProjectsState {
    pub fn new() -> Self {
        Self {
            projects: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn open(&self, dir: &str) -> Result<ProjectInfo, String> {
        let dir_path = std::fs::canonicalize(dir).map_err(|e| format!("Cannot open '{dir}': {e}"))?;
        match crate::workspace::sync(&dir_path, &crate::workspace::store_dir()).await {
            Ok(Some(warning)) => eprintln!("workspace sync warning: {warning}"),
            Err(e) => eprintln!("workspace sync error: {e}"),
            Ok(None) => {}
        }
        let session = OmpSession::spawn(&dir_path, None).await?;
        let session_file = session.session_file();
        let id = uuid::Uuid::new_v4().to_string();

        let project = Arc::new(Project {
            id: id.clone(),
            dir: dir_path.clone(),
            session,
            session_file: session_file.clone(),
            orchestrate: false,
            role: "default".to_string(),
            advisor: false,
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
            advisor: false,
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

        for tab in &mut config.tabs {
            // Already running (e.g. frontend reloaded): reuse the live session.
            if let Some(p) = self.projects.lock().await.get(&tab.id) {
                restored.push(ProjectInfo::from(&**p));
                continue;
            }
            let dir_path = PathBuf::from(&tab.dir);
            if dir_path.is_dir() {
                match crate::workspace::sync(&dir_path, &crate::workspace::store_dir()).await {
                    Ok(Some(warning)) => eprintln!("workspace sync warning: {warning}"),
                    Err(e) => eprintln!("workspace sync error: {e}"),
                    Ok(None) => {}
                }
            }
            let resume_path = tab
                .session_file
                .as_ref()
                .filter(|sf| Path::new(sf).exists())
                .map(Path::new);

            let session_res = match resume_path {
                Some(rf) => match OmpSession::spawn(&dir_path, Some(rf)).await {
                    Ok(s) => Ok(s),
                    Err(_) => OmpSession::spawn(&dir_path, None).await,
                },
                None => OmpSession::spawn(&dir_path, None).await,
            };

            let session = match session_res {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to restore tab {}: {e}", tab.id);
                    continue;
                }
            };

            if tab.role != "default" {
                let _ = roles::set_role(&session, &dir_path, &tab.role).await;
            }

            if tab.advisor {
                let _ = session.command("/advisor on").await;
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
                advisor: tab.advisor,
                restart_pending: Arc::new(AtomicBool::new(false)),
                channels: Arc::new(Mutex::new(Vec::new())),
            });

            self.projects
                .lock()
                .await
                .insert(tab.id.clone(), project.clone());
            restored.push(ProjectInfo::from(&*project));
        }

        let _ = persist::save(&config);
        Ok(restored)
    }

    pub async fn restart(&self, id: &str) -> Result<ProjectInfo, String> {
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

        let new_session = match resume_path {
            Some(rf) => match OmpSession::spawn(&existing.dir, Some(rf)).await {
                Ok(s) => s,
                Err(_) => OmpSession::spawn(&existing.dir, None).await?,
            },
            None => OmpSession::spawn(&existing.dir, None).await?,
        };

        {
            let chs = existing.channels.lock().unwrap();
            for ch in chs.iter() {
                new_session.subscribe(ch.clone());
            }
        }

        if existing.role != "default" {
            let _ = roles::set_role(&new_session, &existing.dir, &existing.role).await;
        }

        if existing.advisor {
            let _ = new_session.command("/advisor on").await;
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
            advisor: existing.advisor,
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

    pub async fn set_advisor(&self, id: &str, advisor: bool) -> Result<(), String> {
        let project = {
            let mut lock = self.projects.lock().await;
            let proj = lock
                .get_mut(id)
                .ok_or_else(|| format!("Project '{id}' not found"))?;
            let mut updated = (**proj).clone();
            updated.advisor = advisor;
            let arc = Arc::new(updated);
            *proj = arc.clone();
            arc
        };
        let slash = if advisor {
            "/advisor on"
        } else {
            "/advisor off"
        };
        project.session.command(slash).await?;
        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.advisor = advisor;
            let _ = persist::save(&config);
        }
        Ok(())
    }

    pub async fn set_role(&self, id: &str, role: &str) -> Result<serde_json::Value, String> {
        let project = self.get_project(id).await?;
        let res = roles::set_role(&project.session, &project.dir, role).await?;
        {
            let mut lock = self.projects.lock().await;
            if let Some(proj) = lock.get_mut(id) {
                let mut updated = (**proj).clone();
                updated.role = role.to_string();
                *proj = Arc::new(updated);
            }
        }
        let mut config = persist::load();
        if let Some(tab) = config.tabs.iter_mut().find(|t| t.id == id) {
            tab.role = role.to_string();
            let _ = persist::save(&config);
        }
        Ok(res)
    }
}
