use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, EarnFailureClass, PaneLoadState, ProviderMode,
    RelayConnectionRow, RelayConnectionStatus, RenderState,
};
use crate::provider_nip90_lane::{
    ProviderNip90LaneMode, ProviderNip90LaneSnapshot, ProviderNip90PublishOutcome,
    ProviderNip90PublishRole, ProviderNip90RelayStatus,
};
use crate::state::job_inbox::JobInboxNetworkRequest;

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: ProviderNip90LaneSnapshot) {
    let previous_mode = state.provider_nip90_lane.mode;
    let selected_url = state.relay_connections.selected_url.clone();
    state.provider_nip90_lane = snapshot;
    state.relay_connections.relays = state
        .provider_nip90_lane
        .relay_health
        .iter()
        .map(|relay| RelayConnectionRow {
            url: relay.relay_url.clone(),
            status: map_relay_status(relay.status),
            latency_ms: relay.latency_ms,
            last_seen_seconds_ago: relay.last_seen_seconds_ago,
            last_error: relay.last_error.clone(),
        })
        .collect();
    state.relay_connections.load_state = PaneLoadState::Ready;
    state.relay_connections.last_error = state.provider_nip90_lane.last_error.clone();
    state.relay_connections.last_action = state.provider_nip90_lane.last_action.clone();
    state.relay_connections.selected_url = selected_url
        .filter(|selected| {
            state
                .relay_connections
                .relays
                .iter()
                .any(|relay| relay.url == *selected)
        })
        .or_else(|| {
            state
                .relay_connections
                .relays
                .first()
                .map(|relay| relay.url.clone())
        });

    if let Some(last_error) = state.provider_nip90_lane.last_error.as_deref() {
        state.provider_runtime.mode = ProviderMode::Degraded;
        state.provider_runtime.degraded_reason_code = Some("NIP90_RELAY_INGRESS_ERROR".to_string());
        state.provider_runtime.last_error_detail = Some(last_error.to_string());
        state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Relay);
        state.provider_runtime.last_result = state.provider_nip90_lane.last_action.clone();
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
    } else if state.provider_nip90_lane.mode == ProviderNip90LaneMode::Online
        && previous_mode != state.provider_nip90_lane.mode
        && state.provider_runtime.mode != ProviderMode::Offline
    {
        state.provider_runtime.mode = ProviderMode::Online;
        state.provider_runtime.degraded_reason_code = None;
        state.provider_runtime.last_error_detail = None;
        if state.provider_runtime.last_authoritative_error_class == Some(EarnFailureClass::Relay) {
            state.provider_runtime.last_authoritative_error_class = None;
        }
        state.provider_runtime.last_result = state.provider_nip90_lane.last_action.clone();
        state.provider_runtime.mode_changed_at = std::time::Instant::now();
    }
}

fn map_relay_status(status: ProviderNip90RelayStatus) -> RelayConnectionStatus {
    match status {
        ProviderNip90RelayStatus::Connected => RelayConnectionStatus::Connected,
        ProviderNip90RelayStatus::Connecting => RelayConnectionStatus::Connecting,
        ProviderNip90RelayStatus::Disconnected => RelayConnectionStatus::Disconnected,
        ProviderNip90RelayStatus::Error => RelayConnectionStatus::Error,
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
    if state.provider_runtime.last_authoritative_error_class == Some(EarnFailureClass::Relay) {
        state.provider_runtime.last_authoritative_error_class = None;
    }

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

pub(super) fn apply_publish_outcome(state: &mut RenderState, outcome: ProviderNip90PublishOutcome) {
    let publish_succeeded = outcome.accepted_relays > 0;

    if !publish_succeeded {
        let error = outcome
            .first_error
            .clone()
            .unwrap_or_else(|| "All relays rejected publish".to_string());
        state.provider_runtime.mode = ProviderMode::Degraded;
        state.provider_runtime.degraded_reason_code = Some("NIP90_PUBLISH_ERROR".to_string());
        state.provider_runtime.last_error_detail = Some(error);
        state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Relay);
    } else if state.provider_runtime.mode != ProviderMode::Offline {
        state.provider_runtime.mode = ProviderMode::Online;
        state.provider_runtime.degraded_reason_code = None;
        state.provider_runtime.last_error_detail = None;
        if state.provider_runtime.last_authoritative_error_class == Some(EarnFailureClass::Relay) {
            state.provider_runtime.last_authoritative_error_class = None;
        }
    }

    let publish_label = if publish_succeeded {
        "published"
    } else {
        "failed to publish"
    };
    state.provider_runtime.last_result = Some(format!(
        "{} {} event {} (accepted={}, rejected={})",
        publish_label,
        outcome.role.label(),
        outcome.event_id,
        outcome.accepted_relays,
        outcome.rejected_relays
    ));

    if let Some(job) = state.active_job.job.as_mut()
        && job.request_id == outcome.request_id
    {
        if publish_succeeded {
            match outcome.role {
                ProviderNip90PublishRole::Result => {
                    if job.sa_tick_result_event_id.is_none() {
                        job.sa_tick_result_event_id = Some(outcome.event_id.clone());
                    }
                }
                ProviderNip90PublishRole::Feedback => {
                    // Reuse settlement/default fields until a dedicated NIP-90 feedback receipt field lands.
                    if job.stage == crate::app_state::JobLifecycleStage::Failed {
                        if job.ac_default_event_id.is_none() {
                            job.ac_default_event_id = Some(outcome.event_id.clone());
                        }
                    } else if job.ac_settlement_event_id.is_none() {
                        job.ac_settlement_event_id = Some(outcome.event_id.clone());
                    }
                }
            }
        }
        state.active_job.append_event(format!(
            "{} {} event {} (accepted={}, rejected={})",
            publish_label,
            outcome.role.label(),
            outcome.event_id,
            outcome.accepted_relays,
            outcome.rejected_relays
        ));
    }

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    state.activity_feed.upsert_event(ActivityEventRow {
        event_id: format!("nip90:{}:{}", outcome.role.label(), outcome.event_id),
        domain: ActivityEventDomain::Network,
        source_tag: "nip90.publish".to_string(),
        summary: if publish_succeeded {
            format!("Published NIP-90 {} event", outcome.role.label())
        } else {
            format!("Failed NIP-90 {} publish", outcome.role.label())
        },
        detail: format!(
            "request={} event_id={} accepted_relays={} rejected_relays={} error={}",
            outcome.request_id,
            outcome.event_id,
            outcome.accepted_relays,
            outcome.rejected_relays,
            outcome.first_error.as_deref().unwrap_or("none")
        ),
        occurred_at_epoch_seconds: now_epoch_seconds,
    });
    state.activity_feed.load_state = PaneLoadState::Ready;
}
