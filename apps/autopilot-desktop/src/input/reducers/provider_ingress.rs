use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, PaneLoadState, ProviderMode, RenderState,
};
use crate::provider_nip90_lane::{ProviderNip90LaneMode, ProviderNip90LaneSnapshot};
use crate::state::job_inbox::JobInboxNetworkRequest;

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: ProviderNip90LaneSnapshot) {
    let previous_mode = state.provider_nip90_lane.mode;
    state.provider_nip90_lane = snapshot;

    if let Some(last_error) = state.provider_nip90_lane.last_error.as_deref() {
        state.provider_runtime.mode = ProviderMode::Degraded;
        state.provider_runtime.degraded_reason_code = Some("NIP90_RELAY_INGRESS_ERROR".to_string());
        state.provider_runtime.last_error_detail = Some(last_error.to_string());
        state.provider_runtime.last_result = state.provider_nip90_lane.last_action.clone();
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
    } else if state.provider_nip90_lane.mode == ProviderNip90LaneMode::Online
        && previous_mode != state.provider_nip90_lane.mode
        && state.provider_runtime.mode != ProviderMode::Offline
    {
        state.provider_runtime.mode = ProviderMode::Online;
        state.provider_runtime.degraded_reason_code = None;
        state.provider_runtime.last_error_detail = None;
        state.provider_runtime.last_result = state.provider_nip90_lane.last_action.clone();
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
    }
}

pub(super) fn apply_ingressed_request(state: &mut RenderState, request: JobInboxNetworkRequest) {
    let is_new = !state
        .job_inbox
        .requests
        .iter()
        .any(|existing| existing.request_id == request.request_id);

    state.job_inbox.upsert_network_request(request.clone());
    state.job_inbox.load_state = PaneLoadState::Ready;
    state.job_inbox.last_error = None;
    state.job_inbox.last_action = Some(format!(
        "Ingested live NIP-90 request {} from relay lane",
        request.request_id
    ));

    state.provider_runtime.last_result = Some(format!(
        "relay ingress received request {} ({})",
        request.request_id, request.capability
    ));
    state.provider_runtime.last_authoritative_status = Some("accepted".to_string());
    state.provider_runtime.last_authoritative_event_id = Some(request.request_id.clone());
    state.provider_runtime.last_authoritative_error_class = None;

    if is_new {
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        state.activity_feed.upsert_event(ActivityEventRow {
            event_id: format!("nip90:req:{}", request.request_id),
            domain: ActivityEventDomain::Network,
            source_tag: "nip90.relay".to_string(),
            summary: format!("Live request {} arrived", request.capability),
            detail: format!(
                "request={} requester={} price_sats={} ttl_seconds={}",
                request.request_id, request.requester, request.price_sats, request.ttl_seconds
            ),
            occurred_at_epoch_seconds: now_epoch_seconds,
        });
        state.activity_feed.load_state = PaneLoadState::Ready;
    }

    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}
