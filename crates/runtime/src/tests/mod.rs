use crate::agent::{Agent, AgentContext, AgentState};
use crate::budget::BudgetPolicy;
use crate::compute::{
    ComputeChunk, ComputeError, ComputeFs, ComputeKind, ComputePolicy, ComputeProvider,
    ComputeRequest, ComputeResponse, ComputeRouter, JobState, ModelInfo, ProviderInfo,
    ProviderLatency, ProviderStatus,
};
use crate::engine::{manual_trigger, TickEngine};
use crate::fs::{AccessLevel, FileHandle, FileService, FsError, FsResult, OpenFlags, Stat};
use crate::idempotency::{IdempotencyJournal, MemoryJournal};
use crate::storage::{AgentStorage, InMemoryStorage, StoredState};
use crate::types::{AgentId, EnvelopeId, Timestamp};
use crate::{AgentEnv, ControlPlane, Envelope, LocalRuntime, Result, TickResult, WatchEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

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

static JOB_COUNTER: AtomicUsize = AtomicUsize::new(1);

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
