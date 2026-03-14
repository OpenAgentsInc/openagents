use crate::app_state::{ActiveJobRecord, JobInboxDecision, RenderState};
use crate::local_inference_runtime::LocalInferenceExecutionMetrics;
use crate::state::job_inbox::JobInboxRequest;
use crate::state::operations::{
    AcceptedForwardComputeOrder, AcceptedSpotComputeOrder, ForwardComputeQuoteCandidate,
    ForwardComputeRfqDraft, SpotComputeCapabilityConstraints, SpotComputeQuoteCandidate,
    SpotComputeRfqDraft,
};
use crate::state::provider_runtime::LocalInferenceBackend;
use crate::state::provider_runtime::{ProviderInventoryProductToggleTarget, ProviderInventoryRow};
use openagents_kernel_core::authority::{
    CreateCapacityInstrumentRequest, CreateCapacityLotRequest, CreateComputeProductRequest,
    CreateContractRequest, CreateWorkUnitRequest, FinalizeVerdictRequest,
    HttpKernelAuthorityClient, KernelAuthority, RecordDeliveryProofRequest, SubmitOutputRequest,
    canonical_kernel_endpoint,
};
use openagents_kernel_core::compute::{
    ApplePlatformCapability, COMPUTE_LAUNCH_TAXONOMY_VERSION, CapacityInstrument,
    CapacityInstrumentKind, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
    CapacityReserveState, ComputeBackendFamily, ComputeCapabilityEnvelope,
    ComputeDeliveryVarianceReason, ComputeEnvironmentBinding, ComputeExecutionKind, ComputeFamily,
    ComputeHostCapability, ComputeProduct, ComputeProductStatus, ComputeProofPosture,
    ComputeProvisioningKind, ComputeSettlementMode, ComputeTopologyKind, DeliveryProof,
    DeliveryProofStatus, DeliveryRejectionReason, GptOssRuntimeCapability,
    PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID, PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID,
    canonical_compute_product_id,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::labor::{
    Contract, ContractStatus, SettlementLink, SettlementStatus, Submission, SubmissionStatus,
    Verdict, VerdictOutcome, WorkUnit, WorkUnitStatus,
};
use openagents_kernel_core::receipts::{
    Asset, AuthAssuranceLevel, EvidenceRef, FeedbackLatencyClass, Money, MoneyAmount,
    PolicyContext, ProvenanceGrade, Receipt, ReceiptHints, SeverityClass, TraceContext,
    VerificationTier,
};
use openagents_kernel_core::snapshots::EconomySnapshot;
use openagents_kernel_core::time::floor_to_minute_utc;
use openagents_provider_substrate::{ProviderAdvertisedProduct, ProviderComputeProduct};
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::watch;

const KERNEL_MUTATION_TIMEOUT: Duration = Duration::from_secs(5);
const KERNEL_STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const KERNEL_STREAM_RETRY_DELAY: Duration = Duration::from_secs(2);
const LAUNCH_PRODUCT_CREATED_AT_MS: i64 = 1_762_000_000_000;
const ONLINE_INVENTORY_WINDOW_DURATION_MS: i64 = 86_400_000;
const ONLINE_INVENTORY_QUANTITY: u64 = 1_024;
const FORWARD_INVENTORY_START_DELAY_MS: i64 = 21_600_000;
const FORWARD_INVENTORY_WINDOW_DURATION_MS: i64 = 3_600_000;
const FORWARD_INVENTORY_QUANTITY: u64 = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum KernelAuthorityMode {
    Unavailable,
    Remote {
        base_url: String,
        bearer_auth: String,
    },
}

#[derive(Debug)]
pub(crate) enum KernelProjectionUpdate {
    Receipt(Receipt),
    Snapshot(EconomySnapshot),
    StreamError {
        stream: &'static str,
        message: String,
    },
}

pub(crate) struct KernelProjectionWorker {
    mode: KernelAuthorityMode,
    update_rx: Option<Receiver<KernelProjectionUpdate>>,
    shutdown_tx: Option<watch::Sender<bool>>,
    join_handle: Option<JoinHandle<()>>,
}

#[derive(Clone, Debug, Default)]
struct PendingSseEvent {
    event_name: Option<String>,
    data_lines: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ReceiptProjectionEnvelope {
    receipt: Receipt,
}

#[derive(Debug, Deserialize, Serialize)]
struct SnapshotProjectionEnvelope {
    snapshot: EconomySnapshot,
}

#[derive(Clone, Debug, Default)]
struct LaunchDeliveryContext {
    execution_output: Option<String>,
    provider_thread_id: Option<String>,
    provider_turn_id: Option<String>,
    gpt_oss_ready_model: Option<String>,
    gpt_oss_metrics: Option<LocalInferenceExecutionMetrics>,
    apple_ready_model: Option<String>,
    apple_metrics: Option<LocalInferenceExecutionMetrics>,
    apple_model_available: bool,
    apple_bridge_status: Option<String>,
}

#[derive(Clone, Debug)]
struct DeliveryProofEvaluation {
    metering_rule_id: &'static str,
    settlement_class: &'static str,
    metered_quantity: u64,
    accepted_quantity: u64,
    status: DeliveryProofStatus,
    variance_reason: Option<ComputeDeliveryVarianceReason>,
    variance_reason_detail: Option<String>,
    rejection_reason: Option<DeliveryRejectionReason>,
    promised_capability_envelope: ComputeCapabilityEnvelope,
    observed_capability_envelope: Option<ComputeCapabilityEnvelope>,
}

impl Default for KernelProjectionWorker {
    fn default() -> Self {
        Self {
            mode: KernelAuthorityMode::Unavailable,
            update_rx: None,
            shutdown_tx: None,
            join_handle: None,
        }
    }
}

impl KernelProjectionWorker {
    pub fn reconfigure(&mut self, mode: KernelAuthorityMode) {
        if self.mode == mode {
            return;
        }
        self.shutdown_async();
        self.mode = mode.clone();
        if let KernelAuthorityMode::Remote {
            base_url,
            bearer_auth,
        } = mode
        {
            let (update_rx, shutdown_tx, join_handle) =
                spawn_remote_projection_runtime(base_url, bearer_auth);
            self.update_rx = Some(update_rx);
            self.shutdown_tx = Some(shutdown_tx);
            self.join_handle = Some(join_handle);
        }
    }

    pub fn uses_remote_authority(&self) -> bool {
        matches!(self.mode, KernelAuthorityMode::Remote { .. })
    }

    pub fn drain_updates(&mut self) -> Vec<KernelProjectionUpdate> {
        let mut updates = Vec::new();
        let Some(update_rx) = self.update_rx.as_ref() else {
            return updates;
        };
        while let Ok(update) = update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn shutdown_async(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(true);
        }
        self.update_rx = None;
        if let Some(join_handle) = self.join_handle.take() {
            std::thread::spawn(move || {
                let _ = join_handle.join();
            });
        }
    }
}

impl Drop for KernelProjectionWorker {
    fn drop(&mut self) {
        self.shutdown_async();
    }
}

pub(crate) fn resolve_kernel_authority_mode(
    control_base_url: Option<&str>,
    bearer_auth: Option<&str>,
) -> KernelAuthorityMode {
    let Some(base_url) = control_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return KernelAuthorityMode::Unavailable;
    };
    let Some(bearer_auth) = bearer_auth
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return KernelAuthorityMode::Unavailable;
    };
    KernelAuthorityMode::Remote {
        base_url,
        bearer_auth,
    }
}

pub(crate) fn sync_kernel_authority_mode(state: &mut RenderState) {
    let mode = resolve_kernel_authority_mode(
        state.hosted_control_base_url.as_deref(),
        state.hosted_control_bearer_token.as_deref(),
    );
    state.kernel_projection_worker.reconfigure(mode);
}

pub(crate) fn drain_kernel_projection_updates(state: &mut RenderState) -> bool {
    let mut changed = false;
    for update in state.kernel_projection_worker.drain_updates() {
        match update {
            KernelProjectionUpdate::Receipt(receipt) => {
                state.sync_bootstrap_error = None;
                state.sync_health.last_error = None;
                state.sync_health.last_action =
                    Some("Kernel receipt projection connected".to_string());
                state
                    .earn_kernel_receipts
                    .apply_authoritative_receipt(receipt, "kernel.projection.receipt");
                changed = true;
            }
            KernelProjectionUpdate::Snapshot(snapshot) => {
                state.sync_bootstrap_error = None;
                state.sync_health.last_error = None;
                state.sync_health.last_action =
                    Some("Kernel snapshot projection connected".to_string());
                if state
                    .economy_snapshot
                    .apply_authoritative_snapshot(snapshot, "kernel.projection.snapshot")
                {
                    changed = true;
                }
            }
            KernelProjectionUpdate::StreamError { stream, message } => {
                let formatted = format!("kernel {stream} stream: {message}");
                state.sync_bootstrap_error = Some(formatted.clone());
                state.sync_health.last_error = Some(formatted.clone());
                state.sync_health.last_action =
                    Some(format!("Kernel {stream} projection retry pending"));
                changed = true;
            }
        }
    }
    changed
}

pub(crate) fn should_compute_local_snapshots(state: &RenderState) -> bool {
    let _ = state;
    false
}

pub(crate) fn kernel_authority_available(state: &RenderState) -> bool {
    !matches!(
        current_authority_mode(state),
        KernelAuthorityMode::Unavailable
    )
}

pub(crate) fn register_accepted_request_with_kernel(
    state: &mut RenderState,
    request: &JobInboxRequest,
) -> Result<(), String> {
    let client = match current_authority_mode(state) {
        KernelAuthorityMode::Unavailable => {
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Kernel authority unavailable; accepting request_id={} capability={} via local-only relay flow",
                request.request_id,
                request.capability
            );
            return Ok(());
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => build_remote_authority_client(base_url, bearer_auth)?,
    };
    let binding = selected_launch_compute_binding_for_request(state, request).ok_or_else(|| {
        format!(
            "unsupported compute capability for canonicalization: {}",
            request.capability
        )
    })?;
    ensure_inventory_session_started(state);
    ensure_launch_compute_product_registered(state, &client, binding)?;
    ensure_online_capacity_lot_registered(state, &client, binding)?;

    let capacity_instrument_request = build_capacity_instrument_request(state, request)?;
    let capacity_instrument_receipt =
        run_kernel_call(client.create_capacity_instrument(capacity_instrument_request))?.receipt;
    state.earn_kernel_receipts.apply_authoritative_receipt(
        capacity_instrument_receipt,
        "kernel.authority.capacity_instrument",
    );

    let work_unit_request = build_work_unit_request(state, request);
    let work_unit_receipt = run_kernel_call(client.create_work_unit(work_unit_request))?.receipt;
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(work_unit_receipt, "kernel.authority.work_unit");

    let contract_request = build_contract_request(state, request);
    let contract_receipt = run_kernel_call(client.create_contract(contract_request))?.receipt;
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(contract_receipt, "kernel.authority.contract");
    Ok(())
}

pub(crate) fn register_online_compute_inventory_with_kernel(
    state: &mut RenderState,
) -> Result<(), String> {
    let bindings = online_inventory_bindings_for_state(state);
    if bindings.is_empty() {
        return Ok(());
    }
    ensure_inventory_session_started(state);
    let client = match current_authority_mode(state) {
        KernelAuthorityMode::Unavailable => {
            tracing::debug!(
                target: "autopilot_desktop::provider",
                "Kernel authority unavailable; provider inventory remains local-only bindings={}",
                bindings.len()
            );
            return Ok(());
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => build_remote_authority_client(base_url, bearer_auth)?,
    };
    for binding in bindings {
        ensure_launch_compute_product_registered(state, &client, binding)?;
        ensure_online_capacity_lot_registered(state, &client, binding)?;
        ensure_forward_capacity_lot_registered(state, &client, binding)?;
    }
    Ok(())
}

pub(crate) fn refresh_provider_inventory_rows(state: &mut RenderState) -> bool {
    let mut changed = state.provider_runtime.refresh_sandbox_supply_if_due();
    let rows = build_provider_inventory_rows(state);
    if state.provider_runtime.inventory_rows != rows {
        state.provider_runtime.inventory_rows = rows;
        changed = true;
    }
    state.provider_runtime.inventory_last_error = None;
    state.provider_runtime.inventory_last_action = Some(format!(
        "Materialized {} provider inventory row{}",
        state.provider_runtime.inventory_rows.len(),
        if state.provider_runtime.inventory_rows.len() == 1 {
            ""
        } else {
            "s"
        }
    ));
    changed
}

pub(crate) fn submit_active_job_output(state: &mut RenderState) -> Result<String, String> {
    let Some(job) = state.active_job.job.as_ref().cloned() else {
        return Err("no active job selected".to_string());
    };
    let client = match current_authority_mode(state) {
        KernelAuthorityMode::Unavailable => {
            let receipt_id = local_projection_receipt_id("submission", job.request_id.as_str());
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Kernel authority unavailable; recorded local-only output submission request_id={} receipt_id={}",
                job.request_id,
                receipt_id
            );
            return Ok(receipt_id);
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => build_remote_authority_client(base_url, bearer_auth)?,
    };
    let submit_request = build_submit_output_request(state, &job);
    let receipt = run_kernel_call(client.submit_output(submit_request))?.receipt;
    let receipt_id = receipt.receipt_id.clone();
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(receipt, "kernel.authority.submission");
    Ok(receipt_id)
}

pub(crate) fn finalize_paid_active_job(state: &mut RenderState) -> Result<String, String> {
    let Some(job) = state.active_job.job.as_ref().cloned() else {
        return Err("no active job selected".to_string());
    };
    let client = match current_authority_mode(state) {
        KernelAuthorityMode::Unavailable => {
            let receipt_id = local_projection_receipt_id("verdict", job.request_id.as_str());
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Kernel authority unavailable; finalized local-only payout request_id={} receipt_id={}",
                job.request_id,
                receipt_id
            );
            return Ok(receipt_id);
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => build_remote_authority_client(base_url, bearer_auth)?,
    };

    let (delivery_proof_request, delivery_evaluation) = build_delivery_proof_request(state, &job)?;
    let delivery_proof_receipt =
        run_kernel_call(client.record_delivery_proof(delivery_proof_request))?.receipt;
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(delivery_proof_receipt, "kernel.authority.delivery_proof");
    if let Some(active_job) = state.active_job.job.as_mut() {
        active_job.delivery_metering_rule_id =
            Some(delivery_evaluation.metering_rule_id.to_string());
        active_job.delivery_proof_status_label =
            Some(delivery_evaluation.status.label().to_string());
        active_job.delivery_metered_quantity = Some(delivery_evaluation.metered_quantity);
        active_job.delivery_accepted_quantity = Some(delivery_evaluation.accepted_quantity);
        active_job.delivery_variance_reason_label = delivery_evaluation
            .variance_reason
            .map(|reason| reason.label().to_string());
        active_job.delivery_rejection_reason_label = delivery_evaluation
            .rejection_reason
            .map(|reason| reason.label().to_string());
    }

    let verdict_request = build_finalize_verdict_request(state, &job);
    let receipt = run_kernel_call(client.finalize_verdict(verdict_request))?.receipt;
    let receipt_id = receipt.receipt_id.clone();
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(receipt, "kernel.authority.verdict");
    Ok(receipt_id)
}

pub(crate) fn attach_compute_linkage_to_active_job(
    state: &mut RenderState,
    request: &JobInboxRequest,
) {
    let Some(linkage) = compute_linkage_for_request(
        state,
        request.request_id.as_str(),
        request.capability.as_str(),
    ) else {
        return;
    };
    let Some(job) = state.active_job.job.as_mut() else {
        return;
    };
    if job.request_id != request.request_id {
        return;
    }
    job.compute_product_id = Some(linkage.product_id.to_string());
    job.capacity_lot_id = Some(linkage.capacity_lot_id);
    job.capacity_instrument_id = Some(linkage.capacity_instrument_id);
    job.delivery_proof_id = Some(linkage.delivery_proof_id);
}

fn build_provider_inventory_rows(state: &RenderState) -> Vec<ProviderInventoryRow> {
    state
        .provider_runtime
        .derived_inventory_products()
        .into_iter()
        .map(|product| build_provider_inventory_row(state, product))
        .collect()
}

fn build_provider_inventory_row(
    state: &RenderState,
    derived_product: ProviderAdvertisedProduct,
) -> ProviderInventoryRow {
    let target = derived_product.product;
    let enabled = derived_product.enabled;
    let backend_ready = derived_product.backend_ready;
    let eligible = derived_product.eligible;
    let total_quantity = if eligible
        && state
            .provider_runtime
            .inventory_session_started_at_ms
            .is_some()
        && matches!(
            state.provider_runtime.mode,
            crate::app_state::ProviderMode::Online | crate::app_state::ProviderMode::Degraded
        ) {
        ONLINE_INVENTORY_QUANTITY
    } else {
        0
    };
    let reserved_quantity = if provider_inventory_active_job_matches(state, target)
        && state
            .active_job
            .job
            .as_ref()
            .is_some_and(|job| !job.stage.is_terminal())
    {
        1
    } else {
        0
    };
    let available_quantity = total_quantity.saturating_sub(reserved_quantity);
    let capacity_lot_id = state
        .provider_runtime
        .inventory_session_started_at_ms
        .filter(|_| eligible)
        .map(|session_started_at_ms| {
            online_capacity_lot_id_for_binding(
                provider_id_for_state(state).as_str(),
                target.product_id(),
                session_started_at_ms,
            )
        });
    let forward_capacity_lot_id = state
        .provider_runtime
        .inventory_session_started_at_ms
        .filter(|_| eligible)
        .map(|session_started_at_ms| {
            forward_capacity_lot_id_for_binding(
                provider_id_for_state(state).as_str(),
                target.product_id(),
                session_started_at_ms,
            )
        });
    ProviderInventoryRow {
        target,
        enabled,
        backend_ready,
        eligible,
        capability_summary: derived_product.capability_summary,
        source_badge: provider_inventory_source_badge(state, target, eligible).to_string(),
        capacity_lot_id,
        total_quantity,
        reserved_quantity,
        available_quantity,
        delivery_state: provider_inventory_delivery_state(state, target).to_string(),
        price_floor_sats: derived_product.price_floor_sats,
        terms_label: derived_product.terms_label,
        forward_capacity_lot_id,
        forward_delivery_window_label: state
            .provider_runtime
            .inventory_session_started_at_ms
            .filter(|_| eligible)
            .map(|session_started_at_ms| {
                format!(
                    "{}..{}",
                    session_started_at_ms.saturating_add(FORWARD_INVENTORY_START_DELAY_MS),
                    session_started_at_ms
                        .saturating_add(FORWARD_INVENTORY_START_DELAY_MS)
                        .saturating_add(FORWARD_INVENTORY_WINDOW_DURATION_MS)
                )
            }),
        forward_total_quantity: if eligible
            && state
                .provider_runtime
                .inventory_session_started_at_ms
                .is_some()
            && matches!(
                state.provider_runtime.mode,
                crate::app_state::ProviderMode::Online | crate::app_state::ProviderMode::Degraded
            ) {
            FORWARD_INVENTORY_QUANTITY
        } else {
            0
        },
        forward_reserved_quantity: 0,
        forward_available_quantity: if eligible
            && state
                .provider_runtime
                .inventory_session_started_at_ms
                .is_some()
            && matches!(
                state.provider_runtime.mode,
                crate::app_state::ProviderMode::Online | crate::app_state::ProviderMode::Degraded
            ) {
            FORWARD_INVENTORY_QUANTITY
        } else {
            0
        },
        forward_terms_label: eligible.then_some(derived_product.forward_terms_label),
    }
}

fn current_authority_mode(state: &RenderState) -> KernelAuthorityMode {
    resolve_kernel_authority_mode(
        state.hosted_control_base_url.as_deref(),
        state.hosted_control_bearer_token.as_deref(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LaunchComputeBinding {
    product_id: &'static str,
    backend_family: ComputeBackendFamily,
    compute_family: ComputeFamily,
    model_policy: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LaunchComputeLinkage {
    product_id: &'static str,
    capacity_lot_id: String,
    capacity_instrument_id: String,
    delivery_proof_id: String,
    compute_family: ComputeFamily,
}

fn remote_authority_client_for_state(
    state: &RenderState,
) -> Result<HttpKernelAuthorityClient, String> {
    match current_authority_mode(state) {
        KernelAuthorityMode::Unavailable => Err(
            "kernel authority unavailable: hosted control endpoint is not configured".to_string(),
        ),
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => build_remote_authority_client(base_url, bearer_auth),
    }
}

fn build_remote_authority_client(
    base_url: &str,
    bearer_auth: &str,
) -> Result<HttpKernelAuthorityClient, String> {
    let client = reqwest::Client::builder()
        .timeout(KERNEL_MUTATION_TIMEOUT)
        .build()
        .map_err(|error| format!("kernel authority client initialization failed: {error}"))?;
    Ok(HttpKernelAuthorityClient::with_client(
        client,
        base_url.to_string(),
        Some(bearer_auth.to_string()),
    ))
}

fn run_kernel_call<F, T>(future: F) -> Result<T, String>
where
    F: Future<Output = anyhow::Result<T>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("kernel authority runtime initialization failed: {error}"))?;
    runtime
        .block_on(future)
        .map_err(|error| format!("kernel authority call failed: {error}"))
}

pub(crate) fn request_spot_compute_quotes(
    state: &RenderState,
    rfq: &SpotComputeRfqDraft,
) -> Result<Vec<SpotComputeQuoteCandidate>, String> {
    let client = remote_authority_client_for_state(state)?;
    let products =
        run_kernel_call(client.list_compute_products(Some(ComputeProductStatus::Active)))?;
    let lots = run_kernel_call(client.list_capacity_lots(None, None))?;
    let instruments = run_kernel_call(client.list_capacity_instruments(None, None, None))?;
    Ok(build_spot_compute_quotes_from_market(
        rfq,
        products.as_slice(),
        lots.as_slice(),
        instruments.as_slice(),
    ))
}

pub(crate) fn accept_spot_compute_quote(
    state: &mut RenderState,
    rfq: &SpotComputeRfqDraft,
    quote: &SpotComputeQuoteCandidate,
) -> Result<AcceptedSpotComputeOrder, String> {
    let client = remote_authority_client_for_state(state)?;
    let created_at_ms = current_epoch_ms();
    let delivery_window_ms = (rfq.window_minutes as i64)
        .saturating_mul(60_000)
        .max(60_000);
    let delivery_start_ms = created_at_ms;
    let delivery_end_ms = created_at_ms.saturating_add(delivery_window_ms);
    let request = CreateCapacityInstrumentRequest {
        idempotency_key: format!(
            "desktop.buy.compute_spot:{}:{}",
            canonical_kernel_id_component(rfq.rfq_id.as_str()),
            canonical_kernel_id_component(quote.capacity_lot_id.as_str())
        ),
        trace: TraceContext {
            session_id: Some("desktop.compute.spot".to_string()),
            trajectory_hash: Some(format!(
                "traj:{}:{}",
                canonical_kernel_id_component(rfq.rfq_id.as_str()),
                canonical_kernel_id_component(quote.quote_id.as_str())
            )),
            job_hash: Some(rfq.rfq_id.clone()),
            run_id: Some(format!(
                "spot:{}",
                canonical_kernel_id_component(quote.quote_id.as_str())
            )),
            work_unit_id: None,
            contract_id: None,
            claim_id: None,
        },
        policy: kernel_policy_context(state),
        instrument: CapacityInstrument {
            instrument_id: format!(
                "instrument.buy.{}.{}",
                canonical_kernel_id_component(rfq.rfq_id.as_str()),
                canonical_kernel_id_component(quote.capacity_lot_id.as_str())
            ),
            product_id: quote.product_id.clone(),
            capacity_lot_id: Some(quote.capacity_lot_id.clone()),
            buyer_id: Some(provider_id_for_state(state)),
            provider_id: Some(quote.provider_id.clone()),
            delivery_start_ms,
            delivery_end_ms,
            quantity: quote.requested_quantity,
            fixed_price: Some(btc_sats_money(quote.price_sats)),
            reference_index_id: None,
            kind: CapacityInstrumentKind::Spot,
            settlement_mode: ComputeSettlementMode::Physical,
            created_at_ms,
            status: CapacityInstrumentStatus::Active,
            environment_binding: quote.environment_binding.clone(),
            closure_reason: None,
            non_delivery_reason: None,
            settlement_failure_reason: None,
            lifecycle_reason_detail: None,
            metadata: json!({
                "rfq_id": rfq.rfq_id,
                "quote_id": quote.quote_id,
                "compute_family": quote.compute_family_label(),
                "backend_family": quote.backend_label(),
                "execution_kind": quote.execution_label(),
                "topology_kind": quote.topology_label(),
                "provisioning_kind": quote.provisioning_label(),
                "proof_posture": quote.proof_posture_label(),
                "environment_ref": quote.environment_ref(),
                "sandbox_profile_ref": quote.sandbox_profile_ref.as_deref(),
                "source_badge": quote.source_badge,
                "terms_label": quote.terms_label,
            }),
        },
        evidence: vec![
            evidence_ref(
                "spot_rfq_ref",
                format!("oa://autopilot/compute/rfq/{}", rfq.rfq_id),
                rfq.summary().as_str(),
            ),
            evidence_ref(
                "capacity_lot_ref",
                format!("oa://kernel/compute/lots/{}", quote.capacity_lot_id),
                quote.capacity_lot_id.as_str(),
            ),
        ],
        hints: receipt_hints_for_notional(quote.price_sats),
    };
    let response = run_kernel_call(client.create_capacity_instrument(request))?;
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(response.receipt, "kernel.authority.capacity_instrument.buy");
    let instrument_id = response.instrument.instrument_id.clone();
    let accepted_at_epoch_seconds = (created_at_ms / 1_000).max(0) as u64;
    Ok(AcceptedSpotComputeOrder {
        order_id: format!(
            "spot-order-{}",
            canonical_kernel_id_component(quote.quote_id.as_str())
        ),
        rfq_id: rfq.rfq_id.clone(),
        quote_id: quote.quote_id.clone(),
        instrument_id,
        product_id: quote.product_id.clone(),
        capacity_lot_id: quote.capacity_lot_id.clone(),
        provider_id: quote.provider_id.clone(),
        backend_family: quote.backend_family,
        compute_family: quote.compute_family,
        execution_kind: quote.execution_kind,
        topology_kind: quote.topology_kind,
        provisioning_kind: quote.provisioning_kind,
        proof_posture: quote.proof_posture,
        environment_binding: quote.environment_binding.clone(),
        sandbox_profile_ref: quote.sandbox_profile_ref.clone(),
        quantity: quote.requested_quantity,
        price_sats: quote.price_sats,
        delivery_window_label: quote.delivery_window_label.clone(),
        authority_status: "spot-accepted".to_string(),
        accepted_at_epoch_seconds,
    })
}

pub(crate) fn request_forward_compute_quotes(
    state: &RenderState,
    rfq: &ForwardComputeRfqDraft,
) -> Result<Vec<ForwardComputeQuoteCandidate>, String> {
    let client = remote_authority_client_for_state(state)?;
    let products =
        run_kernel_call(client.list_compute_products(Some(ComputeProductStatus::Active)))?;
    let lots = run_kernel_call(client.list_capacity_lots(None, None))?;
    let instruments = run_kernel_call(client.list_capacity_instruments(None, None, None))?;
    Ok(build_forward_compute_quotes_from_market(
        rfq,
        products.as_slice(),
        lots.as_slice(),
        instruments.as_slice(),
    ))
}

pub(crate) fn accept_forward_compute_quote(
    state: &mut RenderState,
    rfq: &ForwardComputeRfqDraft,
    quote: &ForwardComputeQuoteCandidate,
) -> Result<AcceptedForwardComputeOrder, String> {
    let client = remote_authority_client_for_state(state)?;
    let created_at_ms = current_epoch_ms();
    let request = CreateCapacityInstrumentRequest {
        idempotency_key: format!(
            "desktop.buy.compute_forward:{}:{}",
            canonical_kernel_id_component(rfq.rfq_id.as_str()),
            canonical_kernel_id_component(quote.capacity_lot_id.as_str())
        ),
        trace: TraceContext {
            session_id: Some("desktop.compute.forward".to_string()),
            trajectory_hash: Some(format!(
                "traj:{}:{}",
                canonical_kernel_id_component(rfq.rfq_id.as_str()),
                canonical_kernel_id_component(quote.quote_id.as_str())
            )),
            job_hash: Some(rfq.rfq_id.clone()),
            run_id: Some(format!(
                "forward:{}",
                canonical_kernel_id_component(quote.quote_id.as_str())
            )),
            work_unit_id: None,
            contract_id: None,
            claim_id: None,
        },
        policy: kernel_policy_context(state),
        instrument: CapacityInstrument {
            instrument_id: format!(
                "instrument.forward.{}.{}",
                canonical_kernel_id_component(rfq.rfq_id.as_str()),
                canonical_kernel_id_component(quote.capacity_lot_id.as_str())
            ),
            product_id: quote.product_id.clone(),
            capacity_lot_id: Some(quote.capacity_lot_id.clone()),
            buyer_id: Some(provider_id_for_state(state)),
            provider_id: Some(quote.provider_id.clone()),
            delivery_start_ms: quote.delivery_start_ms,
            delivery_end_ms: quote.delivery_end_ms,
            quantity: quote.requested_quantity,
            fixed_price: Some(btc_sats_money(quote.price_sats)),
            reference_index_id: None,
            kind: CapacityInstrumentKind::ForwardPhysical,
            settlement_mode: ComputeSettlementMode::Physical,
            created_at_ms,
            status: CapacityInstrumentStatus::Active,
            environment_binding: quote.environment_binding.clone(),
            closure_reason: None,
            non_delivery_reason: None,
            settlement_failure_reason: None,
            lifecycle_reason_detail: None,
            metadata: json!({
                "rfq_id": rfq.rfq_id,
                "quote_id": quote.quote_id,
                "compute_family": quote.compute_family_label(),
                "backend_family": quote.backend_label(),
                "execution_kind": quote.execution_label(),
                "topology_kind": quote.topology_label(),
                "provisioning_kind": quote.provisioning_label(),
                "proof_posture": quote.proof_posture_label(),
                "environment_ref": quote.environment_ref(),
                "sandbox_profile_ref": quote.sandbox_profile_ref.as_deref(),
                "source_badge": quote.source_badge,
                "terms_label": quote.terms_label,
                "collateral_summary": quote.collateral_summary,
                "remedy_summary": quote.remedy_summary,
                "delivery_start_minutes": rfq.delivery_start_minutes,
                "market_phase": "forward_physical",
            }),
        },
        evidence: vec![
            evidence_ref(
                "forward_rfq_ref",
                format!("oa://autopilot/compute/forward-rfq/{}", rfq.rfq_id),
                rfq.summary().as_str(),
            ),
            evidence_ref(
                "capacity_lot_ref",
                format!("oa://kernel/compute/lots/{}", quote.capacity_lot_id),
                quote.capacity_lot_id.as_str(),
            ),
        ],
        hints: receipt_hints_for_notional(quote.price_sats),
    };
    let response = run_kernel_call(client.create_capacity_instrument(request))?;
    state.earn_kernel_receipts.apply_authoritative_receipt(
        response.receipt,
        "kernel.authority.capacity_instrument.forward",
    );
    let instrument_id = response.instrument.instrument_id.clone();
    let accepted_at_epoch_seconds = (created_at_ms / 1_000).max(0) as u64;
    Ok(AcceptedForwardComputeOrder {
        order_id: format!(
            "forward-order-{}",
            canonical_kernel_id_component(quote.quote_id.as_str())
        ),
        rfq_id: rfq.rfq_id.clone(),
        quote_id: quote.quote_id.clone(),
        instrument_id,
        product_id: quote.product_id.clone(),
        capacity_lot_id: quote.capacity_lot_id.clone(),
        provider_id: quote.provider_id.clone(),
        backend_family: quote.backend_family,
        compute_family: quote.compute_family,
        execution_kind: quote.execution_kind,
        topology_kind: quote.topology_kind,
        provisioning_kind: quote.provisioning_kind,
        proof_posture: quote.proof_posture,
        environment_binding: quote.environment_binding.clone(),
        sandbox_profile_ref: quote.sandbox_profile_ref.clone(),
        quantity: quote.requested_quantity,
        price_sats: quote.price_sats,
        delivery_start_ms: quote.delivery_start_ms,
        delivery_end_ms: quote.delivery_end_ms,
        delivery_window_label: quote.delivery_window_label.clone(),
        collateral_summary: quote.collateral_summary.clone(),
        remedy_summary: quote.remedy_summary.clone(),
        authority_status: "forward-accepted".to_string(),
        accepted_at_epoch_seconds,
    })
}

fn build_spot_compute_quotes_from_market(
    rfq: &SpotComputeRfqDraft,
    products: &[ComputeProduct],
    lots: &[CapacityLot],
    instruments: &[CapacityInstrument],
) -> Vec<SpotComputeQuoteCandidate> {
    let mut quotes = Vec::new();
    for product in products {
        let Some(envelope) = product.capability_envelope.as_ref() else {
            continue;
        };
        let Some(quoteable) = quoteable_compute_product(product) else {
            continue;
        };
        if !spot_rfq_matches_envelope(
            rfq,
            envelope,
            quoteable.compute_family,
            quoteable.backend_family,
        ) {
            continue;
        }
        for lot in lots
            .iter()
            .filter(|lot| lot.product_id == product.product_id)
        {
            if !spot_lot_is_quotable(rfq, lot) {
                continue;
            }
            let environment_binding = effective_environment_binding(lot, envelope);
            let sandbox_profile_ref = sandbox_profile_ref_for_quote(product, lot);
            if !spot_rfq_matches_quote_posture(
                &rfq.capability_constraints,
                &quoteable,
                environment_binding.as_ref(),
                sandbox_profile_ref.as_deref(),
            ) {
                continue;
            }
            let reserved_quantity =
                reserved_quantity_for_lot(instruments, lot.capacity_lot_id.as_str());
            let available_quantity = lot.quantity.saturating_sub(reserved_quantity);
            if available_quantity < rfq.quantity {
                continue;
            }
            let price_sats = lot
                .min_unit_price
                .as_ref()
                .and_then(money_as_sats)
                .unwrap_or_else(|| price_floor_sats_for_product_id(product.product_id.as_str()))
                .saturating_mul(rfq.quantity);
            if price_sats > rfq.max_price_sats {
                continue;
            }
            quotes.push(SpotComputeQuoteCandidate {
                quote_id: format!(
                    "quote.{}.{}",
                    canonical_kernel_id_component(rfq.rfq_id.as_str()),
                    canonical_kernel_id_component(lot.capacity_lot_id.as_str())
                ),
                rfq_id: rfq.rfq_id.clone(),
                product_id: product.product_id.clone(),
                capacity_lot_id: lot.capacity_lot_id.clone(),
                provider_id: lot.provider_id.clone(),
                backend_family: quoteable.backend_family,
                compute_family: quoteable.compute_family,
                execution_kind: Some(quoteable.execution_kind),
                topology_kind: Some(quoteable.topology_kind),
                provisioning_kind: Some(quoteable.provisioning_kind),
                proof_posture: Some(quoteable.proof_posture),
                environment_binding,
                sandbox_profile_ref,
                available_quantity,
                requested_quantity: rfq.quantity,
                price_sats,
                delivery_window_label: format!(
                    "{}m inside lot {}..{}",
                    rfq.window_minutes, lot.delivery_start_ms, lot.delivery_end_ms
                ),
                capability_summary: quoteable.capability_summary.clone(),
                source_badge: quote_source_badge(product, lot).to_string(),
                terms_label: quote_terms_label(product, lot).to_string(),
            });
        }
    }
    quotes.sort_by(|left, right| {
        left.price_sats
            .cmp(&right.price_sats)
            .then_with(|| left.product_id.cmp(&right.product_id))
            .then_with(|| left.capacity_lot_id.cmp(&right.capacity_lot_id))
    });
    quotes
}

fn build_forward_compute_quotes_from_market(
    rfq: &ForwardComputeRfqDraft,
    products: &[ComputeProduct],
    lots: &[CapacityLot],
    instruments: &[CapacityInstrument],
) -> Vec<ForwardComputeQuoteCandidate> {
    let mut quotes = Vec::new();
    let desired_start_ms = current_epoch_ms()
        .saturating_add((rfq.delivery_start_minutes as i64).saturating_mul(60_000));
    let desired_end_ms = desired_start_ms.saturating_add(
        (rfq.window_minutes as i64)
            .saturating_mul(60_000)
            .max(60_000),
    );
    for product in products {
        let Some(envelope) = product.capability_envelope.as_ref() else {
            continue;
        };
        let Some(quoteable) = quoteable_compute_product(product) else {
            continue;
        };
        if !spot_rfq_matches_envelope(
            &SpotComputeRfqDraft {
                rfq_id: rfq.rfq_id.clone(),
                compute_family: rfq.compute_family,
                preferred_backend: rfq.preferred_backend,
                quantity: rfq.quantity,
                window_minutes: rfq.window_minutes,
                max_price_sats: rfq.max_price_sats,
                capability_constraints: rfq.capability_constraints.clone(),
            },
            envelope,
            quoteable.compute_family,
            quoteable.backend_family,
        ) {
            continue;
        }
        for lot in lots
            .iter()
            .filter(|lot| lot.product_id == product.product_id)
        {
            if lot.delivery_start_ms <= current_epoch_ms() {
                continue;
            }
            if lot.delivery_start_ms > desired_start_ms || lot.delivery_end_ms < desired_end_ms {
                continue;
            }
            let environment_binding = effective_environment_binding(lot, envelope);
            let sandbox_profile_ref = sandbox_profile_ref_for_quote(product, lot);
            if !spot_rfq_matches_quote_posture(
                &rfq.capability_constraints,
                &quoteable,
                environment_binding.as_ref(),
                sandbox_profile_ref.as_deref(),
            ) {
                continue;
            }
            let reserved_quantity =
                reserved_quantity_for_lot(instruments, lot.capacity_lot_id.as_str());
            let available_quantity = lot.quantity.saturating_sub(reserved_quantity);
            if available_quantity < rfq.quantity {
                continue;
            }
            let price_sats = lot
                .min_unit_price
                .as_ref()
                .and_then(money_as_sats)
                .unwrap_or_else(|| {
                    price_floor_sats_for_product_id(product.product_id.as_str()).saturating_mul(2)
                })
                .saturating_mul(rfq.quantity);
            if price_sats > rfq.max_price_sats {
                continue;
            }
            quotes.push(ForwardComputeQuoteCandidate {
                quote_id: format!(
                    "forward-quote.{}.{}",
                    canonical_kernel_id_component(rfq.rfq_id.as_str()),
                    canonical_kernel_id_component(lot.capacity_lot_id.as_str())
                ),
                rfq_id: rfq.rfq_id.clone(),
                product_id: product.product_id.clone(),
                capacity_lot_id: lot.capacity_lot_id.clone(),
                provider_id: lot.provider_id.clone(),
                backend_family: quoteable.backend_family,
                compute_family: quoteable.compute_family,
                execution_kind: Some(quoteable.execution_kind),
                topology_kind: Some(quoteable.topology_kind),
                provisioning_kind: Some(quoteable.provisioning_kind),
                proof_posture: Some(quoteable.proof_posture),
                environment_binding,
                sandbox_profile_ref,
                available_quantity,
                requested_quantity: rfq.quantity,
                price_sats,
                delivery_start_ms: lot.delivery_start_ms,
                delivery_end_ms: lot.delivery_end_ms,
                delivery_window_label: format!(
                    "start+{}m / {}..{}",
                    rfq.delivery_start_minutes, lot.delivery_start_ms, lot.delivery_end_ms
                ),
                capability_summary: quoteable.capability_summary.clone(),
                source_badge: "desktop.forward_inventory".to_string(),
                terms_label: forward_terms_label_for_product_id(product.product_id.as_str())
                    .to_string(),
                collateral_summary: "bond=performance_bond".to_string(),
                remedy_summary: forward_remedy_profile_for_product_id(product.product_id.as_str())
                    .to_string(),
            });
        }
    }
    quotes.sort_by(|left, right| {
        left.price_sats
            .cmp(&right.price_sats)
            .then_with(|| left.delivery_start_ms.cmp(&right.delivery_start_ms))
            .then_with(|| left.product_id.cmp(&right.product_id))
            .then_with(|| left.capacity_lot_id.cmp(&right.capacity_lot_id))
    });
    quotes
}

fn online_inventory_bindings_for_state(state: &RenderState) -> Vec<LaunchComputeBinding> {
    let mut bindings = Vec::new();
    if state.provider_runtime.apple_fm.is_ready()
        && state.provider_runtime.product_enabled(
            ProviderInventoryProductToggleTarget::AppleFoundationModelsInference.product_id(),
        )
        && let Some(binding) = compute_binding_for_backend_and_capability(
            LocalInferenceBackend::AppleFoundationModels,
            "text_generation",
        )
    {
        bindings.push(binding);
    }
    if state.provider_runtime.gpt_oss.is_ready()
        && state
            .provider_runtime
            .product_enabled(ProviderInventoryProductToggleTarget::GptOssInference.product_id())
        && let Some(binding) = compute_binding_for_backend_and_capability(
            LocalInferenceBackend::GptOss,
            "text_generation",
        )
    {
        bindings.push(binding);
    }
    bindings
}

fn ensure_inventory_session_started(state: &mut RenderState) -> i64 {
    if let Some(session_started_at_ms) = state.provider_runtime.inventory_session_started_at_ms {
        return session_started_at_ms;
    }
    let session_started_at_ms = current_epoch_ms();
    state.provider_runtime.inventory_session_started_at_ms = Some(session_started_at_ms);
    session_started_at_ms
}

fn ensure_launch_compute_product_registered(
    state: &mut RenderState,
    client: &HttpKernelAuthorityClient,
    binding: LaunchComputeBinding,
) -> Result<(), String> {
    let compute_product_request = build_compute_product_request(binding);
    let compute_product_receipt =
        run_kernel_call(client.create_compute_product(compute_product_request))?.receipt;
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(compute_product_receipt, "kernel.authority.compute_product");
    Ok(())
}

fn ensure_online_capacity_lot_registered(
    state: &mut RenderState,
    client: &HttpKernelAuthorityClient,
    binding: LaunchComputeBinding,
) -> Result<(), String> {
    let capacity_lot_request = build_online_capacity_lot_request(state, binding)?;
    let capacity_lot_receipt =
        run_kernel_call(client.create_capacity_lot(capacity_lot_request))?.receipt;
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(capacity_lot_receipt, "kernel.authority.capacity_lot");
    Ok(())
}

fn ensure_forward_capacity_lot_registered(
    state: &mut RenderState,
    client: &HttpKernelAuthorityClient,
    binding: LaunchComputeBinding,
) -> Result<(), String> {
    let capacity_lot_request = build_forward_capacity_lot_request(state, binding)?;
    let capacity_lot_receipt =
        run_kernel_call(client.create_capacity_lot(capacity_lot_request))?.receipt;
    state.earn_kernel_receipts.apply_authoritative_receipt(
        capacity_lot_receipt,
        "kernel.authority.capacity_lot.forward",
    );
    Ok(())
}

fn spawn_remote_projection_runtime(
    base_url: String,
    bearer_auth: String,
) -> (
    Receiver<KernelProjectionUpdate>,
    watch::Sender<bool>,
    JoinHandle<()>,
) {
    let (update_tx, update_rx) = mpsc::channel::<KernelProjectionUpdate>();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let join_handle = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                    stream: "runtime",
                    message: format!("kernel projection runtime initialization failed: {error}"),
                });
                return;
            }
        };

        runtime.block_on(async move {
            let http_client = match reqwest::Client::builder()
                .connect_timeout(KERNEL_STREAM_CONNECT_TIMEOUT)
                .build()
            {
                Ok(client) => client,
                Err(error) => {
                    let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                        stream: "runtime",
                        message: format!("kernel projection client initialization failed: {error}"),
                    });
                    return;
                }
            };
            let snapshot_client = HttpKernelAuthorityClient::with_client(
                http_client.clone(),
                base_url.clone(),
                Some(bearer_auth.clone()),
            );
            let current_minute_start_ms = floor_to_minute_utc(current_epoch_ms());
            match snapshot_client.get_snapshot(current_minute_start_ms).await {
                Ok(snapshot) => {
                    let _ = update_tx.send(KernelProjectionUpdate::Snapshot(snapshot));
                }
                Err(error) => {
                    let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                        stream: "snapshots",
                        message: error.to_string(),
                    });
                }
            }

            let receipt_url =
                match canonical_kernel_endpoint(base_url.as_str(), "/v1/kernel/stream/receipts") {
                    Ok(url) => url,
                    Err(error) => {
                        let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                            stream: "receipts",
                            message: error.to_string(),
                        });
                        return;
                    }
                };
            let snapshot_url =
                match canonical_kernel_endpoint(base_url.as_str(), "/v1/kernel/stream/snapshots") {
                    Ok(url) => url,
                    Err(error) => {
                        let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                            stream: "snapshots",
                            message: error.to_string(),
                        });
                        return;
                    }
                };

            let receipt_task = tokio::spawn(run_projection_stream::<ReceiptProjectionEnvelope, _>(
                http_client.clone(),
                receipt_url,
                bearer_auth.clone(),
                "receipt",
                update_tx.clone(),
                shutdown_rx.clone(),
                |envelope| KernelProjectionUpdate::Receipt(envelope.receipt),
            ));
            let snapshot_task =
                tokio::spawn(run_projection_stream::<SnapshotProjectionEnvelope, _>(
                    http_client,
                    snapshot_url,
                    bearer_auth,
                    "snapshot",
                    update_tx,
                    shutdown_rx,
                    |envelope| KernelProjectionUpdate::Snapshot(envelope.snapshot),
                ));
            let _ = tokio::join!(receipt_task, snapshot_task);
        });
    });
    (update_rx, shutdown_tx, join_handle)
}

async fn run_projection_stream<T, F>(
    client: reqwest::Client,
    url: Url,
    bearer_auth: String,
    expected_event_name: &'static str,
    update_tx: Sender<KernelProjectionUpdate>,
    mut shutdown_rx: watch::Receiver<bool>,
    into_update: F,
) where
    T: DeserializeOwned,
    F: Fn(T) -> KernelProjectionUpdate + Send + Copy + 'static,
{
    loop {
        if *shutdown_rx.borrow() {
            return;
        }

        let request = client
            .get(url.clone())
            .bearer_auth(bearer_auth.as_str())
            .header(reqwest::header::ACCEPT, "text/event-stream");
        let response = tokio::select! {
            _ = wait_for_shutdown(&mut shutdown_rx) => return,
            result = request.send() => result,
        };

        let response = match response {
            Ok(response) if response.status().is_success() => response,
            Ok(response) => {
                let message = format!(
                    "status={} while opening {} projection stream",
                    response.status().as_u16(),
                    expected_event_name
                );
                let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                    stream: expected_event_name,
                    message,
                });
                if sleep_until_retry(&mut shutdown_rx).await {
                    return;
                }
                continue;
            }
            Err(error) => {
                let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                    stream: expected_event_name,
                    message: format!("projection connection failed: {error}"),
                });
                if sleep_until_retry(&mut shutdown_rx).await {
                    return;
                }
                continue;
            }
        };

        match consume_projection_stream(
            response,
            expected_event_name,
            &update_tx,
            &mut shutdown_rx,
            into_update,
        )
        .await
        {
            StreamLoopResult::Stopped => return,
            StreamLoopResult::Retry(message) => {
                let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                    stream: expected_event_name,
                    message,
                });
                if sleep_until_retry(&mut shutdown_rx).await {
                    return;
                }
            }
        }
    }
}

enum StreamLoopResult {
    Retry(String),
    Stopped,
}

async fn consume_projection_stream<T, F>(
    mut response: reqwest::Response,
    expected_event_name: &'static str,
    update_tx: &Sender<KernelProjectionUpdate>,
    shutdown_rx: &mut watch::Receiver<bool>,
    into_update: F,
) -> StreamLoopResult
where
    T: DeserializeOwned,
    F: Fn(T) -> KernelProjectionUpdate + Copy,
{
    let mut pending_buffer = String::new();
    let mut pending_event = PendingSseEvent::default();
    loop {
        let chunk = {
            let next_chunk = response.chunk();
            tokio::pin!(next_chunk);
            tokio::select! {
                _ = wait_for_shutdown(shutdown_rx) => return StreamLoopResult::Stopped,
                result = &mut next_chunk => result,
            }
        };

        match chunk {
            Ok(Some(bytes)) => {
                pending_buffer.push_str(&String::from_utf8_lossy(&bytes));
                if let Some(error) = consume_sse_buffer(
                    &mut pending_buffer,
                    &mut pending_event,
                    expected_event_name,
                    update_tx,
                    into_update,
                ) {
                    return StreamLoopResult::Retry(error);
                }
            }
            Ok(None) => {
                return StreamLoopResult::Retry(format!(
                    "{} projection stream closed",
                    expected_event_name
                ));
            }
            Err(error) => {
                return StreamLoopResult::Retry(format!(
                    "{} projection stream read failed: {error}",
                    expected_event_name
                ));
            }
        }
    }
}

fn consume_sse_buffer<T, F>(
    pending_buffer: &mut String,
    pending_event: &mut PendingSseEvent,
    expected_event_name: &'static str,
    update_tx: &Sender<KernelProjectionUpdate>,
    into_update: F,
) -> Option<String>
where
    T: DeserializeOwned,
    F: Fn(T) -> KernelProjectionUpdate + Copy,
{
    let mut consumed_up_to = 0usize;
    while let Some(relative_end) = pending_buffer[consumed_up_to..].find('\n') {
        let line_end = consumed_up_to + relative_end;
        let mut line = pending_buffer[consumed_up_to..line_end].to_string();
        if line.ends_with('\r') {
            line.pop();
        }
        consumed_up_to = line_end.saturating_add(1);

        if line.is_empty() {
            if let Some(error) =
                flush_pending_sse_event(pending_event, expected_event_name, update_tx, into_update)
            {
                return Some(error);
            }
            continue;
        }

        if line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            pending_event.event_name = Some(value.trim().to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("data:") {
            pending_event
                .data_lines
                .push(value.trim_start().to_string());
        }
    }

    pending_buffer.drain(..consumed_up_to);
    None
}

fn flush_pending_sse_event<T, F>(
    pending_event: &mut PendingSseEvent,
    expected_event_name: &'static str,
    update_tx: &Sender<KernelProjectionUpdate>,
    into_update: F,
) -> Option<String>
where
    T: DeserializeOwned,
    F: Fn(T) -> KernelProjectionUpdate + Copy,
{
    let event_name = pending_event.event_name.take();
    if pending_event.data_lines.is_empty() {
        pending_event.data_lines.clear();
        return None;
    }
    let payload = pending_event.data_lines.join("\n");
    pending_event.data_lines.clear();

    if let Some(event_name) = event_name.as_deref()
        && event_name != expected_event_name
    {
        return None;
    }

    let envelope = serde_json::from_str::<T>(payload.as_str()).map_err(|error| {
        format!(
            "invalid {} projection payload: {}",
            expected_event_name, error
        )
    });
    match envelope {
        Ok(envelope) => {
            let _ = update_tx.send(into_update(envelope));
            None
        }
        Err(error) => Some(error),
    }
}

async fn wait_for_shutdown(shutdown_rx: &mut watch::Receiver<bool>) {
    if *shutdown_rx.borrow() {
        return;
    }
    let _ = shutdown_rx.changed().await;
}

async fn sleep_until_retry(shutdown_rx: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = wait_for_shutdown(shutdown_rx) => true,
        _ = tokio::time::sleep(KERNEL_STREAM_RETRY_DELAY) => false,
    }
}

fn build_compute_product_request(binding: LaunchComputeBinding) -> CreateComputeProductRequest {
    let (apple_platform, gpt_oss_runtime, backend_family) = match binding.backend_family {
        ComputeBackendFamily::GptOss => (
            None,
            Some(GptOssRuntimeCapability {
                runtime_ready: None,
                model_name: None,
                quantization: None,
            }),
            "gpt_oss",
        ),
        ComputeBackendFamily::AppleFoundationModels => (
            Some(ApplePlatformCapability {
                apple_silicon_required: true,
                apple_intelligence_required: true,
                apple_intelligence_available: None,
                minimum_macos_version: Some("26.0".to_string()),
            }),
            None,
            "apple_foundation_models",
        ),
    };
    CreateComputeProductRequest {
        idempotency_key: format!("desktop.compute_product:{}", binding.product_id),
        trace: TraceContext {
            session_id: Some("kernel.compute.launch".to_string()),
            trajectory_hash: Some(format!("traj:{}", binding.product_id)),
            job_hash: Some(binding.product_id.to_string()),
            run_id: Some(format!(
                "launch:{}",
                canonical_kernel_id_component(binding.product_id)
            )),
            work_unit_id: None,
            contract_id: None,
            claim_id: None,
        },
        policy: PolicyContext {
            policy_bundle_id: "policy.compute.launch".to_string(),
            policy_version: "1".to_string(),
            approved_by: "openagents.compute.market".to_string(),
        },
        product: ComputeProduct {
            product_id: binding.product_id.to_string(),
            resource_class: "compute".to_string(),
            capacity_unit: "request".to_string(),
            window_spec: "session".to_string(),
            region_spec: vec!["local".to_string()],
            performance_band: Some("desktop-local".to_string()),
            sla_terms_ref: Some("sla.autopilot.best_effort".to_string()),
            cost_proof_required: false,
            attestation_required: false,
            settlement_mode: ComputeSettlementMode::Physical,
            index_eligible: false,
            status: ComputeProductStatus::Active,
            version: "v1".to_string(),
            created_at_ms: LAUNCH_PRODUCT_CREATED_AT_MS,
            taxonomy_version: Some(COMPUTE_LAUNCH_TAXONOMY_VERSION.to_string()),
            capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(binding.backend_family),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(binding.compute_family),
                topology_kind: None,
                provisioning_kind: None,
                proof_posture: None,
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some(binding.model_policy.to_string()),
                model_family: None,
                host_capability: None,
                apple_platform,
                gpt_oss_runtime,
                latency_ms_p50: None,
                throughput_per_minute: None,
                concurrency_limit: Some(1),
            }),
            metadata: json!({
                "source": "desktop.online_inventory",
                "backend_family": backend_family,
                "compute_family": compute_family_label(binding.compute_family),
                "product_family": binding.product_id,
            }),
        },
        evidence: Vec::new(),
        hints: receipt_hints_for_notional(0),
    }
}

fn build_online_capacity_lot_request(
    state: &RenderState,
    binding: LaunchComputeBinding,
) -> Result<CreateCapacityLotRequest, String> {
    let session_started_at_ms = state
        .provider_runtime
        .inventory_session_started_at_ms
        .ok_or_else(|| "provider inventory session missing".to_string())?;
    let provider_id = provider_id_for_state(state);
    let delivery_start_ms = session_started_at_ms;
    let delivery_end_ms = session_started_at_ms.saturating_add(ONLINE_INVENTORY_WINDOW_DURATION_MS);
    Ok(CreateCapacityLotRequest {
        idempotency_key: format!(
            "desktop.online.compute_lot:{}:{}:{}",
            canonical_kernel_id_component(provider_id.as_str()),
            canonical_kernel_id_component(binding.product_id),
            session_started_at_ms
        ),
        trace: TraceContext {
            session_id: Some("desktop.online_inventory".to_string()),
            trajectory_hash: Some(format!(
                "traj:{}:{}",
                canonical_kernel_id_component(provider_id.as_str()),
                canonical_kernel_id_component(binding.product_id)
            )),
            job_hash: Some(binding.product_id.to_string()),
            run_id: Some(format!(
                "inventory:{}:{}",
                canonical_kernel_id_component(provider_id.as_str()),
                canonical_kernel_id_component(binding.product_id)
            )),
            work_unit_id: None,
            contract_id: None,
            claim_id: None,
        },
        policy: kernel_policy_context(state),
        lot: CapacityLot {
            capacity_lot_id: online_capacity_lot_id_for_binding(
                provider_id.as_str(),
                binding.product_id,
                session_started_at_ms,
            ),
            product_id: binding.product_id.to_string(),
            provider_id: provider_id.clone(),
            delivery_start_ms,
            delivery_end_ms,
            quantity: ONLINE_INVENTORY_QUANTITY,
            min_unit_price: Some(btc_sats_money(price_floor_sats_for_product_id(
                binding.product_id,
            ))),
            region_hint: Some("local".to_string()),
            attestation_posture: Some("desktop.local.best_effort".to_string()),
            reserve_state: CapacityReserveState::Available,
            offer_expires_at_ms: delivery_end_ms,
            status: CapacityLotStatus::Open,
            environment_binding: None,
            metadata: json!({
                "source": "desktop.go_online",
                "compute_family": compute_family_label(binding.compute_family),
                "provider_id": provider_id,
                "session_started_at_ms": session_started_at_ms,
                "ready_model": ready_model_for_binding(state, binding),
                "configured_model": configured_model_for_binding(state, binding),
                "source_badge": "desktop.go_online",
                "terms_label": terms_label_for_product_id(binding.product_id),
                "price_floor_sats": price_floor_sats_for_product_id(binding.product_id),
            }),
        },
        evidence: provider_inventory_evidence_refs(state),
        hints: receipt_hints_for_notional(0),
    })
}

fn build_forward_capacity_lot_request(
    state: &RenderState,
    binding: LaunchComputeBinding,
) -> Result<CreateCapacityLotRequest, String> {
    let session_started_at_ms = state
        .provider_runtime
        .inventory_session_started_at_ms
        .ok_or_else(|| "provider inventory session missing".to_string())?;
    let provider_id = provider_id_for_state(state);
    let delivery_start_ms = session_started_at_ms.saturating_add(FORWARD_INVENTORY_START_DELAY_MS);
    let delivery_end_ms = delivery_start_ms.saturating_add(FORWARD_INVENTORY_WINDOW_DURATION_MS);
    let floor_sats = price_floor_sats_for_product_id(binding.product_id).saturating_mul(2);
    Ok(CreateCapacityLotRequest {
        idempotency_key: format!(
            "desktop.forward.compute_lot:{}:{}:{}",
            canonical_kernel_id_component(provider_id.as_str()),
            canonical_kernel_id_component(binding.product_id),
            session_started_at_ms
        ),
        trace: TraceContext {
            session_id: Some("desktop.forward_inventory".to_string()),
            trajectory_hash: Some(format!(
                "traj:forward:{}:{}",
                canonical_kernel_id_component(provider_id.as_str()),
                canonical_kernel_id_component(binding.product_id)
            )),
            job_hash: Some(binding.product_id.to_string()),
            run_id: Some(format!(
                "forward_inventory:{}:{}",
                canonical_kernel_id_component(provider_id.as_str()),
                canonical_kernel_id_component(binding.product_id)
            )),
            work_unit_id: None,
            contract_id: None,
            claim_id: None,
        },
        policy: kernel_policy_context(state),
        lot: CapacityLot {
            capacity_lot_id: forward_capacity_lot_id_for_binding(
                provider_id.as_str(),
                binding.product_id,
                session_started_at_ms,
            ),
            product_id: binding.product_id.to_string(),
            provider_id: provider_id.clone(),
            delivery_start_ms,
            delivery_end_ms,
            quantity: FORWARD_INVENTORY_QUANTITY,
            min_unit_price: Some(btc_sats_money(floor_sats)),
            region_hint: Some("local".to_string()),
            attestation_posture: Some("desktop.local.forward_commitment".to_string()),
            reserve_state: CapacityReserveState::Available,
            offer_expires_at_ms: delivery_start_ms,
            status: CapacityLotStatus::Open,
            environment_binding: None,
            metadata: json!({
                "source": "desktop.forward_inventory",
                "market_phase": "forward_physical",
                "compute_family": compute_family_label(binding.compute_family),
                "provider_id": provider_id,
                "session_started_at_ms": session_started_at_ms,
                "delivery_start_ms": delivery_start_ms,
                "delivery_end_ms": delivery_end_ms,
                "ready_model": ready_model_for_binding(state, binding),
                "configured_model": configured_model_for_binding(state, binding),
                "source_badge": "desktop.forward_inventory",
                "terms_label": forward_terms_label_for_product_id(binding.product_id),
                "price_floor_sats": floor_sats,
                "remedy_profile": forward_remedy_profile_for_product_id(binding.product_id),
                "bond_posture": {
                    "provider_bond_required": true,
                    "bond_mode": "performance_bond"
                },
            }),
        },
        evidence: provider_inventory_evidence_refs(state),
        hints: receipt_hints_for_notional(0),
    })
}

fn build_capacity_instrument_request(
    state: &RenderState,
    request: &JobInboxRequest,
) -> Result<CreateCapacityInstrumentRequest, String> {
    let Some(linkage) = compute_linkage_for_request(
        state,
        request.request_id.as_str(),
        request.capability.as_str(),
    ) else {
        return Err(format!(
            "unsupported compute capability for canonicalization: {}",
            request.capability
        ));
    };
    let delivery_start_ms = current_epoch_ms();
    let delivery_end_ms =
        delivery_start_ms.saturating_add((request.ttl_seconds as i64).saturating_mul(1_000));
    Ok(CreateCapacityInstrumentRequest {
        idempotency_key: format!("desktop.accept.compute_instrument:{}", request.request_id),
        trace: trace_context_for_request(request, false),
        policy: kernel_policy_context(state),
        instrument: CapacityInstrument {
            instrument_id: linkage.capacity_instrument_id.clone(),
            product_id: linkage.product_id.to_string(),
            capacity_lot_id: Some(linkage.capacity_lot_id.clone()),
            buyer_id: Some(request.requester.clone()),
            provider_id: Some(provider_id_for_state(state)),
            delivery_start_ms,
            delivery_end_ms,
            quantity: 1,
            fixed_price: Some(btc_sats_money(request.price_sats)),
            reference_index_id: None,
            kind: CapacityInstrumentKind::Spot,
            settlement_mode: ComputeSettlementMode::Physical,
            created_at_ms: current_epoch_ms(),
            status: CapacityInstrumentStatus::Active,
            environment_binding: None,
            closure_reason: None,
            non_delivery_reason: None,
            settlement_failure_reason: None,
            lifecycle_reason_detail: None,
            metadata: json!({
                "request_id": request.request_id.clone(),
                "provider_job_id": format!("job-{}", request.request_id),
                "work_unit_id": work_unit_id_for_request(request.request_id.as_str()),
                "contract_id": contract_id_for_request(request.request_id.as_str()),
                "compute_family": compute_family_label(linkage.compute_family),
            }),
        },
        evidence: request_evidence_refs(
            request.request_id.as_str(),
            request.skill_scope_id.as_deref(),
        ),
        hints: receipt_hints_for_notional(request.price_sats),
    })
}

fn build_delivery_proof_request(
    state: &RenderState,
    job: &ActiveJobRecord,
) -> Result<(RecordDeliveryProofRequest, DeliveryProofEvaluation), String> {
    let Some(linkage) = compute_linkage_for_active_job(job).or_else(|| {
        compute_linkage_for_request(state, job.request_id.as_str(), job.capability.as_str())
    }) else {
        return Err(format!(
            "unsupported compute capability for canonicalization: {}",
            job.capability
        ));
    };
    let binding = compute_binding_for_product_id(linkage.product_id)
        .ok_or_else(|| format!("unsupported launch compute product: {}", linkage.product_id))?;
    let delivery_context = delivery_context_for_state(state);
    let evaluation = evaluate_delivery_proof(job, binding, &delivery_context);
    let attestation_digest = job.execution_provenance.as_ref().map(|provenance| {
        let digest_input = format!(
            "{}:{}:{}:{}",
            provenance.backend,
            provenance.served_model,
            provenance.normalized_prompt_digest,
            provenance.normalized_options_digest
        );
        sha256_prefixed_text(digest_input.as_str())
    });
    let execution_output_digest = delivery_context
        .execution_output
        .as_deref()
        .map(sha256_prefixed_text);
    let request = RecordDeliveryProofRequest {
        idempotency_key: format!("desktop.finalize.compute_delivery:{}", job.request_id),
        trace: trace_context_for_job(job),
        policy: kernel_policy_context(state),
        delivery_proof: DeliveryProof {
            delivery_proof_id: linkage.delivery_proof_id.clone(),
            capacity_lot_id: linkage.capacity_lot_id.clone(),
            product_id: linkage.product_id.to_string(),
            instrument_id: Some(linkage.capacity_instrument_id.clone()),
            contract_id: Some(contract_id_for_request(job.request_id.as_str())),
            created_at_ms: current_epoch_ms(),
            metered_quantity: evaluation.metered_quantity,
            accepted_quantity: evaluation.accepted_quantity,
            performance_band_observed: Some("desktop-local".to_string()),
            variance_reason: evaluation.variance_reason,
            variance_reason_detail: evaluation.variance_reason_detail.clone(),
            attestation_digest,
            cost_attestation_ref: job
                .payment_id
                .as_ref()
                .map(|payment_id| format!("oa://wallet/payments/{payment_id}")),
            status: evaluation.status,
            rejection_reason: evaluation.rejection_reason,
            topology_evidence: None,
            sandbox_evidence: None,
            verification_evidence: None,
            promised_capability_envelope: Some(evaluation.promised_capability_envelope.clone()),
            observed_capability_envelope: evaluation.observed_capability_envelope.clone(),
            metadata: json!({
                "request_id": job.request_id.clone(),
                "job_id": job.job_id.clone(),
                "metering_rule_id": evaluation.metering_rule_id,
                "settlement_class": evaluation.settlement_class,
                "compute_family": compute_family_label(linkage.compute_family),
                "delivery_status": evaluation.status.label(),
                "variance_reason": evaluation.variance_reason.map(|reason| reason.label().to_string()),
                "variance_reason_detail": evaluation.variance_reason_detail.clone(),
                "rejection_reason": evaluation.rejection_reason.map(|reason| reason.label().to_string()),
                "rejection_reason_detail": (evaluation.rejection_reason.is_some())
                    .then(|| evaluation.variance_reason_detail.clone())
                    .flatten(),
                "served_model": job
                    .execution_provenance
                    .as_ref()
                    .map(|provenance| provenance.served_model.clone()),
                "requested_model": job.requested_model.clone(),
                "execution_output_digest": execution_output_digest,
                "prompt_token_count": job
                    .execution_provenance
                    .as_ref()
                    .and_then(|provenance| provenance.prompt_token_count),
                "generated_token_count": job
                    .execution_provenance
                    .as_ref()
                    .and_then(|provenance| provenance.generated_token_count),
                "total_duration_ns": job
                    .execution_provenance
                    .as_ref()
                    .and_then(|provenance| provenance.total_duration_ns),
                "provider_thread_id": delivery_context.provider_thread_id.clone(),
                "provider_turn_id": delivery_context.provider_turn_id.clone(),
                "runtime_identity_backend": job
                    .execution_provenance
                    .as_ref()
                    .map(|provenance| provenance.backend.clone()),
                "apple_bridge_status": delivery_context.apple_bridge_status.clone(),
                "promised_capability_envelope": evaluation.promised_capability_envelope.clone(),
                "observed_capability_envelope": evaluation.observed_capability_envelope.clone(),
            }),
        },
        evidence: delivery_proof_evidence_refs(
            job,
            delivery_context.execution_output.as_deref(),
            &evaluation,
        ),
        hints: receipt_hints_for_notional(job.quoted_price_sats),
    };
    Ok((request, evaluation))
}

fn delivery_context_for_state(state: &RenderState) -> LaunchDeliveryContext {
    LaunchDeliveryContext {
        execution_output: state.active_job.execution_output.clone(),
        provider_thread_id: state.active_job.execution_thread_id.clone(),
        provider_turn_id: state.active_job.execution_turn_id.clone(),
        gpt_oss_ready_model: state.provider_runtime.gpt_oss.ready_model.clone(),
        gpt_oss_metrics: state.provider_runtime.gpt_oss.last_metrics.clone(),
        apple_ready_model: state.provider_runtime.apple_fm.ready_model.clone(),
        apple_metrics: state.provider_runtime.apple_fm.last_metrics.clone(),
        apple_model_available: state.provider_runtime.apple_fm.model_available,
        apple_bridge_status: state.provider_runtime.apple_fm.bridge_status.clone(),
    }
}

fn evaluate_delivery_proof(
    job: &ActiveJobRecord,
    binding: LaunchComputeBinding,
    context: &LaunchDeliveryContext,
) -> DeliveryProofEvaluation {
    let promised_capability_envelope =
        promised_capability_envelope_for_delivery(job, binding, context);
    let observed_capability_envelope =
        observed_capability_envelope_for_delivery(job, binding, context);
    let metering_rule_id = metering_rule_id_for_binding(binding);
    let settlement_class = compute_family_label(binding.compute_family);
    let output_text = context
        .execution_output
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let metered_quantity = match binding.compute_family {
        ComputeFamily::Inference => {
            if observed_capability_envelope.is_some() && output_text.is_some() {
                1
            } else {
                0
            }
        }
        ComputeFamily::Embeddings => embedding_quantity_from_output(output_text),
        ComputeFamily::SandboxExecution
        | ComputeFamily::Evaluation
        | ComputeFamily::Training
        | ComputeFamily::AdapterHosting => 0,
    };

    let mut evaluation = DeliveryProofEvaluation {
        metering_rule_id,
        settlement_class,
        metered_quantity,
        accepted_quantity: metered_quantity.min(1),
        status: DeliveryProofStatus::Accepted,
        variance_reason: None,
        variance_reason_detail: None,
        rejection_reason: None,
        promised_capability_envelope,
        observed_capability_envelope,
    };

    if binding.backend_family == ComputeBackendFamily::AppleFoundationModels
        && binding.compute_family == ComputeFamily::Embeddings
    {
        set_delivery_rejection(
            &mut evaluation,
            DeliveryRejectionReason::NonConformingDelivery,
            "apple_foundation_models_embeddings_not_supported",
        );
        return evaluation;
    }

    let Some(observed) = evaluation.observed_capability_envelope.clone() else {
        set_delivery_rejection(
            &mut evaluation,
            DeliveryRejectionReason::AttestationMissing,
            "runtime adapter did not emit observed capability envelope",
        );
        return evaluation;
    };

    if observed.backend_family != Some(binding.backend_family) {
        set_delivery_rejection(
            &mut evaluation,
            DeliveryRejectionReason::RuntimeIdentityMismatch,
            "observed backend did not match committed launch product",
        );
        return evaluation;
    }
    if observed.compute_family != Some(binding.compute_family) {
        set_delivery_rejection(
            &mut evaluation,
            DeliveryRejectionReason::NonConformingDelivery,
            "observed compute family did not match committed launch product",
        );
        return evaluation;
    }
    if metered_quantity == 0 {
        let detail = match binding.compute_family {
            ComputeFamily::Inference => {
                "inference delivery missing execution output or runtime attestation"
            }
            ComputeFamily::Embeddings => "embedding delivery missing vector-like execution output",
            ComputeFamily::SandboxExecution
            | ComputeFamily::Evaluation
            | ComputeFamily::Training
            | ComputeFamily::AdapterHosting => {
                "delivery used a compute family outside the retained MVP launch set"
            }
        };
        set_delivery_rejection(
            &mut evaluation,
            DeliveryRejectionReason::NonConformingDelivery,
            detail,
        );
        return evaluation;
    }

    if let Some(requested_model) = job.requested_model.as_deref()
        && job
            .execution_provenance
            .as_ref()
            .is_some_and(|provenance| provenance.served_model != requested_model)
    {
        set_delivery_variance(
            &mut evaluation,
            ComputeDeliveryVarianceReason::ModelPolicyDrift,
            format!(
                "requested model '{}' settled against '{}'",
                requested_model,
                job.execution_provenance
                    .as_ref()
                    .map(|provenance| provenance.served_model.as_str())
                    .unwrap_or("unknown")
            ),
        );
        return evaluation;
    }

    if let (Some(promised_latency), Some(observed_latency)) = (
        evaluation.promised_capability_envelope.latency_ms_p50,
        observed.latency_ms_p50,
    ) && observed_latency > promised_latency
    {
        set_delivery_variance(
            &mut evaluation,
            ComputeDeliveryVarianceReason::LatencyBreach,
            format!(
                "observed p50 latency {}ms exceeded promised {}ms",
                observed_latency, promised_latency
            ),
        );
        return evaluation;
    }

    if let (Some(promised_throughput), Some(observed_throughput)) = (
        evaluation
            .promised_capability_envelope
            .throughput_per_minute,
        observed.throughput_per_minute,
    ) && observed_throughput < promised_throughput
    {
        set_delivery_variance(
            &mut evaluation,
            ComputeDeliveryVarianceReason::ThroughputShortfall,
            format!(
                "observed throughput {} fell below promised {}",
                observed_throughput, promised_throughput
            ),
        );
        return evaluation;
    }

    if capability_envelope_mismatch(&evaluation.promised_capability_envelope, &observed) {
        set_delivery_variance(
            &mut evaluation,
            ComputeDeliveryVarianceReason::CapabilityEnvelopeMismatch,
            "observed capability envelope diverged from committed launch product".to_string(),
        );
    }
    evaluation
}

fn promised_capability_envelope_for_delivery(
    job: &ActiveJobRecord,
    binding: LaunchComputeBinding,
    context: &LaunchDeliveryContext,
) -> ComputeCapabilityEnvelope {
    let model_identity = job
        .requested_model
        .clone()
        .or_else(|| ready_model_from_delivery_context(binding, context));
    let throughput_per_minute = throughput_from_metrics(
        context_metrics_for_binding(binding, context),
        inferred_quantity_for_family(binding.compute_family, context.execution_output.as_deref()),
    );
    ComputeCapabilityEnvelope {
        backend_family: Some(binding.backend_family),
        execution_kind: Some(ComputeExecutionKind::LocalInference),
        compute_family: Some(binding.compute_family),
        topology_kind: None,
        provisioning_kind: None,
        proof_posture: None,
        validator_requirements: None,
        artifact_residency: None,
        environment_binding: None,
        checkpoint_binding: None,
        model_policy: Some(binding.model_policy.to_string()),
        model_family: model_identity.clone(),
        host_capability: None,
        apple_platform: apple_platform_for_binding(binding, context),
        gpt_oss_runtime: gpt_oss_runtime_for_binding(binding, model_identity),
        latency_ms_p50: latency_from_metrics(context_metrics_for_binding(binding, context)),
        throughput_per_minute,
        concurrency_limit: Some(1),
    }
}

fn observed_capability_envelope_for_delivery(
    job: &ActiveJobRecord,
    binding: LaunchComputeBinding,
    context: &LaunchDeliveryContext,
) -> Option<ComputeCapabilityEnvelope> {
    let provenance = job.execution_provenance.as_ref()?;
    let observed_backend_family = backend_family_from_runtime_label(provenance.backend.as_str())?;
    let inferred_output_quantity =
        inferred_quantity_for_family(binding.compute_family, context.execution_output.as_deref());
    Some(ComputeCapabilityEnvelope {
        backend_family: Some(observed_backend_family),
        execution_kind: Some(ComputeExecutionKind::LocalInference),
        compute_family: Some(binding.compute_family),
        topology_kind: None,
        provisioning_kind: None,
        proof_posture: None,
        validator_requirements: None,
        artifact_residency: None,
        environment_binding: None,
        checkpoint_binding: None,
        model_policy: Some(binding.model_policy.to_string()),
        model_family: Some(provenance.served_model.clone()),
        host_capability: None,
        apple_platform: match observed_backend_family {
            ComputeBackendFamily::AppleFoundationModels => Some(ApplePlatformCapability {
                apple_silicon_required: true,
                apple_intelligence_required: true,
                apple_intelligence_available: Some(context.apple_model_available),
                minimum_macos_version: Some("26.0".to_string()),
            }),
            ComputeBackendFamily::GptOss => None,
        },
        gpt_oss_runtime: match observed_backend_family {
            ComputeBackendFamily::GptOss => Some(GptOssRuntimeCapability {
                runtime_ready: Some(true),
                model_name: Some(provenance.served_model.clone()),
                quantization: None,
            }),
            ComputeBackendFamily::AppleFoundationModels => None,
        },
        latency_ms_p50: latency_from_provenance(provenance),
        throughput_per_minute: throughput_from_provenance(provenance, inferred_output_quantity),
        concurrency_limit: Some(1),
    })
}

fn metering_rule_id_for_binding(binding: LaunchComputeBinding) -> &'static str {
    match (binding.backend_family, binding.compute_family) {
        (ComputeBackendFamily::GptOss, ComputeFamily::Inference) => "meter.gpt_oss.inference.v1",
        (ComputeBackendFamily::GptOss, ComputeFamily::Embeddings) => {
            "meter.gpt_oss.embeddings.unsupported"
        }
        (ComputeBackendFamily::AppleFoundationModels, ComputeFamily::Inference) => {
            "meter.apple_fm.inference.v1"
        }
        (ComputeBackendFamily::AppleFoundationModels, ComputeFamily::Embeddings) => {
            "meter.apple_fm.embeddings.unsupported"
        }
        (_, ComputeFamily::SandboxExecution)
        | (_, ComputeFamily::Evaluation)
        | (_, ComputeFamily::Training)
        | (_, ComputeFamily::AdapterHosting) => "meter.compute.unsupported",
    }
}

fn latency_from_provenance(
    provenance: &crate::local_inference_runtime::LocalInferenceExecutionProvenance,
) -> Option<u32> {
    provenance
        .total_duration_ns
        .map(|duration| duration / 1_000_000)
        .and_then(|duration_ms| u32::try_from(duration_ms).ok())
}

fn latency_from_metrics(metrics: Option<&LocalInferenceExecutionMetrics>) -> Option<u32> {
    metrics
        .and_then(|metrics| metrics.total_duration_ns)
        .map(|duration| duration / 1_000_000)
        .and_then(|duration_ms| u32::try_from(duration_ms).ok())
}

fn throughput_from_provenance(
    provenance: &crate::local_inference_runtime::LocalInferenceExecutionProvenance,
    quantity_hint: u64,
) -> Option<u32> {
    let duration_ns = provenance.total_duration_ns?;
    if duration_ns == 0 {
        return None;
    }
    let units = match provenance.generated_token_count {
        Some(count) if count > 0 => count,
        _ if quantity_hint > 0 => quantity_hint,
        _ => 1,
    };
    throughput_per_minute(duration_ns, units)
}

fn throughput_from_metrics(
    metrics: Option<&LocalInferenceExecutionMetrics>,
    quantity_hint: u64,
) -> Option<u32> {
    let duration_ns = metrics.and_then(|metrics| metrics.total_duration_ns)?;
    if duration_ns == 0 || quantity_hint == 0 {
        return None;
    }
    throughput_per_minute(duration_ns, quantity_hint)
}

fn throughput_per_minute(duration_ns: u64, units: u64) -> Option<u32> {
    let per_minute = (u128::from(units))
        .saturating_mul(60_000_000_000u128)
        .checked_div(u128::from(duration_ns))?;
    u32::try_from(per_minute).ok()
}

fn inferred_quantity_for_family(
    compute_family: ComputeFamily,
    execution_output: Option<&str>,
) -> u64 {
    match compute_family {
        ComputeFamily::Inference => 1,
        ComputeFamily::Embeddings => embedding_quantity_from_output(
            execution_output
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        ),
        ComputeFamily::SandboxExecution
        | ComputeFamily::Evaluation
        | ComputeFamily::Training
        | ComputeFamily::AdapterHosting => 0,
    }
}

fn embedding_quantity_from_output(execution_output: Option<&str>) -> u64 {
    let Some(execution_output) = execution_output else {
        return 0;
    };
    let Ok(parsed) = serde_json::from_str::<Value>(execution_output) else {
        return 0;
    };
    match parsed {
        Value::Array(values) if values.is_empty() => 0,
        Value::Array(values) if values.iter().all(Value::is_number) => 1,
        Value::Array(values) => values
            .iter()
            .filter(|value| {
                value
                    .as_array()
                    .is_some_and(|items| items.iter().all(Value::is_number))
            })
            .count() as u64,
        _ => 0,
    }
}

fn backend_family_from_runtime_label(value: &str) -> Option<ComputeBackendFamily> {
    match value.trim() {
        "gpt_oss" | "psionic" | "ollama" => Some(ComputeBackendFamily::GptOss),
        "apple_foundation_models" => Some(ComputeBackendFamily::AppleFoundationModels),
        _ => None,
    }
}

fn context_metrics_for_binding(
    binding: LaunchComputeBinding,
    context: &LaunchDeliveryContext,
) -> Option<&LocalInferenceExecutionMetrics> {
    match binding.backend_family {
        ComputeBackendFamily::GptOss => context.gpt_oss_metrics.as_ref(),
        ComputeBackendFamily::AppleFoundationModels => context.apple_metrics.as_ref(),
    }
}

fn ready_model_from_delivery_context(
    binding: LaunchComputeBinding,
    context: &LaunchDeliveryContext,
) -> Option<String> {
    match binding.backend_family {
        ComputeBackendFamily::GptOss => context.gpt_oss_ready_model.clone(),
        ComputeBackendFamily::AppleFoundationModels => context.apple_ready_model.clone(),
    }
}

fn apple_platform_for_binding(
    binding: LaunchComputeBinding,
    context: &LaunchDeliveryContext,
) -> Option<ApplePlatformCapability> {
    if binding.backend_family != ComputeBackendFamily::AppleFoundationModels {
        return None;
    }
    Some(ApplePlatformCapability {
        apple_silicon_required: true,
        apple_intelligence_required: true,
        apple_intelligence_available: Some(context.apple_model_available),
        minimum_macos_version: Some("26.0".to_string()),
    })
}

fn gpt_oss_runtime_for_binding(
    binding: LaunchComputeBinding,
    model_name: Option<String>,
) -> Option<GptOssRuntimeCapability> {
    if binding.backend_family != ComputeBackendFamily::GptOss {
        return None;
    }
    Some(GptOssRuntimeCapability {
        runtime_ready: Some(true),
        model_name,
        quantization: None,
    })
}

fn capability_envelope_mismatch(
    promised: &ComputeCapabilityEnvelope,
    observed: &ComputeCapabilityEnvelope,
) -> bool {
    if promised.backend_family.is_some() && promised.backend_family != observed.backend_family {
        return true;
    }
    if promised.execution_kind.is_some() && promised.execution_kind != observed.execution_kind {
        return true;
    }
    if promised.compute_family.is_some() && promised.compute_family != observed.compute_family {
        return true;
    }
    if let Some(promised_model_family) = promised.model_family.as_deref()
        && observed.model_family.as_deref() != Some(promised_model_family)
    {
        return true;
    }
    if let Some(promised_concurrency) = promised.concurrency_limit
        && observed
            .concurrency_limit
            .is_some_and(|observed_concurrency| observed_concurrency < promised_concurrency)
    {
        return true;
    }
    false
}

fn set_delivery_variance(
    evaluation: &mut DeliveryProofEvaluation,
    reason: ComputeDeliveryVarianceReason,
    detail: impl Into<String>,
) {
    evaluation.variance_reason = Some(reason);
    evaluation.variance_reason_detail = Some(detail.into());
}

fn set_delivery_rejection(
    evaluation: &mut DeliveryProofEvaluation,
    reason: DeliveryRejectionReason,
    detail: impl Into<String>,
) {
    evaluation.status = DeliveryProofStatus::Rejected;
    evaluation.accepted_quantity = 0;
    evaluation.variance_reason = None;
    evaluation.rejection_reason = Some(reason);
    evaluation.variance_reason_detail = Some(detail.into());
}

fn delivery_proof_evidence_refs(
    job: &ActiveJobRecord,
    execution_output: Option<&str>,
    evaluation: &DeliveryProofEvaluation,
) -> Vec<EvidenceRef> {
    let mut evidence = submission_evidence_refs(job, execution_output);
    evidence.push(evidence_ref(
        "compute_delivery_metering_rule",
        format!(
            "oa://kernel/compute/metering/{}",
            canonical_kernel_id_component(evaluation.metering_rule_id)
        ),
        evaluation.metering_rule_id,
    ));
    evidence.push(evidence_ref(
        "compute_delivery_promised_envelope",
        format!("oa://kernel/compute/promised/{}", job.job_id),
        serde_json::to_string(&evaluation.promised_capability_envelope)
            .unwrap_or_else(|_| "null".to_string())
            .as_str(),
    ));
    if let Some(observed) = evaluation.observed_capability_envelope.as_ref() {
        evidence.push(evidence_ref(
            "compute_delivery_observed_envelope",
            format!("oa://kernel/compute/observed/{}", job.job_id),
            serde_json::to_string(observed)
                .unwrap_or_else(|_| "null".to_string())
                .as_str(),
        ));
    }
    if let Some(variance_reason) = evaluation.variance_reason {
        evidence.push(evidence_ref(
            "compute_delivery_variance",
            format!("oa://kernel/compute/variance/{}", job.job_id),
            variance_reason.label(),
        ));
    }
    if let Some(rejection_reason) = evaluation.rejection_reason {
        evidence.push(evidence_ref(
            "compute_delivery_rejection",
            format!("oa://kernel/compute/rejection/{}", job.job_id),
            rejection_reason.label(),
        ));
    }
    evidence
}

fn build_work_unit_request(
    state: &RenderState,
    request: &JobInboxRequest,
) -> CreateWorkUnitRequest {
    let work_unit_id = work_unit_id_for_request(request.request_id.as_str());
    let compute_linkage = compute_linkage_for_request(
        state,
        request.request_id.as_str(),
        request.capability.as_str(),
    );
    CreateWorkUnitRequest {
        idempotency_key: format!("desktop.accept.work_unit:{}", request.request_id),
        trace: trace_context_for_request(request, true),
        policy: kernel_policy_context(state),
        work_unit: WorkUnit {
            work_unit_id,
            external_request_id: Some(request.request_id.clone()),
            requester_id: Some(request.requester.clone()),
            provider_id: state
                .nostr_identity
                .as_ref()
                .map(|identity| identity.npub.clone())
                .or_else(|| Some("autopilot.desktop".to_string())),
            capability: Some(request.capability.clone()),
            demand_source: Some(request.demand_source.label().to_string()),
            created_at_ms: current_epoch_ms(),
            status: WorkUnitStatus::Created,
            quoted_price: Some(btc_sats_money(request.price_sats)),
            metadata: json!({
                "request_kind": request.request_kind,
                "execution_input": request.execution_input.clone(),
                "ttl_seconds": request.ttl_seconds,
                "skill_scope_id": request.skill_scope_id.clone(),
                "skl_manifest_a": request.skl_manifest_a.clone(),
                "skl_manifest_event_id": request.skl_manifest_event_id.clone(),
                "compute_product_id": compute_linkage.as_ref().map(|linkage| linkage.product_id),
                "capacity_lot_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_lot_id.clone()),
                "capacity_instrument_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_instrument_id.clone()),
            }),
        },
        evidence: request_evidence_refs(
            request.request_id.as_str(),
            request.skill_scope_id.as_deref(),
        ),
        hints: receipt_hints_for_notional(request.price_sats),
    }
}

fn build_contract_request(state: &RenderState, request: &JobInboxRequest) -> CreateContractRequest {
    let compute_linkage = compute_linkage_for_request(
        state,
        request.request_id.as_str(),
        request.capability.as_str(),
    );
    CreateContractRequest {
        idempotency_key: format!("desktop.accept.contract:{}", request.request_id),
        trace: trace_context_for_request(request, false),
        policy: kernel_policy_context(state),
        contract: Contract {
            contract_id: contract_id_for_request(request.request_id.as_str()),
            work_unit_id: work_unit_id_for_request(request.request_id.as_str()),
            provider_id: state
                .nostr_identity
                .as_ref()
                .map(|identity| identity.npub.clone())
                .or_else(|| Some("autopilot.desktop".to_string())),
            created_at_ms: current_epoch_ms(),
            status: ContractStatus::Created,
            settlement_asset: Some(Asset::Btc),
            quoted_price: Some(btc_sats_money(request.price_sats)),
            warranty_window_ms: Some((request.ttl_seconds as u64).saturating_mul(1_000)),
            metadata: json!({
                "request_id": request.request_id.clone(),
                "requester": request.requester.clone(),
                "demand_source": request.demand_source.label(),
                "provider_job_id": format!("job-{}", request.request_id),
                "ttl_seconds": request.ttl_seconds,
                "compute_product_id": compute_linkage.as_ref().map(|linkage| linkage.product_id),
                "capacity_lot_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_lot_id.clone()),
                "capacity_instrument_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_instrument_id.clone()),
            }),
        },
        evidence: request_evidence_refs(
            request.request_id.as_str(),
            request.skill_scope_id.as_deref(),
        ),
        hints: receipt_hints_for_notional(request.price_sats),
    }
}

fn build_submit_output_request(state: &RenderState, job: &ActiveJobRecord) -> SubmitOutputRequest {
    let execution_output = state
        .active_job
        .execution_output
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("provider execution completed without explicit output");
    let compute_linkage = compute_linkage_for_active_job(job).or_else(|| {
        compute_linkage_for_request(state, job.request_id.as_str(), job.capability.as_str())
    });
    SubmitOutputRequest {
        idempotency_key: format!("desktop.submit.output:{}", job.request_id),
        trace: trace_context_for_job(job),
        policy: kernel_policy_context(state),
        submission: Submission {
            submission_id: format!(
                "submission.{}",
                canonical_kernel_id_component(job.request_id.as_str())
            ),
            contract_id: contract_id_for_request(job.request_id.as_str()),
            work_unit_id: work_unit_id_for_request(job.request_id.as_str()),
            created_at_ms: current_epoch_ms(),
            status: SubmissionStatus::Received,
            output_ref: Some(format!("oa://autopilot/jobs/{}/output", job.job_id)),
            provenance_digest: Some(sha256_prefixed_text(execution_output)),
            metadata: json!({
                "request_id": job.request_id.clone(),
                "job_id": job.job_id.clone(),
                "request_kind": job.request_kind,
                "capability": job.capability.clone(),
                "demand_source": job.demand_source.label(),
                "input": job.execution_input.clone(),
                "output": execution_output,
                "provider_thread_id": state.active_job.execution_thread_id.clone(),
                "provider_turn_id": state.active_job.execution_turn_id.clone(),
                "result_event_id": job.sa_tick_result_event_id.clone(),
                "local_execution_provenance": job
                    .execution_provenance
                    .as_ref()
                    .map(|provenance| provenance.receipt_payload()),
                "compute_product_id": compute_linkage.as_ref().map(|linkage| linkage.product_id),
                "capacity_lot_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_lot_id.clone()),
                "capacity_instrument_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_instrument_id.clone()),
            }),
        },
        evidence: submission_evidence_refs(job, state.active_job.execution_output.as_deref()),
        hints: receipt_hints_for_notional(job.quoted_price_sats),
    }
}

fn build_finalize_verdict_request(
    state: &RenderState,
    job: &ActiveJobRecord,
) -> FinalizeVerdictRequest {
    let verdict_id = format!(
        "verdict.{}",
        canonical_kernel_id_component(job.request_id.as_str())
    );
    let payment_pointer = job
        .payment_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let settlement_status = if payment_pointer.is_some() {
        SettlementStatus::Settled
    } else {
        SettlementStatus::Pending
    };
    let compute_linkage = compute_linkage_for_active_job(job).or_else(|| {
        compute_linkage_for_request(state, job.request_id.as_str(), job.capability.as_str())
    });
    FinalizeVerdictRequest {
        idempotency_key: format!("desktop.finalize.verdict:{}", job.request_id),
        trace: trace_context_for_job(job),
        policy: kernel_policy_context(state),
        verdict: Verdict {
            verdict_id: verdict_id.clone(),
            contract_id: contract_id_for_request(job.request_id.as_str()),
            work_unit_id: work_unit_id_for_request(job.request_id.as_str()),
            created_at_ms: current_epoch_ms(),
            outcome: VerdictOutcome::Pass,
            verification_tier: Some(VerificationTier::TierOObjective),
            settlement_status,
            reason_code: Some("desktop.job.paid".to_string()),
            metadata: json!({
                "request_id": job.request_id.clone(),
                "job_id": job.job_id.clone(),
                "demand_source": job.demand_source.label(),
                "quoted_price_sats": job.quoted_price_sats,
                "result_event_id": job.sa_tick_result_event_id.clone(),
                "compute_product_id": compute_linkage.as_ref().map(|linkage| linkage.product_id),
                "capacity_lot_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_lot_id.clone()),
                "capacity_instrument_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.capacity_instrument_id.clone()),
                "delivery_proof_id": compute_linkage
                    .as_ref()
                    .map(|linkage| linkage.delivery_proof_id.clone()),
            }),
        },
        settlement_link: payment_pointer
            .clone()
            .map(|payment_pointer| SettlementLink {
                settlement_id: format!(
                    "settlement.{}",
                    canonical_kernel_id_component(job.request_id.as_str())
                ),
                contract_id: contract_id_for_request(job.request_id.as_str()),
                work_unit_id: work_unit_id_for_request(job.request_id.as_str()),
                verdict_id,
                created_at_ms: current_epoch_ms(),
                payment_pointer: Some(payment_pointer),
                settled_amount: Some(btc_sats_money(job.quoted_price_sats)),
                status: SettlementStatus::Settled,
                metadata: json!({
                    "job_id": job.job_id.clone(),
                    "demand_source": job.demand_source.label(),
                    "compute_product_id": compute_linkage
                        .as_ref()
                        .map(|linkage| linkage.product_id),
                    "capacity_instrument_id": compute_linkage
                        .as_ref()
                        .map(|linkage| linkage.capacity_instrument_id.clone()),
                    "delivery_proof_id": compute_linkage
                        .as_ref()
                        .map(|linkage| linkage.delivery_proof_id.clone()),
                }),
            }),
        claim_hook: None,
        evidence: verdict_evidence_refs(job),
        hints: receipt_hints_for_notional(job.quoted_price_sats),
    }
}

fn kernel_policy_context(state: &RenderState) -> PolicyContext {
    PolicyContext {
        policy_bundle_id: "policy.autopilot.desktop".to_string(),
        policy_version: "1".to_string(),
        approved_by: state
            .nostr_identity
            .as_ref()
            .map(|identity| identity.npub.clone())
            .unwrap_or_else(|| "autopilot.desktop".to_string()),
    }
}

fn trace_context_for_request(request: &JobInboxRequest, work_unit_only: bool) -> TraceContext {
    TraceContext {
        session_id: Some("desktop.local".to_string()),
        trajectory_hash: Some(format!("traj:{}", request.request_id)),
        job_hash: Some(request.request_id.clone()),
        run_id: Some(format!("job-{}", request.request_id)),
        work_unit_id: Some(work_unit_id_for_request(request.request_id.as_str())),
        contract_id: (!work_unit_only)
            .then(|| contract_id_for_request(request.request_id.as_str())),
        claim_id: None,
    }
}

fn trace_context_for_job(job: &ActiveJobRecord) -> TraceContext {
    TraceContext {
        session_id: Some("desktop.local".to_string()),
        trajectory_hash: job.sa_trajectory_session_id.clone(),
        job_hash: Some(job.request_id.clone()),
        run_id: Some(job.job_id.clone()),
        work_unit_id: Some(work_unit_id_for_request(job.request_id.as_str())),
        contract_id: Some(contract_id_for_request(job.request_id.as_str())),
        claim_id: None,
    }
}

fn request_evidence_refs(request_id: &str, skill_scope_id: Option<&str>) -> Vec<EvidenceRef> {
    let mut evidence = vec![evidence_ref(
        "request_ref",
        format!("oa://autopilot/requests/{request_id}"),
        request_id,
    )];
    if let Some(skill_scope_id) = skill_scope_id.filter(|value| !value.trim().is_empty()) {
        evidence.push(evidence_ref(
            "skill_scope_ref",
            format!("oa://autopilot/skills/{skill_scope_id}"),
            skill_scope_id,
        ));
    }
    evidence
}

fn submission_evidence_refs(
    job: &ActiveJobRecord,
    execution_output: Option<&str>,
) -> Vec<EvidenceRef> {
    let mut evidence =
        request_evidence_refs(job.request_id.as_str(), job.skill_scope_id.as_deref());
    if let Some(result_event_id) = job.sa_tick_result_event_id.as_deref() {
        evidence.push(evidence_ref(
            "nostr_event_ref",
            format!("nostr:event:{result_event_id}"),
            result_event_id,
        ));
    }
    if let Some(output) = execution_output
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        evidence.push(evidence_ref(
            "execution_output_ref",
            format!("oa://autopilot/jobs/{}/output", job.job_id),
            output,
        ));
    }
    if let Some(provenance) = job.execution_provenance.as_ref() {
        evidence.push(EvidenceRef::new(
            "execution_backend_ref",
            format!("oa://autopilot/jobs/{}/execution/backend", job.job_id),
            sha256_prefixed_text(provenance.base_url.as_str()),
        ));
        evidence.push(EvidenceRef::new(
            "attestation:model_version",
            format!(
                "oa://autopilot/jobs/{}/execution/model/{}",
                job.job_id,
                canonical_kernel_id_component(provenance.served_model.as_str()),
            ),
            sha256_prefixed_text(provenance.served_model.as_str()),
        ));
        evidence.push(EvidenceRef::new(
            "execution_prompt_digest",
            format!("oa://autopilot/jobs/{}/execution/prompt", job.job_id),
            provenance.normalized_prompt_digest.clone(),
        ));
        evidence.push(EvidenceRef::new(
            "execution_options_digest",
            format!("oa://autopilot/jobs/{}/execution/options", job.job_id),
            provenance.normalized_options_digest.clone(),
        ));
    }
    evidence
}

fn verdict_evidence_refs(job: &ActiveJobRecord) -> Vec<EvidenceRef> {
    let mut evidence = submission_evidence_refs(job, None);
    if let Some(payment_id) = job
        .payment_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        evidence.push(evidence_ref(
            "payment_pointer_ref",
            format!("oa://wallet/payments/{payment_id}"),
            payment_id,
        ));
    }
    evidence
}

fn evidence_ref(kind: impl Into<String>, uri: impl Into<String>, value: &str) -> EvidenceRef {
    EvidenceRef::new(kind, uri, sha256_prefixed_text(value))
}

fn receipt_hints_for_notional(notional_sats: u64) -> ReceiptHints {
    ReceiptHints {
        category: Some("compute".to_string()),
        tfb_class: Some(FeedbackLatencyClass::Short),
        severity: Some(SeverityClass::Low),
        achieved_verification_tier: Some(VerificationTier::TierOObjective),
        verification_correlated: Some(false),
        provenance_grade: Some(ProvenanceGrade::P1Toolchain),
        auth_assurance_level: Some(AuthAssuranceLevel::Authenticated),
        personhood_proved: Some(false),
        reason_code: None,
        notional: Some(Money {
            asset: openagents_kernel_core::receipts::Asset::Btc,
            amount: MoneyAmount::AmountSats(notional_sats),
        }),
        liability_premium: None,
    }
}

fn btc_sats_money(amount_sats: u64) -> Money {
    Money {
        asset: Asset::Btc,
        amount: MoneyAmount::AmountSats(amount_sats),
    }
}

fn compute_binding_for_backend_and_capability(
    backend: LocalInferenceBackend,
    capability: &str,
) -> Option<LaunchComputeBinding> {
    let normalized = capability
        .trim()
        .to_ascii_lowercase()
        .replace(['.', '-'], "_");
    match (backend, normalized.as_str()) {
        (LocalInferenceBackend::GptOss, "text_generation") => Some(LaunchComputeBinding {
            product_id: PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID,
            backend_family: ComputeBackendFamily::GptOss,
            compute_family: ComputeFamily::Inference,
            model_policy: "psionic.local.inference.gpt_oss.single_node.launch",
        }),
        (LocalInferenceBackend::AppleFoundationModels, "text_generation") => {
            Some(LaunchComputeBinding {
                product_id: PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID,
                backend_family: ComputeBackendFamily::AppleFoundationModels,
                compute_family: ComputeFamily::Inference,
                model_policy: "psionic.local.inference.apple_foundation_models.single_node.launch",
            })
        }
        _ => None,
    }
}

fn compute_binding_for_product_id(product_id: &str) -> Option<LaunchComputeBinding> {
    match canonical_compute_product_id(product_id)? {
        PSIONIC_LOCAL_GPT_OSS_INFERENCE_PRODUCT_ID => compute_binding_for_backend_and_capability(
            LocalInferenceBackend::GptOss,
            "text_generation",
        ),
        PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID => compute_binding_for_backend_and_capability(
            LocalInferenceBackend::AppleFoundationModels,
            "text_generation",
        ),
        _ => None,
    }
}

fn selected_launch_compute_binding_for_request(
    state: &RenderState,
    request: &JobInboxRequest,
) -> Option<LaunchComputeBinding> {
    let normalized = request
        .capability
        .trim()
        .to_ascii_lowercase()
        .replace(['.', '-'], "_");
    match normalized.as_str() {
        "text_generation" => {
            let candidate_backends = [
                LocalInferenceBackend::AppleFoundationModels,
                LocalInferenceBackend::GptOss,
            ];
            candidate_backends.into_iter().find_map(|backend| {
                let backend_ready = match backend {
                    LocalInferenceBackend::AppleFoundationModels => {
                        state.provider_runtime.apple_fm.is_ready()
                    }
                    LocalInferenceBackend::GptOss => state.provider_runtime.gpt_oss.is_ready(),
                };
                if !backend_ready {
                    return None;
                }
                let binding =
                    compute_binding_for_backend_and_capability(backend, "text_generation")?;
                state
                    .provider_runtime
                    .product_enabled(binding.product_id)
                    .then_some(binding)
            })
        }
        _ => None,
    }
}

fn compute_linkage_for_request(
    state: &RenderState,
    request_id: &str,
    capability: &str,
) -> Option<LaunchComputeLinkage> {
    let normalized = capability
        .trim()
        .to_ascii_lowercase()
        .replace(['.', '-'], "_");
    let binding = match normalized.as_str() {
        "text_generation" => state
            .provider_runtime
            .active_inference_backend()
            .and_then(|backend| compute_binding_for_backend_and_capability(backend, capability))?,
        _ => return None,
    };
    let session_started_at_ms = state.provider_runtime.inventory_session_started_at_ms?;
    let provider_id = provider_id_for_state(state);
    Some(LaunchComputeLinkage {
        product_id: binding.product_id,
        capacity_lot_id: online_capacity_lot_id_for_binding(
            provider_id.as_str(),
            binding.product_id,
            session_started_at_ms,
        ),
        capacity_instrument_id: capacity_instrument_id_for_request(request_id),
        delivery_proof_id: delivery_proof_id_for_request(request_id),
        compute_family: binding.compute_family,
    })
}

fn compute_linkage_for_active_job(job: &ActiveJobRecord) -> Option<LaunchComputeLinkage> {
    let binding = job
        .compute_product_id
        .as_deref()
        .and_then(compute_binding_for_product_id)?;
    Some(LaunchComputeLinkage {
        product_id: binding.product_id,
        capacity_lot_id: job.capacity_lot_id.clone()?,
        capacity_instrument_id: job.capacity_instrument_id.clone()?,
        delivery_proof_id: job
            .delivery_proof_id
            .clone()
            .unwrap_or_else(|| delivery_proof_id_for_request(job.request_id.as_str())),
        compute_family: binding.compute_family,
    })
}

fn provider_inventory_evidence_refs(state: &RenderState) -> Vec<EvidenceRef> {
    let provider_id = provider_id_for_state(state);
    let mut evidence = vec![evidence_ref(
        "provider_identity_ref",
        format!("oa://autopilot/providers/{provider_id}"),
        provider_id.as_str(),
    )];
    if let Some(model_name) = state.provider_runtime.gpt_oss.ready_model.as_deref() {
        evidence.push(evidence_ref(
            "attestation:model_version",
            format!(
                "oa://autopilot/provider/gpt_oss/models/{}",
                canonical_kernel_id_component(model_name)
            ),
            model_name,
        ));
    }
    if let Some(model_name) = state.provider_runtime.apple_fm.ready_model.as_deref() {
        evidence.push(evidence_ref(
            "attestation:model_version",
            format!(
                "oa://autopilot/provider/apple_foundation_models/models/{}",
                canonical_kernel_id_component(model_name)
            ),
            model_name,
        ));
    }
    let base_url = state.gpt_oss_execution.base_url.trim();
    if !base_url.is_empty() {
        evidence.push(evidence_ref(
            "execution_backend_ref",
            "oa://autopilot/provider/gpt_oss/backend",
            base_url,
        ));
    }
    let apple_base_url = state.apple_fm_execution.base_url.trim();
    if !apple_base_url.is_empty() {
        evidence.push(evidence_ref(
            "execution_backend_ref",
            "oa://autopilot/provider/apple_foundation_models/backend",
            apple_base_url,
        ));
    }
    evidence
}

fn provider_id_for_state(state: &RenderState) -> String {
    state
        .nostr_identity
        .as_ref()
        .map(|identity| identity.npub.clone())
        .unwrap_or_else(|| "autopilot.desktop".to_string())
}

fn provider_inventory_active_job_matches(
    state: &RenderState,
    target: ProviderInventoryProductToggleTarget,
) -> bool {
    state
        .active_job
        .job
        .as_ref()
        .and_then(|job| {
            job.compute_product_id
                .as_deref()
                .map(str::to_string)
                .or_else(|| {
                    compute_linkage_for_request(
                        state,
                        job.request_id.as_str(),
                        job.capability.as_str(),
                    )
                    .map(|linkage| linkage.product_id.to_string())
                })
        })
        .is_some_and(|product_id| product_id == target.product_id())
}

fn provider_inventory_source_badge(
    state: &RenderState,
    target: ProviderInventoryProductToggleTarget,
    eligible: bool,
) -> &'static str {
    if !state
        .provider_runtime
        .inventory_controls
        .is_advertised(target)
    {
        "disabled.local_policy"
    } else if eligible
        && state
            .provider_runtime
            .inventory_session_started_at_ms
            .is_some()
        && matches!(
            state.provider_runtime.mode,
            crate::app_state::ProviderMode::Online | crate::app_state::ProviderMode::Degraded
        )
    {
        "desktop.go_online"
    } else {
        "desktop.local_preview"
    }
}

fn provider_inventory_delivery_state(
    state: &RenderState,
    target: ProviderInventoryProductToggleTarget,
) -> &'static str {
    if !state
        .provider_runtime
        .inventory_controls
        .is_advertised(target)
    {
        return "disabled";
    }
    if !state
        .provider_runtime
        .derived_inventory_products()
        .into_iter()
        .find(|product| product.product == target)
        .is_some_and(|product| product.backend_ready)
    {
        return "backend_unavailable";
    }
    if let Some(job) = state.active_job.job.as_ref()
        && provider_inventory_active_job_matches(state, target)
    {
        return job.stage.label();
    }
    if matches!(
        state.provider_runtime.mode,
        crate::app_state::ProviderMode::Offline
    ) {
        "offline"
    } else {
        "idle"
    }
}

#[derive(Clone, Debug)]
struct QuoteableComputeProduct {
    compute_family: ComputeFamily,
    backend_family: Option<ComputeBackendFamily>,
    execution_kind: ComputeExecutionKind,
    topology_kind: ComputeTopologyKind,
    provisioning_kind: ComputeProvisioningKind,
    proof_posture: ComputeProofPosture,
    capability_summary: String,
}

fn spot_rfq_matches_envelope(
    rfq: &SpotComputeRfqDraft,
    envelope: &ComputeCapabilityEnvelope,
    compute_family: ComputeFamily,
    backend_family: Option<ComputeBackendFamily>,
) -> bool {
    if compute_family != rfq.compute_family {
        return false;
    }
    if let Some(preferred_backend) = rfq.preferred_backend
        && backend_family != Some(preferred_backend)
    {
        return false;
    }
    spot_rfq_matches_host_capability(
        &rfq.capability_constraints,
        envelope.host_capability.as_ref(),
    ) && spot_rfq_matches_model_constraints(&rfq.capability_constraints, envelope)
        && rfq
            .capability_constraints
            .max_latency_ms
            .is_none_or(|max_latency_ms| {
                envelope
                    .latency_ms_p50
                    .is_some_and(|latency_ms| latency_ms <= max_latency_ms)
            })
        && rfq
            .capability_constraints
            .min_throughput_per_minute
            .is_none_or(|min_throughput| {
                envelope
                    .throughput_per_minute
                    .is_some_and(|throughput| throughput >= min_throughput)
            })
}

fn quoteable_compute_product(product: &ComputeProduct) -> Option<QuoteableComputeProduct> {
    let envelope = product.capability_envelope.as_ref()?;
    let compute_family = inferred_compute_family(product, envelope)?;
    let backend_family = inferred_backend_family(product, envelope);
    let execution_kind = inferred_execution_kind(product, envelope, compute_family);
    let topology_kind = inferred_topology_kind(envelope, execution_kind);
    let provisioning_kind = inferred_provisioning_kind(envelope, execution_kind);
    let proof_posture = inferred_proof_posture(envelope, execution_kind);
    Some(QuoteableComputeProduct {
        compute_family,
        backend_family,
        execution_kind,
        topology_kind,
        provisioning_kind,
        proof_posture,
        capability_summary: capability_summary_for_product(
            product,
            envelope,
            backend_family,
            compute_family,
            execution_kind,
            topology_kind,
            provisioning_kind,
            proof_posture,
        ),
    })
}

fn inferred_compute_family(
    product: &ComputeProduct,
    envelope: &ComputeCapabilityEnvelope,
) -> Option<ComputeFamily> {
    envelope.compute_family.or_else(|| {
        ProviderComputeProduct::for_product_id(product.product_id.as_str()).map(|product| {
            match product {
                ProviderComputeProduct::GptOssInference
                | ProviderComputeProduct::AppleFoundationModelsInference => {
                    ComputeFamily::Inference
                }
                ProviderComputeProduct::GptOssEmbeddings => ComputeFamily::Embeddings,
                ProviderComputeProduct::SandboxContainerExec
                | ProviderComputeProduct::SandboxPythonExec
                | ProviderComputeProduct::SandboxNodeExec
                | ProviderComputeProduct::SandboxPosixExec => ComputeFamily::SandboxExecution,
            }
        })
    })
}

fn inferred_backend_family(
    product: &ComputeProduct,
    envelope: &ComputeCapabilityEnvelope,
) -> Option<ComputeBackendFamily> {
    envelope.backend_family.or_else(|| {
        ProviderComputeProduct::for_product_id(product.product_id.as_str()).and_then(|product| {
            match product {
                ProviderComputeProduct::GptOssInference
                | ProviderComputeProduct::GptOssEmbeddings => Some(ComputeBackendFamily::GptOss),
                ProviderComputeProduct::AppleFoundationModelsInference => {
                    Some(ComputeBackendFamily::AppleFoundationModels)
                }
                ProviderComputeProduct::SandboxContainerExec
                | ProviderComputeProduct::SandboxPythonExec
                | ProviderComputeProduct::SandboxNodeExec
                | ProviderComputeProduct::SandboxPosixExec => None,
            }
        })
    })
}

fn inferred_execution_kind(
    product: &ComputeProduct,
    envelope: &ComputeCapabilityEnvelope,
    compute_family: ComputeFamily,
) -> ComputeExecutionKind {
    envelope.execution_kind.unwrap_or_else(|| {
        ProviderComputeProduct::for_product_id(product.product_id.as_str())
            .map(|product| match product {
                ProviderComputeProduct::GptOssInference
                | ProviderComputeProduct::GptOssEmbeddings
                | ProviderComputeProduct::AppleFoundationModelsInference => {
                    ComputeExecutionKind::LocalInference
                }
                ProviderComputeProduct::SandboxContainerExec
                | ProviderComputeProduct::SandboxPythonExec
                | ProviderComputeProduct::SandboxNodeExec
                | ProviderComputeProduct::SandboxPosixExec => {
                    ComputeExecutionKind::SandboxExecution
                }
            })
            .unwrap_or(match compute_family {
                ComputeFamily::SandboxExecution => ComputeExecutionKind::SandboxExecution,
                ComputeFamily::Evaluation => ComputeExecutionKind::EvaluationRun,
                ComputeFamily::Training => ComputeExecutionKind::TrainingJob,
                ComputeFamily::Inference
                | ComputeFamily::Embeddings
                | ComputeFamily::AdapterHosting => ComputeExecutionKind::LocalInference,
            })
    })
}

fn inferred_topology_kind(
    envelope: &ComputeCapabilityEnvelope,
    execution_kind: ComputeExecutionKind,
) -> ComputeTopologyKind {
    envelope.topology_kind.unwrap_or(match execution_kind {
        ComputeExecutionKind::LocalInference | ComputeExecutionKind::EvaluationRun => {
            ComputeTopologyKind::SingleNode
        }
        ComputeExecutionKind::ClusteredInference => ComputeTopologyKind::RemoteWholeRequest,
        ComputeExecutionKind::SandboxExecution => ComputeTopologyKind::SandboxIsolated,
        ComputeExecutionKind::TrainingJob => ComputeTopologyKind::TrainingElastic,
    })
}

fn inferred_provisioning_kind(
    envelope: &ComputeCapabilityEnvelope,
    execution_kind: ComputeExecutionKind,
) -> ComputeProvisioningKind {
    envelope.provisioning_kind.unwrap_or(match execution_kind {
        ComputeExecutionKind::LocalInference | ComputeExecutionKind::EvaluationRun => {
            ComputeProvisioningKind::DesktopLocal
        }
        ComputeExecutionKind::ClusteredInference => ComputeProvisioningKind::ClusterAttached,
        ComputeExecutionKind::SandboxExecution => ComputeProvisioningKind::RemoteSandbox,
        ComputeExecutionKind::TrainingJob => ComputeProvisioningKind::ReservedClusterWindow,
    })
}

fn inferred_proof_posture(
    envelope: &ComputeCapabilityEnvelope,
    execution_kind: ComputeExecutionKind,
) -> ComputeProofPosture {
    envelope.proof_posture.unwrap_or(match execution_kind {
        ComputeExecutionKind::SandboxExecution => ComputeProofPosture::TopologyAndDelivery,
        ComputeExecutionKind::ClusteredInference | ComputeExecutionKind::TrainingJob => {
            ComputeProofPosture::ChallengeEligible
        }
        ComputeExecutionKind::LocalInference | ComputeExecutionKind::EvaluationRun => {
            ComputeProofPosture::DeliveryProofOnly
        }
    })
}

fn effective_environment_binding(
    lot: &CapacityLot,
    envelope: &ComputeCapabilityEnvelope,
) -> Option<ComputeEnvironmentBinding> {
    lot.environment_binding
        .clone()
        .or_else(|| envelope.environment_binding.clone())
}

fn sandbox_profile_ref_for_quote(product: &ComputeProduct, lot: &CapacityLot) -> Option<String> {
    lot.metadata
        .get("sandbox_profile_ref")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            product
                .metadata
                .get("sandbox_profile_ref")
                .and_then(serde_json::Value::as_str)
        })
        .map(ToString::to_string)
}

fn normalized_constraint_label(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['.', '-', ' '], "_")
}

fn spot_rfq_matches_quote_posture(
    constraints: &SpotComputeCapabilityConstraints,
    quoteable: &QuoteableComputeProduct,
    environment_binding: Option<&ComputeEnvironmentBinding>,
    sandbox_profile_ref: Option<&str>,
) -> bool {
    if let Some(expected_topology) = constraints.topology_kind.as_deref()
        && normalized_constraint_label(expected_topology) != quoteable.topology_kind.label()
    {
        return false;
    }
    if let Some(expected_proof_posture) = constraints.proof_posture.as_deref()
        && normalized_constraint_label(expected_proof_posture) != quoteable.proof_posture.label()
    {
        return false;
    }
    if let Some(expected_environment_ref) = constraints.environment_ref.as_deref()
        && environment_binding.map(|binding| binding.environment_ref.as_str())
            != Some(expected_environment_ref)
    {
        return false;
    }
    if let Some(expected_profile_ref) = constraints.sandbox_profile_ref.as_deref()
        && sandbox_profile_ref != Some(expected_profile_ref)
    {
        return false;
    }
    true
}

fn spot_rfq_matches_host_capability(
    constraints: &SpotComputeCapabilityConstraints,
    host_capability: Option<&ComputeHostCapability>,
) -> bool {
    if constraints.accelerator_vendor.is_none()
        && constraints.accelerator_family.is_none()
        && constraints.min_memory_gb.is_none()
    {
        return true;
    }
    let Some(host_capability) = host_capability else {
        return false;
    };
    if let Some(expected_vendor) = constraints.accelerator_vendor.as_deref()
        && host_capability.accelerator_vendor.as_deref() != Some(expected_vendor)
    {
        return false;
    }
    if let Some(expected_family) = constraints.accelerator_family.as_deref()
        && host_capability.accelerator_family.as_deref() != Some(expected_family)
    {
        return false;
    }
    if let Some(min_memory_gb) = constraints.min_memory_gb
        && host_capability
            .memory_gb
            .is_none_or(|memory_gb| memory_gb < min_memory_gb)
    {
        return false;
    }
    true
}

fn spot_rfq_matches_model_constraints(
    constraints: &SpotComputeCapabilityConstraints,
    envelope: &ComputeCapabilityEnvelope,
) -> bool {
    if let Some(model_policy) = constraints.model_policy.as_deref()
        && envelope.model_policy.as_deref() != Some(model_policy)
    {
        return false;
    }
    if let Some(model_family) = constraints.model_family.as_deref()
        && envelope.model_family.as_deref() != Some(model_family)
    {
        return false;
    }
    true
}

fn spot_lot_is_quotable(rfq: &SpotComputeRfqDraft, lot: &CapacityLot) -> bool {
    if matches!(
        lot.status,
        CapacityLotStatus::Cancelled | CapacityLotStatus::Expired | CapacityLotStatus::Delivered
    ) {
        return false;
    }
    let now_ms = current_epoch_ms();
    if lot.offer_expires_at_ms < now_ms {
        return false;
    }
    let requested_end_ms =
        now_ms.saturating_add((rfq.window_minutes as i64).saturating_mul(60_000));
    requested_end_ms <= lot.delivery_end_ms
}

fn reserved_quantity_for_lot(instruments: &[CapacityInstrument], capacity_lot_id: &str) -> u64 {
    instruments
        .iter()
        .filter(|instrument| instrument.capacity_lot_id.as_deref() == Some(capacity_lot_id))
        .filter(|instrument| {
            !matches!(
                instrument.status,
                CapacityInstrumentStatus::Settled
                    | CapacityInstrumentStatus::Defaulted
                    | CapacityInstrumentStatus::Cancelled
                    | CapacityInstrumentStatus::Expired
            )
        })
        .map(|instrument| instrument.quantity)
        .sum()
}

fn money_as_sats(money: &Money) -> Option<u64> {
    if money.asset != Asset::Btc {
        return None;
    }
    match money.amount {
        MoneyAmount::AmountSats(value) => Some(value),
        MoneyAmount::AmountMsats(value) => Some(value.saturating_add(999) / 1000),
    }
}

fn quote_source_badge(product: &ComputeProduct, lot: &CapacityLot) -> String {
    lot.metadata
        .get("source_badge")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            lot.metadata
                .get("source")
                .and_then(serde_json::Value::as_str)
        })
        .or_else(|| {
            product
                .metadata
                .get("source")
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("kernel.authority")
        .to_string()
}

fn quote_terms_label(product: &ComputeProduct, lot: &CapacityLot) -> String {
    lot.metadata
        .get("terms_label")
        .and_then(serde_json::Value::as_str)
        .or_else(|| product.sla_terms_ref.as_deref())
        .unwrap_or("spot session / local best effort")
        .to_string()
}

fn capability_summary_for_product(
    product: &ComputeProduct,
    envelope: &ComputeCapabilityEnvelope,
    backend_family: Option<ComputeBackendFamily>,
    compute_family: ComputeFamily,
    execution_kind: ComputeExecutionKind,
    topology_kind: ComputeTopologyKind,
    provisioning_kind: ComputeProvisioningKind,
    proof_posture: ComputeProofPosture,
) -> String {
    let model_policy = envelope.model_policy.as_deref().unwrap_or("none");
    let model_family = envelope.model_family.as_deref().unwrap_or("none");
    let latency_ms = envelope
        .latency_ms_p50
        .map(|latency| latency.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let throughput = envelope
        .throughput_per_minute
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let environment_ref = envelope
        .environment_binding
        .as_ref()
        .map(|binding| binding.environment_ref.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("none");
    let sandbox_profile_ref = product
        .metadata
        .get("sandbox_profile_ref")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("none");
    let host_summary = envelope
        .host_capability
        .as_ref()
        .map(|host_capability| {
            format!(
                " accelerator_vendor={} accelerator_family={} memory_gb={}",
                host_capability
                    .accelerator_vendor
                    .as_deref()
                    .unwrap_or("n/a"),
                host_capability
                    .accelerator_family
                    .as_deref()
                    .unwrap_or("n/a"),
                host_capability
                    .memory_gb
                    .map(|memory| memory.to_string())
                    .unwrap_or_else(|| "n/a".to_string())
            )
        })
        .unwrap_or_default();
    format!(
        "backend={} execution={} family={} topology={} provisioning={} proof={} model_policy={} model_family={} latency_ms_p50={} throughput_per_minute={} environment_ref={} sandbox_profile_ref={}{}",
        backend_family_label(backend_family, compute_family, execution_kind),
        execution_kind_label(execution_kind),
        compute_family_label(compute_family),
        topology_kind.label(),
        provisioning_kind.label(),
        proof_posture.label(),
        model_policy,
        model_family,
        latency_ms,
        throughput,
        environment_ref,
        sandbox_profile_ref,
        host_summary
    )
}

fn execution_kind_label(execution_kind: ComputeExecutionKind) -> &'static str {
    match execution_kind {
        ComputeExecutionKind::LocalInference => "local_inference",
        ComputeExecutionKind::ClusteredInference => "clustered_inference",
        ComputeExecutionKind::SandboxExecution => "sandbox_execution",
        ComputeExecutionKind::EvaluationRun => "evaluation_run",
        ComputeExecutionKind::TrainingJob => "training_job",
    }
}

fn backend_family_label(
    backend_family: Option<ComputeBackendFamily>,
    compute_family: ComputeFamily,
    execution_kind: ComputeExecutionKind,
) -> &'static str {
    match backend_family {
        Some(ComputeBackendFamily::GptOss) => "gpt_oss",
        Some(ComputeBackendFamily::AppleFoundationModels) => "apple_foundation_models",
        None if matches!(compute_family, ComputeFamily::SandboxExecution)
            || matches!(execution_kind, ComputeExecutionKind::SandboxExecution) =>
        {
            "sandbox"
        }
        None => "unknown",
    }
}

fn compute_family_label(compute_family: ComputeFamily) -> &'static str {
    match compute_family {
        ComputeFamily::Inference => "inference",
        ComputeFamily::Embeddings => "embeddings",
        ComputeFamily::SandboxExecution => "sandbox_execution",
        ComputeFamily::Evaluation => "evaluation",
        ComputeFamily::Training => "training",
        ComputeFamily::AdapterHosting => "adapter_hosting",
    }
}

fn price_floor_sats_for_product_id(product_id: &str) -> u64 {
    ProviderInventoryProductToggleTarget::for_product_id(product_id)
        .map(ProviderInventoryProductToggleTarget::default_price_floor_sats)
        .unwrap_or(0)
}

fn terms_label_for_product_id(product_id: &str) -> &'static str {
    ProviderInventoryProductToggleTarget::for_product_id(product_id)
        .map(ProviderInventoryProductToggleTarget::terms_label)
        .unwrap_or("spot session / local best effort")
}

fn forward_terms_label_for_product_id(product_id: &str) -> &'static str {
    ProviderInventoryProductToggleTarget::for_product_id(product_id)
        .map(ProviderInventoryProductToggleTarget::forward_terms_label)
        .unwrap_or("forward physical / committed local window")
}

fn forward_remedy_profile_for_product_id(product_id: &str) -> &'static str {
    match canonical_compute_product_id(product_id).unwrap_or(product_id) {
        "psionic.remote_sandbox.sandbox_execution.container_exec.sandbox_isolated"
        | "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
        | "psionic.remote_sandbox.sandbox_execution.node_exec.sandbox_isolated"
        | "psionic.remote_sandbox.sandbox_execution.posix_exec.sandbox_isolated" => {
            "forward_physical.sandbox.v1"
        }
        PSIONIC_LOCAL_APPLE_FM_INFERENCE_PRODUCT_ID => "forward_physical.apple_fm.v1",
        _ => "forward_physical.inference.v1",
    }
}

fn ready_model_for_binding(state: &RenderState, binding: LaunchComputeBinding) -> Option<String> {
    match binding.backend_family {
        ComputeBackendFamily::GptOss => state.provider_runtime.gpt_oss.ready_model.clone(),
        ComputeBackendFamily::AppleFoundationModels => {
            state.provider_runtime.apple_fm.ready_model.clone()
        }
    }
}

fn configured_model_for_binding(
    state: &RenderState,
    binding: LaunchComputeBinding,
) -> Option<String> {
    match binding.backend_family {
        ComputeBackendFamily::GptOss => state.provider_runtime.gpt_oss.configured_model.clone(),
        ComputeBackendFamily::AppleFoundationModels => {
            state.provider_runtime.apple_fm.ready_model.clone()
        }
    }
}

fn work_unit_id_for_request(request_id: &str) -> String {
    format!("work_unit.{}", canonical_kernel_id_component(request_id))
}

fn contract_id_for_request(request_id: &str) -> String {
    format!("contract.{}", canonical_kernel_id_component(request_id))
}

fn online_capacity_lot_id_for_binding(
    provider_id: &str,
    product_id: &str,
    session_started_at_ms: i64,
) -> String {
    format!(
        "lot.online.{}.{}.{}",
        canonical_kernel_id_component(provider_id),
        canonical_kernel_id_component(product_id),
        session_started_at_ms.max(0)
    )
}

fn forward_capacity_lot_id_for_binding(
    provider_id: &str,
    product_id: &str,
    session_started_at_ms: i64,
) -> String {
    format!(
        "lot.forward.{}.{}.{}",
        canonical_kernel_id_component(provider_id),
        canonical_kernel_id_component(product_id),
        session_started_at_ms.max(0)
    )
}

fn capacity_instrument_id_for_request(request_id: &str) -> String {
    format!("instrument.{}", canonical_kernel_id_component(request_id))
}

fn delivery_proof_id_for_request(request_id: &str) -> String {
    format!("delivery.{}", canonical_kernel_id_component(request_id))
}

fn canonical_kernel_id_component(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    trimmed
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn local_projection_receipt_id(kind: &str, request_id: &str) -> String {
    format!(
        "projection.{}.{}",
        canonical_kernel_id_component(kind),
        canonical_kernel_id_component(request_id)
    )
}

pub(crate) fn is_local_projection_receipt_id(receipt_id: &str) -> bool {
    receipt_id.starts_with("projection.")
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().min(i64::MAX as u128) as i64
        })
}

pub(crate) fn reset_request_decision_after_kernel_error(
    state: &mut RenderState,
    request_id: &str,
    error: &str,
) {
    if let Some(request) = state
        .job_inbox
        .requests
        .iter_mut()
        .find(|request| request.request_id == request_id)
    {
        request.decision = JobInboxDecision::Pending;
    }
    state.job_inbox.last_error = Some(error.to_string());
    state.job_inbox.last_action = Some(format!(
        "Kernel authority rejected acceptance for {}",
        request_id
    ));
    state.job_inbox.load_state = crate::app_state::PaneLoadState::Error;
}

#[cfg(test)]
mod tests {
    use super::{
        KernelAuthorityMode, LaunchDeliveryContext, PendingSseEvent, ReceiptProjectionEnvelope,
        build_compute_product_request, build_forward_compute_quotes_from_market,
        build_spot_compute_quotes_from_market, compute_binding_for_backend_and_capability,
        compute_binding_for_product_id, compute_linkage_for_active_job, consume_sse_buffer,
        current_epoch_ms, delivery_proof_id_for_request, evaluate_delivery_proof,
        flush_pending_sse_event, forward_capacity_lot_id_for_binding,
        is_local_projection_receipt_id, local_projection_receipt_id,
        online_capacity_lot_id_for_binding, resolve_kernel_authority_mode,
        submission_evidence_refs,
    };
    use crate::app_state::{ActiveJobRecord, JobDemandSource, JobLifecycleStage};
    use crate::app_state::{
        ForwardComputeRfqDraft, SpotComputeCapabilityConstraints, SpotComputeRfqDraft,
    };
    use crate::economy_kernel_receipts::{
        PolicyContext, ReceiptBuilder, ReceiptHints, TraceContext,
    };
    use crate::state::provider_runtime::LocalInferenceBackend;
    use openagents_kernel_core::compute::{
        CapacityInstrument, CapacityInstrumentStatus, CapacityLot, CapacityLotStatus,
        CapacityReserveState, ComputeBackendFamily, ComputeCapabilityEnvelope,
        ComputeDeliveryVarianceReason, ComputeEnvironmentBinding, ComputeExecutionKind,
        ComputeFamily, ComputeProduct, ComputeProductStatus, ComputeProofPosture,
        ComputeProvisioningKind, ComputeSettlementMode, ComputeTopologyKind, DeliveryProofStatus,
        GptOssRuntimeCapability,
    };
    use openagents_kernel_core::receipts::{Asset, Money, MoneyAmount};
    use serde_json::json;
    use std::sync::mpsc;

    fn fixture_receipt() -> crate::economy_kernel_receipts::Receipt {
        ReceiptBuilder::new(
            "receipt.kernel.test:1",
            "kernel.test.v1",
            1_762_000_000_000,
            "idempotency:test",
            TraceContext::default(),
            PolicyContext {
                policy_bundle_id: "policy.test".to_string(),
                policy_version: "1".to_string(),
                approved_by: "tester".to_string(),
            },
        )
        .with_hints(ReceiptHints::default())
        .build()
        .expect("fixture receipt")
    }

    fn fixture_active_job_with_gpt_oss_provenance() -> ActiveJobRecord {
        ActiveJobRecord {
            job_id: "job-gpt_oss-001".to_string(),
            request_id: "req-gpt_oss-001".to_string(),
            requester: "npub1buyer".to_string(),
            source_relay_url: None,
            demand_source: JobDemandSource::OpenNetwork,
            demand_risk_class: crate::app_state::JobDemandRiskClass::SpeculativeOpenNetwork,
            demand_risk_disposition: crate::app_state::JobDemandRiskDisposition::ManualReviewOnly,
            demand_risk_note:
                "untargeted open-network demand stays visible but requires manual review"
                    .to_string(),
            request_kind: nostr::nip90::KIND_JOB_TEXT_GENERATION,
            capability: "text.generation".to_string(),
            execution_input: Some("Write a haiku about rust".to_string()),
            execution_prompt: Some("Write a haiku about rust".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            execution_provenance: Some(
                crate::local_inference_runtime::LocalInferenceExecutionProvenance {
                    backend: "gpt_oss".to_string(),
                    requested_model: Some("llama3.2:latest".to_string()),
                    served_model: "llama3.2:latest".to_string(),
                    normalized_prompt_digest: "sha256:prompt".to_string(),
                    normalized_options_json: "{\"num_predict\":64}".to_string(),
                    normalized_options_digest: "sha256:options".to_string(),
                    base_url: "http://127.0.0.1:11434".to_string(),
                    total_duration_ns: Some(1_000_000),
                    load_duration_ns: Some(0),
                    prompt_token_count: Some(11),
                    generated_token_count: Some(7),
                    warm_start: Some(true),
                },
            ),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("req-gpt_oss-001".to_string()),
            sa_tick_result_event_id: Some("result-gpt_oss-001".to_string()),
            sa_trajectory_session_id: Some("traj:req-gpt_oss-001".to_string()),
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            compute_product_id: Some("psionic.local.inference.gpt_oss.single_node".to_string()),
            capacity_lot_id: Some(
                "lot.online.npub1buyer.psionic.local.inference.gpt_oss.single_node.1762000000000"
                    .to_string(),
            ),
            capacity_instrument_id: Some("instrument.req-gpt_oss-001".to_string()),
            delivery_proof_id: Some("delivery.req-gpt_oss-001".to_string()),
            delivery_metering_rule_id: None,
            delivery_proof_status_label: None,
            delivery_metered_quantity: None,
            delivery_accepted_quantity: None,
            delivery_variance_reason_label: None,
            delivery_rejection_reason_label: None,
            quoted_price_sats: 21,
            ttl_seconds: 90,
            request_created_at_epoch_seconds: Some(1_760_000_000),
            request_expires_at_epoch_seconds: Some(1_760_000_090),
            accepted_at_epoch_seconds: Some(1_760_000_015),
            stage: JobLifecycleStage::Delivered,
            invoice_id: None,
            settlement_bolt11: None,
            settlement_payment_hash: None,
            payment_id: None,
            failure_reason: None,
            events: Vec::new(),
        }
    }

    fn fixture_delivery_context(output: &str) -> LaunchDeliveryContext {
        LaunchDeliveryContext {
            execution_output: Some(output.to_string()),
            provider_thread_id: Some("thread-1".to_string()),
            provider_turn_id: Some("turn-1".to_string()),
            gpt_oss_ready_model: Some("llama3.2:latest".to_string()),
            gpt_oss_metrics: Some(
                crate::local_inference_runtime::LocalInferenceExecutionMetrics {
                    total_duration_ns: Some(1_000_000_000),
                    load_duration_ns: Some(10_000_000),
                    prompt_eval_count: Some(11),
                    prompt_eval_duration_ns: Some(100_000_000),
                    eval_count: Some(7),
                    eval_duration_ns: Some(900_000_000),
                },
            ),
            apple_ready_model: Some("apple-foundation-model".to_string()),
            apple_metrics: None,
            apple_model_available: true,
            apple_bridge_status: Some("running".to_string()),
        }
    }

    fn fixture_compute_product(product_id: &str, compute_family: ComputeFamily) -> ComputeProduct {
        ComputeProduct {
            product_id: product_id.to_string(),
            resource_class: "compute".to_string(),
            capacity_unit: "request".to_string(),
            window_spec: "session".to_string(),
            region_spec: vec!["local".to_string()],
            performance_band: Some("desktop-local".to_string()),
            sla_terms_ref: Some("sla.autopilot.best_effort".to_string()),
            cost_proof_required: false,
            attestation_required: false,
            settlement_mode: ComputeSettlementMode::Physical,
            index_eligible: false,
            status: ComputeProductStatus::Active,
            version: "v1".to_string(),
            created_at_ms: 1_762_000_000_000,
            taxonomy_version: Some(
                openagents_kernel_core::compute::COMPUTE_LAUNCH_TAXONOMY_VERSION.to_string(),
            ),
            capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: Some(ComputeBackendFamily::GptOss),
                execution_kind: Some(ComputeExecutionKind::LocalInference),
                compute_family: Some(compute_family),
                topology_kind: None,
                provisioning_kind: None,
                proof_posture: None,
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: None,
                checkpoint_binding: None,
                model_policy: Some(product_id.to_string()),
                model_family: Some("nomic-embed-text".to_string()),
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: Some(GptOssRuntimeCapability {
                    runtime_ready: Some(true),
                    model_name: Some("nomic-embed-text".to_string()),
                    quantization: Some("q4_k_m".to_string()),
                }),
                latency_ms_p50: Some(240),
                throughput_per_minute: Some(4000),
                concurrency_limit: Some(2),
            }),
            metadata: json!({
                "source": "desktop.go_online",
                "source_badge": "desktop.go_online",
                "terms_label": "spot session / local best effort"
            }),
        }
    }

    fn fixture_capacity_lot(
        product_id: &str,
        quantity: u64,
        min_unit_price_sats: u64,
    ) -> CapacityLot {
        CapacityLot {
            capacity_lot_id: format!("lot.online.provider.{}", product_id.replace(':', "_")),
            product_id: product_id.to_string(),
            provider_id: "npub1provider".to_string(),
            delivery_start_ms: 0,
            delivery_end_ms: i64::MAX / 4,
            quantity,
            min_unit_price: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(min_unit_price_sats),
            }),
            region_hint: Some("local".to_string()),
            attestation_posture: Some("desktop.local.best_effort".to_string()),
            reserve_state: CapacityReserveState::Available,
            offer_expires_at_ms: i64::MAX / 4,
            status: CapacityLotStatus::Open,
            environment_binding: None,
            metadata: json!({
                "source_badge": "desktop.go_online",
                "terms_label": "spot session / local best effort"
            }),
        }
    }

    #[test]
    fn resolve_kernel_authority_mode_uses_remote_when_complete_config_present() {
        let mode =
            resolve_kernel_authority_mode(Some("https://control.example.com"), Some("token-123"));
        assert_eq!(
            mode,
            KernelAuthorityMode::Remote {
                base_url: "https://control.example.com".to_string(),
                bearer_auth: "token-123".to_string(),
            }
        );
    }

    #[test]
    fn local_projection_receipt_ids_are_tagged_and_sanitized() {
        let receipt_id = local_projection_receipt_id("verdict", "req:alpha/beta");

        assert_eq!(receipt_id, "projection.verdict.req_alpha_beta");
        assert!(is_local_projection_receipt_id(receipt_id.as_str()));
    }

    #[test]
    fn authoritative_receipt_ids_are_not_marked_local_projection() {
        assert!(!is_local_projection_receipt_id(
            "receipt.kernel.authority.verdict:123"
        ));
    }

    #[test]
    fn flush_pending_sse_event_decodes_named_receipt_event() {
        let receipt = fixture_receipt();
        let payload = serde_json::to_string(&ReceiptProjectionEnvelope {
            receipt: receipt.clone(),
        })
        .expect("payload");
        let (tx, rx) = mpsc::channel();
        let mut pending_event = PendingSseEvent {
            event_name: Some("receipt".to_string()),
            data_lines: vec![payload],
        };

        let error = flush_pending_sse_event(
            &mut pending_event,
            "receipt",
            &tx,
            |envelope: ReceiptProjectionEnvelope| {
                super::KernelProjectionUpdate::Receipt(envelope.receipt)
            },
        );
        assert!(error.is_none(), "unexpected parser error: {error:?}");
        match rx.try_recv().expect("receipt update") {
            super::KernelProjectionUpdate::Receipt(decoded) => {
                assert_eq!(decoded.receipt_id, receipt.receipt_id);
            }
            other => panic!("unexpected projection update: {other:?}"),
        }
    }

    #[test]
    fn consume_sse_buffer_ignores_keepalives_and_partial_lines() {
        let receipt = fixture_receipt();
        let payload = serde_json::to_string(&ReceiptProjectionEnvelope {
            receipt: receipt.clone(),
        })
        .expect("payload");
        let (tx, rx) = mpsc::channel();
        let mut pending_event = PendingSseEvent::default();
        let mut pending_buffer = format!(":keep-alive\n\nevent: receipt\ndata: {payload}\n");
        assert!(
            consume_sse_buffer::<ReceiptProjectionEnvelope, _>(
                &mut pending_buffer,
                &mut pending_event,
                "receipt",
                &tx,
                |envelope| super::KernelProjectionUpdate::Receipt(envelope.receipt),
            )
            .is_none()
        );
        assert!(rx.try_recv().is_err(), "blank line has not arrived yet");

        pending_buffer.push('\n');
        assert!(
            consume_sse_buffer::<ReceiptProjectionEnvelope, _>(
                &mut pending_buffer,
                &mut pending_event,
                "receipt",
                &tx,
                |envelope| super::KernelProjectionUpdate::Receipt(envelope.receipt),
            )
            .is_none()
        );
        match rx.try_recv().expect("receipt update") {
            super::KernelProjectionUpdate::Receipt(decoded) => {
                assert_eq!(decoded.receipt_id, receipt.receipt_id);
            }
            other => panic!("unexpected projection update: {other:?}"),
        }
    }

    #[test]
    fn submission_evidence_refs_include_gpt_oss_provenance() {
        let job = fixture_active_job_with_gpt_oss_provenance();
        let evidence = submission_evidence_refs(&job, Some("hello from gpt_oss"));

        assert!(evidence.iter().any(|row| {
            row.kind == "execution_backend_ref"
                && row.uri == "oa://autopilot/jobs/job-gpt_oss-001/execution/backend"
        }));
        assert!(evidence.iter().any(|row| {
            row.kind == "attestation:model_version"
                && row.uri == "oa://autopilot/jobs/job-gpt_oss-001/execution/model/llama3.2_latest"
        }));
        assert!(
            evidence.iter().any(|row| {
                row.kind == "execution_prompt_digest" && row.digest == "sha256:prompt"
            })
        );
        assert!(evidence.iter().any(|row| {
            row.kind == "execution_options_digest" && row.digest == "sha256:options"
        }));
    }

    #[test]
    fn compute_binding_maps_launch_capabilities_to_launch_products() {
        let inference = compute_binding_for_backend_and_capability(
            LocalInferenceBackend::GptOss,
            "text_generation",
        )
        .expect("inference binding");
        assert_eq!(
            inference.product_id,
            "psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(inference.compute_family, ComputeFamily::Inference);

        assert!(
            compute_binding_for_backend_and_capability(
                LocalInferenceBackend::GptOss,
                "text.embeddings",
            )
            .is_none()
        );
        assert!(compute_binding_for_product_id("gpt_oss.embeddings").is_none());

        let apple_inference = compute_binding_for_backend_and_capability(
            LocalInferenceBackend::AppleFoundationModels,
            "text_generation",
        )
        .expect("apple inference binding");
        assert_eq!(
            apple_inference.product_id,
            "psionic.local.inference.apple_foundation_models.single_node"
        );
        assert_eq!(apple_inference.compute_family, ComputeFamily::Inference);

        assert!(
            compute_binding_for_backend_and_capability(LocalInferenceBackend::GptOss, "gpu.h100")
                .is_none()
        );
    }

    #[test]
    fn launch_compute_product_requests_are_provider_independent() {
        let request = build_compute_product_request(
            compute_binding_for_backend_and_capability(
                LocalInferenceBackend::GptOss,
                "text_generation",
            )
            .expect("inference binding"),
        );
        assert_eq!(
            request.idempotency_key,
            "desktop.compute_product:psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(
            request.product.product_id,
            "psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(
            request.product.created_at_ms,
            super::LAUNCH_PRODUCT_CREATED_AT_MS
        );
        assert!(request.evidence.is_empty());
        assert_eq!(request.policy.approved_by, "openagents.compute.market");
    }

    #[test]
    fn apple_launch_compute_product_request_uses_apple_backend_family() {
        let request = build_compute_product_request(
            compute_binding_for_backend_and_capability(
                LocalInferenceBackend::AppleFoundationModels,
                "text_generation",
            )
            .expect("apple inference binding"),
        );
        assert_eq!(
            request.idempotency_key,
            "desktop.compute_product:psionic.local.inference.apple_foundation_models.single_node"
        );
        assert_eq!(
            request.product.product_id,
            "psionic.local.inference.apple_foundation_models.single_node"
        );
        assert_eq!(
            request
                .product
                .capability_envelope
                .as_ref()
                .and_then(|envelope| envelope.backend_family),
            Some(openagents_kernel_core::compute::ComputeBackendFamily::AppleFoundationModels)
        );
    }

    #[test]
    fn online_inventory_lot_ids_are_provider_and_session_scoped() {
        let lot_id = online_capacity_lot_id_for_binding(
            "npub1buyer",
            "psionic.local.inference.gpt_oss.single_node",
            1_762_000_000_000,
        );
        assert_eq!(
            lot_id,
            "lot.online.npub1buyer.psionic.local.inference.gpt_oss.single_node.1762000000000"
        );
    }

    #[test]
    fn forward_inventory_lot_ids_are_provider_and_session_scoped() {
        let lot_id = forward_capacity_lot_id_for_binding(
            "npub1buyer",
            "psionic.local.inference.gpt_oss.single_node",
            1_762_000_000_000,
        );
        assert_eq!(
            lot_id,
            "lot.forward.npub1buyer.psionic.local.inference.gpt_oss.single_node.1762000000000"
        );
    }

    #[test]
    fn compute_linkage_uses_active_job_inventory_and_request_instrument_ids() {
        let linkage = compute_linkage_for_active_job(&fixture_active_job_with_gpt_oss_provenance())
            .expect("linkage");
        assert_eq!(
            linkage.product_id,
            "psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(
            linkage.capacity_lot_id,
            "lot.online.npub1buyer.psionic.local.inference.gpt_oss.single_node.1762000000000"
        );
        assert_eq!(linkage.capacity_instrument_id, "instrument.req-gpt_oss-001");
        assert_eq!(
            linkage.delivery_proof_id,
            delivery_proof_id_for_request("req-gpt_oss-001")
        );
    }

    #[test]
    fn spot_quotes_only_return_matching_launch_family_and_available_supply() {
        let rfq = SpotComputeRfqDraft {
            rfq_id: "rfq-1".to_string(),
            compute_family: ComputeFamily::Inference,
            preferred_backend: Some(ComputeBackendFamily::GptOss),
            quantity: 2,
            window_minutes: 15,
            max_price_sats: 50,
            capability_constraints: SpotComputeCapabilityConstraints::default(),
        };
        let products = vec![
            fixture_compute_product("gpt_oss.embeddings", ComputeFamily::Embeddings),
            fixture_compute_product("gpt_oss.text_generation", ComputeFamily::Inference),
        ];
        let lots = vec![
            fixture_capacity_lot("gpt_oss.embeddings", 4, 8),
            fixture_capacity_lot("gpt_oss.text_generation", 4, 21),
        ];
        let instruments = vec![CapacityInstrument {
            instrument_id: "instrument.active.1".to_string(),
            product_id: "gpt_oss.text_generation".to_string(),
            capacity_lot_id: Some(lots[1].capacity_lot_id.clone()),
            buyer_id: Some("npub1buyer".to_string()),
            provider_id: Some("npub1provider".to_string()),
            delivery_start_ms: 0,
            delivery_end_ms: 1,
            quantity: 1,
            fixed_price: None,
            reference_index_id: None,
            kind: openagents_kernel_core::compute::CapacityInstrumentKind::Spot,
            settlement_mode: ComputeSettlementMode::Physical,
            created_at_ms: 0,
            status: CapacityInstrumentStatus::Active,
            environment_binding: None,
            closure_reason: None,
            non_delivery_reason: None,
            settlement_failure_reason: None,
            lifecycle_reason_detail: None,
            metadata: json!({}),
        }];

        let quotes = build_spot_compute_quotes_from_market(&rfq, &products, &lots, &instruments);
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].product_id, "gpt_oss.text_generation");
        assert_eq!(quotes[0].requested_quantity, 2);
        assert_eq!(quotes[0].available_quantity, 3);
        assert_eq!(quotes[0].price_sats, 42);
    }

    #[test]
    fn forward_quotes_only_return_future_matching_supply() {
        let rfq = ForwardComputeRfqDraft {
            rfq_id: "rfq-forward-1".to_string(),
            compute_family: ComputeFamily::Inference,
            preferred_backend: Some(ComputeBackendFamily::GptOss),
            quantity: 1,
            delivery_start_minutes: 180,
            window_minutes: 60,
            max_price_sats: 60,
            capability_constraints: SpotComputeCapabilityConstraints::default(),
        };
        let products = vec![fixture_compute_product(
            "gpt_oss.text_generation",
            ComputeFamily::Inference,
        )];
        let mut forward_lot = fixture_capacity_lot("gpt_oss.text_generation", 4, 21);
        let now_ms = current_epoch_ms();
        forward_lot.capacity_lot_id = "lot.forward.provider.gpt_oss.text_generation".to_string();
        forward_lot.delivery_start_ms = now_ms + 180 * 60_000;
        forward_lot.delivery_end_ms = forward_lot.delivery_start_ms + 60 * 60_000;
        forward_lot.metadata = json!({
            "source_badge": "desktop.forward_inventory",
            "terms_label": "forward physical / committed local window"
        });
        let lots = vec![forward_lot];
        let instruments = Vec::<CapacityInstrument>::new();

        let quotes = build_forward_compute_quotes_from_market(&rfq, &products, &lots, &instruments);
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].product_id, "gpt_oss.text_generation");
        assert_eq!(quotes[0].requested_quantity, 1);
        assert_eq!(quotes[0].source_badge, "desktop.forward_inventory");
    }

    #[test]
    fn sandbox_quotes_match_profile_environment_and_posture_constraints() {
        let rfq = SpotComputeRfqDraft {
            rfq_id: "rfq-sandbox-1".to_string(),
            compute_family: ComputeFamily::SandboxExecution,
            preferred_backend: None,
            quantity: 1,
            window_minutes: 30,
            max_price_sats: 80,
            capability_constraints: SpotComputeCapabilityConstraints {
                topology_kind: Some("sandbox_isolated".to_string()),
                proof_posture: Some("topology_and_delivery".to_string()),
                environment_ref: Some("env://sandbox/python".to_string()),
                sandbox_profile_ref: Some("python-batch".to_string()),
                ..SpotComputeCapabilityConstraints::default()
            },
        };
        let products = vec![ComputeProduct {
            product_id: "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
                .to_string(),
            resource_class: "compute".to_string(),
            capacity_unit: "job".to_string(),
            window_spec: "session".to_string(),
            region_spec: vec!["local".to_string()],
            performance_band: Some("sandbox".to_string()),
            sla_terms_ref: Some("sandbox best effort".to_string()),
            cost_proof_required: false,
            attestation_required: false,
            settlement_mode: ComputeSettlementMode::Physical,
            index_eligible: false,
            status: ComputeProductStatus::Active,
            version: "v1".to_string(),
            created_at_ms: 1_762_000_000_000,
            taxonomy_version: Some(
                openagents_kernel_core::compute::COMPUTE_LAUNCH_TAXONOMY_VERSION.to_string(),
            ),
            capability_envelope: Some(ComputeCapabilityEnvelope {
                backend_family: None,
                execution_kind: Some(ComputeExecutionKind::SandboxExecution),
                compute_family: Some(ComputeFamily::SandboxExecution),
                topology_kind: Some(ComputeTopologyKind::SandboxIsolated),
                provisioning_kind: Some(ComputeProvisioningKind::RemoteSandbox),
                proof_posture: Some(ComputeProofPosture::TopologyAndDelivery),
                validator_requirements: None,
                artifact_residency: None,
                environment_binding: Some(ComputeEnvironmentBinding {
                    environment_ref: "env://sandbox/python".to_string(),
                    environment_version: Some("v1".to_string()),
                    ..ComputeEnvironmentBinding::default()
                }),
                checkpoint_binding: None,
                model_policy: Some("sandbox.python.exec".to_string()),
                model_family: None,
                host_capability: None,
                apple_platform: None,
                gpt_oss_runtime: None,
                latency_ms_p50: Some(600),
                throughput_per_minute: Some(120),
                concurrency_limit: Some(1),
            }),
            metadata: json!({
                "source": "desktop.sandbox_inventory",
                "sandbox_profile_ref": "python-batch"
            }),
        }];
        let lots = vec![CapacityLot {
            capacity_lot_id: "lot.sandbox.python".to_string(),
            product_id: products[0].product_id.clone(),
            provider_id: "npub1sandbox".to_string(),
            delivery_start_ms: 0,
            delivery_end_ms: i64::MAX / 4,
            quantity: 2,
            min_unit_price: Some(Money {
                asset: Asset::Btc,
                amount: MoneyAmount::AmountSats(34),
            }),
            region_hint: Some("local".to_string()),
            attestation_posture: Some("sandbox".to_string()),
            reserve_state: CapacityReserveState::Available,
            offer_expires_at_ms: i64::MAX / 4,
            status: CapacityLotStatus::Open,
            environment_binding: Some(ComputeEnvironmentBinding {
                environment_ref: "env://sandbox/python".to_string(),
                environment_version: Some("v1".to_string()),
                ..ComputeEnvironmentBinding::default()
            }),
            metadata: json!({
                "source_badge": "desktop.sandbox_inventory",
                "terms_label": "spot session / declared sandbox profile",
                "sandbox_profile_ref": "python-batch"
            }),
        }];

        let quotes = build_spot_compute_quotes_from_market(&rfq, &products, &lots, &[]);

        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].backend_label(), "sandbox");
        assert_eq!(quotes[0].execution_label(), "sandbox_execution");
        assert_eq!(quotes[0].topology_label(), "sandbox_isolated");
        assert_eq!(quotes[0].proof_posture_label(), "topology_and_delivery");
        assert_eq!(quotes[0].environment_ref(), Some("env://sandbox/python"));
        assert_eq!(
            quotes[0].sandbox_profile_ref.as_deref(),
            Some("python-batch")
        );
    }

    #[test]
    fn delivery_evaluation_accepts_gpt_oss_inference_with_metering_rule() {
        let job = fixture_active_job_with_gpt_oss_provenance();
        let binding = compute_binding_for_backend_and_capability(
            LocalInferenceBackend::GptOss,
            "text_generation",
        )
        .expect("inference binding");

        let evaluation = evaluate_delivery_proof(&job, binding, &fixture_delivery_context("hello"));

        assert_eq!(evaluation.metering_rule_id, "meter.gpt_oss.inference.v1");
        assert_eq!(evaluation.status, DeliveryProofStatus::Accepted);
        assert_eq!(evaluation.metered_quantity, 1);
        assert_eq!(evaluation.accepted_quantity, 1);
        assert_eq!(evaluation.rejection_reason, None);
        assert_eq!(
            evaluation
                .observed_capability_envelope
                .as_ref()
                .and_then(|envelope| envelope.backend_family),
            Some(ComputeBackendFamily::GptOss)
        );
        assert_eq!(
            evaluation
                .observed_capability_envelope
                .as_ref()
                .and_then(|envelope| envelope.compute_family),
            Some(ComputeFamily::Inference)
        );
    }

    #[test]
    fn delivery_evaluation_marks_model_drift_as_variance() {
        let mut job = fixture_active_job_with_gpt_oss_provenance();
        job.requested_model = Some("llama3.2:latest".to_string());
        if let Some(provenance) = job.execution_provenance.as_mut() {
            provenance.served_model = "llama3.1:latest".to_string();
        }
        let binding = compute_binding_for_backend_and_capability(
            LocalInferenceBackend::GptOss,
            "text_generation",
        )
        .expect("inference binding");

        let evaluation = evaluate_delivery_proof(&job, binding, &fixture_delivery_context("hello"));

        assert_eq!(evaluation.status, DeliveryProofStatus::Accepted);
        assert_eq!(
            evaluation.variance_reason,
            Some(ComputeDeliveryVarianceReason::ModelPolicyDrift)
        );
        assert!(evaluation.rejection_reason.is_none());
    }

    #[test]
    fn gpt_oss_embeddings_launch_bindings_are_retired_locally() {
        assert!(
            compute_binding_for_backend_and_capability(
                LocalInferenceBackend::GptOss,
                "text_embeddings",
            )
            .is_none()
        );
        assert!(compute_binding_for_product_id("gpt_oss.embeddings").is_none());
    }
}
