//! Tick execution engine and helpers.

use crate::agent::{Agent, AgentContext, AgentState, SeenCache, SeenCacheState};
use crate::error::Result;
use crate::storage::{AgentStorage, StorageOp, StoredState};
use crate::tick::TickResult;
use crate::trigger::Trigger;
use crate::types::{AgentId, EnvelopeId, Timestamp};
use futures::lock::Mutex as AsyncMutex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SEEN_CACHE_KEY: &str = "__runtime_seen_cache";
const DEFAULT_SEEN_CAPACITY: usize = 1024;

/// Executes ticks with per-agent locking and state persistence.
pub struct TickEngine {
    storage: Arc<dyn AgentStorage>,
    locks: AgentLockTable,
    seen_capacity: usize,
}

impl TickEngine {
    /// Create a tick engine with default seen-cache capacity.
    pub fn new(storage: Arc<dyn AgentStorage>) -> Self {
        Self {
            storage,
            locks: AgentLockTable::new(),
            seen_capacity: DEFAULT_SEEN_CAPACITY,
        }
    }

    /// Create a tick engine with custom seen-cache capacity.
    pub fn with_seen_capacity(storage: Arc<dyn AgentStorage>, seen_capacity: usize) -> Self {
        Self {
            storage,
            locks: AgentLockTable::new(),
            seen_capacity: seen_capacity.max(1),
        }
    }

    /// Execute a single tick for the given agent.
    pub async fn tick<A: Agent>(
        &self,
        agent_id: AgentId,
        agent: &A,
        trigger: Trigger,
    ) -> Result<TickResult> {
        let lock = self.locks.lock_for(&agent_id);
        let _guard = lock.lock().await;
        let start = Instant::now();

        let (state, seen_cache, is_new) = self.load_state::<A::State>(&agent_id).await?;
        let mut ctx = AgentContext::new(agent_id.clone(), state, seen_cache);

        if is_new {
            agent.on_create(&mut ctx)?;
        }

        agent.on_wake(&mut ctx)?;

        let mut result = match agent.on_trigger(&mut ctx, trigger) {
            Ok(result) => result,
            Err(err) => {
                agent.on_error(&mut ctx, &err);
                return Err(err);
            }
        };

        agent.on_sleep(&mut ctx)?;

        result.duration = start.elapsed();

        self.commit_state(&agent_id, &ctx).await?;
        Ok(result)
    }

    async fn load_state<S: AgentState>(&self, agent_id: &AgentId) -> Result<(S, SeenCache, bool)> {
        let state_bytes = self.storage.load_state(agent_id).await?;
        let (state, is_new) = match state_bytes {
            Some(bytes) => (StoredState::decode::<S>(&bytes)?, false),
            None => (S::default(), true),
        };

        let seen_bytes = self.storage.get(agent_id, SEEN_CACHE_KEY).await?;
        let seen_cache = match seen_bytes {
            Some(bytes) => {
                let state: SeenCacheState = serde_json::from_slice(&bytes)?;
                SeenCache::from_entries(state.entries, self.seen_capacity)
            }
            None => SeenCache::new(self.seen_capacity),
        };

        Ok((state, seen_cache, is_new))
    }

    async fn commit_state<S: AgentState>(
        &self,
        agent_id: &AgentId,
        ctx: &AgentContext<S>,
    ) -> Result<()> {
        let state_bytes = StoredState::encode(&ctx.state)?;
        let seen_state = SeenCacheState {
            entries: ctx.seen_cache.entries(),
        };
        let seen_bytes = serde_json::to_vec(&seen_state)?;

        self.storage
            .transaction(
                agent_id,
                vec![
                    StorageOp::SetState { state: state_bytes },
                    StorageOp::Set {
                        key: SEEN_CACHE_KEY.to_string(),
                        value: seen_bytes,
                    },
                ],
            )
            .await?;

        Ok(())
    }
}

struct AgentLockTable {
    inner: Mutex<HashMap<AgentId, Arc<AsyncMutex<()>>>>,
}

impl AgentLockTable {
    fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    fn lock_for(&self, agent_id: &AgentId) -> Arc<AsyncMutex<()>> {
        let mut map = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(agent_id.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }
}

/// Minimal helper to build manual triggers in tests or callers.
pub fn manual_trigger(envelope_id: EnvelopeId, source: impl Into<String>) -> Trigger {
    Trigger::Manual(crate::trigger::ManualTrigger {
        meta: crate::trigger::TriggerMeta {
            envelope_id,
            source: source.into(),
            seq: None,
            created_at: Timestamp::now(),
        },
        invoked_by: None,
        reason: None,
    })
}
