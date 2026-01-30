use std::fmt;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use futures::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

mod replay;

pub use replay::{EventRecorder, ReplayKind, ReplayReader, ReplayRecord};

const DEFAULT_EVENT_BUFFER: usize = 256;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub event_buffer: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            event_buffer: DEFAULT_EVENT_BUFFER,
        }
    }
}

#[derive(Debug, Clone)]
pub struct App {
    event_tx: broadcast::Sender<AppEvent>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let (event_tx, _) = broadcast::channel(config.event_buffer.max(1));
        Self { event_tx }
    }

    pub fn events(&self) -> AppEventStream {
        stream_from_broadcast(self.event_tx.subscribe())
    }

    pub fn open_workspace(&self, path: impl Into<PathBuf>) -> WorkspaceHandle {
        let path = path.into();
        let workspace_id = WorkspaceId::new();
        let event = AppEvent::WorkspaceOpened {
            workspace_id,
            path: path.clone(),
        };
        let _ = self.event_tx.send(event.clone());
        WorkspaceHandle {
            workspace_id,
            path,
            event_tx: self.event_tx.clone(),
            initial_events: vec![event],
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(AppConfig::default())
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceHandle {
    workspace_id: WorkspaceId,
    path: PathBuf,
    event_tx: broadcast::Sender<AppEvent>,
    initial_events: Vec<AppEvent>,
}

impl WorkspaceHandle {
    pub fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn events(&self) -> AppEventStream {
        let initial = self.initial_events.clone();
        let initial_stream = futures::stream::iter(initial);
        let live_stream = stream_from_broadcast(self.event_tx.subscribe());
        Box::pin(initial_stream.chain(live_stream))
    }

    pub fn start_session(&self, label: Option<String>) -> SessionHandle {
        let session_id = SessionId::new();
        let event = AppEvent::SessionStarted {
            workspace_id: self.workspace_id,
            session_id,
            label: label.clone(),
        };
        let _ = self.event_tx.send(event);
        SessionHandle {
            workspace_id: self.workspace_id,
            session_id,
            event_tx: self.event_tx.clone(),
            label,
        }
    }

    pub fn dispatch(&self, action: UserAction) {
        let _ = self.event_tx.send(AppEvent::UserActionDispatched {
            workspace_id: self.workspace_id,
            action,
        });
    }
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    workspace_id: WorkspaceId,
    session_id: SessionId,
    event_tx: broadcast::Sender<AppEvent>,
    label: Option<String>,
}

impl SessionHandle {
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn dispatch(&self, action: UserAction) {
        let _ = self.event_tx.send(AppEvent::UserActionDispatched {
            workspace_id: self.workspace_id,
            action,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserAction {
    Message {
        session_id: SessionId,
        text: String,
        model: Option<String>,
        reasoning: Option<String>,
    },
    NewChat {
        session_id: SessionId,
        model: Option<String>,
    },
    PylonInit,
    PylonStart,
    PylonStop,
    PylonRefresh,
    WalletRefresh,
    DvmProviderStart,
    DvmProviderStop,
    DvmProviderRefresh,
    DvmHistoryRefresh,
    Nip90Submit {
        kind: u16,
        prompt: String,
        relays: Vec<String>,
        provider: Option<String>,
    },
    MoltbookRefresh,
    MoltbookSay {
        text: String,
        submolt: Option<String>,
    },
    MoltbookComment {
        post_id: String,
        text: String,
    },
    MoltbookUpvote {
        post_id: String,
    },
    ThreadsRefresh,
    ThreadsLoadMore {
        cursor: Option<String>,
    },
    ThreadOpen {
        thread_id: String,
    },
    OpenFile {
        path: String,
    },
    SaveFile {
        path: String,
        contents: String,
    },
    Command {
        session_id: SessionId,
        name: String,
        args: Vec<String>,
    },
    Interrupt {
        session_id: SessionId,
        thread_id: Option<String>,
        turn_id: Option<String>,
    },
    FullAutoToggle {
        session_id: SessionId,
        enabled: bool,
        thread_id: Option<String>,
        continue_prompt: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PylonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_secs: Option<u64>,
    pub provider_active: Option<bool>,
    pub host_active: Option<bool>,
    pub jobs_completed: u64,
    pub earnings_msats: u64,
    pub identity_exists: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WalletStatus {
    pub network: Option<String>,
    pub spark_sats: u64,
    pub lightning_sats: u64,
    pub onchain_sats: u64,
    pub total_sats: u64,
    pub spark_address: Option<String>,
    pub bitcoin_address: Option<String>,
    pub identity_exists: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DvmProviderStatus {
    pub running: bool,
    pub provider_active: Option<bool>,
    pub host_active: Option<bool>,
    pub min_price_msats: u64,
    pub require_payment: bool,
    pub default_model: String,
    pub backend_preference: Vec<String>,
    pub network: String,
    pub enable_payments: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DvmJobSummary {
    pub id: String,
    pub status: String,
    pub kind: u16,
    pub price_msats: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DvmEarningsSummary {
    pub total_msats: u64,
    pub total_sats: u64,
    pub job_count: u64,
    pub by_source: Vec<(String, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DvmHistorySnapshot {
    pub summary: DvmEarningsSummary,
    pub status_counts: Vec<(String, u64)>,
    pub jobs: Vec<DvmJobSummary>,
    pub last_error: Option<String>,
}

/// Summary of a Moltbook post for the UI feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoltbookPostSummary {
    pub id: String,
    pub title: Option<String>,
    pub content_preview: Option<String>,
    pub author_name: Option<String>,
    pub score: Option<i64>,
    pub comment_count: Option<u64>,
    pub created_at: Option<String>,
    pub submolt: Option<String>,
}

/// Summary of the current Moltbook agent profile (stats for engagement UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MoltbookProfileSummary {
    pub agent_name: String,
    pub posts_count: u64,
    pub comments_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub preview: String,
    pub model_provider: String,
    pub cwd: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadTurn {
    pub id: String,
    pub items: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadSnapshot {
    pub id: String,
    pub preview: String,
    pub turns: Vec<ThreadTurn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppEvent {
    WorkspaceOpened {
        workspace_id: WorkspaceId,
        path: PathBuf,
    },
    SessionStarted {
        workspace_id: WorkspaceId,
        session_id: SessionId,
        label: Option<String>,
    },
    UserActionDispatched {
        workspace_id: WorkspaceId,
        action: UserAction,
    },
    AppServerEvent {
        message: String,
    },
    PylonStatus {
        status: PylonStatus,
    },
    WalletStatus {
        status: WalletStatus,
    },
    DvmProviderStatus {
        status: DvmProviderStatus,
    },
    DvmHistory {
        snapshot: DvmHistorySnapshot,
    },
    Nip90Log {
        message: String,
    },
    MoltbookFeedUpdated {
        posts: Vec<MoltbookPostSummary>,
    },
    MoltbookLog {
        message: String,
    },
    MoltbookProfileLoaded {
        profile: MoltbookProfileSummary,
    },
    ThreadsUpdated {
        threads: Vec<ThreadSummary>,
        next_cursor: Option<String>,
        append: bool,
    },
    ThreadLoaded {
        session_id: SessionId,
        thread: ThreadSnapshot,
        model: String,
    },
    FileOpened {
        path: PathBuf,
        contents: String,
    },
    FileOpenFailed {
        path: PathBuf,
        error: String,
    },
    FileSaved {
        path: PathBuf,
    },
    FileSaveFailed {
        path: PathBuf,
        error: String,
    },
}

pub type AppEventStream = Pin<Box<dyn Stream<Item = AppEvent> + Send>>;

fn stream_from_broadcast(receiver: broadcast::Receiver<AppEvent>) -> AppEventStream {
    Box::pin(BroadcastStream::new(receiver).filter_map(|item| async move { item.ok() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn emits_workspace_open_event() {
        let app = App::default();
        let workspace = app.open_workspace("/tmp/autopilot-test");
        let mut events = workspace.events();

        let next = events.next().await.expect("expected event");
        match next {
            AppEvent::WorkspaceOpened { path, .. } => {
                assert_eq!(path, PathBuf::from("/tmp/autopilot-test"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
