use std::{process::Stdio, time::Duration};

use serde::Serialize;
use tokio::process::Command;

use crate::{git, projects::ProjectsState};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    pub success: bool,
    pub old_version: String,
    pub new_version: String,
    pub version_changed: bool,
    pub output: String,
    pub error: Option<String>,
    pub restarts_triggered: Vec<String>,
    pub restarts_pending: Vec<String>,
}

async fn omp_version() -> String {
    match Command::new("omp")
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .await
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => String::new(),
    }
}

pub async fn app_update(state: &ProjectsState) -> UpdateReport {
    let mut report = UpdateReport {
        old_version: omp_version().await,
        ..Default::default()
    };

    let out = Command::new("omp")
        .arg("update")
        .stdin(Stdio::null())
        .output()
        .await;
    match out {
        Ok(o) => {
            report.output = String::from_utf8_lossy(&o.stdout).to_string();
            if !o.status.success() {
                report.error = Some(String::from_utf8_lossy(&o.stderr).to_string());
                return report;
            }
        }
        Err(e) => {
            report.error = Some(format!("Failed to run omp update: {e}"));
            return report;
        }
    }

    report.success = true;
    report.new_version = omp_version().await;
    report.version_changed = report.old_version != report.new_version;
    if !report.version_changed {
        return report;
    }

    let projects: Vec<_> = state.projects.lock().await.values().cloned().collect();
    for p in projects {
        let id = p.id.clone();
        match state.restart_when_settled(p).await {
            Ok(true) => report.restarts_triggered.push(id),
            Ok(false) => report.restarts_pending.push(id),
            Err(_) => {}
        }
    }
    report
}

impl ProjectsState {
    /// Fetch every open project once an hour.
    pub async fn hourly_fetch(self) {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            let projects: Vec<_> = self.projects.lock().await.values().cloned().collect();
            for p in projects {
                let _ = git::git_fetch(&p.dir).await;
            }
        }
    }
}
