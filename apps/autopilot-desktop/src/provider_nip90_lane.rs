use crate::state::job_inbox::{JobExecutionParam, JobInboxNetworkRequest, JobInboxValidation};
use nostr::nip90::{
    DataVendingRequest, InputType, JOB_REQUEST_KIND_MAX, JOB_REQUEST_KIND_MIN, JOB_RESULT_KIND_MAX,
    JOB_RESULT_KIND_MIN, JobRequest, JobResult, KIND_JOB_CODE_REVIEW, KIND_JOB_FEEDBACK,
    KIND_JOB_IMAGE_GENERATION, KIND_JOB_PATCH_GEN, KIND_JOB_REPO_INDEX, KIND_JOB_RLM_SUBQUERY,
    KIND_JOB_SANDBOX_RUN, KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION,
    KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION,
    OPENAGENTS_DATA_VENDING_PROFILE, is_job_feedback_kind, is_job_request_kind, is_job_result_kind,
};
use nostr::{Event, EventTemplate};
use nostr_client::{
    ConnectionState, PoolConfig, RelayAuthIdentity, RelayConfig, RelayConnection, RelayMessage,
    RelayPool,
};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

const LANE_POLL: Duration = Duration::from_millis(15);
const RELAY_RECV_TIMEOUT: Duration = Duration::from_millis(1);
const MAX_MESSAGES_PER_RELAY_POLL: usize = 48;
const MAX_UPDATES_PER_DRAIN: usize = 32;
const MAX_PREVIEW_SEEN_REQUEST_IDS: usize = 4096;
const SUBSCRIPTION_ID_PREFIX: &str = "autopilot-provider-nip90-ingress";
const DEFAULT_TTL_SECONDS: u64 = 60;
const LIVE_REQUEST_FILTER_LOOKBACK_SECONDS: u64 = 30;
const LIVE_FILTER_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const CATCHUP_RECV_TIMEOUT: Duration = Duration::from_millis(150);
const CATCHUP_QUERY_TIMEOUT: Duration = Duration::from_millis(1500);
const NIP89_HANDLER_KIND: u16 = 31_990;
const HANDLER_PUBLISH_RETRY: Duration = Duration::from_secs(10);
const HANDLER_METADATA_NAME: &str = "Autopilot";
const HANDLER_METADATA_ABOUT: &str = "OpenAgents Autopilot compute provider for open NIP-90 jobs.";
const HANDLER_STATUS_HEALTHY: &str = "healthy";
const HANDLER_STATUS_DEGRADED: &str = "degraded";
const SUPPORTED_TEXT_OUTPUT_MIME: [&str; 2] = ["text/plain", "text/markdown"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNip90LaneMode {
    Offline,
    Connecting,
    Preview,
    Online,
    Degraded,
}

impl ProviderNip90LaneMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Preview => "preview",
            Self::Online => "online",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNip90RelayStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

impl ProviderNip90RelayStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderNip90RelayHealthRow {
    pub relay_url: String,
    pub status: ProviderNip90RelayStatus,
    pub latency_ms: Option<u32>,
    pub last_seen_seconds_ago: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProviderNip90LaneSnapshot {
    pub mode: ProviderNip90LaneMode,
    pub configured_relays: Vec<String>,
    pub relay_health: Vec<ProviderNip90RelayHealthRow>,
    pub connected_relays: usize,
    pub last_request_event_id: Option<String>,
    pub last_request_at: Option<Instant>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderNip90ComputeCapability {
    pub backend: String,
    pub reachable: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderNip90DataVendingProfile {
    pub profile_id: String,
    pub request_kind: u16,
    pub result_kind: u16,
    pub kind_posture: String,
    pub targeting_posture: String,
    pub asset_families: Vec<String>,
    pub delivery_modes: Vec<String>,
    pub preview_postures: Vec<String>,
}

impl ProviderNip90ComputeCapability {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.ready_model.is_some()
    }

    pub fn backend_or_default(&self) -> &str {
        let backend = self.backend.trim();
        if backend.is_empty() {
            "unknown"
        } else {
            backend
        }
    }
}

impl ProviderNip90LaneSnapshot {
    pub fn with_relays(relays: Vec<String>) -> Self {
        let configured_relays = normalize_relays(relays);
        Self {
            relay_health: relay_health_rows_for(
                &configured_relays,
                ProviderNip90RelayStatus::Disconnected,
            ),
            configured_relays,
            ..Self::default()
        }
    }
}

impl Default for ProviderNip90LaneSnapshot {
    fn default() -> Self {
        Self {
            mode: ProviderNip90LaneMode::Offline,
            configured_relays: Vec::new(),
            relay_health: Vec::new(),
            connected_relays: 0,
            last_request_event_id: None,
            last_request_at: None,
            last_error: None,
            last_action: Some("NIP-90 ingress lane idle".to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ProviderNip90LaneCommand {
    ConfigureIdentity {
        identity: Option<ProviderNip90AuthIdentity>,
    },
    ConfigureComputeCapability {
        capability: ProviderNip90ComputeCapability,
    },
    ConfigureDataVendingProfile {
        profile: Option<ProviderNip90DataVendingProfile>,
    },
    ConfigureRelays {
        relays: Vec<String>,
    },
    SetOnline {
        online: bool,
    },
    PublishEvent {
        request_id: String,
        role: ProviderNip90PublishRole,
        event: Box<Event>,
    },
    TrackProviderPublishRequestIds {
        request_ids: Vec<String>,
    },
    TrackBuyerRequestIds {
        request_ids: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNip90PublishRole {
    Capability,
    Request,
    Feedback,
    Result,
}

impl ProviderNip90PublishRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Capability => "capability",
            Self::Request => "request",
            Self::Feedback => "feedback",
            Self::Result => "result",
        }
    }

    pub const fn protocol_label(self) -> &'static str {
        match self {
            Self::Capability => "NIP-89 handler",
            Self::Request => "NIP-90 request",
            Self::Feedback => "NIP-90 feedback",
            Self::Result => "NIP-90 result",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderNip90AuthIdentity {
    pub npub: String,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

#[derive(Clone, Debug)]
pub struct ProviderNip90PublishOutcome {
    pub request_id: String,
    pub role: ProviderNip90PublishRole,
    pub event_id: String,
    pub selected_relays: Vec<String>,
    pub accepted_relay_urls: Vec<String>,
    pub rejected_relay_urls: Vec<String>,
    pub accepted_relays: usize,
    pub rejected_relays: usize,
    pub first_error: Option<String>,
    pub parsed_event_shape: Option<String>,
    pub raw_event_json: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNip90BuyerResponseKind {
    Feedback,
    Result,
}

impl ProviderNip90BuyerResponseKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Feedback => "feedback",
            Self::Result => "result",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProviderNip90BuyerResponseEvent {
    pub request_id: String,
    pub provider_pubkey: String,
    pub event_id: String,
    pub relay_url: Option<String>,
    pub kind: ProviderNip90BuyerResponseKind,
    pub status: Option<String>,
    pub status_extra: Option<String>,
    pub amount_msats: Option<u64>,
    pub bolt11: Option<String>,
    pub parsed_event_shape: Option<String>,
    pub raw_event_json: Option<String>,
}

#[derive(Clone, Debug)]
pub enum ProviderNip90LaneUpdate {
    Snapshot(Box<ProviderNip90LaneSnapshot>),
    IngressedRequest(JobInboxNetworkRequest),
    BuyerResponseEvent(ProviderNip90BuyerResponseEvent),
    PublishOutcome(ProviderNip90PublishOutcome),
}

pub struct ProviderNip90LaneWorker {
    command_tx: Sender<ProviderNip90LaneCommand>,
    update_rx: Receiver<ProviderNip90LaneUpdate>,
}

impl ProviderNip90LaneWorker {
    pub fn spawn(initial_relays: Vec<String>) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<ProviderNip90LaneCommand>();
        let (update_tx, update_rx) = mpsc::channel::<ProviderNip90LaneUpdate>();

        std::thread::spawn(move || run_lane_loop(command_rx, update_tx, initial_relays));

        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue(&self, command: ProviderNip90LaneCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|error| format!("NIP-90 provider lane offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<ProviderNip90LaneUpdate> {
        let mut updates = Vec::new();
        while updates.len() < MAX_UPDATES_PER_DRAIN {
            let Ok(update) = self.update_rx.try_recv() else {
                break;
            };
            updates.push(update);
        }
        updates
    }
}

struct ProviderNip90LaneState {
    snapshot: ProviderNip90LaneSnapshot,
    wants_online: bool,
    pool: Option<Arc<RelayPool>>,
    auth_identity: Option<ProviderNip90AuthIdentity>,
    compute_capability: ProviderNip90ComputeCapability,
    data_vending_profile: Option<ProviderNip90DataVendingProfile>,
    handler_publication_state: HandlerPublicationState,
    next_handler_publish_retry_at: Option<Instant>,
    tracked_provider_publish_request_ids: Vec<String>,
    tracked_buyer_request_ids: Vec<String>,
    preview_seen_request_ids: HashSet<String>,
    relay_last_seen: HashMap<String, Instant>,
    relay_latency_ms: HashMap<String, u32>,
    relay_last_error: HashMap<String, String>,
    next_live_filter_refresh_at: Option<Instant>,
    ingress_subscription_generation: u64,
    ingress_subscription_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandlerPublicationState {
    None,
    Healthy,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DesiredLaneState {
    Offline,
    Preview,
    Online,
}

impl ProviderNip90LaneState {
    fn next_ingress_subscription_id(&mut self) -> String {
        self.ingress_subscription_generation =
            self.ingress_subscription_generation.saturating_add(1);
        let next = format!(
            "{SUBSCRIPTION_ID_PREFIX}-{}",
            self.ingress_subscription_generation
        );
        self.ingress_subscription_id = next.clone();
        next
    }

    fn desired_state(&self) -> DesiredLaneState {
        if self.snapshot.configured_relays.is_empty() {
            DesiredLaneState::Offline
        } else if self.wants_online {
            DesiredLaneState::Online
        } else {
            DesiredLaneState::Preview
        }
    }

    fn connection_expected(&self) -> bool {
        !matches!(self.desired_state(), DesiredLaneState::Offline)
    }

    fn provider_request_ingress_enabled(&self) -> bool {
        self.compute_capability.is_ready() || self.data_vending_profile.is_some()
    }

    fn ingress_filters(&self) -> Vec<serde_json::Value> {
        build_ingress_filters(
            self.provider_request_ingress_enabled(),
            self.tracked_buyer_request_ids.as_slice(),
            self.auth_identity.as_ref(),
            self.compute_capability.is_ready(),
            self.data_vending_profile.as_ref(),
        )
    }

    fn preview_last_action(&self, connected_relays: usize) -> String {
        if self.provider_request_ingress_enabled() {
            format!(
                "Relay preview active ({}/{})",
                connected_relays,
                self.snapshot.configured_relays.len()
            )
        } else if self.tracked_buyer_request_ids.is_empty() {
            format!(
                "Buyer relay transport active ({}/{})",
                connected_relays,
                self.snapshot.configured_relays.len()
            )
        } else {
            format!(
                "Buyer response relay tracking active ({}/{})",
                connected_relays,
                self.snapshot.configured_relays.len()
            )
        }
    }

    fn preview_connecting_action(&self) -> String {
        if self.provider_request_ingress_enabled() {
            "Connecting relay preview".to_string()
        } else if self.tracked_buyer_request_ids.is_empty() {
            "Connecting buyer relay transport".to_string()
        } else {
            "Connecting buyer response relay tracking".to_string()
        }
    }

    fn preview_degraded_action(&self) -> String {
        if self.provider_request_ingress_enabled() {
            "Relay preview degraded".to_string()
        } else if self.tracked_buyer_request_ids.is_empty() {
            "Buyer relay transport degraded".to_string()
        } else {
            "Buyer response relay tracking degraded".to_string()
        }
    }

    fn preview_zero_connected_error(&self) -> String {
        if self.provider_request_ingress_enabled() {
            "Relay preview has zero connected relays while provider is offline".to_string()
        } else if self.tracked_buyer_request_ids.is_empty() {
            "Buyer relay transport has zero connected relays".to_string()
        } else {
            "Buyer response relay tracking has zero connected relays".to_string()
        }
    }

    fn preview_failed_to_connect_action(&self) -> String {
        if self.provider_request_ingress_enabled() {
            "Relay preview failed to connect".to_string()
        } else if self.tracked_buyer_request_ids.is_empty() {
            "Buyer relay transport failed to connect".to_string()
        } else {
            "Buyer response relay tracking failed to connect".to_string()
        }
    }

    fn clear_preview_request_cache(&mut self) {
        self.preview_seen_request_ids.clear();
    }

    fn preview_request_should_reach_ui(&mut self, request: &JobInboxNetworkRequest) -> bool {
        if self.preview_seen_request_ids.len() >= MAX_PREVIEW_SEEN_REQUEST_IDS {
            self.preview_seen_request_ids.clear();
        }

        if !request.target_provider_pubkeys.is_empty() {
            let Some(identity) = self.auth_identity.as_ref() else {
                return false;
            };
            let local_pubkey = identity.public_key_hex.trim().to_ascii_lowercase();
            let local_npub = identity.npub.trim().to_ascii_lowercase();
            let targeted_here = request.target_provider_pubkeys.iter().any(|target| {
                let normalized = target.trim().to_ascii_lowercase();
                normalized == local_pubkey || normalized == local_npub
            });
            if !targeted_here {
                return false;
            }
        }

        self.preview_seen_request_ids
            .insert(request.request_id.clone())
    }
}

fn run_lane_loop(
    command_rx: Receiver<ProviderNip90LaneCommand>,
    update_tx: Sender<ProviderNip90LaneUpdate>,
    initial_relays: Vec<String>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let configured_relays = normalize_relays(initial_relays);
            let snapshot = ProviderNip90LaneSnapshot {
                mode: ProviderNip90LaneMode::Degraded,
                relay_health: relay_health_rows_for(
                    &configured_relays,
                    ProviderNip90RelayStatus::Error,
                ),
                configured_relays,
                connected_relays: 0,
                last_request_event_id: None,
                last_request_at: None,
                last_error: Some(format!(
                    "Failed to initialize NIP-90 ingress runtime: {error}"
                )),
                last_action: Some("NIP-90 ingress lane failed to start".to_string()),
            };
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(snapshot)));
            return;
        }
    };

    let mut state = ProviderNip90LaneState {
        snapshot: ProviderNip90LaneSnapshot::with_relays(initial_relays),
        wants_online: false,
        pool: None,
        auth_identity: None,
        compute_capability: ProviderNip90ComputeCapability::default(),
        data_vending_profile: None,
        handler_publication_state: HandlerPublicationState::None,
        next_handler_publish_retry_at: None,
        tracked_provider_publish_request_ids: Vec::new(),
        tracked_buyer_request_ids: Vec::new(),
        preview_seen_request_ids: HashSet::new(),
        relay_last_seen: HashMap::new(),
        relay_latency_ms: HashMap::new(),
        relay_last_error: HashMap::new(),
        next_live_filter_refresh_at: None,
        ingress_subscription_generation: 1,
        ingress_subscription_id: format!("{SUBSCRIPTION_ID_PREFIX}-1"),
    };
    refresh_relay_health_snapshot(&runtime, &mut state);

    let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
        state.snapshot.clone(),
    )));

    loop {
        match command_rx.recv_timeout(LANE_POLL) {
            Ok(command) => {
                handle_command_or_publish(&runtime, &mut state, &update_tx, command);
                while let Ok(pending) = command_rx.try_recv() {
                    handle_command_or_publish(&runtime, &mut state, &update_tx, pending);
                }
                let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                    state.snapshot.clone(),
                )));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let mode_before = state.snapshot.mode;
        let connected_before = state.snapshot.connected_relays;
        let relay_health_before = state.snapshot.relay_health.clone();
        if ensure_connected_pool(&runtime, &mut state).is_err() {
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
            continue;
        }
        if state.snapshot.mode != mode_before
            || state.snapshot.connected_relays != connected_before
            || state.snapshot.relay_health != relay_health_before
        {
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }

        if !state.connection_expected() {
            continue;
        }

        let Some(pool) = state.pool.as_ref().cloned() else {
            continue;
        };

        let mut catchup_outcome = CatchupOutcome::default();
        if should_refresh_live_filters(&state) {
            resubscribe_ingress_filters(&runtime, &mut state, pool.clone());
            catchup_outcome = runtime.block_on(catchup_ingress_with_fresh_queries(&state));
            state.next_live_filter_refresh_at = Some(Instant::now() + LIVE_FILTER_REFRESH_INTERVAL);
        }

        let relay_health_before_poll = state.snapshot.relay_health.clone();
        let mut outcome = runtime.block_on(poll_ingress(
            pool,
            state.tracked_buyer_request_ids.as_slice(),
            state
                .auth_identity
                .as_ref()
                .map(|identity| identity.public_key_hex.as_str()),
        ));
        outcome.requests.extend(catchup_outcome.requests);
        outcome.buyer_events.extend(catchup_outcome.buyer_events);
        apply_poll_outcome_telemetry(&mut state, &outcome);
        refresh_relay_health_snapshot(&runtime, &mut state);

        if state.snapshot.connected_relays != outcome.connected_relays {
            state.snapshot.connected_relays = outcome.connected_relays;
        }
        let desired_state = state.desired_state();
        if outcome.connected_relays == 0 {
            state.snapshot.mode = ProviderNip90LaneMode::Degraded;
            state.snapshot.last_error = Some(match desired_state {
                DesiredLaneState::Offline => "Provider relay lane offline".to_string(),
                DesiredLaneState::Preview => {
                    "Relay preview has zero connected relays while provider is offline".to_string()
                }
                DesiredLaneState::Online => {
                    "Provider ingress has zero connected relays while online".to_string()
                }
            });
        } else {
            state.snapshot.mode = match desired_state {
                DesiredLaneState::Offline => ProviderNip90LaneMode::Offline,
                DesiredLaneState::Preview => ProviderNip90LaneMode::Preview,
                DesiredLaneState::Online => ProviderNip90LaneMode::Online,
            };
            state.snapshot.last_error = None;
        }

        if outcome.connected_relays == 0 {
            if let Some(error) = outcome.last_error {
                state.snapshot.mode = ProviderNip90LaneMode::Degraded;
                state.snapshot.last_error = Some(error);
            }
        }

        if state.snapshot.relay_health != relay_health_before_poll
            || state.snapshot.connected_relays != connected_before
            || state.snapshot.mode != mode_before
        {
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }

        maybe_publish_handler_info(&runtime, &mut state, &update_tx);

        for request in outcome.requests {
            if matches!(desired_state, DesiredLaneState::Preview)
                && !state.preview_request_should_reach_ui(&request)
            {
                continue;
            }
            state.snapshot.last_request_event_id = Some(request.request_id.clone());
            state.snapshot.last_request_at = Some(Instant::now());
            state.snapshot.last_error = None;
            state.snapshot.last_action =
                Some(if matches!(desired_state, DesiredLaneState::Preview) {
                    "Relay preview observing market activity".to_string()
                } else {
                    format!(
                        "ingressed live NIP-90 request {} from relays",
                        request.request_id
                    )
                });
            let _ = update_tx.send(ProviderNip90LaneUpdate::IngressedRequest(request));
            if !matches!(desired_state, DesiredLaneState::Preview) {
                let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                    state.snapshot.clone(),
                )));
            }
        }

        for buyer_event in outcome.buyer_events {
            state.snapshot.last_error = None;
            state.snapshot.last_action = Some(format!(
                "ingressed buyer {} event {} for request {}",
                buyer_event.kind.label(),
                buyer_event.event_id,
                buyer_event.request_id
            ));
            let _ = update_tx.send(ProviderNip90LaneUpdate::BuyerResponseEvent(buyer_event));
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }
    }
}

fn handle_command_or_publish(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    update_tx: &Sender<ProviderNip90LaneUpdate>,
    command: ProviderNip90LaneCommand,
) {
    match command {
        ProviderNip90LaneCommand::ConfigureIdentity { .. }
        | ProviderNip90LaneCommand::ConfigureComputeCapability { .. }
        | ProviderNip90LaneCommand::ConfigureDataVendingProfile { .. }
        | ProviderNip90LaneCommand::ConfigureRelays { .. }
        | ProviderNip90LaneCommand::SetOnline { .. }
        | ProviderNip90LaneCommand::TrackProviderPublishRequestIds { .. }
        | ProviderNip90LaneCommand::TrackBuyerRequestIds { .. } => {
            handle_command(runtime, state, command);
        }
        ProviderNip90LaneCommand::PublishEvent {
            request_id,
            role,
            event,
        } => {
            handle_publish_event(runtime, state, update_tx, request_id, role, *event);
        }
    }
}

fn handle_command(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    command: ProviderNip90LaneCommand,
) {
    match command {
        ProviderNip90LaneCommand::ConfigureIdentity { identity } => {
            if identity == state.auth_identity {
                return;
            }
            state.auth_identity = identity;
            state.clear_preview_request_cache();
            state.handler_publication_state = HandlerPublicationState::None;
            state.next_handler_publish_retry_at = None;
            state.snapshot.last_action = Some("Updated provider relay identity".to_string());
            if state.pool.is_some() {
                disconnect_pool(runtime, state);
                if state.connection_expected() {
                    state.snapshot.mode = ProviderNip90LaneMode::Connecting;
                    state.snapshot.last_action =
                        Some("Rebinding provider relay identity".to_string());
                }
            }
            state.next_live_filter_refresh_at = Some(Instant::now());
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::ConfigureComputeCapability { capability } => {
            if capability == state.compute_capability {
                return;
            }
            let was_ready = state.compute_capability.is_ready();
            let is_ready = capability.is_ready();
            let backend = capability.backend_or_default().replace('_', " ");
            let status = if is_ready {
                format!(
                    "{} capability ready for model '{}'",
                    backend,
                    capability.ready_model.as_deref().unwrap_or("unknown")
                )
            } else {
                capability
                    .last_error
                    .clone()
                    .unwrap_or_else(|| format!("{backend} capability unavailable"))
            };
            state.compute_capability = capability;
            state.clear_preview_request_cache();
            state.handler_publication_state = HandlerPublicationState::None;
            state.next_handler_publish_retry_at = None;
            state.snapshot.last_action = Some(if is_ready {
                status
            } else if was_ready {
                format!("{backend} capability degraded: {status}")
            } else {
                format!("{backend} capability pending: {status}")
            });
            if let Some(pool) = state.pool.as_ref().cloned() {
                resubscribe_ingress_filters(runtime, state, pool);
            }
            state.next_live_filter_refresh_at = Some(Instant::now());
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::ConfigureDataVendingProfile { profile } => {
            if profile == state.data_vending_profile {
                return;
            }
            let summary = profile
                .as_ref()
                .map(|profile| {
                    format!(
                        "Configured data-vending profile {} kind {} ({})",
                        profile.profile_id, profile.request_kind, profile.kind_posture
                    )
                })
                .unwrap_or_else(|| "Cleared data-vending profile".to_string());
            state.data_vending_profile = profile;
            state.clear_preview_request_cache();
            state.handler_publication_state = HandlerPublicationState::None;
            state.next_handler_publish_retry_at = None;
            state.snapshot.last_action = Some(summary);
            if let Some(pool) = state.pool.as_ref().cloned() {
                resubscribe_ingress_filters(runtime, state, pool);
            }
            state.next_live_filter_refresh_at = Some(Instant::now());
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::ConfigureRelays { relays } => {
            let normalized = normalize_relays(relays);
            if normalized == state.snapshot.configured_relays {
                return;
            }

            state.snapshot.configured_relays = normalized;
            state.clear_preview_request_cache();
            state.handler_publication_state = HandlerPublicationState::None;
            state.next_handler_publish_retry_at = None;
            state.snapshot.connected_relays = 0;
            prune_relay_observation_maps(state);
            state.snapshot.relay_health = relay_health_rows_for(
                &state.snapshot.configured_relays,
                if state.connection_expected() {
                    ProviderNip90RelayStatus::Connecting
                } else {
                    ProviderNip90RelayStatus::Disconnected
                },
            );
            state.snapshot.last_action =
                Some("Updated relay observation configuration".to_string());

            if state.pool.is_some() {
                disconnect_pool(runtime, state);
            }
            state.next_live_filter_refresh_at = Some(Instant::now());
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::SetOnline { online } => {
            state.wants_online = online;
            state.clear_preview_request_cache();
            state.handler_publication_state = HandlerPublicationState::None;
            state.next_handler_publish_retry_at = None;
            state.next_live_filter_refresh_at = if online { Some(Instant::now()) } else { None };
            if online {
                state.snapshot.mode = ProviderNip90LaneMode::Connecting;
                state.snapshot.last_action = Some("Connecting provider relay ingress".to_string());
                state.snapshot.relay_health = relay_health_rows_for(
                    &state.snapshot.configured_relays,
                    ProviderNip90RelayStatus::Connecting,
                );
            } else {
                state.snapshot.last_error = None;
                if state.snapshot.configured_relays.is_empty() {
                    disconnect_pool(runtime, state);
                    state.snapshot.mode = ProviderNip90LaneMode::Offline;
                    state.snapshot.last_action = Some("Provider relay ingress offline".to_string());
                } else if state.pool.is_some() {
                    state.snapshot.mode = ProviderNip90LaneMode::Preview;
                    state.snapshot.last_action =
                        Some(state.preview_last_action(state.snapshot.connected_relays));
                } else {
                    state.snapshot.mode = ProviderNip90LaneMode::Connecting;
                    state.snapshot.last_action = Some(state.preview_connecting_action());
                    state.snapshot.relay_health = relay_health_rows_for(
                        &state.snapshot.configured_relays,
                        ProviderNip90RelayStatus::Connecting,
                    );
                }
            }
            if let Some(pool) = state.pool.as_ref().cloned() {
                resubscribe_ingress_filters(runtime, state, pool);
            }
            state.next_live_filter_refresh_at = Some(Instant::now());
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::TrackProviderPublishRequestIds { request_ids } => {
            let normalized = normalize_request_ids(request_ids);
            if normalized == state.tracked_provider_publish_request_ids {
                return;
            }
            state.tracked_provider_publish_request_ids = normalized;
            state.snapshot.last_action = Some(format!(
                "Tracking provider publish continuity for {} request id(s)",
                state.tracked_provider_publish_request_ids.len()
            ));
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::TrackBuyerRequestIds { request_ids } => {
            let normalized = normalize_request_ids(request_ids);
            if normalized == state.tracked_buyer_request_ids {
                return;
            }
            state.tracked_buyer_request_ids = normalized;
            state.snapshot.last_action = Some(format!(
                "Tracking buyer response events for {} request id(s)",
                state.tracked_buyer_request_ids.len()
            ));
            if let Some(pool) = state.pool.as_ref().cloned() {
                resubscribe_ingress_filters(runtime, state, pool);
            }
            refresh_relay_health_snapshot(runtime, state);
        }
        ProviderNip90LaneCommand::PublishEvent { .. } => {}
    }
}

fn handle_publish_event(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    update_tx: &Sender<ProviderNip90LaneUpdate>,
    request_id: String,
    role: ProviderNip90PublishRole,
    event: Event,
) -> bool {
    let event_id = event.id.clone();
    let parsed_event_shape = Some(format_generic_event_shape(&event));
    let raw_event_json = serde_json::to_string_pretty(&event).ok();

    let buyer_feedback_while_offline = !state.wants_online
        && role == ProviderNip90PublishRole::Feedback
        && state
            .tracked_buyer_request_ids
            .iter()
            .any(|tracked_request_id| tracked_request_id == request_id.as_str());
    let provider_publish_while_offline = !state.wants_online
        && matches!(
            role,
            ProviderNip90PublishRole::Feedback | ProviderNip90PublishRole::Result
        )
        && state
            .tracked_provider_publish_request_ids
            .iter()
            .any(|tracked_request_id| tracked_request_id == request_id.as_str());

    if !state.wants_online
        && role != ProviderNip90PublishRole::Request
        && !buyer_feedback_while_offline
        && !provider_publish_while_offline
    {
        let message = format!(
            "Cannot publish {} while provider lane is offline",
            role.protocol_label()
        );
        state.snapshot.mode = ProviderNip90LaneMode::Degraded;
        state.snapshot.last_error = Some(message.clone());
        state.snapshot.last_action = Some(format!("publish {} failed: offline", role.label()));
        let _ = update_tx.send(ProviderNip90LaneUpdate::PublishOutcome(
            ProviderNip90PublishOutcome {
                request_id,
                role,
                event_id,
                selected_relays: Vec::new(),
                accepted_relay_urls: Vec::new(),
                rejected_relay_urls: Vec::new(),
                accepted_relays: 0,
                rejected_relays: 0,
                first_error: Some(message),
                parsed_event_shape,
                raw_event_json,
            },
        ));
        return false;
    }

    if ensure_connected_pool(runtime, state).is_err() {
        let message = state
            .snapshot
            .last_error
            .clone()
            .unwrap_or_else(|| "Unable to connect relays for publish".to_string());
        let _ = update_tx.send(ProviderNip90LaneUpdate::PublishOutcome(
            ProviderNip90PublishOutcome {
                request_id,
                role,
                event_id,
                selected_relays: Vec::new(),
                accepted_relay_urls: Vec::new(),
                rejected_relay_urls: Vec::new(),
                accepted_relays: 0,
                rejected_relays: 0,
                first_error: Some(message),
                parsed_event_shape,
                raw_event_json,
            },
        ));
        return false;
    }

    let Some(pool) = state.pool.as_ref().cloned() else {
        let message = format!(
            "Cannot publish {}: relay pool unavailable",
            role.protocol_label()
        );
        state.snapshot.mode = ProviderNip90LaneMode::Degraded;
        state.snapshot.last_error = Some(message.clone());
        state.snapshot.last_action =
            Some(format!("publish {} failed: no relay pool", role.label()));
        let _ = update_tx.send(ProviderNip90LaneUpdate::PublishOutcome(
            ProviderNip90PublishOutcome {
                request_id,
                role,
                event_id,
                selected_relays: Vec::new(),
                accepted_relay_urls: Vec::new(),
                rejected_relay_urls: Vec::new(),
                accepted_relays: 0,
                rejected_relays: 0,
                first_error: Some(message),
                parsed_event_shape,
                raw_event_json,
            },
        ));
        return false;
    };

    state.snapshot.last_error = None;
    state.snapshot.last_action = Some(format!(
        "publishing {} event {} to {} relay(s)",
        role.label(),
        event_id,
        state.snapshot.connected_relays
    ));
    let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
        state.snapshot.clone(),
    )));
    tracing::info!(
        target: "autopilot_desktop::provider",
        "Provider lane publishing request_id={} role={} event_id={} connected_relays={} wants_online={}",
        request_id,
        role.label(),
        event_id,
        state.snapshot.connected_relays,
        state.wants_online
    );

    match runtime.block_on(pool.publish(&event)) {
        Ok(confirmations) => {
            let selected_relays = normalize_relay_urls(state.snapshot.configured_relays.clone());
            let accepted_relay_urls = normalize_relay_urls(
                confirmations
                    .iter()
                    .filter(|entry| entry.accepted)
                    .map(|entry| entry.relay_url.clone())
                    .collect(),
            );
            let rejected_relay_urls = normalize_relay_urls(
                confirmations
                    .iter()
                    .filter(|entry| !entry.accepted)
                    .map(|entry| entry.relay_url.clone())
                    .collect(),
            );
            let accepted_relays = confirmations.iter().filter(|entry| entry.accepted).count();
            let rejected_relays = confirmations.len().saturating_sub(accepted_relays);
            let first_error = confirmations
                .iter()
                .find(|entry| !entry.accepted)
                .map(|entry| entry.message.clone());

            if accepted_relays == 0 {
                state.snapshot.mode = ProviderNip90LaneMode::Degraded;
                state.snapshot.last_error = first_error
                    .clone()
                    .or_else(|| Some(format!("All relays rejected {} publish", role.label())));
            } else {
                state.snapshot.mode = ProviderNip90LaneMode::Online;
                state.snapshot.last_error = None;
            }
            state.snapshot.last_action = Some(format!(
                "published {} event {} (accepted={}, rejected={})",
                role.label(),
                event_id,
                accepted_relays,
                rejected_relays
            ));
            tracing::info!(
                target: "autopilot_desktop::provider",
                "Provider lane publish outcome request_id={} role={} event_id={} accepted_relays={} rejected_relays={} first_error={}",
                request_id,
                role.label(),
                event_id,
                accepted_relays,
                rejected_relays,
                first_error.as_deref().unwrap_or("none")
            );
            let _ = update_tx.send(ProviderNip90LaneUpdate::PublishOutcome(
                ProviderNip90PublishOutcome {
                    request_id,
                    role,
                    event_id,
                    selected_relays,
                    accepted_relay_urls,
                    rejected_relay_urls,
                    accepted_relays,
                    rejected_relays,
                    first_error,
                    parsed_event_shape,
                    raw_event_json,
                },
            ));
            accepted_relays > 0
        }
        Err(error) => {
            let message = format!("Failed publishing {}: {error}", role.protocol_label());
            state.snapshot.mode = ProviderNip90LaneMode::Degraded;
            state.snapshot.last_error = Some(message.clone());
            state.snapshot.last_action = Some(format!("publish {} failed", role.label()));
            tracing::error!(
                target: "autopilot_desktop::provider",
                "Provider lane publish errored request_id={} role={} event_id={} error={}",
                request_id,
                role.label(),
                event_id,
                message
            );
            let _ = update_tx.send(ProviderNip90LaneUpdate::PublishOutcome(
                ProviderNip90PublishOutcome {
                    request_id,
                    role,
                    event_id,
                    selected_relays: Vec::new(),
                    accepted_relay_urls: Vec::new(),
                    rejected_relay_urls: Vec::new(),
                    accepted_relays: 0,
                    rejected_relays: 0,
                    first_error: Some(message),
                    parsed_event_shape,
                    raw_event_json,
                },
            ));
            false
        }
    }
}

fn disconnect_pool(runtime: &tokio::runtime::Runtime, state: &mut ProviderNip90LaneState) {
    if let Some(pool) = state.pool.take() {
        let _ = runtime.block_on(pool.disconnect_all());
    }
    state.snapshot.connected_relays = 0;
}

fn ensure_connected_pool(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
) -> Result<(), ()> {
    let desired_state = state.desired_state();
    if matches!(desired_state, DesiredLaneState::Offline) {
        if state.pool.is_some() {
            disconnect_pool(runtime, state);
        }
        state.snapshot.mode = ProviderNip90LaneMode::Offline;
        state.snapshot.last_error = None;
        state.snapshot.last_action = Some("Provider relay ingress offline".to_string());
        refresh_relay_health_snapshot(runtime, state);
        return Ok(());
    }

    if state.snapshot.configured_relays.is_empty() {
        state.snapshot.mode = ProviderNip90LaneMode::Degraded;
        state.snapshot.last_error =
            Some("No relay URLs configured for provider ingress".to_string());
        state.snapshot.last_action = Some("Provider ingress failed: missing relays".to_string());
        state.snapshot.relay_health = relay_health_rows_for(
            &state.snapshot.configured_relays,
            ProviderNip90RelayStatus::Error,
        );
        return Err(());
    }

    if let Some(pool) = state.pool.as_ref().cloned() {
        reconnect_disconnected_relays(runtime, state, pool);
        refresh_relay_health_snapshot(runtime, state);
        if state.snapshot.connected_relays > 0 {
            state.snapshot.last_error = None;
            match desired_state {
                DesiredLaneState::Preview => {
                    state.snapshot.mode = ProviderNip90LaneMode::Preview;
                    state.snapshot.last_action =
                        Some(state.preview_last_action(state.snapshot.connected_relays));
                }
                DesiredLaneState::Online => {
                    state.snapshot.mode = ProviderNip90LaneMode::Online;
                    state.snapshot.last_action = Some(format!(
                        "Provider relay ingress online ({}/{})",
                        state.snapshot.connected_relays,
                        state.snapshot.configured_relays.len()
                    ));
                }
                DesiredLaneState::Offline => {}
            }
            return Ok(());
        }

        state.snapshot.mode = ProviderNip90LaneMode::Degraded;
        state.snapshot.last_error = Some(match desired_state {
            DesiredLaneState::Preview => state.preview_zero_connected_error(),
            DesiredLaneState::Online => {
                "Provider ingress has zero connected relays while online".to_string()
            }
            DesiredLaneState::Offline => "Provider relay ingress offline".to_string(),
        });
        state.snapshot.last_action = Some(match desired_state {
            DesiredLaneState::Preview => state.preview_degraded_action(),
            DesiredLaneState::Online => "Provider relay ingress degraded".to_string(),
            DesiredLaneState::Offline => "Provider relay ingress offline".to_string(),
        });
        return Err(());
    }

    let mut first_error: Option<String> = None;
    let mut connected_relays = 0usize;
    let pool = Arc::new(RelayPool::new(pool_config_for(state)));

    runtime.block_on(async {
        for relay in &state.snapshot.configured_relays {
            let relay_key = relay_map_key(relay);
            if let Err(error) = pool.add_relay(relay.as_str()).await {
                let detail = format!("Failed adding relay {relay}: {error}");
                state.relay_last_error.insert(relay_key, detail.clone());
                if first_error.is_none() {
                    first_error = Some(detail);
                }
            }
        }

        let filters = state.ingress_filters();
        for relay in &state.snapshot.configured_relays {
            let relay_key = relay_map_key(relay);
            let connect_started = Instant::now();
            match pool.connect_relay(relay).await {
                Ok(()) => {
                    let connect_latency = elapsed_millis_u32(connect_started.elapsed());
                    state.relay_latency_ms.insert(relay_key.clone(), connect_latency);
                    state
                        .relay_last_seen
                        .insert(relay_key.clone(), Instant::now());

                    match pool.relay(relay).await {
                        Some(connection) => {
                            let subscription_result = if filters.is_empty() {
                                connection
                                    .unsubscribe(state.ingress_subscription_id.as_str())
                                    .await
                            } else {
                                connection
                                    .subscribe_filters(
                                        state.ingress_subscription_id.as_str(),
                                        filters.clone(),
                                    )
                                    .await
                            };
                            if let Err(error) = subscription_result {
                                let detail = format!(
                                    "Failed subscribing provider ingress filters for {relay}: {error}"
                                );
                                state
                                    .relay_last_error
                                    .insert(relay_key.clone(), detail.clone());
                                if first_error.is_none() {
                                    first_error = Some(detail);
                                }
                                continue;
                            }
                        }
                        None => {
                            let detail =
                                format!("Relay {relay} missing from pool after connect");
                            state
                                .relay_last_error
                                .insert(relay_key.clone(), detail.clone());
                            if first_error.is_none() {
                                first_error = Some(detail);
                            }
                            continue;
                        }
                    }

                    state.relay_last_error.remove(&relay_key);
                    connected_relays = connected_relays.saturating_add(1);
                }
                Err(error) => {
                    let detail = format!("Failed connecting relay {relay}: {error}");
                    state
                        .relay_last_error
                        .insert(relay_key, detail.clone());
                    if first_error.is_none() {
                        first_error = Some(detail);
                    }
                }
            }
        }
    });

    if connected_relays == 0 {
        let _ = runtime.block_on(pool.disconnect_all());
        state.snapshot.mode = ProviderNip90LaneMode::Degraded;
        state.snapshot.last_error = first_error;
        state.snapshot.last_action = Some(match desired_state {
            DesiredLaneState::Preview => state.preview_failed_to_connect_action(),
            DesiredLaneState::Online => "Provider relay ingress failed to connect".to_string(),
            DesiredLaneState::Offline => "Provider relay ingress offline".to_string(),
        });
        refresh_relay_health_snapshot(runtime, state);
        return Err(());
    }

    state.pool = Some(pool);
    refresh_relay_health_snapshot(runtime, state);
    match desired_state {
        DesiredLaneState::Preview => {
            state.snapshot.mode = ProviderNip90LaneMode::Preview;
            state.snapshot.last_error = None;
            state.snapshot.last_action = Some(state.preview_last_action(connected_relays));
        }
        DesiredLaneState::Online => {
            state.snapshot.mode = ProviderNip90LaneMode::Online;
            state.snapshot.last_error = None;
            state.snapshot.last_action = Some(format!(
                "Provider relay ingress online ({}/{})",
                connected_relays,
                state.snapshot.configured_relays.len()
            ));
        }
        DesiredLaneState::Offline => {
            state.snapshot.mode = ProviderNip90LaneMode::Offline;
            state.snapshot.last_error = None;
            state.snapshot.last_action = Some("Provider relay ingress offline".to_string());
        }
    }
    Ok(())
}

fn pool_config_for(state: &ProviderNip90LaneState) -> PoolConfig {
    PoolConfig {
        relay_config: RelayConfig {
            nip42_identity: state
                .auth_identity
                .as_ref()
                .map(|identity| RelayAuthIdentity {
                    private_key_hex: identity.private_key_hex.clone(),
                }),
            ..RelayConfig::default()
        },
        ..PoolConfig::default()
    }
}

fn reconnect_disconnected_relays(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    pool: Arc<RelayPool>,
) {
    runtime.block_on(async {
        let filters = state.ingress_filters();
        for relay in &state.snapshot.configured_relays {
            let relay_key = relay_map_key(relay);
            let Some(connection) = pool.relay(relay).await else {
                continue;
            };

            if connection.state().await == ConnectionState::Connected {
                continue;
            }

            let connect_started = Instant::now();
            match connection.connect().await {
                Ok(()) => {
                    let connect_latency = elapsed_millis_u32(connect_started.elapsed());
                    state.relay_latency_ms.insert(relay_key.clone(), connect_latency);
                    state
                        .relay_last_seen
                        .insert(relay_key.clone(), Instant::now());
                    let subscription_result = if filters.is_empty() {
                        connection
                            .unsubscribe(state.ingress_subscription_id.as_str())
                            .await
                    } else {
                        connection
                            .subscribe_filters(
                                state.ingress_subscription_id.as_str(),
                                filters.clone(),
                            )
                            .await
                    };
                    if let Err(error) = subscription_result {
                        state.relay_last_error.insert(
                            relay_key.clone(),
                            format!(
                                "Failed re-subscribing provider ingress filters for {relay}: {error}"
                            ),
                        );
                    } else {
                        state.relay_last_error.remove(&relay_key);
                    }
                }
                Err(error) => {
                    state.relay_last_error.insert(
                        relay_key,
                        format!("Failed reconnecting relay {relay}: {error}"),
                    );
                }
            }
        }
    });
}

async fn replace_ingress_subscription(
    connection: &nostr_client::RelayConnection,
    previous_subscription_id: &str,
    next_subscription_id: &str,
    filters: Vec<serde_json::Value>,
) -> Result<(), nostr_client::ClientError> {
    // Refreshing a live subscription means replacing the prior relay-side view,
    // not layering another REQ with the same id on top of it.
    let _ = connection.unsubscribe(previous_subscription_id).await;
    if filters.is_empty() {
        Ok(())
    } else {
        connection
            .subscribe_filters(next_subscription_id, filters)
            .await
    }
}

fn resubscribe_ingress_filters(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    pool: Arc<RelayPool>,
) {
    runtime.block_on(async {
        let filters = state.ingress_filters();
        let previous_subscription_id = state.ingress_subscription_id.clone();
        let next_subscription_id = if filters.is_empty() {
            previous_subscription_id.clone()
        } else {
            state.next_ingress_subscription_id()
        };
        for relay in &state.snapshot.configured_relays {
            let relay_key = relay_map_key(relay);
            let Some(connection) = pool.relay(relay).await else {
                continue;
            };
            if connection.state().await != ConnectionState::Connected {
                continue;
            }
            let subscription_result = replace_ingress_subscription(
                &connection,
                previous_subscription_id.as_str(),
                next_subscription_id.as_str(),
                filters.clone(),
            )
            .await;
            if let Err(error) = subscription_result {
                state.relay_last_error.insert(
                    relay_key.clone(),
                    format!("Failed refreshing provider ingress filters for {relay}: {error}"),
                );
            } else {
                state.relay_last_error.remove(&relay_key);
            }
        }
    });
}

fn should_refresh_live_filters(state: &ProviderNip90LaneState) -> bool {
    state.wants_online
        && state.snapshot.connected_relays > 0
        && (state.provider_request_ingress_enabled() || !state.tracked_buyer_request_ids.is_empty())
        && state
            .next_live_filter_refresh_at
            .is_none_or(|refresh_at| Instant::now() >= refresh_at)
}

fn maybe_publish_handler_info(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    update_tx: &Sender<ProviderNip90LaneUpdate>,
) {
    if !state.wants_online || state.snapshot.connected_relays == 0 {
        return;
    }
    if state
        .next_handler_publish_retry_at
        .is_some_and(|retry_at| Instant::now() < retry_at)
    {
        return;
    }
    let Some(identity) = state.auth_identity.clone() else {
        return;
    };

    let desired_publication =
        if state.compute_capability.is_ready() || state.data_vending_profile.is_some() {
            HandlerPublicationState::Healthy
        } else {
            HandlerPublicationState::Disabled
        };
    if state.handler_publication_state == desired_publication {
        return;
    }

    let event = match build_provider_handler_event(&identity, desired_publication, state) {
        Ok(event) => event,
        Err(error) => {
            state.snapshot.last_error = Some(error.clone());
            state.snapshot.last_action = Some("provider handler publish failed".to_string());
            state.next_handler_publish_retry_at = Some(Instant::now() + HANDLER_PUBLISH_RETRY);
            return;
        }
    };
    let published = handle_publish_event(
        runtime,
        state,
        update_tx,
        handler_request_id(identity.public_key_hex.as_str()),
        ProviderNip90PublishRole::Capability,
        event,
    );
    if published {
        state.handler_publication_state = desired_publication;
        state.next_handler_publish_retry_at = None;
    } else {
        state.next_handler_publish_retry_at = Some(Instant::now() + HANDLER_PUBLISH_RETRY);
    }
}

fn build_ingress_filters(
    observe_provider_requests: bool,
    tracked_buyer_request_ids: &[String],
    auth_identity: Option<&ProviderNip90AuthIdentity>,
    compute_ready: bool,
    data_vending_profile: Option<&ProviderNip90DataVendingProfile>,
) -> Vec<serde_json::Value> {
    let mut filters = Vec::new();
    if observe_provider_requests {
        let request_since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| {
                duration
                    .as_secs()
                    .saturating_sub(LIVE_REQUEST_FILTER_LOOKBACK_SECONDS)
            })
            .unwrap_or(0);

        if let Some(profile) = data_vending_profile
            && profile
                .targeting_posture
                .trim()
                .eq_ignore_ascii_case("targeted_only")
        {
            let mut target_pubkeys = auth_identity
                .map(|identity| {
                    vec![
                        identity.public_key_hex.trim().to_ascii_lowercase(),
                        identity.npub.trim().to_ascii_lowercase(),
                    ]
                })
                .unwrap_or_default();
            target_pubkeys.retain(|value| !value.is_empty());
            target_pubkeys.sort();
            target_pubkeys.dedup();
            if !target_pubkeys.is_empty() {
                filters.push(json!({
                    "kinds": [profile.request_kind],
                    "#p": target_pubkeys,
                    "since": request_since,
                    "limit": 128
                }));
            }
            // Some public relays store targeted requests correctly but do not
            // reliably fan them out on `#p`-only live subscriptions for custom
            // DVM kinds. Subscribe to the dedicated request kind as a fallback
            // and keep strict local target checks in the intake path.
            filters.push(json!({
                "kinds": [profile.request_kind],
                "since": request_since,
                "limit": 128
            }));
        }

        if compute_ready || data_vending_profile.is_none() {
            let kinds = (JOB_REQUEST_KIND_MIN..=JOB_REQUEST_KIND_MAX)
                .filter(|kind| {
                    data_vending_profile.is_none_or(|profile| *kind != profile.request_kind)
                })
                .map(serde_json::Value::from)
                .collect::<Vec<_>>();
            if !kinds.is_empty() {
                filters.push(json!({
                    "kinds": kinds,
                    "since": request_since,
                    "limit": 256
                }));
            }
        }
    }
    if !tracked_buyer_request_ids.is_empty() {
        let response_kinds = (JOB_RESULT_KIND_MIN..=JOB_RESULT_KIND_MAX)
            .map(serde_json::Value::from)
            .chain(std::iter::once(serde_json::Value::from(KIND_JOB_FEEDBACK)))
            .collect::<Vec<_>>();
        filters.push(json!({
            "kinds": response_kinds,
            "#e": tracked_buyer_request_ids,
            "limit": 256
        }));
    }
    filters
}

fn elapsed_millis_u32(duration: Duration) -> u32 {
    duration.as_millis().min(u128::from(u32::MAX)) as u32
}

fn relay_map_key(relay_url: &str) -> String {
    relay_url.trim_end_matches('/').to_string()
}

fn normalize_relay_urls(relays: Vec<String>) -> Vec<String> {
    let mut normalized = relays
        .into_iter()
        .map(|relay| relay_map_key(relay.trim()))
        .filter(|relay| !relay.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn relay_health_rows_for(
    configured_relays: &[String],
    status: ProviderNip90RelayStatus,
) -> Vec<ProviderNip90RelayHealthRow> {
    configured_relays
        .iter()
        .map(|relay| ProviderNip90RelayHealthRow {
            relay_url: relay.clone(),
            status,
            latency_ms: None,
            last_seen_seconds_ago: None,
            last_error: None,
        })
        .collect()
}

fn prune_relay_observation_maps(state: &mut ProviderNip90LaneState) {
    let configured = state
        .snapshot
        .configured_relays
        .iter()
        .map(|relay| relay_map_key(relay))
        .collect::<HashSet<_>>();
    state
        .relay_last_seen
        .retain(|relay, _| configured.contains(relay));
    state
        .relay_latency_ms
        .retain(|relay, _| configured.contains(relay));
    state
        .relay_last_error
        .retain(|relay, _| configured.contains(relay));
}

fn refresh_relay_health_snapshot(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
) {
    prune_relay_observation_maps(state);
    let connection_expected = state.connection_expected();

    let connection_states = if let Some(pool) = state.pool.as_ref().cloned() {
        runtime.block_on(async {
            let relays = pool.relays().await;
            let mut states = HashMap::new();
            for relay in relays {
                states.insert(relay_map_key(relay.url()), relay.state().await);
            }
            states
        })
    } else {
        HashMap::new()
    };

    let now = Instant::now();
    state.snapshot.relay_health = state
        .snapshot
        .configured_relays
        .iter()
        .map(|relay| {
            let relay_key = relay_map_key(relay);
            let status = match connection_states.get(&relay_key).copied() {
                Some(ConnectionState::Connected) => ProviderNip90RelayStatus::Connected,
                Some(ConnectionState::Connecting) => ProviderNip90RelayStatus::Connecting,
                Some(ConnectionState::Disconnected) => {
                    if state.relay_last_error.contains_key(&relay_key) {
                        ProviderNip90RelayStatus::Error
                    } else if connection_expected {
                        ProviderNip90RelayStatus::Connecting
                    } else {
                        ProviderNip90RelayStatus::Disconnected
                    }
                }
                None => {
                    if state.relay_last_error.contains_key(&relay_key) {
                        ProviderNip90RelayStatus::Error
                    } else if connection_expected {
                        ProviderNip90RelayStatus::Connecting
                    } else {
                        ProviderNip90RelayStatus::Disconnected
                    }
                }
            };

            ProviderNip90RelayHealthRow {
                relay_url: relay.clone(),
                status,
                latency_ms: state.relay_latency_ms.get(&relay_key).copied(),
                last_seen_seconds_ago: state
                    .relay_last_seen
                    .get(&relay_key)
                    .map(|seen| now.saturating_duration_since(*seen).as_secs()),
                last_error: state.relay_last_error.get(&relay_key).cloned(),
            }
        })
        .collect();

    state.snapshot.connected_relays = state
        .snapshot
        .relay_health
        .iter()
        .filter(|relay| relay.status == ProviderNip90RelayStatus::Connected)
        .count();
}

fn apply_poll_outcome_telemetry(state: &mut ProviderNip90LaneState, outcome: &PollOutcome) {
    let now = Instant::now();
    for (relay, latency_ms) in &outcome.relay_latency_ms {
        state.relay_latency_ms.insert(relay.clone(), *latency_ms);
    }
    for relay in &outcome.relay_seen {
        state.relay_last_seen.insert(relay.clone(), now);
        state.relay_last_error.remove(relay);
    }
    for (relay, error) in &outcome.relay_errors {
        state.relay_last_error.insert(relay.clone(), error.clone());
    }
}

struct PollOutcome {
    requests: Vec<JobInboxNetworkRequest>,
    buyer_events: Vec<ProviderNip90BuyerResponseEvent>,
    connected_relays: usize,
    last_error: Option<String>,
    relay_errors: Vec<(String, String)>,
    relay_seen: Vec<String>,
    relay_latency_ms: Vec<(String, u32)>,
}

#[derive(Default)]
struct CatchupOutcome {
    requests: Vec<JobInboxNetworkRequest>,
    buyer_events: Vec<ProviderNip90BuyerResponseEvent>,
}

async fn poll_ingress(
    pool: Arc<RelayPool>,
    tracked_buyer_request_ids: &[String],
    local_pubkey_hex: Option<&str>,
) -> PollOutcome {
    let relays = pool.relays().await;
    let mut requests = Vec::new();
    let mut buyer_events = Vec::new();
    let mut connected_relays = 0usize;
    let mut last_error = None;
    let mut relay_errors = Vec::new();
    let mut relay_seen = Vec::new();
    let mut relay_latency_ms = Vec::new();
    let tracked_buyer_request_ids = tracked_buyer_request_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();

    for relay in relays {
        let relay_url = relay_map_key(relay.url());
        if relay.state().await == ConnectionState::Connected {
            connected_relays = connected_relays.saturating_add(1);
        }

        for _ in 0..MAX_MESSAGES_PER_RELAY_POLL {
            let recv_started = Instant::now();
            match tokio::time::timeout(RELAY_RECV_TIMEOUT, relay.recv()).await {
                Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                    relay_seen.push(relay_url.clone());
                    relay_latency_ms.push((
                        relay_url.clone(),
                        elapsed_millis_u32(recv_started.elapsed()),
                    ));
                    if let Some(request) = event_to_inbox_request(&event, Some(relay_url.as_str()))
                    {
                        requests.push(request);
                    } else if let Some(buyer_event) = event_to_buyer_response_event(
                        &event,
                        &tracked_buyer_request_ids,
                        local_pubkey_hex,
                        Some(relay_url.as_str()),
                    ) {
                        buyer_events.push(buyer_event);
                    }
                }
                Ok(Ok(Some(_))) => {
                    relay_seen.push(relay_url.clone());
                    relay_latency_ms.push((
                        relay_url.clone(),
                        elapsed_millis_u32(recv_started.elapsed()),
                    ));
                    continue;
                }
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    let message = format!("relay recv failed on {relay_url}: {error}");
                    last_error = Some(message.clone());
                    relay_errors.push((relay_url.clone(), message));
                    break;
                }
                Err(_) => break,
            }
        }
    }

    PollOutcome {
        requests,
        buyer_events,
        connected_relays,
        last_error,
        relay_errors,
        relay_seen,
        relay_latency_ms,
    }
}

async fn catchup_ingress_with_fresh_queries(state: &ProviderNip90LaneState) -> CatchupOutcome {
    let filters = state.ingress_filters();
    if filters.is_empty() || state.snapshot.configured_relays.is_empty() {
        return CatchupOutcome::default();
    }

    let tracked_buyer_request_ids = state
        .tracked_buyer_request_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let local_pubkey_hex = state
        .auth_identity
        .as_ref()
        .map(|identity| identity.public_key_hex.clone());
    let mut outcome = CatchupOutcome::default();
    let mut seen_request_ids = HashSet::<String>::new();
    let mut seen_buyer_event_ids = HashSet::<String>::new();

    for relay in &state.snapshot.configured_relays {
        let Ok(connection) = RelayConnection::new(relay.as_str()) else {
            continue;
        };
        if connection.connect().await.is_err() {
            continue;
        }
        let relay_url = relay_map_key(relay);

        for (filter_index, filter) in filters.iter().enumerate() {
            let subscription_id = format!(
                "{}-catchup-{}-{filter_index}",
                state.ingress_subscription_id,
                relay_url
                    .chars()
                    .filter(|ch| ch.is_ascii_alphanumeric())
                    .take(12)
                    .collect::<String>()
            );
            if connection
                .subscribe_filters(subscription_id.as_str(), vec![filter.clone()])
                .await
                .is_err()
            {
                continue;
            }

            let deadline = Instant::now() + CATCHUP_QUERY_TIMEOUT;
            loop {
                match tokio::time::timeout(CATCHUP_RECV_TIMEOUT, connection.recv()).await {
                    Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                        if let Some(request) =
                            event_to_inbox_request(&event, Some(relay_url.as_str()))
                        {
                            if seen_request_ids.insert(request.request_id.clone()) {
                                outcome.requests.push(request);
                            }
                        } else if let Some(buyer_event) = event_to_buyer_response_event(
                            &event,
                            &tracked_buyer_request_ids,
                            local_pubkey_hex.as_deref(),
                            Some(relay_url.as_str()),
                        ) && seen_buyer_event_ids.insert(buyer_event.event_id.clone())
                        {
                            outcome.buyer_events.push(buyer_event);
                        }
                    }
                    Ok(Ok(Some(RelayMessage::Eose(done_subscription_id))))
                        if done_subscription_id == subscription_id =>
                    {
                        break;
                    }
                    Ok(Ok(Some(_))) => {}
                    Ok(Ok(None)) | Ok(Err(_)) | Err(_) => {
                        if Instant::now() >= deadline {
                            break;
                        }
                    }
                }
                if Instant::now() >= deadline {
                    break;
                }
            }
            let _ = connection.unsubscribe(subscription_id.as_str()).await;
        }
        let _ = connection.disconnect().await;
    }

    outcome
}

pub(crate) fn event_to_inbox_request(
    event: &Event,
    relay_url: Option<&str>,
) -> Option<JobInboxNetworkRequest> {
    if !is_job_request_kind(event.kind) {
        return None;
    }

    let parsed = JobRequest::from_event(event);
    let raw_event_json = serde_json::to_string_pretty(event).ok();
    let (
        capability,
        skill_scope_id,
        price_sats,
        ttl_seconds,
        target_provider_pubkeys,
        encrypted,
        encrypted_payload,
        execution_input,
        execution_prompt,
        execution_params,
        requested_model,
        requested_output_mime,
        validation,
        parsed_event_shape,
    ) = match parsed.as_ref() {
        Ok(request) => {
            let data_vending_request = DataVendingRequest::from_job_request(request.clone()).ok();
            let bid_msats = request.bid.unwrap_or(0);
            let price_sats = msats_to_sats_ceil(bid_msats);
            let ttl_seconds = extract_ttl_seconds(request).unwrap_or(DEFAULT_TTL_SECONDS);
            let skill_scope_id = extract_param(request, "skill_scope_id")
                .or_else(|| extract_param(request, "skill_scope"));
            let target_provider_pubkeys =
                normalize_provider_keys(request.service_providers.as_slice());
            let encrypted = request.encrypted;
            let encrypted_payload = if encrypted {
                Some(event.content.clone())
            } else {
                None
            };
            let execution_input = if encrypted {
                None
            } else {
                execution_input_from_request(request)
            };
            let mut normalized_params = normalized_request_params(request);
            if let Some(data_request) = data_vending_request.as_ref() {
                append_unique_param(
                    &mut normalized_params,
                    "oa_profile",
                    OPENAGENTS_DATA_VENDING_PROFILE,
                );
                append_unique_param(
                    &mut normalized_params,
                    "oa_asset_ref",
                    data_request.asset_ref.as_str(),
                );
                append_unique_param(
                    &mut normalized_params,
                    "oa_delivery_mode",
                    data_request.delivery_mode.as_str(),
                );
                append_unique_param(
                    &mut normalized_params,
                    "oa_preview_posture",
                    data_request.preview_posture.as_str(),
                );
                for scope in &data_request.permission_scopes {
                    append_unique_param(&mut normalized_params, "oa_scope", scope.as_str());
                }
            }
            let capability = if data_vending_request.is_some() {
                "openagents.data.access".to_string()
            } else {
                capability_for_kind(request.kind)
            };
            let validation = if let Some(data_request) = data_vending_request.as_ref() {
                validate_data_vending_request(
                    request,
                    data_request,
                    price_sats,
                    target_provider_pubkeys.as_slice(),
                    event,
                )
            } else if request.kind == KIND_JOB_TEXT_GENERATION
                && !encrypted
                && normalized_text_generation_prompt(request).is_none()
            {
                JobInboxValidation::Invalid(
                    "text-generation request missing prompt/text input".to_string(),
                )
            } else if request.kind != KIND_JOB_TEXT_GENERATION {
                JobInboxValidation::Invalid(format!(
                    "unsupported request kind {}; provider currently serves only kind 5050 text generation",
                    request.kind
                ))
            } else if let Some(output_mime) = requested_output_mime(request) {
                if supported_output_mime(output_mime.as_str()) {
                    JobInboxValidation::Valid
                } else {
                    JobInboxValidation::Invalid(format!(
                        "unsupported output MIME '{}'; provider currently serves text/plain or text/markdown",
                        output_mime
                    ))
                }
            } else if request.content.trim().is_empty() && request.inputs.is_empty() {
                JobInboxValidation::Invalid("request missing content/input payload".to_string())
            } else if encrypted && event.content.trim().is_empty() {
                JobInboxValidation::Invalid(
                    "request marked encrypted but content payload is empty".to_string(),
                )
            } else if request.bid.is_none() || price_sats == 0 {
                JobInboxValidation::Pending
            } else {
                JobInboxValidation::Valid
            };
            let parsed_event_shape =
                Some(if let Some(data_request) = data_vending_request.as_ref() {
                    format_data_vending_request_shape(
                        event,
                        request,
                        data_request,
                        price_sats,
                        ttl_seconds,
                    )
                } else {
                    format_nip90_request_shape(event, request, price_sats, ttl_seconds)
                });
            (
                capability,
                skill_scope_id,
                price_sats,
                ttl_seconds,
                target_provider_pubkeys,
                encrypted,
                encrypted_payload,
                execution_input,
                if data_vending_request.is_some() {
                    None
                } else {
                    normalized_text_generation_prompt(request)
                },
                normalized_params
                    .into_iter()
                    .map(|(key, value)| JobExecutionParam { key, value })
                    .collect::<Vec<_>>(),
                extract_param(request, "model"),
                requested_output_mime(request),
                validation,
                parsed_event_shape,
            )
        }
        Err(error) => (
            format!("nip90.kind.{}", event.kind),
            None,
            0,
            DEFAULT_TTL_SECONDS,
            Vec::new(),
            false,
            None,
            None,
            None,
            Vec::new(),
            None,
            None,
            JobInboxValidation::Invalid(format!("invalid NIP-90 request tags: {error}")),
            Some(format!(
                "request.parse_error={error}\n{}",
                format_generic_event_shape(event)
            )),
        ),
    };

    Some(JobInboxNetworkRequest {
        request_id: event.id.clone(),
        requester: event.pubkey.clone(),
        source_relay_url: relay_url.map(ToString::to_string),
        demand_source: crate::app_state::JobDemandSource::OpenNetwork,
        request_kind: event.kind,
        capability,
        execution_input,
        execution_prompt,
        execution_params,
        requested_model,
        requested_output_mime,
        target_provider_pubkeys,
        encrypted,
        encrypted_payload,
        parsed_event_shape,
        raw_event_json,
        skill_scope_id,
        skl_manifest_a: None,
        skl_manifest_event_id: None,
        // The request event itself is the external authority for accepted->running.
        sa_tick_request_event_id: Some(event.id.clone()),
        sa_tick_result_event_id: None,
        ac_envelope_event_id: None,
        price_sats,
        ttl_seconds,
        created_at_epoch_seconds: Some(event.created_at),
        expires_at_epoch_seconds: Some(event.created_at.saturating_add(ttl_seconds)),
        validation,
    })
}

fn execution_input_from_request(request: &JobRequest) -> Option<String> {
    let mut sections = Vec::<String>::new();

    if request.kind == KIND_JOB_TEXT_GENERATION {
        if let Some(prompt) = normalized_text_generation_prompt(request) {
            sections.push(format!("Prompt:\n{prompt}"));
        }

        let additional_inputs = request
            .inputs
            .iter()
            .filter(|input| input.input_type != InputType::Text)
            .map(format_input_line)
            .collect::<Vec<_>>();
        if !additional_inputs.is_empty() {
            sections.push(format!(
                "Additional Inputs:\n{}",
                additional_inputs.join("\n")
            ));
        }
    } else {
        let content = request.content.trim();
        if !content.is_empty() {
            sections.push(format!("Content:\n{content}"));
        }

        if !request.inputs.is_empty() {
            let inputs = request
                .inputs
                .iter()
                .map(format_input_line)
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("Inputs:\n{inputs}"));
        }
    }

    let normalized_params = normalized_request_params(request);
    if !normalized_params.is_empty() {
        let params = normalized_params
            .iter()
            .map(|(key, value)| format!("- {key}={value}"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Parameters:\n{params}"));
    }

    if let Some(output) = requested_output_mime(request) {
        sections.push(format!("Requested output: {output}"));
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n\n"))
    }
}

fn normalized_text_generation_prompt(request: &JobRequest) -> Option<String> {
    if request.kind != KIND_JOB_TEXT_GENERATION {
        return None;
    }

    let mut segments = Vec::<String>::new();
    push_unique_prompt_segment(&mut segments, request.content.as_str());
    for input in request
        .inputs
        .iter()
        .filter(|input| input.input_type == InputType::Text)
    {
        push_unique_prompt_segment(&mut segments, input.data.as_str());
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments.join("\n\n"))
    }
}

fn push_unique_prompt_segment(segments: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    if segments.iter().any(|existing| existing == trimmed) {
        return;
    }
    segments.push(trimmed.to_string());
}

fn normalized_request_params(request: &JobRequest) -> Vec<(String, String)> {
    let mut normalized = BTreeMap::<String, String>::new();
    for param in &request.params {
        let key = canonical_param_key(param.key.as_str());
        let value = param.value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        normalized.insert(key, value.to_string());
    }
    normalized.into_iter().collect()
}

fn append_unique_param(params: &mut Vec<(String, String)>, key: &str, value: &str) {
    let key = canonical_param_key(key);
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return;
    }
    if params
        .iter()
        .any(|(existing_key, existing_value)| existing_key == &key && existing_value == value)
    {
        return;
    }
    params.push((key, value.to_string()));
    params.sort();
}

fn validate_data_vending_request(
    request: &JobRequest,
    data_request: &DataVendingRequest,
    price_sats: u64,
    target_provider_pubkeys: &[String],
    event: &Event,
) -> JobInboxValidation {
    if data_request.asset_ref.trim().is_empty() {
        JobInboxValidation::Invalid("data-vending request missing oa_asset_ref".to_string())
    } else if data_request.permission_scopes.is_empty() {
        JobInboxValidation::Invalid("data-vending request missing oa_scope".to_string())
    } else if target_provider_pubkeys.is_empty() {
        JobInboxValidation::Invalid(
            "data-vending request missing target provider for targeted MVP posture".to_string(),
        )
    } else if request.encrypted && event.content.trim().is_empty() {
        JobInboxValidation::Invalid(
            "data-vending request marked encrypted but content payload is empty".to_string(),
        )
    } else if request.bid.is_none() || price_sats == 0 {
        JobInboxValidation::Pending
    } else {
        JobInboxValidation::Valid
    }
}

fn requested_output_mime(request: &JobRequest) -> Option<String> {
    request
        .output
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn supported_output_mime(value: &str) -> bool {
    SUPPORTED_TEXT_OUTPUT_MIME
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
}

fn canonical_param_key(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "top-k" | "top_k" => "top_k".to_string(),
        "top-p" | "top_p" => "top_p".to_string(),
        other => other.to_string(),
    }
}

fn format_input_line(input: &nostr::nip90::JobInput) -> String {
    let mut line = format!("- {}: {}", input.input_type.as_str(), input.data.trim());
    if let Some(marker) = input
        .marker
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        line.push_str(format!(" [marker={marker}]").as_str());
    }
    if let Some(relay) = input
        .relay
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        line.push_str(format!(" [relay={relay}]").as_str());
    }
    line
}

pub(crate) fn event_to_buyer_response_event(
    event: &Event,
    tracked_buyer_request_ids: &HashSet<String>,
    local_pubkey_hex: Option<&str>,
    relay_url: Option<&str>,
) -> Option<ProviderNip90BuyerResponseEvent> {
    if local_pubkey_hex.is_some_and(|pubkey| pubkey == event.pubkey) {
        return None;
    }
    if is_job_feedback_kind(event.kind) {
        let feedback = parse_feedback_event(event, relay_url)?;
        if !tracked_buyer_request_ids.contains(feedback.request_id.as_str()) {
            return None;
        }
        return Some(feedback);
    }
    if is_job_result_kind(event.kind) {
        let result = JobResult::from_event(event).ok()?;
        if !tracked_buyer_request_ids.contains(result.request_id.as_str()) {
            return None;
        }
        let bolt11 = result
            .bolt11
            .clone()
            .or_else(|| extract_bolt11_from_response_event(event));
        let (status, status_extra) = parse_status_tags(event.tags.as_slice());
        let parsed_event_shape = Some(format_buyer_response_event_shape(
            event,
            ProviderNip90BuyerResponseKind::Result,
            result.request_id.as_str(),
            status.as_deref(),
            status_extra.as_deref(),
            result.amount,
            bolt11.as_deref(),
        ));
        let raw_event_json = serde_json::to_string_pretty(event).ok();
        return Some(ProviderNip90BuyerResponseEvent {
            request_id: result.request_id,
            provider_pubkey: event.pubkey.clone(),
            event_id: event.id.clone(),
            relay_url: relay_url.map(ToString::to_string),
            kind: ProviderNip90BuyerResponseKind::Result,
            status,
            status_extra,
            amount_msats: result.amount,
            bolt11,
            parsed_event_shape,
            raw_event_json,
        });
    }
    None
}

fn parse_feedback_event(
    event: &Event,
    relay_url: Option<&str>,
) -> Option<ProviderNip90BuyerResponseEvent> {
    let mut request_id = None::<String>;
    let (status, status_extra) = parse_status_tags(event.tags.as_slice());
    let mut amount_msats = None::<u64>;
    let mut bolt11 = None::<String>;

    for tag in &event.tags {
        if tag.len() < 2 {
            continue;
        }
        match tag[0].as_str() {
            "e" => request_id = Some(tag[1].clone()),
            "amount" => {
                amount_msats = tag[1].parse::<u64>().ok();
                if tag.len() >= 3 {
                    bolt11 = Some(tag[2].clone());
                }
            }
            "bolt11" => {
                bolt11 = Some(tag[1].clone());
            }
            _ => {}
        }
    }
    if bolt11.is_none() {
        bolt11 = extract_bolt11_from_response_event(event);
    }

    let request_id = request_id?;
    let parsed_event_shape = Some(format_buyer_response_event_shape(
        event,
        ProviderNip90BuyerResponseKind::Feedback,
        request_id.as_str(),
        status.as_deref(),
        status_extra.as_deref(),
        amount_msats,
        bolt11.as_deref(),
    ));
    let raw_event_json = serde_json::to_string_pretty(event).ok();

    Some(ProviderNip90BuyerResponseEvent {
        request_id,
        provider_pubkey: event.pubkey.clone(),
        event_id: event.id.clone(),
        relay_url: relay_url.map(ToString::to_string),
        kind: ProviderNip90BuyerResponseKind::Feedback,
        status,
        status_extra,
        amount_msats,
        bolt11,
        parsed_event_shape,
        raw_event_json,
    })
}

fn extract_bolt11_from_response_event(event: &Event) -> Option<String> {
    for tag in &event.tags {
        if tag.len() < 2 {
            continue;
        }
        match tag[0].as_str() {
            "amount" if tag.len() >= 3 => {
                if let Some(invoice) = normalize_bolt11_candidate(tag[2].as_str()) {
                    return Some(invoice);
                }
            }
            "bolt11" | "invoice" | "payment_request" => {
                if let Some(invoice) = normalize_bolt11_candidate(tag[1].as_str()) {
                    return Some(invoice);
                }
            }
            _ => {}
        }
    }

    extract_bolt11_from_content(event.content.as_str())
}

fn extract_bolt11_from_content(content: &str) -> Option<String> {
    if let Some(invoice) = normalize_bolt11_candidate(content) {
        return Some(invoice);
    }

    let value = serde_json::from_str::<serde_json::Value>(content.trim()).ok()?;
    extract_bolt11_from_json_value(&value)
}

fn extract_bolt11_from_json_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => normalize_bolt11_candidate(text),
        serde_json::Value::Array(items) => items.iter().find_map(extract_bolt11_from_json_value),
        serde_json::Value::Object(map) => {
            for key in ["bolt11", "invoice", "payment_request"] {
                if let Some(invoice) = map
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .and_then(normalize_bolt11_candidate)
                {
                    return Some(invoice);
                }
            }
            map.values().find_map(extract_bolt11_from_json_value)
        }
        _ => None,
    }
}

fn normalize_bolt11_candidate(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = if trimmed
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("lightning:"))
    {
        trimmed[10..].trim()
    } else {
        trimmed
    };
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("ln") && !lower.starts_with("lnurl") && lower.contains('1') {
        Some(normalized.to_string())
    } else {
        None
    }
}

fn parse_status_tags(tags: &[Vec<String>]) -> (Option<String>, Option<String>) {
    for tag in tags {
        if tag.first().is_some_and(|value| value == "status") {
            let status = tag.get(1).map(ToString::to_string);
            let status_extra = tag.get(2).map(ToString::to_string);
            return (status, status_extra);
        }
    }
    (None, None)
}

fn format_buyer_response_event_shape(
    event: &Event,
    kind: ProviderNip90BuyerResponseKind,
    request_id: &str,
    status: Option<&str>,
    status_extra: Option<&str>,
    amount_msats: Option<u64>,
    bolt11: Option<&str>,
) -> String {
    format!(
        "{}\nbuyer_response.kind={} request_id={} provider_pubkey={} status={} status_extra={} amount_msats={} bolt11={}",
        format_generic_event_shape(event),
        kind.label(),
        request_id,
        event.pubkey,
        status.unwrap_or("none"),
        status_extra.unwrap_or("none"),
        amount_msats
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        bolt11
            .map(|value| truncate_summary_value(value, 64))
            .unwrap_or_else(|| "none".to_string()),
    )
}

fn truncate_summary_value(value: &str, max_chars: usize) -> String {
    if value.len() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn normalize_provider_keys(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn format_nip90_request_shape(
    event: &Event,
    request: &JobRequest,
    price_sats: u64,
    ttl_seconds: u64,
) -> String {
    let input_types = request
        .inputs
        .iter()
        .map(|input| input.input_type.as_str().to_string())
        .collect::<Vec<_>>();
    let param_keys = request
        .params
        .iter()
        .map(|param| canonical_param_key(param.key.as_str()))
        .collect::<Vec<_>>();
    let relays = request.relays.clone();
    let service_providers = request.service_providers.clone();

    format!(
        "{}\nrequest.kind={} result.kind={} capability={}\nrequest.inputs={} input.types=[{}]\nrequest.params={} param.keys=[{}]\nrequest.output={} request.bid_msats={} request.price_sats={} request.ttl_seconds={}\nrequest.relays={} request.service_providers={} request.encrypted={} content_bytes={}",
        format_generic_event_shape(event),
        request.kind,
        request.result_kind(),
        capability_for_kind(request.kind),
        request.inputs.len(),
        summarize_string_list(input_types.as_slice(), 6),
        request.params.len(),
        summarize_string_list(param_keys.as_slice(), 6),
        request.output.as_deref().unwrap_or("none"),
        request.bid.unwrap_or(0),
        price_sats,
        ttl_seconds,
        summarize_string_list(relays.as_slice(), 4),
        summarize_string_list(service_providers.as_slice(), 4),
        request.encrypted,
        request.content.len(),
    )
}

fn format_data_vending_request_shape(
    event: &Event,
    request: &JobRequest,
    data_request: &DataVendingRequest,
    price_sats: u64,
    ttl_seconds: u64,
) -> String {
    let mut base = format_nip90_request_shape(event, request, price_sats, ttl_seconds);
    base.push_str(
        format!(
            "\nprofile={} asset_ref={} scopes=[{}] delivery_mode={} preview_posture={} targeted={} encrypted={}",
            OPENAGENTS_DATA_VENDING_PROFILE,
            data_request.asset_ref,
            data_request.permission_scopes.join(","),
            data_request.delivery_mode.as_str(),
            data_request.preview_posture.as_str(),
            !data_request.service_providers.is_empty(),
            data_request.encrypted,
        )
        .as_str(),
    );
    base
}

fn format_generic_event_shape(event: &Event) -> String {
    let tag_names = event
        .tags
        .iter()
        .filter_map(|tag| tag.first().cloned())
        .collect::<Vec<_>>();
    format!(
        "event.id={} event.kind={} event.created_at={} event.pubkey={} event.tags={} event.tag_names=[{}] event.content_bytes={} event.sig_hex_len={}",
        event.id,
        event.kind,
        event.created_at,
        event.pubkey,
        event.tags.len(),
        summarize_string_list(tag_names.as_slice(), 8),
        event.content.len(),
        event.sig.len(),
    )
}

fn summarize_string_list(values: &[String], max_items: usize) -> String {
    if values.is_empty() {
        return "none".to_string();
    }
    let limit = max_items.max(1);
    let visible = values
        .iter()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .join(",");
    let hidden = values.len().saturating_sub(limit);
    if hidden > 0 {
        format!("{visible},+{hidden}more")
    } else {
        visible
    }
}

fn build_provider_handler_event(
    identity: &ProviderNip90AuthIdentity,
    publication_state: HandlerPublicationState,
    state: &ProviderNip90LaneState,
) -> Result<Event, String> {
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("failed reading system clock for handler event: {error}"))?
        .as_secs();
    let d_tag = format!(
        "autopilot-provider-{}",
        identity.public_key_hex.chars().take(12).collect::<String>()
    );
    let mut tags = vec![
        vec!["d".to_string(), d_tag],
        vec!["t".to_string(), "autopilot".to_string()],
        vec!["t".to_string(), "openagents".to_string()],
    ];
    if publication_state == HandlerPublicationState::Healthy {
        for kind in supported_handler_kinds(state) {
            tags.push(vec!["k".to_string(), kind.to_string()]);
        }
    }
    let content = serde_json::json!({
        "name": HANDLER_METADATA_NAME,
        "about": HANDLER_METADATA_ABOUT,
        "backend": state.compute_capability.backend_or_default(),
        "status": if publication_state == HandlerPublicationState::Healthy {
            HANDLER_STATUS_HEALTHY
        } else {
            HANDLER_STATUS_DEGRADED
        },
        "serving_model": state
            .compute_capability
            .ready_model
            .as_deref()
            .or(state.compute_capability.configured_model.as_deref()),
        "last_error": state.compute_capability.last_error.clone(),
        "data_vending": state.data_vending_profile.as_ref().map(|profile| json!({
            "profile_id": profile.profile_id,
            "request_kind": profile.request_kind,
            "result_kind": profile.result_kind,
            "kind_posture": profile.kind_posture,
            "targeting_posture": profile.targeting_posture,
            "asset_families": profile.asset_families,
            "delivery_modes": profile.delivery_modes,
            "preview_postures": profile.preview_postures,
        })),
    })
    .to_string();
    let template = EventTemplate {
        created_at,
        kind: NIP89_HANDLER_KIND,
        tags,
        content,
    };
    let private_key = parse_private_key_hex(identity.private_key_hex.as_str())?;
    nostr::finalize_event(&template, &private_key)
        .map_err(|error| format!("failed signing provider handler event: {error}"))
}

fn supported_handler_kinds(state: &ProviderNip90LaneState) -> Vec<u16> {
    let mut kinds = Vec::new();
    if state.compute_capability.is_ready() {
        kinds.push(KIND_JOB_TEXT_GENERATION);
    }
    if let Some(profile) = state.data_vending_profile.as_ref() {
        kinds.push(profile.request_kind);
    }
    kinds.sort();
    kinds.dedup();
    kinds
}

fn handler_request_id(public_key_hex: &str) -> String {
    format!(
        "handler:{}",
        public_key_hex.chars().take(12).collect::<String>()
    )
}

fn parse_private_key_hex(private_key_hex: &str) -> Result<[u8; 32], String> {
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

fn extract_param(request: &JobRequest, key: &str) -> Option<String> {
    let normalized_key = canonical_param_key(key);
    request
        .params
        .iter()
        .rev()
        .find(|param| canonical_param_key(param.key.as_str()) == normalized_key)
        .map(|param| param.value.trim().to_string())
}

fn extract_ttl_seconds(request: &JobRequest) -> Option<u64> {
    ["ttl", "timeout", "timeout_seconds"]
        .iter()
        .find_map(|key| extract_param(request, key))
        .and_then(|raw| raw.parse::<u64>().ok())
}

fn msats_to_sats_ceil(msats: u64) -> u64 {
    if msats == 0 {
        return 0;
    }
    msats.saturating_add(999) / 1000
}

fn capability_for_kind(kind: u16) -> String {
    match kind {
        KIND_JOB_TEXT_EXTRACTION => "text.extraction",
        KIND_JOB_SUMMARIZATION => "text.summarization",
        KIND_JOB_TRANSLATION => "text.translation",
        KIND_JOB_TEXT_GENERATION => "text.generation",
        KIND_JOB_IMAGE_GENERATION => "image.generation",
        KIND_JOB_SPEECH_TO_TEXT => "speech.to_text",
        KIND_JOB_SANDBOX_RUN => "openagents.sandbox.run",
        KIND_JOB_REPO_INDEX => "openagents.repo.index",
        KIND_JOB_PATCH_GEN => "openagents.patch.generate",
        KIND_JOB_CODE_REVIEW => "openagents.code.review",
        KIND_JOB_RLM_SUBQUERY => "openagents.rlm.subquery",
        _ => return format!("nip90.kind.{kind}"),
    }
    .to_string()
}

fn normalize_request_ids(request_ids: Vec<String>) -> Vec<String> {
    let mut normalized = request_ids
        .into_iter()
        .map(|request_id| request_id.trim().to_string())
        .filter(|request_id| !request_id.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_relays(relays: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut normalized = Vec::new();
    for relay in relays {
        let relay = relay.trim();
        if relay.is_empty() {
            continue;
        }
        if !relay.starts_with("wss://") && !relay.starts_with("ws://") {
            continue;
        }
        if seen.insert(relay.to_string()) {
            normalized.push(relay.to_string());
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderNip90AuthIdentity, ProviderNip90BuyerResponseKind, ProviderNip90ComputeCapability,
        ProviderNip90DataVendingProfile, ProviderNip90LaneCommand, ProviderNip90LaneUpdate,
        ProviderNip90LaneWorker, ProviderNip90PublishRole, ProviderNip90RelayStatus,
        ensure_connected_pool, event_to_buyer_response_event, event_to_inbox_request,
        execution_input_from_request, poll_ingress, resubscribe_ingress_filters,
    };
    use crate::app_state::{
        ActiveJobState, EarningsScoreboardState, JobHistoryState, JobHistoryStatus, JobInboxState,
        JobLifecycleStage, ProviderMode, ProviderRuntimeState,
    };
    use crate::state::job_inbox::JobInboxValidation;
    use futures_util::{SinkExt, StreamExt};
    use nostr::Event;
    use nostr::nip90::{
        DataVendingDeliveryMode, DataVendingPreviewPosture, DataVendingRequest, JobInput,
        JobRequest, KIND_JOB_TEXT_GENERATION, OPENAGENTS_DATA_VENDING_PROFILE,
        create_data_vending_request_event,
    };
    use openagents_spark::{Balance, PaymentSummary};
    use serde_json::Value;
    use std::collections::HashSet;
    use std::time::{Duration, Instant};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    fn fixture_auth_identity() -> ProviderNip90AuthIdentity {
        ProviderNip90AuthIdentity {
            npub: "npub1autopilotprovider".to_string(),
            public_key_hex: nostr::get_public_key_hex(
                &hex::decode("d217c1ff2f8a65c3e3a1740db3b9f58b8c848bb45e26d00ed4714e4a0f4ceecf")
                    .expect("decode fixture key")
                    .try_into()
                    .expect("fixture key length"),
            )
            .expect("derive fixture pubkey"),
            private_key_hex: "d217c1ff2f8a65c3e3a1740db3b9f58b8c848bb45e26d00ed4714e4a0f4ceecf"
                .to_string(),
        }
    }

    fn fixture_gpt_oss_capability() -> ProviderNip90ComputeCapability {
        ProviderNip90ComputeCapability {
            backend: "gpt_oss".to_string(),
            reachable: true,
            configured_model: Some("llama3.2:latest".to_string()),
            ready_model: Some("llama3.2:latest".to_string()),
            available_models: vec!["llama3.2:latest".to_string()],
            loaded_models: vec!["llama3.2:latest".to_string()],
            last_error: None,
        }
    }

    fn fixture_apple_fm_capability() -> ProviderNip90ComputeCapability {
        ProviderNip90ComputeCapability {
            backend: "apple_foundation_models".to_string(),
            reachable: true,
            configured_model: None,
            ready_model: Some("apple-foundation-model".to_string()),
            available_models: vec!["apple-foundation-model".to_string()],
            loaded_models: Vec::new(),
            last_error: None,
        }
    }

    fn fixture_lane_state() -> super::ProviderNip90LaneState {
        super::ProviderNip90LaneState {
            snapshot: super::ProviderNip90LaneSnapshot::default(),
            wants_online: false,
            pool: None,
            auth_identity: Some(fixture_auth_identity()),
            compute_capability: ProviderNip90ComputeCapability::default(),
            data_vending_profile: None,
            handler_publication_state: super::HandlerPublicationState::None,
            next_handler_publish_retry_at: None,
            tracked_provider_publish_request_ids: Vec::new(),
            tracked_buyer_request_ids: Vec::new(),
            preview_seen_request_ids: HashSet::new(),
            relay_last_seen: std::collections::HashMap::new(),
            relay_latency_ms: std::collections::HashMap::new(),
            relay_last_error: std::collections::HashMap::new(),
            next_live_filter_refresh_at: None,
            ingress_subscription_generation: 1,
            ingress_subscription_id: format!("{}-1", super::SUBSCRIPTION_ID_PREFIX),
        }
    }

    fn fixture_data_vending_profile() -> ProviderNip90DataVendingProfile {
        ProviderNip90DataVendingProfile {
            profile_id: OPENAGENTS_DATA_VENDING_PROFILE.to_string(),
            request_kind: crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND,
            result_kind: crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND + 1000,
            kind_posture: crate::app_state::OPENAGENTS_DATA_VENDING_KIND_POSTURE.to_string(),
            targeting_posture: crate::app_state::OPENAGENTS_DATA_VENDING_TARGETING_POSTURE
                .to_string(),
            asset_families: vec!["project_context_bundle".to_string()],
            delivery_modes: vec![
                "encrypted_pointer".to_string(),
                "delivery_bundle_ref".to_string(),
            ],
            preview_postures: vec!["metadata_only".to_string(), "inline_preview".to_string()],
        }
    }

    #[test]
    fn build_ingress_filters_target_targeted_data_market_requests_to_local_identity() {
        let identity = fixture_auth_identity();
        let profile = fixture_data_vending_profile();
        let filters =
            super::build_ingress_filters(true, &[], Some(&identity), false, Some(&profile));
        assert_eq!(
            filters.len(),
            2,
            "expected targeted and dedicated-kind fallback filters"
        );
        let filter = filters.first().expect("targeted filter");
        let p_values = filter["#p"].as_array().expect("p filter array");
        assert!(
            p_values
                .iter()
                .any(|value| value.as_str() == Some(identity.public_key_hex.as_str()))
        );
        assert!(
            p_values
                .iter()
                .any(|value| value.as_str() == Some(identity.npub.as_str()))
        );
        assert_eq!(
            filter["kinds"].as_array().expect("kinds array"),
            &vec![Value::from(profile.request_kind)]
        );
        assert!(
            filter.get("since").is_some(),
            "targeted public-relay filter should be live-only"
        );
        let fallback = filters.get(1).expect("fallback filter");
        assert!(
            fallback.get("#p").is_none(),
            "fallback filter should widen relay fanout without target indexing"
        );
        assert_eq!(
            fallback["kinds"].as_array().expect("fallback kinds array"),
            &vec![Value::from(profile.request_kind)]
        );
        assert!(
            fallback.get("since").is_some(),
            "fallback public-relay filter should stay live-only"
        );
    }

    #[test]
    fn maps_nip90_request_event_to_job_inbox_request() {
        let event = Event {
            id: "req-001".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_100,
            kind: 5050,
            tags: vec![
                vec!["bid".to_string(), "25000".to_string()],
                vec!["param".to_string(), "ttl".to_string(), "90".to_string()],
                vec!["p".to_string(), "npub1localprovider".to_string()],
                vec![
                    "param".to_string(),
                    "skill_scope_id".to_string(),
                    "33400:npub1agent:summarize-text:0.1.0".to_string(),
                ],
            ],
            content: "generate summary".to_string(),
            sig: "11".repeat(64),
        };

        let row = event_to_inbox_request(&event, Some("wss://relay.ingress.test/"))
            .expect("event should map to inbox row");
        assert_eq!(row.request_id, "req-001");
        assert_eq!(row.requester, "npub1buyer");
        assert_eq!(
            row.source_relay_url.as_deref(),
            Some("wss://relay.ingress.test/")
        );
        assert_eq!(row.capability, "text.generation");
        assert_eq!(row.demand_source.label(), "open-network");
        assert_eq!(row.request_kind, 5050);
        assert_eq!(row.price_sats, 25);
        assert_eq!(row.ttl_seconds, 90);
        assert_eq!(row.created_at_epoch_seconds, Some(1_760_000_100));
        assert_eq!(row.expires_at_epoch_seconds, Some(1_760_000_190));
        assert_eq!(
            row.target_provider_pubkeys,
            vec!["npub1localprovider".to_string()]
        );
        assert!(!row.encrypted);
        assert!(row.encrypted_payload.is_none());
        assert_eq!(
            row.skill_scope_id,
            Some("33400:npub1agent:summarize-text:0.1.0".to_string())
        );
        assert_eq!(row.sa_tick_request_event_id.as_deref(), Some("req-001"));
        assert!(
            row.parsed_event_shape
                .as_deref()
                .is_some_and(|value| value.contains("request.kind=5050"))
        );
        assert!(
            row.raw_event_json
                .as_deref()
                .is_some_and(|value| value.contains("\"kind\": 5050"))
        );
    }

    #[test]
    fn maps_data_vending_request_event_to_job_inbox_request() {
        let template = create_data_vending_request_event(
            &DataVendingRequest::new(
                crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND,
                "asset://repo-alpha",
                "read.context",
            )
            .expect("data request")
            .with_delivery_mode(DataVendingDeliveryMode::EncryptedPointer)
            .with_preview_posture(DataVendingPreviewPosture::MetadataOnly)
            .with_bid(42_000)
            .add_service_provider("npub1localprovider")
            .add_relay("wss://relay.ingress.test")
            .with_encrypted_content("{\"ciphertext\":\"nip44\"}"),
        )
        .expect("template");
        let event = Event {
            id: "req-data-001".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_220,
            kind: crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND,
            tags: template.tags,
            content: template.content,
            sig: "33".repeat(64),
        };

        let row = event_to_inbox_request(&event, Some("wss://relay.ingress.test/"))
            .expect("data-vending request should map");
        assert_eq!(row.capability, "openagents.data.access");
        assert_eq!(row.price_sats, 42);
        assert!(row.encrypted);
        assert!(matches!(row.validation, JobInboxValidation::Valid));
        assert!(
            row.execution_params
                .iter()
                .any(|param| param.key == "oa_asset_ref" && param.value == "asset://repo-alpha")
        );
        assert!(
            row.execution_params
                .iter()
                .any(|param| param.key == "oa_scope" && param.value == "read.context")
        );
        assert!(
            row.parsed_event_shape
                .as_deref()
                .is_some_and(|shape| shape.contains("profile=openagents.data-vending.v1"))
        );
    }

    #[test]
    fn preview_request_filter_skips_target_mismatches_and_duplicates() {
        let mut state = fixture_lane_state();
        let local_identity = fixture_auth_identity();

        let targeted_elsewhere = Event {
            id: "req-target-other".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_100,
            kind: 5050,
            tags: vec![
                vec!["bid".to_string(), "2000".to_string()],
                vec!["p".to_string(), "npub1someotherprovider".to_string()],
            ],
            content: "generate summary".to_string(),
            sig: "11".repeat(64),
        };
        let targeted_elsewhere = event_to_inbox_request(&targeted_elsewhere, None)
            .expect("event should map to inbox row");
        assert!(
            !state.preview_request_should_reach_ui(&targeted_elsewhere),
            "preview should drop targeted requests for other providers before they hit the UI"
        );

        let targeted_here = Event {
            id: "req-target-local".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_101,
            kind: 5050,
            tags: vec![
                vec!["bid".to_string(), "2000".to_string()],
                vec!["p".to_string(), local_identity.npub.clone()],
            ],
            content: "generate summary".to_string(),
            sig: "22".repeat(64),
        };
        let targeted_here =
            event_to_inbox_request(&targeted_here, None).expect("event should map to inbox row");
        assert!(
            state.preview_request_should_reach_ui(&targeted_here),
            "first matching preview request should reach the UI"
        );
        assert!(
            !state.preview_request_should_reach_ui(&targeted_here),
            "duplicate preview request should not churn the UI repeatedly"
        );
    }

    #[test]
    fn maps_5050_prompt_alias_and_normalized_params() {
        let event = Event {
            id: "req-5050-prompt".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_102,
            kind: 5050,
            tags: vec![
                vec![
                    "i".to_string(),
                    "Write a haiku about rust".to_string(),
                    "prompt".to_string(),
                ],
                vec!["param".to_string(), "top-k".to_string(), "20".to_string()],
                vec!["param".to_string(), "top_p".to_string(), "0.95".to_string()],
                vec!["bid".to_string(), "5000".to_string()],
            ],
            content: String::new(),
            sig: "12".repeat(64),
        };

        let row = event_to_inbox_request(&event, None).expect("event should map to inbox row");
        assert!(matches!(row.validation, JobInboxValidation::Valid));
        let execution_input = row
            .execution_input
            .as_deref()
            .expect("execution input should be present");
        assert!(execution_input.contains("Prompt:\nWrite a haiku about rust"));
        assert!(execution_input.contains("top_k=20"));
        assert!(execution_input.contains("top_p=0.95"));
        assert!(!execution_input.contains("top-k=20"));
    }

    #[test]
    fn marks_missing_bid_as_pending_validation() {
        let event = Event {
            id: "req-002".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_101,
            kind: 5050,
            tags: vec![vec![
                "i".to_string(),
                "generate summary".to_string(),
                "prompt".to_string(),
            ]],
            content: String::new(),
            sig: "22".repeat(64),
        };

        let row = event_to_inbox_request(&event, None).expect("event should map to inbox row");
        assert_eq!(row.request_id, "req-002");
        assert!(matches!(
            row.validation,
            crate::state::job_inbox::JobInboxValidation::Pending
        ));
    }

    #[test]
    fn marks_text_generation_request_without_prompt_as_invalid() {
        let event = Event {
            id: "req-5050-invalid".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_103,
            kind: 5050,
            tags: vec![
                vec![
                    "i".to_string(),
                    "https://example.com".to_string(),
                    "url".to_string(),
                ],
                vec!["bid".to_string(), "5000".to_string()],
            ],
            content: "   ".to_string(),
            sig: "13".repeat(64),
        };

        let row = event_to_inbox_request(&event, None).expect("event should map to inbox row");
        assert!(matches!(
            row.validation,
            JobInboxValidation::Invalid(ref reason)
                if reason == "text-generation request missing prompt/text input"
        ));
    }

    #[test]
    fn marks_unsupported_output_mime_as_invalid() {
        let event = Event {
            id: "req-5050-output-invalid".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_104,
            kind: 5050,
            tags: vec![
                vec![
                    "i".to_string(),
                    "Write a haiku about rust".to_string(),
                    "prompt".to_string(),
                ],
                vec!["output".to_string(), "application/json".to_string()],
                vec!["bid".to_string(), "5000".to_string()],
            ],
            content: String::new(),
            sig: "14".repeat(64),
        };

        let row = event_to_inbox_request(&event, None).expect("event should map to inbox row");
        assert!(matches!(
            row.validation,
            JobInboxValidation::Invalid(ref reason)
                if reason.contains("unsupported output MIME")
        ));
        assert_eq!(
            row.requested_output_mime.as_deref(),
            Some("application/json")
        );
    }

    #[test]
    fn maps_encrypted_nip90_requests_with_payload_metadata() {
        let event = Event {
            id: "req-enc-001".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_102,
            kind: 5050,
            tags: vec![
                vec!["bid".to_string(), "10000".to_string()],
                vec!["encrypted".to_string()],
                vec!["p".to_string(), "aa".repeat(32)],
            ],
            content: "nip44-ciphertext".to_string(),
            sig: "33".repeat(64),
        };

        let row = event_to_inbox_request(&event, None).expect("event should map to inbox row");
        assert!(row.encrypted);
        assert_eq!(row.encrypted_payload.as_deref(), Some("nip44-ciphertext"));
        assert_eq!(row.target_provider_pubkeys, vec!["aa".repeat(32)]);
    }

    #[test]
    fn maps_tracked_buyer_feedback_event() {
        let event = Event {
            id: "feedback-001".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_130,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "payment-required".to_string()],
                vec!["e".to_string(), "req-001".to_string()],
                vec![
                    "amount".to_string(),
                    "10000".to_string(),
                    "lnbc10n1...".to_string(),
                ],
                vec!["p".to_string(), "22".repeat(32)],
            ],
            content: "pay to continue".to_string(),
            sig: "55".repeat(64),
        };
        let tracked = std::collections::HashSet::from(["req-001".to_string()]);
        let buyer_event = event_to_buyer_response_event(
            &event,
            &tracked,
            None,
            Some("wss://relay.feedback.test/"),
        )
        .expect("feedback should map");
        assert_eq!(buyer_event.kind, ProviderNip90BuyerResponseKind::Feedback);
        assert_eq!(buyer_event.request_id, "req-001");
        assert_eq!(
            buyer_event.relay_url.as_deref(),
            Some("wss://relay.feedback.test/")
        );
        assert_eq!(buyer_event.status.as_deref(), Some("payment-required"));
        assert_eq!(buyer_event.amount_msats, Some(10_000));
        assert_eq!(buyer_event.bolt11.as_deref(), Some("lnbc10n1..."));
    }

    #[test]
    fn maps_tracked_buyer_feedback_event_with_invoice_tag_fallback() {
        let event = Event {
            id: "feedback-invoice-tag".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_130,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "payment-required".to_string()],
                vec!["e".to_string(), "req-001".to_string()],
                vec!["amount".to_string(), "10000".to_string()],
                vec![
                    "invoice".to_string(),
                    "lightning:lnbc10n1invoicefallback".to_string(),
                ],
                vec!["p".to_string(), "22".repeat(32)],
            ],
            content: "pay to continue".to_string(),
            sig: "55".repeat(64),
        };
        let tracked = std::collections::HashSet::from(["req-001".to_string()]);
        let buyer_event = event_to_buyer_response_event(&event, &tracked, None, None)
            .expect("feedback should map");
        assert_eq!(
            buyer_event.bolt11.as_deref(),
            Some("lnbc10n1invoicefallback")
        );
    }

    #[test]
    fn maps_tracked_buyer_feedback_event_with_json_content_fallback() {
        let event = Event {
            id: "feedback-json-content".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_130,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "payment-required".to_string()],
                vec!["e".to_string(), "req-001".to_string()],
                vec!["amount".to_string(), "10000".to_string()],
                vec!["p".to_string(), "22".repeat(32)],
            ],
            content: r#"{"payment_request":"lnbc10n1jsoncontent"}"#.to_string(),
            sig: "55".repeat(64),
        };
        let tracked = std::collections::HashSet::from(["req-001".to_string()]);
        let buyer_event = event_to_buyer_response_event(&event, &tracked, None, None)
            .expect("feedback should map");
        assert_eq!(buyer_event.bolt11.as_deref(), Some("lnbc10n1jsoncontent"));
    }

    #[test]
    fn maps_tracked_buyer_result_event() {
        let event = Event {
            id: "result-001".to_string(),
            pubkey: "33".repeat(32),
            created_at: 1_760_000_131,
            kind: 6050,
            tags: vec![
                vec!["status".to_string(), "success".to_string()],
                vec!["e".to_string(), "req-001".to_string()],
                vec!["p".to_string(), "44".repeat(32)],
                vec![
                    "amount".to_string(),
                    "10000".to_string(),
                    "lnbc10n1...".to_string(),
                ],
            ],
            content: "{\"result\":\"done\"}".to_string(),
            sig: "66".repeat(64),
        };
        let tracked = std::collections::HashSet::from(["req-001".to_string()]);
        let buyer_event =
            event_to_buyer_response_event(&event, &tracked, None, Some("wss://relay.result.test/"))
                .expect("result should map");
        assert_eq!(buyer_event.kind, ProviderNip90BuyerResponseKind::Result);
        assert_eq!(buyer_event.request_id, "req-001");
        assert_eq!(
            buyer_event.relay_url.as_deref(),
            Some("wss://relay.result.test/")
        );
        assert_eq!(buyer_event.status.as_deref(), Some("success"));
        assert_eq!(buyer_event.amount_msats, Some(10_000));
        assert_eq!(buyer_event.bolt11.as_deref(), Some("lnbc10n1..."));
    }

    #[test]
    fn ignores_untracked_buyer_response_event() {
        let event = Event {
            id: "feedback-ignored".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_132,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "processing".to_string()],
                vec!["e".to_string(), "req-unknown".to_string()],
            ],
            content: "processing".to_string(),
            sig: "77".repeat(64),
        };
        let tracked = std::collections::HashSet::from(["req-001".to_string()]);
        assert!(
            event_to_buyer_response_event(&event, &tracked, None, None).is_none(),
            "untracked request ids should not emit buyer response events"
        );
    }

    #[test]
    fn ignores_self_authored_buyer_response_event() {
        let identity = fixture_auth_identity();
        let event = Event {
            id: "feedback-self-authored".to_string(),
            pubkey: identity.public_key_hex.clone(),
            created_at: 1_760_000_133,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "success".to_string()],
                vec!["e".to_string(), "req-001".to_string()],
                vec!["p".to_string(), "33".repeat(32)],
            ],
            content: "resolved".to_string(),
            sig: "88".repeat(64),
        };
        let tracked = std::collections::HashSet::from(["req-001".to_string()]);
        assert!(
            event_to_buyer_response_event(
                &event,
                &tracked,
                Some(identity.public_key_hex.as_str()),
                None,
            )
            .is_none(),
            "self-authored buyer feedback should not be re-ingested as provider activity"
        );
    }

    #[test]
    fn worker_previews_live_relay_request_while_provider_is_offline() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task) = runtime.block_on(spawn_mock_relay_with_request());
        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_gpt_oss_capability(),
            })
            .expect("queue ready capability");

        let deadline = Instant::now() + Duration::from_secs(4);
        let mut ingressed = false;
        let mut preview_snapshot_seen = false;
        let mut last_snapshot: Option<String> = None;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                match update {
                    ProviderNip90LaneUpdate::IngressedRequest(row)
                        if row.request_id == "request-live-1" =>
                    {
                        ingressed = true;
                    }
                    ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                        if snapshot.mode == super::ProviderNip90LaneMode::Preview
                            && snapshot.connected_relays > 0
                        {
                            preview_snapshot_seen = true;
                        }
                        last_snapshot = Some(format!(
                            "mode={} connected_relays={} relays={} last_error={:?} last_action={:?}",
                            snapshot.mode.label(),
                            snapshot.connected_relays,
                            snapshot.relay_health.len(),
                            snapshot.last_error,
                            snapshot.last_action
                        ));
                    }
                    _ => {}
                }
            }
            if ingressed && preview_snapshot_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            preview_snapshot_seen,
            "expected preview snapshot before going online (last snapshot: {})",
            last_snapshot.clone().unwrap_or_else(|| "none".to_string())
        );
        assert!(
            ingressed,
            "expected preview ingress while offline (last snapshot: {})",
            last_snapshot.unwrap_or_else(|| "none".to_string())
        );

        relay_task.abort();
    }

    #[test]
    fn worker_keeps_buyer_only_preview_when_provider_capability_is_unavailable() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task) = runtime.block_on(spawn_mock_relay_with_request());
        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);

        let deadline = Instant::now() + Duration::from_secs(4);
        let mut ingressed = false;
        let mut buyer_transport_snapshot_seen = false;
        let mut last_snapshot: Option<String> = None;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                match update {
                    ProviderNip90LaneUpdate::IngressedRequest(row)
                        if row.request_id == "request-live-1" =>
                    {
                        ingressed = true;
                    }
                    ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                        if snapshot.mode == super::ProviderNip90LaneMode::Preview
                            && snapshot.connected_relays > 0
                            && snapshot
                                .last_action
                                .as_deref()
                                .is_some_and(|action| action.starts_with("Buyer relay transport"))
                        {
                            buyer_transport_snapshot_seen = true;
                        }
                        last_snapshot = Some(format!(
                            "mode={} connected_relays={} relays={} last_error={:?} last_action={:?}",
                            snapshot.mode.label(),
                            snapshot.connected_relays,
                            snapshot.relay_health.len(),
                            snapshot.last_error,
                            snapshot.last_action
                        ));
                    }
                    _ => {}
                }
            }
            if buyer_transport_snapshot_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            buyer_transport_snapshot_seen,
            "expected buyer-only relay preview snapshot (last snapshot: {})",
            last_snapshot.clone().unwrap_or_else(|| "none".to_string())
        );
        assert!(
            !ingressed,
            "provider requests should stay hidden when provider capability is unavailable (last snapshot: {})",
            last_snapshot.unwrap_or_else(|| "none".to_string())
        );

        relay_task.abort();
    }

    #[test]
    fn worker_ingests_live_relay_request() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task) = runtime.block_on(spawn_mock_relay_with_request());
        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_gpt_oss_capability(),
            })
            .expect("queue ready capability");
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let deadline = Instant::now() + Duration::from_secs(4);
        let mut ingressed = false;
        let mut transport_row_seen = false;
        let mut last_snapshot: Option<String> = None;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                match update {
                    ProviderNip90LaneUpdate::IngressedRequest(row)
                        if row.request_id == "request-live-1" =>
                    {
                        assert_eq!(row.demand_source.label(), "open-network");
                        ingressed = true;
                        break;
                    }
                    ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                        if snapshot
                            .relay_health
                            .iter()
                            .any(|relay| relay.status != ProviderNip90RelayStatus::Error)
                        {
                            transport_row_seen = true;
                        }
                        last_snapshot = Some(format!(
                            "mode={} connected_relays={} relays={} last_error={:?} last_action={:?}",
                            snapshot.mode.label(),
                            snapshot.connected_relays,
                            snapshot.relay_health.len(),
                            snapshot.last_error,
                            snapshot.last_action
                        ));
                    }
                    _ => {}
                }
            }
            if ingressed {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            ingressed,
            "expected live NIP-90 ingress from relay (last snapshot: {})",
            last_snapshot.unwrap_or_else(|| "none".to_string())
        );
        assert!(
            transport_row_seen,
            "expected transport relay status row in snapshot"
        );
        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn worker_resubscribes_request_ingress_after_capability_becomes_ready() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task) = runtime.block_on(spawn_mock_relay_with_request());
        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);

        let preview_deadline = Instant::now() + Duration::from_secs(4);
        let mut buyer_transport_preview_seen = false;
        while Instant::now() < preview_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == super::ProviderNip90LaneMode::Preview
                    && snapshot.connected_relays > 0
                    && snapshot
                        .last_action
                        .as_deref()
                        .is_some_and(|action| action.starts_with("Buyer relay transport"))
                {
                    buyer_transport_preview_seen = true;
                    break;
                }
            }
            if buyer_transport_preview_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            buyer_transport_preview_seen,
            "expected initial buyer-only preview before capability becomes ready"
        );

        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_gpt_oss_capability(),
            })
            .expect("queue ready capability");
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let deadline = Instant::now() + Duration::from_secs(8);
        let mut ingressed = false;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::IngressedRequest(row) = update
                    && row.request_id == "request-live-1"
                {
                    ingressed = true;
                    break;
                }
            }
            if ingressed {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            ingressed,
            "expected request ingress after capability transition and resubscribe"
        );

        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn resubscribe_replaces_live_subscription_when_data_vending_profile_becomes_ready() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task) =
            runtime.block_on(spawn_mock_relay_requires_close_before_resubscribe());
        let mut state = fixture_lane_state();
        state.snapshot.configured_relays = vec![relay_url];

        assert!(
            ensure_connected_pool(&runtime, &mut state).is_ok(),
            "expected initial preview pool connection"
        );
        assert_eq!(state.snapshot.connected_relays, 1);

        state.auth_identity = Some(fixture_auth_identity());
        state.data_vending_profile = Some(fixture_data_vending_profile());
        state.wants_online = true;

        let pool = state.pool.as_ref().cloned().expect("pool should exist");
        resubscribe_ingress_filters(&runtime, &mut state, pool.clone());

        let local_pubkey = state
            .auth_identity
            .as_ref()
            .map(|identity| identity.public_key_hex.clone())
            .expect("identity should be present");
        let deadline = Instant::now() + Duration::from_secs(4);
        let mut ingressed = false;
        while Instant::now() < deadline {
            let outcome =
                runtime.block_on(poll_ingress(pool.clone(), &[], Some(local_pubkey.as_str())));
            if outcome
                .requests
                .iter()
                .any(|row| row.request_id == "request-after-close")
            {
                ingressed = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }

        assert!(
            ingressed,
            "expected data-vending request ingress after replacing the live subscription"
        );

        relay_task.abort();
    }

    #[test]
    fn worker_publishes_signed_feedback_event_to_connected_relay() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let online_deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < online_deadline {
            let mut online = false;
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == super::ProviderNip90LaneMode::Online
                    && snapshot.connected_relays > 0
                {
                    online = true;
                }
            }
            if online {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let event = Event {
            id: "feedback-event-1".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_120,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "processing".to_string()],
                vec!["e".to_string(), "request-live-1".to_string()],
                vec!["p".to_string(), "npub1remote-buyer".to_string()],
            ],
            content: "processing".to_string(),
            sig: "22".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: "request-live-1".to_string(),
                role: ProviderNip90PublishRole::Feedback,
                event: Box::new(event.clone()),
            })
            .expect("queue publish command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == "request-live-1"
                    && outcome.role == ProviderNip90PublishRole::Feedback
                {
                    assert_eq!(outcome.event_id, "feedback-event-1");
                    assert!(
                        outcome.accepted_relays >= 1,
                        "expected at least one accepted relay, got {}",
                        outcome.accepted_relays
                    );
                    outcome_seen = true;
                }
            }
            if outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(outcome_seen, "expected publish outcome update");
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive published event");
        assert_eq!(published.id, "feedback-event-1");
        assert_eq!(published.kind, 7000);

        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn worker_publishes_buyer_request_event_while_provider_lane_is_offline() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);

        let preview_deadline = Instant::now() + Duration::from_secs(4);
        let mut preview_ready = false;
        while Instant::now() < preview_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == super::ProviderNip90LaneMode::Preview
                    && snapshot.connected_relays > 0
                {
                    preview_ready = true;
                }
            }
            if preview_ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            preview_ready,
            "relay lane should enter preview while offline"
        );

        let event = Event {
            id: "buyer-request-event-1".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_121,
            kind: 5050,
            tags: vec![
                vec!["relays".to_string(), "wss://relay.example".to_string()],
                vec!["bid".to_string(), "2000".to_string()],
                vec![
                    "param".to_string(),
                    "request_type".to_string(),
                    "smoke-test".to_string(),
                ],
            ],
            content: "buy mode request".to_string(),
            sig: "33".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: "buyer-request-offline-1".to_string(),
                role: ProviderNip90PublishRole::Request,
                event: Box::new(event.clone()),
            })
            .expect("queue request publish command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == "buyer-request-offline-1"
                    && outcome.role == ProviderNip90PublishRole::Request
                {
                    assert_eq!(outcome.event_id, "buyer-request-event-1");
                    assert!(
                        outcome.accepted_relays >= 1,
                        "expected at least one accepted relay, got {}",
                        outcome.accepted_relays
                    );
                    assert_eq!(outcome.first_error, None);
                    outcome_seen = true;
                }
            }
            if outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(outcome_seen, "expected publish outcome update");
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive published request event");
        assert_eq!(published.id, "buyer-request-event-1");
        assert_eq!(published.kind, 5050);

        relay_task.abort();
    }

    #[test]
    fn worker_publishes_buyer_feedback_event_while_provider_lane_is_offline() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::TrackBuyerRequestIds {
                request_ids: vec!["buyer-request-offline-1".to_string()],
            })
            .expect("queue buyer request tracking");

        let preview_deadline = Instant::now() + Duration::from_secs(4);
        let mut preview_ready = false;
        while Instant::now() < preview_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == super::ProviderNip90LaneMode::Preview
                    && snapshot.connected_relays > 0
                    && snapshot
                        .last_action
                        .as_deref()
                        .is_some_and(|action| action.starts_with("Buyer response relay tracking"))
                {
                    preview_ready = true;
                }
            }
            if preview_ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            preview_ready,
            "buyer response relay tracking should be active"
        );

        let event = Event {
            id: "buyer-feedback-event-1".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_122,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "processing".to_string()],
                vec!["e".to_string(), "buyer-request-offline-1".to_string()],
                vec!["p".to_string(), "22".repeat(32)],
            ],
            content: "duplicate provider result ignored".to_string(),
            sig: "44".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: "buyer-request-offline-1".to_string(),
                role: ProviderNip90PublishRole::Feedback,
                event: Box::new(event.clone()),
            })
            .expect("queue buyer feedback publish command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == "buyer-request-offline-1"
                    && outcome.role == ProviderNip90PublishRole::Feedback
                {
                    assert_eq!(outcome.event_id, "buyer-feedback-event-1");
                    assert!(
                        outcome.accepted_relays >= 1,
                        "expected at least one accepted relay, got {}",
                        outcome.accepted_relays
                    );
                    assert_eq!(outcome.first_error, None);
                    outcome_seen = true;
                }
            }
            if outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            outcome_seen,
            "expected buyer feedback publish outcome update"
        );
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive published buyer feedback event");
        assert_eq!(published.id, "buyer-feedback-event-1");
        assert_eq!(published.kind, 7000);

        relay_task.abort();
    }

    #[test]
    fn worker_publishes_provider_feedback_event_while_offline_for_tracked_active_job() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::TrackProviderPublishRequestIds {
                request_ids: vec!["provider-request-offline-1".to_string()],
            })
            .expect("queue provider publish continuity tracking");

        let preview_deadline = Instant::now() + Duration::from_secs(4);
        let mut preview_ready = false;
        while Instant::now() < preview_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == super::ProviderNip90LaneMode::Preview
                    && snapshot.connected_relays > 0
                {
                    preview_ready = true;
                }
            }
            if preview_ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(preview_ready, "preview transport should be active");

        let event = Event {
            id: "provider-feedback-event-1".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_132,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "processing".to_string()],
                vec!["e".to_string(), "provider-request-offline-1".to_string()],
                vec!["p".to_string(), "22".repeat(32)],
            ],
            content: "provider still draining accepted job".to_string(),
            sig: "44".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: "provider-request-offline-1".to_string(),
                role: ProviderNip90PublishRole::Feedback,
                event: Box::new(event.clone()),
            })
            .expect("queue provider feedback publish command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == "provider-request-offline-1"
                    && outcome.role == ProviderNip90PublishRole::Feedback
                {
                    assert_eq!(outcome.event_id, "provider-feedback-event-1");
                    assert!(outcome.accepted_relays >= 1);
                    assert_eq!(outcome.first_error, None);
                    outcome_seen = true;
                }
            }
            if outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(outcome_seen, "expected provider feedback publish outcome");
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive published provider feedback event");
        assert_eq!(published.id, "provider-feedback-event-1");
        assert_eq!(published.kind, 7000);

        relay_task.abort();
    }

    #[test]
    fn worker_publishes_provider_result_event_while_offline_for_tracked_active_job() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::TrackProviderPublishRequestIds {
                request_ids: vec!["provider-request-offline-2".to_string()],
            })
            .expect("queue provider publish continuity tracking");

        let preview_deadline = Instant::now() + Duration::from_secs(4);
        let mut preview_ready = false;
        while Instant::now() < preview_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::Snapshot(snapshot) = update
                    && snapshot.mode == super::ProviderNip90LaneMode::Preview
                    && snapshot.connected_relays > 0
                {
                    preview_ready = true;
                }
            }
            if preview_ready {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(preview_ready, "preview transport should be active");

        let event = Event {
            id: "provider-result-event-1".to_string(),
            pubkey: "11".repeat(32),
            created_at: 1_760_000_133,
            kind: 6050,
            tags: vec![
                vec!["status".to_string(), "success".to_string()],
                vec!["e".to_string(), "provider-request-offline-2".to_string()],
                vec!["p".to_string(), "22".repeat(32)],
            ],
            content: "{\"result\":\"done\"}".to_string(),
            sig: "55".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: "provider-request-offline-2".to_string(),
                role: ProviderNip90PublishRole::Result,
                event: Box::new(event.clone()),
            })
            .expect("queue provider result publish command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == "provider-request-offline-2"
                    && outcome.role == ProviderNip90PublishRole::Result
                {
                    assert_eq!(outcome.event_id, "provider-result-event-1");
                    assert!(outcome.accepted_relays >= 1);
                    assert_eq!(outcome.first_error, None);
                    outcome_seen = true;
                }
            }
            if outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(outcome_seen, "expected provider result publish outcome");
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive published provider result event");
        assert_eq!(published.id, "provider-result-event-1");
        assert_eq!(published.kind, 6050);

        relay_task.abort();
    }

    #[test]
    fn worker_publishes_nip89_handler_info_when_online_with_identity() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(fixture_auth_identity()),
            })
            .expect("queue identity command");
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_gpt_oss_capability(),
            })
            .expect("queue gpt_oss capability command");
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut capability_outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.role == ProviderNip90PublishRole::Capability
                {
                    assert!(outcome.accepted_relays >= 1);
                    capability_outcome_seen = true;
                }
            }
            if capability_outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            capability_outcome_seen,
            "expected capability publish outcome after going online"
        );
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive handler event");
        assert_eq!(published.kind, 31_990);
        assert!(
            published
                .tags
                .iter()
                .any(|tag| tag.first().is_some_and(|value| value == "d")),
            "expected addressable d-tag on handler event"
        );
        assert!(
            published.tags.iter().any(|tag| {
                tag.first().is_some_and(|value| value == "k")
                    && tag.get(1).is_some_and(|value| value == "5050")
            }),
            "expected text generation handler kind tag"
        );
        let metadata: serde_json::Value =
            serde_json::from_str(published.content.as_str()).expect("parse handler metadata");
        assert_eq!(metadata["name"], "Autopilot");
        assert_eq!(metadata["backend"], "gpt_oss");
        assert_eq!(metadata["status"], "healthy");
        assert_eq!(metadata["serving_model"], "llama3.2:latest");

        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn build_provider_handler_event_includes_data_vending_profile_metadata() {
        let mut state = fixture_lane_state();
        state.data_vending_profile = Some(fixture_data_vending_profile());

        let event = super::build_provider_handler_event(
            &fixture_auth_identity(),
            super::HandlerPublicationState::Healthy,
            &state,
        )
        .expect("handler event");
        assert!(
            event.tags.iter().any(|tag| {
                tag.first().is_some_and(|value| value == "k")
                    && tag.get(1).is_some_and(|value| {
                        value.as_str()
                            == crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND
                                .to_string()
                    })
            }),
            "expected data-vending request kind tag"
        );
        let metadata: serde_json::Value =
            serde_json::from_str(event.content.as_str()).expect("parse handler metadata");
        assert_eq!(
            metadata["data_vending"]["profile_id"],
            OPENAGENTS_DATA_VENDING_PROFILE
        );
        assert_eq!(
            metadata["data_vending"]["request_kind"],
            crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND
        );
        assert_eq!(
            metadata["data_vending"]["kind_posture"],
            crate::app_state::OPENAGENTS_DATA_VENDING_KIND_POSTURE
        );
    }

    #[test]
    fn worker_publishes_apple_fm_handler_info_when_online_with_identity() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(fixture_auth_identity()),
            })
            .expect("queue identity command");
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_apple_fm_capability(),
            })
            .expect("queue apple fm capability command");
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut capability_outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.role == ProviderNip90PublishRole::Capability
                {
                    assert!(outcome.accepted_relays >= 1);
                    capability_outcome_seen = true;
                }
            }
            if capability_outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            capability_outcome_seen,
            "expected capability publish outcome after going online"
        );
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive handler event");
        let metadata: serde_json::Value =
            serde_json::from_str(published.content.as_str()).expect("parse handler metadata");
        assert_eq!(metadata["backend"], "apple_foundation_models");
        assert_eq!(metadata["serving_model"], "apple-foundation-model");
        assert_eq!(metadata["status"], "healthy");

        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn worker_publishes_disabled_handler_when_gpt_oss_unhealthy() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_for_publish());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureIdentity {
                identity: Some(fixture_auth_identity()),
            })
            .expect("queue identity command");
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let publish_deadline = Instant::now() + Duration::from_secs(4);
        let mut capability_outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.role == ProviderNip90PublishRole::Capability
                {
                    assert!(outcome.accepted_relays >= 1);
                    capability_outcome_seen = true;
                }
            }
            if capability_outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            capability_outcome_seen,
            "expected capability publish outcome after going online"
        );
        let published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive handler event");
        assert_eq!(published.kind, 31_990);
        assert!(
            !published.tags.iter().any(|tag| {
                tag.first().is_some_and(|value| value == "k")
                    && tag.get(1).is_some_and(|value| value == "5050")
            }),
            "expected disabled handler to omit supported kind tags"
        );
        let metadata: serde_json::Value =
            serde_json::from_str(published.content.as_str()).expect("parse handler metadata");
        assert_eq!(metadata["status"], "degraded");

        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn desktop_earn_harness_relay_execute_publish_wallet_confirm_end_to_end() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build tokio runtime for relay harness");
        let (relay_url, relay_task, published_rx) =
            runtime.block_on(spawn_mock_relay_with_request_and_publish_capture());

        let mut worker = ProviderNip90LaneWorker::spawn(vec![relay_url]);
        worker
            .enqueue(ProviderNip90LaneCommand::ConfigureComputeCapability {
                capability: fixture_gpt_oss_capability(),
            })
            .expect("queue ready capability");
        worker
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let ingest_deadline = Instant::now() + Duration::from_secs(5);
        let mut online = false;
        let mut ingressed_request = None;
        while Instant::now() < ingest_deadline {
            for update in worker.drain_updates() {
                match update {
                    ProviderNip90LaneUpdate::Snapshot(snapshot)
                        if snapshot.mode == super::ProviderNip90LaneMode::Online
                            && snapshot.connected_relays > 0 =>
                    {
                        online = true;
                    }
                    ProviderNip90LaneUpdate::IngressedRequest(request)
                        if request.request_id == "request-live-1" =>
                    {
                        ingressed_request = Some(request);
                    }
                    _ => {}
                }
            }
            if online && ingressed_request.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(online, "relay lane should be online");
        let request = ingressed_request.expect("relay ingress should provide request");

        let mut inbox = JobInboxState::default();
        inbox.upsert_network_request(request.clone());
        assert!(
            inbox.select_by_index(0),
            "ingressed row should be selectable"
        );
        inbox
            .decide_selected(true, "relay request accepted")
            .expect("accept ingressed request");
        let accepted = inbox
            .selected_request()
            .expect("accepted request remains selected")
            .clone();

        let mut active = ActiveJobState::default();
        active.start_from_request(&accepted);
        assert_eq!(
            active
                .advance_stage()
                .expect("accepted->running transition"),
            JobLifecycleStage::Running
        );

        let feedback_event = Event {
            id: "feedback-processing-live-1".to_string(),
            pubkey: "44".repeat(32),
            created_at: 1_760_000_125,
            kind: 7000,
            tags: vec![
                vec!["status".to_string(), "processing".to_string()],
                vec!["e".to_string(), request.request_id.clone()],
                vec!["p".to_string(), request.requester.clone()],
            ],
            content: "processing".to_string(),
            sig: "55".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request.request_id.clone(),
                role: ProviderNip90PublishRole::Feedback,
                event: Box::new(feedback_event.clone()),
            })
            .expect("queue processing feedback publish");

        let result_event = Event {
            id: "result-live-1".to_string(),
            pubkey: "66".repeat(32),
            created_at: 1_760_000_126,
            kind: request.request_kind.saturating_add(1000),
            tags: vec![
                vec!["status".to_string(), "success".to_string()],
                vec!["e".to_string(), request.request_id.clone()],
                vec!["p".to_string(), request.requester.clone()],
                vec![
                    "amount".to_string(),
                    (request.price_sats.saturating_mul(1000)).to_string(),
                ],
            ],
            content: "{\"result\":\"ok\"}".to_string(),
            sig: "77".repeat(64),
        };
        worker
            .enqueue(ProviderNip90LaneCommand::PublishEvent {
                request_id: request.request_id.clone(),
                role: ProviderNip90PublishRole::Result,
                event: Box::new(result_event.clone()),
            })
            .expect("queue result publish");

        let publish_deadline = Instant::now() + Duration::from_secs(5);
        let mut feedback_outcome_seen = false;
        let mut result_outcome_seen = false;
        while Instant::now() < publish_deadline {
            for update in worker.drain_updates() {
                if let ProviderNip90LaneUpdate::PublishOutcome(outcome) = update
                    && outcome.request_id == request.request_id
                {
                    assert!(
                        outcome.accepted_relays >= 1,
                        "expected at least one accepted relay, got {}",
                        outcome.accepted_relays
                    );
                    match outcome.role {
                        ProviderNip90PublishRole::Capability => {}
                        ProviderNip90PublishRole::Request => {}
                        ProviderNip90PublishRole::Feedback => feedback_outcome_seen = true,
                        ProviderNip90PublishRole::Result => result_outcome_seen = true,
                    }
                }
            }
            if feedback_outcome_seen && result_outcome_seen {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            feedback_outcome_seen && result_outcome_seen,
            "expected publish outcomes for feedback and result"
        );

        let first_published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive first published event");
        let second_published = published_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("relay should receive second published event");
        let published_ids = HashSet::from([first_published.id, second_published.id]);
        assert!(published_ids.contains("feedback-processing-live-1"));
        assert!(published_ids.contains("result-live-1"));

        active
            .job
            .as_mut()
            .expect("active job present")
            .sa_tick_result_event_id = Some(result_event.id.clone());
        assert_eq!(
            active
                .advance_stage()
                .expect("running->delivered transition"),
            JobLifecycleStage::Delivered
        );
        active.job.as_mut().expect("active job present").payment_id =
            Some("wallet-payment-e2e-001".to_string());
        assert_eq!(
            active.advance_stage().expect("delivered->paid transition"),
            JobLifecycleStage::Paid
        );

        let terminal_job = active.job.clone().expect("terminal active job");
        let mut history = JobHistoryState::default();
        history.record_from_active_job(&terminal_job, JobHistoryStatus::Succeeded, None);
        assert_eq!(
            history.rows.len(),
            1,
            "history should capture settled receipt"
        );
        assert_eq!(
            history.rows[0].payment_pointer, "wallet-payment-e2e-001",
            "history should retain authoritative wallet pointer"
        );

        let mut wallet = crate::spark_wallet::SparkPaneState::default();
        wallet.balance = Some(Balance {
            spark_sats: 500,
            lightning_sats: 0,
            onchain_sats: 0,
        });
        wallet.recent_payments.push(PaymentSummary {
            id: "wallet-payment-e2e-001".to_string(),
            direction: "receive".to_string(),
            status: "succeeded".to_string(),
            amount_sats: request.price_sats,
            timestamp: history.reference_epoch_seconds,
            ..Default::default()
        });

        let mut provider = ProviderRuntimeState::default();
        provider.mode = ProviderMode::Online;
        provider.online_since = Some(Instant::now());

        let mut scoreboard = EarningsScoreboardState::default();
        scoreboard.refresh_from_sources(Instant::now(), &provider, &history, &wallet);
        assert_eq!(scoreboard.jobs_today, 1);
        assert_eq!(scoreboard.sats_today, request.price_sats);
        assert_eq!(scoreboard.lifetime_sats, request.price_sats);

        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    #[test]
    fn execution_input_from_request_preserves_content_inputs_and_normalizes_text_generation_params()
    {
        let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
            .expect("request kind should be valid")
            .add_input(JobInput::text("Attachment text"))
            .add_param("temperature", "0.1")
            .add_param("top-k", "32")
            .add_param("top_p", "0.85")
            .with_output("text/plain");
        let mut request = request;
        request.content = "Summarize the attachment.".to_string();

        let execution_input =
            execution_input_from_request(&request).expect("execution input should be captured");
        assert!(execution_input.contains("Prompt:\nSummarize the attachment.\n\nAttachment text"));
        assert!(execution_input.contains("temperature=0.1"));
        assert!(execution_input.contains("top_k=32"));
        assert!(execution_input.contains("top_p=0.85"));
        assert!(!execution_input.contains("top-k=32"));
        assert!(execution_input.contains("Requested output: text/plain"));
    }

    async fn spawn_mock_relay_with_request() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock relay listener");
        let addr = listener.local_addr().expect("resolve listener addr");

        let handle = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut ws = accept_async(stream)
                        .await
                        .expect("upgrade websocket connection");

                    loop {
                        let Some(message) = ws.next().await else {
                            break;
                        };
                        let Ok(message) = message else {
                            break;
                        };
                        let Message::Text(text) = message else {
                            continue;
                        };
                        let value: Value =
                            serde_json::from_str(text.as_ref()).expect("parse relay frame");
                        let Some(frame) = value.as_array() else {
                            continue;
                        };
                        let Some(kind) = frame.first().and_then(Value::as_str) else {
                            continue;
                        };
                        if kind != "REQ" {
                            continue;
                        }

                        let subscription_id = frame[1].as_str().expect("REQ subscription id");
                        let request = Event {
                            id: "request-live-1".to_string(),
                            pubkey: "npub1remote-buyer".to_string(),
                            created_at: 1_760_000_110,
                            kind: 5050,
                            tags: vec![
                                vec!["bid".to_string(), "50000".to_string()],
                                vec!["param".to_string(), "ttl".to_string(), "45".to_string()],
                            ],
                            content: "Generate a short summary".to_string(),
                            sig: "33".repeat(64),
                        };
                        let payload = serde_json::json!(["EVENT", subscription_id, request]);
                        ws.send(Message::Text(payload.to_string().into()))
                            .await
                            .expect("send request event");
                    }
                });
            }
        });

        (format!("ws://{}", addr), handle)
    }

    async fn spawn_mock_relay_for_publish()
    -> (String, JoinHandle<()>, std::sync::mpsc::Receiver<Event>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock relay listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let (published_tx, published_rx) = std::sync::mpsc::channel::<Event>();

        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept websocket client");
            let mut ws = accept_async(stream)
                .await
                .expect("upgrade websocket connection");

            loop {
                let Some(message) = ws.next().await else {
                    break;
                };
                let Ok(message) = message else {
                    break;
                };
                let Message::Text(text) = message else {
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
                        let subscription_id = frame[1].as_str().expect("REQ subscription id");
                        let payload = serde_json::json!(["EOSE", subscription_id]);
                        ws.send(Message::Text(payload.to_string().into()))
                            .await
                            .expect("send eose");
                    }
                    "EVENT" => {
                        let event: Event = serde_json::from_value(frame[1].clone())
                            .expect("parse published event");
                        let _ = published_tx.send(event.clone());
                        let ok = serde_json::json!(["OK", event.id, true, "accepted"]);
                        ws.send(Message::Text(ok.to_string().into()))
                            .await
                            .expect("send ok");
                    }
                    _ => {}
                }
            }
        });

        (format!("ws://{}", addr), handle, published_rx)
    }

    async fn spawn_mock_relay_with_request_and_publish_capture()
    -> (String, JoinHandle<()>, std::sync::mpsc::Receiver<Event>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock relay listener");
        let addr = listener.local_addr().expect("resolve listener addr");
        let (published_tx, published_rx) = std::sync::mpsc::channel::<Event>();

        let handle = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    break;
                };
                let published_tx = published_tx.clone();
                tokio::spawn(async move {
                    let mut ws = accept_async(stream)
                        .await
                        .expect("upgrade websocket connection");

                    loop {
                        let Some(message) = ws.next().await else {
                            break;
                        };
                        let Ok(message) = message else {
                            break;
                        };
                        let Message::Text(text) = message else {
                            continue;
                        };
                        let value: Value =
                            serde_json::from_str(text.as_ref()).expect("parse relay frame");
                        let Some(frame) = value.as_array() else {
                            continue;
                        };
                        let Some(kind) = frame.first().and_then(Value::as_str) else {
                            continue;
                        };
                        match kind {
                            "REQ" => {
                                let subscription_id =
                                    frame[1].as_str().expect("REQ subscription id");
                                let request = Event {
                                    id: "request-live-1".to_string(),
                                    pubkey: "npub1remote-buyer".to_string(),
                                    created_at: 1_760_000_110,
                                    kind: 5050,
                                    tags: vec![
                                        vec!["bid".to_string(), "50000".to_string()],
                                        vec![
                                            "param".to_string(),
                                            "ttl".to_string(),
                                            "45".to_string(),
                                        ],
                                    ],
                                    content: "Generate a short summary".to_string(),
                                    sig: "33".repeat(64),
                                };
                                let event_payload =
                                    serde_json::json!(["EVENT", subscription_id, request]);
                                ws.send(Message::Text(event_payload.to_string().into()))
                                    .await
                                    .expect("send request event");
                                let eose_payload = serde_json::json!(["EOSE", subscription_id]);
                                ws.send(Message::Text(eose_payload.to_string().into()))
                                    .await
                                    .expect("send eose");
                            }
                            "EVENT" => {
                                let event: Event = serde_json::from_value(frame[1].clone())
                                    .expect("parse published event");
                                let _ = published_tx.send(event.clone());
                                let ok = serde_json::json!(["OK", event.id, true, "accepted"]);
                                ws.send(Message::Text(ok.to_string().into()))
                                    .await
                                    .expect("send ok");
                            }
                            _ => {}
                        }
                    }
                });
            }
        });

        (format!("ws://{}", addr), handle, published_rx)
    }

    async fn spawn_mock_relay_requires_close_before_resubscribe() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock relay listener");
        let addr = listener.local_addr().expect("resolve listener addr");

        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept websocket client");
            let mut ws = accept_async(stream)
                .await
                .expect("upgrade websocket connection");

            let mut saw_close = false;
            loop {
                let Some(message) = ws.next().await else {
                    break;
                };
                let Ok(message) = message else {
                    break;
                };
                let Message::Text(text) = message else {
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
                        let subscription_id = frame[1].as_str().expect("REQ subscription id");
                        if saw_close {
                            let request = Event {
                                id: "request-after-close".to_string(),
                                pubkey: "npub1remote-buyer".to_string(),
                                created_at: 1_760_000_210,
                                kind: crate::app_state::OPENAGENTS_DATA_VENDING_LOCAL_REQUEST_KIND,
                                tags: vec![
                                    vec!["bid".to_string(), "1000".to_string()],
                                    vec!["p".to_string(), fixture_auth_identity().public_key_hex],
                                    vec![
                                        "param".to_string(),
                                        "oa_asset_ref".to_string(),
                                        "asset:test".to_string(),
                                    ],
                                    vec![
                                        "param".to_string(),
                                        "oa_scope".to_string(),
                                        "read".to_string(),
                                    ],
                                    vec![
                                        "param".to_string(),
                                        "oa_delivery_mode".to_string(),
                                        "bundle_ref".to_string(),
                                    ],
                                    vec![
                                        "param".to_string(),
                                        "oa_preview_posture".to_string(),
                                        "none".to_string(),
                                    ],
                                ],
                                content: "deliver asset:test".to_string(),
                                sig: "44".repeat(64),
                            };
                            let payload = serde_json::json!(["EVENT", subscription_id, request]);
                            ws.send(Message::Text(payload.to_string().into()))
                                .await
                                .expect("send request event after close");
                        }
                        let eose_payload = serde_json::json!(["EOSE", subscription_id]);
                        ws.send(Message::Text(eose_payload.to_string().into()))
                            .await
                            .expect("send eose");
                    }
                    "CLOSE" => {
                        saw_close = true;
                    }
                    _ => {}
                }
            }
        });

        (format!("ws://{}", addr), handle)
    }
}
