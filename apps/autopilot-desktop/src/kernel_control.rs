use crate::app_state::{ActiveJobRecord, JobInboxDecision, RenderState};
use crate::state::job_inbox::JobInboxRequest;
use openagents_kernel_core::authority::{
    CreateContractRequest, CreateWorkUnitRequest, FinalizeVerdictRequest,
    HttpKernelAuthorityClient, KernelAuthority, SubmitOutputRequest, canonical_kernel_endpoint,
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
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::future::Future;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::watch;

const ENV_FORCE_LOCAL_KERNEL_AUTHORITY: &str = "OA_DESKTOP_FORCE_LOCAL_KERNEL_AUTHORITY";
const KERNEL_MUTATION_TIMEOUT: Duration = Duration::from_secs(5);
const KERNEL_STREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const KERNEL_STREAM_RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum KernelAuthorityMode {
    Local,
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

impl Default for KernelProjectionWorker {
    fn default() -> Self {
        Self {
            mode: KernelAuthorityMode::Local,
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
    if force_local_kernel_authority() {
        return KernelAuthorityMode::Local;
    }
    let Some(base_url) = control_base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return KernelAuthorityMode::Local;
    };
    let Some(bearer_auth) = bearer_auth
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        return KernelAuthorityMode::Local;
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
    !state.kernel_projection_worker.uses_remote_authority()
}

pub(crate) fn register_accepted_request_with_kernel(
    state: &mut RenderState,
    request: &JobInboxRequest,
) -> Result<(), String> {
    let work_unit_request = build_work_unit_request(state, request);
    let work_unit_receipt = match current_authority_mode(state) {
        KernelAuthorityMode::Local => {
            let authority = state.kernel_local_authority.clone();
            run_kernel_call(authority.create_work_unit(work_unit_request))?.receipt
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => {
            let client = build_remote_authority_client(base_url, bearer_auth)?;
            run_kernel_call(client.create_work_unit(work_unit_request))?.receipt
        }
    };
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(work_unit_receipt, "kernel.authority.work_unit");

    let contract_request = build_contract_request(state, request);
    let contract_receipt = match current_authority_mode(state) {
        KernelAuthorityMode::Local => {
            let authority = state.kernel_local_authority.clone();
            run_kernel_call(authority.create_contract(contract_request))?.receipt
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => {
            let client = build_remote_authority_client(base_url, bearer_auth)?;
            run_kernel_call(client.create_contract(contract_request))?.receipt
        }
    };
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(contract_receipt, "kernel.authority.contract");
    Ok(())
}

pub(crate) fn submit_active_job_output(state: &mut RenderState) -> Result<String, String> {
    let Some(job) = state.active_job.job.as_ref().cloned() else {
        return Err("no active job selected".to_string());
    };
    let submit_request = build_submit_output_request(state, &job);
    let receipt = match current_authority_mode(state) {
        KernelAuthorityMode::Local => {
            let authority = state.kernel_local_authority.clone();
            run_kernel_call(authority.submit_output(submit_request))?.receipt
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => {
            let client = build_remote_authority_client(base_url, bearer_auth)?;
            run_kernel_call(client.submit_output(submit_request))?.receipt
        }
    };
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
    let verdict_request = build_finalize_verdict_request(state, &job);
    let receipt = match current_authority_mode(state) {
        KernelAuthorityMode::Local => {
            let authority = state.kernel_local_authority.clone();
            run_kernel_call(authority.finalize_verdict(verdict_request))?.receipt
        }
        KernelAuthorityMode::Remote {
            ref base_url,
            ref bearer_auth,
        } => {
            let client = build_remote_authority_client(base_url, bearer_auth)?;
            run_kernel_call(client.finalize_verdict(verdict_request))?.receipt
        }
    };
    let receipt_id = receipt.receipt_id.clone();
    state
        .earn_kernel_receipts
        .apply_authoritative_receipt(receipt, "kernel.authority.verdict");
    Ok(receipt_id)
}

fn current_authority_mode(state: &RenderState) -> KernelAuthorityMode {
    resolve_kernel_authority_mode(
        state.hosted_control_base_url.as_deref(),
        state.hosted_control_bearer_token.as_deref(),
    )
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

            let receipt_url = match canonical_kernel_endpoint(
                base_url.as_str(),
                "/v1/kernel/stream/receipts",
            ) {
                Ok(url) => url,
                Err(error) => {
                    let _ = update_tx.send(KernelProjectionUpdate::StreamError {
                        stream: "receipts",
                        message: error.to_string(),
                    });
                    return;
                }
            };
            let snapshot_url = match canonical_kernel_endpoint(
                base_url.as_str(),
                "/v1/kernel/stream/snapshots",
            ) {
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
            pending_event.data_lines.push(value.trim_start().to_string());
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

fn build_work_unit_request(state: &RenderState, request: &JobInboxRequest) -> CreateWorkUnitRequest {
    let work_unit_id = work_unit_id_for_request(request.request_id.as_str());
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
            }),
        },
        settlement_link: payment_pointer.clone().map(|payment_pointer| SettlementLink {
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
        contract_id: (!work_unit_only).then(|| contract_id_for_request(request.request_id.as_str())),
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

fn submission_evidence_refs(job: &ActiveJobRecord, execution_output: Option<&str>) -> Vec<EvidenceRef> {
    let mut evidence = request_evidence_refs(job.request_id.as_str(), job.skill_scope_id.as_deref());
    if let Some(result_event_id) = job.sa_tick_result_event_id.as_deref() {
        evidence.push(evidence_ref(
            "nostr_event_ref",
            format!("nostr:event:{result_event_id}"),
            result_event_id,
        ));
    }
    if let Some(output) = execution_output.map(str::trim).filter(|value| !value.is_empty()) {
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
    if let Some(payment_id) = job.payment_id.as_deref().filter(|value| !value.trim().is_empty()) {
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

fn work_unit_id_for_request(request_id: &str) -> String {
    format!("work_unit.{}", canonical_kernel_id_component(request_id))
}

fn contract_id_for_request(request_id: &str) -> String {
    format!("contract.{}", canonical_kernel_id_component(request_id))
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

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis().min(i64::MAX as u128) as i64)
}

fn force_local_kernel_authority() -> bool {
    std::env::var(ENV_FORCE_LOCAL_KERNEL_AUTHORITY)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
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
        KernelAuthorityMode, PendingSseEvent, ReceiptProjectionEnvelope, consume_sse_buffer,
        flush_pending_sse_event, resolve_kernel_authority_mode, submission_evidence_refs,
    };
    use crate::app_state::{ActiveJobRecord, JobDemandSource, JobLifecycleStage};
    use crate::economy_kernel_receipts::{
        PolicyContext, ReceiptBuilder, ReceiptHints, TraceContext,
    };
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

    fn fixture_active_job_with_ollama_provenance() -> ActiveJobRecord {
        ActiveJobRecord {
            job_id: "job-ollama-001".to_string(),
            request_id: "req-ollama-001".to_string(),
            requester: "npub1buyer".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: nostr::nip90::KIND_JOB_TEXT_GENERATION,
            capability: "text.generation".to_string(),
            execution_input: Some("Write a haiku about rust".to_string()),
            execution_prompt: Some("Write a haiku about rust".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            execution_provenance: Some(crate::ollama_execution::OllamaExecutionProvenance {
                requested_model: Some("llama3.2:latest".to_string()),
                served_model: "llama3.2:latest".to_string(),
                normalized_prompt_digest: "sha256:prompt".to_string(),
                normalized_options_json: r#"{"num_predict":64}"#.to_string(),
                normalized_options_digest: "sha256:options".to_string(),
                base_url: "http://127.0.0.1:11434".to_string(),
                total_duration_ns: Some(1_000_000),
                load_duration_ns: Some(0),
                prompt_token_count: Some(11),
                generated_token_count: Some(7),
                warm_start: Some(true),
            }),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("req-ollama-001".to_string()),
            sa_tick_result_event_id: Some("result-ollama-001".to_string()),
            sa_trajectory_session_id: Some("traj:req-ollama-001".to_string()),
            ac_envelope_event_id: None,
            ac_settlement_event_id: None,
            ac_default_event_id: None,
            quoted_price_sats: 21,
            ttl_seconds: 90,
            stage: JobLifecycleStage::Delivered,
            invoice_id: None,
            payment_id: None,
            failure_reason: None,
            events: Vec::new(),
        }
    }

    #[test]
    fn resolve_kernel_authority_mode_uses_remote_when_complete_config_present() {
        let mode = resolve_kernel_authority_mode(
            Some("https://control.example.com"),
            Some("token-123"),
        );
        assert_eq!(
            mode,
            KernelAuthorityMode::Remote {
                base_url: "https://control.example.com".to_string(),
                bearer_auth: "token-123".to_string(),
            }
        );
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
        assert!(consume_sse_buffer::<ReceiptProjectionEnvelope, _>(
            &mut pending_buffer,
            &mut pending_event,
            "receipt",
            &tx,
            |envelope| super::KernelProjectionUpdate::Receipt(envelope.receipt),
        )
        .is_none());
        assert!(rx.try_recv().is_err(), "blank line has not arrived yet");

        pending_buffer.push('\n');
        assert!(consume_sse_buffer::<ReceiptProjectionEnvelope, _>(
            &mut pending_buffer,
            &mut pending_event,
            "receipt",
            &tx,
            |envelope| super::KernelProjectionUpdate::Receipt(envelope.receipt),
        )
        .is_none());
        match rx.try_recv().expect("receipt update") {
            super::KernelProjectionUpdate::Receipt(decoded) => {
                assert_eq!(decoded.receipt_id, receipt.receipt_id);
            }
            other => panic!("unexpected projection update: {other:?}"),
        }
    }


    #[test]
    fn submission_evidence_refs_include_ollama_provenance() {
        let job = fixture_active_job_with_ollama_provenance();
        let evidence = submission_evidence_refs(&job, Some("hello from ollama"));

        assert!(evidence.iter().any(|row| {
            row.kind == "execution_backend_ref"
                && row.uri == "oa://autopilot/jobs/job-ollama-001/execution/backend"
        }));
        assert!(evidence.iter().any(|row| {
            row.kind == "attestation:model_version"
                && row.uri == "oa://autopilot/jobs/job-ollama-001/execution/model/llama3.2_latest"
        }));
        assert!(evidence.iter().any(|row| {
            row.kind == "execution_prompt_digest" && row.digest == "sha256:prompt"
        }));
        assert!(evidence.iter().any(|row| {
            row.kind == "execution_options_digest" && row.digest == "sha256:options"
        }));
    }
}
