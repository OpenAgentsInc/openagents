use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;
use web_time::Instant;

use crate::app::codex_app_server as app_server;
use crate::app::config::workspaces_file;

const THREAD_NAME_MAX_LEN: usize = 38;
const FOCUS_REFRESH_COOLDOWN_SECS: u64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) codex_bin: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) codex_bin: Option<String>,
    pub(crate) connected: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceThreadSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) preview: String,
    pub(crate) created_at: i64,
}

#[derive(Debug)]
pub(crate) enum WorkspaceEvent {
    WorkspacesLoaded {
        workspaces: Vec<WorkspaceInfo>,
    },
    WorkspaceAdded {
        workspace: WorkspaceInfo,
    },
    WorkspaceAddFailed {
        workspace_id: String,
        error: String,
    },
    WorkspaceConnected {
        workspace_id: String,
    },
    WorkspaceConnectFailed {
        workspace_id: String,
        error: String,
    },
    ThreadsListed {
        workspace_id: String,
        threads: Vec<WorkspaceThreadSummary>,
    },
    ThreadsListFailed {
        workspace_id: String,
        error: String,
    },
    AppServerNotification {
        workspace_id: String,
        notification: app_server::AppServerNotification,
    },
    AppServerRequest {
        workspace_id: String,
        request: app_server::AppServerRequest,
    },
}

#[derive(Debug)]
pub(crate) enum WorkspaceCommand {
    Reload,
    Add {
        path: PathBuf,
        codex_bin: Option<String>,
    },
    Connect {
        workspace_id: String,
    },
    ListThreads {
        workspace_id: String,
    },
}

pub(crate) struct WorkspaceRuntime {
    cmd_tx: mpsc::Sender<WorkspaceCommand>,
    pub(crate) event_rx: mpsc::Receiver<WorkspaceEvent>,
}

impl WorkspaceRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkspaceCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<WorkspaceEvent>(256);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_workspace_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn reload(&self) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::Reload);
    }

    pub(crate) fn add_workspace(&self, path: PathBuf, codex_bin: Option<String>) {
        let _ = self.cmd_tx.try_send(WorkspaceCommand::Add { path, codex_bin });
    }

    pub(crate) fn connect_workspace(&self, workspace_id: String) {
        let _ = self
            .cmd_tx
            .try_send(WorkspaceCommand::Connect { workspace_id });
    }

    pub(crate) fn list_threads(&self, workspace_id: String) {
        let _ = self
            .cmd_tx
            .try_send(WorkspaceCommand::ListThreads { workspace_id });
    }
}

impl Default for WorkspaceRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct WorkspaceState {
    pub(crate) runtime: WorkspaceRuntime,
    pub(crate) workspaces: Vec<WorkspaceInfo>,
    pub(crate) threads_by_workspace: HashMap<String, Vec<WorkspaceThreadSummary>>,
    pub(crate) active_workspace_id: Option<String>,
    pub(crate) status_message: Option<String>,
    last_focus_refresh: Option<Instant>,
    initial_restore_pending: bool,
}

impl WorkspaceState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: WorkspaceRuntime::new(),
            workspaces: Vec::new(),
            threads_by_workspace: HashMap::new(),
            active_workspace_id: None,
            status_message: None,
            last_focus_refresh: None,
            initial_restore_pending: true,
        }
    }

    pub(crate) fn apply_loaded(&mut self, workspaces: Vec<WorkspaceInfo>) {
        let active_id = self.active_workspace_id.clone();
        self.workspaces = workspaces;
        if let Some(active_id) = active_id {
            if self.workspaces.iter().any(|ws| ws.id == active_id) {
                self.active_workspace_id = Some(active_id);
            } else {
                self.active_workspace_id = self.workspaces.first().map(|ws| ws.id.clone());
            }
        } else {
            self.active_workspace_id = self.workspaces.first().map(|ws| ws.id.clone());
        }
    }

    pub(crate) fn apply_workspace_added(&mut self, workspace: WorkspaceInfo) {
        if self.workspaces.iter().any(|ws| ws.id == workspace.id) {
            return;
        }
        self.active_workspace_id = Some(workspace.id.clone());
        self.workspaces.push(workspace);
    }

    pub(crate) fn apply_workspace_connected(&mut self, workspace_id: &str) {
        if let Some(workspace) = self
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.id == workspace_id)
        {
            workspace.connected = true;
        }
    }

    pub(crate) fn apply_threads(
        &mut self,
        workspace_id: String,
        threads: Vec<WorkspaceThreadSummary>,
    ) {
        self.threads_by_workspace.insert(workspace_id, threads);
    }

    pub(crate) fn connect_all_if_needed(&mut self) {
        if !self.initial_restore_pending {
            return;
        }
        self.initial_restore_pending = false;
        for workspace in &self.workspaces {
            if !workspace.connected {
                self.runtime.connect_workspace(workspace.id.clone());
            }
        }
    }

    pub(crate) fn refresh_on_focus(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_focus_refresh {
            if now.duration_since(last).as_secs() < FOCUS_REFRESH_COOLDOWN_SECS {
                return;
            }
        }
        self.last_focus_refresh = Some(now);
        self.runtime.reload();
        for workspace in &self.workspaces {
            if workspace.connected {
                self.runtime.list_threads(workspace.id.clone());
            }
        }
    }

    pub(crate) fn list_workspace_summary(&self) -> String {
        if self.workspaces.is_empty() {
            return "No workspaces saved. Use /workspace add to add one.".to_string();
        }
        let mut lines = Vec::new();
        for workspace in &self.workspaces {
            let status = if workspace.connected { "connected" } else { "offline" };
            lines.push(format!(
                "- {} ({})\n  id: {}\n  path: {}",
                workspace.name, status, workspace.id, workspace.path
            ));
        }
        lines.join("\n")
    }

    pub(crate) fn connect_by_hint(&self, hint: &str) -> Option<String> {
        let trimmed = hint.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Some(workspace) = self.workspaces.iter().find(|ws| ws.id == trimmed) {
            return Some(workspace.id.clone());
        }
        if let Some(workspace) = self
            .workspaces
            .iter()
            .find(|ws| ws.id.starts_with(trimmed))
        {
            return Some(workspace.id.clone());
        }
        if let Some(workspace) = self
            .workspaces
            .iter()
            .find(|ws| ws.name.to_lowercase() == trimmed.to_lowercase())
        {
            return Some(workspace.id.clone());
        }
        None
    }
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

struct WorkspaceSession {
    entry: WorkspaceEntry,
    client: app_server::AppServerClient,
    notification_task: tokio::task::JoinHandle<()>,
    request_task: tokio::task::JoinHandle<()>,
}

async fn run_workspace_loop(
    mut cmd_rx: mpsc::Receiver<WorkspaceCommand>,
    event_tx: mpsc::Sender<WorkspaceEvent>,
) {
    let storage_path = workspaces_file();
    let mut workspaces = load_workspace_entries(&storage_path);
    let mut sessions: HashMap<String, WorkspaceSession> = HashMap::new();

    let loaded = workspaces
        .values()
        .map(|entry| entry_to_info(entry, sessions.contains_key(&entry.id)))
        .collect::<Vec<_>>();
    let _ = event_tx
        .send(WorkspaceEvent::WorkspacesLoaded { workspaces: loaded })
        .await;

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            WorkspaceCommand::Reload => {
                workspaces = load_workspace_entries(&storage_path);
                let missing_ids: Vec<String> = sessions
                    .keys()
                    .filter(|id| !workspaces.contains_key(*id))
                    .cloned()
                    .collect();
                for id in missing_ids {
                    if let Some(session) = sessions.remove(&id) {
                        session.notification_task.abort();
                        session.request_task.abort();
                        let _ = session.client.shutdown().await;
                    }
                }
                let loaded = workspaces
                    .values()
                    .map(|entry| entry_to_info(entry, sessions.contains_key(&entry.id)))
                    .collect::<Vec<_>>();
                let _ = event_tx
                    .send(WorkspaceEvent::WorkspacesLoaded { workspaces: loaded })
                    .await;
            }
            WorkspaceCommand::Add { path, codex_bin } => {
                let entry = create_workspace_entry(&path, codex_bin);
                workspaces.insert(entry.id.clone(), entry.clone());
                if let Err(err) = save_workspace_entries(&storage_path, &workspaces) {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceAddFailed {
                            workspace_id: entry.id.clone(),
                            error: err,
                        })
                        .await;
                    continue;
                }
                let info = entry_to_info(&entry, false);
                let _ = event_tx
                    .send(WorkspaceEvent::WorkspaceAdded { workspace: info })
                    .await;
                if let Err(err) = connect_workspace_session(&entry, &mut sessions, &event_tx).await
                {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceConnectFailed {
                            workspace_id: entry.id.clone(),
                            error: err,
                        })
                        .await;
                }
            }
            WorkspaceCommand::Connect { workspace_id } => {
                let entry = match workspaces.get(&workspace_id) {
                    Some(entry) => entry.clone(),
                    None => {
                        let _ = event_tx
                            .send(WorkspaceEvent::WorkspaceConnectFailed {
                                workspace_id,
                                error: "Workspace not found".to_string(),
                            })
                            .await;
                        continue;
                    }
                };
                if sessions.contains_key(&entry.id) {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceConnected {
                            workspace_id: entry.id.clone(),
                        })
                        .await;
                    continue;
                }
                if let Err(err) = connect_workspace_session(&entry, &mut sessions, &event_tx).await
                {
                    let _ = event_tx
                        .send(WorkspaceEvent::WorkspaceConnectFailed {
                            workspace_id: entry.id.clone(),
                            error: err,
                        })
                        .await;
                }
            }
            WorkspaceCommand::ListThreads { workspace_id } => {
                let Some(session) = sessions.get(&workspace_id) else {
                    let _ = event_tx
                        .send(WorkspaceEvent::ThreadsListFailed {
                            workspace_id,
                            error: "Workspace not connected".to_string(),
                        })
                        .await;
                    continue;
                };
                match list_threads_for_workspace(session).await {
                    Ok(threads) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadsListed {
                                workspace_id: session.entry.id.clone(),
                                threads,
                            })
                            .await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(WorkspaceEvent::ThreadsListFailed {
                                workspace_id: session.entry.id.clone(),
                                error: err,
                            })
                            .await;
                    }
                }
            }
        }
    }
}

async fn connect_workspace_session(
    entry: &WorkspaceEntry,
    sessions: &mut HashMap<String, WorkspaceSession>,
    event_tx: &mpsc::Sender<WorkspaceEvent>,
) -> Result<(), String> {
    let (client, channels) = app_server::AppServerClient::spawn(app_server::AppServerConfig {
        cwd: Some(PathBuf::from(&entry.path)),
        wire_log: None,
    })
    .await
    .map_err(|err| format!("Failed to spawn app-server: {}", err))?;

    let client_info = app_server::ClientInfo {
        name: "autopilot".to_string(),
        title: Some("Autopilot".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    client
        .initialize(client_info)
        .await
        .map_err(|err| format!("Failed to initialize app-server: {}", err))?;

    let workspace_id = entry.id.clone();
    let mut notification_rx = channels.notifications;
    let event_tx_notify = event_tx.clone();
    let notification_task = tokio::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            let _ = event_tx_notify
                .send(WorkspaceEvent::AppServerNotification {
                    workspace_id: workspace_id.clone(),
                    notification,
                })
                .await;
        }
    });

    let workspace_id = entry.id.clone();
    let mut request_rx = channels.requests;
    let event_tx_request = event_tx.clone();
    let request_task = tokio::spawn(async move {
        while let Some(request) = request_rx.recv().await {
            let _ = event_tx_request
                .send(WorkspaceEvent::AppServerRequest {
                    workspace_id: workspace_id.clone(),
                    request,
                })
                .await;
        }
    });

    sessions.insert(
        entry.id.clone(),
        WorkspaceSession {
            entry: entry.clone(),
            client,
            notification_task,
            request_task,
        },
    );

    let _ = event_tx
        .send(WorkspaceEvent::WorkspaceConnected {
            workspace_id: entry.id.clone(),
        })
        .await;

    if let Some(session) = sessions.get(&entry.id) {
        match list_threads_for_workspace(session).await {
            Ok(threads) => {
                let _ = event_tx
                    .send(WorkspaceEvent::ThreadsListed {
                        workspace_id: entry.id.clone(),
                        threads,
                    })
                    .await;
            }
            Err(err) => {
                let _ = event_tx
                    .send(WorkspaceEvent::ThreadsListFailed {
                        workspace_id: entry.id.clone(),
                        error: err,
                    })
                    .await;
            }
        }
    }

    Ok(())
}

async fn list_threads_for_workspace(
    session: &WorkspaceSession,
) -> Result<Vec<WorkspaceThreadSummary>, String> {
    let mut threads: Vec<app_server::ThreadSummary> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let response = session
            .client
            .thread_list(app_server::ThreadListParams {
                cursor: cursor.clone(),
                limit: Some(50),
                model_providers: None,
            })
            .await
            .map_err(|err| format!("thread/list failed: {}", err))?;
        threads.extend(response.data);
        cursor = response.next_cursor;
        if cursor.is_none() {
            break;
        }
    }

    let filtered = threads
        .into_iter()
        .filter(|thread| {
            thread
                .cwd
                .as_deref()
                .map(|cwd| cwd == session.entry.path)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    let mut ordered = filtered;
    ordered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    Ok(ordered
        .into_iter()
        .enumerate()
        .map(|(idx, thread)| {
            let preview = thread.preview.trim().to_string();
            let fallback = format!("Agent {}", idx + 1);
            WorkspaceThreadSummary {
                id: thread.id,
                name: format_thread_name(&preview, &fallback),
                preview,
                created_at: thread.created_at,
            }
        })
        .collect())
}

fn format_thread_name(preview: &str, fallback: &str) -> String {
    if preview.is_empty() {
        return fallback.to_string();
    }
    let mut chars = preview.chars();
    let truncated: String = chars.by_ref().take(THREAD_NAME_MAX_LEN).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        preview.to_string()
    }
}

fn create_workspace_entry(path: &Path, codex_bin: Option<String>) -> WorkspaceEntry {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("Workspace")
        .to_string();
    WorkspaceEntry {
        id: Uuid::new_v4().to_string(),
        name,
        path: path.to_string_lossy().to_string(),
        codex_bin,
    }
}

fn entry_to_info(entry: &WorkspaceEntry, connected: bool) -> WorkspaceInfo {
    WorkspaceInfo {
        id: entry.id.clone(),
        name: entry.name.clone(),
        path: entry.path.clone(),
        codex_bin: entry.codex_bin.clone(),
        connected,
    }
}

fn load_workspace_entries(path: &Path) -> HashMap<String, WorkspaceEntry> {
    if !path.exists() {
        return HashMap::new();
    }
    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(_) => return HashMap::new(),
    };
    let entries: Vec<WorkspaceEntry> = serde_json::from_str(&data).unwrap_or_default();
    entries
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect()
}

fn save_workspace_entries(
    path: &Path,
    entries: &HashMap<String, WorkspaceEntry>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let list: Vec<WorkspaceEntry> = entries.values().cloned().collect();
    let data = serde_json::to_string_pretty(&list).map_err(|err| err.to_string())?;
    std::fs::write(path, data).map_err(|err| err.to_string())
}
