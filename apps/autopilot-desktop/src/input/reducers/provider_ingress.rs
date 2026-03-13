use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, EarnFailureClass, PaneLoadState, ProviderMode,
    RelayConnectionRow, RelayConnectionStatus, RenderState,
};
use crate::nip90_compute_domain_events;
use crate::provider_nip90_lane::{
    ProviderNip90BuyerResponseEvent, ProviderNip90BuyerResponseKind, ProviderNip90LaneCommand,
    ProviderNip90LaneMode, ProviderNip90LaneSnapshot, ProviderNip90PublishOutcome,
    ProviderNip90PublishRole, ProviderNip90RelayStatus,
};
use crate::spark_wallet::SparkWalletCommand;
use crate::state::job_inbox::{
    JobInboxNetworkRequest, JobInboxValidation, local_provider_keys, normalize_provider_keys,
};
use crate::state::operations::{BuyerResolutionAction, BuyerResolutionReason};
use crate::state::provider_runtime::LocalInferenceBackend;
use nostr::nip90::{JobFeedback, JobStatus, create_job_feedback_event};
use nostr::{Event, EventTemplate, NostrIdentity};
use openagents_provider_substrate::{
    ProviderIngressMode, ProviderLifecycleInput, ProviderLifecycleTransition,
    derive_provider_lifecycle,
};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: ProviderNip90LaneSnapshot) {
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
    state.relay_connections.last_error =
        relay_connections_surface_error(state.provider_nip90_lane.last_error.as_deref());
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
    sync_provider_runtime_mode_from_provider_state(state);
}

pub(super) fn sync_provider_runtime_mode_from_provider_state(state: &mut RenderState) {
    let now = std::time::Instant::now();
    let availability = state.provider_runtime.availability();
    let backend_unavailable_detail = state
        .provider_runtime
        .apple_fm
        .availability_error_message()
        .or_else(|| state.provider_runtime.gpt_oss.last_error.clone())
        .or_else(|| state.gpt_oss_execution.last_error.clone());
    let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
        current_mode: state.provider_runtime.mode,
        ingress_mode: map_ingress_mode(state.provider_nip90_lane.mode),
        relay_error: state.provider_nip90_lane.last_error.as_deref(),
        availability: &availability,
        backend_unavailable_detail: backend_unavailable_detail.as_deref(),
    });

    match transition {
        ProviderLifecycleTransition::StayOffline => {
            state.provider_runtime.mode = ProviderMode::Offline;
            state.provider_runtime.degraded_reason_code = None;
            state.provider_runtime.last_error_detail = None;
            if matches!(
                state.provider_nip90_lane.mode,
                ProviderNip90LaneMode::Preview | ProviderNip90LaneMode::Connecting
            ) {
                state.provider_runtime.last_result = state.provider_nip90_lane.last_action.clone();
            }
        }
        ProviderLifecycleTransition::HoldCurrent => {
            if state
                .provider_runtime
                .last_error_detail
                .as_deref()
                .is_some_and(|detail| {
                    detail
                        .trim()
                        .eq_ignore_ascii_case("Foundation Models is available")
                })
            {
                state.provider_runtime.last_error_detail = None;
                if state.provider_runtime.last_authoritative_error_class
                    == Some(EarnFailureClass::Execution)
                {
                    state.provider_runtime.last_authoritative_error_class = None;
                }
            }

            if matches!(state.provider_runtime.mode, ProviderMode::Degraded)
                && matches!(
                    state.provider_nip90_lane.mode,
                    ProviderNip90LaneMode::Offline
                        | ProviderNip90LaneMode::Preview
                        | ProviderNip90LaneMode::Connecting
                )
                && availability.active_inference_backend().is_some()
                && state.provider_runtime.last_authoritative_error_class
                    == Some(EarnFailureClass::Execution)
            {
                state.provider_runtime.mode = ProviderMode::Offline;
                state.provider_runtime.degraded_reason_code = None;
                state.provider_runtime.last_error_detail = None;
                state.provider_runtime.last_authoritative_error_class = None;
                state.provider_runtime.last_result = state
                    .provider_runtime
                    .apple_fm
                    .last_action
                    .clone()
                    .or_else(|| state.provider_runtime.gpt_oss.last_action.clone());
                state.provider_runtime.mode_changed_at = now;
            }
        }
        ProviderLifecycleTransition::Degraded {
            reason_code,
            error_detail,
            failure_class,
        } => {
            state.provider_runtime.mode = ProviderMode::Degraded;
            state.provider_runtime.degraded_reason_code = Some(reason_code.to_string());
            state.provider_runtime.last_error_detail = Some(error_detail);
            state.provider_runtime.last_authoritative_error_class = Some(failure_class);
            state.provider_runtime.last_result =
                state.provider_nip90_lane.last_action.clone().or_else(|| {
                    state
                        .provider_runtime
                        .apple_fm
                        .last_action
                        .clone()
                        .or_else(|| state.provider_runtime.gpt_oss.last_action.clone())
                });
            state.provider_runtime.mode_changed_at = now;
        }
        ProviderLifecycleTransition::Online { active_backend } => {
            state.provider_runtime.mode = ProviderMode::Online;
            state.provider_runtime.degraded_reason_code = None;
            state.provider_runtime.last_error_detail = None;
            if state.provider_runtime.last_authoritative_error_class
                == Some(EarnFailureClass::Relay)
                || state.provider_runtime.last_authoritative_error_class
                    == Some(EarnFailureClass::Execution)
            {
                state.provider_runtime.last_authoritative_error_class = None;
            }
            state.provider_runtime.last_result = state
                .provider_nip90_lane
                .last_action
                .clone()
                .or_else(|| match active_backend {
                    LocalInferenceBackend::AppleFoundationModels => {
                        state.provider_runtime.apple_fm.last_action.clone()
                    }
                    LocalInferenceBackend::GptOss => {
                        state.provider_runtime.gpt_oss.last_action.clone()
                    }
                });
            state.provider_runtime.mode_changed_at = now;
        }
    }
}

fn map_ingress_mode(mode: ProviderNip90LaneMode) -> ProviderIngressMode {
    match mode {
        ProviderNip90LaneMode::Offline => ProviderIngressMode::Offline,
        ProviderNip90LaneMode::Preview => ProviderIngressMode::Preview,
        ProviderNip90LaneMode::Connecting => ProviderIngressMode::Connecting,
        ProviderNip90LaneMode::Online => ProviderIngressMode::Online,
        ProviderNip90LaneMode::Degraded => ProviderIngressMode::Degraded,
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

fn relay_connections_surface_error(error: Option<&str>) -> Option<String> {
    let error = error?.trim();
    if error.is_empty() || error.to_ascii_lowercase().contains("publish") {
        None
    } else {
        Some(error.to_string())
    }
}

pub(super) fn apply_ingressed_request(
    state: &mut RenderState,
    mut request: JobInboxNetworkRequest,
) {
    if state.provider_runtime.active_inference_backend().is_none() {
        return;
    }

    let preview_only = ingress_is_preview_only(state.provider_nip90_lane.mode);
    apply_encrypted_request_handling(state, &mut request);
    if let Some(reason) = target_policy_reject_reason(state, &request) {
        apply_ignored_ingress_request(state, &request, reason.as_str());
        return;
    }

    let is_new = !state
        .job_inbox
        .requests
        .iter()
        .any(|existing| existing.request_id == request.request_id);

    if preview_only && !is_new {
        return;
    }

    state.job_inbox.upsert_network_request(request.clone());
    state.job_inbox.load_state = PaneLoadState::Ready;
    state.job_inbox.last_error = None;
    state.job_inbox.last_action = Some(if preview_only {
        "Observed preview NIP-90 request from relay lane".to_string()
    } else {
        format!(
            "Ingested live NIP-90 request {} from relay lane",
            request.request_id
        )
    });

    if is_new {
        if preview_only {
            tracing::debug!(
                target: "autopilot_desktop::provider",
                "Provider preview observed request_id={} capability={} price_sats={} ttl_seconds={}",
                request.request_id,
                request.capability,
                request.price_sats,
                request.ttl_seconds
            );
        } else {
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Provider ingress request_id={} capability={} price_sats={} ttl_seconds={} preview_only=false",
                request.request_id,
                request.capability,
                request.price_sats,
                request.ttl_seconds
            );
        }
    }

    if preview_only {
        state.provider_runtime.last_result =
            Some("relay preview observing market activity".to_string());
    } else {
        state.provider_runtime.last_result = Some(format!(
            "relay ingress received request {} ({})",
            request.request_id, request.capability
        ));
        state.provider_runtime.last_authoritative_status = Some("accepted".to_string());
        state.provider_runtime.last_authoritative_event_id = Some(request.request_id.clone());
        if state.provider_runtime.last_authoritative_error_class == Some(EarnFailureClass::Relay) {
            state.provider_runtime.last_authoritative_error_class = None;
        }
    }

    if is_new {
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        if !preview_only {
            let parsed_shape = request
                .parsed_event_shape
                .as_deref()
                .unwrap_or("shape unavailable");
            let raw_json = request
                .raw_event_json
                .as_deref()
                .unwrap_or("raw event json unavailable");
            state.earn_job_lifecycle_projection.record_ingress_request(
                &request,
                now_epoch_seconds,
                "nip90.relay.ingress",
            );
            state.earn_kernel_receipts.record_ingress_request(
                &request,
                now_epoch_seconds,
                "nip90.relay.ingress",
            );
            state.activity_feed.upsert_event(ActivityEventRow {
                event_id: format!("nip90:req:{}", request.request_id),
                domain: ActivityEventDomain::Network,
                source_tag: "nip90.relay".to_string(),
                summary: format!("Live request {} arrived", request.capability),
                detail: format!(
                    "request={} requester={} price_sats={} ttl_seconds={}\n\nshape:\n{}\n\nraw_event_json:\n{}",
                    request.request_id,
                    request.requester,
                    request.price_sats,
                    request.ttl_seconds,
                    parsed_shape,
                    raw_json,
                ),
                occurred_at_epoch_seconds: now_epoch_seconds,
            });
            state.activity_feed.load_state = PaneLoadState::Ready;
        }
    }

    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn apply_encrypted_request_handling(state: &RenderState, request: &mut JobInboxNetworkRequest) {
    if !request.encrypted {
        return;
    }

    let Some(payload) = request
        .encrypted_payload
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        request.validation = JobInboxValidation::Invalid(
            "encrypted request payload is empty; expected NIP-44 ciphertext in content".to_string(),
        );
        append_parsed_shape_line(
            &mut request.parsed_event_shape,
            "request.encrypted_payload_status=invalid_empty_payload".to_string(),
        );
        return;
    };

    let Some(identity) = state.nostr_identity.as_ref() else {
        request.validation = JobInboxValidation::Invalid(
            "encrypted request cannot be decrypted because local Nostr identity is unavailable"
                .to_string(),
        );
        append_parsed_shape_line(
            &mut request.parsed_event_shape,
            "request.encrypted_payload_status=missing_local_identity".to_string(),
        );
        return;
    };

    match decrypt_encrypted_request_payload(
        identity.private_key_hex.as_str(),
        request.requester.as_str(),
        payload,
    ) {
        Ok(plaintext) => {
            let preview = sanitize_payload_preview(plaintext.as_str(), 220);
            request.execution_input = Some(format!("Encrypted request content:\n{plaintext}"));
            append_parsed_shape_line(
                &mut request.parsed_event_shape,
                format!(
                    "request.encrypted_payload_status=decrypted plaintext_bytes={} plaintext.preview={}",
                    plaintext.len(),
                    preview
                ),
            );
        }
        Err(error) => {
            let reason = format!(
                "encrypted request decryption failed; verify identity key and payload: {error}"
            );
            request.validation = JobInboxValidation::Invalid(reason.clone());
            append_parsed_shape_line(
                &mut request.parsed_event_shape,
                format!("request.encrypted_payload_status=decrypt_failed error={error}"),
            );
        }
    }
}

fn target_policy_reject_reason(
    state: &RenderState,
    request: &JobInboxNetworkRequest,
) -> Option<String> {
    target_policy_reject_reason_for(
        request.target_provider_pubkeys.as_slice(),
        state.nostr_identity.as_ref(),
    )
}

fn target_policy_reject_reason_for(
    target_provider_pubkeys: &[String],
    identity: Option<&nostr::NostrIdentity>,
) -> Option<String> {
    if target_provider_pubkeys.is_empty() {
        return None;
    }

    let targets = normalize_provider_keys(target_provider_pubkeys);
    if targets.is_empty() {
        return None;
    }

    let Some(identity) = identity else {
        return Some(
            "request contains target provider `p` tags but local Nostr identity is unavailable"
                .to_string(),
        );
    };

    let local_keys = local_provider_keys(identity);
    let target_match = targets
        .iter()
        .any(|target| local_keys.iter().any(|local| local == target));
    if target_match {
        return None;
    }

    Some(format!(
        "request target policy mismatch (targets=[{}], local=[{}])",
        targets.join(","),
        local_keys.join(",")
    ))
}

fn apply_ignored_ingress_request(
    state: &mut RenderState,
    request: &JobInboxNetworkRequest,
    reason: &str,
) {
    let preview_only = ingress_is_preview_only(state.provider_nip90_lane.mode);
    if preview_only {
        return;
    }
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    state
        .earn_kernel_receipts
        .record_network_preflight_rejection(
            request,
            reason,
            now_epoch_seconds,
            "nip90.relay.ingress.reject",
        );

    state.job_inbox.load_state = PaneLoadState::Ready;
    state.job_inbox.last_error = None;
    state.job_inbox.last_action = Some(format!(
        "Ignored live NIP-90 request {} ({})",
        request.request_id, reason
    ));

    state.provider_runtime.last_result = Some(format!(
        "relay ingress ignored request {} ({})",
        request.request_id, reason
    ));
    state.provider_runtime.last_authoritative_status = Some("ignored".to_string());
    state.provider_runtime.last_authoritative_event_id = Some(request.request_id.clone());
    if state.provider_runtime.last_authoritative_error_class == Some(EarnFailureClass::Relay) {
        state.provider_runtime.last_authoritative_error_class = None;
    }

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    state.activity_feed.upsert_event(ActivityEventRow {
        event_id: format!("nip90:req:ignored:{}", request.request_id),
        domain: ActivityEventDomain::Network,
        source_tag: "nip90.policy".to_string(),
        summary: "Ignored live NIP-90 request".to_string(),
        detail: format!(
            "request={} requester={} targets={} encrypted={} reason={}\n\nshape:\n{}\n\nraw_event_json:\n{}",
            request.request_id,
            request.requester,
            if request.target_provider_pubkeys.is_empty() {
                "none".to_string()
            } else {
                request.target_provider_pubkeys.join(",")
            },
            request.encrypted,
            reason,
            request
                .parsed_event_shape
                .as_deref()
                .unwrap_or("shape unavailable"),
            request
                .raw_event_json
                .as_deref()
                .unwrap_or("raw event json unavailable"),
        ),
        occurred_at_epoch_seconds: now_epoch_seconds,
    });
    state.activity_feed.load_state = PaneLoadState::Ready;

    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn ingress_is_preview_only(mode: ProviderNip90LaneMode) -> bool {
    !matches!(
        mode,
        ProviderNip90LaneMode::Online | ProviderNip90LaneMode::Degraded
    )
}

fn append_parsed_shape_line(shape: &mut Option<String>, line: String) {
    if let Some(existing) = shape.as_mut() {
        if !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(line.as_str());
    } else {
        *shape = Some(line);
    }
}

fn sanitize_payload_preview(raw: &str, limit: usize) -> String {
    let mut normalized = raw
        .chars()
        .filter_map(|ch| match ch {
            '\n' | '\r' | '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect::<String>()
        .trim()
        .to_string();
    if normalized.len() > limit {
        normalized.truncate(limit);
        normalized.push_str("...");
    }
    normalized
}

fn decrypt_encrypted_request_payload(
    recipient_private_key_hex: &str,
    sender_pubkey_hex: &str,
    payload: &str,
) -> Result<String, String> {
    let recipient_private_key = parse_private_key_hex(recipient_private_key_hex)?;
    let sender_pubkey_bytes = decode_xonly_pubkey_hex(sender_pubkey_hex)?;
    let mut last_error = String::new();

    for prefix in [0x02u8, 0x03u8] {
        let mut compressed_pubkey = vec![prefix];
        compressed_pubkey.extend_from_slice(sender_pubkey_bytes.as_slice());
        match nostr::nip44::decrypt(
            &recipient_private_key,
            compressed_pubkey.as_slice(),
            payload,
        ) {
            Ok(plaintext) => return Ok(plaintext),
            Err(error) => last_error = error.to_string(),
        }
    }

    Err(format!("NIP-44 decrypt failed: {last_error}"))
}

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid local private key hex: {error}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "invalid local private key length {}, expected 32 bytes",
            bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(bytes.as_slice());
    Ok(key)
}

fn decode_xonly_pubkey_hex(sender_pubkey_hex: &str) -> Result<Vec<u8>, String> {
    let bytes = hex::decode(sender_pubkey_hex.trim())
        .map_err(|error| format!("invalid requester pubkey hex: {error}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "invalid requester pubkey length {}, expected 32-byte x-only key",
            bytes.len()
        ));
    }
    Ok(bytes)
}

pub(super) fn apply_buyer_response_event(
    state: &mut RenderState,
    event: ProviderNip90BuyerResponseEvent,
) {
    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    tracing::info!(
        target: "autopilot_desktop::buyer",
        "Buyer response request_id={} kind={} event_id={} provider_nostr={} status={} status_extra={} amount_msats={} bolt11_present={}",
        event.request_id,
        event.kind.label(),
        event.event_id,
        event.provider_pubkey,
        event.status.as_deref().unwrap_or("none"),
        event.status_extra.as_deref().unwrap_or("none"),
        event
            .amount_msats
            .map(|amount| amount.to_string())
            .unwrap_or_else(|| "none".to_string()),
        event.bolt11.as_ref().is_some_and(|bolt11| !bolt11.trim().is_empty())
    );
    let should_process_auto_payment = state.network_requests.should_process_buyer_response_event(
        event.request_id.as_str(),
        event.provider_pubkey.as_str(),
        event.event_id.as_str(),
    );
    let resolution_action = match event.kind {
        ProviderNip90BuyerResponseKind::Feedback => state
            .network_requests
            .apply_nip90_buyer_feedback_event_with_relay(
                event.request_id.as_str(),
                event.provider_pubkey.as_str(),
                event.event_id.as_str(),
                event.relay_url.as_deref(),
                event.status.as_deref(),
                event.status_extra.as_deref(),
                event.amount_msats,
                event.bolt11.as_deref(),
            ),
        ProviderNip90BuyerResponseKind::Result => state
            .network_requests
            .apply_nip90_buyer_result_event_with_relay(
                event.request_id.as_str(),
                event.provider_pubkey.as_str(),
                event.event_id.as_str(),
                event.relay_url.as_deref(),
                event.status.as_deref(),
            ),
    };
    if resolution_action.is_none() && should_process_auto_payment {
        queue_auto_payment_for_buyer_event(state, &event, now_epoch_seconds);
    }

    state.provider_runtime.last_result = Some(format!(
        "buyer request {} received {} event {} from {}",
        event.request_id,
        event.kind.label(),
        event.event_id,
        event.provider_pubkey
    ));
    state.provider_runtime.last_authoritative_status = event
        .status
        .as_ref()
        .map(|status| format!("buyer.{}", status));
    state.provider_runtime.last_authoritative_event_id = Some(event.event_id.clone());

    state.activity_feed.upsert_event(ActivityEventRow {
        event_id: format!("nip90:buyer:{}:{}", event.kind.label(), event.event_id),
        domain: ActivityEventDomain::Network,
        source_tag: "nip90.buyer".to_string(),
        summary: format!(
            "Buyer request {} {} update",
            event.request_id,
            event.kind.label()
        ),
        detail: format!(
            "request={} provider_nostr={} relay_url={} event_id={} kind={} status={} status_extra={} amount_msats={} bolt11={}\n\nshape:\n{}\n\nraw_event_json:\n{}",
            event.request_id,
            event.provider_pubkey,
            event.relay_url.as_deref().unwrap_or("unknown"),
            event.event_id,
            event.kind.label(),
            event.status.as_deref().unwrap_or("none"),
            event.status_extra.as_deref().unwrap_or("none"),
            event
                .amount_msats
                .map(|amount| amount.to_string())
                .unwrap_or_else(|| "none".to_string()),
            event.bolt11.as_deref().unwrap_or("none"),
            event
                .parsed_event_shape
                .as_deref()
                .unwrap_or("shape unavailable"),
            event
                .raw_event_json
                .as_deref()
                .unwrap_or("raw event json unavailable"),
        ),
        occurred_at_epoch_seconds: now_epoch_seconds,
    });
    state.activity_feed.load_state = PaneLoadState::Ready;

    if let Some(action) = resolution_action {
        emit_buyer_resolution_telemetry(state, &event, &action, now_epoch_seconds);
        queue_buyer_resolution_feedback(state, &action, now_epoch_seconds);
    }

    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

fn queue_auto_payment_for_buyer_event(
    state: &mut RenderState,
    event: &ProviderNip90BuyerResponseEvent,
    now_epoch_seconds: u64,
) {
    if matches!(event.kind, ProviderNip90BuyerResponseKind::Feedback)
        && event
            .status
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
            == Some("payment-required")
        && event
            .bolt11
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
    {
        tracing::error!(
            target: "autopilot_desktop::buyer",
            "Buyer payment-required feedback missing bolt11 request_id={} event_id={}",
            event.request_id,
            event.event_id
        );
        state.network_requests.record_auto_payment_notice(
            event.request_id.as_str(),
            "provider returned payment-required without bolt11 invoice; waiting for a valid invoice event",
            now_epoch_seconds,
        );
        state.provider_runtime.last_result = Some(format!(
            "buyer request {} is waiting for a valid provider invoice",
            event.request_id
        ));
        return;
    }

    let Some((payment_request, amount_sats)) = state
        .network_requests
        .prepare_auto_payment_attempt_for_provider(
            event.request_id.as_str(),
            event.provider_pubkey.as_str(),
            now_epoch_seconds,
        )
    else {
        if let Some(refusal) = state
            .network_requests
            .auto_payment_budget_refusal_for_provider(
                event.request_id.as_str(),
                event.provider_pubkey.as_str(),
            )
        {
            tracing::warn!(
                target: "autopilot_desktop::buyer",
                "Refusing over-budget Spark payment request_id={} provider_nostr={} invoice_amount_sats={} approved_budget_sats={} amount_mismatch={} event_id={}",
                event.request_id,
                refusal.provider_pubkey,
                refusal.invoice_amount_sats,
                refusal.approved_budget_sats,
                refusal.amount_mismatch,
                event.event_id
            );
            let notice = refusal.notice_message();
            state.network_requests.record_auto_payment_notice(
                event.request_id.as_str(),
                notice.as_str(),
                now_epoch_seconds,
            );
            state.provider_runtime.last_result = Some(format!(
                "buyer request {} blocked: {}",
                event.request_id, notice
            ));
        }
        return;
    };
    tracing::info!(
        target: "autopilot_desktop::buyer",
        "Queueing Spark payment for buyer request_id={} amount_sats={} bolt11_present={}",
        event.request_id,
        amount_sats
            .map(|amount| amount.to_string())
            .unwrap_or_else(|| "none".to_string()),
        !payment_request.trim().is_empty()
    );

    if let Err(error) = state.spark_worker.enqueue(SparkWalletCommand::SendPayment {
        payment_request,
        amount_sats,
    }) {
        tracing::error!(
            target: "autopilot_desktop::buyer",
            "Failed to enqueue Spark payment for request_id={}: {}",
            event.request_id,
            error
        );
        state.network_requests.mark_auto_payment_failed(
            event.request_id.as_str(),
            format!("failed to enqueue Spark payment command: {error}").as_str(),
            now_epoch_seconds,
        );
        return;
    }

    nip90_compute_domain_events::emit_buyer_queued_payment(
        event.request_id.as_str(),
        Some(event.provider_pubkey.as_str()),
        Some(event.event_id.as_str()),
        amount_sats,
    );
}

fn emit_buyer_resolution_telemetry(
    state: &mut RenderState,
    observed_event: &ProviderNip90BuyerResponseEvent,
    action: &BuyerResolutionAction,
    now_epoch_seconds: u64,
) {
    state.activity_feed.upsert_event(ActivityEventRow {
        event_id: format!(
            "buyer-resolution:{}:{}:{}",
            action.reason.code(),
            action.request_id,
            observed_event.event_id
        ),
        domain: ActivityEventDomain::Network,
        source_tag: "nip90.buyer_resolution".to_string(),
        summary: format!(
            "Request {} marked {} for provider {}",
            action.request_id,
            action.reason.code(),
            action.provider_pubkey
        ),
        detail: format!(
            "request={} provider_nostr={} observed_event={} observed_kind={} observed_status={} observed_status_extra={} resolution_mode=race",
            action.request_id,
            action.provider_pubkey,
            observed_event.event_id,
            observed_event.kind.label(),
            observed_event.status.as_deref().unwrap_or("none"),
            observed_event.status_extra.as_deref().unwrap_or("none"),
        ),
        occurred_at_epoch_seconds: now_epoch_seconds,
    });
    state.activity_feed.load_state = PaneLoadState::Ready;
}

fn queue_buyer_resolution_feedback(
    state: &mut RenderState,
    action: &BuyerResolutionAction,
    now_epoch_seconds: u64,
) {
    let Some(identity) = state.nostr_identity.as_ref() else {
        state.provider_runtime.last_error_detail = Some(
            "Cannot publish buyer race-resolution feedback: Nostr identity unavailable".to_string(),
        );
        state.activity_feed.upsert_event(ActivityEventRow {
            event_id: format!(
                "buyer-resolution-feedback:error:{}:{}",
                action.request_id, action.provider_pubkey
            ),
            domain: ActivityEventDomain::Network,
            source_tag: "nip90.buyer_resolution".to_string(),
            summary: format!(
                "Failed buyer feedback {} for {}",
                action.reason.code(),
                action.provider_pubkey
            ),
            detail: "nostr identity unavailable".to_string(),
            occurred_at_epoch_seconds: now_epoch_seconds,
        });
        return;
    };

    let event = match build_buyer_resolution_feedback_event(identity, action) {
        Ok(event) => event,
        Err(error) => {
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.activity_feed.upsert_event(ActivityEventRow {
                event_id: format!(
                    "buyer-resolution-feedback:error:{}:{}",
                    action.request_id, action.provider_pubkey
                ),
                domain: ActivityEventDomain::Network,
                source_tag: "nip90.buyer_resolution".to_string(),
                summary: format!(
                    "Failed buyer feedback {} for {}",
                    action.reason.code(),
                    action.provider_pubkey
                ),
                detail: error,
                occurred_at_epoch_seconds: now_epoch_seconds,
            });
            return;
        }
    };
    let feedback_event_id = event.id.clone();
    let queue_result =
        state.queue_provider_nip90_lane_command(ProviderNip90LaneCommand::PublishEvent {
            request_id: action.request_id.clone(),
            role: ProviderNip90PublishRole::Feedback,
            event: Box::new(event),
        });
    match queue_result {
        Ok(_) => {
            state.network_requests.record_resolution_feedback(
                action.request_id.as_str(),
                action.provider_pubkey.as_str(),
                feedback_event_id.as_str(),
                action.reason,
            );
            state.activity_feed.upsert_event(ActivityEventRow {
                event_id: format!(
                    "buyer-resolution-feedback:{}:{}:{}",
                    action.reason.code(),
                    action.request_id,
                    feedback_event_id
                ),
                domain: ActivityEventDomain::Network,
                source_tag: "nip90.buyer_resolution".to_string(),
                summary: format!(
                    "Queued buyer feedback {} for {}",
                    action.reason.code(),
                    action.provider_pubkey
                ),
                detail: format!(
                    "request={} provider_nostr={} feedback_event_id={}",
                    action.request_id, action.provider_pubkey, feedback_event_id
                ),
                occurred_at_epoch_seconds: now_epoch_seconds,
            });
        }
        Err(error) => {
            state.provider_runtime.last_error_detail = Some(error.clone());
            state.activity_feed.upsert_event(ActivityEventRow {
                event_id: format!(
                    "buyer-resolution-feedback:error:{}:{}",
                    action.request_id, action.provider_pubkey
                ),
                domain: ActivityEventDomain::Network,
                source_tag: "nip90.buyer_resolution".to_string(),
                summary: format!(
                    "Failed buyer feedback {} for {}",
                    action.reason.code(),
                    action.provider_pubkey
                ),
                detail: error,
                occurred_at_epoch_seconds: now_epoch_seconds,
            });
        }
    }
    state.activity_feed.load_state = PaneLoadState::Ready;
}

fn build_buyer_resolution_feedback_event(
    identity: &NostrIdentity,
    action: &BuyerResolutionAction,
) -> Result<Event, String> {
    let (status, status_extra, content) = match action.reason {
        BuyerResolutionReason::LostRace => (
            JobStatus::Success,
            BuyerResolutionReason::LostRace.code(),
            "Another provider already won this public race; stop work and do not expect payment.",
        ),
        BuyerResolutionReason::LateResultUnpaid => (
            JobStatus::Success,
            BuyerResolutionReason::LateResultUnpaid.code(),
            "Another provider already won this public race; this late result was observed but will not be paid.",
        ),
        BuyerResolutionReason::FirstValidResult => {
            return Err("first-valid-result does not emit loser feedback".to_string());
        }
    };
    let template = create_job_feedback_event(
        &JobFeedback::new(
            status,
            action.request_id.clone(),
            action.provider_pubkey.clone(),
        )
        .with_status_extra(status_extra)
        .with_content(content),
    );
    sign_nip90_feedback_template(identity, &template)
}

fn sign_nip90_feedback_template(
    identity: &NostrIdentity,
    template: &EventTemplate,
) -> Result<Event, String> {
    let private_key = parse_nostr_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| format!("Cannot sign buyer resolution feedback: {error}"))
}

fn parse_nostr_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| format!("invalid identity private_key_hex: {error}"))?;
    if key_bytes.len() != 32 {
        return Err(format!(
            "invalid identity private_key_hex length {}, expected 32 bytes",
            key_bytes.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

pub(super) fn apply_publish_outcome(state: &mut RenderState, outcome: ProviderNip90PublishOutcome) {
    let publish_succeeded = outcome.accepted_relays > 0;
    if outcome.role == ProviderNip90PublishRole::Request {
        let request_type = state
            .network_requests
            .submitted
            .iter()
            .find(|request| request.request_id == outcome.request_id)
            .map(|request| request.request_type.as_str())
            .unwrap_or("unknown");
        if publish_succeeded {
            tracing::info!(
                target: "autopilot_desktop::buyer",
                "Published NIP-90 request request_id={} request_type={} event_id={} accepted_relays={} rejected_relays={}",
                outcome.request_id,
                request_type,
                outcome.event_id,
                outcome.accepted_relays,
                outcome.rejected_relays
            );
        } else {
            tracing::error!(
                target: "autopilot_desktop::buyer",
                "Failed NIP-90 request publish request_id={} request_type={} event_id={} accepted_relays={} rejected_relays={} error={}",
                outcome.request_id,
                request_type,
                outcome.event_id,
                outcome.accepted_relays,
                outcome.rejected_relays,
                outcome
                    .first_error
                    .as_deref()
                    .unwrap_or("all relays rejected publish")
            );
        }
    }

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
        "{} {} {} (accepted={}, rejected={})",
        publish_label,
        outcome.role.protocol_label(),
        outcome.event_id,
        outcome.accepted_relays,
        outcome.rejected_relays
    ));

    let active_job_matches_publish =
        super::active_job_matches_publish_outcome(&state.active_job, &outcome);
    if active_job_matches_publish && let Some(job) = state.active_job.job.as_mut() {
        if publish_succeeded {
            match outcome.role {
                ProviderNip90PublishRole::Capability => {}
                ProviderNip90PublishRole::Request => {}
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
            "{} {} {} (accepted={}, rejected={})",
            publish_label,
            outcome.role.protocol_label(),
            outcome.event_id,
            outcome.accepted_relays,
            outcome.rejected_relays
        ));
    }

    super::apply_active_job_publish_outcome(state, &outcome);

    if outcome.role == ProviderNip90PublishRole::Request {
        state
            .network_requests
            .apply_nip90_request_publish_outcome_with_relays(
                outcome.request_id.as_str(),
                outcome.event_id.as_str(),
                outcome.selected_relays.as_slice(),
                outcome.accepted_relay_urls.as_slice(),
                outcome.rejected_relay_urls.as_slice(),
                outcome.accepted_relays,
                outcome.rejected_relays,
                outcome.first_error.as_deref(),
            );
    }

    let now_epoch_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    state.activity_feed.upsert_event(ActivityEventRow {
        event_id: format!("nostr:{}:{}", outcome.role.label(), outcome.event_id),
        domain: ActivityEventDomain::Network,
        source_tag: if outcome.role == ProviderNip90PublishRole::Capability {
            "nip89.publish".to_string()
        } else {
            "nip90.publish".to_string()
        },
        summary: if publish_succeeded {
            format!("Published {}", outcome.role.protocol_label())
        } else {
            format!("Failed {} publish", outcome.role.protocol_label())
        },
        detail: format!(
            "request={} event_id={} accepted_relays={} rejected_relays={} error={}\n\nshape:\n{}\n\nraw_event_json:\n{}",
            outcome.request_id,
            outcome.event_id,
            outcome.accepted_relays,
            outcome.rejected_relays,
            outcome.first_error.as_deref().unwrap_or("none"),
            outcome
                .parsed_event_shape
                .as_deref()
                .unwrap_or("shape unavailable"),
            outcome
                .raw_event_json
                .as_deref()
                .unwrap_or("raw event json unavailable"),
        ),
        occurred_at_epoch_seconds: now_epoch_seconds,
    });
    state.activity_feed.load_state = PaneLoadState::Ready;
}

#[cfg(test)]
mod tests {
    use super::{
        decrypt_encrypted_request_payload, ingress_is_preview_only, normalize_provider_keys,
        relay_connections_surface_error, target_policy_reject_reason_for,
    };
    use crate::provider_nip90_lane::ProviderNip90LaneMode;

    fn fixture_identity() -> nostr::NostrIdentity {
        nostr::NostrIdentity {
            identity_path: std::path::PathBuf::from("/tmp/test-identity.mnemonic"),
            mnemonic: "test mnemonic".to_string(),
            npub: "npub1localprovider".to_string(),
            nsec: "nsec1localprovider".to_string(),
            public_key_hex: "aa".repeat(32),
            private_key_hex: "11".repeat(32),
        }
    }

    #[test]
    fn normalize_provider_keys_trims_dedups_and_lowercases() {
        let normalized = normalize_provider_keys(&[
            "  NPUB1LOCALPROVIDER  ".to_string(),
            "".to_string(),
            "npub1localprovider".to_string(),
            "AA".repeat(32),
        ]);
        assert_eq!(
            normalized,
            vec!["aa".repeat(32), "npub1localprovider".to_string()]
        );
    }

    #[test]
    fn target_policy_accepts_when_local_provider_is_targeted() {
        let identity = fixture_identity();
        let reject_reason =
            target_policy_reject_reason_for(&[" npub1localprovider ".to_string()], Some(&identity));
        assert!(reject_reason.is_none());
    }

    #[test]
    fn target_policy_rejects_when_local_provider_not_targeted() {
        let identity = fixture_identity();
        let reject_reason =
            target_policy_reject_reason_for(&["npub1otherprovider".to_string()], Some(&identity));
        assert!(
            reject_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("target policy mismatch"))
        );
    }

    #[test]
    fn target_policy_rejects_targeted_requests_without_identity() {
        let reject_reason =
            target_policy_reject_reason_for(&["npub1otherprovider".to_string()], None);
        assert!(
            reject_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("local Nostr identity is unavailable"))
        );
    }

    #[test]
    fn decrypt_encrypted_request_payload_roundtrip_works_for_valid_keys() {
        let sender_secret = nostr::generate_secret_key();
        let sender_pubkey_hex =
            nostr::get_public_key_hex(&sender_secret).expect("sender public key should derive");

        let recipient_secret = nostr::generate_secret_key();
        let recipient_pubkey_hex = nostr::get_public_key_hex(&recipient_secret)
            .expect("recipient public key should derive");
        let recipient_pubkey_bytes =
            hex::decode(recipient_pubkey_hex).expect("recipient pubkey hex should decode");
        let mut recipient_compressed_pubkey = vec![0x02u8];
        recipient_compressed_pubkey.extend_from_slice(recipient_pubkey_bytes.as_slice());

        let payload = nostr::nip44::encrypt(
            &sender_secret,
            recipient_compressed_pubkey.as_slice(),
            "encrypted hello",
        )
        .expect("ciphertext should encrypt");

        let decrypted = decrypt_encrypted_request_payload(
            hex::encode(recipient_secret).as_str(),
            sender_pubkey_hex.as_str(),
            payload.as_str(),
        )
        .expect("payload should decrypt");
        assert_eq!(decrypted, "encrypted hello");
    }

    #[test]
    fn relay_connections_surface_error_hides_publish_failures() {
        assert_eq!(
            relay_connections_surface_error(Some(
                "Cannot publish NIP-90 result while provider lane is offline"
            )),
            None
        );
        assert_eq!(
            relay_connections_surface_error(Some(
                "Failed publishing NIP-90 feedback: relay refused write"
            )),
            None
        );
        assert_eq!(
            relay_connections_surface_error(Some("relay recv failed on wss://relay: boom")),
            Some("relay recv failed on wss://relay: boom".to_string())
        );
    }

    #[test]
    fn ingress_preview_detection_tracks_lane_mode_not_provider_mode_only() {
        assert!(
            ingress_is_preview_only(ProviderNip90LaneMode::Connecting),
            "connecting preview traffic should stay preview-only until the lane is online"
        );

        assert!(
            ingress_is_preview_only(ProviderNip90LaneMode::Preview),
            "preview lane traffic should stay preview-only"
        );

        assert!(
            !ingress_is_preview_only(ProviderNip90LaneMode::Online),
            "online lane traffic should enter the live accounting path"
        );
    }
}
