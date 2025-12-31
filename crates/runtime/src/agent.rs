//! Agent traits and execution context.

use crate::error::{AgentError, Result};
use crate::types::{AgentId, EnvelopeId};
use crate::trigger::Trigger;
use crate::tick::TickResult;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// A persistent autonomous entity with identity, memory, and agency.
pub trait Agent: Send + Sync + 'static {
    /// Agent's persistent state type.
    type State: AgentState;

    /// Agent's configuration type.
    type Config: AgentConfig;

    /// Called once when agent is first created.
    fn on_create(&self, _ctx: &mut AgentContext<Self::State>) -> Result<()> {
        Ok(())
    }

    /// Called when agent wakes from hibernation.
    fn on_wake(&self, _ctx: &mut AgentContext<Self::State>) -> Result<()> {
        Ok(())
    }

    /// Called before agent hibernates.
    fn on_sleep(&self, _ctx: &mut AgentContext<Self::State>) -> Result<()> {
        Ok(())
    }

    /// Called when agent receives a trigger (message, alarm, event).
    fn on_trigger(
        &self,
        ctx: &mut AgentContext<Self::State>,
        trigger: Trigger,
    ) -> Result<TickResult>;

    /// Called when an error occurs during tick execution.
    fn on_error(&self, _ctx: &mut AgentContext<Self::State>, _error: &AgentError) {}

    /// Called before agent is terminated.
    fn on_terminate(&self, _ctx: &mut AgentContext<Self::State>) -> Result<()> {
        Ok(())
    }
}

/// Marker trait for agent state types.
pub trait AgentState: Serialize + DeserializeOwned + Default + Send + Sync {
    /// Version for schema migration.
    fn version() -> u32 {
        1
    }

    /// Migrate from previous version.
    fn migrate(from_version: u32, data: &[u8]) -> Result<Self> {
        if from_version == Self::version() {
            Ok(serde_json::from_slice(data)?)
        } else {
            Err(AgentError::StateMigrationRequired {
                from_version,
                to_version: Self::version(),
            })
        }
    }
}

/// Configuration for agent behavior.
pub trait AgentConfig: Serialize + DeserializeOwned + Default + Send + Sync {
    /// Validate configuration.
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

impl AgentConfig for () {}

impl AgentState for () {}

/// Execution context for agent ticks.
pub struct AgentContext<S: AgentState> {
    /// Mutable access to agent state.
    pub state: S,
    agent_id: AgentId,
    pub(crate) seen_cache: SeenCache,
}

impl<S: AgentState> AgentContext<S> {
    pub(crate) fn new(agent_id: AgentId, state: S, seen_cache: SeenCache) -> Self {
        Self {
            state,
            agent_id,
            seen_cache,
        }
    }

    /// Agent's unique id.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Check if an envelope id was already processed.
    pub fn seen(&self, envelope_id: &EnvelopeId) -> bool {
        self.seen_cache.contains(envelope_id)
    }

    /// Mark an envelope id as processed.
    pub fn mark_seen(&mut self, envelope_id: EnvelopeId) {
        self.seen_cache.insert(envelope_id);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SeenCacheState {
    pub entries: Vec<EnvelopeId>,
}

#[derive(Clone, Debug)]
pub(crate) struct SeenCache {
    capacity: usize,
    order: VecDeque<EnvelopeId>,
    set: HashSet<EnvelopeId>,
}

impl SeenCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::with_capacity(capacity),
            set: HashSet::with_capacity(capacity),
        }
    }

    pub fn from_entries(entries: Vec<EnvelopeId>, capacity: usize) -> Self {
        let mut cache = SeenCache::new(capacity);
        for entry in entries {
            cache.insert(entry);
        }
        cache
    }

    pub fn contains(&self, envelope_id: &EnvelopeId) -> bool {
        self.set.contains(envelope_id)
    }

    pub fn insert(&mut self, envelope_id: EnvelopeId) {
        if self.set.contains(&envelope_id) {
            return;
        }

        self.order.push_back(envelope_id.clone());
        self.set.insert(envelope_id);

        if self.order.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            }
        }
    }

    pub fn entries(&self) -> Vec<EnvelopeId> {
        self.order.iter().cloned().collect()
    }
}
