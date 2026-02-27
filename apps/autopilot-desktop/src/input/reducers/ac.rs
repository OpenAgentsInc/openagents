use crate::app_state::{PaneLoadState, RenderState};
use crate::runtime_lanes::{AcLaneSnapshot, RuntimeCommandResponse, RuntimeCommandStatus};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: AcLaneSnapshot) {
    state.ac_lane = snapshot;
    sync_credit_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(
    state: &mut RenderState,
    response: &RuntimeCommandResponse,
    summary: &str,
) {
    state.network_requests.apply_authority_response(response);
    state.provider_runtime.last_result = Some(summary.to_string());
    if response.status != RuntimeCommandStatus::Accepted {
        let error = response.error.as_ref().map_or_else(
            || "AC lane command rejected".to_string(),
            |err| err.message.clone(),
        );
        state.credit_desk.last_error = Some(error.clone());
        state.credit_desk.load_state = PaneLoadState::Error;
        state.credit_settlement_ledger.last_error = Some(error);
        state.credit_settlement_ledger.load_state = PaneLoadState::Error;
    }
}

fn sync_credit_pane_snapshots(state: &mut RenderState) {
    state.credit_desk.intent_event_id = state.ac_lane.intent_event_id.clone();
    state.credit_desk.offer_event_id = state.ac_lane.offer_event_id.clone();
    state.credit_desk.envelope_event_id = state.ac_lane.envelope_event_id.clone();
    state.credit_desk.spend_event_id = state.ac_lane.spend_auth_event_id.clone();
    if state.credit_desk.intent_event_id.is_some() {
        state.credit_desk.load_state = PaneLoadState::Ready;
    }

    state.credit_settlement_ledger.settlement_event_id = state.ac_lane.settlement_event_id.clone();
    state.credit_settlement_ledger.default_event_id = state.ac_lane.default_event_id.clone();
    if state.credit_settlement_ledger.settlement_event_id.is_some()
        || state.credit_settlement_ledger.default_event_id.is_some()
    {
        state.credit_settlement_ledger.load_state = PaneLoadState::Ready;
    }
}
