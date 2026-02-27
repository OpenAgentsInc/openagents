use crate::app_state::{PaneLoadState, RenderState};
use crate::runtime_lanes::{
    RuntimeCommandResponse, RuntimeCommandStatus, SkillTrustTier, SklLaneSnapshot,
};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: SklLaneSnapshot) {
    state.skl_lane = snapshot;
    sync_skill_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(
    state: &mut RenderState,
    response: &RuntimeCommandResponse,
    summary: &str,
) {
    state.sync_health.last_action = Some(summary.to_string());
    if response.status != RuntimeCommandStatus::Accepted {
        state.sync_health.last_error = response
            .error
            .as_ref()
            .map(|error| format!("SKL {}: {}", error.class.label(), error.message));
        let error = response.error.as_ref().map_or_else(
            || "SKL lane command rejected".to_string(),
            |err| err.message.clone(),
        );
        state.skill_registry.last_error = Some(error.clone());
        state.skill_registry.load_state = PaneLoadState::Error;
        state.skill_trust_revocation.last_error = Some(error);
        state.skill_trust_revocation.load_state = PaneLoadState::Error;
    }
}

fn sync_skill_pane_snapshots(state: &mut RenderState) {
    state.skill_registry.manifest_a = state.skl_lane.manifest_a.clone();
    state.skill_registry.manifest_event_id = state.skl_lane.manifest_event_id.clone();
    state.skill_registry.version_event_id = state.skl_lane.version_log_event_id.clone();
    state.skill_registry.search_result_event_id = state.skl_lane.search_result_event_id.clone();
    if state.skill_registry.manifest_event_id.is_some()
        || state.skill_registry.search_result_event_id.is_some()
    {
        state.skill_registry.load_state = PaneLoadState::Ready;
    }

    state.skill_trust_revocation.trust_tier = state.skl_lane.trust_tier.label().to_string();
    state.skill_trust_revocation.manifest_a = state.skl_lane.manifest_a.clone();
    state.skill_trust_revocation.kill_switch_active = state.skl_lane.kill_switch_active;
    state.skill_trust_revocation.revocation_event_id = state.skl_lane.revocation_event_id.clone();
    state.skill_trust_revocation.attestation_count =
        trust_tier_attestation_count(state.skl_lane.trust_tier);
    if state.skill_trust_revocation.manifest_a.is_some() {
        state.skill_trust_revocation.load_state = PaneLoadState::Ready;
    }
}

fn trust_tier_attestation_count(trust_tier: SkillTrustTier) -> u32 {
    match trust_tier {
        SkillTrustTier::Unknown => 0,
        SkillTrustTier::Provisional => 1,
        SkillTrustTier::Trusted => 3,
    }
}
