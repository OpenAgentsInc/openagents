use std::path::{Path, PathBuf};
use std::pin::Pin;

use futures::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserAction {
    Message {
        session_id: SessionId,
        text: String,
        model: Option<String>,
    },
    Command {
        session_id: SessionId,
        name: String,
        args: Vec<String>,
    },
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
