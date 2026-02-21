//! Browser (WASM) runtime with OpenAgents API-backed providers.

use crate::agent::Agent;
use crate::budget::BudgetPolicy;
use crate::compute::{
    ApiTokenProvider, ComputeFs, ComputePolicy, ComputeRouter, OpenAgentsComputeProvider,
};
use crate::containers::{
    ContainerFs, ContainerPolicy, ContainerRouter, OpenAgentsAuth, WasmOpenAgentsContainerProvider,
};
use crate::env::AgentEnv;
use crate::envelope::Envelope;
use crate::error::Result;
use crate::fs::AccessLevel;
use crate::idempotency::MemoryJournal;
use crate::identity::{InMemorySigner, SigningService};
use crate::storage::{AgentStorage, IndexedDbStorage};
use crate::tick::TickResult;
use crate::trigger::Trigger;
use crate::types::{AgentId, EnvelopeId, Timestamp};
use crate::{StatusSnapshot, TickEngine, manual_trigger};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const DEFAULT_TICK_BUDGET_USD: u64 = 500_000;
const DEFAULT_DAY_BUDGET_USD: u64 = 50_000_000;
const DEFAULT_APPROVAL_USD: u64 = 5_000_000;

fn default_budget_policy() -> BudgetPolicy {
    BudgetPolicy {
        per_tick_usd: DEFAULT_TICK_BUDGET_USD,
        per_day_usd: DEFAULT_DAY_BUDGET_USD,
        approval_threshold_usd: DEFAULT_APPROVAL_USD,
        approvers: Vec::new(),
    }
}

/// Browser runtime configuration.
pub struct BrowserRuntimeConfig {
    /// OpenAgents API base URL.
    pub api_base_url: String,
    /// Compute provider id to register.
    pub compute_provider_id: String,
    /// Container provider ids to register.
    pub container_provider_ids: Vec<String>,
    /// Compute policy (mounted at `/compute`).
    pub compute_policy: ComputePolicy,
    /// Container policy (mounted at `/containers`).
    pub container_policy: ContainerPolicy,
    /// Budget policy for `/compute`.
    pub compute_budget: BudgetPolicy,
    /// Budget policy for `/containers`.
    pub container_budget: BudgetPolicy,
    /// Agent storage backend (default: IndexedDB).
    pub storage: Arc<dyn AgentStorage>,
    /// Signing service for identity + OpenAgents auth.
    pub signer: Arc<dyn SigningService>,
}

impl BrowserRuntimeConfig {
    /// Create a browser runtime configuration with OpenAgents defaults.
    pub fn new(api_base_url: impl Into<String>) -> Self {
        let mut container_policy = ContainerPolicy::default();
        container_policy.require_api_auth = true;
        let budget = default_budget_policy();
        Self {
            api_base_url: api_base_url.into(),
            compute_provider_id: "cloudflare".to_string(),
            container_provider_ids: vec!["cloudflare".to_string()],
            compute_policy: ComputePolicy::default(),
            container_policy,
            compute_budget: budget.clone(),
            container_budget: budget,
            storage: Arc::new(IndexedDbStorage::new("runtime")),
            signer: Arc::new(InMemorySigner::new()),
        }
    }
}

/// Browser runtime registry with in-process agents.
pub struct BrowserRuntime {
    config: BrowserRuntimeConfig,
    storage: Arc<dyn AgentStorage>,
    signer: Arc<dyn SigningService>,
    tick_engine: TickEngine,
    agents: Arc<Mutex<HashMap<AgentId, Arc<AgentEntry>>>>,
}

impl BrowserRuntime {
    /// Create a browser runtime from configuration.
    pub fn new(config: BrowserRuntimeConfig) -> Self {
        let storage = config.storage.clone();
        let signer = config.signer.clone();
        Self {
            tick_engine: TickEngine::new(storage.clone()),
            storage,
            signer,
            config,
            agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register an agent instance with the runtime.
    pub fn register_agent<A: Agent>(&self, id: AgentId, agent: A) -> Result<()> {
        let env = self.build_env(&id);
        let entry = Arc::new(AgentEntry::new(id, env, agent));
        let mut guard = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(entry.id.clone(), entry.clone());
        entry.refresh_status();
        Ok(())
    }

    /// List all registered agent ids.
    pub fn list_agents(&self) -> Vec<AgentId> {
        let guard = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        guard.keys().cloned().collect()
    }

    /// Fetch an agent environment.
    pub fn env(&self, id: &AgentId) -> Option<Arc<AgentEnv>> {
        self.entry(id).map(|entry| entry.env.clone())
    }

    /// Send an envelope to an agent's inbox.
    pub fn send_envelope(&self, id: &AgentId, envelope: Envelope) -> Result<()> {
        let entry = self
            .entry(id)
            .ok_or_else(|| "agent not found".to_string())?;
        let data = serde_json::to_vec(&envelope)?;
        entry
            .env
            .write("/inbox", &data)
            .map_err(|err| err.to_string())?;
        entry.refresh_status();
        Ok(())
    }

    /// Trigger a tick with a custom trigger.
    pub async fn tick(&self, id: &AgentId, trigger: Trigger) -> Result<TickResult> {
        let entry = self
            .entry(id)
            .ok_or_else(|| "agent not found".to_string())?;
        let cause = trigger_source(&trigger).to_string();
        let result = entry
            .agent
            .tick(&self.tick_engine, entry.id.clone(), trigger)
            .await?;
        entry.record_tick(&cause);
        Ok(result)
    }

    /// Trigger a manual tick for an agent.
    pub async fn tick_manual(&self, id: &AgentId) -> Result<TickResult> {
        let entry = self
            .entry(id)
            .ok_or_else(|| "agent not found".to_string())?;
        let envelope_id = EnvelopeId::new(Uuid::new_v4().to_string());
        let trigger = manual_trigger(envelope_id, "browser");
        let result = entry
            .agent
            .tick(&self.tick_engine, entry.id.clone(), trigger)
            .await?;
        entry.record_tick("manual");
        Ok(result)
    }

    fn entry(&self, id: &AgentId) -> Option<Arc<AgentEntry>> {
        let guard = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        guard.get(id).cloned()
    }

    fn build_env(&self, agent_id: &AgentId) -> Arc<AgentEnv> {
        let mut env =
            AgentEnv::with_signer(agent_id.clone(), self.storage.clone(), self.signer.clone());

        let auth = Arc::new(OpenAgentsAuth::with_base_url(
            agent_id.clone(),
            self.storage.clone(),
            self.signer.clone(),
            self.config.api_base_url.clone(),
        ));

        let compute_policy = self.compute_policy();
        let container_policy = self.container_policy();

        let token_provider: Arc<dyn ApiTokenProvider> = auth.clone();
        let mut compute_router = ComputeRouter::new();
        compute_router.register(Arc::new(OpenAgentsComputeProvider::new(
            self.config.api_base_url.clone(),
            self.config.compute_provider_id.clone(),
            token_provider,
        )));
        let compute_fs = ComputeFs::new(
            agent_id.clone(),
            compute_router,
            compute_policy,
            self.config.compute_budget.clone(),
            Arc::new(MemoryJournal::new()),
        );
        env.mount("/compute", Arc::new(compute_fs), AccessLevel::ReadWrite);

        let mut container_router = ContainerRouter::new();
        for provider_id in &self.config.container_provider_ids {
            let provider: Arc<dyn crate::containers::ContainerProvider> = match provider_id.as_str()
            {
                "cloudflare" => Arc::new(WasmOpenAgentsContainerProvider::cloudflare(
                    self.config.api_base_url.clone(),
                    auth.clone(),
                )),
                "daytona" => Arc::new(WasmOpenAgentsContainerProvider::daytona(
                    self.config.api_base_url.clone(),
                    auth.clone(),
                )),
                _ => Arc::new(WasmOpenAgentsContainerProvider::new(
                    provider_id.clone(),
                    format!("OpenAgents ({})", provider_id),
                    self.config.api_base_url.clone(),
                    auth.clone(),
                )),
            };
            container_router.register(provider);
        }

        let container_fs = ContainerFs::with_auth(
            agent_id.clone(),
            container_router,
            container_policy,
            self.config.container_budget.clone(),
            Arc::new(MemoryJournal::new()),
            auth,
        );
        env.mount(
            "/containers",
            Arc::new(container_fs),
            AccessLevel::ReadWrite,
        );

        Arc::new(env)
    }

    fn compute_policy(&self) -> ComputePolicy {
        let mut policy = self.config.compute_policy.clone();
        if policy.allowed_providers.is_empty() {
            policy
                .allowed_providers
                .push(self.config.compute_provider_id.clone());
        }
        policy
    }

    fn container_policy(&self) -> ContainerPolicy {
        let mut policy = self.config.container_policy.clone();
        if policy.allowed_providers.is_empty() {
            policy.allowed_providers = self.config.container_provider_ids.clone();
        }
        policy
    }
}

fn trigger_source(trigger: &Trigger) -> &str {
    match trigger {
        Trigger::Message(t) => &t.meta.source,
        Trigger::Alarm(t) => &t.meta.source,
        Trigger::Event(t) => &t.meta.source,
        Trigger::Manual(t) => &t.meta.source,
        Trigger::Initialize(t) => &t.meta.source,
    }
}

struct AgentEntry {
    id: AgentId,
    env: Arc<AgentEnv>,
    agent: Arc<dyn DynAgent>,
    created_at: Timestamp,
    state: Mutex<AgentRuntimeState>,
}

impl AgentEntry {
    fn new<A: Agent>(id: AgentId, env: Arc<AgentEnv>, agent: A) -> Self {
        let created_at = Timestamp::now();
        Self {
            id,
            env,
            agent: Arc::new(AgentRunner::new(agent)),
            created_at,
            state: Mutex::new(AgentRuntimeState::new()),
        }
    }

    fn record_tick(&self, cause: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.tick_count += 1;
        state.last_tick_at = Some(Timestamp::now());
        state.last_tick_cause = Some(cause.to_string());
        drop(state);
        self.refresh_status();
    }

    fn refresh_status(&self) {
        let queue_depth = {
            let queue_ref = self.env.inbox.queue();
            let queue = queue_ref.lock().unwrap_or_else(|e| e.into_inner());
            queue.len() as u64
        };
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let payload = serde_json::json!({
            "state": "active",
            "created_at": self.created_at.as_millis(),
            "last_tick_at": state.last_tick_at.map(|t| t.as_millis()),
            "last_tick_cause": state.last_tick_cause.clone(),
            "tick_count": state.tick_count,
            "queue_depth": queue_depth,
        });
        let snapshot = StatusSnapshot {
            agent_id: self.id.clone(),
            payload,
        };
        self.env.status.set_snapshot(snapshot);
    }
}

#[derive(Default)]
struct AgentRuntimeState {
    tick_count: u64,
    last_tick_at: Option<Timestamp>,
    last_tick_cause: Option<String>,
}

impl AgentRuntimeState {
    fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
trait DynAgent: Send + Sync {
    async fn tick(
        &self,
        engine: &TickEngine,
        agent_id: AgentId,
        trigger: Trigger,
    ) -> Result<TickResult>;
}

struct AgentRunner<A: Agent> {
    agent: A,
}

impl<A: Agent> AgentRunner<A> {
    fn new(agent: A) -> Self {
        Self { agent }
    }
}

#[async_trait]
impl<A: Agent> DynAgent for AgentRunner<A> {
    async fn tick(
        &self,
        engine: &TickEngine,
        agent_id: AgentId,
        trigger: Trigger,
    ) -> Result<TickResult> {
        engine.tick(agent_id, &self.agent, trigger).await
    }
}
