use serde::{Deserialize, Serialize};

use crate::intent::{CommandIntent, IntentId, QueuedIntent};
use crate::route::AppRoute;
use crate::state::{AppState, AuthStatus, AuthUser, SessionSnapshot, StreamStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppAction {
    BootstrapFromPath {
        path: String,
    },
    Navigate {
        route: AppRoute,
    },
    AuthEmailUpdated {
        email: Option<String>,
    },
    AuthChallengeRequested {
        email: String,
    },
    AuthChallengeAccepted {
        email: String,
        challenge_id: String,
    },
    AuthVerifyRequested,
    AuthSessionRestoreRequested,
    AuthSessionRefreshRequested,
    AuthSessionEstablished {
        user: AuthUser,
        session: SessionSnapshot,
        token_type: String,
        access_token: String,
        refresh_token: String,
    },
    AuthReauthRequired {
        message: String,
    },
    AuthSignedOut,
    AuthFailed {
        message: String,
    },
    AuthStatusChanged {
        status: AuthStatus,
    },
    StreamStatusChanged {
        status: StreamStatus,
    },
    ActiveWorkerChanged {
        worker_id: Option<String>,
    },
    QueueIntent {
        intent: CommandIntent,
    },
    IntentFailed {
        id: IntentId,
        message: String,
    },
    IntentCompleted {
        id: IntentId,
    },
    DrainIntents,
    ClearError,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReducerResult {
    pub drained_intents: Vec<QueuedIntent>,
}

pub fn apply_action(state: &mut AppState, action: AppAction) -> ReducerResult {
    match action {
        AppAction::BootstrapFromPath { path } => {
            let route = AppRoute::from_path(&path);
            if route != state.route {
                state.route_history.push(state.route.clone());
                state.route = route;
            }
            ReducerResult::default()
        }
        AppAction::Navigate { route } => {
            if route != state.route {
                state.route_history.push(state.route.clone());
                state.route = route;
            }
            ReducerResult::default()
        }
        AppAction::AuthEmailUpdated { email } => {
            state.auth.email = email;
            ReducerResult::default()
        }
        AppAction::AuthChallengeRequested { email } => {
            state.auth.status = AuthStatus::SendingCode;
            state.auth.email = Some(email);
            state.auth.challenge_id = None;
            state.auth.last_error = None;
            ReducerResult::default()
        }
        AppAction::AuthChallengeAccepted {
            email,
            challenge_id,
        } => {
            state.auth.status = AuthStatus::AwaitingCode;
            state.auth.email = Some(email);
            state.auth.challenge_id = Some(challenge_id);
            state.auth.last_error = None;
            ReducerResult::default()
        }
        AppAction::AuthVerifyRequested => {
            state.auth.status = AuthStatus::VerifyingCode;
            state.auth.last_error = None;
            ReducerResult::default()
        }
        AppAction::AuthSessionRestoreRequested => {
            state.auth.status = AuthStatus::SessionRestoring;
            state.auth.last_error = None;
            ReducerResult::default()
        }
        AppAction::AuthSessionRefreshRequested => {
            state.auth.status = AuthStatus::SessionRefreshing;
            state.auth.last_error = None;
            ReducerResult::default()
        }
        AppAction::AuthSessionEstablished {
            user,
            session,
            token_type,
            access_token,
            refresh_token,
        } => {
            state.auth.status = AuthStatus::SignedIn;
            state.auth.email = Some(user.email.clone());
            state.auth.challenge_id = None;
            state.auth.token_type = Some(token_type);
            state.auth.access_token = Some(access_token);
            state.auth.refresh_token = Some(refresh_token);
            state.auth.user = Some(user);
            state.auth.session = Some(session);
            state.auth.last_error = None;
            ReducerResult::default()
        }
        AppAction::AuthReauthRequired { message } => {
            state.auth.status = AuthStatus::ReauthRequired;
            state.auth.access_token = None;
            state.auth.refresh_token = None;
            state.auth.session = None;
            state.auth.last_error = Some(message);
            ReducerResult::default()
        }
        AppAction::AuthSignedOut => {
            let preserved_email = state.auth.email.clone();
            state.auth = crate::state::AuthState::default();
            state.auth.email = preserved_email;
            ReducerResult::default()
        }
        AppAction::AuthFailed { message } => {
            state.auth.last_error = Some(message);
            ReducerResult::default()
        }
        AppAction::AuthStatusChanged { status } => {
            state.auth.status = status;
            ReducerResult::default()
        }
        AppAction::StreamStatusChanged { status } => {
            state.stream.status = status;
            ReducerResult::default()
        }
        AppAction::ActiveWorkerChanged { worker_id } => {
            state.stream.active_worker_id = worker_id;
            ReducerResult::default()
        }
        AppAction::QueueIntent { intent } => {
            let id = IntentId(state.next_intent_id);
            state.next_intent_id = state.next_intent_id.saturating_add(1);
            state.intent_queue.push_back(QueuedIntent { id, intent });
            ReducerResult::default()
        }
        AppAction::IntentFailed { id, message } => {
            state.last_error = Some(format!("intent {} failed: {}", id.0, message));
            ReducerResult::default()
        }
        AppAction::IntentCompleted { id: _ } => {
            state.last_error = None;
            ReducerResult::default()
        }
        AppAction::DrainIntents => ReducerResult {
            drained_intents: state.intent_queue.drain(..).collect(),
        },
        AppAction::ClearError => {
            state.last_error = None;
            ReducerResult::default()
        }
    }
}
