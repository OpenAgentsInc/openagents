use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use super::types::*;

static STORE: OnceLock<RwLock<HashMap<String, VibeSnapshot>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VibeSnapshot {
    pub projects: Vec<Project>,
    pub templates: Vec<ProjectTemplate>,
    pub files: Vec<ProjectFile>,
    pub tables: Vec<DatabaseTable>,
    pub deployments: Vec<Deployment>,
    pub domains: Vec<Domain>,
    pub analytics: Vec<AnalyticsData>,
    pub logs: Vec<String>,
    pub tasks: Vec<AgentTask>,
}

impl VibeSnapshot {
    pub fn mock() -> Self {
        Self {
            projects: mock_projects(),
            templates: mock_templates(),
            files: mock_files(),
            tables: mock_tables(),
            deployments: mock_deployments(),
            domains: mock_domains(),
            analytics: mock_analytics(),
            logs: mock_terminal_logs(),
            tasks: mock_agent_tasks(),
        }
    }
}

fn state() -> &'static RwLock<HashMap<String, VibeSnapshot>> {
    STORE.get_or_init(|| {
        let mut map = HashMap::new();
        for project in mock_projects() {
            map.insert(project.id.clone(), VibeSnapshot::mock());
        }
        RwLock::new(map)
    })
}

fn with_snapshot<F, T>(project_id: &str, f: F) -> T
where
    F: FnOnce(&mut VibeSnapshot) -> T,
{
    let mut guard = state().write().expect("VibeSnapshot lock poisoned");
    let entry = guard
        .entry(project_id.to_string())
        .or_insert_with(VibeSnapshot::mock);
    f(entry)
}

/// Load current Vibe state (backed by an in-memory snapshot for now).
pub async fn get_vibe_snapshot(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let guard = state().read().unwrap();
    Ok(guard
        .get(&project_id)
        .cloned()
        .unwrap_or_else(VibeSnapshot::mock))
}

/// Append log lines and a running task to simulate a WASI job.
pub async fn run_wasi_job(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        let new_id = snap.tasks.len() + 1;
        snap.logs
            .push(format!("[wasi] job {new_id} started on {project_id}"));
        snap.logs.push("[wasi] mounting /workspace + /cap".to_string());
        snap.tasks.push(AgentTask {
            id: new_id,
            title: format!("Run WASI job #{new_id}"),
            status: "running".to_string(),
        });
        snap.clone()
    });
    Ok(updated)
}

/// Append a log entry to simulate tailing logs.
pub async fn tail_logs(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        snap.logs
            .push(format!("[logs] streaming /logs/events.ndjson ({project_id})"));
        snap.clone()
    });
    Ok(updated)
}

/// Insert a new deployment entry.
pub async fn trigger_deploy(project_id: String) -> Result<VibeSnapshot, ServerFnError> {
    let updated = with_snapshot(&project_id, |snap| {
        let version = format!("v0.3.{}", snap.deployments.len() + 2);
        snap.deployments.insert(
            0,
            Deployment {
                version,
                status: "Deploying".to_string(),
                timestamp: chrono::Utc::now(),
            },
        );
        snap.clone()
    });
    Ok(updated)
}
