use crate::state::job_inbox::{JobInboxNetworkRequest, JobInboxValidation};
use nostr::Event;
use nostr::nip90::{
    JOB_REQUEST_KIND_MAX, JOB_REQUEST_KIND_MIN, JobRequest, KIND_JOB_CODE_REVIEW,
    KIND_JOB_IMAGE_GENERATION, KIND_JOB_PATCH_GEN, KIND_JOB_REPO_INDEX, KIND_JOB_RLM_SUBQUERY,
    KIND_JOB_SANDBOX_RUN, KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION,
    KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION, is_job_request_kind,
};
use nostr_client::{ConnectionState, PoolConfig, RelayMessage, RelayPool};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

const LANE_POLL: Duration = Duration::from_millis(120);
const RELAY_RECV_TIMEOUT: Duration = Duration::from_millis(4);
const MAX_MESSAGES_PER_RELAY_POLL: usize = 6;
const SUBSCRIPTION_ID: &str = "autopilot-provider-nip90-ingress";
const DEFAULT_TTL_SECONDS: u64 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNip90LaneMode {
    Offline,
    Connecting,
    Online,
    Degraded,
}

impl ProviderNip90LaneMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Online => "online",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProviderNip90LaneSnapshot {
    pub mode: ProviderNip90LaneMode,
    pub configured_relays: Vec<String>,
    pub connected_relays: usize,
    pub last_request_event_id: Option<String>,
    pub last_request_at: Option<Instant>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
}

impl ProviderNip90LaneSnapshot {
    pub fn with_relays(relays: Vec<String>) -> Self {
        Self {
            configured_relays: normalize_relays(relays),
            ..Self::default()
        }
    }
}

impl Default for ProviderNip90LaneSnapshot {
    fn default() -> Self {
        Self {
            mode: ProviderNip90LaneMode::Offline,
            configured_relays: Vec::new(),
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
    ConfigureRelays { relays: Vec<String> },
    SetOnline { online: bool },
}

#[derive(Clone, Debug)]
pub enum ProviderNip90LaneUpdate {
    Snapshot(Box<ProviderNip90LaneSnapshot>),
    IngressedRequest(JobInboxNetworkRequest),
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
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

struct ProviderNip90LaneState {
    snapshot: ProviderNip90LaneSnapshot,
    wants_online: bool,
    pool: Option<Arc<RelayPool>>,
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
            let snapshot = ProviderNip90LaneSnapshot {
                mode: ProviderNip90LaneMode::Degraded,
                configured_relays: normalize_relays(initial_relays),
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
    };

    let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
        state.snapshot.clone(),
    )));

    loop {
        match command_rx.recv_timeout(LANE_POLL) {
            Ok(command) => {
                handle_command(&runtime, &mut state, command);
                let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                    state.snapshot.clone(),
                )));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let mode_before = state.snapshot.mode;
        let connected_before = state.snapshot.connected_relays;
        if ensure_online_pool(&runtime, &mut state).is_err() {
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
            continue;
        }
        if state.snapshot.mode != mode_before || state.snapshot.connected_relays != connected_before
        {
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }

        if !state.wants_online {
            continue;
        }

        let Some(pool) = state.pool.as_ref().cloned() else {
            continue;
        };

        let outcome = runtime.block_on(poll_ingress(pool));
        if state.snapshot.connected_relays != outcome.connected_relays {
            state.snapshot.connected_relays = outcome.connected_relays;
            if outcome.connected_relays == 0 {
                state.snapshot.mode = ProviderNip90LaneMode::Degraded;
                state.snapshot.last_error =
                    Some("Provider ingress has zero connected relays while online".to_string());
            } else if state.snapshot.mode != ProviderNip90LaneMode::Online {
                state.snapshot.mode = ProviderNip90LaneMode::Online;
                state.snapshot.last_error = None;
            }
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }

        if let Some(error) = outcome.last_error {
            state.snapshot.mode = ProviderNip90LaneMode::Degraded;
            state.snapshot.last_error = Some(error);
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }

        for request in outcome.requests {
            state.snapshot.last_request_event_id = Some(request.request_id.clone());
            state.snapshot.last_request_at = Some(Instant::now());
            state.snapshot.last_error = None;
            state.snapshot.last_action = Some(format!(
                "ingressed live NIP-90 request {} from relays",
                request.request_id
            ));
            let _ = update_tx.send(ProviderNip90LaneUpdate::IngressedRequest(request));
            let _ = update_tx.send(ProviderNip90LaneUpdate::Snapshot(Box::new(
                state.snapshot.clone(),
            )));
        }
    }
}

fn handle_command(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
    command: ProviderNip90LaneCommand,
) {
    match command {
        ProviderNip90LaneCommand::ConfigureRelays { relays } => {
            let normalized = normalize_relays(relays);
            if normalized == state.snapshot.configured_relays {
                return;
            }

            state.snapshot.configured_relays = normalized;
            state.snapshot.connected_relays = 0;
            state.snapshot.last_action = Some("Updated provider relay configuration".to_string());

            if state.wants_online {
                disconnect_pool(runtime, state);
            }
        }
        ProviderNip90LaneCommand::SetOnline { online } => {
            state.wants_online = online;
            if online {
                state.snapshot.mode = ProviderNip90LaneMode::Connecting;
                state.snapshot.last_action = Some("Connecting provider relay ingress".to_string());
            } else {
                disconnect_pool(runtime, state);
                state.snapshot.mode = ProviderNip90LaneMode::Offline;
                state.snapshot.last_error = None;
                state.snapshot.last_action = Some("Provider relay ingress offline".to_string());
            }
        }
    }
}

fn disconnect_pool(runtime: &tokio::runtime::Runtime, state: &mut ProviderNip90LaneState) {
    if let Some(pool) = state.pool.take() {
        let _ = runtime.block_on(pool.disconnect_all());
    }
    state.snapshot.connected_relays = 0;
}

fn ensure_online_pool(
    runtime: &tokio::runtime::Runtime,
    state: &mut ProviderNip90LaneState,
) -> Result<(), ()> {
    if !state.wants_online || state.pool.is_some() {
        return Ok(());
    }

    if state.snapshot.configured_relays.is_empty() {
        state.snapshot.mode = ProviderNip90LaneMode::Degraded;
        state.snapshot.last_error =
            Some("No relay URLs configured for provider ingress".to_string());
        state.snapshot.last_action = Some("Provider ingress failed: missing relays".to_string());
        return Err(());
    }

    let mut connect_error: Option<String> = None;
    let pool = runtime.block_on(async {
        let pool = Arc::new(RelayPool::new(PoolConfig::default()));
        for relay in &state.snapshot.configured_relays {
            if let Err(error) = pool.add_relay(relay.as_str()).await {
                connect_error = Some(format!("Failed adding relay {relay}: {error}"));
                return None;
            }
        }
        if let Err(error) = pool.connect_all().await {
            connect_error = Some(format!("Failed connecting relays: {error}"));
            return None;
        }

        let kinds = (JOB_REQUEST_KIND_MIN..=JOB_REQUEST_KIND_MAX)
            .map(serde_json::Value::from)
            .collect::<Vec<_>>();
        let filters = vec![json!({"kinds": kinds, "limit": 256})];
        if let Err(error) = pool.subscribe_filters(SUBSCRIPTION_ID, filters).await {
            connect_error = Some(format!(
                "Failed subscribing provider ingress filters: {error}"
            ));
            let _ = pool.disconnect_all().await;
            return None;
        }
        Some(pool)
    });

    if let Some(pool) = pool {
        state.snapshot.connected_relays = runtime.block_on(connected_relay_count(&pool));
        state.pool = Some(pool);
        state.snapshot.mode = ProviderNip90LaneMode::Online;
        state.snapshot.last_error = None;
        state.snapshot.last_action = Some("Provider relay ingress online".to_string());
        return Ok(());
    }

    state.snapshot.mode = ProviderNip90LaneMode::Degraded;
    state.snapshot.last_error = connect_error;
    state.snapshot.last_action = Some("Provider relay ingress failed to connect".to_string());
    Err(())
}

async fn connected_relay_count(pool: &RelayPool) -> usize {
    let relays = pool.relays().await;
    let mut connected = 0usize;
    for relay in relays {
        if relay.state().await == ConnectionState::Connected {
            connected = connected.saturating_add(1);
        }
    }
    connected
}

struct PollOutcome {
    requests: Vec<JobInboxNetworkRequest>,
    connected_relays: usize,
    last_error: Option<String>,
}

async fn poll_ingress(pool: Arc<RelayPool>) -> PollOutcome {
    let relays = pool.relays().await;
    let mut requests = Vec::new();
    let mut connected_relays = 0usize;
    let mut last_error = None;

    for relay in relays {
        if relay.state().await == ConnectionState::Connected {
            connected_relays = connected_relays.saturating_add(1);
        }

        for _ in 0..MAX_MESSAGES_PER_RELAY_POLL {
            match tokio::time::timeout(RELAY_RECV_TIMEOUT, relay.recv()).await {
                Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                    if let Some(request) = event_to_inbox_request(&event) {
                        requests.push(request);
                    }
                }
                Ok(Ok(Some(_))) => continue,
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    last_error = Some(format!("relay recv failed: {error}"));
                    break;
                }
                Err(_) => break,
            }
        }
    }

    PollOutcome {
        requests,
        connected_relays,
        last_error,
    }
}

fn event_to_inbox_request(event: &Event) -> Option<JobInboxNetworkRequest> {
    if !is_job_request_kind(event.kind) {
        return None;
    }

    let parsed = JobRequest::from_event(event);
    let (skill_scope_id, price_sats, ttl_seconds, validation) = match parsed.as_ref() {
        Ok(request) => {
            let bid_msats = request.bid.unwrap_or(0);
            let price_sats = msats_to_sats_ceil(bid_msats);
            let ttl_seconds = extract_ttl_seconds(request).unwrap_or(DEFAULT_TTL_SECONDS);
            let skill_scope_id = extract_param(request, "skill_scope_id")
                .or_else(|| extract_param(request, "skill_scope"));
            let validation = if request.content.trim().is_empty() && request.inputs.is_empty() {
                JobInboxValidation::Invalid("request missing content/input payload".to_string())
            } else if request.bid.is_none() || price_sats == 0 {
                JobInboxValidation::Pending
            } else {
                JobInboxValidation::Valid
            };
            (skill_scope_id, price_sats, ttl_seconds, validation)
        }
        Err(error) => (
            None,
            0,
            DEFAULT_TTL_SECONDS,
            JobInboxValidation::Invalid(format!("invalid NIP-90 request tags: {error}")),
        ),
    };

    Some(JobInboxNetworkRequest {
        request_id: event.id.clone(),
        requester: event.pubkey.clone(),
        capability: capability_for_kind(event.kind),
        skill_scope_id,
        skl_manifest_a: None,
        skl_manifest_event_id: None,
        sa_tick_request_event_id: None,
        sa_tick_result_event_id: None,
        ac_envelope_event_id: None,
        price_sats,
        ttl_seconds,
        validation,
    })
}

fn extract_param(request: &JobRequest, key: &str) -> Option<String> {
    request
        .params
        .iter()
        .find(|param| param.key.eq_ignore_ascii_case(key))
        .map(|param| param.value.clone())
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
        ProviderNip90LaneCommand, ProviderNip90LaneUpdate, ProviderNip90LaneWorker,
        event_to_inbox_request,
    };
    use futures_util::{SinkExt, StreamExt};
    use nostr::Event;
    use serde_json::Value;
    use std::time::{Duration, Instant};
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

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
                vec![
                    "param".to_string(),
                    "skill_scope_id".to_string(),
                    "33400:npub1agent:summarize-text:0.1.0".to_string(),
                ],
            ],
            content: "generate summary".to_string(),
            sig: "11".repeat(64),
        };

        let row = event_to_inbox_request(&event).expect("event should map to inbox row");
        assert_eq!(row.request_id, "req-001");
        assert_eq!(row.requester, "npub1buyer");
        assert_eq!(row.capability, "text.generation");
        assert_eq!(row.price_sats, 25);
        assert_eq!(row.ttl_seconds, 90);
        assert_eq!(
            row.skill_scope_id,
            Some("33400:npub1agent:summarize-text:0.1.0".to_string())
        );
    }

    #[test]
    fn marks_missing_bid_as_pending_validation() {
        let event = Event {
            id: "req-002".to_string(),
            pubkey: "npub1buyer".to_string(),
            created_at: 1_760_000_101,
            kind: 5930,
            tags: vec![],
            content: "echo hi".to_string(),
            sig: "22".repeat(64),
        };

        let row = event_to_inbox_request(&event).expect("event should map to inbox row");
        assert_eq!(row.request_id, "req-002");
        assert!(matches!(
            row.validation,
            crate::state::job_inbox::JobInboxValidation::Pending
        ));
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
            .enqueue(ProviderNip90LaneCommand::SetOnline { online: true })
            .expect("queue online command");

        let deadline = Instant::now() + Duration::from_secs(4);
        let mut ingressed = false;
        let mut last_snapshot: Option<String> = None;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                match update {
                    ProviderNip90LaneUpdate::IngressedRequest(row)
                        if row.request_id == "request-live-1" =>
                    {
                        ingressed = true;
                        break;
                    }
                    ProviderNip90LaneUpdate::Snapshot(snapshot) => {
                        last_snapshot = Some(format!(
                            "mode={} connected_relays={} last_error={:?} last_action={:?}",
                            snapshot.mode.label(),
                            snapshot.connected_relays,
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
        let _ = worker.enqueue(ProviderNip90LaneCommand::SetOnline { online: false });
        relay_task.abort();
    }

    async fn spawn_mock_relay_with_request() -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock relay listener");
        let addr = listener.local_addr().expect("resolve listener addr");

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

        (format!("ws://{}", addr), handle)
    }
}
