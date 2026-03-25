use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use nostr::nip90::{
    JobFeedback, JobResult, JobStatus, create_job_feedback_event, create_job_result_event,
};
use nostr::{
    ChannelMetadata, Event, EventTemplate, GroupMetadata, GroupMetadataEvent,
    ManagedChannelCreateEvent, ManagedChannelHints, ManagedChannelType, NostrIdentity,
    finalize_event,
};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use tokio_tungstenite::{accept_async, tungstenite::Message};

use crate::app_state::{
    AutopilotChatState, DefaultNip28ChannelConfig, MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
    MISSION_CONTROL_BUY_MODE_INTERVAL, MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS,
    MISSION_CONTROL_BUY_MODE_REQUEST_TYPE, MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
    ManagedChatDeliveryState, ManagedChatProjectionState, NetworkRequestSubmission,
};
use crate::autopilot_compute_presence::pump_provider_chat_presence_with_config;
use crate::input::build_mission_control_buy_mode_request_event;
use crate::nip28_chat_lane::{Nip28ChatLaneUpdate, Nip28ChatLaneWorker};
use crate::nip90_compute_flow::buy_mode_request_flow_snapshots;
use crate::provider_nip90_lane::{
    ProviderNip90AuthIdentity, ProviderNip90BuyerResponseKind, ProviderNip90LaneCommand,
    ProviderNip90LaneUpdate, ProviderNip90LaneWorker, ProviderNip90PublishRole,
};
use crate::spark_wallet::SparkPaneState;
use crate::state::operations::{BuyerResolutionMode, NetworkRequestStatus, NetworkRequestsState};
use crate::state::provider_runtime::{ProviderMode, ProviderRuntimeState};

const BENCH_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const BENCH_LOOP_SLEEP: Duration = Duration::from_millis(5);
const DEFAULT_PROVIDER_COMPUTE_MS: u64 = 40;

#[derive(Clone, Debug, Serialize)]
pub struct ThroughputBenchReport {
    pub generated_at_epoch_seconds: u64,
    pub mission_control_buy_mode_interval_millis: u64,
    pub scenarios: Vec<ThroughputScenarioSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThroughputScenarioSummary {
    pub name: String,
    pub buyers: usize,
    pub providers: usize,
    pub total_jobs: usize,
    pub completed_jobs: usize,
    pub total_duration_ms: u64,
    pub jobs_per_minute: f64,
    pub p50_total_latency_ms: u64,
    pub p95_total_latency_ms: u64,
    pub p50_request_publish_latency_ms: u64,
    pub p50_result_latency_ms: u64,
    pub p50_payment_required_latency_ms: u64,
    pub p50_paid_latency_ms: u64,
    pub provider_distribution: Vec<ThroughputProviderDistribution>,
    pub jobs: Vec<ThroughputJobResult>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThroughputProviderDistribution {
    pub provider_pubkey: String,
    pub selected_jobs: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThroughputJobResult {
    pub request_id: String,
    pub buyer_label: String,
    pub provider_pubkey: String,
    pub dispatch_to_request_publish_ms: u64,
    pub dispatch_to_result_ms: u64,
    pub dispatch_to_payment_required_ms: u64,
    pub dispatch_to_paid_ms: u64,
}

#[derive(Clone, Debug)]
struct ScenarioConfig {
    name: &'static str,
    buyers: usize,
    providers: usize,
    jobs_per_buyer: usize,
    provider_compute_ms: u64,
    role_flip: bool,
}

struct BuyerHarness {
    label: String,
    identity: NostrIdentity,
    chat: AutopilotChatState,
    relay_urls: Vec<String>,
    chat_lane: Nip28ChatLaneWorker,
    request_lane: ProviderNip90LaneWorker,
    requests: NetworkRequestsState,
    wallet: SparkPaneState,
    remaining_jobs: usize,
    next_dispatch_at: Instant,
    next_command_seq: u64,
    active_request_id: Option<String>,
}

struct ProviderHarness {
    identity: NostrIdentity,
    chat: AutopilotChatState,
    relay_urls: Vec<String>,
    chat_lane: Nip28ChatLaneWorker,
    lane: ProviderNip90LaneWorker,
    runtime: ProviderRuntimeState,
    queue: VecDeque<QueuedProviderJob>,
    active: Option<ActiveProviderJob>,
}

#[derive(Clone, Debug)]
struct QueuedProviderJob {
    request: crate::app_state::JobInboxNetworkRequest,
}

#[derive(Clone, Debug)]
struct ActiveProviderJob {
    request: crate::app_state::JobInboxNetworkRequest,
    ready_at: Instant,
    events_enqueued: bool,
    invoice: String,
}

#[derive(Clone, Debug)]
struct JobMetric {
    request_id: String,
    buyer_label: String,
    provider_pubkey: String,
    dispatch_at: Instant,
    request_published_at: Option<Instant>,
    result_at: Option<Instant>,
    payment_required_at: Option<Instant>,
    paid_at: Option<Instant>,
}

pub fn run_default_throughput_bench(
    provider_compute_ms: Option<u64>,
) -> Result<ThroughputBenchReport> {
    let provider_compute_ms = provider_compute_ms.unwrap_or(DEFAULT_PROVIDER_COMPUTE_MS);
    let scenarios = vec![
        ScenarioConfig {
            name: "single_pair_serial",
            buyers: 1,
            providers: 1,
            jobs_per_buyer: 3,
            provider_compute_ms,
            role_flip: false,
        },
        ScenarioConfig {
            name: "multi_buyer_multi_provider",
            buyers: 3,
            providers: 3,
            jobs_per_buyer: 2,
            provider_compute_ms,
            role_flip: false,
        },
        ScenarioConfig {
            name: "role_flip_pair",
            buyers: 1,
            providers: 1,
            jobs_per_buyer: 2,
            provider_compute_ms,
            role_flip: true,
        },
    ];

    let mut summaries = Vec::with_capacity(scenarios.len());
    for scenario in &scenarios {
        summaries.push(run_scenario(scenario)?);
    }

    Ok(ThroughputBenchReport {
        generated_at_epoch_seconds: current_epoch_seconds(),
        mission_control_buy_mode_interval_millis: MISSION_CONTROL_BUY_MODE_INTERVAL_MILLIS,
        scenarios: summaries,
    })
}

fn run_scenario(config: &ScenarioConfig) -> Result<ThroughputScenarioSummary> {
    if config.role_flip {
        let first_phase = run_standard_phase(config, 0x31, 0x41, "phase_a")?;
        let second_phase = run_standard_phase(config, 0x61, 0x71, "phase_b")?;
        return summarize_scenario(
            config,
            first_phase
                .into_iter()
                .chain(second_phase)
                .collect::<Vec<_>>()
                .as_slice(),
        );
    }

    let metrics = run_standard_phase(config, 0x11, 0x21, config.name)?;
    summarize_scenario(config, metrics.as_slice())
}

fn run_standard_phase(
    config: &ScenarioConfig,
    buyer_seed_base: u8,
    provider_seed_base: u8,
    label_prefix: &str,
) -> Result<Vec<JobMetric>> {
    let relay = TestNip28Relay::spawn();
    let channel_id = "ab".repeat(32);
    let chat_config = DefaultNip28ChannelConfig {
        relay_url: relay.url.clone(),
        channel_id: channel_id.clone(),
        team_channel_id: None,
        private_key_hex: None,
    };
    relay.store_events(vec![
        build_group_metadata_event(),
        build_channel_create_event(chat_config.channel_id.as_str()),
    ]);

    let mut buyers = Vec::with_capacity(config.buyers);
    for index in 0..config.buyers {
        let identity = test_identity(
            buyer_seed_base.saturating_add(index as u8),
            format!("{label_prefix}-buyer-{index}").as_str(),
        );
        let label = format!("buyer-{index}");
        let projection_path = projection_path(format!("{label_prefix}-{label}-chat").as_str());
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(projection_path);
        chat.managed_chat_projection
            .set_local_pubkey(Some(identity.public_key_hex.as_str()));
        buyers.push(BuyerHarness {
            label,
            identity,
            chat,
            relay_urls: vec![relay.url.clone()],
            chat_lane: Nip28ChatLaneWorker::spawn_with_config(chat_config.clone()),
            request_lane: ProviderNip90LaneWorker::spawn(vec![relay.url.clone()]),
            requests: NetworkRequestsState::default(),
            wallet: funded_wallet(config.jobs_per_buyer as u64),
            remaining_jobs: config.jobs_per_buyer,
            next_dispatch_at: Instant::now(),
            next_command_seq: 1,
            active_request_id: None,
        });
    }

    let mut providers = Vec::with_capacity(config.providers);
    for index in 0..config.providers {
        let identity = test_identity(
            provider_seed_base.saturating_add(index as u8),
            format!("{label_prefix}-provider-{index}").as_str(),
        );
        let projection_path =
            projection_path(format!("{label_prefix}-provider-{index}-chat").as_str());
        let mut chat = AutopilotChatState::default();
        chat.managed_chat_projection =
            ManagedChatProjectionState::from_projection_path_for_tests(projection_path);
        chat.managed_chat_projection
            .set_local_pubkey(Some(identity.public_key_hex.as_str()));
        let lane = ProviderNip90LaneWorker::spawn(vec![relay.url.clone()]);
        lane.enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
            identity: Some(provider_auth_identity(&identity)),
        })
        .map_err(|error| anyhow!(error))?;
        lane.enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
            capability: fixture_compute_capability(),
        })
        .map_err(|error| anyhow!(error))?;
        lane.enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .map_err(|error| anyhow!(error))?;
        providers.push(ProviderHarness {
            identity,
            chat,
            relay_urls: vec![relay.url.clone()],
            chat_lane: Nip28ChatLaneWorker::spawn_with_config(chat_config.clone()),
            lane,
            runtime: ready_provider_runtime(Instant::now()),
            queue: VecDeque::new(),
            active: None,
        });
    }

    for buyer in &buyers {
        let identity = provider_auth_identity(&buyer.identity);
        buyer
            .request_lane
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(identity),
            })
            .map_err(|error| anyhow!(error))?;
    }

    wait_for_channels_loaded(
        &chat_config,
        buyers.as_mut_slice(),
        providers.as_mut_slice(),
    )?;
    wait_for_provider_lanes_online(providers.as_mut_slice())?;
    publish_provider_presence(
        &chat_config,
        buyers.as_mut_slice(),
        providers.as_mut_slice(),
    )?;

    let total_jobs = config.buyers.saturating_mul(config.jobs_per_buyer);
    let mut metrics = HashMap::<String, JobMetric>::new();
    let started_at = Instant::now();

    while metrics
        .values()
        .filter(|metric| metric.paid_at.is_some())
        .count()
        < total_jobs
    {
        let now = Instant::now();
        let now_epoch_seconds = current_epoch_seconds();

        pump_all_chat_lanes(buyers.as_mut_slice(), providers.as_mut_slice());
        dispatch_due_requests(
            buyers.as_mut_slice(),
            &relay.url,
            &chat_config,
            now,
            now_epoch_seconds,
            &mut metrics,
        )?;
        process_provider_updates(
            providers.as_mut_slice(),
            &mut metrics,
            config.provider_compute_ms,
            now,
        );
        advance_provider_work(providers.as_mut_slice(), now);
        process_buyer_updates(
            buyers.as_mut_slice(),
            &mut metrics,
            now_epoch_seconds,
            &chat_config,
        );
        release_paid_provider_slots(providers.as_mut_slice(), &metrics);

        if started_at.elapsed() > Duration::from_secs(180) {
            return Err(anyhow!(
                "throughput benchmark timed out after {:?}",
                started_at.elapsed()
            ));
        }
        thread::sleep(BENCH_LOOP_SLEEP);
    }

    let mut rows = metrics.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.dispatch_at.cmp(&right.dispatch_at));
    Ok(rows)
}

fn funded_wallet(job_count: u64) -> SparkPaneState {
    let mut wallet = SparkPaneState::default();
    wallet.balance = Some(openagents_spark::Balance {
        spark_sats: job_count
            .saturating_mul(MISSION_CONTROL_BUY_MODE_BUDGET_SATS.saturating_add(4))
            .saturating_add(1_000),
        lightning_sats: 0,
        onchain_sats: 0,
    });
    wallet
}

fn wait_for_channels_loaded(
    config: &DefaultNip28ChannelConfig,
    buyers: &mut [BuyerHarness],
    providers: &mut [ProviderHarness],
) -> Result<()> {
    let deadline = Instant::now() + BENCH_WAIT_TIMEOUT;
    while Instant::now() < deadline {
        pump_all_chat_lanes(buyers, providers);
        let buyers_ready = buyers.iter().all(|buyer| {
            buyer
                .chat
                .configured_main_managed_chat_channel(config)
                .is_some()
        });
        let providers_ready = providers.iter().all(|provider| {
            provider
                .chat
                .configured_main_managed_chat_channel(config)
                .is_some()
        });
        if buyers_ready && providers_ready {
            return Ok(());
        }
        thread::sleep(BENCH_LOOP_SLEEP);
    }
    Err(anyhow!("timed out waiting for NIP-28 main channel to load"))
}

fn publish_provider_presence(
    config: &DefaultNip28ChannelConfig,
    buyers: &mut [BuyerHarness],
    providers: &mut [ProviderHarness],
) -> Result<()> {
    let now = Instant::now();
    let now_epoch_seconds = current_epoch_seconds();
    for provider in providers.iter_mut() {
        let published = pump_provider_chat_presence_with_config(
            &mut provider.runtime,
            &mut provider.chat,
            Some(&provider.identity),
            now,
            now_epoch_seconds,
            config,
        );
        if !published {
            return Err(anyhow!("provider presence did not publish"));
        }
        let channel = provider
            .chat
            .configured_main_managed_chat_channel(config)
            .cloned()
            .ok_or_else(|| anyhow!("configured main channel missing for provider"))?;
        let _ = crate::input::queue_managed_chat_message_to_channel_with_relay(
            &mut provider.chat,
            &provider.identity,
            channel.group_id.as_str(),
            channel.channel_id.as_str(),
            Some(config.relay_url.as_str()),
            format!("{} ready for compute", provider.identity.public_key_hex).as_str(),
            None,
        );
    }

    let deadline = Instant::now() + BENCH_WAIT_TIMEOUT;
    while Instant::now() < deadline {
        pump_all_chat_lanes(buyers, providers);
        let all_buyers_ready = buyers.iter().all(|buyer| {
            let selection = buyer
                .chat
                .select_autopilot_buy_mode_target_with_config(config, current_epoch_seconds());
            selection.eligible_peer_count >= providers.len()
        });
        if all_buyers_ready {
            return Ok(());
        }
        thread::sleep(BENCH_LOOP_SLEEP);
    }
    Err(anyhow!(
        "timed out waiting for buyers to observe provider presence"
    ))
}

fn wait_for_provider_lanes_online(providers: &mut [ProviderHarness]) -> Result<()> {
    let deadline = Instant::now() + BENCH_WAIT_TIMEOUT;
    let mut online = vec![false; providers.len()];
    while Instant::now() < deadline {
        for (index, provider) in providers.iter_mut().enumerate() {
            for update in provider.lane.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == crate::provider_nip90_lane::ProviderNip90LaneMode::Online
                    && snapshot.connected_relays > 0
                {
                    online[index] = true;
                }
            }
        }
        if online.iter().all(|value| *value) {
            return Ok(());
        }
        thread::sleep(BENCH_LOOP_SLEEP);
    }
    Err(anyhow!(
        "timed out waiting for provider lanes to come online"
    ))
}

fn pump_all_chat_lanes(buyers: &mut [BuyerHarness], providers: &mut [ProviderHarness]) {
    for buyer in buyers {
        pump_nip28_lane(
            &mut buyer.chat,
            &mut buyer.chat_lane,
            buyer.relay_urls.as_slice(),
        );
    }
    for provider in providers {
        pump_nip28_lane(
            &mut provider.chat,
            &mut provider.chat_lane,
            provider.relay_urls.as_slice(),
        );
    }
}

fn dispatch_due_requests(
    buyers: &mut [BuyerHarness],
    relay_url: &str,
    config: &DefaultNip28ChannelConfig,
    now: Instant,
    now_epoch_seconds: u64,
    metrics: &mut HashMap<String, JobMetric>,
) -> Result<()> {
    for buyer in buyers {
        if buyer.remaining_jobs == 0
            || buyer.active_request_id.is_some()
            || buyer.next_dispatch_at > now
        {
            continue;
        }
        let selection = buyer
            .chat
            .select_autopilot_buy_mode_target_with_config(config, now_epoch_seconds);
        let Some(target_provider_pubkey) = selection.selected_peer_pubkey.clone() else {
            buyer.next_dispatch_at = now + MISSION_CONTROL_BUY_MODE_INTERVAL;
            continue;
        };
        let request_event = build_mission_control_buy_mode_request_event(
            Some(&buyer.identity),
            &[relay_url.to_string()],
            &[target_provider_pubkey.clone()],
        )
        .map_err(|error| anyhow!(error))?;
        let request_id = buyer
            .requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some(request_event.id.clone()),
                request_type: MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: crate::headless_compute::HEADLESS_BUY_MODE_PROMPT.to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec![target_provider_pubkey.clone()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: MISSION_CONTROL_BUY_MODE_BUDGET_SATS,
                timeout_seconds: MISSION_CONTROL_BUY_MODE_TIMEOUT_SECONDS,
                authority_command_seq: buyer.next_command_seq,
            })
            .map_err(|error| anyhow!(error))?;
        buyer.next_command_seq = buyer.next_command_seq.saturating_add(1);
        buyer
            .chat
            .note_buy_mode_target_dispatch(target_provider_pubkey.as_str());
        buyer
            .request_lane
            .enqueue(ProviderNip90LaneCommand::TrackBuyerRequestIds {
                request_ids: vec![request_id.clone()],
            })
            .map_err(|error| anyhow!(error))?;
        buyer
            .request_lane
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request_id.clone(),
                role: ProviderNip90PublishRole::Request,
                event: Box::new(request_event),
            })
            .map_err(|error| anyhow!(error))?;
        buyer.remaining_jobs = buyer.remaining_jobs.saturating_sub(1);
        buyer.active_request_id = Some(request_id.clone());
        buyer.next_dispatch_at = now + MISSION_CONTROL_BUY_MODE_INTERVAL;
        metrics.insert(
            request_id.clone(),
            JobMetric {
                request_id,
                buyer_label: buyer.label.clone(),
                provider_pubkey: target_provider_pubkey,
                dispatch_at: now,
                request_published_at: None,
                result_at: None,
                payment_required_at: None,
                paid_at: None,
            },
        );
    }
    Ok(())
}

fn process_provider_updates(
    providers: &mut [ProviderHarness],
    metrics: &mut HashMap<String, JobMetric>,
    provider_compute_ms: u64,
    now: Instant,
) {
    for provider in providers {
        for update in provider.lane.drain_updates() {
            match update {
                ProviderNip90LaneUpdate::IngressedRequest(request) => {
                    if let Some(metric) = metrics.get_mut(request.request_id.as_str())
                        && metric.result_at.is_none()
                    {
                        provider.queue.push_back(QueuedProviderJob { request });
                    }
                }
                ProviderNip90LaneUpdate::PublishOutcome(outcome) => {
                    if outcome.role == ProviderNip90PublishRole::Result
                        && let Some(metric) = metrics.get_mut(outcome.request_id.as_str())
                        && metric.request_published_at.is_none()
                    {
                        metric.request_published_at = Some(now);
                    }
                }
                ProviderNip90LaneUpdate::Snapshot(_)
                | ProviderNip90LaneUpdate::BuyerResponseEvent(_) => {}
            }
        }

        if provider.active.is_none()
            && let Some(queued) = provider.queue.pop_front()
        {
            provider.active = Some(ActiveProviderJob {
                request: queued.request,
                ready_at: now + Duration::from_millis(provider_compute_ms),
                events_enqueued: false,
                invoice: String::new(),
            });
        }
    }
}

fn advance_provider_work(providers: &mut [ProviderHarness], now: Instant) {
    for provider in providers {
        let Some(active) = provider.active.as_mut() else {
            continue;
        };
        if active.events_enqueued || active.ready_at > now {
            continue;
        }
        active.invoice = format!(
            "lnbc{}n{}",
            active.request.price_sats.saturating_mul(10),
            active.request.request_id
        );
        let result_event =
            build_provider_result_event(&provider.identity, &active.request, "BUY MODE OK.");
        let feedback_event = build_provider_payment_required_feedback_event(
            &provider.identity,
            &active.request,
            active.invoice.as_str(),
        );
        let request_id = active.request.request_id.clone();
        let _ = provider
            .lane
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request_id.clone(),
                role: ProviderNip90PublishRole::Result,
                event: Box::new(result_event),
            });
        let _ = provider
            .lane
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id,
                role: ProviderNip90PublishRole::Feedback,
                event: Box::new(feedback_event),
            });
        active.events_enqueued = true;
    }
}

fn process_buyer_updates(
    buyers: &mut [BuyerHarness],
    metrics: &mut HashMap<String, JobMetric>,
    now_epoch_seconds: u64,
    config: &DefaultNip28ChannelConfig,
) {
    for buyer in buyers {
        let active_request_id = buyer.active_request_id.clone();
        for update in buyer.request_lane.drain_updates() {
            match update {
                ProviderNip90LaneUpdate::PublishOutcome(outcome) => {
                    if outcome.role == ProviderNip90PublishRole::Request {
                        buyer.requests.apply_nip90_request_publish_outcome(
                            outcome.request_id.as_str(),
                            outcome.event_id.as_str(),
                            outcome.accepted_relays,
                            outcome.rejected_relays,
                            outcome.first_error.as_deref(),
                        );
                        if let Some(metric) = metrics.get_mut(outcome.request_id.as_str()) {
                            metric.request_published_at.get_or_insert(Instant::now());
                        }
                    }
                }
                ProviderNip90LaneUpdate::BuyerResponseEvent(event) => match event.kind {
                    ProviderNip90BuyerResponseKind::Result => {
                        let _ = buyer.requests.apply_nip90_buyer_result_event(
                            event.request_id.as_str(),
                            event.provider_pubkey.as_str(),
                            event.event_id.as_str(),
                            event.status.as_deref(),
                        );
                        if let Some(metric) = metrics.get_mut(event.request_id.as_str()) {
                            metric.result_at.get_or_insert(Instant::now());
                        }
                    }
                    ProviderNip90BuyerResponseKind::Feedback => {
                        let _ = buyer.requests.apply_nip90_buyer_feedback_event(
                            event.request_id.as_str(),
                            event.provider_pubkey.as_str(),
                            event.event_id.as_str(),
                            event.status.as_deref(),
                            event.status_extra.as_deref(),
                            event.amount_msats,
                            event.bolt11.as_deref(),
                        );
                        if event.status.as_deref() == Some("payment-required")
                            && let Some(metric) = metrics.get_mut(event.request_id.as_str())
                        {
                            metric.payment_required_at.get_or_insert(Instant::now());
                        }
                        if let Some((_bolt11, amount_sats)) =
                            buyer.requests.prepare_auto_payment_attempt_for_provider(
                                event.request_id.as_str(),
                                event.provider_pubkey.as_str(),
                                now_epoch_seconds.saturating_add(30),
                            )
                        {
                            let amount_sats =
                                amount_sats.unwrap_or(MISSION_CONTROL_BUY_MODE_BUDGET_SATS);
                            let payment_id = format!("wallet-{}-{}", buyer.label, event.request_id);
                            buyer.requests.record_auto_payment_pointer(
                                event.request_id.as_str(),
                                payment_id.as_str(),
                            );
                            buyer.requests.mark_auto_payment_sent(
                                event.request_id.as_str(),
                                payment_id.as_str(),
                                now_epoch_seconds.saturating_add(31),
                            );
                            if let Some(balance) = buyer.wallet.balance.as_mut() {
                                balance.spark_sats = balance.spark_sats.saturating_sub(amount_sats);
                            }
                            buyer
                                .wallet
                                .recent_payments
                                .push(openagents_spark::PaymentSummary {
                                    id: payment_id,
                                    direction: "send".to_string(),
                                    status: "succeeded".to_string(),
                                    amount_sats,
                                    fees_sats: 0,
                                    timestamp: now_epoch_seconds.saturating_add(31),
                                    method: "lightning".to_string(),
                                    description: Some(
                                        "Throughput benchmark settlement".to_string(),
                                    ),
                                    invoice: event.bolt11.clone(),
                                    destination_pubkey: Some(event.provider_pubkey.clone()),
                                    payment_hash: Some(format!(
                                        "payment-hash-{}",
                                        event.request_id
                                    )),
                                    htlc_status: None,
                                    htlc_expiry_epoch_seconds: None,
                                    status_detail: None,
                                });
                        }
                    }
                },
                ProviderNip90LaneUpdate::IngressedRequest(_)
                | ProviderNip90LaneUpdate::Snapshot(_) => {}
            }
        }

        if let Some(request_id) = active_request_id
            && let Some(flow) = buy_mode_request_flow_snapshots(&buyer.requests, &buyer.wallet)
                .into_iter()
                .find(|flow| flow.request_id == request_id)
            && flow.status == NetworkRequestStatus::Paid
        {
            if let Some(metric) = metrics.get_mut(request_id.as_str()) {
                metric.paid_at.get_or_insert(Instant::now());
            }
            buyer.active_request_id = None;
        }

        if buyer
            .chat
            .configured_main_managed_chat_channel(config)
            .is_none()
        {
            let _ = buyer.chat.maybe_auto_select_default_nip28_channel();
        }
    }
}

fn release_paid_provider_slots(
    providers: &mut [ProviderHarness],
    metrics: &HashMap<String, JobMetric>,
) {
    for provider in providers {
        let Some(active) = provider.active.as_ref() else {
            continue;
        };
        let paid = metrics
            .get(active.request.request_id.as_str())
            .is_some_and(|metric| metric.paid_at.is_some());
        if paid {
            provider.active = None;
        }
    }
}

fn summarize_scenario(
    config: &ScenarioConfig,
    metrics: &[JobMetric],
) -> Result<ThroughputScenarioSummary> {
    if metrics.is_empty() {
        return Err(anyhow!("scenario {} produced no metrics", config.name));
    }
    let mut jobs = Vec::with_capacity(metrics.len());
    let mut total_latencies = Vec::with_capacity(metrics.len());
    let mut request_publish_latencies = Vec::with_capacity(metrics.len());
    let mut result_latencies = Vec::with_capacity(metrics.len());
    let mut payment_required_latencies = Vec::with_capacity(metrics.len());
    let mut paid_latencies = Vec::with_capacity(metrics.len());
    let mut provider_distribution = BTreeMap::<String, usize>::new();
    let first_dispatch = metrics
        .iter()
        .map(|metric| metric.dispatch_at)
        .min()
        .ok_or_else(|| anyhow!("missing first dispatch"))?;
    let last_paid = metrics
        .iter()
        .filter_map(|metric| metric.paid_at)
        .max()
        .ok_or_else(|| anyhow!("missing final paid timestamp"))?;

    for metric in metrics {
        let request_publish_latency =
            duration_between(metric.dispatch_at, metric.request_published_at)?;
        let result_latency = duration_between(metric.dispatch_at, metric.result_at)?;
        let payment_required_latency =
            duration_between(metric.dispatch_at, metric.payment_required_at)?;
        let paid_latency = duration_between(metric.dispatch_at, metric.paid_at)?;
        total_latencies.push(paid_latency);
        request_publish_latencies.push(request_publish_latency);
        result_latencies.push(result_latency);
        payment_required_latencies.push(payment_required_latency);
        paid_latencies.push(paid_latency);
        *provider_distribution
            .entry(metric.provider_pubkey.clone())
            .or_insert(0) += 1;
        jobs.push(ThroughputJobResult {
            request_id: metric.request_id.clone(),
            buyer_label: metric.buyer_label.clone(),
            provider_pubkey: metric.provider_pubkey.clone(),
            dispatch_to_request_publish_ms: request_publish_latency,
            dispatch_to_result_ms: result_latency,
            dispatch_to_payment_required_ms: payment_required_latency,
            dispatch_to_paid_ms: paid_latency,
        });
    }

    let total_duration_ms = millis_between(first_dispatch, last_paid);
    let jobs_per_minute = if total_duration_ms == 0 {
        jobs.len() as f64
    } else {
        (jobs.len() as f64) * 60_000.0 / (total_duration_ms as f64)
    };

    Ok(ThroughputScenarioSummary {
        name: config.name.to_string(),
        buyers: config.buyers,
        providers: config.providers,
        total_jobs: config.buyers.saturating_mul(config.jobs_per_buyer)
            * if config.role_flip { 2 } else { 1 },
        completed_jobs: jobs.len(),
        total_duration_ms,
        jobs_per_minute,
        p50_total_latency_ms: percentile(total_latencies.as_mut_slice(), 0.50),
        p95_total_latency_ms: percentile(total_latencies.as_mut_slice(), 0.95),
        p50_request_publish_latency_ms: percentile(request_publish_latencies.as_mut_slice(), 0.50),
        p50_result_latency_ms: percentile(result_latencies.as_mut_slice(), 0.50),
        p50_payment_required_latency_ms: percentile(
            payment_required_latencies.as_mut_slice(),
            0.50,
        ),
        p50_paid_latency_ms: percentile(paid_latencies.as_mut_slice(), 0.50),
        provider_distribution: provider_distribution
            .into_iter()
            .map(
                |(provider_pubkey, selected_jobs)| ThroughputProviderDistribution {
                    provider_pubkey,
                    selected_jobs,
                },
            )
            .collect(),
        jobs,
    })
}

fn duration_between(start: Instant, end: Option<Instant>) -> Result<u64> {
    let end = end.ok_or_else(|| anyhow!("missing lifecycle timestamp"))?;
    Ok(millis_between(start, end))
}

fn millis_between(start: Instant, end: Instant) -> u64 {
    let duration = end.saturating_duration_since(start);
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn percentile(values: &mut [u64], ratio: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = ((values.len().saturating_sub(1)) as f64 * ratio).round() as usize;
    values[index.min(values.len().saturating_sub(1))]
}

fn projection_path(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join("openagents-throughput-bench");
    let _ = fs::create_dir_all(&root);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    root.join(format!("{label}-{nanos}.json"))
}

fn current_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn repeated_hex(ch: char, len: usize) -> String {
    std::iter::repeat_n(ch, len).collect()
}

fn test_identity(seed: u8, label: &str) -> NostrIdentity {
    let mnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean";
    let keypair = nostr::derive_keypair_with_account(mnemonic, u32::from(seed.max(1)))
        .expect("derive deterministic benchmark keypair");
    NostrIdentity {
        identity_path: PathBuf::from(format!("/tmp/openagents-throughput-{label}")),
        mnemonic: mnemonic.to_string(),
        npub: keypair.npub().expect("benchmark npub"),
        nsec: keypair.nsec().expect("benchmark nsec"),
        public_key_hex: keypair.public_key_hex(),
        private_key_hex: keypair.private_key_hex(),
    }
}

fn provider_auth_identity(identity: &NostrIdentity) -> ProviderNip90AuthIdentity {
    ProviderNip90AuthIdentity {
        npub: identity.npub.clone(),
        public_key_hex: identity.public_key_hex.clone(),
        private_key_hex: identity.private_key_hex.clone(),
    }
}

fn ready_provider_runtime(now: Instant) -> ProviderRuntimeState {
    let mut runtime = ProviderRuntimeState::default();
    runtime.mode = ProviderMode::Online;
    runtime.mode_changed_at = now;
    runtime.inventory_session_started_at_ms = Some(25_000);
    runtime.apple_fm.reachable = true;
    runtime.apple_fm.model_available = true;
    runtime.apple_fm.ready_model = Some("apple-foundation-model".to_string());
    runtime
}

fn fixture_compute_capability() -> crate::provider_nip90_lane::ProviderNip90ComputeCapability {
    crate::provider_nip90_lane::ProviderNip90ComputeCapability {
        backend: "apple-foundation-model".to_string(),
        reachable: true,
        configured_model: Some("apple-foundation-model".to_string()),
        ready_model: Some("apple-foundation-model".to_string()),
        available_models: vec!["apple-foundation-model".to_string()],
        loaded_models: vec!["apple-foundation-model".to_string()],
        last_error: None,
    }
}

fn sign_template(identity: &NostrIdentity, template: &EventTemplate) -> Event {
    let key_bytes = hex::decode(identity.private_key_hex.as_str()).expect("decode key hex");
    let private_key: [u8; 32] = key_bytes.try_into().expect("32 byte private key");
    finalize_event(template, &private_key).expect("sign test event")
}

fn build_group_metadata_event() -> Event {
    let template = GroupMetadataEvent::new(
        "oa-main",
        GroupMetadata::new().with_name("OpenAgents Main"),
        10,
    )
    .expect("group metadata");
    Event {
        id: repeated_hex('a', 64),
        pubkey: repeated_hex('1', 64),
        created_at: 10,
        kind: 39_000,
        tags: template.to_tags(),
        content: String::new(),
        sig: repeated_hex('f', 128),
    }
}

fn build_channel_create_event(channel_id: &str) -> Event {
    let template = ManagedChannelCreateEvent::new(
        "oa-main",
        ChannelMetadata::new("main", "OpenAgents main channel", ""),
        20,
    )
    .expect("channel create")
    .with_hints(
        ManagedChannelHints::new()
            .with_slug("main")
            .with_channel_type(ManagedChannelType::Ops)
            .with_category_id("main")
            .with_category_label("Main")
            .with_position(1),
    )
    .expect("channel hints");
    Event {
        id: channel_id.to_string(),
        pubkey: repeated_hex('2', 64),
        created_at: 20,
        kind: 40,
        tags: template.to_tags().expect("channel tags"),
        content: template.content().expect("channel content"),
        sig: repeated_hex('f', 128),
    }
}

fn build_provider_result_event(
    identity: &NostrIdentity,
    request: &crate::app_state::JobInboxNetworkRequest,
    output: &str,
) -> Event {
    let mut result = JobResult::new(
        request.request_kind,
        request.request_id.clone(),
        request.requester.clone(),
        output.trim().to_string(),
    )
    .expect("provider result");
    if request.price_sats > 0 {
        result = result.with_amount(request.price_sats.saturating_mul(1000), None);
    }
    sign_template(identity, &create_job_result_event(&result))
}

fn build_provider_payment_required_feedback_event(
    identity: &NostrIdentity,
    request: &crate::app_state::JobInboxNetworkRequest,
    bolt11: &str,
) -> Event {
    let feedback = JobFeedback::new(
        JobStatus::PaymentRequired,
        request.request_id.as_str(),
        request.requester.as_str(),
    )
    .with_status_extra("lightning settlement required".to_string())
    .with_amount(
        request.price_sats.saturating_mul(1000),
        Some(bolt11.to_string()),
    );
    sign_template(identity, &create_job_feedback_event(&feedback))
}

fn pump_nip28_lane(
    chat: &mut AutopilotChatState,
    lane_worker: &mut Nip28ChatLaneWorker,
    relay_urls: &[String],
) {
    for update in lane_worker.drain_updates() {
        match update {
            Nip28ChatLaneUpdate::RelayEvent(event) => {
                chat.managed_chat_projection.record_relay_event(event);
            }
            Nip28ChatLaneUpdate::PublishAck { event_id } => {
                let _ = chat.managed_chat_projection.ack_outbound_message(&event_id);
                lane_worker.clear_dispatched(&event_id);
            }
            Nip28ChatLaneUpdate::PublishError { event_id, message } => {
                let _ = chat
                    .managed_chat_projection
                    .fail_outbound_message(&event_id, &message);
                lane_worker.clear_dispatched(&event_id);
            }
            Nip28ChatLaneUpdate::Eose { .. }
            | Nip28ChatLaneUpdate::ConnectionError { .. }
            | Nip28ChatLaneUpdate::AuthChallengeReceived { .. } => {}
            Nip28ChatLaneUpdate::Snapshot(snapshot) => {
                chat.managed_chat_lane = snapshot;
            }
        }
    }
    let pending_events = chat
        .managed_chat_projection
        .outbound_messages
        .iter()
        .filter(|message| message.delivery_state == ManagedChatDeliveryState::Publishing)
        .map(|message| message.event.clone())
        .collect::<Vec<_>>();
    for event in pending_events {
        lane_worker.publish(event);
    }
    lane_worker.sync_managed_chat_subscriptions(
        relay_urls.to_vec(),
        chat.managed_chat_projection.discovered_channel_ids(),
        chat.managed_chat_projection.subscription_since_created_at(
            crate::nip28_chat_lane::NIP28_CHAT_BACKFILL_OVERLAP_SECS,
        ),
    );
    let _ = chat.maybe_auto_select_default_nip28_channel();
}

#[derive(Clone, Debug, Default)]
struct TestRelayFilter {
    ids: Option<HashSet<String>>,
    kinds: Option<HashSet<u16>>,
    e_tags: Option<HashSet<String>>,
    limit: usize,
}

impl TestRelayFilter {
    fn matches_event(&self, event: &Event) -> bool {
        if let Some(ids) = self.ids.as_ref()
            && !ids.contains(event.id.as_str())
        {
            return false;
        }
        if let Some(kinds) = self.kinds.as_ref()
            && !kinds.contains(&event.kind)
        {
            return false;
        }
        if let Some(expected_e_tags) = self.e_tags.as_ref() {
            let matched = event.tags.iter().any(|tag| {
                tag.first().is_some_and(|value| value == "e")
                    && tag
                        .get(1)
                        .is_some_and(|value| expected_e_tags.contains(value.as_str()))
            });
            if !matched {
                return false;
            }
        }
        true
    }
}

struct TestRelayClient {
    sender: tokio_mpsc::UnboundedSender<Message>,
    subscriptions: HashMap<String, Vec<TestRelayFilter>>,
}

struct TestRelayState {
    next_client_id: u64,
    events: VecDeque<Event>,
    clients: HashMap<u64, TestRelayClient>,
}

impl TestRelayState {
    fn new() -> Self {
        Self {
            next_client_id: 0,
            events: VecDeque::new(),
            clients: HashMap::new(),
        }
    }

    fn register_client(&mut self, sender: tokio_mpsc::UnboundedSender<Message>) -> u64 {
        self.next_client_id = self.next_client_id.saturating_add(1);
        let client_id = self.next_client_id;
        self.clients.insert(
            client_id,
            TestRelayClient {
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
        filters: Vec<TestRelayFilter>,
    ) -> Vec<Event> {
        let matching = matching_events(self.events.iter(), filters.as_slice());
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.subscriptions.insert(subscription_id, filters);
        }
        matching
    }

    fn close_subscription(&mut self, client_id: u64, subscription_id: &str) {
        if let Some(client) = self.clients.get_mut(&client_id) {
            client.subscriptions.remove(subscription_id);
        }
    }

    fn store_and_fanout(&mut self, event: Event) {
        if self.events.iter().any(|stored| stored.id == event.id) {
            return;
        }
        self.events.push_back(event.clone());
        let mut deliveries = Vec::<(tokio_mpsc::UnboundedSender<Message>, String)>::new();
        for client in self.clients.values() {
            for (subscription_id, filters) in &client.subscriptions {
                if filters.iter().any(|filter| filter.matches_event(&event)) {
                    let payload = json!(["EVENT", subscription_id, event]);
                    deliveries.push((client.sender.clone(), payload.to_string()));
                }
            }
        }
        for (sender, payload) in deliveries {
            let _ = sender.send(Message::Text(payload.into()));
        }
    }
}

fn parse_filters(values: &[Value]) -> Vec<TestRelayFilter> {
    values
        .iter()
        .filter_map(Value::as_object)
        .map(|object| {
            let ids = object.get("ids").and_then(Value::as_array).map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect::<HashSet<_>>()
            });
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
            TestRelayFilter {
                ids,
                kinds,
                e_tags,
                limit,
            }
        })
        .collect()
}

fn matching_events<'a>(
    events: impl Iterator<Item = &'a Event>,
    filters: &[TestRelayFilter],
) -> Vec<Event> {
    if filters.is_empty() {
        return Vec::new();
    }
    let limit = filters
        .iter()
        .map(|filter| filter.limit)
        .max()
        .unwrap_or(256);
    let mut matching = Vec::new();
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

async fn handle_relay_connection(state: Arc<Mutex<TestRelayState>>, stream: tokio::net::TcpStream) {
    let websocket = accept_async(stream)
        .await
        .expect("upgrade websocket relay connection");
    let (mut writer, mut reader) = websocket.split();
    let (outbound_tx, mut outbound_rx) = tokio_mpsc::unbounded_channel::<Message>();
    let writer_task = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            if writer.send(message).await.is_err() {
                break;
            }
        }
    });

    let client_id = {
        let mut guard = state.lock().expect("lock relay state");
        guard.register_client(outbound_tx.clone())
    };

    while let Some(frame) = reader.next().await {
        let Ok(frame) = frame else {
            break;
        };
        let Message::Text(text) = frame else {
            continue;
        };
        let value: Value = serde_json::from_str(text.as_ref()).expect("parse relay frame");
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
                let subscription_id = frame[1].as_str().expect("subscription id");
                let filters = parse_filters(&frame[2..]);
                let matching = {
                    let mut guard = state.lock().expect("lock relay state");
                    guard.set_subscription(client_id, subscription_id.to_string(), filters)
                };
                for event in matching {
                    let payload = json!(["EVENT", subscription_id, event]);
                    let _ = outbound_tx.send(Message::Text(payload.to_string().into()));
                }
                let eose = json!(["EOSE", subscription_id]);
                let _ = outbound_tx.send(Message::Text(eose.to_string().into()));
            }
            "EVENT" => {
                if frame.len() < 2 {
                    continue;
                }
                let event = serde_json::from_value::<Event>(frame[1].clone()).expect("relay event");
                {
                    let mut guard = state.lock().expect("lock relay state");
                    guard.store_and_fanout(event.clone());
                }
                let ok = json!(["OK", event.id, true, "accepted"]);
                let _ = outbound_tx.send(Message::Text(ok.to_string().into()));
            }
            "CLOSE" => {
                if let Some(subscription_id) = frame.get(1).and_then(Value::as_str) {
                    let mut guard = state.lock().expect("lock relay state");
                    guard.close_subscription(client_id, subscription_id);
                }
            }
            _ => {}
        }
    }

    {
        let mut guard = state.lock().expect("lock relay state");
        guard.remove_client(client_id);
    }
    writer_task.abort();
}

struct TestNip28Relay {
    url: String,
    state: Arc<Mutex<TestRelayState>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl TestNip28Relay {
    fn spawn() -> Self {
        let state = Arc::new(Mutex::new(TestRelayState::new()));
        let (ready_tx, ready_rx) = mpsc::channel::<String>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let relay_state = Arc::clone(&state);
        let join_handle = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("build relay runtime");
            runtime.block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("bind relay listener");
                let local_addr = listener.local_addr().expect("relay local addr");
                ready_tx
                    .send(format!("ws://{local_addr}"))
                    .expect("send relay addr");
                let mut shutdown_rx = shutdown_rx;
                loop {
                    tokio::select! {
                        _ = &mut shutdown_rx => break,
                        accept = listener.accept() => {
                            let Ok((stream, _)) = accept else {
                                break;
                            };
                            let relay_state = Arc::clone(&relay_state);
                            tokio::spawn(async move {
                                handle_relay_connection(relay_state, stream).await;
                            });
                        }
                    }
                }
            });
        });
        let url = ready_rx.recv().expect("receive relay addr");
        Self {
            url,
            state,
            shutdown_tx: Some(shutdown_tx),
            join_handle: Some(join_handle),
        }
    }

    fn store_events<I>(&self, events: I)
    where
        I: IntoIterator<Item = Event>,
    {
        let mut guard = self.state.lock().expect("lock relay state");
        for event in events {
            guard.store_and_fanout(event);
        }
    }
}

impl Drop for TestNip28Relay {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}
