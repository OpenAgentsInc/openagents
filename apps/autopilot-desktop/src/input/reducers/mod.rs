//! Runtime update reducers are split by domain to keep `input.rs` focused on event routing.

mod ac;
mod apple_fm_workbench;
mod cad;
mod codex;
mod jobs;
mod local_inference;
mod provider_ingress;
mod sa;
mod skl;
pub(super) mod wallet;
pub(super) use cad::CadChatPromptApplyOutcome;

use crate::app_state::RenderState;
use crate::pane_system::{
    ActiveJobPaneAction, AgentProfileStatePaneAction, AgentScheduleTickPaneAction,
    CadDemoPaneAction, CreditDeskPaneAction, CreditSettlementLedgerPaneAction,
    JobHistoryPaneAction, JobInboxPaneAction, SkillRegistryPaneAction,
    SkillTrustRevocationPaneAction, TrajectoryAuditPaneAction,
};
use crate::runtime_lanes::{
    AcLaneUpdate, RuntimeCommandResponse, RuntimeCommandStatus, RuntimeLane, SaLaneUpdate,
    SklLaneUpdate,
};

pub(super) fn run_job_inbox_action(state: &mut RenderState, action: JobInboxPaneAction) -> bool {
    jobs::run_job_inbox_action(state, action)
}

pub(super) fn run_active_job_action(state: &mut RenderState, action: ActiveJobPaneAction) -> bool {
    jobs::run_active_job_action(state, action)
}

pub(super) fn run_job_history_action(
    state: &mut RenderState,
    action: JobHistoryPaneAction,
) -> bool {
    jobs::run_job_history_action(state, action)
}

pub(super) fn queue_codex_readiness_refresh(
    state: &mut RenderState,
    refresh_token: bool,
    reason: &str,
) {
    codex::queue_codex_readiness_refresh(state, refresh_token, reason);
}

pub(super) fn run_job_inbox_auto_admission_tick(state: &mut RenderState) -> bool {
    jobs::run_job_inbox_auto_admission_tick(state)
}

pub(super) fn run_active_job_execution_tick(state: &mut RenderState) -> bool {
    jobs::run_active_job_execution_tick(state)
}

pub(super) fn active_job_owns_codex_command_response(
    state: &RenderState,
    command_seq: u64,
) -> bool {
    jobs::active_job_owns_codex_command_response(state, command_seq)
}

pub(super) fn apply_active_job_codex_command_response(
    state: &mut RenderState,
    response: &crate::codex_lane::CodexLaneCommandResponse,
) {
    jobs::apply_active_job_codex_command_response(state, response)
}

pub(super) fn apply_active_job_codex_notification(
    state: &mut RenderState,
    notification: &crate::codex_lane::CodexLaneNotification,
) -> bool {
    jobs::apply_active_job_codex_notification(state, notification)
}

pub(super) fn apply_active_job_publish_outcome(
    state: &mut RenderState,
    outcome: &crate::provider_nip90_lane::ProviderNip90PublishOutcome,
) {
    jobs::apply_active_job_publish_outcome(state, outcome)
}

pub(super) fn apply_provider_ingressed_request_from_desktop_control(
    state: &mut RenderState,
    request: crate::app_state::JobInboxNetworkRequest,
) {
    provider_ingress::apply_ingressed_request(state, request);
}

pub(super) fn apply_provider_buyer_response_from_desktop_control(
    state: &mut RenderState,
    event: crate::provider_nip90_lane::ProviderNip90BuyerResponseEvent,
) {
    provider_ingress::apply_buyer_response_event(state, event);
}

pub(super) fn active_job_matches_publish_outcome(
    active_job: &crate::app_state::ActiveJobState,
    outcome: &crate::provider_nip90_lane::ProviderNip90PublishOutcome,
) -> bool {
    jobs::active_job_matches_publish_outcome(active_job, outcome)
}

pub(super) fn transition_active_job_to_paid(
    state: &mut RenderState,
    source: &str,
    now: std::time::Instant,
) -> Result<crate::app_state::JobLifecycleStage, String> {
    jobs::transition_active_job_to_paid(state, source, now)
}

pub(super) fn drain_runtime_lane_updates(state: &mut RenderState) -> bool {
    let mut changed = false;

    for update in state.sa_lane_worker.drain_updates() {
        changed = true;
        match update {
            SaLaneUpdate::Snapshot(snapshot) => sa::apply_lane_snapshot(state, *snapshot),
            SaLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.skl_lane_worker.drain_updates() {
        changed = true;
        match update {
            SklLaneUpdate::Snapshot(snapshot) => skl::apply_lane_snapshot(state, *snapshot),
            SklLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.ac_lane_worker.drain_updates() {
        changed = true;
        match update {
            AcLaneUpdate::Snapshot(snapshot) => ac::apply_lane_snapshot(state, *snapshot),
            AcLaneUpdate::CommandResponse(response) => {
                apply_runtime_command_response(state, response);
            }
        }
    }

    for update in state.provider_nip90_lane_worker.drain_updates() {
        changed = true;
        match update {
            crate::provider_nip90_lane::ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                provider_ingress::apply_lane_snapshot(state, *snapshot);
            }
            crate::provider_nip90_lane::ProviderNip90LaneUpdate::IngressedRequest(request) => {
                provider_ingress::apply_ingressed_request(state, request);
            }
            crate::provider_nip90_lane::ProviderNip90LaneUpdate::BuyerResponseEvent(event) => {
                provider_ingress::apply_buyer_response_event(state, event);
            }
            crate::provider_nip90_lane::ProviderNip90LaneUpdate::PublishOutcome(outcome) => {
                provider_ingress::apply_publish_outcome(state, outcome);
            }
        }
    }

    for update in state.apple_fm_execution_worker.drain_updates() {
        changed |= apple_fm_workbench::apply_bridge_update(state, &update);
        changed |= jobs::apply_active_job_apple_fm_update(state, update);
    }

    for update in state.local_inference_runtime.drain_updates() {
        if let crate::local_inference_runtime::LocalInferenceRuntimeUpdate::Failed(failed) = &update
        {
            tracing::error!(
                "local inference runtime failed request_id={} error={}",
                failed.request_id,
                failed.error
            );
        }
        changed |= local_inference::apply_runtime_update(state, &update);
        changed |= jobs::apply_active_job_local_inference_runtime_update(state, update);
    }

    for update in state.codex_lane_worker.drain_updates() {
        changed = true;
        match update {
            crate::codex_lane::CodexLaneUpdate::Snapshot(snapshot) => {
                codex::apply_lane_snapshot(state, *snapshot);
            }
            crate::codex_lane::CodexLaneUpdate::CommandResponse(response) => {
                codex::apply_command_response(state, response);
            }
            crate::codex_lane::CodexLaneUpdate::Notification(notification) => {
                codex::apply_notification(state, notification);
            }
        }
    }

    let nip28_updates = state.nip28_chat_lane_worker.drain_updates();
    if !nip28_updates.is_empty() {
        tracing::debug!(count = nip28_updates.len(), "nip28: drain");
    }
    let mut nip28_relay_events = Vec::new();
    for update in nip28_updates {
        use crate::nip28_chat_lane::Nip28ChatLaneUpdate;
        changed = true;
        match update {
            Nip28ChatLaneUpdate::RelayEvent(event) => {
                nip28_relay_events.push(event);
            }
            Nip28ChatLaneUpdate::PublishAck { event_id } => {
                tracing::info!(event_id = %event_id, "nip28: outbound ack");
                let _ = state
                    .autopilot_chat
                    .managed_chat_projection
                    .ack_outbound_message(&event_id);
                state.nip28_chat_lane_worker.clear_dispatched(&event_id);
            }
            Nip28ChatLaneUpdate::PublishError { event_id, message } => {
                tracing::warn!(event_id = %event_id, message = %message, "nip28: outbound error");
                let _ = state
                    .autopilot_chat
                    .managed_chat_projection
                    .fail_outbound_message(&event_id, &message);
                state.nip28_chat_lane_worker.clear_dispatched(&event_id);
            }
            Nip28ChatLaneUpdate::Eose { .. } | Nip28ChatLaneUpdate::ConnectionError { .. } => {}
        }
    }
    if !nip28_relay_events.is_empty() {
        state
            .autopilot_chat
            .managed_chat_projection
            .record_relay_events(nip28_relay_events);
    }

    {
        use crate::app_state::ManagedChatDeliveryState;
        let pending_events: Vec<_> = state
            .autopilot_chat
            .managed_chat_projection
            .outbound_messages
            .iter()
            .filter(|message| message.delivery_state == ManagedChatDeliveryState::Publishing)
            .map(|message| message.event.clone())
            .collect();
        for event in pending_events {
            state.nip28_chat_lane_worker.publish(event);
        }
    }

    if state
        .autopilot_chat
        .maybe_auto_select_default_nip28_channel()
    {
        tracing::info!("nip28: auto-selected default channel");
        changed = true;
    }

    changed
}

pub(super) fn drain_spark_worker_updates(state: &mut RenderState) -> bool {
    wallet::drain_spark_worker_updates(state)
}

pub(super) fn drain_stable_sats_blink_worker_updates(state: &mut RenderState) -> bool {
    let updates = state.stable_sats_blink_worker.drain_updates(4);
    if updates.is_empty() {
        return false;
    }

    let mut changed = false;
    for update in updates {
        match update {
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::CommandStarted {
                request_id,
                kind,
            } => {
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                let operation_kind = map_stablesats_operation_kind(kind).or_else(|| {
                    state
                        .stable_sats_simulation
                        .treasury_operations
                        .iter()
                        .rev()
                        .find(|entry| entry.request_id == request_id)
                        .map(|entry| entry.kind)
                });
                if let Some(operation_kind) = operation_kind {
                    state
                        .stable_sats_simulation
                        .record_treasury_operation_running(
                            request_id,
                            operation_kind,
                            now_epoch_seconds,
                            format!("{} running", kind.label()),
                        );
                    changed = true;
                }
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::CommandCancelled {
                request_id,
                kind,
                detail,
            } => {
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                if let Some(operation_kind) = map_stablesats_operation_kind(kind) {
                    state
                        .stable_sats_simulation
                        .record_treasury_operation_finished(
                            request_id,
                            operation_kind,
                            crate::app_state::StableSatsTreasuryOperationStatus::Cancelled,
                            now_epoch_seconds,
                            detail,
                        );
                    changed = true;
                }
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::Completed(snapshot) => {
                if !state
                    .stable_sats_simulation
                    .finish_live_refresh(snapshot.request_id)
                {
                    continue;
                }
                let wallet_snapshots = snapshot
                    .wallet_snapshots
                    .iter()
                    .map(|wallet| {
                        (
                            wallet.owner_id.clone(),
                            wallet.btc_balance_sats,
                            wallet.usd_balance_cents,
                            wallet.source_ref.clone(),
                        )
                    })
                    .collect::<Vec<_>>();
                let wallet_failures = snapshot
                    .wallet_failures
                    .iter()
                    .map(|failure| (failure.owner_id.clone(), failure.error.clone()))
                    .collect::<Vec<_>>();
                state.stable_sats_simulation.apply_live_wallet_snapshots(
                    snapshot.now_epoch_seconds,
                    snapshot.price_usd_cents_per_btc,
                    wallet_snapshots.as_slice(),
                    wallet_failures.as_slice(),
                );
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        snapshot.request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::Refresh,
                        crate::app_state::StableSatsTreasuryOperationStatus::Settled,
                        snapshot.now_epoch_seconds,
                        format!(
                            "live refresh settled with {} wallet snapshot(s) and {} failure(s)",
                            snapshot.wallet_snapshots.len(),
                            snapshot.wallet_failures.len()
                        ),
                    );
                state.provider_runtime.last_result =
                    state.stable_sats_simulation.last_action.clone();
                let event_id = format!(
                    "sim:stablesats:round:{}",
                    state.stable_sats_simulation.rounds_run
                );
                state
                    .activity_feed
                    .upsert_event(crate::app_state::ActivityEventRow {
                        event_id,
                        domain: crate::app_state::ActivityEventDomain::Wallet,
                        source_tag: "blink.live".to_string(),
                        summary: "StableSats live Blink balances refreshed".to_string(),
                        detail: format!(
                            "mode={} round={} quote={} converted_sats={} converted_usd_cents={}",
                            state.stable_sats_simulation.mode.label(),
                            state.stable_sats_simulation.rounds_run,
                            state.stable_sats_simulation.price_usd_cents_per_btc,
                            state.stable_sats_simulation.total_converted_sats,
                            state.stable_sats_simulation.total_converted_usd_cents
                        ),
                        occurred_at_epoch_seconds: snapshot.now_epoch_seconds,
                    });
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::Failed { request_id, error } => {
                if state
                    .stable_sats_simulation
                    .fail_live_refresh(request_id, error)
                {
                    state.provider_runtime.last_result =
                        state.stable_sats_simulation.last_action.clone();
                    changed = true;
                }
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::SwapQuoteCompleted(result) => {
                let quote_id = result
                    .payload
                    .pointer("/quote/quoteId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        result.request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::SwapQuote,
                        crate::app_state::StableSatsTreasuryOperationStatus::Settled,
                        result.now_epoch_seconds,
                        format!(
                            "swap quote completed goal={} request={} quote={}",
                            result.goal_id, result.adapter_request_id, quote_id
                        ),
                    );
                state
                    .activity_feed
                    .upsert_event(crate::app_state::ActivityEventRow {
                        event_id: format!("swap:quote:{}:{}", result.goal_id, result.request_id),
                        domain: crate::app_state::ActivityEventDomain::Wallet,
                        source_tag: "blink.live".to_string(),
                        summary: "Blink swap quote completed".to_string(),
                        detail: format!(
                            "goal={} request={} quote={} script={} args={}",
                            result.goal_id,
                            result.adapter_request_id,
                            quote_id,
                            result.script_path,
                            result.script_args.join(" ")
                        ),
                        occurred_at_epoch_seconds: result.now_epoch_seconds,
                    });
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "swap quote completed goal={} request={} quote={} worker_request={}",
                    result.goal_id, result.adapter_request_id, quote_id, result.request_id
                ));
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::SwapQuoteFailed {
                request_id,
                goal_id,
                adapter_request_id,
                error,
            } => {
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::SwapQuote,
                        crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                        now_epoch_seconds,
                        format!(
                            "swap quote failed goal={} request={}: {}",
                            goal_id, adapter_request_id, error
                        ),
                    );
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "swap quote failed goal={} request={} worker_request={} error={}",
                    goal_id, adapter_request_id, request_id, error
                ));
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::SwapExecuteCompleted(result) => {
                let status = result
                    .payload
                    .get("status")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("UNKNOWN");
                let transaction_id = result
                    .payload
                    .pointer("/execution/transactionId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        result.request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::SwapExecute,
                        crate::app_state::StableSatsTreasuryOperationStatus::Settled,
                        result.now_epoch_seconds,
                        format!(
                            "swap execute completed goal={} quote={} status={} tx={}",
                            result.goal_id, result.quote_id, status, transaction_id
                        ),
                    );
                state
                    .activity_feed
                    .upsert_event(crate::app_state::ActivityEventRow {
                        event_id: format!("swap:execute:{}:{}", result.goal_id, result.request_id),
                        domain: crate::app_state::ActivityEventDomain::Wallet,
                        source_tag: "blink.live".to_string(),
                        summary: "Blink swap execute completed".to_string(),
                        detail: format!(
                            "goal={} quote={} status={} tx={} script={} args={}",
                            result.goal_id,
                            result.quote_id,
                            status,
                            transaction_id,
                            result.script_path,
                            result.script_args.join(" ")
                        ),
                        occurred_at_epoch_seconds: result.now_epoch_seconds,
                    });
                state.activity_feed.load_state = crate::app_state::PaneLoadState::Ready;
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "swap execute completed goal={} quote={} status={} tx={} worker_request={}",
                    result.goal_id, result.quote_id, status, transaction_id, result.request_id
                ));
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::SwapExecuteFailed {
                request_id,
                goal_id,
                quote_id,
                error,
            } => {
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::SwapExecute,
                        crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                        now_epoch_seconds,
                        format!(
                            "swap execute failed goal={} quote={}: {}",
                            goal_id, quote_id, error
                        ),
                    );
                state.autopilot_chat.record_turn_timeline_event(format!(
                    "swap execute failed goal={} quote={} worker_request={} error={}",
                    goal_id, quote_id, request_id, error
                ));
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::TransferCompleted(result) => {
                let operation_kind = match result.asset {
                    crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::BtcSats => {
                        crate::app_state::StableSatsTreasuryOperationKind::TransferBtc
                    }
                    crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::UsdCents => {
                        crate::app_state::StableSatsTreasuryOperationKind::TransferUsd
                    }
                };
                let payment_status_upper = result.payment_status.trim().to_ascii_uppercase();
                let success = matches!(payment_status_upper.as_str(), "SUCCESS" | "ALREADY_PAID");
                let operation_status = if success {
                    crate::app_state::StableSatsTreasuryOperationStatus::Settled
                } else {
                    crate::app_state::StableSatsTreasuryOperationStatus::Failed
                };
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        result.request_id,
                        operation_kind,
                        operation_status,
                        result.now_epoch_seconds,
                        format!(
                            "transfer {} {} status={} reference={}",
                            result.amount,
                            result.asset.label(),
                            result.payment_status,
                            result.payment_reference.as_deref().unwrap_or("n/a")
                        ),
                    );
                state.stable_sats_simulation.record_treasury_receipt(
                    result.request_id,
                    operation_kind,
                    result.now_epoch_seconds,
                    serde_json::json!({
                        "request_id": result.request_id,
                        "kind": operation_kind.label(),
                        "status": operation_status.label(),
                        "asset": result.asset.label(),
                        "amount": result.amount,
                        "payment_status": result.payment_status,
                        "payment_reference": result.payment_reference,
                        "estimated_fee_sats": result.estimated_fee_sats,
                        "effective_fee": result.effective_fee,
                        "source": {
                            "owner_id": result.from_owner_id,
                            "wallet_name": result.from_wallet_name,
                            "pre_btc_sats": result.source_pre_btc_sats,
                            "pre_usd_cents": result.source_pre_usd_cents,
                            "post_btc_sats": result.source_post_btc_sats,
                            "post_usd_cents": result.source_post_usd_cents,
                        },
                        "destination": {
                            "owner_id": result.to_owner_id,
                            "wallet_name": result.to_wallet_name,
                            "pre_btc_sats": result.destination_pre_btc_sats,
                            "pre_usd_cents": result.destination_pre_usd_cents,
                            "post_btc_sats": result.destination_post_btc_sats,
                            "post_usd_cents": result.destination_post_usd_cents,
                        },
                        "raw": result.payload,
                    }),
                );
                state.stable_sats_simulation.apply_wallet_balance(
                    result.from_owner_id.as_str(),
                    result.source_post_btc_sats,
                    result.source_post_usd_cents,
                    format!(
                        "sent {} {} status={} fee={}",
                        result.amount,
                        result.asset.label(),
                        result.payment_status,
                        result.effective_fee
                    ),
                );
                state.stable_sats_simulation.apply_wallet_balance(
                    result.to_owner_id.as_str(),
                    result.destination_post_btc_sats,
                    result.destination_post_usd_cents,
                    format!(
                        "received {} {} status={}",
                        result.amount,
                        result.asset.label(),
                        result.payment_status
                    ),
                );
                state.stable_sats_simulation.record_external_transfer(
                    result.now_epoch_seconds,
                    format!("blink:live:transfer:worker:{:08}", result.request_id),
                    format!("{}:{}", result.from_wallet_name, result.asset.label()),
                    format!("{}:{}", result.to_wallet_name, result.asset.label()),
                    match result.asset {
                        crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::BtcSats => {
                            crate::app_state::StableSatsTransferAsset::BtcSats
                        }
                        crate::stablesats_blink_worker::StableSatsBlinkTransferAsset::UsdCents => {
                            crate::app_state::StableSatsTransferAsset::UsdCents
                        }
                    },
                    result.amount,
                    result.effective_fee,
                    if success {
                        crate::app_state::StableSatsTransferStatus::Settled
                    } else {
                        crate::app_state::StableSatsTransferStatus::Failed
                    },
                    format!(
                        "transfer {} {} status={} ref={}",
                        result.amount,
                        result.asset.label(),
                        result.payment_status,
                        result.payment_reference.as_deref().unwrap_or("n/a")
                    ),
                );
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::TransferFailed {
                request_id,
                from_owner_id,
                to_owner_id,
                error,
            } => {
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                let operation_kind = state
                    .stable_sats_simulation
                    .treasury_operations
                    .iter()
                    .rev()
                    .find(|entry| entry.request_id == request_id)
                    .map(|entry| entry.kind)
                    .unwrap_or(crate::app_state::StableSatsTreasuryOperationKind::TransferBtc);
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        request_id,
                        operation_kind,
                        crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                        now_epoch_seconds,
                        format!(
                            "transfer failed from={} to={} error={}",
                            from_owner_id, to_owner_id, error
                        ),
                    );
                state.stable_sats_simulation.record_treasury_receipt(
                    request_id,
                    operation_kind,
                    now_epoch_seconds,
                    serde_json::json!({
                        "request_id": request_id,
                        "kind": operation_kind.label(),
                        "status": "failed",
                        "from_owner_id": from_owner_id,
                        "to_owner_id": to_owner_id,
                        "error": error,
                    }),
                );
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::ConvertCompleted(result) => {
                let status_upper = result.status.trim().to_ascii_uppercase();
                let success = matches!(status_upper.as_str(), "SUCCESS");
                let operation_status = if success {
                    crate::app_state::StableSatsTreasuryOperationStatus::Settled
                } else {
                    crate::app_state::StableSatsTreasuryOperationStatus::Failed
                };
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        result.request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::Convert,
                        operation_status,
                        result.now_epoch_seconds,
                        format!(
                            "convert {} {} direction={} status={} tx={}",
                            result.amount,
                            result.unit,
                            result.direction,
                            result.status,
                            result.transaction_id.as_deref().unwrap_or("n/a")
                        ),
                    );
                state.stable_sats_simulation.record_treasury_receipt(
                    result.request_id,
                    crate::app_state::StableSatsTreasuryOperationKind::Convert,
                    result.now_epoch_seconds,
                    serde_json::json!({
                        "request_id": result.request_id,
                        "kind": "convert",
                        "status": result.status,
                        "owner_id": result.owner_id,
                        "wallet_name": result.wallet_name,
                        "direction": result.direction,
                        "amount": result.amount,
                        "unit": result.unit,
                        "quote_id": result.quote_id,
                        "transaction_id": result.transaction_id,
                        "fee_sats": result.fee_sats,
                        "effective_spread_bps": result.effective_spread_bps,
                        "pre_btc_sats": result.pre_btc_sats,
                        "pre_usd_cents": result.pre_usd_cents,
                        "post_btc_sats": result.post_btc_sats,
                        "post_usd_cents": result.post_usd_cents,
                        "raw": result.payload,
                    }),
                );
                state.stable_sats_simulation.apply_wallet_balance(
                    result.owner_id.as_str(),
                    result.post_btc_sats,
                    result.post_usd_cents,
                    format!(
                        "convert {} {} direction={} status={}",
                        result.amount, result.unit, result.direction, result.status
                    ),
                );
                let (asset, amount) = if result.pre_btc_sats > result.post_btc_sats {
                    (
                        crate::app_state::StableSatsTransferAsset::BtcSats,
                        result.pre_btc_sats.saturating_sub(result.post_btc_sats),
                    )
                } else {
                    (
                        crate::app_state::StableSatsTransferAsset::UsdCents,
                        result.pre_usd_cents.saturating_sub(result.post_usd_cents),
                    )
                };
                state.stable_sats_simulation.record_external_transfer(
                    result.now_epoch_seconds,
                    format!("blink:live:convert:{:08}", result.request_id),
                    format!("{}:{}", result.wallet_name, "source"),
                    format!("{}:{}", result.wallet_name, "target"),
                    asset,
                    amount,
                    result.fee_sats,
                    if success {
                        crate::app_state::StableSatsTransferStatus::Settled
                    } else {
                        crate::app_state::StableSatsTransferStatus::Failed
                    },
                    format!(
                        "convert direction={} amount={} {} status={} quote={}",
                        result.direction,
                        result.amount,
                        result.unit,
                        result.status,
                        result.quote_id.as_deref().unwrap_or("n/a")
                    ),
                );
                changed = true;
            }
            crate::stablesats_blink_worker::StableSatsBlinkUpdate::ConvertFailed {
                request_id,
                owner_id,
                error,
            } => {
                let now_epoch_seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                state
                    .stable_sats_simulation
                    .record_treasury_operation_finished(
                        request_id,
                        crate::app_state::StableSatsTreasuryOperationKind::Convert,
                        crate::app_state::StableSatsTreasuryOperationStatus::Failed,
                        now_epoch_seconds,
                        format!("convert failed owner={} error={}", owner_id, error),
                    );
                state.stable_sats_simulation.record_treasury_receipt(
                    request_id,
                    crate::app_state::StableSatsTreasuryOperationKind::Convert,
                    now_epoch_seconds,
                    serde_json::json!({
                        "request_id": request_id,
                        "kind": "convert",
                        "status": "failed",
                        "owner_id": owner_id,
                        "error": error,
                    }),
                );
                changed = true;
            }
        }
    }

    changed
}

fn map_stablesats_operation_kind(
    kind: crate::stablesats_blink_worker::StableSatsBlinkCommandKind,
) -> Option<crate::app_state::StableSatsTreasuryOperationKind> {
    match kind {
        crate::stablesats_blink_worker::StableSatsBlinkCommandKind::Refresh => {
            Some(crate::app_state::StableSatsTreasuryOperationKind::Refresh)
        }
        crate::stablesats_blink_worker::StableSatsBlinkCommandKind::SwapQuote => {
            Some(crate::app_state::StableSatsTreasuryOperationKind::SwapQuote)
        }
        crate::stablesats_blink_worker::StableSatsBlinkCommandKind::SwapExecute => {
            Some(crate::app_state::StableSatsTreasuryOperationKind::SwapExecute)
        }
        crate::stablesats_blink_worker::StableSatsBlinkCommandKind::Transfer => None,
        crate::stablesats_blink_worker::StableSatsBlinkCommandKind::Convert => {
            Some(crate::app_state::StableSatsTreasuryOperationKind::Convert)
        }
    }
}

pub(super) fn run_agent_profile_state_action(
    state: &mut RenderState,
    action: AgentProfileStatePaneAction,
) -> bool {
    sa::run_agent_profile_state_action(state, action)
}

pub(super) fn refresh_goal_profile_state(state: &mut RenderState) -> bool {
    let profile_changed = sa::refresh_goal_profile_state(state);
    let schedule_changed = sa::refresh_goal_schedule_state(state);
    profile_changed || schedule_changed
}

pub(super) fn run_agent_schedule_tick_action(
    state: &mut RenderState,
    action: AgentScheduleTickPaneAction,
) -> bool {
    sa::run_agent_schedule_tick_action(state, action)
}

pub(super) fn run_trajectory_audit_action(
    state: &mut RenderState,
    action: TrajectoryAuditPaneAction,
) -> bool {
    sa::run_trajectory_audit_action(state, action)
}

pub(super) fn run_skill_registry_action(
    state: &mut RenderState,
    action: SkillRegistryPaneAction,
) -> bool {
    skl::run_skill_registry_action(state, action)
}

pub(super) fn run_skill_trust_revocation_action(
    state: &mut RenderState,
    action: SkillTrustRevocationPaneAction,
) -> bool {
    skl::run_skill_trust_revocation_action(state, action)
}

pub(super) fn run_credit_desk_action(
    state: &mut RenderState,
    action: CreditDeskPaneAction,
) -> bool {
    ac::run_credit_desk_action(state, action)
}

pub(super) fn run_credit_settlement_ledger_action(
    state: &mut RenderState,
    action: CreditSettlementLedgerPaneAction,
) -> bool {
    ac::run_credit_settlement_ledger_action(state, action)
}

pub(super) fn run_cad_demo_action(state: &mut RenderState, action: CadDemoPaneAction) -> bool {
    cad::run_cad_demo_action(state, action)
}

pub(super) fn bootstrap_startup_parallel_jaw_gripper(state: &mut RenderState) -> bool {
    cad::bootstrap_startup_parallel_jaw_gripper(state)
}

pub(super) fn apply_chat_prompt_to_cad_session(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
) -> bool {
    cad::apply_chat_prompt_to_cad_session(state, thread_id, prompt)
}

pub(super) fn apply_chat_prompt_to_cad_session_with_trigger(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
    rebuild_trigger_prefix: Option<&str>,
) -> bool {
    cad::apply_chat_prompt_to_cad_session_with_trigger(
        state,
        thread_id,
        prompt,
        rebuild_trigger_prefix,
    )
}

pub(super) fn apply_chat_prompt_to_cad_session_with_trigger_outcome(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
    rebuild_trigger_prefix: Option<&str>,
) -> CadChatPromptApplyOutcome {
    cad::apply_chat_prompt_to_cad_session_with_trigger_outcome(
        state,
        thread_id,
        prompt,
        rebuild_trigger_prefix,
    )
}

pub(super) fn sync_cad_build_progress_to_chat(state: &mut RenderState) {
    cad::sync_cad_build_progress_to_chat(state)
}

fn apply_runtime_command_response(state: &mut RenderState, response: RuntimeCommandResponse) {
    if !apply_stream_event_seq(state, "runtime.command", response.command_seq) {
        return;
    }

    let summary = command_response_summary(&response);
    match response.lane {
        RuntimeLane::SaLifecycle => sa::apply_command_response(state, &response, &summary),
        RuntimeLane::SklDiscoveryTrust => skl::apply_command_response(state, &response, &summary),
        RuntimeLane::AcCredit => ac::apply_command_response(state, &response, &summary),
    }

    if response.status != RuntimeCommandStatus::Accepted {
        super::upsert_runtime_incident_alert(state, &response);
    }

    state.record_runtime_command_response(response);
}

pub(super) fn apply_stream_event_seq(state: &mut RenderState, stream_id: &str, seq: u64) -> bool {
    match state.sync_apply_engine.apply_seq(stream_id, seq) {
        Ok(crate::sync_apply::StreamApplyDecision::Applied { .. }) => {
            state.sync_health.last_applied_event_seq = state.sync_apply_engine.max_checkpoint_seq();
            state.sync_health.cursor_last_advanced_seconds_ago = 0;
            mirror_remote_checkpoint_ack(state, stream_id, seq);
            true
        }
        Ok(crate::sync_apply::StreamApplyDecision::Duplicate { .. }) => {
            state.sync_health.duplicate_drop_count =
                state.sync_health.duplicate_drop_count.saturating_add(1);
            state.sync_health.last_action = Some(format!(
                "Dropped duplicate stream event {} seq={}",
                stream_id, seq
            ));
            false
        }
        Ok(crate::sync_apply::StreamApplyDecision::OutOfOrder {
            expected_seq,
            received_seq,
            ..
        }) => {
            let rewind_to = expected_seq.saturating_sub(1);
            let _ = state.sync_apply_engine.rewind_stream(stream_id, rewind_to);
            state.sync_health.rebootstrap();
            state.sync_health.last_error = Some(format!(
                "Out-of-order stream event {} expected={} received={}; rewound to {}",
                stream_id, expected_seq, received_seq, rewind_to
            ));
            state.sync_health.last_action =
                Some(format!("Replay rebootstrap required for {stream_id}"));
            let worker_id = state.sync_lifecycle_worker_id.clone();
            let _ = state.sync_lifecycle.mark_disconnect(
                worker_id.as_str(),
                crate::sync_lifecycle::RuntimeSyncDisconnectReason::StaleCursor,
                Some(format!(
                    "out-of-order stream event {} expected={} received={}",
                    stream_id, expected_seq, received_seq
                )),
            );
            state.sync_lifecycle.mark_replay_bootstrap(
                worker_id.as_str(),
                rewind_to,
                Some(rewind_to),
            );
            state.sync_lifecycle_snapshot = state.sync_lifecycle.snapshot(worker_id.as_str());
            false
        }
        Err(error) => {
            state.sync_health.last_error = Some(format!(
                "sync checkpoint apply failed for {} seq {}: {}",
                stream_id, seq, error
            ));
            state.sync_health.load_state = crate::app_state::PaneLoadState::Error;
            state.sync_health.last_applied_event_seq =
                state.sync_health.last_applied_event_seq.saturating_add(1);
            state.sync_health.cursor_last_advanced_seconds_ago = 0;
            true
        }
    }
}

fn mirror_remote_checkpoint_ack(state: &mut RenderState, stream_id: &str, seq: u64) {
    let Some(client) = state.spacetime_presence.live_client() else {
        return;
    };
    if let Err(error) =
        client.ack_checkpoint(state.sync_lifecycle_worker_id.as_str(), stream_id, seq, seq)
    {
        state.sync_health.last_error = Some(format!(
            "remote sync checkpoint mirror failed for {} seq {}: {}",
            stream_id, seq, error
        ));
    }
}

fn command_response_summary(response: &RuntimeCommandResponse) -> String {
    let mut parts = vec![format!(
        "{} {} {}",
        response.lane.label(),
        response.command.label(),
        response.status.label()
    )];
    if let Some(event_id) = response.event_id.as_deref() {
        parts.push(format!("event:{event_id}"));
    }
    if let Some(error) = response.error.as_ref() {
        parts.push(format!("{}:{}", error.class.label(), error.message));
    }
    parts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::{command_response_summary, map_stablesats_operation_kind};

    #[test]
    fn maps_stablesats_worker_command_kinds_to_operation_kinds() {
        use crate::app_state::StableSatsTreasuryOperationKind;
        use crate::stablesats_blink_worker::StableSatsBlinkCommandKind;

        assert_eq!(
            map_stablesats_operation_kind(StableSatsBlinkCommandKind::Refresh),
            Some(StableSatsTreasuryOperationKind::Refresh)
        );
        assert_eq!(
            map_stablesats_operation_kind(StableSatsBlinkCommandKind::SwapQuote),
            Some(StableSatsTreasuryOperationKind::SwapQuote)
        );
        assert_eq!(
            map_stablesats_operation_kind(StableSatsBlinkCommandKind::SwapExecute),
            Some(StableSatsTreasuryOperationKind::SwapExecute)
        );
        assert_eq!(
            map_stablesats_operation_kind(StableSatsBlinkCommandKind::Convert),
            Some(StableSatsTreasuryOperationKind::Convert)
        );
        assert_eq!(
            map_stablesats_operation_kind(StableSatsBlinkCommandKind::Transfer),
            None
        );
    }

    #[test]
    fn command_response_summary_preserves_event_and_error_context() {
        let summary = command_response_summary(&crate::runtime_lanes::RuntimeCommandResponse {
            lane: crate::runtime_lanes::RuntimeLane::SaLifecycle,
            command_seq: 42,
            command: crate::runtime_lanes::RuntimeCommandKind::PublishTickRequest,
            status: crate::runtime_lanes::RuntimeCommandStatus::Rejected,
            event_id: Some("event-123".to_string()),
            error: Some(crate::runtime_lanes::RuntimeCommandError {
                class: crate::runtime_lanes::RuntimeCommandErrorClass::Validation,
                message: "payload mismatch".to_string(),
            }),
        });
        assert!(summary.contains("sa_lifecycle PublishTickRequest rejected"));
        assert!(summary.contains("event:event-123"));
        assert!(summary.contains("validation:payload mismatch"));
    }
}
