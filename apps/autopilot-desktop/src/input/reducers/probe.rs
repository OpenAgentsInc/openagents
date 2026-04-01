use crate::app_state::{
    AutopilotRole, AutopilotThreadListEntry, RenderState,
};
use crate::probe_lane::{
    ProbeLaneCommandResponse, ProbeLaneLifecycle, ProbeLaneNotification, ProbeLaneSnapshot,
};
use probe_protocol::runtime::{QueuedTurnStatus, RuntimeProgressEvent};
use probe_protocol::session::{SessionMetadata, TranscriptEvent, TranscriptItemKind};

fn probe_live_turn_id(session_id: &str) -> String {
    format!("probe-live:{session_id}")
}

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: ProbeLaneSnapshot) {
    let lifecycle = snapshot.lifecycle;
    state.probe_lane = snapshot;
    if state.uses_probe_runtime() {
        state
            .autopilot_chat
            .set_connection_status(lifecycle.label().to_string());
        if lifecycle == ProbeLaneLifecycle::Error || lifecycle == ProbeLaneLifecycle::Disconnected {
            state.autopilot_chat.last_error = state.probe_lane.last_error.clone();
        }
    }
}

pub(super) fn apply_command_response(state: &mut RenderState, response: ProbeLaneCommandResponse) {
    if state.uses_probe_runtime() && response.status == crate::probe_lane::ProbeLaneCommandStatus::Error {
        state.autopilot_chat.last_error = response.error.clone();
    }
    state.record_probe_command_response(response);
}

pub(super) fn apply_notification(state: &mut RenderState, notification: ProbeLaneNotification) {
    match &notification {
        ProbeLaneNotification::SessionsListed {
            sessions,
            workspace_session_id,
        } => {
            let entries = sessions
                .iter()
                .map(session_metadata_to_thread_entry)
                .collect::<Vec<_>>();
            state.autopilot_chat.set_thread_entries(entries);
            if let Some(session_id) = workspace_session_id.as_deref() {
                state.autopilot_chat.ensure_thread(session_id.to_string());
            }
            if state.uses_probe_runtime() {
                state.autopilot_chat.last_error = None;
            }
        }
        ProbeLaneNotification::SessionLoaded { snapshot, control } => {
            let thread_id = snapshot.session.id.as_str().to_string();
            state.autopilot_chat.ensure_thread(thread_id.clone());
            state.autopilot_chat.set_thread_name(
                thread_id.as_str(),
                Some(snapshot.session.title.clone()),
            );
            state.autopilot_chat.set_thread_preview(
                thread_id.as_str(),
                probe_session_preview(&snapshot.session, snapshot.transcript.as_slice()),
            );
            state.autopilot_chat.set_thread_status(
                thread_id.as_str(),
                Some(probe_status_label(&snapshot.session, control)),
            );
            state.autopilot_chat.set_thread_workspace_location(
                thread_id.as_str(),
                Some(snapshot.session.cwd.display().to_string()),
                Some(snapshot.session.transcript_path.display().to_string()),
            );
            state.autopilot_chat.set_active_thread_transcript(
                thread_id.as_str(),
                probe_transcript_messages(snapshot.transcript.as_slice()),
            );
            hydrate_probe_pending_approvals(state, &thread_id, snapshot.pending_approvals.as_slice());
            sync_probe_turn_status(state, control);
            if state.uses_probe_runtime() {
                state
                    .autopilot_chat
                    .set_connection_status(state.probe_lane.lifecycle.label().to_string());
                state.autopilot_chat.last_error = None;
            }
        }
        ProbeLaneNotification::RuntimeProgress { session_id, event } => {
            apply_runtime_progress(state, session_id.as_str(), event);
        }
        ProbeLaneNotification::PendingApprovalsUpdated {
            session_id,
            approvals,
        } => {
            hydrate_probe_pending_approvals(state, session_id.as_str(), approvals.as_slice());
            if approvals.is_empty() && state.autopilot_chat.last_turn_status.as_deref() == Some("paused") {
                state
                    .autopilot_chat
                    .set_turn_status(Some(String::from("running")));
            }
        }
        ProbeLaneNotification::TurnQueued { response, control } => {
            state.autopilot_chat.set_turn_status(Some(String::from("queued")));
            state.autopilot_chat.set_thread_status(
                response.turn.session_id.as_str(),
                Some(probe_control_status_label(control)),
            );
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe queued turn: {} position={}",
                response.turn.turn_id,
                response.turn.queue_position.unwrap_or(0)
            ));
        }
        ProbeLaneNotification::TurnInterrupted { response, control } => {
            state.autopilot_chat.set_turn_status(Some(String::from("cancelled")));
            state.autopilot_chat.set_thread_status(
                response.session_id.as_str(),
                Some(probe_control_status_label(control)),
            );
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe interrupt: {}", response.message));
        }
        ProbeLaneNotification::QueuedTurnCancelled { response, control } => {
            state.autopilot_chat.set_turn_status(Some(String::from("cancelled")));
            state.autopilot_chat.set_thread_status(
                response.session_id.as_str(),
                Some(probe_control_status_label(control)),
            );
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe queue cancel: {}", response.message));
        }
    }
    state.record_probe_notification(notification);
}

fn apply_runtime_progress(state: &mut RenderState, session_id: &str, event: &RuntimeProgressEvent) {
    let turn_id = probe_live_turn_id(session_id);
    match event {
        RuntimeProgressEvent::TurnStarted { .. } => {
            state.autopilot_chat.mark_turn_started(turn_id);
            state
                .autopilot_chat
                .set_turn_status(Some(String::from("running")));
            state
                .autopilot_chat
                .set_thread_status(session_id, Some(String::from("running")));
        }
        RuntimeProgressEvent::AssistantDelta { delta, .. } => {
            state
                .autopilot_chat
                .append_turn_delta_for_turn(turn_id.as_str(), delta);
        }
        RuntimeProgressEvent::AssistantSnapshot { snapshot, .. } => {
            state
                .autopilot_chat
                .set_turn_message_for_turn(turn_id.as_str(), snapshot);
        }
        RuntimeProgressEvent::ModelRequestFailed { error, .. } => {
            state
                .autopilot_chat
                .mark_turn_error_for(turn_id.as_str(), error.clone());
        }
        RuntimeProgressEvent::AssistantTurnCommitted { assistant_text, .. } => {
            state
                .autopilot_chat
                .set_turn_message_for_turn(turn_id.as_str(), assistant_text);
            state
                .autopilot_chat
                .mark_turn_completed_for(turn_id.as_str());
        }
        RuntimeProgressEvent::ToolExecutionStarted { tool_name, .. } => {
            state
                .autopilot_chat
                .record_turn_timeline_event(format!("probe tool started: {tool_name}"));
        }
        RuntimeProgressEvent::ToolExecutionCompleted { tool, .. } => {
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe tool completed: {} ({:?})",
                tool.name, tool.tool_execution.risk_class
            ));
        }
        RuntimeProgressEvent::ToolRefused { tool, .. } => {
            state.autopilot_chat.set_turn_status(Some(String::from("refused")));
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe tool refused: {}",
                tool.name
            ));
        }
        RuntimeProgressEvent::ToolPaused { tool, .. } => {
            state.autopilot_chat.set_turn_status(Some(String::from("paused")));
            state.autopilot_chat.record_turn_timeline_event(format!(
                "probe tool paused: {}",
                tool.name
            ));
        }
        RuntimeProgressEvent::AssistantStreamFinished { .. }
        | RuntimeProgressEvent::ModelRequestStarted { .. }
        | RuntimeProgressEvent::AssistantStreamStarted { .. }
        | RuntimeProgressEvent::TimeToFirstTokenObserved { .. }
        | RuntimeProgressEvent::ToolCallDelta { .. }
        | RuntimeProgressEvent::ToolCallRequested { .. } => {}
    }
}

fn hydrate_probe_pending_approvals(
    state: &mut RenderState,
    thread_id: &str,
    approvals: &[probe_protocol::session::PendingToolApproval],
) {
    state.autopilot_chat.pending_command_approvals = approvals
        .iter()
        .map(|approval| crate::app_state::AutopilotApprovalRequest {
            request_id: codex_client::AppServerRequestId::String(format!(
                "probe-approval-{}",
                approval.tool_call_id
            )),
            thread_id: thread_id.to_string(),
            turn_id: probe_live_turn_id(thread_id),
            item_id: approval.tool_call_id.clone(),
            reason: approval
                .reason
                .clone()
                .or_else(|| Some(format!("{} {:?}", approval.tool_name, approval.risk_class))),
            command: Some(approval.tool_name.clone()),
            cwd: Some(thread_id.to_string()),
        })
        .collect();
}

fn sync_probe_turn_status(state: &mut RenderState, control: &probe_protocol::runtime::InspectSessionTurnsResponse) {
    if let Some(active_turn) = control.active_turn.as_ref() {
        let status = if active_turn.awaiting_approval {
            "paused"
        } else {
            "running"
        };
        state
            .autopilot_chat
            .set_turn_status(Some(status.to_string()));
        return;
    }
    if let Some(recent_turn) = control.recent_turns.first() {
        let status = match recent_turn.status {
            QueuedTurnStatus::Queued => "queued",
            QueuedTurnStatus::Running => {
                if recent_turn.awaiting_approval {
                    "paused"
                } else {
                    "running"
                }
            }
            QueuedTurnStatus::Completed => "completed",
            QueuedTurnStatus::Failed => {
                if recent_turn
                    .failure_message
                    .as_deref()
                    .is_some_and(|message| message.to_ascii_lowercase().contains("timed out"))
                {
                    "timed_out"
                } else {
                    "failed"
                }
            }
            QueuedTurnStatus::Cancelled => "cancelled",
            QueuedTurnStatus::TimedOut => "timed_out",
        };
        state.autopilot_chat.set_turn_status(Some(status.to_string()));
    }
}

fn session_metadata_to_thread_entry(session: &SessionMetadata) -> AutopilotThreadListEntry {
    AutopilotThreadListEntry {
        thread_id: session.id.as_str().to_string(),
        thread_name: Some(session.title.clone()),
        preview: session.cwd.display().to_string(),
        status: Some(match session.state {
            probe_protocol::session::SessionState::Active => String::from("active"),
            probe_protocol::session::SessionState::Archived => String::from("archived"),
        }),
        loaded: true,
        cwd: Some(session.cwd.display().to_string()),
        path: Some(session.transcript_path.display().to_string()),
        created_at: session.created_at_ms as i64,
        updated_at: session.updated_at_ms as i64,
    }
}

fn probe_session_preview(
    session: &SessionMetadata,
    transcript: &[TranscriptEvent],
) -> Option<String> {
    transcript
        .iter()
        .flat_map(|event| event.turn.items.iter())
        .find(|item| matches!(item.kind, TranscriptItemKind::UserMessage))
        .map(|item| item.text.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| Some(session.cwd.display().to_string()))
}

fn probe_status_label(
    session: &SessionMetadata,
    control: &probe_protocol::runtime::InspectSessionTurnsResponse,
) -> String {
    let state = match session.state {
        probe_protocol::session::SessionState::Active => "active",
        probe_protocol::session::SessionState::Archived => "archived",
    };
    format!("{state}:{}", probe_control_status_label(control))
}

fn probe_control_status_label(
    control: &probe_protocol::runtime::InspectSessionTurnsResponse,
) -> String {
    if let Some(active_turn) = control.active_turn.as_ref() {
        return if active_turn.awaiting_approval {
            String::from("paused")
        } else {
            String::from("running")
        };
    }
    if !control.queued_turns.is_empty() {
        return String::from("queued");
    }
    if let Some(recent_turn) = control.recent_turns.first() {
        return match recent_turn.status {
            QueuedTurnStatus::Queued => String::from("queued"),
            QueuedTurnStatus::Running => {
                if recent_turn.awaiting_approval {
                    String::from("paused")
                } else {
                    String::from("running")
                }
            }
            QueuedTurnStatus::Completed => String::from("completed"),
            QueuedTurnStatus::Failed => String::from("failed"),
            QueuedTurnStatus::Cancelled => String::from("cancelled"),
            QueuedTurnStatus::TimedOut => String::from("timed_out"),
        };
    }
    String::from("idle")
}

fn probe_transcript_messages(transcript: &[TranscriptEvent]) -> Vec<(AutopilotRole, String)> {
    let mut messages = Vec::new();
    for event in transcript {
        let mut user_lines = Vec::new();
        let mut assistant_lines = Vec::new();
        for item in &event.turn.items {
            let text = item.text.trim();
            if text.is_empty() {
                continue;
            }
            match item.kind {
                TranscriptItemKind::UserMessage => user_lines.push(text.to_string()),
                TranscriptItemKind::AssistantMessage => assistant_lines.push(text.to_string()),
                TranscriptItemKind::Note => assistant_lines.push(format!("[note] {text}")),
                TranscriptItemKind::ToolCall => {
                    let tool_name = item.name.as_deref().unwrap_or("tool");
                    assistant_lines.push(format!("[tool] {tool_name}: {text}"));
                }
                TranscriptItemKind::ToolResult => {
                    let tool_name = item.name.as_deref().unwrap_or("tool");
                    assistant_lines.push(format!("[tool-result] {tool_name}: {text}"));
                }
            }
        }
        if !user_lines.is_empty() {
            messages.push((AutopilotRole::User, user_lines.join("\n\n")));
        }
        if !assistant_lines.is_empty() {
            messages.push((AutopilotRole::Codex, assistant_lines.join("\n\n")));
        }
    }
    messages
}
