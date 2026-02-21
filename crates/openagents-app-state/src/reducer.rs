use serde::{Deserialize, Serialize};

use crate::intent::{CommandIntent, IntentId, QueuedIntent};
use crate::route::AppRoute;
use crate::state::{AppState, AuthStatus, StreamStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppAction {
    BootstrapFromPath { path: String },
    Navigate { route: AppRoute },
    AuthStatusChanged { status: AuthStatus },
    StreamStatusChanged { status: StreamStatus },
    ActiveWorkerChanged { worker_id: Option<String> },
    QueueIntent { intent: CommandIntent },
    IntentFailed { id: IntentId, message: String },
    IntentCompleted { id: IntentId },
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
