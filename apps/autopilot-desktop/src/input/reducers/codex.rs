use crate::app_state::RenderState;
use crate::codex_lane::{
    CodexLaneCommandKind, CodexLaneCommandResponse, CodexLaneCommandStatus, CodexLaneNotification,
    CodexLaneSnapshot,
};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: CodexLaneSnapshot) {
    state
        .autopilot_chat
        .set_connection_status(snapshot.lifecycle.label().to_string());
    if let Some(thread_id) = snapshot.active_thread_id.as_ref() {
        state.autopilot_chat.ensure_thread(thread_id.clone());
    }
    if let Some(error) = snapshot.last_error.as_ref() {
        state.autopilot_chat.last_error = Some(error.clone());
    }
    state.codex_lane = snapshot;
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(state: &mut RenderState, response: CodexLaneCommandResponse) {
    if response.status != CodexLaneCommandStatus::Accepted {
        state.sync_health.last_error = response
            .error
            .as_ref()
            .map(|error| format!("codex {}: {error}", response.status.label()));
        if response.command == CodexLaneCommandKind::TurnStart {
            state.autopilot_chat.mark_pending_turn_dispatch_failed(
                response
                    .error
                    .clone()
                    .unwrap_or_else(|| "turn/start rejected".to_string()),
            );
        }
    } else if response.command == CodexLaneCommandKind::TurnStart {
        state.autopilot_chat.last_error = None;
    }
    state.sync_health.last_action = Some(format!(
        "codex {} {}",
        response.command.label(),
        response.status.label()
    ));
    state.record_codex_command_response(response);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_notification(state: &mut RenderState, notification: CodexLaneNotification) {
    let stored = notification.clone();
    match notification {
        CodexLaneNotification::ThreadListLoaded { thread_ids } => {
            state.autopilot_chat.set_threads(thread_ids);
        }
        CodexLaneNotification::ThreadSelected { thread_id }
        | CodexLaneNotification::ThreadStarted { thread_id } => {
            state.autopilot_chat.ensure_thread(thread_id);
        }
        CodexLaneNotification::TurnStarted { thread_id, turn_id } => {
            state.autopilot_chat.ensure_thread(thread_id);
            state.autopilot_chat.mark_turn_started(turn_id);
        }
        CodexLaneNotification::AgentMessageDelta { delta, .. } => {
            state.autopilot_chat.append_turn_delta(&delta);
        }
        CodexLaneNotification::TurnCompleted { .. } => {
            state.autopilot_chat.mark_turn_completed();
        }
        CodexLaneNotification::TurnError { message, .. } => {
            state.autopilot_chat.mark_turn_error(message);
        }
        CodexLaneNotification::ServerRequest { .. } | CodexLaneNotification::Raw { .. } => {}
    }

    state.sync_health.last_action = Some("codex notification received".to_string());
    state.record_codex_notification(stored);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}
