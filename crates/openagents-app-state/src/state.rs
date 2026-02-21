use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::intent::QueuedIntent;
use crate::route::AppRoute;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    SignedOut,
    SendingCode,
    AwaitingCode,
    VerifyingCode,
    SessionRestoring,
    SessionRefreshing,
    SignedIn,
    ReauthRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLifecycleStatus {
    Active,
    ReauthRequired,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: String,
    pub email: String,
    pub name: String,
    pub workos_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub token_name: String,
    pub active_org_id: String,
    pub status: SessionLifecycleStatus,
    pub reauth_required: bool,
    pub issued_at: Option<String>,
    pub access_expires_at: Option<String>,
    pub refresh_expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthState {
    pub status: AuthStatus,
    pub email: Option<String>,
    pub challenge_id: Option<String>,
    pub token_type: Option<String>,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub user: Option<AuthUser>,
    pub session: Option<SessionSnapshot>,
    pub last_error: Option<String>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            status: AuthStatus::SignedOut,
            email: None,
            challenge_id: None,
            token_type: None,
            access_token: None,
            refresh_token: None,
            user: None,
            session: None,
            last_error: None,
        }
    }
}

impl AuthState {
    #[must_use]
    pub fn has_tokens(&self) -> bool {
        self.access_token.is_some() && self.refresh_token.is_some()
    }

    #[must_use]
    pub fn has_active_session(&self) -> bool {
        self.status == AuthStatus::SignedIn
            && self.session.as_ref().is_some_and(|session| {
                session.status == SessionLifecycleStatus::Active && !session.reauth_required
            })
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
    pub topic_watermarks: BTreeMap<String, u64>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            status: StreamStatus::Disconnected,
            active_worker_id: None,
            last_seq: None,
            topic_watermarks: BTreeMap::new(),
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
