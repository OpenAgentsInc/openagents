//! Phase 6: Advanced Features
//!
//! - FROSTR integration for threshold-protected agent identity
//! - NIP-SA solver agent coordination
//! - Multi-backend support (Codex, OpenAI, local models)
//! - Agent cost tracking and budget enforcement

use crate::hooks::{Hook, HookResult, ToolCall, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    pub threshold: u32,
    pub total_signers: u32,
    pub signer_pubkeys: Vec<String>,
    pub group_pubkey: String,
}

impl ThresholdConfig {
    pub fn new(threshold: u32, total: u32) -> Self {
        Self {
            threshold,
            total_signers: total,
            signer_pubkeys: Vec::new(),
            group_pubkey: String::new(),
        }
    }

    pub fn with_signers(mut self, signers: Vec<String>) -> Self {
        self.signer_pubkeys = signers;
        self
    }

    pub fn with_group_pubkey(mut self, pubkey: String) -> Self {
        self.group_pubkey = pubkey;
        self
    }

    pub fn is_valid(&self) -> bool {
        self.threshold > 0
            && self.threshold <= self.total_signers
            && self.signer_pubkeys.len() == self.total_signers as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutonomyLevel {
    #[default]
    Supervised,
    SemiAutonomous,
    FullyAutonomous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub agent_pubkey: String,
    pub name: String,
    pub autonomy_level: AutonomyLevel,
    pub threshold_config: Option<ThresholdConfig>,
    pub operator_pubkey: Option<String>,
    pub model: String,
}

impl AgentIdentity {
    pub fn new(pubkey: &str, name: &str, model: &str) -> Self {
        Self {
            agent_pubkey: pubkey.to_string(),
            name: name.to_string(),
            autonomy_level: AutonomyLevel::default(),
            threshold_config: None,
            operator_pubkey: None,
            model: model.to_string(),
        }
    }

    pub fn with_threshold(mut self, config: ThresholdConfig) -> Self {
        self.threshold_config = Some(config);
        self
    }

    pub fn with_autonomy(mut self, level: AutonomyLevel) -> Self {
        self.autonomy_level = level;
        self
    }

    pub fn with_operator(mut self, pubkey: &str) -> Self {
        self.operator_pubkey = Some(pubkey.to_string());
        self
    }

    pub fn is_threshold_protected(&self) -> bool {
        self.threshold_config.as_ref().is_some_and(|c| c.is_valid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendProvider {
    Codex,
    OpenAI,
    Codex,
    GptOss,
    Local,
}

impl BackendProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendProvider::Codex => "codex",
            BackendProvider::OpenAI => "openai",
            BackendProvider::Codex => "codex",
            BackendProvider::GptOss => "gpt-oss",
            BackendProvider::Local => "local",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub provider: BackendProvider,
    pub model: String,
    pub endpoint: Option<String>,
    pub cost_per_1k_input: u64,
    pub cost_per_1k_output: u64,
    pub enabled: bool,
}

impl BackendConfig {
    pub fn codex(model: &str) -> Self {
        Self {
            provider: BackendProvider::Codex,
            model: model.to_string(),
            endpoint: None,
            cost_per_1k_input: 3,
            cost_per_1k_output: 15,
            enabled: true,
        }
    }

    pub fn openai(model: &str) -> Self {
        Self {
            provider: BackendProvider::OpenAI,
            model: model.to_string(),
            endpoint: None,
            cost_per_1k_input: 5,
            cost_per_1k_output: 15,
            enabled: true,
        }
    }

    pub fn local(model: &str, endpoint: &str) -> Self {
        Self {
            provider: BackendProvider::Local,
            model: model.to_string(),
            endpoint: Some(endpoint.to_string()),
            cost_per_1k_input: 0,
            cost_per_1k_output: 0,
            enabled: true,
        }
    }

    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        (input_tokens * self.cost_per_1k_input / 1000)
            + (output_tokens * self.cost_per_1k_output / 1000)
    }
}

pub struct MultiBackendRouter {
    backends: HashMap<BackendProvider, BackendConfig>,
    default_backend: BackendProvider,
    agent_backends: HashMap<String, BackendProvider>,
}

impl MultiBackendRouter {
    pub fn new(default: BackendProvider) -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: default,
            agent_backends: HashMap::new(),
        }
    }

    pub fn add_backend(mut self, config: BackendConfig) -> Self {
        self.backends.insert(config.provider, config);
        self
    }

    pub fn route_agent(mut self, agent_name: &str, provider: BackendProvider) -> Self {
        self.agent_backends.insert(agent_name.to_string(), provider);
        self
    }

    pub fn get_backend(&self, agent_name: &str) -> Option<&BackendConfig> {
        let provider = self
            .agent_backends
            .get(agent_name)
            .copied()
            .unwrap_or(self.default_backend);

        self.backends.get(&provider).filter(|b| b.enabled)
    }

    pub fn list_enabled(&self) -> Vec<BackendProvider> {
        self.backends
            .values()
            .filter(|b| b.enabled)
            .map(|b| b.provider)
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecord {
    pub agent_name: String,
    pub backend: BackendProvider,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_sats: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub daily_limit_sats: u64,
    pub session_limit_sats: u64,
    pub warn_threshold_pct: u8,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            daily_limit_sats: 100_000,
            session_limit_sats: 10_000,
            warn_threshold_pct: 80,
        }
    }
}

pub struct CostTracker {
    records: RwLock<Vec<CostRecord>>,
    budgets: HashMap<String, BudgetConfig>,
    default_budget: BudgetConfig,
}

impl CostTracker {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(Vec::new()),
            budgets: HashMap::new(),
            default_budget: BudgetConfig::default(),
        }
    }

    pub fn with_budget(mut self, agent_name: &str, budget: BudgetConfig) -> Self {
        self.budgets.insert(agent_name.to_string(), budget);
        self
    }

    pub fn with_default_budget(mut self, budget: BudgetConfig) -> Self {
        self.default_budget = budget;
        self
    }

    pub fn record(&self, record: CostRecord) -> Result<(), String> {
        let mut guard = self.records.write().map_err(|e| e.to_string())?;
        guard.push(record);
        Ok(())
    }

    pub fn get_budget(&self, agent_name: &str) -> &BudgetConfig {
        self.budgets.get(agent_name).unwrap_or(&self.default_budget)
    }

    pub fn session_total(&self, agent_name: &str, session_start: u64) -> u64 {
        let guard = match self.records.read() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        guard
            .iter()
            .filter(|r| r.agent_name == agent_name && r.timestamp >= session_start)
            .map(|r| r.cost_sats)
            .sum()
    }

    pub fn daily_total(&self, agent_name: &str) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let day_start = now - (now % 86400);

        let guard = match self.records.read() {
            Ok(g) => g,
            Err(_) => return 0,
        };

        guard
            .iter()
            .filter(|r| r.agent_name == agent_name && r.timestamp >= day_start)
            .map(|r| r.cost_sats)
            .sum()
    }

    pub fn check_budget(&self, agent_name: &str, session_start: u64) -> BudgetStatus {
        let budget = self.get_budget(agent_name);
        let session = self.session_total(agent_name, session_start);
        let daily = self.daily_total(agent_name);

        if session >= budget.session_limit_sats {
            return BudgetStatus::SessionExceeded;
        }
        if daily >= budget.daily_limit_sats {
            return BudgetStatus::DailyExceeded;
        }

        let session_pct = (session * 100 / budget.session_limit_sats.max(1)) as u8;
        let daily_pct = (daily * 100 / budget.daily_limit_sats.max(1)) as u8;

        if session_pct >= budget.warn_threshold_pct || daily_pct >= budget.warn_threshold_pct {
            return BudgetStatus::Warning {
                session_pct,
                daily_pct,
            };
        }

        BudgetStatus::Ok
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetStatus {
    Ok,
    Warning { session_pct: u8, daily_pct: u8 },
    SessionExceeded,
    DailyExceeded,
}

pub struct CostTrackingHook {
    tracker: Arc<CostTracker>,
    router: Arc<MultiBackendRouter>,
    session_start: u64,
}

impl CostTrackingHook {
    pub fn new(tracker: Arc<CostTracker>, router: Arc<MultiBackendRouter>) -> Self {
        Self {
            tracker,
            router,
            session_start: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }
}

#[async_trait]
impl Hook for CostTrackingHook {
    fn name(&self) -> &str {
        "cost-tracking"
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        if let Some(agent) = call.parameters.get("agent").and_then(|v| v.as_str()) {
            let status = self.tracker.check_budget(agent, self.session_start);
            match status {
                BudgetStatus::SessionExceeded => {
                    return HookResult::Block {
                        message: format!("Session budget exceeded for agent '{}'", agent),
                    };
                }
                BudgetStatus::DailyExceeded => {
                    return HookResult::Block {
                        message: format!("Daily budget exceeded for agent '{}'", agent),
                    };
                }
                BudgetStatus::Warning {
                    session_pct,
                    daily_pct,
                } => {
                    tracing::warn!(
                        "Budget warning for '{}': session {}%, daily {}%",
                        agent,
                        session_pct,
                        daily_pct
                    );
                }
                BudgetStatus::Ok => {}
            }
        }
        HookResult::Continue
    }

    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        if let Some(agent) = call.parameters.get("agent").and_then(|v| v.as_str())
            && let Some(backend) = self.router.get_backend(agent)
        {
            let input_tokens = call
                .parameters
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let output_tokens = output.content.len() as u64;
            let cost = backend.calculate_cost(input_tokens, output_tokens);

            let record = CostRecord {
                agent_name: agent.to_string(),
                backend: backend.provider,
                input_tokens,
                output_tokens,
                cost_sats: cost,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };

            if let Err(e) = self.tracker.record(record) {
                tracing::warn!("Failed to record cost: {}", e);
            }
        }
        HookResult::Continue
    }
}

pub struct SolverAgentCoordinator {
    identity: AgentIdentity,
    pending_approvals: RwLock<Vec<PendingApproval>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    pub request_id: String,
    pub action_type: String,
    pub description: String,
    pub cost_estimate_sats: u64,
    pub created_at: u64,
}

impl SolverAgentCoordinator {
    pub fn new(identity: AgentIdentity) -> Self {
        Self {
            identity,
            pending_approvals: RwLock::new(Vec::new()),
        }
    }

    pub fn requires_approval(&self, _action: &str, cost: u64) -> bool {
        match self.identity.autonomy_level {
            AutonomyLevel::FullyAutonomous => false,
            AutonomyLevel::SemiAutonomous => cost > 1000,
            AutonomyLevel::Supervised => true,
        }
    }

    pub fn request_approval(&self, action: &str, description: &str, cost: u64) -> String {
        let request_id = format!("approval-{}", uuid_v4());

        let approval = PendingApproval {
            request_id: request_id.clone(),
            action_type: action.to_string(),
            description: description.to_string(),
            cost_estimate_sats: cost,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };

        if let Ok(mut guard) = self.pending_approvals.write() {
            guard.push(approval);
        }

        request_id
    }

    pub fn list_pending(&self) -> Vec<PendingApproval> {
        self.pending_approvals
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    pub fn approve(&self, request_id: &str) -> bool {
        if let Ok(mut guard) = self.pending_approvals.write()
            && let Some(pos) = guard.iter().position(|a| a.request_id == request_id)
        {
            guard.remove(pos);
            return true;
        }
        false
    }

    pub fn reject(&self, request_id: &str) -> bool {
        self.approve(request_id)
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:016x}", now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_config_valid() {
        let config = ThresholdConfig::new(2, 3).with_signers(vec![
            "pk1".to_string(),
            "pk2".to_string(),
            "pk3".to_string(),
        ]);
        assert!(config.is_valid());
    }

    #[test]
    fn test_threshold_config_invalid() {
        let config = ThresholdConfig::new(3, 2);
        assert!(!config.is_valid());

        let config = ThresholdConfig::new(2, 3).with_signers(vec!["pk1".to_string()]);
        assert!(!config.is_valid());
    }

    #[test]
    fn test_agent_identity() {
        let identity = AgentIdentity::new("pubkey", "TestAgent", "codex-sonnet-4")
            .with_autonomy(AutonomyLevel::SemiAutonomous)
            .with_operator("operator-pk");

        assert_eq!(identity.name, "TestAgent");
        assert_eq!(identity.autonomy_level, AutonomyLevel::SemiAutonomous);
        assert!(!identity.is_threshold_protected());
    }

    #[test]
    fn test_agent_identity_with_threshold() {
        let threshold = ThresholdConfig::new(2, 3).with_signers(vec![
            "pk1".to_string(),
            "pk2".to_string(),
            "pk3".to_string(),
        ]);

        let identity =
            AgentIdentity::new("pubkey", "TestAgent", "codex-sonnet-4").with_threshold(threshold);

        assert!(identity.is_threshold_protected());
    }

    #[test]
    fn test_backend_config_cost() {
        let config = BackendConfig::codex("sonnet-4");
        let cost = config.calculate_cost(1000, 500);
        assert_eq!(cost, 3 + 7);
    }

    #[test]
    fn test_multi_backend_router() {
        let router = MultiBackendRouter::new(BackendProvider::Codex)
            .add_backend(BackendConfig::codex("sonnet-4"))
            .add_backend(BackendConfig::openai("gpt-4"))
            .route_agent("oracle", BackendProvider::OpenAI);

        assert!(router.get_backend("sisyphus").is_some());
        assert_eq!(
            router.get_backend("sisyphus").unwrap().provider,
            BackendProvider::Codex
        );

        assert!(router.get_backend("oracle").is_some());
        assert_eq!(
            router.get_backend("oracle").unwrap().provider,
            BackendProvider::OpenAI
        );
    }

    #[test]
    fn test_cost_tracker() {
        let tracker = CostTracker::new();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let record = CostRecord {
            agent_name: "test-agent".to_string(),
            backend: BackendProvider::Codex,
            input_tokens: 1000,
            output_tokens: 500,
            cost_sats: 100,
            timestamp: now,
        };

        tracker.record(record).unwrap();
        assert_eq!(tracker.session_total("test-agent", now - 10), 100);
    }

    #[test]
    fn test_budget_status() {
        let tracker = CostTracker::new().with_budget(
            "test-agent",
            BudgetConfig {
                daily_limit_sats: 1000,
                session_limit_sats: 500,
                warn_threshold_pct: 80,
            },
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        assert_eq!(tracker.check_budget("test-agent", now), BudgetStatus::Ok);

        for i in 0..5 {
            let record = CostRecord {
                agent_name: "test-agent".to_string(),
                backend: BackendProvider::Codex,
                input_tokens: 100,
                output_tokens: 50,
                cost_sats: 100,
                timestamp: now + i,
            };
            tracker.record(record).unwrap();
        }

        let status = tracker.check_budget("test-agent", now);
        assert!(matches!(status, BudgetStatus::SessionExceeded));
    }

    #[test]
    fn test_solver_coordinator_approval() {
        let identity = AgentIdentity::new("pk", "Agent", "codex");
        let coordinator = SolverAgentCoordinator::new(identity);

        let id = coordinator.request_approval("purchase_skill", "Buy web-scraper skill", 500);
        assert!(!id.is_empty());

        let pending = coordinator.list_pending();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].action_type, "purchase_skill");

        assert!(coordinator.approve(&id));
        assert!(coordinator.list_pending().is_empty());
    }

    #[test]
    fn test_autonomy_requires_approval() {
        let supervised =
            AgentIdentity::new("pk", "A", "m").with_autonomy(AutonomyLevel::Supervised);
        let coord_s = SolverAgentCoordinator::new(supervised);
        assert!(coord_s.requires_approval("any", 1));

        let semi = AgentIdentity::new("pk", "A", "m").with_autonomy(AutonomyLevel::SemiAutonomous);
        let coord_semi = SolverAgentCoordinator::new(semi);
        assert!(!coord_semi.requires_approval("any", 500));
        assert!(coord_semi.requires_approval("any", 1500));

        let auto = AgentIdentity::new("pk", "A", "m").with_autonomy(AutonomyLevel::FullyAutonomous);
        let coord_auto = SolverAgentCoordinator::new(auto);
        assert!(!coord_auto.requires_approval("any", 10000));
    }
}
