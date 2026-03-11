use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use nostr::nip90::{
    JobFeedback, JobInput, JobRequest, JobResult, JobStatus, KIND_JOB_TEXT_GENERATION,
    create_job_feedback_event, create_job_request_event, create_job_result_event,
};
use nostr::{Event, EventTemplate, NostrIdentity, load_or_create_identity};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{error, info, warn};

use crate::apple_fm_bridge::{
    AppleFmBridgeCommand, AppleFmBridgeSnapshot, AppleFmBridgeUpdate, AppleFmBridgeWorker,
    AppleFmGenerateJob,
};
use crate::provider_nip90_lane::{
    ProviderNip90AuthIdentity, ProviderNip90BuyerResponseEvent, ProviderNip90BuyerResponseKind,
    ProviderNip90ComputeCapability, ProviderNip90LaneCommand, ProviderNip90LaneUpdate,
    ProviderNip90LaneWorker, ProviderNip90PublishOutcome, ProviderNip90PublishRole,
};
use crate::spark_wallet::{
    SparkPaneState, SparkWalletCommand, SparkWalletWorker, configured_network,
    is_settled_wallet_payment_status, is_terminal_wallet_payment_status,
};
use crate::state::job_inbox::JobInboxNetworkRequest;

pub const HEADLESS_BUY_MODE_REQUEST_TYPE: &str = "mission_control.buy_mode.5050";
pub const HEADLESS_BUY_MODE_REQUEST_KIND: u16 = KIND_JOB_TEXT_GENERATION;
pub const HEADLESS_BUY_MODE_BUDGET_SATS: u64 = 2;
pub const HEADLESS_BUY_MODE_INTERVAL_SECONDS: u64 = 12;
pub const HEADLESS_BUY_MODE_TIMEOUT_SECONDS: u64 = 75;
pub const HEADLESS_BUY_MODE_PROMPT: &str = "Reply with the exact text BUY MODE OK.";

const LOOP_SLEEP: Duration = Duration::from_millis(100);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const ACTIVE_WALLET_REFRESH: Duration = Duration::from_secs(5);
const IDLE_WALLET_REFRESH: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct HeadlessRelayConfig {
    pub listen_addr: SocketAddr,
    pub event_capacity: usize,
}

impl Default for HeadlessRelayConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:18490"
                .parse()
                .expect("default headless relay address should parse"),
            event_capacity: 1_024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeadlessProviderBackend {
    AppleFoundationModels,
    Canned,
}

#[derive(Clone, Debug)]
pub struct HeadlessProviderConfig {
    pub relay_urls: Vec<String>,
    pub identity_path: Option<PathBuf>,
    pub backend: HeadlessProviderBackend,
    pub max_settled_jobs: Option<usize>,
    pub invoice_expiry_seconds: u32,
}

impl Default for HeadlessProviderConfig {
    fn default() -> Self {
        Self {
            relay_urls: Vec::new(),
            identity_path: None,
            backend: HeadlessProviderBackend::Canned,
            max_settled_jobs: None,
            invoice_expiry_seconds: 3_600,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HeadlessBuyerConfig {
    pub relay_urls: Vec<String>,
    pub identity_path: Option<PathBuf>,
    pub request_type: String,
    pub prompt: String,
    pub budget_sats: u64,
    pub timeout_seconds: u64,
    pub interval_seconds: u64,
    pub target_provider_pubkeys: Vec<String>,
    pub max_settled_requests: Option<usize>,
    pub fail_fast: bool,
}

impl Default for HeadlessBuyerConfig {
    fn default() -> Self {
        Self {
            relay_urls: Vec::new(),
            identity_path: None,
            request_type: HEADLESS_BUY_MODE_REQUEST_TYPE.to_string(),
            prompt: HEADLESS_BUY_MODE_PROMPT.to_string(),
            budget_sats: HEADLESS_BUY_MODE_BUDGET_SATS,
            timeout_seconds: HEADLESS_BUY_MODE_TIMEOUT_SECONDS,
            interval_seconds: HEADLESS_BUY_MODE_INTERVAL_SECONDS,
            target_provider_pubkeys: Vec::new(),
            max_settled_requests: None,
            fail_fast: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HeadlessIdentitySummary {
    pub identity_path: PathBuf,
    pub npub: String,
    pub public_key_hex: String,
}

pub fn identity_summary(identity_path: Option<PathBuf>) -> Result<HeadlessIdentitySummary> {
    let identity = load_identity(identity_path.as_deref())?;
    Ok(HeadlessIdentitySummary {
        identity_path: identity.identity_path.clone(),
        npub: identity.npub.clone(),
        public_key_hex: identity.public_key_hex.clone(),
    })
}

pub async fn run_headless_relay(config: HeadlessRelayConfig) -> Result<()> {
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.listen_addr))?;
    let local_addr = listener
        .local_addr()
        .context("failed to resolve headless relay listen address")?;
    info!(
        target: "autopilot_desktop::headless_relay",
        "Headless relay listening on ws://{}",
        local_addr
    );

    let state = Arc::new(Mutex::new(HeadlessRelayState::new(config.event_capacity)));

    loop {
        tokio::select! {
            accept = listener.accept() => {
                let (stream, remote_addr) = accept.context("headless relay accept failed")?;
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(error) = handle_relay_connection(state, stream).await {
                        warn!(
                            target: "autopilot_desktop::headless_relay",
                            "relay connection {} failed: {}",
                            remote_addr,
                            error
                        );
                    }
                });
            }
            signal = tokio::signal::ctrl_c() => {
                match signal {
                    Ok(()) => {
                        info!(
                            target: "autopilot_desktop::headless_relay",
                            "received ctrl-c; stopping headless relay"
                        );
                        return Ok(());
                    }
                    Err(error) => {
                        return Err(anyhow!("failed to wait for ctrl-c: {error}"));
                    }
                }
            }
        }
    }
}

pub fn run_headless_provider(config: HeadlessProviderConfig) -> Result<()> {
    ensure_relays(config.relay_urls.as_slice(), "provider")?;
    let identity = load_identity(config.identity_path.as_deref())?;
    info!(
        target: "autopilot_desktop::headless_provider",
        "provider identity ready npub={} pubkey={} identity_path={}",
        identity.npub,
        identity.public_key_hex,
        identity.identity_path.display()
    );

    let mut spark_state = SparkPaneState::with_network(configured_network());
    let mut spark_worker = SparkWalletWorker::spawn(spark_state.network);
    spark_worker
        .enqueue(SparkWalletCommand::Refresh)
        .map_err(|error| anyhow!("failed to queue provider Spark refresh: {error}"))?;
    wait_for_wallet_refresh(
        &mut spark_worker,
        &mut spark_state,
        "provider",
        STARTUP_TIMEOUT,
    )?;
    log_wallet_balance("provider", &spark_state);

    let (mut apple_worker, capability) = match config.backend {
        HeadlessProviderBackend::AppleFoundationModels => {
            let (worker, snapshot) = wait_for_apple_fm_ready(STARTUP_TIMEOUT)?;
            let capability = apple_fm_capability(snapshot);
            (Some(worker), capability)
        }
        HeadlessProviderBackend::Canned => (None, canned_capability()),
    };
    info!(
        target: "autopilot_desktop::headless_provider",
        "provider backend={} model={}",
        capability.backend_or_default(),
        capability
            .ready_model
            .as_deref()
            .unwrap_or("unavailable")
    );

    let mut lane = ProviderNip90LaneWorker::spawn(config.relay_urls.clone());
    lane.enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
        identity: Some(provider_auth_identity(&identity)),
    })
    .map_err(|error| anyhow!("failed to configure provider identity: {error}"))?;
    lane.enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability { capability })
        .map_err(|error| anyhow!("failed to configure provider capability: {error}"))?;
    lane.enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
        .map_err(|error| anyhow!("failed to set provider online: {error}"))?;
    wait_for_lane_connection(&mut lane, "provider", STARTUP_TIMEOUT)?;

    let mut active_job: Option<ActiveProviderJob> = None;
    let mut settled_jobs = 0usize;
    let mut next_wallet_refresh = Instant::now();
    let mut last_provider_payment_id = spark_state
        .recent_payments
        .first()
        .map(|payment| payment.id.clone());
    let mut last_lane_snapshot = None::<(usize, Option<String>, Option<String>)>;

    loop {
        for update in lane.drain_updates() {
            match update {
                ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                    let current = (
                        snapshot.connected_relays,
                        snapshot.last_action.clone(),
                        snapshot.last_error.clone(),
                    );
                    if last_lane_snapshot.as_ref() != Some(&current) {
                        log_lane_snapshot(
                            "provider",
                            snapshot.connected_relays,
                            snapshot.last_action.as_deref(),
                            snapshot.last_error.as_deref(),
                        );
                        last_lane_snapshot = Some(current);
                    }
                }
                ProviderNip90LaneUpdate::IngressedRequest(request) => {
                    if active_job.is_some() {
                        info!(
                            target: "autopilot_desktop::headless_provider",
                            "ignoring request {} while provider is busy",
                            request.request_id
                        );
                        continue;
                    }
                    if !provider_request_is_supported(&request, &identity) {
                        info!(
                            target: "autopilot_desktop::headless_provider",
                            "ignoring unsupported or untargeted request {}",
                            request.request_id
                        );
                        continue;
                    }
                    info!(
                        target: "autopilot_desktop::headless_provider",
                        "accepted request_id={} price_sats={} ttl_seconds={} prompt={}",
                        request.request_id,
                        request.price_sats,
                        request.ttl_seconds,
                        truncate_for_log(provider_prompt(&request).as_str(), 160)
                    );
                    let mut job =
                        ActiveProviderJob::new(request.clone(), spark_total_sats(&spark_state));
                    publish_processing_feedback(&lane, &identity, &job.request)?;
                    match config.backend {
                        HeadlessProviderBackend::AppleFoundationModels => {
                            let Some(worker) = apple_worker.as_ref() else {
                                bail!("Apple FM worker missing for provider execution");
                            };
                            worker
                                .enqueue(AppleFmBridgeCommand::Generate(AppleFmGenerateJob {
                                    request_id: job.request.request_id.clone(),
                                    prompt: provider_prompt(&job.request),
                                    requested_model: None,
                                }))
                                .map_err(|error| {
                                    anyhow!("failed to queue Apple FM job: {error}")
                                })?;
                        }
                        HeadlessProviderBackend::Canned => {
                            let output = canned_provider_output(&job.request);
                            handle_provider_execution_complete(
                                &lane,
                                &identity,
                                &mut spark_worker,
                                &mut job,
                                output,
                                config.invoice_expiry_seconds,
                            )?;
                        }
                    }
                    active_job = Some(job);
                }
                ProviderNip90LaneUpdate::PublishOutcome(outcome) => {
                    log_publish_outcome("provider", &outcome);
                }
                ProviderNip90LaneUpdate::BuyerResponseEvent(_) => {}
            }
        }

        if let Some(worker) = apple_worker.as_mut() {
            for update in worker.drain_updates() {
                match update {
                    AppleFmBridgeUpdate::Snapshot(snapshot) => {
                        log_apple_snapshot(&snapshot);
                    }
                    AppleFmBridgeUpdate::Started(started) => {
                        info!(
                            target: "autopilot_desktop::headless_provider",
                            "Apple FM execution started request_id={} model={}",
                            started.request_id,
                            started.model
                        );
                    }
                    AppleFmBridgeUpdate::Completed(completed) => {
                        if let Some(job) = active_job.as_mut()
                            && job.request.request_id == completed.request_id
                        {
                            handle_provider_execution_complete(
                                &lane,
                                &identity,
                                &mut spark_worker,
                                job,
                                completed.output,
                                config.invoice_expiry_seconds,
                            )?;
                        }
                    }
                    AppleFmBridgeUpdate::Failed(failed) => {
                        if let Some(job) = active_job.as_ref()
                            && job.request.request_id == failed.request_id
                        {
                            bail!(
                                "Apple Foundation Models execution failed for {}: {}",
                                failed.request_id,
                                failed.error
                            );
                        }
                    }
                    AppleFmBridgeUpdate::Workbench(_)
                    | AppleFmBridgeUpdate::MissionControlSummary(_) => {}
                }
            }
        }

        if Instant::now() >= next_wallet_refresh {
            spark_worker
                .enqueue(SparkWalletCommand::Refresh)
                .map_err(|error| anyhow!("failed to queue provider wallet refresh: {error}"))?;
            next_wallet_refresh = Instant::now()
                + if active_job.is_some() {
                    ACTIVE_WALLET_REFRESH
                } else {
                    IDLE_WALLET_REFRESH
                };
        }

        let spark_changed = spark_worker.drain_updates(&mut spark_state);
        if spark_changed {
            let newest_payment = spark_state
                .recent_payments
                .first()
                .map(|payment| payment.id.clone());
            if newest_payment != last_provider_payment_id {
                if let Some(payment) = spark_state.recent_payments.first() {
                    info!(
                        target: "autopilot_desktop::headless_provider",
                        "provider wallet payment id={} direction={} status={} amount_sats={}",
                        payment.id,
                        payment.direction,
                        payment.status,
                        payment.amount_sats
                    );
                }
                last_provider_payment_id = newest_payment;
            }
            if let Some(job) = active_job.as_mut() {
                handle_provider_wallet_update(&lane, &identity, job, &spark_state)?;
                if job.settled {
                    settled_jobs = settled_jobs.saturating_add(1);
                    info!(
                        target: "autopilot_desktop::headless_provider",
                        "provider settled request_id={} total_provider_balance_sats={}",
                        job.request.request_id,
                        spark_total_sats(&spark_state)
                    );
                    active_job = None;
                    if config
                        .max_settled_jobs
                        .is_some_and(|max| settled_jobs >= max)
                    {
                        info!(
                            target: "autopilot_desktop::headless_provider",
                            "provider reached max settled jobs {}; exiting",
                            settled_jobs
                        );
                        return Ok(());
                    }
                }
            }
        }

        if let Some(job) = active_job.as_ref()
            && job.deadline_exceeded()
        {
            bail!(
                "provider request {} exceeded settlement deadline",
                job.request.request_id
            );
        }

        std::thread::sleep(LOOP_SLEEP);
    }
}

pub fn run_headless_buyer(config: HeadlessBuyerConfig) -> Result<()> {
    ensure_relays(config.relay_urls.as_slice(), "buyer")?;
    let identity = load_identity(config.identity_path.as_deref())?;
    info!(
        target: "autopilot_desktop::headless_buyer",
        "buyer identity ready npub={} pubkey={} identity_path={}",
        identity.npub,
        identity.public_key_hex,
        identity.identity_path.display()
    );

    let mut spark_state = SparkPaneState::with_network(configured_network());
    let mut spark_worker = SparkWalletWorker::spawn(spark_state.network);
    spark_worker
        .enqueue(SparkWalletCommand::Refresh)
        .map_err(|error| anyhow!("failed to queue buyer Spark refresh: {error}"))?;
    wait_for_wallet_refresh(
        &mut spark_worker,
        &mut spark_state,
        "buyer",
        STARTUP_TIMEOUT,
    )?;
    let initial_balance = spark_total_sats(&spark_state);
    if initial_balance < config.budget_sats {
        bail!(
            "buyer wallet has {} sats but requires at least {} sats",
            initial_balance,
            config.budget_sats
        );
    }
    log_wallet_balance("buyer", &spark_state);

    let mut lane = ProviderNip90LaneWorker::spawn(config.relay_urls.clone());
    lane.enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
        identity: Some(provider_auth_identity(&identity)),
    })
    .map_err(|error| anyhow!("failed to configure buyer identity: {error}"))?;
    wait_for_lane_connection(&mut lane, "buyer", STARTUP_TIMEOUT)?;

    let mut settled_requests = 0usize;
    let mut active_request: Option<ActiveBuyerRequest> = None;
    let mut next_dispatch_at = Instant::now();
    let mut next_wallet_refresh = Instant::now() + IDLE_WALLET_REFRESH;
    let mut last_seen_payment_id = spark_state.last_payment_id.clone();
    let mut last_lane_snapshot = None::<(usize, Option<String>, Option<String>)>;

    loop {
        for update in lane.drain_updates() {
            match update {
                ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                    let current = (
                        snapshot.connected_relays,
                        snapshot.last_action.clone(),
                        snapshot.last_error.clone(),
                    );
                    if last_lane_snapshot.as_ref() != Some(&current) {
                        log_lane_snapshot(
                            "buyer",
                            snapshot.connected_relays,
                            snapshot.last_action.as_deref(),
                            snapshot.last_error.as_deref(),
                        );
                        last_lane_snapshot = Some(current);
                    }
                }
                ProviderNip90LaneUpdate::PublishOutcome(outcome) => {
                    if outcome.role == ProviderNip90PublishRole::Request {
                        log_publish_outcome("buyer", &outcome);
                        if let Some(request) = active_request.as_mut()
                            && request.request_id == outcome.request_id
                            && outcome.accepted_relays == 0
                        {
                            request.failed_reason =
                                Some(outcome.first_error.clone().unwrap_or_else(|| {
                                    "all relays rejected request publish".to_string()
                                }));
                        }
                    }
                }
                ProviderNip90LaneUpdate::BuyerResponseEvent(event) => {
                    if let Some(request) = active_request.as_mut()
                        && request.request_id == event.request_id
                    {
                        handle_buyer_response_event(request, event, &mut spark_worker)?;
                    }
                }
                ProviderNip90LaneUpdate::IngressedRequest(_) => {}
            }
        }

        if Instant::now() >= next_wallet_refresh {
            spark_worker
                .enqueue(SparkWalletCommand::Refresh)
                .map_err(|error| anyhow!("failed to queue buyer wallet refresh: {error}"))?;
            next_wallet_refresh = Instant::now()
                + if active_request
                    .as_ref()
                    .is_some_and(ActiveBuyerRequest::payment_pending)
                {
                    ACTIVE_WALLET_REFRESH
                } else {
                    IDLE_WALLET_REFRESH
                };
        }

        let spark_changed = spark_worker.drain_updates(&mut spark_state);
        if spark_changed {
            if spark_state.last_payment_id != last_seen_payment_id {
                if let Some(request) = active_request.as_mut()
                    && request.payment_enqueued
                    && request.payment_pointer.is_none()
                    && let Some(pointer) = spark_state.last_payment_id.as_deref()
                {
                    request.payment_pointer = Some(pointer.to_string());
                    info!(
                        target: "autopilot_desktop::headless_buyer",
                        "buyer payment pointer assigned request_id={} payment_id={}",
                        request.request_id,
                        pointer
                    );
                }
                last_seen_payment_id = spark_state.last_payment_id.clone();
            }

            if let Some(request) = active_request.as_mut() {
                handle_buyer_wallet_update(request, &spark_state);
                if let Some(reason) = request.failed_reason.as_deref() {
                    error!(
                        target: "autopilot_desktop::headless_buyer",
                        "buyer request failed request_id={} reason={}",
                        request.request_id,
                        reason
                    );
                    if config.fail_fast {
                        bail!("buyer request {} failed: {}", request.request_id, reason);
                    }
                    active_request = None;
                    next_dispatch_at =
                        Instant::now() + Duration::from_secs(config.interval_seconds.max(1));
                } else if request.is_terminal_success() {
                    settled_requests = settled_requests.saturating_add(1);
                    info!(
                        target: "autopilot_desktop::headless_buyer",
                        "buyer settled request_id={} provider={} result={}",
                        request.request_id,
                        request
                            .provider_pubkey
                            .as_deref()
                            .unwrap_or("unknown"),
                        truncate_for_log(
                            request
                                .result_content
                                .as_deref()
                                .unwrap_or("no result content observed"),
                            220
                        )
                    );
                    active_request = None;
                    next_dispatch_at =
                        Instant::now() + Duration::from_secs(config.interval_seconds.max(1));
                    if config
                        .max_settled_requests
                        .is_some_and(|max| settled_requests >= max)
                    {
                        info!(
                            target: "autopilot_desktop::headless_buyer",
                            "buyer reached max settled requests {}; exiting",
                            settled_requests
                        );
                        return Ok(());
                    }
                }
            }
        }

        if let Some(request) = active_request.as_ref()
            && request.deadline_exceeded(config.timeout_seconds)
        {
            if config.fail_fast {
                bail!("buyer request {} exceeded timeout", request.request_id);
            }
            error!(
                target: "autopilot_desktop::headless_buyer",
                "buyer request {} exceeded timeout; clearing request",
                request.request_id
            );
            active_request = None;
            next_dispatch_at = Instant::now() + Duration::from_secs(config.interval_seconds.max(1));
        }

        if active_request.is_none() && Instant::now() >= next_dispatch_at {
            let current_balance = spark_total_sats(&spark_state);
            if current_balance < config.budget_sats {
                let message = format!(
                    "buyer wallet balance {} sats is below required {} sats",
                    current_balance, config.budget_sats
                );
                if config.fail_fast {
                    bail!(message);
                }
                warn!(
                    target: "autopilot_desktop::headless_buyer",
                    "{}",
                    message
                );
                next_dispatch_at =
                    Instant::now() + Duration::from_secs(config.interval_seconds.max(1));
            } else {
                let request_event = build_buyer_request_event(
                    &identity,
                    config.request_type.as_str(),
                    config.prompt.as_str(),
                    config.budget_sats,
                    config.timeout_seconds,
                    config.relay_urls.as_slice(),
                    config.target_provider_pubkeys.as_slice(),
                )?;
                let request_id = request_event.id.clone();
                lane.enqueue(ProviderNip90LaneCommand::TrackBuyerRequestIds {
                    request_ids: vec![request_id.clone()],
                })
                .map_err(|error| anyhow!("failed to track buyer request id: {error}"))?;
                lane.enqueue(ProviderNip90LaneCommand::PublishEvent {
                    request_id: request_id.clone(),
                    role: ProviderNip90PublishRole::Request,
                    event: Box::new(request_event),
                })
                .map_err(|error| anyhow!("failed to publish buyer request: {error}"))?;
                info!(
                    target: "autopilot_desktop::headless_buyer",
                    "queued buyer request request_id={} budget_sats={} timeout_seconds={} prompt={}",
                    request_id,
                    config.budget_sats,
                    config.timeout_seconds,
                    truncate_for_log(config.prompt.as_str(), 160)
                );
                active_request = Some(ActiveBuyerRequest::new(request_id));
                next_wallet_refresh = Instant::now() + ACTIVE_WALLET_REFRESH;
            }
        }

        std::thread::sleep(LOOP_SLEEP);
    }
}

fn handle_provider_execution_complete(
    lane: &ProviderNip90LaneWorker,
    identity: &NostrIdentity,
    spark_worker: &mut SparkWalletWorker,
    job: &mut ActiveProviderJob,
    output: String,
    invoice_expiry_seconds: u32,
) -> Result<()> {
    job.output = Some(output.clone());
    let result_event = build_provider_result_event(identity, &job.request, output.as_str())?;
    let result_event_id = result_event.id.clone();
    lane.enqueue(ProviderNip90LaneCommand::PublishEvent {
        request_id: job.request.request_id.clone(),
        role: ProviderNip90PublishRole::Result,
        event: Box::new(result_event),
    })
    .map_err(|error| anyhow!("failed to queue provider result publish: {error}"))?;
    job.result_event_id = Some(result_event_id.clone());
    info!(
        target: "autopilot_desktop::headless_provider",
        "provider result queued request_id={} event_id={} chars={}",
        job.request.request_id,
        result_event_id,
        output.chars().count()
    );

    spark_worker
        .enqueue(SparkWalletCommand::CreateBolt11Invoice {
            amount_sats: job.request.price_sats,
            description: Some(format!(
                "OpenAgents headless job {}",
                job.request.request_id
            )),
            expiry_seconds: Some(invoice_expiry_seconds),
        })
        .map_err(|error| anyhow!("failed to queue provider bolt11 invoice: {error}"))?;
    job.invoice_requested = true;
    info!(
        target: "autopilot_desktop::headless_provider",
        "provider requested bolt11 invoice request_id={} amount_sats={}",
        job.request.request_id,
        job.request.price_sats
    );
    Ok(())
}

fn handle_provider_wallet_update(
    lane: &ProviderNip90LaneWorker,
    identity: &NostrIdentity,
    job: &mut ActiveProviderJob,
    spark_state: &SparkPaneState,
) -> Result<()> {
    if job.invoice_requested
        && spark_state
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Created Lightning invoice"))
        && let Some(invoice) = spark_state.last_invoice.as_deref()
        && job.bolt11.as_deref() != Some(invoice)
    {
        let feedback_event = build_provider_feedback_event(
            identity,
            &job.request,
            JobStatus::PaymentRequired,
            "lightning settlement required",
            Some("pay the attached Lightning invoice to settle this result".to_string()),
            true,
            Some(invoice),
        )?;
        let feedback_event_id = feedback_event.id.clone();
        lane.enqueue(ProviderNip90LaneCommand::PublishEvent {
            request_id: job.request.request_id.clone(),
            role: ProviderNip90PublishRole::Feedback,
            event: Box::new(feedback_event),
        })
        .map_err(|error| anyhow!("failed to queue provider payment-required feedback: {error}"))?;
        job.invoice_requested = false;
        job.bolt11 = Some(invoice.to_string());
        job.payment_required_event_id = Some(feedback_event_id.clone());
        job.invoice_created_at_epoch_seconds = spark_state.last_invoice_created_at_epoch_seconds;
        info!(
            target: "autopilot_desktop::headless_provider",
            "provider queued payment-required feedback request_id={} event_id={} amount_sats={}",
            job.request.request_id,
            feedback_event_id,
            job.request.price_sats
        );
    }

    if spark_state.last_error.is_some() && job.invoice_requested {
        bail!(
            "provider invoice creation failed for {}: {}",
            job.request.request_id,
            spark_state.last_error.as_deref().unwrap_or("unknown error")
        );
    }

    if job.bolt11.is_some() && !job.settled && provider_payment_observed(job, spark_state) {
        let success_feedback = build_provider_feedback_event(
            identity,
            &job.request,
            JobStatus::Success,
            "wallet-confirmed settlement recorded",
            Some("execution lane settled".to_string()),
            true,
            None,
        )?;
        let success_feedback_id = success_feedback.id.clone();
        lane.enqueue(ProviderNip90LaneCommand::PublishEvent {
            request_id: job.request.request_id.clone(),
            role: ProviderNip90PublishRole::Feedback,
            event: Box::new(success_feedback),
        })
        .map_err(|error| anyhow!("failed to queue provider success feedback: {error}"))?;
        job.success_feedback_event_id = Some(success_feedback_id.clone());
        job.settled = true;
        info!(
            target: "autopilot_desktop::headless_provider",
            "provider settlement confirmed request_id={} success_feedback_id={} balance_before={} balance_after={}",
            job.request.request_id,
            success_feedback_id,
            job.balance_before_sats,
            spark_total_sats(spark_state)
        );
    }

    Ok(())
}

fn handle_buyer_response_event(
    request: &mut ActiveBuyerRequest,
    event: ProviderNip90BuyerResponseEvent,
    spark_worker: &mut SparkWalletWorker,
) -> Result<()> {
    request.provider_pubkey = Some(event.provider_pubkey.clone());
    match event.kind {
        ProviderNip90BuyerResponseKind::Feedback => {
            let status = event.status.as_deref().unwrap_or("unknown").to_string();
            info!(
                target: "autopilot_desktop::headless_buyer",
                "buyer feedback request_id={} provider={} status={} event_id={}",
                event.request_id,
                event.provider_pubkey,
                status,
                event.event_id
            );
            if status.eq_ignore_ascii_case("success") {
                request.success_feedback_event_id = Some(event.event_id.clone());
            }
            if status.eq_ignore_ascii_case("payment-required") {
                request.payment_feedback_event_id = Some(event.event_id.clone());
                let Some(bolt11) = event.bolt11.as_deref() else {
                    request.failed_reason = Some(
                        "provider returned payment-required without bolt11 invoice".to_string(),
                    );
                    return Ok(());
                };
                if !request.payment_enqueued {
                    let amount_sats = event
                        .amount_msats
                        .map(|value| value.saturating_add(999) / 1000);
                    spark_worker
                        .enqueue(SparkWalletCommand::SendPayment {
                            payment_request: bolt11.to_string(),
                            amount_sats,
                        })
                        .map_err(|error| anyhow!("failed to queue buyer Spark payment: {error}"))?;
                    request.payment_enqueued = true;
                    request.payment_request = Some(bolt11.to_string());
                    info!(
                        target: "autopilot_desktop::headless_buyer",
                        "buyer queued Spark payment request_id={} amount_sats={} bolt11_present=true",
                        event.request_id,
                        amount_sats
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "none".to_string())
                    );
                }
            }
        }
        ProviderNip90BuyerResponseKind::Result => {
            request.result_event_id = Some(event.event_id.clone());
            request.result_content = buyer_result_content(event.raw_event_json.as_deref());
            info!(
                target: "autopilot_desktop::headless_buyer",
                "buyer result observed request_id={} provider={} event_id={} result={}",
                event.request_id,
                event.provider_pubkey,
                event.event_id,
                truncate_for_log(
                    request
                        .result_content
                        .as_deref()
                        .unwrap_or("no result content"),
                    220
                )
            );
        }
    }
    Ok(())
}

fn handle_buyer_wallet_update(request: &mut ActiveBuyerRequest, spark_state: &SparkPaneState) {
    if request.payment_enqueued
        && request.payment_pointer.is_none()
        && spark_state
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Payment send failed"))
    {
        request.failed_reason = Some(
            spark_state
                .last_error
                .clone()
                .unwrap_or_else(|| "Spark payment send failed".to_string()),
        );
        return;
    }

    let Some(pointer) = request.payment_pointer.as_deref() else {
        return;
    };
    let Some(payment) = spark_state
        .recent_payments
        .iter()
        .find(|payment| payment.id == pointer)
    else {
        return;
    };

    if is_settled_wallet_payment_status(payment.status.as_str()) {
        if !request.payment_settled {
            request.payment_settled = true;
            info!(
                target: "autopilot_desktop::headless_buyer",
                "buyer payment settled request_id={} payment_id={} amount_sats={}",
                request.request_id,
                payment.id,
                payment.amount_sats
            );
        }
        return;
    }

    if is_terminal_wallet_payment_status(payment.status.as_str()) {
        request.failed_reason = Some(format!(
            "Spark payment {} for {} is {}",
            pointer, request.request_id, payment.status
        ));
    }
}

fn provider_payment_observed(job: &ActiveProviderJob, spark_state: &SparkPaneState) -> bool {
    spark_state.recent_payments.iter().any(|payment| {
        payment.direction == "receive"
            && is_settled_wallet_payment_status(payment.status.as_str())
            && payment.amount_sats == job.request.price_sats
            && job
                .invoice_created_at_epoch_seconds
                .is_none_or(|created_at| payment.timestamp >= created_at.saturating_sub(5))
    })
}

fn publish_processing_feedback(
    lane: &ProviderNip90LaneWorker,
    identity: &NostrIdentity,
    request: &JobInboxNetworkRequest,
) -> Result<()> {
    let event = build_provider_feedback_event(
        identity,
        request,
        JobStatus::Processing,
        "provider execution started",
        Some("execution lane processing".to_string()),
        false,
        None,
    )?;
    let event_id = event.id.clone();
    lane.enqueue(ProviderNip90LaneCommand::PublishEvent {
        request_id: request.request_id.clone(),
        role: ProviderNip90PublishRole::Feedback,
        event: Box::new(event),
    })
    .map_err(|error| anyhow!("failed to queue provider processing feedback: {error}"))?;
    info!(
        target: "autopilot_desktop::headless_provider",
        "provider processing feedback queued request_id={} event_id={}",
        request.request_id,
        event_id
    );
    Ok(())
}

fn build_buyer_request_event(
    identity: &NostrIdentity,
    request_type: &str,
    prompt: &str,
    budget_sats: u64,
    timeout_seconds: u64,
    relay_urls: &[String],
    target_provider_pubkeys: &[String],
) -> Result<Event> {
    let request_kind = headless_request_kind_for_type(request_type);
    let mut request = JobRequest::new(request_kind)
        .map_err(|error| anyhow!("failed to build buyer request: {error}"))?
        .add_input(JobInput::text(prompt).with_marker("prompt"))
        .add_param("request_type", request_type)
        .add_param("oa_resolution_mode", "race")
        .add_param("timeout_seconds", timeout_seconds.to_string())
        .with_bid(budget_sats.saturating_mul(1000));
    for relay in normalized_relays(relay_urls) {
        request = request.add_relay(relay);
    }
    for provider in normalized_targets(target_provider_pubkeys) {
        request = request.add_service_provider(provider);
    }
    let template = create_job_request_event(&request);
    sign_template(identity, &template)
}

fn build_provider_result_event(
    identity: &NostrIdentity,
    request: &JobInboxNetworkRequest,
    output: &str,
) -> Result<Event> {
    let mut result = JobResult::new(
        request.request_kind,
        request.request_id.clone(),
        request.requester.clone(),
        visible_result_content(request.request_kind, output),
    )
    .map_err(|error| anyhow!("failed to build provider result: {error}"))?;
    if request.price_sats > 0 {
        result = result.with_amount(request.price_sats.saturating_mul(1000), None);
    }
    let template = create_job_result_event(&result);
    sign_template(identity, &template)
}

fn build_provider_feedback_event(
    identity: &NostrIdentity,
    request: &JobInboxNetworkRequest,
    status: JobStatus,
    status_extra: &str,
    content: Option<String>,
    include_amount: bool,
    bolt11: Option<&str>,
) -> Result<Event> {
    let mut feedback = JobFeedback::new(
        status,
        request.request_id.as_str(),
        request.requester.as_str(),
    )
    .with_status_extra(status_extra.to_string());
    if let Some(content) = content {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            feedback = feedback.with_content(trimmed.to_string());
        }
    }
    if include_amount && request.price_sats > 0 {
        feedback = feedback.with_amount(
            request.price_sats.saturating_mul(1000),
            bolt11.map(ToString::to_string),
        );
    }
    let template = create_job_feedback_event(&feedback);
    sign_template(identity, &template)
}

fn sign_template(identity: &NostrIdentity, template: &EventTemplate) -> Result<Event> {
    let private_key = parse_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(template, &private_key)
        .map_err(|error| anyhow!("failed to sign nostr event: {error}"))
}

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32]> {
    let key_bytes = hex::decode(private_key_hex.trim())
        .map_err(|error| anyhow!("invalid identity private key hex: {error}"))?;
    if key_bytes.len() != 32 {
        bail!(
            "invalid identity private key length {}, expected 32 bytes",
            key_bytes.len()
        );
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(key_bytes.as_slice());
    Ok(key)
}

fn visible_result_content(request_kind: u16, output: &str) -> String {
    if request_kind == KIND_JOB_TEXT_GENERATION {
        output.trim().to_string()
    } else {
        serde_json::json!({
            "status": "completed",
            "output": output.trim(),
        })
        .to_string()
    }
}

fn headless_request_kind_for_type(request_type: &str) -> u16 {
    let normalized = request_type.trim().to_ascii_lowercase();
    if normalized.contains("summary") || normalized.contains("summariz") {
        nostr::nip90::KIND_JOB_SUMMARIZATION
    } else if normalized.contains("translate") {
        nostr::nip90::KIND_JOB_TRANSLATION
    } else if normalized.contains("extract") {
        nostr::nip90::KIND_JOB_TEXT_EXTRACTION
    } else {
        KIND_JOB_TEXT_GENERATION
    }
}

fn provider_auth_identity(identity: &NostrIdentity) -> ProviderNip90AuthIdentity {
    ProviderNip90AuthIdentity {
        npub: identity.npub.clone(),
        public_key_hex: identity.public_key_hex.clone(),
        private_key_hex: identity.private_key_hex.clone(),
    }
}

fn canned_capability() -> ProviderNip90ComputeCapability {
    ProviderNip90ComputeCapability {
        backend: "canned".to_string(),
        reachable: true,
        configured_model: Some("canned-headless".to_string()),
        ready_model: Some("canned-headless".to_string()),
        available_models: vec!["canned-headless".to_string()],
        loaded_models: vec!["canned-headless".to_string()],
        last_error: None,
    }
}

fn apple_fm_capability(snapshot: AppleFmBridgeSnapshot) -> ProviderNip90ComputeCapability {
    ProviderNip90ComputeCapability {
        backend: "apple_foundation_models".to_string(),
        reachable: snapshot.reachable,
        configured_model: snapshot.ready_model.clone(),
        ready_model: snapshot.ready_model.clone(),
        available_models: snapshot.available_models.clone(),
        loaded_models: snapshot.ready_model.clone().into_iter().collect::<Vec<_>>(),
        last_error: snapshot.last_error.clone(),
    }
}

fn wait_for_apple_fm_ready(
    timeout: Duration,
) -> Result<(AppleFmBridgeWorker, AppleFmBridgeSnapshot)> {
    let mut worker = AppleFmBridgeWorker::spawn();
    worker
        .enqueue(AppleFmBridgeCommand::EnsureBridgeRunning)
        .map_err(|error| anyhow!("failed to queue Apple FM bridge start: {error}"))?;
    let deadline = Instant::now() + timeout;
    let mut latest_snapshot = AppleFmBridgeSnapshot::default();
    while Instant::now() < deadline {
        for update in worker.drain_updates() {
            if let AppleFmBridgeUpdate::Snapshot(snapshot) = update {
                latest_snapshot = *snapshot;
                log_apple_snapshot(&latest_snapshot);
                if latest_snapshot.is_ready() {
                    return Ok((worker, latest_snapshot));
                }
                if latest_snapshot.last_error.is_some()
                    && latest_snapshot.bridge_status.as_deref() == Some("failed")
                {
                    bail!(
                        "Apple Foundation Models bridge failed: {}",
                        latest_snapshot
                            .last_error
                            .as_deref()
                            .unwrap_or("unknown error")
                    );
                }
            }
        }
        std::thread::sleep(LOOP_SLEEP);
    }

    bail!(
        "timed out waiting for Apple Foundation Models bridge readiness (last_error={})",
        latest_snapshot.last_error.as_deref().unwrap_or("none")
    )
}

fn wait_for_wallet_refresh(
    worker: &mut SparkWalletWorker,
    state: &mut SparkPaneState,
    label: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if worker.drain_updates(state) && state.balance.is_some() {
            return Ok(());
        }
        if state
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("Missing Breez API key"))
        {
            bail!(
                "{label} Spark wallet failed to initialize: {}",
                state.last_error.as_deref().unwrap_or_default()
            );
        }
        std::thread::sleep(LOOP_SLEEP);
    }
    bail!("{label} Spark wallet refresh timed out")
}

fn wait_for_lane_connection(
    lane: &mut ProviderNip90LaneWorker,
    label: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        for update in lane.drain_updates() {
            if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update {
                log_lane_snapshot(
                    label,
                    snapshot.connected_relays,
                    snapshot.last_action.as_deref(),
                    snapshot.last_error.as_deref(),
                );
                if snapshot.connected_relays > 0 {
                    return Ok(());
                }
            }
        }
        std::thread::sleep(LOOP_SLEEP);
    }
    bail!("{label} NIP-90 relay lane failed to connect before timeout")
}

fn load_identity(identity_path: Option<&Path>) -> Result<NostrIdentity> {
    match identity_path {
        Some(path) => {
            // SAFETY: this process sets the identity-path env before spawning worker threads.
            unsafe { std::env::set_var(nostr::ENV_IDENTITY_MNEMONIC_PATH, path) };
        }
        None => {
            // SAFETY: this process clears the identity-path env before spawning worker threads.
            unsafe { std::env::remove_var(nostr::ENV_IDENTITY_MNEMONIC_PATH) };
        }
    };
    load_or_create_identity().context("failed to load or create Nostr identity")
}

fn ensure_relays(relays: &[String], label: &str) -> Result<()> {
    if normalized_relays(relays).is_empty() {
        bail!("{label} requires at least one --relay URL")
    }
    Ok(())
}

fn normalized_relays(relays: &[String]) -> Vec<String> {
    let mut values = relays
        .iter()
        .map(|relay| relay.trim().to_string())
        .filter(|relay| !relay.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn normalized_targets(targets: &[String]) -> Vec<String> {
    let mut values = targets
        .iter()
        .map(|target| target.trim().to_ascii_lowercase())
        .filter(|target| !target.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn provider_request_is_supported(
    request: &JobInboxNetworkRequest,
    identity: &NostrIdentity,
) -> bool {
    if request.request_kind != KIND_JOB_TEXT_GENERATION {
        return false;
    }
    if request.target_provider_pubkeys.is_empty() {
        return true;
    }
    let local_hex = identity.public_key_hex.trim().to_ascii_lowercase();
    let local_npub = identity.npub.trim().to_ascii_lowercase();
    request.target_provider_pubkeys.iter().any(|provider| {
        let normalized = provider.trim().to_ascii_lowercase();
        normalized == local_hex || normalized == local_npub
    })
}

fn provider_prompt(request: &JobInboxNetworkRequest) -> String {
    request
        .execution_prompt
        .clone()
        .or_else(|| request.execution_input.clone())
        .unwrap_or_else(|| "Reply with the exact text BUY MODE OK.".to_string())
}

fn canned_provider_output(request: &JobInboxNetworkRequest) -> String {
    let prompt = provider_prompt(request);
    if prompt
        .to_ascii_uppercase()
        .contains("REPLY WITH THE EXACT TEXT BUY MODE OK")
    {
        "BUY MODE OK.".to_string()
    } else {
        format!(
            "CANNED PROVIDER OK: {}",
            truncate_for_log(prompt.as_str(), 120)
        )
    }
}

fn buyer_result_content(raw_event_json: Option<&str>) -> Option<String> {
    let raw = raw_event_json?.trim();
    if raw.is_empty() {
        return None;
    }
    let event = serde_json::from_str::<Event>(raw).ok()?;
    let content = event.content.trim();
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

fn spark_total_sats(state: &SparkPaneState) -> u64 {
    state
        .balance
        .as_ref()
        .map_or(0, |balance| balance.total_sats())
}

fn current_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn truncate_for_log(value: &str, max_chars: usize) -> String {
    let flattened = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if flattened.chars().count() <= max_chars {
        flattened
    } else {
        format!(
            "{}...",
            flattened.chars().take(max_chars).collect::<String>()
        )
    }
}

fn log_wallet_balance(label: &str, spark_state: &SparkPaneState) {
    match label {
        "provider" => info!(
            target: "autopilot_desktop::headless_provider",
            "provider wallet balance total_sats={} network={} status={}",
            spark_total_sats(spark_state),
            spark_state.network_name(),
            spark_state.network_status_label()
        ),
        _ => info!(
            target: "autopilot_desktop::headless_buyer",
            "buyer wallet balance total_sats={} network={} status={}",
            spark_total_sats(spark_state),
            spark_state.network_name(),
            spark_state.network_status_label()
        ),
    }
}

fn log_lane_snapshot(
    label: &str,
    connected_relays: usize,
    last_action: Option<&str>,
    last_error: Option<&str>,
) {
    match (label, last_error) {
        ("provider", Some(error)) => warn!(
            target: "autopilot_desktop::headless_provider",
            "provider relay lane connected_relays={} action={} error={}",
            connected_relays,
            last_action.unwrap_or("none"),
            error
        ),
        ("provider", None) => info!(
            target: "autopilot_desktop::headless_provider",
            "provider relay lane connected_relays={} action={}",
            connected_relays,
            last_action.unwrap_or("none")
        ),
        (_, Some(error)) => warn!(
            target: "autopilot_desktop::headless_buyer",
            "buyer relay lane connected_relays={} action={} error={}",
            connected_relays,
            last_action.unwrap_or("none"),
            error
        ),
        (_, None) => info!(
            target: "autopilot_desktop::headless_buyer",
            "buyer relay lane connected_relays={} action={}",
            connected_relays,
            last_action.unwrap_or("none")
        ),
    }
}

fn log_publish_outcome(label: &str, outcome: &ProviderNip90PublishOutcome) {
    match (label, outcome.accepted_relays > 0) {
        ("provider", true) => info!(
            target: "autopilot_desktop::headless_provider",
            "provider publish role={} request_id={} event_id={} accepted_relays={} rejected_relays={}",
            outcome.role.label(),
            outcome.request_id,
            outcome.event_id,
            outcome.accepted_relays,
            outcome.rejected_relays
        ),
        ("provider", false) => error!(
            target: "autopilot_desktop::headless_provider",
            "provider publish failed role={} request_id={} event_id={} error={}",
            outcome.role.label(),
            outcome.request_id,
            outcome.event_id,
            outcome
                .first_error
                .as_deref()
                .unwrap_or("all relays rejected publish")
        ),
        (_, true) => info!(
            target: "autopilot_desktop::headless_buyer",
            "buyer publish role={} request_id={} event_id={} accepted_relays={} rejected_relays={}",
            outcome.role.label(),
            outcome.request_id,
            outcome.event_id,
            outcome.accepted_relays,
            outcome.rejected_relays
        ),
        (_, false) => error!(
            target: "autopilot_desktop::headless_buyer",
            "buyer publish failed role={} request_id={} event_id={} error={}",
            outcome.role.label(),
            outcome.request_id,
            outcome.event_id,
            outcome
                .first_error
                .as_deref()
                .unwrap_or("all relays rejected publish")
        ),
    }
}

fn log_apple_snapshot(snapshot: &AppleFmBridgeSnapshot) {
    if snapshot.is_ready() {
        info!(
            target: "autopilot_desktop::headless_provider",
            "Apple FM ready bridge_status={} model={} reachable=true",
            snapshot.bridge_status.as_deref().unwrap_or("unknown"),
            snapshot.ready_model.as_deref().unwrap_or("unknown")
        );
    } else if let Some(error) = snapshot.last_error.as_deref() {
        warn!(
            target: "autopilot_desktop::headless_provider",
            "Apple FM bridge_status={} reachable={} error={}",
            snapshot.bridge_status.as_deref().unwrap_or("unknown"),
            snapshot.reachable,
            error
        );
    }
}

struct ActiveBuyerRequest {
    request_id: String,
    dispatched_at: Instant,
    provider_pubkey: Option<String>,
    result_event_id: Option<String>,
    result_content: Option<String>,
    payment_feedback_event_id: Option<String>,
    success_feedback_event_id: Option<String>,
    payment_request: Option<String>,
    payment_enqueued: bool,
    payment_pointer: Option<String>,
    payment_settled: bool,
    failed_reason: Option<String>,
}

impl ActiveBuyerRequest {
    fn new(request_id: String) -> Self {
        Self {
            request_id,
            dispatched_at: Instant::now(),
            ..Self::default()
        }
    }

    fn payment_pending(&self) -> bool {
        self.payment_enqueued && !self.payment_settled && self.failed_reason.is_none()
    }

    fn is_terminal_success(&self) -> bool {
        self.result_event_id.is_some()
            && self.payment_settled
            && self.success_feedback_event_id.is_some()
            && self.failed_reason.is_none()
    }

    fn deadline_exceeded(&self, timeout_seconds: u64) -> bool {
        self.dispatched_at.elapsed() > Duration::from_secs(timeout_seconds.saturating_add(60))
    }
}

impl Default for ActiveBuyerRequest {
    fn default() -> Self {
        Self {
            request_id: String::new(),
            dispatched_at: Instant::now(),
            provider_pubkey: None,
            result_event_id: None,
            result_content: None,
            payment_feedback_event_id: None,
            success_feedback_event_id: None,
            payment_request: None,
            payment_enqueued: false,
            payment_pointer: None,
            payment_settled: false,
            failed_reason: None,
        }
    }
}

struct ActiveProviderJob {
    request: JobInboxNetworkRequest,
    accepted_at: Instant,
    deadline: Instant,
    balance_before_sats: u64,
    output: Option<String>,
    result_event_id: Option<String>,
    bolt11: Option<String>,
    invoice_requested: bool,
    invoice_created_at_epoch_seconds: Option<u64>,
    payment_required_event_id: Option<String>,
    success_feedback_event_id: Option<String>,
    settled: bool,
}

impl ActiveProviderJob {
    fn new(request: JobInboxNetworkRequest, balance_before_sats: u64) -> Self {
        let deadline =
            Instant::now() + Duration::from_secs(request.ttl_seconds.saturating_add(120).max(180));
        Self {
            request,
            accepted_at: Instant::now(),
            deadline,
            balance_before_sats,
            output: None,
            result_event_id: None,
            bolt11: None,
            invoice_requested: false,
            invoice_created_at_epoch_seconds: None,
            payment_required_event_id: None,
            success_feedback_event_id: None,
            settled: false,
        }
    }

    fn deadline_exceeded(&self) -> bool {
        Instant::now() > self.deadline
    }
}

#[derive(Clone, Debug, Default)]
struct HeadlessRelayFilter {
    kinds: Option<HashSet<u16>>,
    e_tags: Option<HashSet<String>>,
    limit: usize,
}

impl HeadlessRelayFilter {
    fn matches_event(&self, event: &Event) -> bool {
        if let Some(kinds) = self.kinds.as_ref()
            && !kinds.contains(&event.kind)
        {
            return false;
        }
        if let Some(expected_request_ids) = self.e_tags.as_ref() {
            let matched = event.tags.iter().any(|tag| {
                tag.first().is_some_and(|value| value == "e")
                    && tag
                        .get(1)
                        .is_some_and(|value| expected_request_ids.contains(value))
            });
            if !matched {
                return false;
            }
        }
        true
    }
}

struct HeadlessRelayClient {
    sender: mpsc::UnboundedSender<Message>,
    subscriptions: HashMap<String, Vec<HeadlessRelayFilter>>,
}

struct HeadlessRelayState {
    next_client_id: u64,
    event_capacity: usize,
    events: VecDeque<Event>,
    clients: HashMap<u64, HeadlessRelayClient>,
}

impl HeadlessRelayState {
    fn new(event_capacity: usize) -> Self {
        Self {
            next_client_id: 0,
            event_capacity: event_capacity.max(32),
            events: VecDeque::new(),
            clients: HashMap::new(),
        }
    }

    fn register_client(&mut self, sender: mpsc::UnboundedSender<Message>) -> u64 {
        self.next_client_id = self.next_client_id.saturating_add(1);
        let client_id = self.next_client_id;
        self.clients.insert(
            client_id,
            HeadlessRelayClient {
                sender,
                subscriptions: HashMap::new(),
            },
        );
        client_id
    }

    fn remove_client(&mut self, client_id: u64) {
        self.clients.remove(&client_id);
    }

    fn set_subscription(
        &mut self,
        client_id: u64,
        subscription_id: String,
        filters: Vec<HeadlessRelayFilter>,
    ) -> Vec<Event> {
        let matches = matching_events(self.events.iter(), filters.as_slice());
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.subscriptions.insert(subscription_id, filters);
        }
        matches
    }

    fn close_subscription(&mut self, client_id: u64, subscription_id: &str) {
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.subscriptions.remove(subscription_id);
        }
    }

    fn store_and_fanout(&mut self, event: Event) -> usize {
        if self.events.iter().any(|stored| stored.id == event.id) {
            return 0;
        }
        self.events.push_back(event.clone());
        while self.events.len() > self.event_capacity {
            self.events.pop_front();
        }

        let mut deliveries = Vec::<(mpsc::UnboundedSender<Message>, String)>::new();
        for client in self.clients.values() {
            for (subscription_id, filters) in &client.subscriptions {
                if filters.iter().any(|filter| filter.matches_event(&event)) {
                    let payload = serde_json::json!(["EVENT", subscription_id, event]);
                    deliveries.push((client.sender.clone(), payload.to_string()));
                }
            }
        }

        for (sender, payload) in deliveries {
            let _ = sender.send(Message::Text(payload.into()));
        }
        self.clients
            .values()
            .map(|client| {
                client
                    .subscriptions
                    .values()
                    .filter(|filters| filters.iter().any(|filter| filter.matches_event(&event)))
                    .count()
            })
            .sum()
    }
}

fn matching_events<'a>(
    events: impl Iterator<Item = &'a Event>,
    filters: &[HeadlessRelayFilter],
) -> Vec<Event> {
    if filters.is_empty() {
        return Vec::new();
    }

    let limit = filters
        .iter()
        .map(|filter| filter.limit)
        .max()
        .unwrap_or(256);
    let mut matching = Vec::<Event>::new();
    let mut seen = HashSet::<String>::new();
    for event in events {
        if filters.iter().any(|filter| filter.matches_event(event)) && seen.insert(event.id.clone())
        {
            matching.push(event.clone());
            if matching.len() >= limit {
                break;
            }
        }
    }
    matching
}

async fn handle_relay_connection(
    state: Arc<Mutex<HeadlessRelayState>>,
    stream: tokio::net::TcpStream,
) -> Result<()> {
    let websocket = accept_async(stream)
        .await
        .context("failed to upgrade headless relay websocket")?;
    let (mut writer, mut reader) = websocket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Message>();
    let writer_task = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            if writer.send(message).await.is_err() {
                break;
            }
        }
    });

    let client_id = {
        let mut guard = state.lock().await;
        guard.register_client(outbound_tx.clone())
    };

    while let Some(frame) = reader.next().await {
        let frame = frame.context("failed to read relay websocket frame")?;
        let Message::Text(text) = frame else {
            continue;
        };
        let value: Value =
            serde_json::from_str(text.as_str()).context("failed to parse relay frame JSON")?;
        let Some(frame) = value.as_array() else {
            continue;
        };
        let Some(kind) = frame.first().and_then(Value::as_str) else {
            continue;
        };
        match kind {
            "REQ" => {
                if frame.len() < 3 {
                    continue;
                }
                let Some(subscription_id) = frame.get(1).and_then(Value::as_str) else {
                    continue;
                };
                let filters = parse_relay_filters(&frame[2..]);
                info!(
                    target: "autopilot_desktop::headless_relay",
                    "REQ client_id={} subscription_id={} filters={}",
                    client_id,
                    subscription_id,
                    serde_json::to_string(&frame[2..]).unwrap_or_else(|_| "[]".to_string())
                );
                let matching = {
                    let mut guard = state.lock().await;
                    guard.set_subscription(client_id, subscription_id.to_string(), filters)
                };
                info!(
                    target: "autopilot_desktop::headless_relay",
                    "REQ client_id={} subscription_id={} replayed_events={}",
                    client_id,
                    subscription_id,
                    matching.len()
                );
                for event in matching {
                    let payload = serde_json::json!(["EVENT", subscription_id, event]);
                    let _ = outbound_tx.send(Message::Text(payload.to_string().into()));
                }
                let eose = serde_json::json!(["EOSE", subscription_id]);
                let _ = outbound_tx.send(Message::Text(eose.to_string().into()));
            }
            "EVENT" => {
                if frame.len() < 2 {
                    continue;
                }
                let Ok(event) = serde_json::from_value::<Event>(frame[1].clone()) else {
                    continue;
                };
                let matched_subscriptions = {
                    let mut guard = state.lock().await;
                    guard.store_and_fanout(event.clone())
                };
                info!(
                    target: "autopilot_desktop::headless_relay",
                    "EVENT client_id={} event_id={} kind={} pubkey={} fanout_matches={}",
                    client_id,
                    event.id,
                    event.kind,
                    event.pubkey,
                    matched_subscriptions
                );
                let ok = serde_json::json!(["OK", event.id, true, "accepted"]);
                let _ = outbound_tx.send(Message::Text(ok.to_string().into()));
            }
            "CLOSE" => {
                if let Some(subscription_id) = frame.get(1).and_then(Value::as_str) {
                    state
                        .lock()
                        .await
                        .close_subscription(client_id, subscription_id);
                }
            }
            _ => {}
        }
    }

    {
        let mut guard = state.lock().await;
        guard.remove_client(client_id);
    }
    writer_task.abort();
    Ok(())
}

fn parse_relay_filters(values: &[Value]) -> Vec<HeadlessRelayFilter> {
    values
        .iter()
        .filter_map(|value| value.as_object())
        .map(|object| {
            let kinds = object.get("kinds").and_then(Value::as_array).map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_u64)
                    .filter_map(|kind| u16::try_from(kind).ok())
                    .collect::<HashSet<_>>()
            });
            let e_tags = object.get("#e").and_then(Value::as_array).map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<HashSet<_>>()
            });
            let limit = object
                .get("limit")
                .and_then(Value::as_u64)
                .and_then(|limit| usize::try_from(limit).ok())
                .unwrap_or(256)
                .max(1);
            HeadlessRelayFilter {
                kinds,
                e_tags,
                limit,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveBuyerRequest, buyer_result_content, canned_provider_output, parse_relay_filters,
        truncate_for_log,
    };
    use crate::state::job_inbox::{JobInboxNetworkRequest, JobInboxValidation};
    use nostr::Event;

    fn fixture_request() -> JobInboxNetworkRequest {
        JobInboxNetworkRequest {
            request_id: "req-123".to_string(),
            requester: "buyer".to_string(),
            demand_source: crate::app_state::JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "text.generation".to_string(),
            execution_input: Some("input".to_string()),
            execution_prompt: Some("Reply with the exact text BUY MODE OK.".to_string()),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: Vec::new(),
            encrypted: false,
            encrypted_payload: None,
            parsed_event_shape: None,
            raw_event_json: None,
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some("req-123".to_string()),
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 75,
            validation: JobInboxValidation::Valid,
        }
    }

    #[test]
    fn relay_filter_matches_kind_and_request_reference() {
        let filters = parse_relay_filters(&[serde_json::json!({
            "kinds": [7000],
            "#e": ["req-123"],
            "limit": 16,
        })]);
        assert_eq!(filters.len(), 1);
        let filter = &filters[0];
        let event = Event {
            id: "feedback-123".to_string(),
            pubkey: "aa".repeat(32),
            created_at: 1_770_000_000,
            kind: 7000,
            tags: vec![vec!["e".to_string(), "req-123".to_string()]],
            content: "ok".to_string(),
            sig: "bb".repeat(64),
        };
        assert!(filter.matches_event(&event));
    }

    #[test]
    fn buyer_result_content_extracts_content_from_raw_event() {
        let raw = serde_json::json!({
            "id": "result-123",
            "pubkey": "aa",
            "created_at": 1,
            "kind": 6050,
            "tags": [["e", "req-123"]],
            "content": "BUY MODE OK.",
            "sig": "bb"
        })
        .to_string();
        assert_eq!(
            buyer_result_content(Some(raw.as_str())),
            Some("BUY MODE OK.".to_string())
        );
    }

    #[test]
    fn canned_provider_honors_buy_mode_prompt() {
        assert_eq!(canned_provider_output(&fixture_request()), "BUY MODE OK.");
    }

    #[test]
    fn buyer_request_success_requires_result_payment_and_provider_success_feedback() {
        let mut request = ActiveBuyerRequest::new("req-123".to_string());
        assert!(!request.is_terminal_success());
        request.result_event_id = Some("result-123".to_string());
        assert!(!request.is_terminal_success());
        request.payment_settled = true;
        assert!(!request.is_terminal_success());
        request.success_feedback_event_id = Some("feedback-123".to_string());
        assert!(request.is_terminal_success());
    }

    #[test]
    fn truncate_for_log_adds_ellipsis_when_needed() {
        let value = truncate_for_log("a b c d e f", 5);
        assert!(value.ends_with("..."));
    }
}
