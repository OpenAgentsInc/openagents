use crate::agent::{Agent, AgentContext, AgentState};
use crate::engine::{manual_trigger, TickEngine};
use crate::storage::{AgentStorage, InMemoryStorage, StoredState};
use crate::types::{AgentId, EnvelopeId, Timestamp};
use crate::{AgentEnv, Envelope, Result, TickResult, WatchEvent};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
