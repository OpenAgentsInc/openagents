use crate::app_state::{PaneLoadState, RenderState};
use crate::apple_fm_bridge::{
    AppleFmBridgeUpdate, AppleFmMissionControlSummaryUpdate, AppleFmWorkbenchLogLevel,
    AppleFmWorkbenchOperation, AppleFmWorkbenchUpdate,
};
use wgpui::components::sections::{TerminalLine, TerminalStream};

const PREVIEW_LIMIT: usize = 1600;
const MISSION_CONTROL_SUMMARY_LINE_KEY_PREFIX: &str = "mission-control.local-fm-summary.";

pub(super) fn apply_bridge_update(state: &mut RenderState, update: &AppleFmBridgeUpdate) -> bool {
    match update {
        AppleFmBridgeUpdate::Snapshot(snapshot) => {
            if state.apple_fm_workbench.pending_request_id.is_some() {
                return false;
            }
            state.apple_fm_workbench.last_model = snapshot
                .ready_model
                .clone()
                .or_else(|| state.apple_fm_workbench.last_model.clone());
            state.apple_fm_workbench.last_action = snapshot
                .last_action
                .clone()
                .or_else(|| state.apple_fm_workbench.last_action.clone());
            state.apple_fm_workbench.last_error = snapshot.last_error.clone();
            state.apple_fm_workbench.load_state = if snapshot.last_error.is_some() {
                PaneLoadState::Error
            } else if snapshot.reachable {
                PaneLoadState::Ready
            } else {
                PaneLoadState::Loading
            };
            true
        }
        AppleFmBridgeUpdate::Workbench(update) => {
            apply_workbench_update(state, update.as_ref());
            true
        }
        AppleFmBridgeUpdate::MissionControlSummary(update) => {
            apply_mission_control_summary_update(state, update.as_ref());
            true
        }
        AppleFmBridgeUpdate::Started(_)
        | AppleFmBridgeUpdate::Completed(_)
        | AppleFmBridgeUpdate::Failed(_) => false,
    }
}

fn apply_workbench_update(state: &mut RenderState, update: &AppleFmWorkbenchUpdate) {
    match update {
        AppleFmWorkbenchUpdate::Started(started) => {
            state.apple_fm_workbench.load_state = PaneLoadState::Loading;
            state.apple_fm_workbench.last_error = None;
            state.apple_fm_workbench.pending_request_id = Some(started.request_id.clone());
            state.apple_fm_workbench.last_request_id = Some(started.request_id.clone());
            state.apple_fm_workbench.last_operation = Some(started.operation.clone());
            state.apple_fm_workbench.last_action =
                Some(format!("Running Apple FM {}", started.operation));
            state.apple_fm_workbench.event_log.clear();
            state
                .apple_fm_workbench
                .event_log
                .push_line(TerminalLine::new(
                    TerminalStream::Stdout,
                    format!("started {}", started.operation),
                ));
        }
        AppleFmWorkbenchUpdate::Event(event) => {
            state
                .apple_fm_workbench
                .event_log
                .push_line(TerminalLine::new(
                    terminal_stream(event.level),
                    event.line.clone(),
                ));
        }
        AppleFmWorkbenchUpdate::Completed(completed) => {
            state.apple_fm_workbench.load_state = PaneLoadState::Ready;
            state.apple_fm_workbench.last_error = None;
            state.apple_fm_workbench.pending_request_id = None;
            state.apple_fm_workbench.last_request_id = Some(completed.request_id.clone());
            state.apple_fm_workbench.last_operation = Some(completed.operation.clone());
            state.apple_fm_workbench.last_action = Some(completed.summary.clone());
            state.apple_fm_workbench.last_model = completed.model.clone();
            state.apple_fm_workbench.active_session_id = completed.session_id.clone();
            state.apple_fm_workbench.output_chars = completed.response_text.chars().count();
            state.apple_fm_workbench.output_preview =
                truncate_preview(completed.response_text.as_str());
            state.apple_fm_workbench.session_preview = completed
                .session_json
                .as_deref()
                .map(truncate_preview)
                .unwrap_or_default();
            state.apple_fm_workbench.structured_preview = completed
                .structured_json
                .as_deref()
                .map(truncate_preview)
                .unwrap_or_default();
            state.apple_fm_workbench.usage_preview = completed
                .usage_json
                .as_deref()
                .map(truncate_preview)
                .unwrap_or_default();

            match (
                completed.operation.as_str(),
                completed.session_id.as_ref(),
                completed.transcript_json.as_ref(),
            ) {
                (label, None, _) if label == AppleFmWorkbenchOperation::DeleteSession.label() => {
                    state
                        .apple_fm_workbench_inputs
                        .session_id
                        .set_value(String::new());
                    state
                        .apple_fm_workbench_inputs
                        .transcript_json
                        .set_value(String::new());
                }
                (_, Some(session_id), _) => {
                    state
                        .apple_fm_workbench_inputs
                        .session_id
                        .set_value(session_id.clone());
                }
                _ => {}
            }
            if let Some(transcript_json) = completed.transcript_json.as_ref() {
                state
                    .apple_fm_workbench_inputs
                    .transcript_json
                    .set_value(transcript_json.clone());
            }

            state
                .apple_fm_workbench
                .event_log
                .push_line(TerminalLine::new(
                    TerminalStream::Stdout,
                    completed.summary.clone(),
                ));
        }
        AppleFmWorkbenchUpdate::Failed(failed) => {
            state.apple_fm_workbench.load_state = PaneLoadState::Error;
            state.apple_fm_workbench.pending_request_id = None;
            state.apple_fm_workbench.last_request_id = Some(failed.request_id.clone());
            state.apple_fm_workbench.last_operation = Some(failed.operation.clone());
            state.apple_fm_workbench.last_action =
                Some(format!("Apple FM {} failed", failed.operation));
            state.apple_fm_workbench.last_error = Some(failed.error.clone());
            state
                .apple_fm_workbench
                .event_log
                .push_line(TerminalLine::new(
                    TerminalStream::Stderr,
                    failed.error.clone(),
                ));
        }
    }
}

fn apply_mission_control_summary_update(
    state: &mut RenderState,
    update: &AppleFmMissionControlSummaryUpdate,
) {
    match update {
        AppleFmMissionControlSummaryUpdate::Started(started) => {
            state.provider_control.local_fm_summary_pending_request_id =
                Some(started.request_id.clone());
            state.provider_control.local_fm_summary_text.clear();
            state.log_stream.upsert_runtime_log_line(
                mission_control_summary_line_key(started.request_id.as_str()),
                TerminalStream::Stdout,
                "Local FM summary streaming...",
            );
            state.provider_control.record_action(format!(
                "Local FM summary test running [{}]",
                started.request_id
            ));
        }
        AppleFmMissionControlSummaryUpdate::Delta(delta) => {
            if state
                .provider_control
                .local_fm_summary_pending_request_id
                .as_deref()
                != Some(delta.request_id.as_str())
            {
                return;
            }
            state
                .provider_control
                .local_fm_summary_text
                .push_str(&delta.delta);
            let text = if state
                .provider_control
                .local_fm_summary_text
                .trim()
                .is_empty()
            {
                "Local FM summary streaming...".to_string()
            } else {
                format!(
                    "Local FM summary > {}",
                    state.provider_control.local_fm_summary_text
                )
            };
            state.log_stream.upsert_runtime_log_line(
                mission_control_summary_line_key(delta.request_id.as_str()),
                TerminalStream::Stdout,
                text,
            );
        }
        AppleFmMissionControlSummaryUpdate::Completed(completed) => {
            state.provider_control.local_fm_summary_pending_request_id = None;
            state.provider_control.local_fm_summary_text = completed.response_text.clone();
            state.log_stream.upsert_runtime_log_line(
                mission_control_summary_line_key(completed.request_id.as_str()),
                TerminalStream::Stdout,
                format!("Local FM summary > {}", completed.response_text.trim()),
            );
            if let Some(model) = completed.model.as_deref() {
                state.log_stream.push_runtime_log_line(
                    TerminalStream::Stdout,
                    format!("Local FM summary completed via {model}"),
                );
            } else {
                state
                    .log_stream
                    .push_runtime_log_line(TerminalStream::Stdout, completed.summary.clone());
            }
            state
                .provider_control
                .record_action(completed.summary.clone());
            state.provider_runtime.last_result = Some(completed.summary.clone());
        }
        AppleFmMissionControlSummaryUpdate::Failed(failed) => {
            state.provider_control.local_fm_summary_pending_request_id = None;
            state.provider_control.local_fm_summary_text.clear();
            state.log_stream.upsert_runtime_log_line(
                mission_control_summary_line_key(failed.request_id.as_str()),
                TerminalStream::Stderr,
                format!("Local FM summary failed // {}", failed.error),
            );
            state.provider_control.record_error(failed.error.clone());
        }
    }
}

fn terminal_stream(level: AppleFmWorkbenchLogLevel) -> TerminalStream {
    match level {
        AppleFmWorkbenchLogLevel::Info => TerminalStream::Stdout,
        AppleFmWorkbenchLogLevel::Error => TerminalStream::Stderr,
    }
}

fn truncate_preview(value: &str) -> String {
    if value.chars().count() <= PREVIEW_LIMIT {
        value.to_string()
    } else {
        let prefix = value.chars().take(PREVIEW_LIMIT).collect::<String>();
        format!("{prefix}\n\n[truncated]")
    }
}

fn mission_control_summary_line_key(request_id: &str) -> String {
    format!("{MISSION_CONTROL_SUMMARY_LINE_KEY_PREFIX}{request_id}")
}
