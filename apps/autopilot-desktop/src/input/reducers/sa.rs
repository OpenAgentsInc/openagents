use crate::app_state::{PaneLoadState, ProviderMode, RenderState};
use crate::runtime_lanes::{
    RuntimeCommandResponse, RuntimeCommandStatus, SaLaneSnapshot, SaRunnerMode,
};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: SaLaneSnapshot) {
    state.provider_runtime.mode = match snapshot.mode {
        SaRunnerMode::Offline => ProviderMode::Offline,
        SaRunnerMode::Connecting => ProviderMode::Connecting,
        SaRunnerMode::Online => ProviderMode::Online,
    };
    state.provider_runtime.mode_changed_at = snapshot.mode_changed_at;
    state.provider_runtime.connecting_until = snapshot.connect_until;
    state.provider_runtime.online_since = snapshot.online_since;
    state.provider_runtime.last_heartbeat_at = snapshot.last_heartbeat_at;
    state.provider_runtime.heartbeat_interval =
        std::time::Duration::from_secs(snapshot.heartbeat_seconds.max(1));
    state.provider_runtime.queue_depth = snapshot.queue_depth;
    state.provider_runtime.last_result = snapshot.last_result.clone();
    state.provider_runtime.degraded_reason_code = snapshot.degraded_reason_code.clone();
    state.provider_runtime.last_error_detail = snapshot.last_error_detail.clone();
    state.sa_lane = snapshot;
    sync_agent_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(
    state: &mut RenderState,
    response: &RuntimeCommandResponse,
    summary: &str,
) {
    state.provider_runtime.last_result = Some(summary.to_string());
    state.provider_runtime.last_authoritative_status = Some(response.status.label().to_string());
    state
        .provider_runtime
        .last_authoritative_event_id
        .clone_from(&response.event_id);
    state.provider_runtime.last_authoritative_error_class = response
        .error
        .as_ref()
        .map(|error| error.class.label().to_string());
    if response.status != RuntimeCommandStatus::Accepted {
        state.provider_runtime.last_error_detail = response
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .or_else(|| Some("SA lane command rejected".to_string()));
        state.provider_runtime.mode = ProviderMode::Degraded;
        state.provider_runtime.degraded_reason_code = response
            .error
            .as_ref()
            .map(|error| format!("SA_{}", error.class.label().to_ascii_uppercase()))
            .or_else(|| Some("SA_COMMAND_REJECTED".to_string()));
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
        let error = response.error.as_ref().map_or_else(
            || "SA lane command rejected".to_string(),
            |err| err.message.clone(),
        );
        state.agent_profile_state.last_error = Some(error.clone());
        state.agent_profile_state.load_state = PaneLoadState::Error;
        state.agent_schedule_tick.last_error = Some(error.clone());
        state.agent_schedule_tick.load_state = PaneLoadState::Error;
        state.trajectory_audit.last_error = Some(error);
        state.trajectory_audit.load_state = PaneLoadState::Error;
    }
}

fn sync_agent_pane_snapshots(state: &mut RenderState) {
    state.agent_profile_state.profile_event_id = state.sa_lane.profile_event_id.clone();
    state.agent_profile_state.state_event_id = state.sa_lane.state_event_id.clone();
    state.agent_profile_state.goals_event_id = state.sa_lane.state_event_id.clone();
    if state.agent_profile_state.profile_event_id.is_some()
        || state.agent_profile_state.state_event_id.is_some()
    {
        state.agent_profile_state.load_state = PaneLoadState::Ready;
    }

    state.agent_schedule_tick.heartbeat_seconds = state.sa_lane.heartbeat_seconds;
    state.agent_schedule_tick.schedule_event_id = state.sa_lane.schedule_event_id.clone();
    state.agent_schedule_tick.tick_request_event_id =
        state.sa_lane.last_tick_request_event_id.clone();
    state.agent_schedule_tick.tick_result_event_id =
        state.sa_lane.last_tick_result_event_id.clone();
    if let Some(outcome) = state.sa_lane.last_result.as_deref() {
        state.agent_schedule_tick.last_tick_outcome = outcome.to_string();
    }
    if state.agent_schedule_tick.schedule_event_id.is_some() {
        state.agent_schedule_tick.load_state = PaneLoadState::Ready;
    }

    state.trajectory_audit.active_session_id = state
        .sa_lane
        .last_tick_request_event_id
        .as_deref()
        .map(|event| format!("traj:{event}"));
    if state.trajectory_audit.active_session_id.is_some() {
        state.trajectory_audit.load_state = PaneLoadState::Ready;
    }
}
