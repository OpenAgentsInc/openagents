use serde::{Deserialize, Serialize};

use crate::route::AppRoute;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntentId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandIntent {
    Bootstrap,
    RefreshSession,
    RequestSyncToken { scopes: Vec<String> },
    ConnectStream { worker_id: Option<String> },
    DisconnectStream,
    SendThreadMessage { thread_id: String, text: String },
    Navigate { route: AppRoute },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedIntent {
    pub id: IntentId,
    pub intent: CommandIntent,
}
