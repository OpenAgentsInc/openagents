use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Clone, Debug)]
pub(crate) enum ConversationRole {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub(crate) enum ReviewState {
    Started,
    Completed,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConversationChange {
    pub(crate) path: String,
    pub(crate) kind: Option<String>,
    pub(crate) diff: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ToolItemData {
    pub(crate) tool_name: String,
    pub(crate) tool_type: String,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) status: Option<String>,
    pub(crate) output: String,
    pub(crate) input_value: Option<Value>,
    pub(crate) output_value: Option<Value>,
    pub(crate) changes: Vec<ConversationChange>,
}

#[derive(Clone, Debug)]
pub(crate) enum ConversationItem {
    Message {
        id: String,
        role: ConversationRole,
        text: String,
    },
    Reasoning {
        id: String,
        summary: String,
        content: String,
    },
    Tool {
        id: String,
        data: ToolItemData,
    },
    Review {
        id: String,
        state: ReviewState,
        text: String,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadStatus {
    pub(crate) is_processing: bool,
    pub(crate) has_unread: bool,
    pub(crate) is_reviewing: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ThreadTimeline {
    pub(crate) items: Vec<ConversationItem>,
    index_by_id: HashMap<String, usize>,
}

impl ThreadTimeline {
    pub(crate) fn upsert(&mut self, item: ConversationItem) {
        let id = item.id().to_string();
        if let Some(index) = self.index_by_id.get(&id).copied() {
            if let Some(existing) = self.items.get_mut(index) {
                existing.merge(item);
            }
            return;
        }
        self.index_by_id.insert(id, self.items.len());
        self.items.push(item);
    }

    pub(crate) fn append_agent_delta(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Message { text, .. }) = self.items.get_mut(index) {
                text.push_str(delta);
            }
            return;
        }
        let item = ConversationItem::Message {
            id: item_id.to_string(),
            role: ConversationRole::Assistant,
            text: delta.to_string(),
        };
        self.index_by_id.insert(item_id.to_string(), self.items.len());
        self.items.push(item);
    }

    pub(crate) fn append_reasoning_summary(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Reasoning { summary, .. }) = self.items.get_mut(index) {
                summary.push_str(delta);
            }
            return;
        }
        let item = ConversationItem::Reasoning {
            id: item_id.to_string(),
            summary: delta.to_string(),
            content: String::new(),
        };
        self.index_by_id.insert(item_id.to_string(), self.items.len());
        self.items.push(item);
    }

    pub(crate) fn append_reasoning_content(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Reasoning { content, .. }) = self.items.get_mut(index) {
                content.push_str(delta);
            }
            return;
        }
        let item = ConversationItem::Reasoning {
            id: item_id.to_string(),
            summary: String::new(),
            content: delta.to_string(),
        };
        self.index_by_id.insert(item_id.to_string(), self.items.len());
        self.items.push(item);
    }

    pub(crate) fn append_tool_output(&mut self, item_id: &str, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Some(index) = self.index_by_id.get(item_id).copied() {
            if let Some(ConversationItem::Tool { data, .. }) = self.items.get_mut(index) {
                data.output.push_str(delta);
            }
        }
    }
}

impl ConversationItem {
    pub(crate) fn id(&self) -> &str {
        match self {
            ConversationItem::Message { id, .. } => id,
            ConversationItem::Reasoning { id, .. } => id,
            ConversationItem::Tool { id, .. } => id,
            ConversationItem::Review { id, .. } => id,
        }
    }

    fn merge(&mut self, incoming: ConversationItem) {
        match (self, incoming) {
            (
                ConversationItem::Message { role, text, .. },
                ConversationItem::Message {
                    role: new_role,
                    text: new_text,
                    ..
                },
            ) => {
                *role = new_role;
                if !new_text.is_empty() {
                    *text = new_text;
                }
            }
            (
                ConversationItem::Reasoning { summary, content, .. },
                ConversationItem::Reasoning {
                    summary: new_summary,
                    content: new_content,
                    ..
                },
            ) => {
                if !new_summary.is_empty() {
                    *summary = new_summary;
                }
                if !new_content.is_empty() {
                    *content = new_content;
                }
            }
            (
                ConversationItem::Tool { data, .. },
                ConversationItem::Tool { data: new_data, .. },
            ) => {
                data.merge(new_data);
            }
            (
                ConversationItem::Review { state, text, .. },
                ConversationItem::Review {
                    state: new_state,
                    text: new_text,
                    ..
                },
            ) => {
                *state = new_state;
                if !new_text.is_empty() {
                    *text = new_text;
                }
            }
            (slot, incoming) => {
                *slot = incoming;
            }
        }
    }
}

impl ToolItemData {
    fn merge(&mut self, incoming: ToolItemData) {
        if !incoming.tool_name.is_empty() {
            self.tool_name = incoming.tool_name;
        }
        if !incoming.tool_type.is_empty() {
            self.tool_type = incoming.tool_type;
        }
        if !incoming.title.is_empty() {
            self.title = incoming.title;
        }
        if !incoming.detail.is_empty() {
            self.detail = incoming.detail;
        }
        if let Some(status) = incoming.status {
            if !status.trim().is_empty() {
                self.status = Some(status);
            }
        }
        if !incoming.output.is_empty() {
            self.output = incoming.output;
        }
        if let Some(input_value) = incoming.input_value {
            self.input_value = Some(input_value);
        }
        if let Some(output_value) = incoming.output_value {
            self.output_value = Some(output_value);
        }
        if !incoming.changes.is_empty() {
            self.changes = incoming.changes;
        }
    }
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
    pub(crate) active_thread_by_workspace: HashMap<String, String>,
    pub(crate) thread_status_by_id: HashMap<String, ThreadStatus>,
    pub(crate) timelines_by_thread: HashMap<String, ThreadTimeline>,
    pub(crate) timeline_dirty: bool,
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
            active_thread_by_workspace: HashMap::new(),
            thread_status_by_id: HashMap::new(),
            timelines_by_thread: HashMap::new(),
            timeline_dirty: false,
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
        self.timeline_dirty = true;
    }

    pub(crate) fn apply_workspace_added(&mut self, workspace: WorkspaceInfo) {
        if self.workspaces.iter().any(|ws| ws.id == workspace.id) {
            return;
        }
        self.active_workspace_id = Some(workspace.id.clone());
        self.workspaces.push(workspace);
        self.timeline_dirty = true;
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
        for thread in &threads {
            self.thread_status_by_id
                .entry(thread.id.clone())
                .or_default();
        }
        if let Some(first) = threads.first() {
            self.active_thread_by_workspace
                .entry(workspace_id.clone())
                .or_insert_with(|| first.id.clone());
        }
        self.threads_by_workspace.insert(workspace_id, threads);
        self.timeline_dirty = true;
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

    pub(crate) fn set_active_workspace(&mut self, workspace_id: String) {
        if self.active_workspace_id.as_deref() == Some(&workspace_id) {
            return;
        }
        self.active_workspace_id = Some(workspace_id);
        self.timeline_dirty = true;
    }

    pub(crate) fn active_thread_id(&self) -> Option<String> {
        let workspace_id = self.active_workspace_id.as_ref()?;
        self.active_thread_by_workspace
            .get(workspace_id)
            .cloned()
            .or_else(|| {
                self.threads_by_workspace
                    .get(workspace_id)
                    .and_then(|threads| threads.first().map(|thread| thread.id.clone()))
            })
    }

    pub(crate) fn set_active_thread(&mut self, workspace_id: &str, thread_id: &str) {
        self.active_thread_by_workspace
            .insert(workspace_id.to_string(), thread_id.to_string());
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.has_unread = false;
        }
        self.timeline_dirty = true;
    }

    pub(crate) fn ensure_thread(&mut self, workspace_id: &str, thread_id: &str) {
        let threads = self.threads_by_workspace.entry(workspace_id.to_string()).or_default();
        if !threads.iter().any(|thread| thread.id == thread_id) {
            let name = format!("Agent {}", threads.len() + 1);
            threads.push(WorkspaceThreadSummary {
                id: thread_id.to_string(),
                name,
                preview: String::new(),
                created_at: 0,
            });
        }
        self.thread_status_by_id
            .entry(thread_id.to_string())
            .or_default();
        if !self.active_thread_by_workspace.contains_key(workspace_id) {
            self.active_thread_by_workspace
                .insert(workspace_id.to_string(), thread_id.to_string());
        }
    }

    pub(crate) fn timeline_for_thread_mut(&mut self, thread_id: &str) -> &mut ThreadTimeline {
        self.timelines_by_thread
            .entry(thread_id.to_string())
            .or_default()
    }

    pub(crate) fn active_timeline(&self) -> Option<&ThreadTimeline> {
        let thread_id = self.active_thread_id()?;
        self.timelines_by_thread.get(&thread_id)
    }

    pub(crate) fn mark_processing(&mut self, workspace_id: &str, thread_id: &str, active: bool) {
        self.ensure_thread(workspace_id, thread_id);
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.is_processing = active;
        }
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn mark_reviewing(&mut self, workspace_id: &str, thread_id: &str, active: bool) {
        self.ensure_thread(workspace_id, thread_id);
        if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
            status.is_reviewing = active;
        }
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn mark_unread(&mut self, workspace_id: &str, thread_id: &str) {
        self.ensure_thread(workspace_id, thread_id);
        if !self.is_active_thread(workspace_id, thread_id) {
            if let Some(status) = self.thread_status_by_id.get_mut(thread_id) {
                status.has_unread = true;
            }
        }
    }

    pub(crate) fn apply_item_update(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item: ConversationItem,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.upsert(item);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_agent_delta(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_agent_delta(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_reasoning_summary(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_reasoning_summary(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_reasoning_content(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_reasoning_content(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    pub(crate) fn append_tool_output(
        &mut self,
        workspace_id: &str,
        thread_id: &str,
        item_id: &str,
        delta: &str,
    ) {
        self.ensure_thread(workspace_id, thread_id);
        let timeline = self.timeline_for_thread_mut(thread_id);
        timeline.append_tool_output(item_id, delta);
        self.mark_dirty_for(workspace_id, thread_id);
    }

    fn is_active_thread(&self, workspace_id: &str, thread_id: &str) -> bool {
        self.active_thread_by_workspace
            .get(workspace_id)
            .map(|active| active == thread_id)
            .unwrap_or(false)
    }

    fn mark_dirty_for(&mut self, workspace_id: &str, thread_id: &str) {
        if self.is_active_thread(workspace_id, thread_id) {
            self.timeline_dirty = true;
        } else {
            self.mark_unread(workspace_id, thread_id);
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

pub(crate) fn conversation_item_from_value(item: &Value) -> Option<ConversationItem> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    let id = item.get("id").and_then(Value::as_str)?.to_string();
    match item_type {
        "userMessage" => {
            let content = item.get("content").and_then(Value::as_array).cloned().unwrap_or_default();
            let text = user_inputs_to_text(&content);
            Some(ConversationItem::Message {
                id,
                role: ConversationRole::User,
                text: if text.is_empty() { "[message]".to_string() } else { text },
            })
        }
        "agentMessage" => {
            let text = item.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            Some(ConversationItem::Message {
                id,
                role: ConversationRole::Assistant,
                text,
            })
        }
        "reasoning" => {
            let summary = item
                .get("summary")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts))
                .or_else(|| item.get("summary").and_then(Value::as_str).map(|s| s.to_string()))
                .unwrap_or_default();
            let content = item
                .get("content")
                .and_then(Value::as_array)
                .map(|parts| join_string_array(parts))
                .or_else(|| item.get("content").and_then(Value::as_str).map(|s| s.to_string()))
                .unwrap_or_default();
            Some(ConversationItem::Reasoning { id, summary, content })
        }
        "commandExecution" => {
            let command = command_string_from_item(item);
            let cwd = item.get("cwd").and_then(Value::as_str).unwrap_or("").to_string();
            let title = if command.is_empty() {
                "Command".to_string()
            } else {
                format!("Command: {}", command)
            };
            let output = item
                .get("aggregatedOutput")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let mut data = ToolItemData {
                tool_name: "Bash".to_string(),
                tool_type: item_type.to_string(),
                title,
                detail: cwd,
                status: item.get("status").and_then(Value::as_str).map(|s| s.to_string()),
                output,
                ..ToolItemData::default()
            };
            if !command.is_empty() {
                data.input_value = Some(serde_json::json!({ "command": command }));
            }
            Some(ConversationItem::Tool { id, data })
        }
        "fileChange" => {
            let (changes, paths, first_path, first_diff) = parse_file_changes(item);
            let detail = if paths.is_empty() {
                "Pending changes".to_string()
            } else {
                paths.join(", ")
            };
            let output = changes
                .iter()
                .filter_map(|change| change.diff.clone())
                .filter(|diff| !diff.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut data = ToolItemData {
                tool_name: "Edit".to_string(),
                tool_type: item_type.to_string(),
                title: "File changes".to_string(),
                detail,
                status: item.get("status").and_then(Value::as_str).map(|s| s.to_string()),
                output,
                changes,
                ..ToolItemData::default()
            };
            if !paths.is_empty() || first_path.is_some() {
                data.input_value = Some(serde_json::json!({
                    "files": paths,
                    "file_path": first_path,
                }));
            }
            if let Some(diff) = first_diff {
                data.output_value = Some(serde_json::json!({ "diff": diff }));
            }
            Some(ConversationItem::Tool { id, data })
        }
        "mcpToolCall" => {
            let server = item_string(item, "server");
            let tool = item_string(item, "tool");
            let tool_name = match (server.as_deref(), tool.as_deref()) {
                (Some(server), Some(tool)) => format!("mcp__{}__{}", server, tool),
                (Some(server), None) => format!("mcp__{}__", server),
                _ => "mcp__tool".to_string(),
            };
            let args = item.get("arguments").cloned();
            let detail = args
                .as_ref()
                .map(|value| serde_json::to_string_pretty(value).unwrap_or_default())
                .unwrap_or_default();
            let output_value = item.get("result").cloned().or_else(|| item.get("error").cloned());
            let output = output_value
                .as_ref()
                .map(|value| serde_json::to_string_pretty(value).unwrap_or_default())
                .unwrap_or_default();
            let title = match (server.as_deref(), tool.as_deref()) {
                (Some(server), Some(tool)) => format!("Tool: {} / {}", server, tool),
                (Some(server), None) => format!("Tool: {}", server),
                _ => "MCP tool".to_string(),
            };
            let data = ToolItemData {
                tool_name,
                tool_type: item_type.to_string(),
                title,
                detail,
                status: item.get("status").and_then(Value::as_str).map(|s| s.to_string()),
                output,
                input_value: args,
                output_value,
                ..ToolItemData::default()
            };
            Some(ConversationItem::Tool { id, data })
        }
        "webSearch" => {
            let query = item_string(item, "query").unwrap_or_default();
            let data = ToolItemData {
                tool_name: "Search".to_string(),
                tool_type: item_type.to_string(),
                title: "Web search".to_string(),
                detail: query.clone(),
                status: item.get("status").and_then(Value::as_str).map(|s| s.to_string()),
                input_value: if query.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({ "query": query }))
                },
                ..ToolItemData::default()
            };
            Some(ConversationItem::Tool { id, data })
        }
        "imageView" => {
            let path = item_string(item, "path").unwrap_or_default();
            let data = ToolItemData {
                tool_name: "Read".to_string(),
                tool_type: item_type.to_string(),
                title: "Image view".to_string(),
                detail: path.clone(),
                status: item.get("status").and_then(Value::as_str).map(|s| s.to_string()),
                input_value: if path.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({ "file_path": path }))
                },
                ..ToolItemData::default()
            };
            Some(ConversationItem::Tool { id, data })
        }
        "enteredReviewMode" => {
            let text = item_string(item, "review").unwrap_or_default();
            Some(ConversationItem::Review {
                id,
                state: ReviewState::Started,
                text,
            })
        }
        "exitedReviewMode" => {
            let text = item_string(item, "review").unwrap_or_default();
            Some(ConversationItem::Review {
                id,
                state: ReviewState::Completed,
                text,
            })
        }
        _ => None,
    }
}

pub(crate) fn turn_diff_item(turn_id: &str, diff: &str) -> ConversationItem {
    let data = ToolItemData {
        tool_name: "Diff".to_string(),
        tool_type: "diff".to_string(),
        title: "Turn diff".to_string(),
        detail: String::new(),
        status: Some("updated".to_string()),
        output: diff.to_string(),
        output_value: Some(serde_json::json!({ "diff": diff })),
        ..ToolItemData::default()
    };
    ConversationItem::Tool {
        id: format!("turn-diff-{}", turn_id),
        data,
    }
}

fn join_string_array(parts: &[Value]) -> String {
    parts
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn item_string(item: &Value, key: &str) -> Option<String> {
    item.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

fn command_string_from_item(item: &Value) -> String {
    item.get("command")
        .and_then(value_to_command_string)
        .unwrap_or_default()
}

fn value_to_command_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            if text.trim().is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Array(parts) => {
            let items: Vec<&str> = parts.iter().filter_map(|val| val.as_str()).collect();
            if items.is_empty() {
                None
            } else {
                Some(items.join(" "))
            }
        }
        _ => None,
    }
}

fn parse_file_changes(
    item: &Value,
) -> (Vec<ConversationChange>, Vec<String>, Option<String>, Option<String>) {
    let mut changes = Vec::new();
    let mut paths = Vec::new();
    let mut first_path = None;
    let mut first_diff = None;
    if let Some(change_list) = item.get("changes").and_then(Value::as_array) {
        for change in change_list {
            let path = change.get("path").and_then(Value::as_str).unwrap_or("").to_string();
            if path.is_empty() {
                continue;
            }
            if first_path.is_none() {
                first_path = Some(path.clone());
            }
            let kind = change
                .get("kind")
                .and_then(|value| {
                    if value.is_string() {
                        value.as_str().map(|s| s.to_string())
                    } else {
                        value.get("type").and_then(Value::as_str).map(|s| s.to_string())
                    }
                })
                .map(|value| value.to_ascii_lowercase());
            let diff = change.get("diff").and_then(Value::as_str).map(|s| s.to_string());
            if first_diff.is_none() {
                first_diff = diff.clone();
            }
            let prefix = match kind.as_deref() {
                Some("add") => "A",
                Some("delete") => "D",
                Some(_) => "M",
                None => "",
            };
            let display = if prefix.is_empty() {
                path.clone()
            } else {
                format!("{} {}", prefix, path)
            };
            paths.push(display);
            changes.push(ConversationChange { path, kind, diff });
        }
    }
    (changes, paths, first_path, first_diff)
}

fn user_inputs_to_text(content: &[Value]) -> String {
    content
        .iter()
        .filter_map(|input| {
            let input_type = input.get("type").and_then(Value::as_str)?;
            match input_type {
                "text" => input.get("text").and_then(Value::as_str).map(|s| s.to_string()),
                "skill" => input
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|s| format!("${}", s)),
                "image" | "localImage" => Some("[image]".to_string()),
                _ => None,
            }
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
