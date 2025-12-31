use crate::agent::{Agent, AgentContext, AgentState};
use async_trait::async_trait;
use crate::budget::BudgetPolicy;
use crate::compute::{
    ComputeChunk, ComputeError, ComputeFs, ComputeKind, ComputePolicy, ComputeProvider,
    ComputeRequest, ComputeResponse, ComputeRouter, DvmProvider, JobState, ModelInfo, ProviderInfo,
    ProviderLatency, ProviderStatus,
};
use crate::claude::{
    ClaudeChunk, ClaudeError, ClaudeFs, ClaudePolicy, ClaudeProvider, ClaudeProviderInfo,
    ClaudeProviderStatus, ClaudeRequest, ClaudeResponse, ClaudeRouter, ClaudeSessionAutonomy,
    ClaudeSessionStatus, ClaudeUsage, ClaudeCapabilities, ClaudeModelInfo, ClaudePricing,
    SessionState as ClaudeSessionState, ToolDefinition, ToolLogEntry,
};
use crate::containers::{
    ApiAuthState, AuthMethod, ContainerCapabilities, ContainerError, ContainerFs, ContainerKind,
    ContainerPolicy, ContainerProvider, ContainerProviderInfo, ContainerRequest, ContainerResponse,
    ContainerRouter, ContainerUsage, DvmContainerProvider, ExecState, OpenAgentsApiClient,
    OpenAgentsAuth, OutputChunk, OutputStream, RepoConfig, SessionState,
};
use crate::dvm::DvmTransport;
use crate::engine::{manual_trigger, TickEngine};
use crate::fx::{FxRateSnapshot, FxSource};
use crate::fs::{AccessLevel, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat};
use crate::idempotency::{IdempotencyJournal, MemoryJournal};
use crate::wallet::{WalletError, WalletPayment, WalletService};
use compute::domain::sandbox_run::{CommandResult as SandboxCommandResult, SandboxRunResult};
use crate::storage::{AgentStorage, InMemoryStorage, StoredState};
use crate::types::{AgentId, EnvelopeId, Timestamp};
use crate::{
    AgentEnv, ControlPlane, Envelope, HudFs, HudSettings, LocalRuntime, LogsFs, MetricsFs,
    NostrSigner, Result, RoutedEnvelope, TickResult, TraceEvent, WatchEvent,
};
use crate::identity::SigningService;
use nostr::{
    Event, JobResult, ENCRYPTED_DM_KIND, KIND_JOB_FEEDBACK, KIND_JOB_TEXT_GENERATION,
    KIND_SHORT_TEXT_NOTE,
};
use nostr::nip90::KIND_JOB_SANDBOX_RUN;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Default, Serialize, Deserialize)]
struct DedupState {
    seen_count: u32,
    duplicate_count: u32,
}

impl AgentState for DedupState {}

struct DedupAgent;

impl Agent for DedupAgent {
    type State = DedupState;
    type Config = ();

    fn on_trigger(
        &self,
        ctx: &mut AgentContext<Self::State>,
        trigger: crate::Trigger,
    ) -> Result<TickResult> {
        let envelope_id = trigger.envelope_id().clone();
        if ctx.seen(&envelope_id) {
            ctx.state.duplicate_count += 1;
        } else {
            ctx.mark_seen(envelope_id);
            ctx.state.seen_count += 1;
        }
        Ok(TickResult::success())
    }
}

#[derive(Default, Serialize, Deserialize)]
struct CountState {
    count: u32,
}

impl AgentState for CountState {}

struct CountingAgent;

impl Agent for CountingAgent {
    type State = CountState;
    type Config = ();

    fn on_trigger(
        &self,
        ctx: &mut AgentContext<Self::State>,
        _trigger: crate::Trigger,
    ) -> Result<TickResult> {
        ctx.state.count += 1;
        Ok(TickResult::success())
    }
}

struct SlowAgent {
    active: Arc<AtomicUsize>,
    overlap: Arc<AtomicBool>,
    delay: Duration,
}

impl Agent for SlowAgent {
    type State = ();
    type Config = ();

    fn on_trigger(
        &self,
        _ctx: &mut AgentContext<Self::State>,
        _trigger: crate::Trigger,
    ) -> Result<TickResult> {
        let current = self.active.fetch_add(1, Ordering::SeqCst);
        if current > 0 {
            self.overlap.store(true, Ordering::SeqCst);
        }

        std::thread::sleep(self.delay);

        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(TickResult::success())
    }
}

async fn load_state<S: AgentState>(storage: &dyn AgentStorage, agent_id: &AgentId) -> S {
    let bytes = storage
        .load_state(agent_id)
        .await
        .expect("load_state failed")
        .expect("state missing");
    StoredState::decode::<S>(&bytes).expect("decode state")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_single_tick_lock() {
    let storage = Arc::new(InMemoryStorage::new());
    let engine = Arc::new(TickEngine::new(storage));
    let active = Arc::new(AtomicUsize::new(0));
    let overlap = Arc::new(AtomicBool::new(false));
    let agent = Arc::new(SlowAgent {
        active: active.clone(),
        overlap: overlap.clone(),
        delay: Duration::from_millis(150),
    });
    let agent_id = AgentId::from("agent-lock-test");
    let barrier = Arc::new(tokio::sync::Barrier::new(2));

    let run_tick = |env_id: &str| {
        let engine = engine.clone();
        let agent = agent.clone();
        let agent_id = agent_id.clone();
        let barrier = barrier.clone();
        let envelope_id = EnvelopeId::from(env_id);
        async move {
            barrier.wait().await;
            engine
                .tick(
                    agent_id,
                    agent.as_ref(),
                    manual_trigger(envelope_id, "test"),
                )
                .await
        }
    };

    let (first, second) = tokio::join!(run_tick("env-1"), run_tick("env-2"));
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert!(!overlap.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_dedup_helpers() {
    let storage = Arc::new(InMemoryStorage::new());
    let engine = TickEngine::new(storage.clone());
    let agent = DedupAgent;
    let agent_id = AgentId::from("agent-dedup-test");
    let envelope_id = EnvelopeId::from("env-dedup");

    engine
        .tick(
            agent_id.clone(),
            &agent,
            manual_trigger(envelope_id.clone(), "test"),
        )
        .await
        .expect("first tick failed");

    engine
        .tick(
            agent_id.clone(),
            &agent,
            manual_trigger(envelope_id, "test"),
        )
        .await
        .expect("second tick failed");

    let state: DedupState = load_state(storage.as_ref(), &agent_id).await;
    assert_eq!(state.seen_count, 1);
    assert_eq!(state.duplicate_count, 1);
}

#[cfg(feature = "local")]
#[tokio::test]
async fn test_hibernation_state_persists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("runtime.db");
    let agent_id = AgentId::from("agent-hibernate-test");
    let envelope_one = EnvelopeId::from("env-1");
    let envelope_two = EnvelopeId::from("env-2");

    {
        let storage = Arc::new(crate::storage::SqliteStorage::new(&db_path).expect("storage"));
        let engine = TickEngine::new(storage);
        let agent = CountingAgent;
        engine
            .tick(
                agent_id.clone(),
                &agent,
                manual_trigger(envelope_one, "test"),
            )
            .await
            .expect("first tick failed");
    }

    {
        let storage = Arc::new(crate::storage::SqliteStorage::new(&db_path).expect("storage"));
        let engine = TickEngine::new(storage.clone());
        let agent = CountingAgent;
        engine
            .tick(
                agent_id.clone(),
                &agent,
                manual_trigger(envelope_two, "test"),
            )
            .await
            .expect("second tick failed");

        let state: CountState = load_state(storage.as_ref(), &agent_id).await;
        assert_eq!(state.count, 2);
    }
}

#[test]
fn test_env_status_read() {
    let storage = Arc::new(InMemoryStorage::new());
    let agent_id = AgentId::from("agent-env-status");
    let env = AgentEnv::new(agent_id.clone(), storage);

    let data = env.read("/status").expect("status read");
    let value: serde_json::Value = serde_json::from_slice(&data).expect("status json");
    assert_eq!(value["agent_id"], agent_id.as_str());
}

#[test]
fn test_env_inbox_write() {
    let storage = Arc::new(InMemoryStorage::new());
    let agent_id = AgentId::from("agent-env-inbox");
    let env = AgentEnv::new(agent_id, storage);
    let envelope = Envelope {
        id: EnvelopeId::from("env-1"),
        timestamp: Timestamp::now(),
        payload: serde_json::json!({ "message": "hello" }),
    };

    let data = serde_json::to_vec(&envelope).expect("envelope json");
    env.write("/inbox", &data).expect("write inbox");

    let inbox_bytes = env.read("/inbox").expect("read inbox");
    let items: Vec<Envelope> = serde_json::from_slice(&inbox_bytes).expect("inbox json");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id.as_str(), "env-1");
}

#[test]
fn test_env_logs_watch() {
    let storage = Arc::new(InMemoryStorage::new());
    let agent_id = AgentId::from("agent-env-logs");
    let env = AgentEnv::new(agent_id, storage);
    let logs = env.logs.clone();
    let mut handle = env
        .watch("/logs/trace")
        .expect("watch logs")
        .expect("trace handle");

    std::thread::spawn(move || {
        logs.emit_trace("trace-event");
    });

    let event = handle
        .next(Some(Duration::from_secs(1)))
        .expect("watch next");

    match event {
        Some(WatchEvent::Data(data)) => assert_eq!(data, b"trace-event"),
        other => panic!("unexpected watch event: {:?}", other),
    }
}

#[test]
fn test_budget_enforcement() {
    let storage = Arc::new(InMemoryStorage::new());
    let agent_id = AgentId::from("agent-budget");
    let mut env = AgentEnv::new(agent_id, storage);
    let service = Arc::new(BufferService::new());
    let policy = BudgetPolicy {
        per_tick_usd: 1,
        per_day_usd: 2,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    env.mount("/budget", service, AccessLevel::Budgeted(policy));

    env.write("/budget", b"first").expect("first write");
    let err = env.write("/budget", b"second").expect_err("budget exceeded");
    assert!(matches!(err, FsError::BudgetExceeded));
}

#[test]
fn test_idempotent_effects() {
    let journal = MemoryJournal::new();
    let ttl = Duration::from_millis(50);
    assert!(journal.get("key").expect("get").is_none());
    journal
        .put_with_ttl("key", b"payload", ttl)
        .expect("put");
    assert_eq!(
        journal.get("key").expect("get"),
        Some(b"payload".to_vec())
    );
    std::thread::sleep(Duration::from_millis(60));
    assert!(journal.get("key").expect("expired").is_none());
}

#[test]
fn test_compute_new_usage_and_idempotency() {
    let mut router = ComputeRouter::new();
    router.register(Arc::new(TestProvider::new()));
    let policy = ComputePolicy {
        require_idempotency: true,
        require_max_cost: true,
        ..ComputePolicy::default()
    };
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let compute = ComputeFs::new(
        AgentId::from("agent-compute"),
        router,
        policy,
        budget,
        journal,
    );

    let request = ComputeRequest {
        model: "test-model".to_string(),
        kind: ComputeKind::Complete,
        input: json!({ "prompt": "hello" }),
        stream: false,
        timeout_ms: None,
        idempotency_key: Some("req-1".to_string()),
        max_cost_usd: Some(5),
    };

    let response = submit_compute(&compute, &request);
    let job_id = response["job_id"].as_str().expect("job_id").to_string();

    let result_bytes =
        read_handle(compute.open(&format!("jobs/{}/result", job_id), OpenFlags::read()).unwrap());
    let result: ComputeResponse = serde_json::from_slice(&result_bytes).expect("result");
    assert_eq!(result.provider_id, "test");
    assert_eq!(result.cost_usd, 2);
    assert_eq!(result.output.get("text").and_then(|v| v.as_str()), Some("ok"));

    let usage_bytes = read_handle(compute.open("usage", OpenFlags::read()).unwrap());
    let usage: serde_json::Value = serde_json::from_slice(&usage_bytes).expect("usage");
    assert_eq!(
        usage["tick"]["spent_usd"].as_u64(),
        Some(2)
    );
    assert_eq!(
        usage["tick"]["reserved_usd"].as_u64(),
        Some(0)
    );

    let response_again = submit_compute(&compute, &request);
    let job_id_again = response_again["job_id"].as_str().expect("job_id");
    assert_eq!(job_id_again, job_id);
}

#[test]
fn test_compute_stream_watch() {
    let mut router = ComputeRouter::new();
    router.register(Arc::new(TestProvider::new()));
    let policy = ComputePolicy {
        require_idempotency: true,
        require_max_cost: true,
        ..ComputePolicy::default()
    };
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let compute = ComputeFs::new(
        AgentId::from("agent-compute-stream"),
        router,
        policy,
        budget,
        journal,
    );

    let request = ComputeRequest {
        model: "test-model".to_string(),
        kind: ComputeKind::Complete,
        input: json!({ "prompt": "hello" }),
        stream: true,
        timeout_ms: None,
        idempotency_key: Some("req-stream".to_string()),
        max_cost_usd: Some(5),
    };

    let response = submit_compute(&compute, &request);
    let job_id = response["job_id"].as_str().expect("job_id").to_string();

    let mut watch = compute
        .watch(&format!("jobs/{}/stream", job_id))
        .expect("watch")
        .expect("handle");

    let first = watch.next(Some(Duration::from_millis(100))).expect("first");
    let first_data = match first {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let first_chunk: ComputeChunk = serde_json::from_slice(&first_data).expect("chunk");
    assert_eq!(first_chunk.delta.get("text").and_then(|v| v.as_str()), Some("hello"));

    let second = watch.next(Some(Duration::from_millis(100))).expect("second");
    let second_data = match second {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let second_chunk: ComputeChunk = serde_json::from_slice(&second_data).expect("chunk");
    assert_eq!(second_chunk.delta.get("text").and_then(|v| v.as_str()), Some(" world"));
    assert_eq!(second_chunk.finish_reason.as_deref(), Some("stop"));

    let done = watch.next(Some(Duration::from_millis(50))).expect("done");
    assert!(done.is_none());

    let result_bytes =
        read_handle(compute.open(&format!("jobs/{}/result", job_id), OpenFlags::read()).unwrap());
    let result: ComputeResponse = serde_json::from_slice(&result_bytes).expect("result");
    assert_eq!(
        result.output.get("text").and_then(|v| v.as_str()),
        Some("hello world")
    );
}

#[test]
fn test_compute_stream_fallback_for_non_streaming_provider() {
    let mut router = ComputeRouter::new();
    router.register(Arc::new(NoStreamProvider::new()));
    let policy = ComputePolicy::default();
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let compute = ComputeFs::new(
        AgentId::from("agent-compute-nostream"),
        router,
        policy,
        budget,
        journal,
    );

    let request = ComputeRequest {
        model: "nostream-model".to_string(),
        kind: ComputeKind::Complete,
        input: json!({ "prompt": "hello" }),
        stream: true,
        timeout_ms: None,
        idempotency_key: Some("req-nostream".to_string()),
        max_cost_usd: Some(5),
    };

    let response = submit_compute(&compute, &request);
    let job_id = response["job_id"].as_str().expect("job_id").to_string();

    let mut watch = compute
        .watch(&format!("jobs/{}/stream", job_id))
        .expect("watch")
        .expect("handle");

    let first = watch.next(Some(Duration::from_millis(100))).expect("first");
    let first_data = match first {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let first_chunk: ComputeChunk = serde_json::from_slice(&first_data).expect("chunk");
    assert_eq!(first_chunk.delta.get("text").and_then(|v| v.as_str()), Some("ok"));
    assert_eq!(first_chunk.finish_reason.as_deref(), Some("complete"));

    let done = watch.next(Some(Duration::from_millis(50))).expect("done");
    assert!(done.is_none());
}

#[test]
fn test_claude_new_usage_and_idempotency() {
    let mut router = ClaudeRouter::new();
    router.register(Arc::new(TestClaudeProvider::new()));
    let policy = ClaudePolicy {
        require_idempotency: true,
        require_max_cost: true,
        ..ClaudePolicy::default()
    };
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let claude = ClaudeFs::new(
        AgentId::from("agent-claude"),
        router,
        policy,
        budget,
        journal,
        signer,
    );

    let mut request = ClaudeRequest::new("claude-test");
    request.max_cost_usd = Some(5);
    request.idempotency_key = Some("claude-1".to_string());

    let response = submit_claude(&claude, &request);
    let session_id = response["session_id"].as_str().expect("session_id").to_string();

    let usage_bytes = read_handle(claude.open("usage", OpenFlags::read()).unwrap());
    let usage: serde_json::Value = serde_json::from_slice(&usage_bytes).expect("usage");
    assert_eq!(usage["tick"]["reserved_usd"].as_u64(), Some(5));

    let mut prompt = claude
        .open(&format!("sessions/{}/prompt", session_id), OpenFlags::write())
        .unwrap();
    prompt.write(b"hello").unwrap();
    prompt.flush().unwrap();

    let _ = read_handle(
        claude
            .open(&format!("sessions/{}/response", session_id), OpenFlags::read())
            .unwrap(),
    );

    let usage_bytes = read_handle(claude.open("usage", OpenFlags::read()).unwrap());
    let usage: serde_json::Value = serde_json::from_slice(&usage_bytes).expect("usage");
    assert_eq!(usage["tick"]["spent_usd"].as_u64(), Some(1));
    assert_eq!(usage["tick"]["reserved_usd"].as_u64(), Some(0));

    let response_again = submit_claude(&claude, &request);
    let session_id_again = response_again["session_id"].as_str().expect("session_id");
    assert_eq!(session_id_again, session_id);
}

#[test]
fn test_claude_output_watch() {
    let mut router = ClaudeRouter::new();
    router.register(Arc::new(TestClaudeProvider::new()));
    let policy = ClaudePolicy::default();
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let claude = ClaudeFs::new(
        AgentId::from("agent-claude-stream"),
        router,
        policy,
        budget,
        journal,
        signer,
    );

    let request = ClaudeRequest::new("claude-test");
    let response = submit_claude(&claude, &request);
    let session_id = response["session_id"].as_str().expect("session_id").to_string();

    let mut prompt = claude
        .open(&format!("sessions/{}/prompt", session_id), OpenFlags::write())
        .unwrap();
    prompt.write(b"hello").unwrap();
    prompt.flush().unwrap();

    let mut watch = claude
        .watch(&format!("sessions/{}/output", session_id))
        .expect("watch")
        .expect("handle");
    let first = watch.next(Some(Duration::from_millis(100))).expect("first");
    let first_data = match first {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let chunk: ClaudeChunk = serde_json::from_slice(&first_data).expect("chunk");
    assert!(matches!(chunk.chunk_type, crate::claude::ChunkType::Done));
}

#[test]
fn test_container_new_usage_and_idempotency() {
    let mut router = ContainerRouter::new();
    router.register(Arc::new(TestContainerProvider::new()));
    let policy = ContainerPolicy {
        require_idempotency: true,
        require_max_cost: true,
        ..ContainerPolicy::default()
    };
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let storage = Arc::new(InMemoryStorage::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let containers = ContainerFs::new(
        AgentId::from("agent-containers"),
        router,
        policy,
        budget,
        journal,
        storage,
        signer,
    );

    let request = ContainerRequest {
        kind: ContainerKind::Ephemeral,
        image: Some("test-image".to_string()),
        repo: None,
        commands: vec!["echo ok".to_string()],
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(5),
        idempotency_key: Some("req-1".to_string()),
        timeout_ms: None,
    };

    let response = submit_container(&containers, &request);
    let session_id = response["session_id"].as_str().expect("session_id").to_string();

    let result_bytes = read_handle(
        containers
            .open(&format!("sessions/{}/result", session_id), OpenFlags::read())
            .unwrap(),
    );
    let result: ContainerResponse = serde_json::from_slice(&result_bytes).expect("result");
    assert_eq!(result.provider_id, "test");
    assert_eq!(result.cost_usd, 3);

    let usage_bytes = read_handle(containers.open("usage", OpenFlags::read()).unwrap());
    let usage: serde_json::Value = serde_json::from_slice(&usage_bytes).expect("usage");
    assert_eq!(usage["tick"]["spent_usd"].as_u64(), Some(3));
    assert_eq!(usage["tick"]["reserved_usd"].as_u64(), Some(0));

    let response_again = submit_container(&containers, &request);
    let session_id_again = response_again["session_id"].as_str().expect("session_id");
    assert_eq!(session_id_again, session_id);
}

#[test]
fn test_container_output_watch() {
    let mut router = ContainerRouter::new();
    router.register(Arc::new(TestContainerProvider::new()));
    let policy = ContainerPolicy::default();
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let storage = Arc::new(InMemoryStorage::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let containers = ContainerFs::new(
        AgentId::from("agent-containers-watch"),
        router,
        policy,
        budget,
        journal,
        storage,
        signer,
    );

    let request = ContainerRequest {
        kind: ContainerKind::Ephemeral,
        image: Some("test-image".to_string()),
        repo: None,
        commands: vec!["echo ok".to_string()],
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(5),
        idempotency_key: Some("req-watch".to_string()),
        timeout_ms: None,
    };

    let response = submit_container(&containers, &request);
    let session_id = response["session_id"].as_str().expect("session_id").to_string();
    let mut watch = containers
        .watch(&format!("sessions/{}/output", session_id))
        .expect("watch")
        .expect("handle");

    let first = watch.next(Some(Duration::from_millis(100))).expect("first");
    let first_data = match first {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let first_chunk: OutputChunk = serde_json::from_slice(&first_data).expect("chunk");
    assert_eq!(first_chunk.stream, OutputStream::Stdout);
    assert_eq!(first_chunk.data, "hello");

    let second = watch.next(Some(Duration::from_millis(100))).expect("second");
    let second_data = match second {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let second_chunk: OutputChunk = serde_json::from_slice(&second_data).expect("chunk");
    assert_eq!(second_chunk.stream, OutputStream::Stderr);
    assert_eq!(second_chunk.data, "error");

    let done = watch.next(Some(Duration::from_millis(50))).expect("done");
    assert!(done.is_none());
}

#[test]
fn test_container_exec_and_files() {
    let mut router = ContainerRouter::new();
    router.register(Arc::new(TestContainerProvider::new()));
    let policy = ContainerPolicy::default();
    let budget = BudgetPolicy {
        per_tick_usd: 10,
        per_day_usd: 10,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let storage = Arc::new(InMemoryStorage::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let containers = ContainerFs::new(
        AgentId::from("agent-containers-exec"),
        router,
        policy,
        budget,
        journal,
        storage,
        signer,
    );

    let request = ContainerRequest {
        kind: ContainerKind::Interactive,
        image: Some("test-image".to_string()),
        repo: None,
        commands: Vec::new(),
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(5),
        idempotency_key: Some("req-exec".to_string()),
        timeout_ms: None,
    };

    let response = submit_container(&containers, &request);
    let session_id = response["session_id"].as_str().expect("session_id").to_string();

    let mut exec_handle = containers
        .open(&format!("sessions/{}/exec/new", session_id), OpenFlags::write())
        .expect("exec new");
    exec_handle.write(b"ls -la").expect("write exec");
    exec_handle.flush().expect("flush exec");
    let exec_response: serde_json::Value = serde_json::from_slice(&read_handle(exec_handle))
        .expect("exec response");
    let exec_id = exec_response["exec_id"].as_str().expect("exec_id").to_string();

    let mut exec_watch = containers
        .watch(&format!("sessions/{}/exec/{}/output", session_id, exec_id))
        .expect("watch")
        .expect("handle");
    let first = exec_watch.next(Some(Duration::from_millis(100))).expect("first");
    let first_data = match first {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("no data"),
    };
    let first_chunk: OutputChunk = serde_json::from_slice(&first_data).expect("chunk");
    assert_eq!(first_chunk.exec_id.as_deref(), Some(exec_id.as_str()));

    let result_bytes = read_handle(
        containers
            .open(
                &format!("sessions/{}/exec/{}/result", session_id, exec_id),
                OpenFlags::read(),
            )
            .unwrap(),
    );
    let result: crate::containers::CommandResult =
        serde_json::from_slice(&result_bytes).expect("result");
    assert_eq!(result.exit_code, 0);

    let file_path = "src%2Fmain.rs";
    let mut file_handle = containers
        .open(&format!("sessions/{}/files/{}", session_id, file_path), OpenFlags::write())
        .expect("file write");
    file_handle.write(b"hello").expect("write");
    file_handle.flush().expect("flush");

    let chunk_bytes = read_handle(
        containers
            .open(
                &format!(
                    "sessions/{}/files/{}/chunks/0",
                    session_id, file_path
                ),
                OpenFlags::read(),
            )
            .unwrap(),
    );
    assert_eq!(chunk_bytes, b"hello");
}

#[test]
fn test_container_auth_token_credits_reconcile() {
    let mut router = ContainerRouter::new();
    router.register(Arc::new(TestContainerProvider::with_id("cloudflare")));
    let policy = ContainerPolicy {
        require_max_cost: true,
        ..ContainerPolicy::default()
    };
    let budget = BudgetPolicy {
        per_tick_usd: 20,
        per_day_usd: 20,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let storage = Arc::new(InMemoryStorage::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let api = Arc::new(TestOpenAgentsApi::new(10));
    let auth = Arc::new(OpenAgentsAuth::new(
        AgentId::from("agent-containers-auth"),
        storage.clone(),
        signer.clone(),
        Some(api),
    ));
    let containers = ContainerFs::with_auth(
        AgentId::from("agent-containers-auth"),
        router,
        policy,
        budget,
        journal,
        auth,
    );

    let mut token_handle = containers.open("auth/token", OpenFlags::write()).unwrap();
    token_handle.write(b"oa_test_token").unwrap();
    token_handle.flush().unwrap();

    let status_bytes = read_handle(containers.open("auth/status", OpenFlags::read()).unwrap());
    let status: ApiAuthState = serde_json::from_slice(&status_bytes).expect("status");
    assert!(status.authenticated);
    assert_eq!(status.method, Some(AuthMethod::ApiKey));
    assert_eq!(status.credits_usd, 10);

    let request = ContainerRequest {
        kind: ContainerKind::Ephemeral,
        image: Some("test-image".to_string()),
        repo: None,
        commands: vec!["echo ok".to_string()],
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(5),
        idempotency_key: Some("credits-1".to_string()),
        timeout_ms: None,
    };

    let response = submit_container(&containers, &request);
    let session_id = response["session_id"].as_str().expect("session_id");

    let _result_bytes = read_handle(
        containers
            .open(&format!("sessions/{}/result", session_id), OpenFlags::read())
            .unwrap(),
    );

    let credits_bytes = read_handle(containers.open("auth/credits", OpenFlags::read()).unwrap());
    let credits: serde_json::Value = serde_json::from_slice(&credits_bytes).expect("credits");
    assert_eq!(credits["credits_usd"].as_u64(), Some(7));
}

#[test]
fn test_container_policy_selects_allowed_provider() {
    let mut router = ContainerRouter::new();
    router.register(Arc::new(TestContainerProvider::with_id("local")));
    router.register(Arc::new(TestContainerProvider::with_id("remote")));

    let mut policy = ContainerPolicy::default();
    policy.allowed_providers = vec!["remote".to_string()];

    let budget = BudgetPolicy {
        per_tick_usd: 20,
        per_day_usd: 20,
        approval_threshold_usd: 0,
        approvers: Vec::new(),
    };
    let journal = Arc::new(MemoryJournal::new());
    let storage = Arc::new(InMemoryStorage::new());
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let auth = Arc::new(OpenAgentsAuth::new(
        AgentId::from("agent-containers-select"),
        storage.clone(),
        signer,
        None,
    ));
    let containers = ContainerFs::with_auth(
        AgentId::from("agent-containers-select"),
        router,
        policy,
        budget,
        journal,
        auth,
    );

    let request = ContainerRequest {
        kind: ContainerKind::Ephemeral,
        image: Some("test-image".to_string()),
        repo: None,
        commands: vec!["echo ok".to_string()],
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(5),
        idempotency_key: None,
        timeout_ms: None,
    };

    let response = submit_container(&containers, &request);
    let session_id = response["session_id"].as_str().expect("session_id");
    let result_bytes = read_handle(
        containers
            .open(&format!("sessions/{}/result", session_id), OpenFlags::read())
            .unwrap(),
    );
    let result: ContainerResponse = serde_json::from_slice(&result_bytes).expect("result");
    assert_eq!(result.provider_id, "remote");
}

#[test]
fn test_hud_settings_and_redaction() {
    let storage = Arc::new(InMemoryStorage::new());
    let logs = Arc::new(LogsFs::new());
    let agent_id = AgentId::from("agent-hud");
    let hud = HudFs::new(agent_id.clone(), storage, logs.clone());

    let settings_bytes = read_handle(hud.open("settings", OpenFlags::read()).unwrap());
    let settings: HudSettings = serde_json::from_slice(&settings_bytes).expect("settings");
    assert!(settings.public);

    let mut handle = hud.open("settings", OpenFlags::write()).unwrap();
    let new_settings = HudSettings {
        public: false,
        embed_allowed: false,
        redaction_policy: "standard".to_string(),
    };
    handle
        .write(&serde_json::to_vec(&new_settings).unwrap())
        .unwrap();
    handle.flush().unwrap();

    let settings_bytes = read_handle(hud.open("settings", OpenFlags::read()).unwrap());
    let settings: HudSettings = serde_json::from_slice(&settings_bytes).expect("settings");
    assert!(!settings.public);
    assert!(!settings.embed_allowed);

    let mut watch = hud.watch("stream").unwrap().unwrap();
    logs.emit_trace(br#"{"event":"tool","token":"secret-value"}"#);
    let event = watch.next(Some(Duration::from_millis(100))).expect("event");
    let data = match event {
        Some(WatchEvent::Data(data)) => data,
        _ => panic!("missing data"),
    };
    let value: serde_json::Value = serde_json::from_slice(&data).expect("json");
    assert_eq!(value["token"], "[REDACTED]");
}

#[test]
fn test_metrics_read_write() {
    let metrics = MetricsFs::new();
    let bytes = read_handle(metrics.open("apm", OpenFlags::read()).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("apm");
    assert_eq!(value["value"].as_f64(), Some(0.0));

    let mut handle = metrics.open("apm", OpenFlags::write()).unwrap();
    handle.write(br#"{"value":12.5,"window_secs":60}"#).unwrap();
    handle.flush().unwrap();

    let bytes = read_handle(metrics.open("apm", OpenFlags::read()).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("apm");
    assert_eq!(value["value"].as_f64(), Some(12.5));
}

#[test]
fn test_logs_trajectory() {
    let logs = LogsFs::new();
    logs.emit_trace(br#"{"event":"tick"}"#);

    let bytes = read_handle(logs.open("trajectory", OpenFlags::read()).unwrap());
    let text = String::from_utf8(bytes).expect("utf8");
    let line = text.lines().next().expect("line");
    let event: TraceEvent = serde_json::from_str(line).expect("trace");
    assert!(event.data.contains("tick"));
}

#[test]
fn test_nostr_signer_sign_verify() {
    let signer = NostrSigner::new();
    let agent_id = AgentId::from("agent-nostr");
    let pubkey = signer.pubkey(&agent_id).expect("pubkey");
    let sig = signer.sign(&agent_id, b"hello").expect("sign");
    assert!(signer.verify(&pubkey, b"hello", &sig));
    assert!(!signer.verify(&pubkey, b"other", &sig));
}

#[test]
fn test_nostr_signer_encrypt_decrypt() {
    let signer = NostrSigner::new();
    let sender = AgentId::from("agent-sender");
    let receiver = AgentId::from("agent-receiver");

    let receiver_pubkey = signer.pubkey(&receiver).expect("receiver pubkey");
    let sender_pubkey = signer.pubkey(&sender).expect("sender pubkey");

    let ciphertext = signer
        .encrypt(&sender, &receiver_pubkey, b"secret")
        .expect("encrypt");
    let plaintext = signer
        .decrypt(&receiver, &sender_pubkey, &ciphertext)
        .expect("decrypt");

    assert_eq!(plaintext, b"secret");
}

#[test]
fn test_nostr_driver_routes_mentions() {
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let agent_id = AgentId::from("agent-mention");
    let pubkey_hex = signer.pubkey(&agent_id).expect("pubkey").to_hex();

    let mut pubkey_map = HashMap::new();
    pubkey_map.insert(pubkey_hex.clone(), agent_id.clone());

    let event = fake_event(
        "11".repeat(32),
        KIND_SHORT_TEXT_NOTE,
        "hello".to_string(),
        vec![vec!["p".to_string(), pubkey_hex.clone()]],
    );

    let routed = crate::drivers::nostr::route_event(&signer, &pubkey_map, &event);
    assert_eq!(routed.len(), 1);
    assert_eq!(routed[0].agent_id, agent_id);
    assert_eq!(routed[0].envelope.payload["type"], "nostr_mention");
}

#[test]
fn test_nostr_driver_routes_dm() {
    let signer: Arc<dyn SigningService> = Arc::new(NostrSigner::new());
    let sender = AgentId::from("agent-dm-sender");
    let receiver = AgentId::from("agent-dm-receiver");

    let receiver_pubkey = signer.pubkey(&receiver).expect("receiver pubkey");
    let receiver_pubkey_hex = receiver_pubkey.to_hex();
    let sender_pubkey_hex = signer.pubkey(&sender).expect("sender pubkey").to_hex();

    let ciphertext = signer
        .encrypt(&sender, &receiver_pubkey, b"secret")
        .expect("encrypt");
    let content = String::from_utf8(ciphertext).expect("utf8");

    let event = fake_event(
        sender_pubkey_hex.clone(),
        ENCRYPTED_DM_KIND,
        content,
        vec![vec!["p".to_string(), receiver_pubkey_hex.clone()]],
    );

    let mut pubkey_map = HashMap::new();
    pubkey_map.insert(receiver_pubkey_hex.clone(), receiver.clone());

    let routed = crate::drivers::nostr::route_event(&signer, &pubkey_map, &event);
    assert_eq!(routed.len(), 1);
    assert_eq!(routed[0].agent_id, receiver);
    assert_eq!(routed[0].envelope.payload["type"], "nostr_dm");
    assert_eq!(routed[0].envelope.payload["nostr"]["content"], "secret");
    assert_eq!(routed[0].envelope.payload["nostr"]["decrypted"], true);
}

#[tokio::test]
async fn test_driver_sink_routes_envelope() {
    let storage = Arc::new(InMemoryStorage::new());
    let runtime = Arc::new(LocalRuntime::new(storage));
    let agent_id = AgentId::from("agent-driver");
    runtime
        .register_agent(agent_id.clone(), CountingAgent)
        .await
        .expect("register agent");

    let sink = runtime.driver_sink();
    let envelope = Envelope {
        id: EnvelopeId::new("env-driver"),
        timestamp: Timestamp::now(),
        payload: json!({ "message": "hello" }),
    };
    sink.send(RoutedEnvelope { agent_id: agent_id.clone(), envelope })
        .await
        .expect("send");

    let control_plane = ControlPlane::new(runtime);
    let app = control_plane.router();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();
    let items: Vec<Envelope> = client
        .get(format!("{}/agents/{}/inbox", base, agent_id))
        .send()
        .await
        .expect("get inbox")
        .json()
        .await
        .expect("parse inbox");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id.as_str(), "env-driver");
}

fn fake_event(pubkey: String, kind: u16, content: String, tags: Vec<Vec<String>>) -> Event {
    Event {
        id: "01".repeat(32),
        pubkey,
        created_at: 1_700_000_000,
        kind,
        tags,
        content,
        sig: "00".repeat(64),
    }
}

#[tokio::test]
async fn test_control_plane_http() {
    let storage = Arc::new(InMemoryStorage::new());
    let runtime = Arc::new(LocalRuntime::new(storage));
    let agent_id = AgentId::from("agent-http");
    runtime
        .register_agent(agent_id.clone(), CountingAgent)
        .await
        .expect("register agent");

    let control_plane = ControlPlane::new(runtime);
    let app = control_plane.router();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    let list: Vec<String> = client
        .get(format!("{}/agents", base))
        .send()
        .await
        .expect("list agents")
        .json()
        .await
        .expect("list json");
    assert_eq!(list, vec![agent_id.to_string()]);

    client
        .post(format!("{}/agents/{}/send", base, agent_id))
        .body("hello")
        .send()
        .await
        .expect("send message")
        .error_for_status()
        .expect("send status");

    let inbox: Vec<Envelope> = client
        .get(format!("{}/agents/{}/inbox", base, agent_id))
        .send()
        .await
        .expect("get inbox")
        .json()
        .await
        .expect("inbox json");
    assert_eq!(inbox.len(), 1);

    let result: TickResult = client
        .post(format!("{}/agents/{}/tick", base, agent_id))
        .send()
        .await
        .expect("tick")
        .json()
        .await
        .expect("tick json");
    assert!(result.success);

    let status: serde_json::Value = client
        .get(format!("{}/agents/{}/status", base, agent_id))
        .send()
        .await
        .expect("status")
        .json()
        .await
        .expect("status json");
    assert_eq!(status["agent_id"], agent_id.as_str());
}

#[cfg(feature = "local")]
#[test]
fn test_idempotent_effects_sqlite() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("journal.db");
    let journal = crate::idempotency::SqliteJournal::new(&path).expect("journal");
    let ttl = Duration::from_millis(50);
    assert!(journal.get("key").expect("get").is_none());
    journal
        .put_with_ttl("key", b"payload", ttl)
        .expect("put");
    assert_eq!(
        journal.get("key").expect("get"),
        Some(b"payload".to_vec())
    );
    std::thread::sleep(Duration::from_millis(60));
    assert!(journal.get("key").expect("expired").is_none());
}

struct BufferService {
    buffer: Arc<std::sync::Mutex<Vec<u8>>>,
}

impl BufferService {
    fn new() -> Self {
        Self {
            buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl FileService for BufferService {
    fn open(&self, _path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        if flags.write {
            Ok(Box::new(BufferHandle::new(self.buffer.clone())))
        } else {
            Err(FsError::PermissionDenied)
        }
    }

    fn readdir(&self, _path: &str) -> FsResult<Vec<crate::fs::DirEntry>> {
        Ok(Vec::new())
    }

    fn stat(&self, _path: &str) -> FsResult<Stat> {
        Ok(Stat::file(0))
    }

    fn mkdir(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn remove(&self, _path: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn rename(&self, _from: &str, _to: &str) -> FsResult<()> {
        Err(FsError::PermissionDenied)
    }

    fn watch(&self, _path: &str) -> FsResult<Option<Box<dyn crate::fs::WatchHandle>>> {
        Ok(None)
    }

    fn name(&self) -> &str {
        "buffer"
    }
}

struct BufferHandle {
    buffer: Arc<std::sync::Mutex<Vec<u8>>>,
    pos: usize,
}

impl BufferHandle {
    fn new(buffer: Arc<std::sync::Mutex<Vec<u8>>>) -> Self {
        Self { buffer, pos: 0 }
    }
}

impl FileHandle for BufferHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        let mut guard = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        guard.extend_from_slice(buf);
        self.pos += buf.len();
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: crate::fs::SeekFrom) -> FsResult<u64> {
        Ok(self.pos as u64)
    }

    fn position(&self) -> u64 {
        self.pos as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

fn read_handle(mut handle: Box<dyn FileHandle>) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let read = handle.read(&mut chunk).expect("read");
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read]);
    }
    buf
}

fn submit_compute(fs: &ComputeFs, request: &ComputeRequest) -> serde_json::Value {
    let mut handle = fs.open("new", OpenFlags::write()).expect("open");
    let bytes = serde_json::to_vec(request).expect("serialize");
    handle.write(&bytes).expect("write");
    handle.flush().expect("flush");
    let response = read_handle(handle);
    serde_json::from_slice(&response).expect("response json")
}

fn submit_container(fs: &ContainerFs, request: &ContainerRequest) -> serde_json::Value {
    let mut handle = fs.open("new", OpenFlags::write()).expect("open");
    let bytes = serde_json::to_vec(request).expect("serialize");
    handle.write(&bytes).expect("write");
    handle.flush().expect("flush");
    let response = read_handle(handle);
    serde_json::from_slice(&response).expect("response json")
}

fn submit_claude(fs: &ClaudeFs, request: &ClaudeRequest) -> serde_json::Value {
    let mut handle = fs.open("new", OpenFlags::write()).expect("open");
    let bytes = serde_json::to_vec(request).expect("serialize");
    handle.write(&bytes).expect("write");
    handle.flush().expect("flush");
    let response = read_handle(handle);
    serde_json::from_slice(&response).expect("response json")
}

static JOB_COUNTER: AtomicUsize = AtomicUsize::new(1);
static SESSION_COUNTER: AtomicUsize = AtomicUsize::new(1);
static EXEC_COUNTER: AtomicUsize = AtomicUsize::new(1);
static CLAUDE_SESSION_COUNTER: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone)]
struct TestProvider {
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
    streams: Arc<std::sync::Mutex<HashMap<String, VecDeque<ComputeChunk>>>>,
    responses: Arc<std::sync::Mutex<HashMap<String, ComputeResponse>>>,
}

impl TestProvider {
    fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            streams: Arc::new(std::sync::Mutex::new(HashMap::new())),
            responses: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Clone)]
struct NoStreamProvider {
    jobs: Arc<RwLock<HashMap<String, JobState>>>,
}

impl NoStreamProvider {
    fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ComputeProvider for NoStreamProvider {
    fn id(&self) -> &str {
        "nostream"
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "nostream".to_string(),
            name: "No-Stream Provider".to_string(),
            models: vec![ModelInfo {
                id: "nostream-model".to_string(),
                name: "No-Stream Model".to_string(),
                context_length: Some(1024),
                capabilities: vec![ComputeKind::Complete],
                pricing: None,
            }],
            capabilities: vec![ComputeKind::Complete],
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 1,
                tokens_per_sec: Some(10),
                measured: true,
            },
            region: Some("local".to_string()),
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn supports_model(&self, model: &str) -> bool {
        model == "nostream-model"
    }

    fn submit(&self, request: ComputeRequest) -> std::result::Result<String, ComputeError> {
        let job_id = format!(
            "job-{}",
            JOB_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let response = ComputeResponse {
            job_id: job_id.clone(),
            output: json!({ "text": "ok" }),
            usage: None,
            cost_usd: request.max_cost_usd.unwrap_or(0),
            latency_ms: 1,
            provider_id: "nostream".to_string(),
            model: request.model.clone(),
        };
        self.jobs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job_id.clone(), JobState::Complete(response));
        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .cloned()
    }

    fn poll_stream(
        &self,
        job_id: &str,
    ) -> std::result::Result<Option<ComputeChunk>, ComputeError> {
        if self
            .jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(job_id)
        {
            Ok(None)
        } else {
            Err(ComputeError::JobNotFound)
        }
    }

    fn cancel(&self, job_id: &str) -> std::result::Result<(), ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        if jobs.remove(job_id).is_some() {
            Ok(())
        } else {
            Err(ComputeError::JobNotFound)
        }
    }
}

impl ComputeProvider for TestProvider {
    fn id(&self) -> &str {
        "test"
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "test".to_string(),
            name: "Test Provider".to_string(),
            models: vec![ModelInfo {
                id: "test-model".to_string(),
                name: "Test Model".to_string(),
                context_length: Some(2048),
                capabilities: vec![ComputeKind::Complete],
                pricing: None,
            }],
            capabilities: vec![ComputeKind::Complete],
            pricing: None,
            latency: ProviderLatency {
                ttft_ms: 1,
                tokens_per_sec: Some(100),
                measured: true,
            },
            region: Some("local".to_string()),
            status: ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn supports_model(&self, model: &str) -> bool {
        model == "test-model"
    }

    fn submit(&self, request: ComputeRequest) -> std::result::Result<String, ComputeError> {
        let job_id = format!(
            "job-{}",
            JOB_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        if request.stream {
            let chunks = VecDeque::from(vec![
                ComputeChunk {
                    job_id: job_id.clone(),
                    delta: json!({ "text": "hello" }),
                    finish_reason: None,
                    usage: None,
                },
                ComputeChunk {
                    job_id: job_id.clone(),
                    delta: json!({ "text": " world" }),
                    finish_reason: Some("stop".to_string()),
                    usage: None,
                },
            ]);
            let response = ComputeResponse {
                job_id: job_id.clone(),
                output: json!({ "text": "hello world", "finish_reason": "stop" }),
                usage: None,
                cost_usd: 2,
                latency_ms: 1,
                provider_id: "test".to_string(),
                model: request.model.clone(),
            };
            self.responses
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(job_id.clone(), response);
            self.streams
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(job_id.clone(), chunks);
            self.jobs
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
                    job_id.clone(),
                    JobState::Streaming {
                        started_at: Timestamp::now(),
                        chunks_emitted: 0,
                    },
                );
        } else {
            let response = ComputeResponse {
                job_id: job_id.clone(),
                output: json!({ "text": "ok", "finish_reason": "stop" }),
                usage: None,
                cost_usd: 2,
                latency_ms: 1,
                provider_id: "test".to_string(),
                model: request.model.clone(),
            };
            self.jobs
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(job_id.clone(), JobState::Complete(response));
        }
        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(job_id)
            .cloned()
    }

    fn poll_stream(
        &self,
        job_id: &str,
    ) -> std::result::Result<Option<ComputeChunk>, ComputeError> {
        let has_job = self
            .jobs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(job_id);
        let (chunk, is_last) = {
            let mut streams = self.streams.lock().unwrap_or_else(|e| e.into_inner());
            let queue = match streams.get_mut(job_id) {
                Some(queue) => queue,
                None => {
                    if has_job {
                        return Ok(None);
                    }
                    return Err(ComputeError::JobNotFound);
                }
            };
            let chunk = queue.pop_front();
            let is_last = chunk.is_some() && queue.is_empty();
            if is_last {
                streams.remove(job_id);
            }
            (chunk, is_last)
        };

        let Some(chunk) = chunk else {
            return Ok(None);
        };

        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        if let Some(job) = jobs.get_mut(job_id) {
            if let JobState::Streaming {
                started_at,
                chunks_emitted,
            } = job.clone()
            {
                job.clone_from(&JobState::Streaming {
                    started_at,
                    chunks_emitted: chunks_emitted.saturating_add(1),
                });
            }
        }

        if is_last {
            if let Some(response) = self
                .responses
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(job_id)
            {
                jobs.insert(job_id.to_string(), JobState::Complete(response));
            }
        }

        Ok(Some(chunk))
    }

    fn cancel(&self, job_id: &str) -> std::result::Result<(), ComputeError> {
        let mut jobs = self.jobs.write().unwrap_or_else(|e| e.into_inner());
        let job = jobs.get_mut(job_id).ok_or(ComputeError::JobNotFound)?;
        job.clone_from(&JobState::Failed {
            error: "cancelled".to_string(),
            at: Timestamp::now(),
        });
        Ok(())
    }
}

#[derive(Clone)]
struct TestClaudeProvider {
    sessions: Arc<RwLock<HashMap<String, ClaudeSessionState>>>,
    chunks: Arc<std::sync::Mutex<HashMap<String, VecDeque<ClaudeChunk>>>>,
}

impl TestClaudeProvider {
    fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            chunks: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    fn next_id() -> String {
        format!(
            "claude-{}",
            CLAUDE_SESSION_COUNTER.fetch_add(1, Ordering::SeqCst)
        )
    }
}

impl ClaudeProvider for TestClaudeProvider {
    fn id(&self) -> &str {
        "test-claude"
    }

    fn info(&self) -> ClaudeProviderInfo {
        ClaudeProviderInfo {
            id: "test-claude".to_string(),
            name: "Test Claude".to_string(),
            models: vec![ClaudeModelInfo {
                id: "claude-test".to_string(),
                name: "Claude Test".to_string(),
                context_length: 1024,
                output_limit: 512,
                pricing: None,
            }],
            capabilities: ClaudeCapabilities {
                streaming: true,
                resume: true,
                fork: true,
                tools: true,
                vision: false,
            },
            pricing: None,
            status: ClaudeProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn supports_model(&self, model: &str) -> bool {
        model == "claude-test"
    }

    fn create_session(&self, request: ClaudeRequest) -> std::result::Result<String, ClaudeError> {
        let session_id = Self::next_id();
        let state = ClaudeSessionState::Ready {
            created_at: Timestamp::now(),
        };
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), state);
        if request.initial_prompt.is_some() {
            let mut chunks = self.chunks.lock().unwrap_or_else(|e| e.into_inner());
            chunks.insert(
                session_id.clone(),
                VecDeque::from(vec![ClaudeChunk {
                    session_id: session_id.clone(),
                    chunk_type: crate::claude::ChunkType::Text,
                    delta: Some("ok".to_string()),
                    tool: None,
                    usage: None,
                }]),
            );
        }
        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<ClaudeSessionState> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
    }

    fn send_prompt(&self, session_id: &str, _prompt: &str) -> std::result::Result<(), ClaudeError> {
        let response = ClaudeResponse {
            session_id: session_id.to_string(),
            status: ClaudeSessionStatus::Complete,
            response: Some("ok".to_string()),
            usage: Some(ClaudeUsage {
                input_tokens: 1,
                output_tokens: 1,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: 2,
            }),
            cost_usd: 1,
            reserved_usd: 0,
            provider_id: "test-claude".to_string(),
            model: "claude-test".to_string(),
            tunnel_endpoint: None,
        };
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.to_string(), ClaudeSessionState::Complete(response));
        self.chunks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(session_id.to_string())
            .or_insert_with(VecDeque::new)
            .push_back(ClaudeChunk {
                session_id: session_id.to_string(),
                chunk_type: crate::claude::ChunkType::Done,
                delta: None,
                tool: None,
                usage: None,
            });
        Ok(())
    }

    fn poll_output(
        &self,
        session_id: &str,
    ) -> std::result::Result<Option<ClaudeChunk>, ClaudeError> {
        let mut chunks = self.chunks.lock().unwrap_or_else(|e| e.into_inner());
        let queue = chunks.get_mut(session_id).ok_or(ClaudeError::SessionNotFound)?;
        Ok(queue.pop_front())
    }

    fn approve_tool(&self, _session_id: &str, _approved: bool) -> std::result::Result<(), ClaudeError> {
        Ok(())
    }

    fn fork_session(&self, _session_id: &str) -> std::result::Result<String, ClaudeError> {
        let mut request = ClaudeRequest::new("claude-test");
        request.autonomy = Some(ClaudeSessionAutonomy::Supervised);
        self.create_session(request)
    }

    fn stop(&self, session_id: &str) -> std::result::Result<(), ClaudeError> {
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.to_string(),
                ClaudeSessionState::Failed {
                    error: "stopped".to_string(),
                    at: Timestamp::now(),
                },
            );
        Ok(())
    }

    fn pause(&self, _session_id: &str) -> std::result::Result<(), ClaudeError> {
        Ok(())
    }

    fn resume(&self, _session_id: &str) -> std::result::Result<(), ClaudeError> {
        Ok(())
    }

    fn tool_log(&self, _session_id: &str) -> Option<Vec<ToolLogEntry>> {
        Some(Vec::new())
    }

    fn pending_tool(&self, _session_id: &str) -> Option<crate::claude::PendingToolInfo> {
        None
    }
}

#[derive(Clone)]
struct TestContainerProvider {
    id: String,
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    session_outputs: Arc<std::sync::Mutex<HashMap<String, VecDeque<OutputChunk>>>>,
    execs: Arc<RwLock<HashMap<String, ExecState>>>,
    exec_outputs: Arc<std::sync::Mutex<HashMap<String, VecDeque<OutputChunk>>>>,
    files: Arc<std::sync::Mutex<HashMap<(String, String), Vec<u8>>>>,
}

impl TestContainerProvider {
    fn new() -> Self {
        Self::with_id("test")
    }

    fn with_id(id: &str) -> Self {
        Self {
            id: id.to_string(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_outputs: Arc::new(std::sync::Mutex::new(HashMap::new())),
            execs: Arc::new(RwLock::new(HashMap::new())),
            exec_outputs: Arc::new(std::sync::Mutex::new(HashMap::new())),
            files: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

struct TestOpenAgentsApi {
    credits_usd: u64,
}

type ContainerResult<T> = std::result::Result<T, ContainerError>;

impl TestOpenAgentsApi {
    fn new(credits_usd: u64) -> Self {
        Self { credits_usd }
    }

    fn auth_state(&self, method: AuthMethod) -> ApiAuthState {
        let mut state = ApiAuthState::default();
        state.authenticated = true;
        state.method = Some(method);
        state.token_set = true;
        state.credits_usd = self.credits_usd;
        state
    }
}

impl OpenAgentsApiClient for TestOpenAgentsApi {
    fn authenticate_token(&self, _token: &str) -> ContainerResult<crate::containers::ApiAuthResponse> {
        Ok(crate::containers::ApiAuthResponse {
            state: self.auth_state(AuthMethod::ApiKey),
            access_token: None,
        })
    }

    fn authenticate_nostr(
        &self,
        _response: &crate::containers::NostrAuthResponse,
    ) -> ContainerResult<crate::containers::ApiAuthResponse> {
        Ok(crate::containers::ApiAuthResponse {
            state: self.auth_state(AuthMethod::Nostr),
            access_token: None,
        })
    }

    fn provider_info(
        &self,
        _provider_id: &str,
        _token: Option<&str>,
    ) -> ContainerResult<ContainerProviderInfo> {
        Err(ContainerError::NotSupported {
            capability: "provider_info".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn submit_container(
        &self,
        _provider_id: &str,
        _request: &ContainerRequest,
        _token: &str,
    ) -> ContainerResult<String> {
        Err(ContainerError::NotSupported {
            capability: "submit".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn session_state(&self, _session_id: &str, _token: &str) -> ContainerResult<SessionState> {
        Err(ContainerError::NotSupported {
            capability: "session_state".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn submit_exec(
        &self,
        _session_id: &str,
        _command: &str,
        _token: &str,
    ) -> ContainerResult<String> {
        Err(ContainerError::NotSupported {
            capability: "submit_exec".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn exec_state(&self, _exec_id: &str, _token: &str) -> ContainerResult<ExecState> {
        Err(ContainerError::NotSupported {
            capability: "exec_state".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn poll_output(
        &self,
        _session_id: &str,
        _cursor: Option<&str>,
        _token: &str,
    ) -> ContainerResult<(Option<OutputChunk>, Option<String>)> {
        Err(ContainerError::NotSupported {
            capability: "poll_output".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn poll_exec_output(
        &self,
        _exec_id: &str,
        _cursor: Option<&str>,
        _token: &str,
    ) -> ContainerResult<(Option<OutputChunk>, Option<String>)> {
        Err(ContainerError::NotSupported {
            capability: "poll_exec_output".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn read_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _len: u64,
        _token: &str,
    ) -> ContainerResult<Vec<u8>> {
        Err(ContainerError::NotSupported {
            capability: "read_file".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn write_file(
        &self,
        _session_id: &str,
        _path: &str,
        _offset: u64,
        _data: &[u8],
        _token: &str,
    ) -> ContainerResult<()> {
        Err(ContainerError::NotSupported {
            capability: "write_file".to_string(),
            provider: "test_openagents".to_string(),
        })
    }

    fn stop(&self, _session_id: &str, _token: &str) -> ContainerResult<()> {
        Err(ContainerError::NotSupported {
            capability: "stop".to_string(),
            provider: "test_openagents".to_string(),
        })
    }
}

impl ContainerProvider for TestContainerProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn info(&self) -> ContainerProviderInfo {
        ContainerProviderInfo {
            id: self.id.clone(),
            name: format!("Test Containers ({})", self.id),
            available_images: vec!["test-image".to_string()],
            capabilities: ContainerCapabilities {
                git_clone: false,
                file_access: true,
                interactive: true,
                artifacts: false,
                streaming: true,
            },
            pricing: None,
            latency: crate::containers::ContainerLatency {
                startup_ms: 1,
                measured: true,
            },
            limits: crate::containers::ContainerLimits {
                max_memory_mb: 1024,
                max_cpu_cores: 1.0,
                max_disk_mb: 512,
                max_time_secs: 300,
                network_allowed: false,
            },
            status: crate::containers::ProviderStatus::Available,
        }
    }

    fn is_available(&self) -> bool {
        true
    }

    fn submit(&self, request: ContainerRequest) -> std::result::Result<String, ContainerError> {
        let session_id = format!(
            "session-{}",
            SESSION_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let response = ContainerResponse {
            session_id: session_id.clone(),
            exit_code: Some(0),
            stdout: "ok".to_string(),
            stderr: String::new(),
            command_results: request
                .commands
                .iter()
                .map(|cmd| crate::containers::CommandResult {
                    command: cmd.clone(),
                    exit_code: 0,
                    stdout: "ok".to_string(),
                    stderr: String::new(),
                    duration_ms: 1,
                })
                .collect(),
            artifacts: Vec::new(),
            usage: ContainerUsage::zero(),
            cost_usd: 3,
            reserved_usd: request.max_cost_usd.unwrap_or(0),
            duration_ms: 1,
            provider_id: self.id.clone(),
        };

        let state = if matches!(request.kind, ContainerKind::Interactive) {
            SessionState::Running {
                started_at: Timestamp::now(),
                commands_completed: 0,
            }
        } else {
            SessionState::Complete(response)
        };
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(session_id.clone(), state);
        self.session_outputs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                VecDeque::from(vec![
                    OutputChunk {
                        session_id: session_id.clone(),
                        exec_id: None,
                        stream: OutputStream::Stdout,
                        data: "hello".to_string(),
                    },
                    OutputChunk {
                        session_id: session_id.clone(),
                        exec_id: None,
                        stream: OutputStream::Stderr,
                        data: "error".to_string(),
                    },
                ]),
            );
        Ok(session_id)
    }

    fn get_session(&self, session_id: &str) -> Option<SessionState> {
        self.sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .cloned()
    }

    fn submit_exec(
        &self,
        session_id: &str,
        command: &str,
    ) -> std::result::Result<String, ContainerError> {
        let exec_id = format!("exec-{}", EXEC_COUNTER.fetch_add(1, Ordering::SeqCst));
        self.execs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                ExecState::Complete(crate::containers::CommandResult {
                    command: command.to_string(),
                    exit_code: 0,
                    stdout: "ok".to_string(),
                    stderr: String::new(),
                    duration_ms: 1,
                }),
            );
        self.exec_outputs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                exec_id.clone(),
                VecDeque::from(vec![OutputChunk {
                    session_id: session_id.to_string(),
                    exec_id: Some(exec_id.clone()),
                    stream: OutputStream::Stdout,
                    data: "exec".to_string(),
                }]),
            );
        Ok(exec_id)
    }

    fn get_exec(&self, exec_id: &str) -> Option<ExecState> {
        self.execs
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(exec_id)
            .cloned()
    }

    fn poll_exec_output(
        &self,
        exec_id: &str,
    ) -> std::result::Result<Option<OutputChunk>, ContainerError> {
        let mut outputs = self.exec_outputs.lock().unwrap_or_else(|e| e.into_inner());
        let queue = outputs
            .get_mut(exec_id)
            .ok_or(ContainerError::ExecNotFound)?;
        Ok(queue.pop_front())
    }

    fn cancel_exec(&self, exec_id: &str) -> std::result::Result<(), ContainerError> {
        let mut execs = self.execs.write().unwrap_or_else(|e| e.into_inner());
        let exec = execs.get_mut(exec_id).ok_or(ContainerError::ExecNotFound)?;
        exec.clone_from(&ExecState::Failed {
            error: "cancelled".to_string(),
            at: Timestamp::now(),
        });
        Ok(())
    }

    fn read_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        len: u64,
    ) -> std::result::Result<Vec<u8>, ContainerError> {
        let key = (session_id.to_string(), path.to_string());
        let data = self
            .files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned()
            .unwrap_or_default();
        let start = offset.min(data.len() as u64) as usize;
        let end = (offset.saturating_add(len)).min(data.len() as u64) as usize;
        Ok(data[start..end].to_vec())
    }

    fn write_file(
        &self,
        session_id: &str,
        path: &str,
        offset: u64,
        data: &[u8],
    ) -> std::result::Result<(), ContainerError> {
        let key = (session_id.to_string(), path.to_string());
        let mut files = self.files.lock().unwrap_or_else(|e| e.into_inner());
        let entry = files.entry(key).or_default();
        let offset = offset as usize;
        if offset > entry.len() {
            entry.resize(offset, 0);
        }
        let end = offset + data.len();
        if end > entry.len() {
            entry.resize(end, 0);
        }
        entry[offset..end].copy_from_slice(data);
        Ok(())
    }

    fn stop(&self, session_id: &str) -> std::result::Result<(), ContainerError> {
        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.to_string(),
                SessionState::Expired { at: Timestamp::now() },
            );
        Ok(())
    }

    fn poll_output(
        &self,
        session_id: &str,
    ) -> std::result::Result<Option<OutputChunk>, ContainerError> {
        let mut outputs = self
            .session_outputs
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let queue = outputs
            .get_mut(session_id)
            .ok_or(ContainerError::SessionNotFound)?;
        Ok(queue.pop_front())
    }
}

#[derive(Clone)]
struct TestDvmTransport {
    published: Arc<Mutex<Vec<Event>>>,
    subscribers: Arc<Mutex<HashMap<String, mpsc::Sender<Event>>>>,
}

impl TestDvmTransport {
    fn new() -> Self {
        Self {
            published: Arc::new(Mutex::new(Vec::new())),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn published_events(&self) -> Vec<Event> {
        self.published.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn emit(&self, event: Event) {
        let subscribers = self.subscribers.lock().unwrap_or_else(|e| e.into_inner());
        for sender in subscribers.values() {
            let _ = sender.try_send(event.clone());
        }
    }
}

#[async_trait]
impl DvmTransport for TestDvmTransport {
    async fn connect(&self) -> std::result::Result<(), String> {
        Ok(())
    }

    async fn publish(&self, event: Event) -> std::result::Result<(), String> {
        self.published
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(event);
        Ok(())
    }

    async fn subscribe(
        &self,
        subscription_id: &str,
        _filters: &[serde_json::Value],
    ) -> std::result::Result<mpsc::Receiver<Event>, String> {
        let (tx, rx) = mpsc::channel(64);
        self.subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(subscription_id.to_string(), tx);
        Ok(rx)
    }

    async fn unsubscribe(&self, subscription_id: &str) -> std::result::Result<(), String> {
        self.subscribers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(subscription_id);
        Ok(())
    }

    async fn query(
        &self,
        _filters: &[serde_json::Value],
        _timeout: Duration,
    ) -> std::result::Result<Vec<Event>, String> {
        Ok(Vec::new())
    }

    fn relays(&self) -> Vec<String> {
        vec!["wss://test".to_string()]
    }
}

struct TestWallet {
    balance_sats: u64,
    fx: FxRateSnapshot,
    payments: Arc<Mutex<Vec<WalletPayment>>>,
}

impl TestWallet {
    fn new(balance_sats: u64, fx: FxRateSnapshot) -> Self {
        Self {
            balance_sats,
            fx,
            payments: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn payments(&self) -> Vec<WalletPayment> {
        self.payments.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl WalletService for TestWallet {
    fn balance_sats(&self) -> std::result::Result<u64, WalletError> {
        Ok(self.balance_sats)
    }

    fn pay_invoice(
        &self,
        _invoice: &str,
        amount_sats: Option<u64>,
    ) -> std::result::Result<WalletPayment, WalletError> {
        let amount_sats = amount_sats.unwrap_or(0);
        let payment = WalletPayment {
            payment_id: format!("pay-{}", amount_sats),
            amount_sats,
        };
        self.payments
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(payment.clone());
        Ok(payment)
    }

    fn fx_rate(&self) -> std::result::Result<FxRateSnapshot, WalletError> {
        Ok(self.fx.clone())
    }
}

fn dummy_event(kind: u16, tags: Vec<Vec<String>>, content: &str) -> Event {
    Event {
        id: uuid::Uuid::new_v4().to_string(),
        pubkey: "pubkey".to_string(),
        created_at: 0,
        kind,
        tags,
        content: content.to_string(),
        sig: "sig".to_string(),
    }
}

fn find_tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| {
        if tag.get(0).map(|key| key.as_str()) == Some(name) {
            tag.get(1).map(|value| value.as_str())
        } else {
            None
        }
    })
}

#[test]
fn test_dvm_compute_payment_flow() {
    let transport = Arc::new(TestDvmTransport::new());
    let fx = FxRateSnapshot {
        sats_per_usd: 100_000,
        updated_at: Timestamp::now(),
    };
    let wallet = Arc::new(TestWallet::new(1_000_000, fx.clone()));
    let signer = Arc::new(NostrSigner::new());
    let provider = DvmProvider::with_transport(
        AgentId::from("agent-dvm"),
        transport.clone(),
        signer,
        Some(wallet.clone()),
        FxSource::Fixed {
            sats_per_usd: fx.sats_per_usd,
        },
        60,
    )
    .unwrap();

    let request = ComputeRequest {
        model: "test-model".to_string(),
        kind: ComputeKind::Complete,
        input: json!({ "prompt": "hello" }),
        stream: true,
        timeout_ms: None,
        idempotency_key: None,
        max_cost_usd: Some(100_000),
    };
    let job_id = provider.submit(request).unwrap();
    let request_event_id = transport
        .published_events()
        .iter()
        .find(|event| event.kind == KIND_JOB_TEXT_GENERATION)
        .map(|event| event.id.clone())
        .unwrap();

    let partial_event = dummy_event(
        KIND_JOB_FEEDBACK,
        vec![
            vec!["status".to_string(), "partial".to_string()],
            vec!["e".to_string(), request_event_id.clone()],
        ],
        "chunk-1",
    );
    transport.emit(partial_event);
    std::thread::sleep(Duration::from_millis(50));
    let chunk = provider.poll_stream(&job_id).unwrap().unwrap();
    assert_eq!(chunk.delta["text"], "chunk-1");

    let payment_event = dummy_event(
        KIND_JOB_FEEDBACK,
        vec![
            vec!["status".to_string(), "payment-required".to_string()],
            vec!["e".to_string(), request_event_id.clone()],
            vec![
                "amount".to_string(),
                "1000".to_string(),
                "lnbc1test".to_string(),
            ],
        ],
        "",
    );
    transport.emit(payment_event);
    std::thread::sleep(Duration::from_millis(50));

    let job_result = JobResult::new(
        KIND_JOB_TEXT_GENERATION,
        request_event_id.clone(),
        "customer",
        "done",
    )
    .unwrap()
    .with_amount(1000, Some("lnbc1test".to_string()));
    let result_event = dummy_event(job_result.kind, job_result.to_tags(), &job_result.content);
    transport.emit(result_event);
    std::thread::sleep(Duration::from_millis(100));

    let state = provider.get_job(&job_id).unwrap();
    match state {
        JobState::Complete(response) => {
            assert_eq!(response.cost_usd, 10);
        }
        other => panic!("unexpected state: {:?}", other),
    }

    assert_eq!(wallet.payments().len(), 1);
}

#[test]
fn test_dvm_compute_wallet_fx_source_bid() {
    let transport = Arc::new(TestDvmTransport::new());
    let fx = FxRateSnapshot {
        sats_per_usd: 200_000,
        updated_at: Timestamp::now(),
    };
    let wallet = Arc::new(TestWallet::new(1_000_000, fx.clone()));
    let signer = Arc::new(NostrSigner::new());
    let provider = DvmProvider::with_transport(
        AgentId::from("agent-dvm-wallet-fx"),
        transport.clone(),
        signer,
        Some(wallet),
        FxSource::Wallet,
        60,
    )
    .unwrap();

    let request = ComputeRequest {
        model: "test-model".to_string(),
        kind: ComputeKind::Complete,
        input: json!({ "prompt": "hello" }),
        stream: false,
        timeout_ms: None,
        idempotency_key: None,
        max_cost_usd: Some(150_000),
    };
    let _job_id = provider.submit(request).unwrap();

    let event = transport
        .published_events()
        .into_iter()
        .find(|event| event.kind == KIND_JOB_TEXT_GENERATION)
        .expect("job request");
    let bid_msats: u64 = find_tag_value(&event.tags, "bid")
        .and_then(|value| value.parse().ok())
        .expect("bid tag");
    assert_eq!(bid_msats, 30_000_000);
}

#[test]
fn test_dvm_container_payment_flow() {
    let transport = Arc::new(TestDvmTransport::new());
    let fx = FxRateSnapshot {
        sats_per_usd: 100_000,
        updated_at: Timestamp::now(),
    };
    let wallet = Arc::new(TestWallet::new(1_000_000, fx.clone()));
    let signer = Arc::new(NostrSigner::new());
    let provider = DvmContainerProvider::with_transport(
        AgentId::from("agent-dvm"),
        transport.clone(),
        signer,
        Some(wallet.clone()),
        FxSource::Fixed {
            sats_per_usd: fx.sats_per_usd,
        },
        60,
    )
    .unwrap();

    let request = ContainerRequest {
        kind: ContainerKind::Build,
        image: None,
        repo: Some(RepoConfig {
            url: "https://example.com/repo.git".to_string(),
            git_ref: "main".to_string(),
            subdir: None,
            auth: None,
        }),
        commands: vec!["echo hello".to_string()],
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(100_000),
        idempotency_key: None,
        timeout_ms: None,
    };

    let session_id = provider.submit(request).unwrap();
    let request_event_id = transport
        .published_events()
        .iter()
        .find(|event| event.kind == KIND_JOB_SANDBOX_RUN)
        .map(|event| event.id.clone())
        .unwrap();

    let partial_event = dummy_event(
        KIND_JOB_FEEDBACK,
        vec![
            vec!["status".to_string(), "partial".to_string()],
            vec!["e".to_string(), request_event_id.clone()],
        ],
        "log-1",
    );
    transport.emit(partial_event);
    std::thread::sleep(Duration::from_millis(50));
    let chunk = provider.poll_output(&session_id).unwrap().unwrap();
    assert_eq!(chunk.data, "log-1");

    let run = SandboxRunResult::new(0).add_command_result(SandboxCommandResult {
        command: "echo hello".to_string(),
        exit_code: 0,
        stdout: "hello".to_string(),
        stderr: String::new(),
        duration_ms: 10,
    });
    let content = serde_json::to_string(&run).unwrap();
    let job_result = JobResult::new(
        KIND_JOB_SANDBOX_RUN,
        request_event_id.clone(),
        "customer",
        content,
    )
    .unwrap()
    .with_amount(1000, Some("lnbc1test".to_string()));
    let result_event = dummy_event(job_result.kind, job_result.to_tags(), &job_result.content);
    transport.emit(result_event);
    std::thread::sleep(Duration::from_millis(100));

    let state = provider.get_session(&session_id).unwrap();
    match state {
        SessionState::Complete(response) => {
            assert_eq!(response.exit_code, Some(0));
            assert_eq!(response.cost_usd, 10);
        }
        other => panic!("unexpected state: {:?}", other),
    }
    assert_eq!(wallet.payments().len(), 1);
}

#[test]
fn test_dvm_container_wallet_fx_source_bid() {
    let transport = Arc::new(TestDvmTransport::new());
    let fx = FxRateSnapshot {
        sats_per_usd: 200_000,
        updated_at: Timestamp::now(),
    };
    let wallet = Arc::new(TestWallet::new(1_000_000, fx.clone()));
    let signer = Arc::new(NostrSigner::new());
    let provider = DvmContainerProvider::with_transport(
        AgentId::from("agent-dvm-container-wallet-fx"),
        transport.clone(),
        signer,
        Some(wallet),
        FxSource::Wallet,
        60,
    )
    .unwrap();

    let request = ContainerRequest {
        kind: ContainerKind::Build,
        image: None,
        repo: Some(RepoConfig {
            url: "https://example.com/repo.git".to_string(),
            git_ref: "main".to_string(),
            subdir: None,
            auth: None,
        }),
        commands: vec!["echo hello".to_string()],
        workdir: None,
        env: HashMap::new(),
        limits: crate::containers::ResourceLimits::basic(),
        max_cost_usd: Some(150_000),
        idempotency_key: None,
        timeout_ms: None,
    };

    let _session_id = provider.submit(request).unwrap();
    let event = transport
        .published_events()
        .into_iter()
        .find(|event| event.kind == KIND_JOB_SANDBOX_RUN)
        .expect("sandbox request");
    let bid_msats: u64 = find_tag_value(&event.tags, "bid")
        .and_then(|value| value.parse().ok())
        .expect("bid tag");
    assert_eq!(bid_msats, 30_000_000);
}
