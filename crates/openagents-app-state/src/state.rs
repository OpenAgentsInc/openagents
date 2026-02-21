use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::intent::QueuedIntent;
use crate::route::AppRoute;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    SignedOut,
    SendingCode,
    VerifyingCode,
    SignedIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthState {
    pub status: AuthStatus,
    pub email: Option<String>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            status: AuthStatus::SignedOut,
            email: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamStatus {
    Disconnected,
    Connecting,
    Live,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamState {
    pub status: StreamStatus,
    pub active_worker_id: Option<String>,
    pub last_seq: Option<u64>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            status: StreamStatus::Disconnected,
            active_worker_id: None,
            last_seq: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppState {
    pub route: AppRoute,
    pub route_history: Vec<AppRoute>,
    pub auth: AuthState,
    pub stream: StreamState,
    pub intent_queue: VecDeque<QueuedIntent>,
    pub next_intent_id: u64,
    pub last_error: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            route: AppRoute::default(),
            route_history: Vec::new(),
            auth: AuthState::default(),
            stream: StreamState::default(),
            intent_queue: VecDeque::new(),
            next_intent_id: 1,
            last_error: None,
        }
    }
}
