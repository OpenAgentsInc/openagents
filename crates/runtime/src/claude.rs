//! Claude filesystem service and providers.

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::identity::{PublicKey, Signature, SigningService};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use crate::types::{AgentId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);
const AUTH_CHALLENGE_TTL: Duration = Duration::from_secs(300);

/// Claude session autonomy level (tool-approval mode).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeSessionAutonomy {
    /// Full autonomy - tools execute without approval.
    Full,
    /// Supervised - certain tools require approval.
    #[default]
    Supervised,
    /// Restricted - all tool use requires approval.
    Restricted,
    /// Read-only - no tool use allowed.
    ReadOnly,
}

/// Tool definition for Claude Agent SDK.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g., "Read", "Write", "Bash").
    pub name: String,
    /// Tool description.
    pub description: Option<String>,
    /// Tool-specific configuration.
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
struct ToolPolicy {
    allowed: Vec<String>,
    blocked: Vec<String>,
    approval_required: Vec<String>,
    autonomy: ClaudeSessionAutonomy,
}

#[derive(Debug, Clone, Default)]
struct ClaudeRequestInternal {
    tool_policy: ToolPolicy,
    fork: bool,
    resume_backend_id: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    container: Option<ClaudeContainerConfig>,
    #[cfg(not(target_arch = "wasm32"))]
    executable: Option<claude_agent_sdk::ExecutableConfig>,
}

/// A Claude session request (provider-agnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeRequest {
    /// Model identifier.
    pub model: String,
    /// System prompt for the session.
    pub system_prompt: Option<String>,
    /// Initial prompt/message to send.
    pub initial_prompt: Option<String>,
    /// Tools available to Claude in this session.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Maximum context tokens (for budget control).
    pub max_context_tokens: Option<u64>,
    /// Tunnel endpoint to use (required for remote execution).
    pub tunnel_endpoint: Option<String>,
    /// Maximum cost in micro-USD the caller is willing to pay.
    pub max_cost_usd: Option<u64>,
    /// Idempotency key for deduplication.
    pub idempotency_key: Option<String>,
    /// Session to resume (if continuing previous work).
    pub resume_session_id: Option<String>,
    /// Timeout in milliseconds for session creation.
    pub timeout_ms: Option<u64>,
    /// Autonomy level for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<ClaudeSessionAutonomy>,
    #[serde(skip, default)]
    internal: ClaudeRequestInternal,
}

impl ClaudeRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            system_prompt: None,
            initial_prompt: None,
            tools: Vec::new(),
            max_context_tokens: None,
            tunnel_endpoint: None,
            max_cost_usd: None,
            idempotency_key: None,
            resume_session_id: None,
            timeout_ms: None,
            autonomy: None,
            internal: ClaudeRequestInternal::default(),
        }
    }

    fn with_internal(mut self, internal: ClaudeRequestInternal) -> Self {
        self.internal = internal;
        self
    }
}

/// Claude session response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeResponse {
    /// Session identifier.
    pub session_id: String,
    /// Session status.
    pub status: ClaudeSessionStatus,
    /// Latest response text (if any).
    pub response: Option<String>,
    /// Token usage.
    pub usage: Option<ClaudeUsage>,
    /// Actual cost in micro-USD (post-execution).
    pub cost_usd: u64,
    /// Reserved cost (from max_cost_usd at submission).
    pub reserved_usd: u64,
    /// Provider that handled the request.
    pub provider_id: String,
    /// Model used.
    pub model: String,
    /// Tunnel endpoint used (if applicable).
    pub tunnel_endpoint: Option<String>,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
}

/// A streaming chunk from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeChunk {
    /// Session identifier.
    pub session_id: String,
    /// Chunk type.
    pub chunk_type: ChunkType,
    /// Delta content (for text chunks).
    pub delta: Option<String>,
    /// Tool information (for tool chunks).
    pub tool: Option<ToolChunk>,
    /// Accumulated usage.
    pub usage: Option<ClaudeUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    /// Text output from Claude.
    Text,
    /// Tool execution starting.
    ToolStart,
    /// Tool execution output.
    ToolOutput,
    /// Tool execution complete.
    ToolDone,
    /// Claude finished response.
    Done,
    /// Error occurred.
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChunk {
    pub name: String,
    pub params: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Claude session status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeSessionStatus {
    /// Session is being created.
    Creating,
    /// Session is ready, waiting for prompts.
    Ready,
    /// Claude is actively working.
    Working,
    /// Claude finished current task, waiting for next prompt.
    Idle,
    /// Session completed.
    Complete,
    /// Session failed.
    Failed { error: String },
    /// Waiting for tool approval.
    PendingApproval { tool: String, params: serde_json::Value },
}

/// Session state for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionState {
    /// Session being created.
    Creating { started_at: Timestamp },
    /// Ready for prompts.
    Ready { created_at: Timestamp },
    /// Actively working.
    Working { started_at: Timestamp, current_tool: Option<String> },
    /// Waiting for next prompt.
    Idle {
        last_response_at: Timestamp,
        response: Option<String>,
        usage: Option<ClaudeUsage>,
        cost_usd: u64,
    },
    /// Completed.
    Complete(ClaudeResponse),
    /// Failed.
    Failed { error: String, at: Timestamp },
    /// Waiting for tool approval.
    PendingApproval {
        tool: String,
        params: serde_json::Value,
        since: Timestamp,
    },
}

impl SessionState {
    fn status(&self) -> ClaudeSessionStatus {
        match self {
            SessionState::Creating { .. } => ClaudeSessionStatus::Creating,
            SessionState::Ready { .. } => ClaudeSessionStatus::Ready,
            SessionState::Working { .. } => ClaudeSessionStatus::Working,
            SessionState::Idle { .. } => ClaudeSessionStatus::Idle,
            SessionState::Complete(response) => response.status.clone(),
            SessionState::Failed { error, .. } => ClaudeSessionStatus::Failed { error: error.clone() },
            SessionState::PendingApproval { tool, params, .. } => {
                ClaudeSessionStatus::PendingApproval {
                    tool: tool.clone(),
                    params: params.clone(),
                }
            }
        }
    }
}

/// Policy for Claude sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePolicy {
    /// Allowed provider IDs (empty = all available).
    #[serde(default)]
    pub allowed_providers: Vec<String>,
    /// Allowed models (empty = all).
    #[serde(default)]
    pub allowed_models: Vec<String>,
    /// Blocked models (takes precedence over allowed).
    #[serde(default)]
    pub blocked_models: Vec<String>,
    /// Allowed tools for Claude to use.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Blocked tools (takes precedence).
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    /// Tools that require approval before execution.
    #[serde(default)]
    pub approval_required_tools: Vec<String>,
    /// Maximum cost per tick in micro-USD.
    pub max_cost_usd_per_tick: Option<u64>,
    /// Maximum cost per day in micro-USD.
    pub max_cost_usd_per_day: Option<u64>,
    /// Default max_cost_usd if request doesn't specify.
    pub default_max_cost_usd: Option<u64>,
    /// Require requests to specify max_cost_usd.
    #[serde(default)]
    pub require_max_cost: bool,
    /// Maximum concurrent Claude sessions per agent.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    /// Maximum context tokens per session.
    pub max_context_tokens: Option<u64>,
    /// Default autonomy level for sessions.
    #[serde(default)]
    pub default_autonomy: ClaudeSessionAutonomy,
    /// Require idempotency keys for all requests.
    #[serde(default)]
    pub require_idempotency: bool,
    /// Allowed tunnel endpoints (empty = all configured).
    #[serde(default)]
    pub allowed_tunnels: Vec<String>,
    /// Worker isolation mode.
    #[serde(default)]
    pub isolation_mode: IsolationMode,
    /// Network access mode.
    #[serde(default)]
    pub network_mode: NetworkMode,
    /// Repo filtering mode.
    #[serde(default)]
    pub repo_filter: RepoFilterMode,
    /// Proxy domain allowlist.
    #[serde(default)]
    pub proxy_allowlist: Vec<String>,
}

impl Default for ClaudePolicy {
    fn default() -> Self {
        Self {
            allowed_providers: Vec::new(),
            allowed_models: Vec::new(),
            blocked_models: Vec::new(),
            allowed_tools: Vec::new(),
            blocked_tools: Vec::new(),
            approval_required_tools: Vec::new(),
            max_cost_usd_per_tick: None,
            max_cost_usd_per_day: None,
            default_max_cost_usd: None,
            require_max_cost: false,
            max_concurrent: default_max_concurrent(),
            max_context_tokens: None,
            default_autonomy: ClaudeSessionAutonomy::Supervised,
            require_idempotency: false,
            allowed_tunnels: Vec::new(),
            isolation_mode: IsolationMode::Container,
            network_mode: NetworkMode::ProxyOnly,
            repo_filter: RepoFilterMode::Standard,
            proxy_allowlist: Vec::new(),
        }
    }
}

fn default_max_concurrent() -> u32 {
    3
}

/// Worker isolation mode.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    /// Process on host (dev only).
    Local,
    /// Docker container with hardening.
    #[default]
    Container,
    /// gVisor sandbox.
    Gvisor,
    /// Firecracker microVM.
    Firecracker,
}

/// Network access mode.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// No network restrictions (dev only).
    Host,
    /// Network disabled, only proxy socket.
    #[default]
    ProxyOnly,
    /// Completely air-gapped.
    None,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeContainerRuntime {
    Apple,
    Docker,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct ClaudeContainerConfig {
    runtime: ClaudeContainerRuntime,
    image: String,
    network_mode: NetworkMode,
    proxy_url: Option<String>,
    command: Option<String>,
}

/// Repo filtering mode.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoFilterMode {
    /// No filtering (dangerous).
    None,
    /// Standard denylist.
    #[default]
    Standard,
    /// Strict denylist + allowlist.
    Strict,
    /// Custom filter rules.
    Custom { denylist: Vec<String>, allowlist: Vec<String> },
}

/// Usage state for Claude budgets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaudeUsageState {
    pub reserved_tick_usd: u64,
    pub spent_tick_usd: u64,
    pub reserved_day_usd: u64,
    pub spent_day_usd: u64,
}

/// Provider availability status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClaudeProviderStatus {
    /// Provider is available.
    Available,
    /// Provider is available but degraded.
    Degraded { reason: String },
    /// Provider is unavailable.
    Unavailable { reason: String },
}

/// Per-model pricing overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeModelPricing {
    pub input_per_1k_microusd: u64,
    pub output_per_1k_microusd: u64,
    pub cache_read_per_1k_microusd: u64,
    pub cache_write_per_1k_microusd: u64,
}

/// Claude provider capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCapabilities {
    pub streaming: bool,
    pub resume: bool,
    pub fork: bool,
    pub tools: bool,
    pub vision: bool,
}

/// Claude model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeModelInfo {
    pub id: String,
    pub name: String,
    pub context_length: u64,
    pub output_limit: u64,
    pub pricing: Option<ClaudeModelPricing>,
}

/// Claude pricing metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePricing {
    pub input_per_1k_microusd: u64,
    pub output_per_1k_microusd: u64,
    pub cache_read_per_1k_microusd: u64,
    pub cache_write_per_1k_microusd: u64,
}

/// Information about a Claude provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<ClaudeModelInfo>,
    pub capabilities: ClaudeCapabilities,
    pub pricing: Option<ClaudePricing>,
    pub status: ClaudeProviderStatus,
}

/// Tool approval log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLogEntry {
    pub tool_use_id: String,
    pub tool: String,
    pub params: serde_json::Value,
    pub approved: Option<bool>,
    pub error: Option<String>,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolInfo {
    pub tool_use_id: String,
    pub tool: String,
    pub params: serde_json::Value,
    pub requested_at: Timestamp,
}

/// Tunnel endpoint configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEndpoint {
    pub id: String,
    pub url: String,
    pub auth: TunnelAuth,
    #[serde(default)]
    pub allowed_agents: Vec<String>,
    pub rate_limit: Option<RateLimit>,
}

/// Authentication method for tunnels.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelAuth {
    None,
    Nostr { relay: Option<String> },
    Psk { secret_path: String },
}

impl TunnelAuth {
    fn type_name(&self) -> String {
        match self {
            TunnelAuth::None => "none".to_string(),
            TunnelAuth::Nostr { .. } => "nostr".to_string(),
            TunnelAuth::Psk { .. } => "psk".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub tokens_per_minute: u64,
}

/// Tunnel auth challenge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelAuthChallenge {
    pub challenge: String,
    pub expires_at: Timestamp,
    pub tunnel_id: String,
}

/// Tunnel auth response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelAuthResponse {
    pub challenge: String,
    pub signature: String,
    pub pubkey: String,
    pub tunnel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TunnelSummary {
    id: String,
    url: String,
    auth_type: String,
}

/// In-memory auth state for tunnel challenges/responses.
#[derive(Default)]
pub struct TunnelAuthState {
    /// Active challenges keyed by tunnel id.
    challenges: HashMap<String, TunnelAuthChallenge>,
    /// Signed responses keyed by tunnel id.
    responses: HashMap<String, TunnelAuthResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TunnelAuthStatus {
    tunnel_id: String,
    auth_type: String,
    authorized: bool,
    pubkey: Option<String>,
    challenge_expires_at: Option<Timestamp>,
}

/// Claude provider trait (sync for FileService compatibility).
pub trait ClaudeProvider: Send + Sync {
    fn id(&self) -> &str;
    fn info(&self) -> ClaudeProviderInfo;
    fn is_available(&self) -> bool;
    fn supports_model(&self, model: &str) -> bool;
    fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError>;
    fn get_session(&self, session_id: &str) -> Option<SessionState>;
    fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError>;
    fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError>;
    fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError>;
    fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError>;
    fn stop(&self, session_id: &str) -> Result<(), ClaudeError>;
    fn pause(&self, session_id: &str) -> Result<(), ClaudeError>;
    fn resume(&self, session_id: &str) -> Result<(), ClaudeError>;
    fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>>;
    fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo>;
}

/// Routes Claude requests to appropriate providers.
#[derive(Default)]
pub struct ClaudeRouter {
    providers: Vec<Arc<dyn ClaudeProvider>>,
}

impl ClaudeRouter {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Arc<dyn ClaudeProvider>) {
        self.providers.push(provider);
    }

    pub fn list_providers(&self) -> Vec<ClaudeProviderInfo> {
        self.providers.iter().map(|p| p.info()).collect()
    }

    pub fn provider_by_id(&self, id: &str) -> Option<Arc<dyn ClaudeProvider>> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .cloned()
    }

    pub fn select(
        &self,
        request: &ClaudeRequest,
        policy: &ClaudePolicy,
    ) -> Result<Arc<dyn ClaudeProvider>, ClaudeError> {
        let wants_tunnel = request.tunnel_endpoint.is_some();
        let candidates: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .filter(|p| p.supports_model(&request.model))
            .filter(|p| {
                policy.allowed_providers.is_empty()
                    || policy.allowed_providers.iter().any(|id| id == p.id())
            })
            .filter(|p| !wants_tunnel || p.id() == "tunnel")
            .filter(|_| {
                policy.allowed_models.is_empty()
                    || policy
                        .allowed_models
                        .iter()
                        .any(|pat| matches_pattern(pat, &request.model))
            })
            .filter(|_| {
                !policy
                    .blocked_models
                    .iter()
                    .any(|pat| matches_pattern(pat, &request.model))
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Err(ClaudeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "no provider matches policy filters".to_string(),
            });
        }

        Ok(candidates
            .into_iter()
            .next()
            .ok_or_else(|| ClaudeError::NoProviderAvailable {
                model: request.model.clone(),
                reason: "selection failed".to_string(),
            })?)
    }
}

#[derive(Clone)]
struct SessionRecord {
    provider_id: String,
    reservation: BudgetReservation,
    reconciled: bool,
}

/// Claude capability as a filesystem.
pub struct ClaudeFs {
    agent_id: AgentId,
    router: Arc<RwLock<ClaudeRouter>>,
    policy: Arc<RwLock<ClaudePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    auth_state: Arc<RwLock<TunnelAuthState>>,
    signer: Arc<dyn SigningService>,
    pool: Arc<RwLock<PoolState>>,
    proxy: Arc<RwLock<ProxyState>>,
}

impl ClaudeFs {
    pub fn new(
        agent_id: AgentId,
        router: ClaudeRouter,
        policy: ClaudePolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        Self::with_state(
            agent_id,
            router,
            policy,
            budget_policy,
            journal,
            signer,
            Arc::new(RwLock::new(Vec::new())),
            Arc::new(RwLock::new(TunnelAuthState::default())),
        )
    }

    pub fn with_state(
        agent_id: AgentId,
        router: ClaudeRouter,
        policy: ClaudePolicy,
        budget_policy: BudgetPolicy,
        journal: Arc<dyn IdempotencyJournal>,
        signer: Arc<dyn SigningService>,
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
    ) -> Self {
        Self {
            agent_id,
            router: Arc::new(RwLock::new(router)),
            policy: Arc::new(RwLock::new(policy)),
            budget: Arc::new(Mutex::new(BudgetTracker::new(budget_policy))),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            journal,
            tunnels,
            auth_state,
            signer,
            pool: Arc::new(RwLock::new(PoolState::default())),
            proxy: Arc::new(RwLock::new(ProxyState::default())),
        }
    }

    pub fn tunnels(&self) -> Arc<RwLock<Vec<TunnelEndpoint>>> {
        self.tunnels.clone()
    }

    pub fn auth_state(&self) -> Arc<RwLock<TunnelAuthState>> {
        self.auth_state.clone()
    }

    fn session_provider(&self, session_id: &str) -> FsResult<(Arc<dyn ClaudeProvider>, String)> {
        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
        let record = sessions.get(session_id).ok_or(FsError::NotFound)?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .provider_by_id(&record.provider_id)
            .ok_or(FsError::NotFound)?;
        Ok((provider, record.provider_id.clone()))
    }

    fn reconcile_session(&self, session_id: &str, state: &SessionState) -> FsResult<()> {
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let record = match sessions.get_mut(session_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }

        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            SessionState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
            }
            SessionState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }

        record.reconciled = true;
        Ok(())
    }

    fn usage_json(&self) -> FsResult<Vec<u8>> {
        let tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        let policy = tracker.policy().clone();
        let state = tracker.state().clone();
        let json = serde_json::json!({
            "tick": {
                "reserved_usd": state.reserved_tick_usd,
                "spent_usd": state.spent_tick_usd,
                "limit_usd": policy.per_tick_usd,
                "remaining_usd": state.remaining_tick(&policy),
            },
            "day": {
                "reserved_usd": state.reserved_day_usd,
                "spent_usd": state.spent_day_usd,
                "limit_usd": policy.per_day_usd,
                "remaining_usd": state.remaining_day(&policy),
            }
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn session_usage_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        self.reconcile_session(session_id, &state)?;
        let reserved_usd = self
            .sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|record| record.reservation.amount_usd)
            .unwrap_or(0);

        let (usage, cost_usd) = match state {
            SessionState::Complete(response) => (response.usage, response.cost_usd),
            SessionState::Idle { usage, cost_usd, .. } => (usage, cost_usd),
            _ => (None, 0),
        };
        let json = serde_json::json!({
            "reserved_usd": reserved_usd,
            "cost_usd": cost_usd,
            "usage": usage,
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn session_response_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        self.reconcile_session(session_id, &state)?;
        let reserved_usd = self
            .sessions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(session_id)
            .map(|record| record.reservation.amount_usd)
            .unwrap_or(0);

        match state {
            SessionState::Complete(response) => {
                let json = serde_json::json!({
                    "session_id": response.session_id,
                    "status": response.status,
                    "response": response.response,
                    "usage": response.usage,
                    "cost_usd": response.cost_usd,
                    "reserved_usd": reserved_usd,
                    "provider_id": response.provider_id,
                    "model": response.model,
                    "tunnel_endpoint": response.tunnel_endpoint,
                });
                serde_json::to_vec_pretty(&json)
                    .map_err(|err| FsError::Other(err.to_string()))
            }
            SessionState::Idle { response, usage, cost_usd, .. } => {
                let json = serde_json::json!({
                    "session_id": session_id,
                    "status": ClaudeSessionStatus::Idle,
                    "response": response,
                    "usage": usage,
                    "cost_usd": cost_usd,
                    "reserved_usd": reserved_usd,
                });
                serde_json::to_vec_pretty(&json)
                    .map_err(|err| FsError::Other(err.to_string()))
            }
            SessionState::Failed { error, .. } => Err(FsError::Other(error)),
            _ => Err(FsError::Other("not ready".to_string())),
        }
    }

    fn session_context_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        let response = match state {
            SessionState::Complete(response) => response.response,
            SessionState::Idle { response, .. } => response,
            _ => None,
        };
        let json = serde_json::json!({
            "session_id": session_id,
            "latest_response": response,
        });
        serde_json::to_vec_pretty(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn session_status_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let state = provider.get_session(session_id).ok_or(FsError::NotFound)?;
        self.reconcile_session(session_id, &state)?;
        let json = serde_json::json!({
            "status": state.status(),
        });
        serde_json::to_vec(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn tool_log_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let entries = provider.tool_log(session_id).unwrap_or_default();
        serde_json::to_vec_pretty(&entries).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pending_tool_json(&self, session_id: &str) -> FsResult<Vec<u8>> {
        let (provider, _) = self.session_provider(session_id)?;
        let pending = provider.pending_tool(session_id);
        serde_json::to_vec_pretty(&pending).map_err(|err| FsError::Other(err.to_string()))
    }

    fn auth_status_json(&self) -> FsResult<Vec<u8>> {
        let now = Timestamp::now();
        let tunnels = self.tunnels.read().unwrap_or_else(|e| e.into_inner());
        let mut auth = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
        let mut statuses = Vec::new();
        for tunnel in tunnels.iter() {
            let challenge = auth.challenges.get(&tunnel.id).cloned();
            let response = auth.responses.get(&tunnel.id).cloned();
            let (authorized, pubkey) = match tunnel.auth {
                TunnelAuth::None => (true, None),
                TunnelAuth::Nostr { .. } => {
                    if let (Some(challenge), Some(response)) = (&challenge, response) {
                        if challenge.expires_at.as_millis() <= now.as_millis()
                            || response.challenge != challenge.challenge
                        {
                            auth.responses.remove(&tunnel.id);
                            (false, None)
                        } else {
                            (true, Some(response.pubkey))
                        }
                    } else {
                        (false, None)
                    }
                }
                TunnelAuth::Psk { .. } => (false, None),
            };
            statuses.push(TunnelAuthStatus {
                tunnel_id: tunnel.id.clone(),
                auth_type: tunnel.auth.type_name(),
                authorized,
                pubkey,
                challenge_expires_at: challenge.map(|c| c.expires_at),
            });
        }
        serde_json::to_vec_pretty(&statuses).map_err(|err| FsError::Other(err.to_string()))
    }

    fn tunnel_endpoints_json(&self) -> FsResult<Vec<u8>> {
        let guard = self.tunnels.read().unwrap_or_else(|e| e.into_inner());
        let summary: Vec<TunnelSummary> = guard
            .iter()
            .map(|t| TunnelSummary {
                id: t.id.clone(),
                url: t.url.clone(),
                auth_type: t.auth.type_name(),
            })
            .collect();
        serde_json::to_vec_pretty(&summary).map_err(|err| FsError::Other(err.to_string()))
    }

    fn provider_health_json(&self, provider_id: &str) -> FsResult<Vec<u8>> {
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let info = router
            .list_providers()
            .into_iter()
            .find(|p| p.id == provider_id)
            .ok_or(FsError::NotFound)?;
        let json = serde_json::json!({ "status": info.status });
        serde_json::to_vec_pretty(&json).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pool_config_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.config).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pool_status_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        let status = PoolStatus {
            total_workers: state.workers.len() as u32,
            idle_workers: state
                .workers
                .values()
                .filter(|w| w.status == WorkerStatus::Idle)
                .count() as u32,
            unhealthy_workers: state
                .workers
                .values()
                .filter(|w| w.status == WorkerStatus::Unhealthy)
                .count() as u32,
        };
        serde_json::to_vec_pretty(&status).map_err(|err| FsError::Other(err.to_string()))
    }

    fn pool_metrics_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.metrics).map_err(|err| FsError::Other(err.to_string()))
    }

    fn proxy_status_json(&self) -> FsResult<Vec<u8>> {
        let state = self.proxy.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.status).map_err(|err| FsError::Other(err.to_string()))
    }

    fn proxy_metrics_json(&self) -> FsResult<Vec<u8>> {
        let state = self.proxy.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.metrics).map_err(|err| FsError::Other(err.to_string()))
    }

    fn proxy_allowlist_json(&self) -> FsResult<Vec<u8>> {
        let state = self.proxy.read().unwrap_or_else(|e| e.into_inner());
        serde_json::to_vec_pretty(&state.allowlist).map_err(|err| FsError::Other(err.to_string()))
    }

    fn workers_list_json(&self) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        let workers: Vec<_> = state.workers.values().cloned().collect();
        serde_json::to_vec_pretty(&workers).map_err(|err| FsError::Other(err.to_string()))
    }

    fn worker_field_json(&self, worker_id: &str, field: WorkerField) -> FsResult<Vec<u8>> {
        let state = self.pool.read().unwrap_or_else(|e| e.into_inner());
        let worker = state.workers.get(worker_id).ok_or(FsError::NotFound)?;
        let json = match field {
            WorkerField::Status => serde_json::to_vec_pretty(&worker.status),
            WorkerField::Isolation => serde_json::to_vec_pretty(&worker.isolation),
            WorkerField::Sessions => serde_json::to_vec_pretty(&worker.sessions),
            WorkerField::Metrics => serde_json::to_vec_pretty(&worker.metrics),
        };
        json.map_err(|err| FsError::Other(err.to_string()))
    }
}

impl FileService for ClaudeFs {
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            ["new"] if flags.write => Ok(Box::new(ClaudeNewHandle::new(
                self.agent_id.clone(),
                self.router.clone(),
                self.policy.clone(),
                self.budget.clone(),
                self.sessions.clone(),
                self.journal.clone(),
            ))),
            ["policy"] => {
                if flags.write {
                    Ok(Box::new(ClaudePolicyWriteHandle::new(self.policy.clone())))
                } else {
                    let policy = self.policy.read().unwrap_or_else(|e| e.into_inner());
                    let json = serde_json::to_vec_pretty(&*policy)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    Ok(Box::new(BytesHandle::new(json)))
                }
            }
            ["usage"] => Ok(Box::new(BytesHandle::new(self.usage_json()?))),
            ["providers"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let providers = router.list_providers();
                let json = serde_json::to_vec_pretty(&providers)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "info"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "models"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                let info = router
                    .list_providers()
                    .into_iter()
                    .find(|p| p.id == *id)
                    .ok_or(FsError::NotFound)?;
                let json = serde_json::to_vec_pretty(&info.models)
                    .map_err(|err| FsError::Other(err.to_string()))?;
                Ok(Box::new(BytesHandle::new(json)))
            }
            ["providers", id, "health"] => Ok(Box::new(BytesHandle::new(
                self.provider_health_json(id)?,
            ))),
            ["providers", "tunnel", "endpoints"] => Ok(Box::new(BytesHandle::new(
                self.tunnel_endpoints_json()?,
            ))),
            ["auth", "tunnels"] => {
                if flags.write {
                    Ok(Box::new(AuthTunnelsWriteHandle::new(
                        self.tunnels.clone(),
                        self.auth_state.clone(),
                    )))
                } else {
                    Ok(Box::new(AuthTunnelsReadHandle::new(self.tunnels.clone())))
                }
            }
            ["auth", "challenge"] => {
                if flags.write {
                    Ok(Box::new(AuthChallengeWriteHandle::new(
                        self.tunnels.clone(),
                        self.auth_state.clone(),
                        self.signer.clone(),
                    )))
                } else {
                    Ok(Box::new(AuthChallengeReadHandle::new(
                        self.tunnels.clone(),
                        self.auth_state.clone(),
                    )))
                }
            }
            ["auth", "status"] => Ok(Box::new(BytesHandle::new(self.auth_status_json()?))),
            ["sessions", session_id, "status"] => Ok(Box::new(BytesHandle::new(
                self.session_status_json(session_id)?,
            ))),
            ["sessions", session_id, "prompt"] if flags.write => Ok(Box::new(PromptHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))),
            ["sessions", session_id, "response"] => Ok(Box::new(BytesHandle::new(
                self.session_response_json(session_id)?,
            ))),
            ["sessions", session_id, "context"] => Ok(Box::new(BytesHandle::new(
                self.session_context_json(session_id)?,
            ))),
            ["sessions", session_id, "usage"] => Ok(Box::new(BytesHandle::new(
                self.session_usage_json(session_id)?,
            ))),
            ["sessions", session_id, "tools", "log"] => Ok(Box::new(BytesHandle::new(
                self.tool_log_json(session_id)?,
            ))),
            ["sessions", session_id, "tools", "pending"] => Ok(Box::new(BytesHandle::new(
                self.pending_tool_json(session_id)?,
            ))),
            ["sessions", session_id, "tools", "approve"] if flags.write => {
                Ok(Box::new(ToolApprovalHandle::new(
                    self.router.clone(),
                    session_id.to_string(),
                )))
            }
            ["sessions", session_id, "fork"] if flags.write => Ok(Box::new(ForkHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))),
            ["sessions", session_id, "ctl"] if flags.write => Ok(Box::new(SessionCtlHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))),
            ["pool", "config"] => {
                if flags.write {
                    Ok(Box::new(PoolConfigWriteHandle::new(self.pool.clone())))
                } else {
                    Ok(Box::new(BytesHandle::new(self.pool_config_json()?)))
                }
            }
            ["pool", "status"] => Ok(Box::new(BytesHandle::new(self.pool_status_json()?))),
            ["pool", "metrics"] => Ok(Box::new(BytesHandle::new(self.pool_metrics_json()?))),
            ["proxy", "status"] => Ok(Box::new(BytesHandle::new(self.proxy_status_json()?))),
            ["proxy", "metrics"] => Ok(Box::new(BytesHandle::new(self.proxy_metrics_json()?))),
            ["proxy", "allowlist"] => {
                if flags.write {
                    Ok(Box::new(ProxyAllowlistWriteHandle::new(self.proxy.clone())))
                } else {
                    Ok(Box::new(BytesHandle::new(self.proxy_allowlist_json()?)))
                }
            }
            ["workers"] => Ok(Box::new(BytesHandle::new(self.workers_list_json()?))),
            ["workers", worker_id, "status"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Status)?,
            ))),
            ["workers", worker_id, "isolation"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Isolation)?,
            ))),
            ["workers", worker_id, "sessions"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Sessions)?,
            ))),
            ["workers", worker_id, "metrics"] => Ok(Box::new(BytesHandle::new(
                self.worker_field_json(worker_id, WorkerField::Metrics)?,
            ))),
            _ => Err(FsError::NotFound),
        }
    }

    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>> {
        let path = path.trim_matches('/');
        match path {
            "" => Ok(vec![
                DirEntry::dir("providers"),
                DirEntry::file("new", 0),
                DirEntry::file("policy", 0),
                DirEntry::file("usage", 0),
                DirEntry::dir("auth"),
                DirEntry::dir("sessions"),
                DirEntry::dir("workers"),
                DirEntry::dir("pool"),
                DirEntry::dir("proxy"),
            ]),
            "providers" => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                Ok(router
                    .list_providers()
                    .iter()
                    .map(|p| DirEntry::dir(&p.id))
                    .collect())
            }
            "auth" => Ok(vec![
                DirEntry::file("tunnels", 0),
                DirEntry::file("challenge", 0),
                DirEntry::file("status", 0),
            ]),
            "sessions" => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                Ok(sessions.keys().map(|id| DirEntry::dir(id)).collect())
            }
            "workers" => {
                let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                Ok(pool.workers.keys().map(|id| DirEntry::dir(id)).collect())
            }
            "pool" => Ok(vec![
                DirEntry::file("config", 0),
                DirEntry::file("status", 0),
                DirEntry::file("metrics", 0),
            ]),
            "proxy" => Ok(vec![
                DirEntry::file("status", 0),
                DirEntry::file("allowlist", 0),
                DirEntry::file("metrics", 0),
            ]),
            _ => {
                let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
                match parts.as_slice() {
                    ["providers", id] => {
                        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                        if router.list_providers().iter().any(|p| p.id == *id) {
                            let mut entries = vec![
                                DirEntry::file("info", 0),
                                DirEntry::file("models", 0),
                                DirEntry::file("health", 0),
                            ];
                            if *id == "tunnel" {
                                entries.push(DirEntry::file("endpoints", 0));
                            }
                            Ok(entries)
                        } else {
                            Ok(vec![])
                        }
                    }
                    ["sessions", session_id] => {
                        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                        if sessions.contains_key(*session_id) {
                            Ok(vec![
                                DirEntry::file("status", 0),
                                DirEntry::file("prompt", 0),
                                DirEntry::file("response", 0),
                                DirEntry::file("context", 0),
                                DirEntry::file("output", 0),
                                DirEntry::file("usage", 0),
                                DirEntry::dir("tools"),
                                DirEntry::file("fork", 0),
                                DirEntry::file("ctl", 0),
                            ])
                        } else {
                            Ok(vec![])
                        }
                    }
                    ["sessions", session_id, "tools"] => {
                        let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                        if sessions.contains_key(*session_id) {
                            Ok(vec![
                                DirEntry::file("log", 0),
                                DirEntry::file("pending", 0),
                                DirEntry::file("approve", 0),
                            ])
                        } else {
                            Ok(vec![])
                        }
                    }
                    ["workers", worker_id] => {
                        let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                        if pool.workers.contains_key(*worker_id) {
                            Ok(vec![
                                DirEntry::file("status", 0),
                                DirEntry::file("isolation", 0),
                                DirEntry::file("sessions", 0),
                                DirEntry::file("metrics", 0),
                            ])
                        } else {
                            Ok(vec![])
                        }
                    }
                    _ => Ok(vec![]),
                }
            }
        }
    }

    fn stat(&self, path: &str) -> FsResult<Stat> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            [] => Ok(Stat::dir()),
            ["providers"] | ["sessions"] | ["auth"] | ["workers"] | ["pool"] | ["proxy"] => {
                Ok(Stat::dir())
            }
            ["new"] | ["policy"] => Ok(Stat {
                size: 0,
                is_dir: false,
                created: None,
                modified: None,
                permissions: Permissions::read_write(),
            }),
            ["usage"] => Ok(Stat::file(0)),
            ["providers", id] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", id, "info"] | ["providers", id, "models"] | ["providers", id, "health"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == *id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["providers", "tunnel", "endpoints"] => {
                let router = self.router.read().unwrap_or_else(|e| e.into_inner());
                if router.list_providers().iter().any(|p| p.id == "tunnel") {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["auth", "tunnels"] | ["auth", "challenge"] | ["auth", "status"] => Ok(Stat::file(0)),
            ["sessions", session_id] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, "tools"] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["sessions", session_id, _] | ["sessions", session_id, "tools", _] => {
                let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
                if sessions.contains_key(*session_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["pool", "config"] | ["pool", "status"] | ["pool", "metrics"] => Ok(Stat::file(0)),
            ["proxy", "status"] | ["proxy", "metrics"] | ["proxy", "allowlist"] => {
                Ok(Stat::file(0))
            }
            ["workers", worker_id] => {
                let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                if pool.workers.contains_key(*worker_id) {
                    Ok(Stat::dir())
                } else {
                    Err(FsError::NotFound)
                }
            }
            ["workers", worker_id, _] => {
                let pool = self.pool.read().unwrap_or_else(|e| e.into_inner());
                if pool.workers.contains_key(*worker_id) {
                    Ok(Stat::file(0))
                } else {
                    Err(FsError::NotFound)
                }
            }
            _ => Err(FsError::NotFound),
        }
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

    fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>> {
        let path = path.trim_matches('/');
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        if let ["sessions", session_id, "output"] = parts.as_slice() {
            let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            let record = sessions.get(*session_id).ok_or(FsError::NotFound)?;
            return Ok(Some(Box::new(OutputWatchHandle::new(
                session_id.to_string(),
                record.provider_id.clone(),
                self.router.clone(),
                self.sessions.clone(),
                self.budget.clone(),
            ))));
        }
        Ok(None)
    }

    fn name(&self) -> &str {
        "claude"
    }
}

struct ClaudeNewHandle {
    agent_id: AgentId,
    router: Arc<RwLock<ClaudeRouter>>,
    policy: Arc<RwLock<ClaudePolicy>>,
    budget: Arc<Mutex<BudgetTracker>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    journal: Arc<dyn IdempotencyJournal>,
    request_buf: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ClaudeNewHandle {
    fn new(
        agent_id: AgentId,
        router: Arc<RwLock<ClaudeRouter>>,
        policy: Arc<RwLock<ClaudePolicy>>,
        budget: Arc<Mutex<BudgetTracker>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        journal: Arc<dyn IdempotencyJournal>,
    ) -> Self {
        Self {
            agent_id,
            router,
            policy,
            budget,
            sessions,
            journal,
            request_buf: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn submit_request(&mut self) -> FsResult<()> {
        let mut request: ClaudeRequest = serde_json::from_slice(&self.request_buf)
            .map_err(|err| FsError::Other(err.to_string()))?;

        let policy = self
            .policy
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if policy.require_idempotency && request.idempotency_key.is_none() {
            return Err(FsError::Other(ClaudeError::IdempotencyRequired.to_string()));
        }

        let resolved_autonomy = request
            .autonomy
            .clone()
            .unwrap_or_else(|| policy.default_autonomy.clone());
        request.autonomy = Some(resolved_autonomy.clone());

        if let Some(limit) = policy.max_context_tokens {
            if let Some(requested) = request.max_context_tokens {
                request.max_context_tokens = Some(requested.min(limit));
            } else {
                request.max_context_tokens = Some(limit);
            }
        }

        let max_cost_usd = match (
            request.max_cost_usd,
            policy.default_max_cost_usd,
            policy.require_max_cost,
        ) {
            (Some(cost), _, _) => cost,
            (None, Some(default_cost), _) => {
                request.max_cost_usd = Some(default_cost);
                default_cost
            }
            (None, None, true) => {
                return Err(FsError::Other(ClaudeError::MaxCostRequired.to_string()));
            }
            (None, None, false) => {
                let tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                let budget_policy = tracker.policy();
                if budget_policy.per_tick_usd > 0 {
                    budget_policy.per_tick_usd
                } else {
                    budget_policy.per_day_usd
                }
            }
        };
        request.max_cost_usd = Some(max_cost_usd);

        if !policy.allowed_models.is_empty()
            && !policy
                .allowed_models
                .iter()
                .any(|pat| matches_pattern(pat, &request.model))
        {
            return Err(FsError::Other(ClaudeError::InvalidRequest(
                "model not allowed".to_string(),
            )
            .to_string()));
        }

        if policy
            .blocked_models
            .iter()
            .any(|pat| matches_pattern(pat, &request.model))
        {
            return Err(FsError::Other(ClaudeError::InvalidRequest(
                "model blocked".to_string(),
            )
            .to_string()));
        }

        if let Some(tunnel) = request.tunnel_endpoint.as_ref() {
            if !policy.allowed_tunnels.is_empty()
                && !policy.allowed_tunnels.iter().any(|t| t == tunnel)
            {
                return Err(FsError::Other(ClaudeError::InvalidRequest(
                    "tunnel not allowed".to_string(),
                )
                .to_string()));
            }
        }

        let active_sessions = {
            let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            let router = self.router.read().unwrap_or_else(|e| e.into_inner());
            sessions
                .iter()
                .filter(|(session_id, record)| {
                    let provider = match router.provider_by_id(&record.provider_id) {
                        Some(provider) => provider,
                        None => return false,
                    };
                    let state = match provider.get_session(session_id) {
                        Some(state) => state,
                        None => return false,
                    };
                    !matches!(
                        state,
                        SessionState::Complete(_) | SessionState::Failed { .. }
                    )
                })
                .count()
        };
        if active_sessions as u32 >= policy.max_concurrent {
            return Err(FsError::Other(ClaudeError::InvalidRequest(
                "max concurrent sessions exceeded".to_string(),
            )
            .to_string()));
        }

        if !policy.allowed_tools.is_empty() {
            for tool in &request.tools {
                if !policy.allowed_tools.iter().any(|t| t == &tool.name) {
                    return Err(FsError::Other(ClaudeError::InvalidRequest(
                        format!("tool not allowed: {}", tool.name),
                    )
                    .to_string()));
                }
            }
        }

        for tool in &request.tools {
            if policy.blocked_tools.iter().any(|t| t == &tool.name) {
                return Err(FsError::Other(ClaudeError::InvalidRequest(
                    format!("tool blocked: {}", tool.name),
                )
                .to_string()));
            }
        }

        let tool_policy = ToolPolicy {
            allowed: if !policy.allowed_tools.is_empty() {
                policy.allowed_tools.clone()
            } else {
                request.tools.iter().map(|t| t.name.clone()).collect()
            },
            blocked: policy.blocked_tools.clone(),
            approval_required: policy.approval_required_tools.clone(),
            autonomy: resolved_autonomy,
        };

        request = request.with_internal(ClaudeRequestInternal {
            tool_policy,
            fork: false,
            resume_backend_id: None,
            #[cfg(not(target_arch = "wasm32"))]
            container: None,
            #[cfg(not(target_arch = "wasm32"))]
            executable: None,
        });

        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = if let Some(resume_id) = request.resume_session_id.as_ref() {
            let sessions = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            let record = sessions
                .get(resume_id)
                .ok_or_else(|| FsError::Other("resume session not found".to_string()))?;
            router
                .provider_by_id(&record.provider_id)
                .ok_or(FsError::NotFound)?
        } else {
            router
                .select(&request, &policy)
                .map_err(|err| FsError::Other(err.to_string()))?
        };
        let provider_id = provider.id().to_string();

        #[cfg(not(target_arch = "wasm32"))]
        if matches!(policy.isolation_mode, IsolationMode::Container)
            && matches!(provider_id.as_str(), "local" | "cloud")
        {
            let config = resolve_container_config(&policy)
                .map_err(|err| FsError::Other(err.to_string()))?;
            request.internal.container = Some(config);
        }
        #[cfg(target_arch = "wasm32")]
        if matches!(policy.isolation_mode, IsolationMode::Container)
            && matches!(provider_id.as_str(), "local" | "cloud")
        {
            return Err(FsError::Other(
                "container isolation not supported on wasm".to_string(),
            ));
        }

        let scoped_key = request.idempotency_key.as_ref().map(|key| {
            format!("{}:{}:{}", self.agent_id.as_str(), provider_id, key)
        });

        if let Some(key) = scoped_key.as_ref() {
            if let Some(cached) = self
                .journal
                .get(key)
                .map_err(|err| FsError::Other(err.to_string()))?
            {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&cached) {
                    if let Some(session_id) = value.get("session_id").and_then(|v| v.as_str()) {
                        self.sessions
                            .write()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(session_id.to_string())
                            .or_insert(SessionRecord {
                                provider_id: provider_id.clone(),
                                reservation: BudgetReservation { amount_usd: 0 },
                                reconciled: true,
                            });
                    }
                }
                self.response = Some(cached);
                return Ok(());
            }
        }

        let reservation = {
            let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
            let reservation = tracker.reserve(max_cost_usd).map_err(|_| FsError::BudgetExceeded)?;
            let state = tracker.state().clone();
            if let Some(limit) = policy.max_cost_usd_per_tick {
                if state.reserved_tick_usd + state.spent_tick_usd > limit {
                    tracker.release(reservation);
                    return Err(FsError::BudgetExceeded);
                }
            }
            if let Some(limit) = policy.max_cost_usd_per_day {
                if state.reserved_day_usd + state.spent_day_usd > limit {
                    tracker.release(reservation);
                    return Err(FsError::BudgetExceeded);
                }
            }
            reservation
        };

        let session_id = match provider.create_session(request.clone()) {
            Ok(session_id) => session_id,
            Err(err) => {
                let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
                tracker.release(reservation);
                return Err(FsError::Other(err.to_string()));
            }
        };

        self.sessions
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                session_id.clone(),
                SessionRecord {
                    provider_id,
                    reservation,
                    reconciled: false,
                },
            );

        let response_json = serde_json::json!({
            "session_id": session_id,
            "status": "creating",
            "status_path": format!("/claude/sessions/{}/status", session_id),
            "output_path": format!("/claude/sessions/{}/output", session_id),
            "response_path": format!("/claude/sessions/{}/response", session_id),
            "prompt_path": format!("/claude/sessions/{}/prompt", session_id),
        });
        let response_bytes = serde_json::to_vec(&response_json)
            .map_err(|err| FsError::Other(err.to_string()))?;

        if let Some(key) = scoped_key.as_ref() {
            self.journal
                .put_with_ttl(key, &response_bytes, IDEMPOTENCY_TTL)
                .map_err(|err| FsError::Other(err.to_string()))?;
        }

        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ClaudeNewHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.submit_request()?;
        }
        let response = self.response.as_ref().unwrap();
        if self.position >= response.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), response.len() - self.position);
        buf[..len].copy_from_slice(&response[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.request_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        if self.response.is_none() {
            return Err(FsError::InvalidPath);
        }
        let response = self.response.as_ref().unwrap();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => response.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.response.is_none() && !self.request_buf.is_empty() {
            self.submit_request()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct PromptHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
}

impl PromptHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
        }
    }

    fn send_prompt(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let prompt = String::from_utf8(self.buffer.clone())
            .map_err(|err| FsError::Other(err.to_string()))?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        provider
            .send_prompt(&self.session_id, &prompt)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }
}

impl FileHandle for PromptHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        self.send_prompt()
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ToolApprovalHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
}

impl ToolApprovalHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
        }
    }

    fn submit(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let value: serde_json::Value = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let approved = value
            .get("approved")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| FsError::Other("missing approved field".to_string()))?;
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        provider
            .approve_tool(&self.session_id, approved)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.buffer.clear();
        Ok(())
    }
}

impl FileHandle for ToolApprovalHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        self.submit()
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ForkHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
    response: Option<Vec<u8>>,
    position: usize,
}

impl ForkHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
            response: None,
            position: 0,
        }
    }

    fn fork(&mut self) -> FsResult<()> {
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        let new_id = provider
            .fork_session(&self.session_id)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let response_json = serde_json::json!({ "session_id": new_id });
        let response_bytes = serde_json::to_vec(&response_json)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.response = Some(response_bytes);
        Ok(())
    }
}

impl FileHandle for ForkHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.response.is_none() {
            self.fork()?;
        }
        let response = self.response.as_ref().unwrap();
        if self.position >= response.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), response.len() - self.position);
        buf[..len].copy_from_slice(&response[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        if self.response.is_none() {
            return Err(FsError::InvalidPath);
        }
        let response = self.response.as_ref().unwrap();
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => response.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.response.is_none() {
            self.fork()?;
        }
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct SessionCtlHandle {
    router: Arc<RwLock<ClaudeRouter>>,
    session_id: String,
    buffer: Vec<u8>,
}

impl SessionCtlHandle {
    fn new(router: Arc<RwLock<ClaudeRouter>>, session_id: String) -> Self {
        Self {
            router,
            session_id,
            buffer: Vec::new(),
        }
    }

    fn apply(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let command = String::from_utf8(self.buffer.clone())
            .map_err(|err| FsError::Other(err.to_string()))?;
        let command = command.trim();
        let router = self.router.read().unwrap_or_else(|e| e.into_inner());
        let provider = router
            .providers
            .iter()
            .find(|p| p.get_session(&self.session_id).is_some())
            .cloned()
            .ok_or(FsError::NotFound)?;
        match command {
            "stop" => provider
                .stop(&self.session_id)
                .map_err(|err| FsError::Other(err.to_string()))?,
            "pause" => provider
                .pause(&self.session_id)
                .map_err(|err| FsError::Other(err.to_string()))?,
            "resume" => provider
                .resume(&self.session_id)
                .map_err(|err| FsError::Other(err.to_string()))?,
            _ => return Err(FsError::Other("unknown command".to_string())),
        }
        self.buffer.clear();
        Ok(())
    }
}

impl FileHandle for SessionCtlHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        self.apply()
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ClaudePolicyWriteHandle {
    policy: Arc<RwLock<ClaudePolicy>>,
    buffer: Vec<u8>,
}

impl ClaudePolicyWriteHandle {
    fn new(policy: Arc<RwLock<ClaudePolicy>>) -> Self {
        Self {
            policy,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for ClaudePolicyWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let policy: ClaudePolicy = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let mut guard = self.policy.write().unwrap_or_else(|e| e.into_inner());
        *guard = policy;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthTunnelsReadHandle {
    position: usize,
    payload: Vec<u8>,
}

impl AuthTunnelsReadHandle {
    fn new(tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>) -> Self {
        let guard = tunnels.read().unwrap_or_else(|e| e.into_inner());
        let summary: Vec<TunnelSummary> = guard
            .iter()
            .map(|t| TunnelSummary {
                id: t.id.clone(),
                url: t.url.clone(),
                auth_type: t.auth.type_name(),
            })
            .collect();
        let payload = serde_json::to_vec_pretty(&summary).unwrap_or_default();
        Self {
            position: 0,
            payload,
        }
    }
}

impl FileHandle for AuthTunnelsReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.position >= self.payload.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), self.payload.len() - self.position);
        buf[..len].copy_from_slice(&self.payload[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.payload.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct AuthTunnelsWriteHandle {
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    auth_state: Arc<RwLock<TunnelAuthState>>,
    buffer: Vec<u8>,
}

impl AuthTunnelsWriteHandle {
    fn new(
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
    ) -> Self {
        Self {
            tunnels,
            auth_state,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for AuthTunnelsWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let endpoints: Vec<TunnelEndpoint> = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let mut guard = self.tunnels.write().unwrap_or_else(|e| e.into_inner());
        *guard = endpoints;
        let mut auth_state = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
        auth_state.responses.retain(|id, _| guard.iter().any(|t| &t.id == id));
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct AuthChallengeReadHandle {
    position: usize,
    payload: Vec<u8>,
}

impl AuthChallengeReadHandle {
    fn new(
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
    ) -> Self {
        let now = Timestamp::now();
        let guard = tunnels.read().unwrap_or_else(|e| e.into_inner());
        let mut auth = auth_state.write().unwrap_or_else(|e| e.into_inner());
        let mut challenges = Vec::new();

        for endpoint in guard.iter() {
            let mut expired = false;
            let challenge_snapshot = {
                let challenge = auth
                    .challenges
                    .entry(endpoint.id.clone())
                    .or_insert_with(|| TunnelAuthChallenge {
                        challenge: uuid::Uuid::new_v4().to_string(),
                        expires_at: Timestamp::from_millis(
                            now.as_millis() + AUTH_CHALLENGE_TTL.as_millis() as u64,
                        ),
                        tunnel_id: endpoint.id.clone(),
                    });
                if challenge.expires_at.as_millis() <= now.as_millis() {
                    *challenge = TunnelAuthChallenge {
                        challenge: uuid::Uuid::new_v4().to_string(),
                        expires_at: Timestamp::from_millis(
                            now.as_millis() + AUTH_CHALLENGE_TTL.as_millis() as u64,
                        ),
                        tunnel_id: endpoint.id.clone(),
                    };
                    expired = true;
                }
                challenge.clone()
            };
            if expired {
                auth.responses.remove(&endpoint.id);
            }
            challenges.push(challenge_snapshot);
        }

        let payload = serde_json::to_vec_pretty(&challenges).unwrap_or_default();
        Self {
            position: 0,
            payload,
        }
    }
}

impl FileHandle for AuthChallengeReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.position >= self.payload.len() {
            return Ok(0);
        }
        let len = std::cmp::min(buf.len(), self.payload.len() - self.position);
        buf[..len].copy_from_slice(&self.payload[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.payload.len() as i64 + offset,
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };
        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

struct AuthChallengeWriteHandle {
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    auth_state: Arc<RwLock<TunnelAuthState>>,
    signer: Arc<dyn SigningService>,
    buffer: Vec<u8>,
}

impl AuthChallengeWriteHandle {
    fn new(
        tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
        signer: Arc<dyn SigningService>,
    ) -> Self {
        Self {
            tunnels,
            auth_state,
            signer,
            buffer: Vec::new(),
        }
    }

    fn verify_response(&self, response: &TunnelAuthResponse) -> FsResult<()> {
        let tunnels = self.tunnels.read().unwrap_or_else(|e| e.into_inner());
        let endpoint = tunnels
            .iter()
            .find(|t| t.id == response.tunnel_id)
            .ok_or_else(|| FsError::Other("unknown tunnel".to_string()))?;
        let mut auth_state = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
        let challenge = auth_state
            .challenges
            .get(&response.tunnel_id)
            .ok_or_else(|| FsError::Other("no challenge".to_string()))?;
        if challenge.challenge != response.challenge {
            return Err(FsError::Other("challenge mismatch".to_string()));
        }
        if challenge.expires_at.as_millis() <= Timestamp::now().as_millis() {
            return Err(FsError::Other("challenge expired".to_string()));
        }

        let pubkey = parse_pubkey(&response.pubkey)?;
        let signature = parse_signature(&response.signature)?;

        if !endpoint.allowed_agents.is_empty() {
            let allowed = endpoint.allowed_agents.iter().any(|agent| {
                if let Ok(candidate) = parse_pubkey(agent) {
                    candidate.as_bytes() == pubkey.as_bytes()
                } else {
                    agent.eq_ignore_ascii_case(&response.pubkey)
                }
            });
            if !allowed {
                return Err(FsError::Other("agent not allowed".to_string()));
            }
        }

        if !self
            .signer
            .verify(&pubkey, response.challenge.as_bytes(), &signature)
        {
            return Err(FsError::Other("invalid signature".to_string()));
        }

        auth_state
            .responses
            .insert(response.tunnel_id.clone(), response.clone());

        Ok(())
    }
}

impl FileHandle for AuthChallengeWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let response: TunnelAuthResponse = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        self.verify_response(&response)?;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct PoolConfigWriteHandle {
    pool: Arc<RwLock<PoolState>>,
    buffer: Vec<u8>,
}

impl PoolConfigWriteHandle {
    fn new(pool: Arc<RwLock<PoolState>>) -> Self {
        Self {
            pool,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for PoolConfigWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let config: PoolConfig = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let mut state = self.pool.write().unwrap_or_else(|e| e.into_inner());
        state.config = config;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct ProxyAllowlistWriteHandle {
    proxy: Arc<RwLock<ProxyState>>,
    buffer: Vec<u8>,
}

impl ProxyAllowlistWriteHandle {
    fn new(proxy: Arc<RwLock<ProxyState>>) -> Self {
        Self {
            proxy,
            buffer: Vec::new(),
        }
    }
}

impl FileHandle for ProxyAllowlistWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn write(&mut self, buf: &[u8]) -> FsResult<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let allowlist: Vec<String> = serde_json::from_slice(&self.buffer)
            .map_err(|err| FsError::Other(err.to_string()))?;
        let mut proxy = self.proxy.write().unwrap_or_else(|e| e.into_inner());
        proxy.allowlist = allowlist;
        self.buffer.clear();
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        self.flush()
    }
}

struct OutputWatchHandle {
    session_id: String,
    provider_id: String,
    router: Arc<RwLock<ClaudeRouter>>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    budget: Arc<Mutex<BudgetTracker>>,
}

impl OutputWatchHandle {
    fn new(
        session_id: String,
        provider_id: String,
        router: Arc<RwLock<ClaudeRouter>>,
        sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
        budget: Arc<Mutex<BudgetTracker>>,
    ) -> Self {
        Self {
            session_id,
            provider_id,
            router,
            sessions,
            budget,
        }
    }

    fn reconcile(&self, state: &SessionState) -> FsResult<()> {
        let mut sessions = self.sessions.write().unwrap_or_else(|e| e.into_inner());
        let record = match sessions.get_mut(&self.session_id) {
            Some(record) => record,
            None => return Ok(()),
        };
        if record.reconciled {
            return Ok(());
        }
        let mut tracker = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        match state {
            SessionState::Complete(response) => {
                tracker
                    .reconcile(record.reservation, response.cost_usd)
                    .map_err(|_| FsError::BudgetExceeded)?;
            }
            SessionState::Failed { .. } => {
                tracker.release(record.reservation);
            }
            _ => return Ok(()),
        }
        record.reconciled = true;
        Ok(())
    }
}

impl WatchHandle for OutputWatchHandle {
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            let router = self.router.read().unwrap_or_else(|e| e.into_inner());
            let provider = router
                .provider_by_id(&self.provider_id)
                .ok_or(FsError::NotFound)?;
            match provider.poll_output(&self.session_id) {
                Ok(Some(chunk)) => {
                    let payload = serde_json::to_vec(&chunk)
                        .map_err(|err| FsError::Other(err.to_string()))?;
                    return Ok(Some(WatchEvent::Data(payload)));
                }
                Ok(None) => {
                    if let Some(state) = provider.get_session(&self.session_id) {
                        if matches!(
                            state,
                            SessionState::Complete(_) | SessionState::Failed { .. }
                        ) {
                            self.reconcile(&state)?;
                            return Ok(None);
                        }
                    }
                }
                Err(err) => return Err(FsError::Other(err.to_string())),
            }

            if !wait_for_stream(deadline)? {
                return Ok(None);
            }
        }
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

fn wait_for_stream(deadline: Option<Instant>) -> FsResult<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = deadline;
        return Ok(false);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                return Ok(false);
            }
        }
        std::thread::sleep(Duration::from_millis(25));
        Ok(true)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("no provider available for model {model}: {reason}")]
    NoProviderAvailable { model: String, reason: String },
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("session not found")]
    SessionNotFound,
    #[error("budget exceeded")]
    BudgetExceeded,
    #[error("idempotency key required")]
    IdempotencyRequired,
    #[error("max_cost_usd required")]
    MaxCostRequired,
    #[error("session not ready")]
    NotReady,
    #[error("tunnel required")]
    TunnelRequired,
    #[error("tunnel auth required")]
    TunnelAuthRequired,
    #[error("journal error: {0}")]
    Journal(String),
}

impl From<BudgetError> for ClaudeError {
    fn from(err: BudgetError) -> Self {
        match err {
            BudgetError::Exceeded => ClaudeError::BudgetExceeded,
            BudgetError::ActualExceedsReservation => {
                ClaudeError::ProviderError("actual cost exceeded reservation".to_string())
            }
        }
    }
}

impl From<JournalError> for ClaudeError {
    fn from(err: JournalError) -> Self {
        ClaudeError::Journal(err.to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_container_config(policy: &ClaudePolicy) -> Result<ClaudeContainerConfig, ClaudeError> {
    let image = std::env::var("OPENAGENTS_CLAUDE_CONTAINER_IMAGE").map_err(|_| {
        ClaudeError::ProviderError("OPENAGENTS_CLAUDE_CONTAINER_IMAGE not set".to_string())
    })?;
    let command = std::env::var("OPENAGENTS_CLAUDE_CONTAINER_COMMAND").ok();
    let proxy_url = std::env::var("OPENAGENTS_CLAUDE_PROXY_URL").ok();
    let runtime = resolve_container_runtime()?;
    Ok(ClaudeContainerConfig {
        runtime,
        image,
        network_mode: policy.network_mode.clone(),
        proxy_url,
        command,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_container_runtime() -> Result<ClaudeContainerRuntime, ClaudeError> {
    if let Ok(value) = std::env::var("OPENAGENTS_CLAUDE_CONTAINER_RUNTIME") {
        let normalized = value.trim().to_lowercase();
        match normalized.as_str() {
            "apple" | "container" => {
                if apple_container_available() {
                    return Ok(ClaudeContainerRuntime::Apple);
                }
                return Err(ClaudeError::ProviderError(
                    "apple container runtime unavailable".to_string(),
                ));
            }
            "docker" => {
                if docker_available() {
                    return Ok(ClaudeContainerRuntime::Docker);
                }
                return Err(ClaudeError::ProviderError(
                    "docker runtime unavailable".to_string(),
                ));
            }
            "auto" | "" => {}
            _ => {
                return Err(ClaudeError::ProviderError(format!(
                    "invalid OPENAGENTS_CLAUDE_CONTAINER_RUNTIME: {}",
                    value
                )));
            }
        }
    }

    if apple_container_available() {
        Ok(ClaudeContainerRuntime::Apple)
    } else if docker_available() {
        Ok(ClaudeContainerRuntime::Docker)
    } else {
        Err(ClaudeError::ProviderError(
            "no container runtime available".to_string(),
        ))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .arg("--format")
        .arg("{{.Server.Version}}")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
fn apple_container_available() -> bool {
    let major = macos_version_major().unwrap_or(0);
    if major < 26 {
        return false;
    }
    Command::new("container")
        .args(["system", "status"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(not(all(target_os = "macos", feature = "apple-container")))]
fn apple_container_available() -> bool {
    false
}

#[cfg(all(target_os = "macos", feature = "apple-container"))]
fn macos_version_major() -> Option<u32> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout);
    let mut parts = version.trim().split('.');
    let major = parts.next().unwrap_or("");
    let digits: String = major.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolConfig {
    min_workers: u32,
    max_workers: u32,
    idle_timeout_secs: u64,
    scale_up_threshold: f32,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_workers: 0,
            max_workers: 0,
            idle_timeout_secs: 300,
            scale_up_threshold: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolMetrics {
    total_requests: u64,
    errors: u64,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            errors: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PoolStatus {
    total_workers: u32,
    idle_workers: u32,
    unhealthy_workers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WorkerStatus {
    Idle,
    Busy,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerMetrics {
    requests: u64,
    errors: u64,
}

impl Default for WorkerMetrics {
    fn default() -> Self {
        Self { requests: 0, errors: 0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorkerInfo {
    id: String,
    status: WorkerStatus,
    isolation: IsolationMode,
    sessions: u32,
    metrics: WorkerMetrics,
}

#[derive(Default)]
struct PoolState {
    config: PoolConfig,
    metrics: PoolMetrics,
    workers: HashMap<String, WorkerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyStatus {
    status: String,
}

impl Default for ProxyStatus {
    fn default() -> Self {
        Self {
            status: "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProxyMetrics {
    requests: u64,
    blocked: u64,
}

impl Default for ProxyMetrics {
    fn default() -> Self {
        Self { requests: 0, blocked: 0 }
    }
}

#[derive(Default)]
struct ProxyState {
    status: ProxyStatus,
    allowlist: Vec<String>,
    metrics: ProxyMetrics,
}

#[derive(Clone, Copy)]
enum WorkerField {
    Status,
    Isolation,
    Sessions,
    Metrics,
}

fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == value;
    }
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or("");
    let mut remainder = value;
    if !first.is_empty() {
        if let Some(stripped) = remainder.strip_prefix(first) {
            remainder = stripped;
        } else {
            return false;
        }
    }
    let mut tail = remainder;
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(idx) = tail.find(part) {
            tail = &tail[idx + part.len()..];
        } else {
            return false;
        }
    }
    true
}

fn parse_pubkey(input: &str) -> FsResult<PublicKey> {
    if input.starts_with("npub") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let entity = nostr::decode(input)
                .map_err(|err| FsError::Other(err.to_string()))?;
            if let nostr::Nip19Entity::Pubkey(bytes) = entity {
                return Ok(PublicKey::new(bytes.to_vec()));
            }
            return Err(FsError::Other("invalid npub".to_string()));
        }
        #[cfg(target_arch = "wasm32")]
        {
            return Err(FsError::Other("bech32 pubkey unsupported".to_string()));
        }
    }
    let bytes = hex::decode(input).map_err(|err| FsError::Other(err.to_string()))?;
    Ok(PublicKey::new(bytes))
}

fn parse_signature(input: &str) -> FsResult<Signature> {
    let bytes = hex::decode(input)
        .or_else(|_| STANDARD.decode(input))
        .map_err(|err| FsError::Other(err.to_string()))?;
    Ok(Signature::new(bytes))
}

fn usd_to_microusd(cost: f64) -> u64 {
    if cost <= 0.0 {
        return 0;
    }
    (cost * 1_000_000.0).round() as u64
}

fn extract_text_from_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Object(map) => {
            if let Some(content) = map.get("content") {
                if let Some(text) = extract_text_from_value(content) {
                    return Some(text);
                }
            }
            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_string());
            }
            None
        }
        serde_json::Value::Array(items) => {
            let mut out = String::new();
            for item in items {
                if let Some(text) = extract_text_from_value(item) {
                    out.push_str(&text);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

fn extract_delta_from_event(event: &serde_json::Value) -> Option<String> {
    if let Some(delta) = event.get("delta") {
        if let Some(text) = extract_text_from_value(delta) {
            return Some(text);
        }
    }
    if let Some(text) = extract_text_from_value(event) {
        return Some(text);
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
mod providers {
    use super::*;
    use claude_agent_sdk::permissions::{CallbackPermissionHandler, PermissionRequest};
    use claude_agent_sdk::protocol::{PermissionResult, SdkMessage, SdkResultMessage, SdkSystemMessage};
    use claude_agent_sdk::{ExecutableConfig, Query, QueryOptions, ToolsConfig};
    use futures::{SinkExt, StreamExt};
    use std::path::PathBuf;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio_stream::wrappers::ReceiverStream;
    use tokio_tungstenite::tungstenite::Message as WsMessage;

    #[derive(Clone)]
    struct Executor {
        runtime: Arc<tokio::runtime::Runtime>,
    }

    impl Executor {
        fn new() -> Result<Self, ClaudeError> {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|err| ClaudeError::ProviderError(err.to_string()))?;
            Ok(Self {
                runtime: Arc::new(runtime),
            })
        }

        fn spawn<F>(&self, fut: F)
        where
            F: std::future::Future<Output = ()> + Send + 'static,
        {
            self.runtime.spawn(fut);
        }
    }

    fn container_command(runtime: ClaudeContainerRuntime) -> &'static str {
        match runtime {
            ClaudeContainerRuntime::Apple => "container",
            ClaudeContainerRuntime::Docker => "docker",
        }
    }

    fn container_executable(
        config: &ClaudeContainerConfig,
        env: &HashMap<String, String>,
    ) -> ExecutableConfig {
        let mut args = Vec::new();
        args.push("run".to_string());
        args.push("--rm".to_string());
        args.push("-i".to_string());
        if matches!(config.network_mode, NetworkMode::None) {
            args.push("--network".to_string());
            args.push("none".to_string());
        }
        for (key, value) in env {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }
        if let Some(proxy_url) = config.proxy_url.as_ref() {
            args.push("-e".to_string());
            args.push(format!("HTTPS_PROXY={}", proxy_url));
            args.push("-e".to_string());
            args.push(format!("HTTP_PROXY={}", proxy_url));
            args.push("-e".to_string());
            args.push("NODE_USE_ENV_PROXY=1".to_string());
        }
        args.push(config.image.clone());
        args.push(
            config
                .command
                .clone()
                .unwrap_or_else(|| "claude".to_string()),
        );
        ExecutableConfig {
            path: Some(PathBuf::from(container_command(config.runtime))),
            executable: None,
            executable_args: args,
        }
    }

    #[derive(Clone)]
    struct ProcessSession {
        state: Arc<RwLock<SessionState>>,
        prompt_tx: mpsc::Sender<String>,
        control_tx: mpsc::Sender<ControlCommand>,
        output_rx: Arc<Mutex<mpsc::Receiver<ClaudeChunk>>>,
        tool_log: Arc<Mutex<Vec<ToolLogEntry>>>,
        pending: Arc<Mutex<Option<PendingToolInfo>>>,
        pending_tx: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
        backend_session_id: Arc<Mutex<Option<String>>>,
        request: ClaudeRequest,
    }

    #[derive(Clone, Copy)]
    enum ControlCommand {
        Stop,
        Pause,
    }

    #[derive(Clone)]
    struct ProcessProvider {
        id: String,
        name: String,
        base_env: HashMap<String, String>,
        executor: Executor,
        executable: ExecutableConfig,
        sessions: Arc<RwLock<HashMap<String, ProcessSession>>>,
    }

    impl ProcessProvider {
        fn new(id: impl Into<String>, name: impl Into<String>, base_env: HashMap<String, String>) -> Result<Self, ClaudeError> {
            Ok(Self {
                id: id.into(),
                name: name.into(),
                base_env,
                executor: Executor::new()?,
                executable: ExecutableConfig::default(),
                sessions: Arc::new(RwLock::new(HashMap::new())),
            })
        }

        fn with_executable(mut self, executable: ExecutableConfig) -> Self {
            self.executable = executable;
            self
        }

        fn session(&self, session_id: &str) -> Result<ProcessSession, ClaudeError> {
            let guard = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            guard.get(session_id).cloned().ok_or(ClaudeError::SessionNotFound)
        }

        fn update_state(state: &Arc<RwLock<SessionState>>, next: SessionState) {
            let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
            *guard = next;
        }

        fn map_result_usage(result: &SdkResultMessage) -> (Option<ClaudeUsage>, u64) {
            match result {
                SdkResultMessage::Success(success) => {
                    let usage = ClaudeUsage {
                        input_tokens: success.usage.input_tokens,
                        output_tokens: success.usage.output_tokens,
                        cache_read_tokens: success.usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write_tokens: success.usage.cache_creation_input_tokens.unwrap_or(0),
                        total_tokens: success.usage.input_tokens + success.usage.output_tokens,
                    };
                    (Some(usage), usd_to_microusd(success.total_cost_usd))
                }
                SdkResultMessage::ErrorDuringExecution(error)
                | SdkResultMessage::ErrorMaxTurns(error)
                | SdkResultMessage::ErrorMaxBudget(error)
                | SdkResultMessage::ErrorMaxStructuredOutputRetries(error) => {
                    let usage = ClaudeUsage {
                        input_tokens: error.usage.input_tokens,
                        output_tokens: error.usage.output_tokens,
                        cache_read_tokens: error.usage.cache_read_input_tokens.unwrap_or(0),
                        cache_write_tokens: error.usage.cache_creation_input_tokens.unwrap_or(0),
                        total_tokens: error.usage.input_tokens + error.usage.output_tokens,
                    };
                    (Some(usage), usd_to_microusd(error.total_cost_usd))
                }
            }
        }

        fn spawn_session(&self, session_id: String, request: ClaudeRequest) -> Result<(), ClaudeError> {
            let (prompt_tx, prompt_rx) = mpsc::channel(128);
            let (control_tx, mut control_rx) = mpsc::channel(32);
            let (output_tx, output_rx) = mpsc::channel(256);
            let state = Arc::new(RwLock::new(SessionState::Creating { started_at: Timestamp::now() }));
            let tool_log = Arc::new(Mutex::new(Vec::new()));
            let pending = Arc::new(Mutex::new(None));
            let pending_tx = Arc::new(Mutex::new(None));
            let response = Arc::new(Mutex::new(None));
            let usage = Arc::new(Mutex::new(None));
            let cost_usd = Arc::new(Mutex::new(0));
            let backend_session_id = Arc::new(Mutex::new(None));

            let session = ProcessSession {
                state: state.clone(),
                prompt_tx: prompt_tx.clone(),
                control_tx: control_tx.clone(),
                output_rx: Arc::new(Mutex::new(output_rx)),
                tool_log: tool_log.clone(),
                pending: pending.clone(),
                pending_tx: pending_tx.clone(),
                backend_session_id: backend_session_id.clone(),
                request: request.clone(),
            };

            self.sessions
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(session_id.clone(), session.clone());

            let executor = self.executor.clone();
            let provider_id = self.id.clone();
            let base_env = self.base_env.clone();
            let executable = self.executable.clone();
            executor.spawn(async move {
                let handler = build_permission_handler(
                    request.internal.tool_policy.clone(),
                    state.clone(),
                    tool_log.clone(),
                    pending.clone(),
                    pending_tx.clone(),
                );

                let mut options = QueryOptions::new();
                options.model = Some(request.model.clone());
                if let Some(system_prompt) = request.system_prompt.as_ref() {
                    options.system_prompt = Some(claude_agent_sdk::options::SystemPromptConfig::Custom(
                        system_prompt.clone(),
                    ));
                }
                if let Some(max_cost) = request.max_cost_usd {
                    options.max_budget_usd = Some(max_cost as f64 / 1_000_000.0);
                }
                if !request.tools.is_empty() {
                    let tools: Vec<String> = request.tools.iter().map(|t| t.name.clone()).collect();
                    options.tools = Some(ToolsConfig::list(tools));
                }
                if !request.internal.tool_policy.allowed.is_empty() {
                    options.allowed_tools = Some(request.internal.tool_policy.allowed.clone());
                }
                if !request.internal.tool_policy.blocked.is_empty() {
                    options.disallowed_tools = Some(request.internal.tool_policy.blocked.clone());
                }
                options.include_partial_messages = true;
                if let Some(resume_backend_id) = request.internal.resume_backend_id.clone() {
                    options.resume = Some(resume_backend_id);
                    options.fork_session = request.internal.fork;
                }
                options.env = Some(base_env);
                let executable = request
                    .internal
                    .executable
                    .clone()
                    .unwrap_or(executable);
                options.executable = executable;
                let prompt = request.initial_prompt.clone().unwrap_or_default();

                let mut query = match Query::new(prompt, options, Some(handler)).await {
                    Ok(query) => query,
                    Err(err) => {
                        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::Failed {
                            error: err.to_string(),
                            at: Timestamp::now(),
                        };
                        return;
                    }
                };

                let _ = query.stream_input(ReceiverStream::new(prompt_rx)).await;

                loop {
                    tokio::select! {
                        Some(cmd) = control_rx.recv() => {
                            match cmd {
                                ControlCommand::Stop => {
                                    let _ = query.interrupt().await;
                                    let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Failed { error: "stopped".to_string(), at: Timestamp::now() };
                                    break;
                                }
                                ControlCommand::Pause => {
                                    let _ = query.interrupt().await;
                                    let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Idle { last_response_at: Timestamp::now(), response: response.lock().unwrap_or_else(|e| e.into_inner()).clone(), usage: usage.lock().unwrap_or_else(|e| e.into_inner()).clone(), cost_usd: *cost_usd.lock().unwrap_or_else(|e| e.into_inner()) };
                                }
                            }
                        }
                        msg = query.next() => {
                            let Some(msg) = msg else { break; };
                            match msg {
                                Ok(SdkMessage::System(SdkSystemMessage::Init(init))) => {
                                    *backend_session_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(init.session_id.clone());
                                    ProcessProvider::update_state(&state, SessionState::Ready { created_at: Timestamp::now() });
                                }
                                Ok(SdkMessage::StreamEvent(event)) => {
                                    if let Some(delta) = extract_delta_from_event(&event.event) {
                                        let chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type: ChunkType::Text, delta: Some(delta), tool: None, usage: None };
                                        let _ = output_tx.send(chunk).await;
                                    }
                                }
                                Ok(SdkMessage::Assistant(assistant)) => {
                                    if let Some(text) = extract_text_from_value(&assistant.message) {
                                        {
                                            let mut guard = response.lock().unwrap_or_else(|e| e.into_inner());
                                            *guard = Some(text.clone());
                                        }
                                        let chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type: ChunkType::Text, delta: Some(text), tool: None, usage: None };
                                        let _ = output_tx.send(chunk).await;
                                    }
                                    ProcessProvider::update_state(&state, SessionState::Working { started_at: Timestamp::now(), current_tool: None });
                                }
                                Ok(SdkMessage::ToolProgress(progress)) => {
                                    let chunk = ClaudeChunk {
                                        session_id: session_id.clone(),
                                        chunk_type: ChunkType::ToolStart,
                                        delta: None,
                                        tool: Some(ToolChunk { name: progress.tool_name.clone(), params: None, result: None, error: None }),
                                        usage: None,
                                    };
                                    let _ = output_tx.send(chunk).await;
                                }
                                Ok(SdkMessage::Result(result)) => {
                                    let (usage_value, cost_value) = ProcessProvider::map_result_usage(&result);
                                    *usage.lock().unwrap_or_else(|e| e.into_inner()) = usage_value.clone();
                                    *cost_usd.lock().unwrap_or_else(|e| e.into_inner()) = cost_value;
                                    let (status, response_text) = match &result {
                                        SdkResultMessage::Success(success) => {
                                            if success.is_error {
                                                (ClaudeSessionStatus::Failed { error: "execution error".to_string() }, None)
                                            } else {
                                                (ClaudeSessionStatus::Complete, Some(success.result.clone()))
                                            }
                                        }
                                        SdkResultMessage::ErrorDuringExecution(error)
                                        | SdkResultMessage::ErrorMaxTurns(error)
                                        | SdkResultMessage::ErrorMaxBudget(error)
                                        | SdkResultMessage::ErrorMaxStructuredOutputRetries(error) => {
                                            let message = if error.errors.is_empty() {
                                                "execution error".to_string()
                                            } else {
                                                error.errors.join("; ")
                                            };
                                            (ClaudeSessionStatus::Failed { error: message }, None)
                                        }
                                    };
                                    if let Some(text) = response_text.clone() {
                                        *response.lock().unwrap_or_else(|e| e.into_inner()) = Some(text.clone());
                                    }
                                    let response_value = ClaudeResponse {
                                        session_id: session_id.clone(),
                                        status: status.clone(),
                                        response: response_text.clone(),
                                        usage: usage_value.clone(),
                                        cost_usd: cost_value,
                                        reserved_usd: 0,
                                        provider_id: provider_id.clone(),
                                        model: request.model.clone(),
                                        tunnel_endpoint: request.tunnel_endpoint.clone(),
                                    };
                                    ProcessProvider::update_state(&state, SessionState::Complete(response_value));
                                    let chunk_type = match status {
                                        ClaudeSessionStatus::Failed { .. } => ChunkType::Error,
                                        _ => ChunkType::Done,
                                    };
                                    let done_chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type, delta: response_text, tool: None, usage: usage_value };
                                    let _ = output_tx.send(done_chunk).await;
                                }
                                Ok(_) => {}
                                Err(err) => {
                                    ProcessProvider::update_state(&state, SessionState::Failed { error: err.to_string(), at: Timestamp::now() });
                                    let err_chunk = ClaudeChunk { session_id: session_id.clone(), chunk_type: ChunkType::Error, delta: Some(err.to_string()), tool: None, usage: None };
                                    let _ = output_tx.send(err_chunk).await;
                                }
                            }
                        }
                    }
                }
            });

            Ok(())
        }
    }

    fn build_permission_handler(
        policy: ToolPolicy,
        state: Arc<RwLock<SessionState>>,
        tool_log: Arc<Mutex<Vec<ToolLogEntry>>>,
        pending: Arc<Mutex<Option<PendingToolInfo>>>,
        pending_tx: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
    ) -> Arc<dyn claude_agent_sdk::permissions::PermissionHandler> {
        let handler = move |request: PermissionRequest| {
            let policy = policy.clone();
            let state = state.clone();
            let tool_log = tool_log.clone();
            let pending = pending.clone();
            let pending_tx = pending_tx.clone();
            async move {
                let tool_name = request.tool_name.clone();
                let params = request.input.clone();

                if !policy.allowed.is_empty() && !policy.allowed.iter().any(|t| t == &tool_name) {
                    tool_log.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                        tool_use_id: request.tool_use_id.clone(),
                        tool: tool_name.clone(),
                        params,
                        approved: Some(false),
                        error: Some("tool not allowed".to_string()),
                        timestamp: Timestamp::now(),
                    });
                    return Ok(PermissionResult::deny("tool not allowed"));
                }

                if policy.blocked.iter().any(|t| t == &tool_name) {
                    tool_log.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                        tool_use_id: request.tool_use_id.clone(),
                        tool: tool_name.clone(),
                        params,
                        approved: Some(false),
                        error: Some("tool blocked".to_string()),
                        timestamp: Timestamp::now(),
                    });
                    return Ok(PermissionResult::deny("tool blocked"));
                }

                if policy.autonomy == ClaudeSessionAutonomy::ReadOnly {
                    tool_log.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                        tool_use_id: request.tool_use_id.clone(),
                        tool: tool_name.clone(),
                        params,
                        approved: Some(false),
                        error: Some("read-only".to_string()),
                        timestamp: Timestamp::now(),
                    });
                    return Ok(PermissionResult::deny("read-only"));
                }

                let requires_approval = matches!(policy.autonomy, ClaudeSessionAutonomy::Restricted)
                    || (policy.autonomy == ClaudeSessionAutonomy::Supervised
                        && policy.approval_required.iter().any(|t| t == &tool_name));

                if requires_approval {
                    let (tx, rx) = oneshot::channel();
                    {
                        let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
                        if guard.is_some() {
                            return Ok(PermissionResult::deny("tool approval already pending"));
                        }
                        *guard = Some(PendingToolInfo {
                            tool_use_id: request.tool_use_id.clone(),
                            tool: tool_name.clone(),
                            params: params.clone(),
                            requested_at: Timestamp::now(),
                        });
                        *pending_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
                    }
                    {
                        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::PendingApproval {
                            tool: tool_name.clone(),
                            params: params.clone(),
                            since: Timestamp::now(),
                        };
                    }
                    tool_log.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                        tool_use_id: request.tool_use_id.clone(),
                        tool: tool_name.clone(),
                        params: params.clone(),
                        approved: None,
                        error: None,
                        timestamp: Timestamp::now(),
                    });

                    let approved = rx.await.unwrap_or(false);
                    tool_log.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                        tool_use_id: request.tool_use_id.clone(),
                        tool: tool_name.clone(),
                        params,
                        approved: Some(approved),
                        error: if approved { None } else { Some("denied".to_string()) },
                        timestamp: Timestamp::now(),
                    });
                    {
                        let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
                        *guard = None;
                    }
                    {
                        let mut guard = state.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::Working {
                            started_at: Timestamp::now(),
                            current_tool: Some(tool_name.clone()),
                        };
                    }

                    if approved {
                        Ok(PermissionResult::allow(request.input.clone()))
                    } else {
                        Ok(PermissionResult::deny_and_interrupt("tool denied"))
                    }
                } else {
                    tool_log.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                        tool_use_id: request.tool_use_id.clone(),
                        tool: tool_name.clone(),
                        params,
                        approved: Some(true),
                        error: None,
                        timestamp: Timestamp::now(),
                    });
                    Ok(PermissionResult::allow(request.input.clone()))
                }
            }
        };
        Arc::new(CallbackPermissionHandler::new(handler))
    }

    #[derive(Clone)]
    pub struct LocalProvider {
        inner: ProcessProvider,
    }

    impl LocalProvider {
        pub fn new() -> Result<Self, ClaudeError> {
            let inner = ProcessProvider::new("local", "Local Claude", HashMap::new())?;
            Ok(Self { inner })
        }

        pub fn with_executable(mut self, executable: ExecutableConfig) -> Self {
            self.inner = self.inner.with_executable(executable);
            self
        }
    }

    impl ClaudeProvider for LocalProvider {
        fn id(&self) -> &str {
            &self.inner.id
        }

        fn info(&self) -> ClaudeProviderInfo {
            ClaudeProviderInfo {
                id: self.inner.id.clone(),
                name: self.inner.name.clone(),
                models: Vec::new(),
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

        fn supports_model(&self, _model: &str) -> bool {
            true
        }

        fn create_session(&self, mut request: ClaudeRequest) -> Result<String, ClaudeError> {
            if let Some(container) = request.internal.container.clone() {
                request.internal.executable = Some(container_executable(&container, &self.inner.base_env));
            }
            let session_id = uuid::Uuid::new_v4().to_string();
            if let Some(resume_id) = request.resume_session_id.as_ref() {
                let session = self.inner.session(resume_id)?;
                let backend = session
                    .backend_session_id
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                    .ok_or_else(|| ClaudeError::ProviderError("resume session not ready".to_string()))?;
                request.internal.resume_backend_id = Some(backend);
            }
            self.inner.spawn_session(session_id.clone(), request)?;
            Ok(session_id)
        }

        fn get_session(&self, session_id: &str) -> Option<SessionState> {
            let session = self.inner.session(session_id).ok()?;
            Some(session.state.read().unwrap_or_else(|e| e.into_inner()).clone())
        }

        fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.prompt_tx.try_send(prompt.to_string());
            ProcessProvider::update_state(
                &session.state,
                SessionState::Working {
                    started_at: Timestamp::now(),
                    current_tool: None,
                },
            );
            Ok(())
        }

        fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
            Ok(rx.try_recv().ok())
        }

        fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let tx = session.pending_tx.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(tx) = tx {
                let _ = tx.send(approved);
                Ok(())
            } else {
                Err(ClaudeError::ProviderError("no pending tool".to_string()))
            }
        }

        fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut request = session.request.clone();
            request.resume_session_id = Some(session_id.to_string());
            request.internal.fork = true;
            self.create_session(request)
        }

        fn stop(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Stop);
            Ok(())
        }

        fn pause(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Pause);
            Ok(())
        }

        fn resume(&self, _session_id: &str) -> Result<(), ClaudeError> {
            Ok(())
        }

        fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>> {
            let session = self.inner.session(session_id).ok()?;
            Some(session.tool_log.lock().unwrap_or_else(|e| e.into_inner()).clone())
        }

        fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo> {
            let session = self.inner.session(session_id).ok()?;
            session.pending.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }
    }

    #[derive(Clone)]
    pub struct CloudProvider {
        inner: ProcessProvider,
        api_key: Option<String>,
    }

    impl CloudProvider {
        pub fn new(api_key: Option<String>) -> Result<Self, ClaudeError> {
            let mut env = HashMap::new();
            if let Some(ref key) = api_key {
                env.insert("ANTHROPIC_API_KEY".to_string(), key.clone());
            }
            let inner = ProcessProvider::new("cloud", "Anthropic Cloud", env)?;
            Ok(Self { inner, api_key })
        }

        pub fn from_env() -> Result<Self, ClaudeError> {
            let key = std::env::var("ANTHROPIC_API_KEY").ok();
            Self::new(key)
        }
    }

    impl ClaudeProvider for CloudProvider {
        fn id(&self) -> &str {
            &self.inner.id
        }

        fn info(&self) -> ClaudeProviderInfo {
            ClaudeProviderInfo {
                id: self.inner.id.clone(),
                name: self.inner.name.clone(),
                models: Vec::new(),
                capabilities: ClaudeCapabilities {
                    streaming: true,
                    resume: true,
                    fork: true,
                    tools: true,
                    vision: false,
                },
                pricing: None,
                status: if self.api_key.is_some() {
                    ClaudeProviderStatus::Available
                } else {
                    ClaudeProviderStatus::Unavailable {
                        reason: "missing ANTHROPIC_API_KEY".to_string(),
                    }
                },
            }
        }

        fn is_available(&self) -> bool {
            self.api_key.is_some()
        }

        fn supports_model(&self, _model: &str) -> bool {
            true
        }

        fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError> {
            if self.api_key.is_none() {
                return Err(ClaudeError::ProviderError("missing ANTHROPIC_API_KEY".to_string()));
            }
            let mut request = request;
            if let Some(container) = request.internal.container.clone() {
                request.internal.executable = Some(container_executable(&container, &self.inner.base_env));
            }
            if let Some(resume_id) = request.resume_session_id.as_ref() {
                let session = self.inner.session(resume_id)?;
                let backend = session
                    .backend_session_id
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                    .ok_or_else(|| ClaudeError::ProviderError("resume session not ready".to_string()))?;
                request.internal.resume_backend_id = Some(backend);
            }
            let session_id = uuid::Uuid::new_v4().to_string();
            self.inner.spawn_session(session_id.clone(), request)?;
            Ok(session_id)
        }

        fn get_session(&self, session_id: &str) -> Option<SessionState> {
            let session = self.inner.session(session_id).ok()?;
            Some(session.state.read().unwrap_or_else(|e| e.into_inner()).clone())
        }

        fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.prompt_tx.try_send(prompt.to_string());
            ProcessProvider::update_state(
                &session.state,
                SessionState::Working {
                    started_at: Timestamp::now(),
                    current_tool: None,
                },
            );
            Ok(())
        }

        fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
            Ok(rx.try_recv().ok())
        }

        fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let tx = session.pending_tx.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some(tx) = tx {
                let _ = tx.send(approved);
                Ok(())
            } else {
                Err(ClaudeError::ProviderError("no pending tool".to_string()))
            }
        }

        fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError> {
            let session = self.inner.session(session_id)?;
            let mut request = session.request.clone();
            request.resume_session_id = Some(session_id.to_string());
            request.internal.fork = true;
            self.create_session(request)
        }

        fn stop(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Stop);
            Ok(())
        }

        fn pause(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.inner.session(session_id)?;
            let _ = session.control_tx.try_send(ControlCommand::Pause);
            Ok(())
        }

        fn resume(&self, _session_id: &str) -> Result<(), ClaudeError> {
            Ok(())
        }

        fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>> {
            let session = self.inner.session(session_id).ok()?;
            Some(session.tool_log.lock().unwrap_or_else(|e| e.into_inner()).clone())
        }

        fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo> {
            let session = self.inner.session(session_id).ok()?;
            session.pending.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    enum TunnelMessage {
        Auth { response: TunnelAuthResponse },
        CreateSession { request: ClaudeRequest, session_id: String },
        SessionCreated { session_id: String },
        Prompt { session_id: String, content: String },
        Chunk(ClaudeChunk),
        ToolApproval { session_id: String, tool: String, params: serde_json::Value },
        ToolApprovalResponse { session_id: String, approved: bool },
        Stop { session_id: String },
        Pause { session_id: String },
        Resume { session_id: String },
        Error { session_id: String, error: String },
    }

    #[derive(Clone)]
    struct TunnelSession {
        state: Arc<RwLock<SessionState>>,
        output_rx: Arc<Mutex<mpsc::Receiver<ClaudeChunk>>>,
        sender: mpsc::Sender<TunnelMessage>,
        tool_log: Arc<Mutex<Vec<ToolLogEntry>>>,
        pending: Arc<Mutex<Option<PendingToolInfo>>>,
        current_tool_id: Arc<Mutex<Option<String>>>,
        request: ClaudeRequest,
    }

    #[derive(Clone)]
    pub struct TunnelProvider {
        endpoints: Arc<RwLock<Vec<TunnelEndpoint>>>,
        auth_state: Arc<RwLock<TunnelAuthState>>,
        executor: Executor,
        sessions: Arc<RwLock<HashMap<String, TunnelSession>>>,
    }

    impl TunnelProvider {
        pub fn new(
            endpoints: Arc<RwLock<Vec<TunnelEndpoint>>>,
            auth_state: Arc<RwLock<TunnelAuthState>>,
        ) -> Result<Self, ClaudeError> {
            Ok(Self {
                endpoints,
                auth_state,
                executor: Executor::new()?,
                sessions: Arc::new(RwLock::new(HashMap::new())),
            })
        }

        fn session(&self, session_id: &str) -> Result<TunnelSession, ClaudeError> {
            let guard = self.sessions.read().unwrap_or_else(|e| e.into_inner());
            guard.get(session_id).cloned().ok_or(ClaudeError::SessionNotFound)
        }

        fn endpoint_for(&self, id: &str) -> Result<TunnelEndpoint, ClaudeError> {
            let guard = self.endpoints.read().unwrap_or_else(|e| e.into_inner());
            guard
                .iter()
                .find(|e| e.id == id)
                .cloned()
                .ok_or_else(|| ClaudeError::ProviderError("tunnel not found".to_string()))
        }

        fn auth_for(&self, tunnel_id: &str) -> Option<TunnelAuthResponse> {
            let now = Timestamp::now();
            let mut auth = self.auth_state.write().unwrap_or_else(|e| e.into_inner());
            let challenge = auth.challenges.get(tunnel_id)?;
            if challenge.expires_at.as_millis() <= now.as_millis() {
                auth.responses.remove(tunnel_id);
                return None;
            }
            let response = auth.responses.get(tunnel_id)?;
            if response.challenge != challenge.challenge {
                auth.responses.remove(tunnel_id);
                return None;
            }
            Some(response.clone())
        }

        fn spawn_connection(&self, endpoint: TunnelEndpoint, session_id: String, request: ClaudeRequest) -> Result<TunnelSession, ClaudeError> {
            let (sender_tx, mut sender_rx) = mpsc::channel(128);
            let (output_tx, output_rx) = mpsc::channel(256);
            let state = Arc::new(RwLock::new(SessionState::Creating { started_at: Timestamp::now() }));
            let tool_log = Arc::new(Mutex::new(Vec::new()));
            let pending = Arc::new(Mutex::new(None));
            let current_tool_id = Arc::new(Mutex::new(None));
            let response = Arc::new(Mutex::new(String::new()));
            let usage = Arc::new(Mutex::new(None));
            let auth = self.auth_for(&endpoint.id);

            let executor = self.executor.clone();
            let session_id_clone = session_id.clone();
            let state_clone = state.clone();
            let pending_clone = pending.clone();
            let tool_log_clone = tool_log.clone();
            let current_tool_id_clone = current_tool_id.clone();
            let response_clone = response.clone();
            let usage_clone = usage.clone();
            let request_clone = request.clone();
            executor.spawn(async move {
                let (ws_stream, _) = match tokio_tungstenite::connect_async(&endpoint.url).await {
                    Ok(result) => result,
                    Err(err) => {
                        let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                        *guard = SessionState::Failed { error: err.to_string(), at: Timestamp::now() };
                        return;
                    }
                };
                let (mut write, mut read) = ws_stream.split();

                if let Some(response) = auth.clone() {
                    let auth_msg = TunnelMessage::Auth { response };
                    let _ = write.send(WsMessage::Text(serde_json::to_string(&auth_msg).unwrap_or_default())).await;
                }

                let create_msg = TunnelMessage::CreateSession { request: request_clone.clone(), session_id: session_id_clone.clone() };
                let _ = write.send(WsMessage::Text(serde_json::to_string(&create_msg).unwrap_or_default())).await;

                let write_task = tokio::spawn(async move {
                    while let Some(msg) = sender_rx.recv().await {
                        let payload = serde_json::to_string(&msg).unwrap_or_default();
                        if write.send(WsMessage::Text(payload)).await.is_err() {
                            break;
                        }
                    }
                });

                while let Some(msg) = read.next().await {
                    let msg = match msg {
                        Ok(WsMessage::Text(text)) => serde_json::from_str::<TunnelMessage>(&text).ok(),
                        Ok(WsMessage::Binary(bin)) => serde_json::from_slice::<TunnelMessage>(&bin).ok(),
                        _ => None,
                    };
                    let Some(msg) = msg else { continue; };
                    match msg {
                        TunnelMessage::SessionCreated { .. } => {
                            let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                            *guard = SessionState::Ready { created_at: Timestamp::now() };
                        }
                        TunnelMessage::Chunk(chunk) => {
                            let chunk_type = chunk.chunk_type.clone();
                            let tool = chunk.tool.clone();
                            let delta = chunk.delta.clone();
                            let usage_value = chunk.usage.clone();

                            if let Some(delta) = delta.as_ref() {
                                if matches!(chunk_type, ChunkType::Text | ChunkType::Done) {
                                    let mut guard = response_clone.lock().unwrap_or_else(|e| e.into_inner());
                                    guard.push_str(delta);
                                }
                            }
                            if let Some(usage) = usage_value.clone() {
                                *usage_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(usage);
                            }

                            match chunk_type {
                                ChunkType::Text => {
                                    let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Working {
                                        started_at: Timestamp::now(),
                                        current_tool: None,
                                    };
                                }
                                ChunkType::ToolStart => {
                                    if let Some(tool) = tool.clone() {
                                        let tool_use_id = {
                                            let mut guard = current_tool_id_clone
                                                .lock()
                                                .unwrap_or_else(|e| e.into_inner());
                                            if let Some(existing) = guard.clone() {
                                                existing
                                            } else {
                                                let id = uuid::Uuid::new_v4().to_string();
                                                *guard = Some(id.clone());
                                                id
                                            }
                                        };
                                        tool_log_clone.lock().unwrap_or_else(|e| e.into_inner()).push(
                                            ToolLogEntry {
                                                tool_use_id,
                                                tool: tool.name.clone(),
                                                params: tool.params.clone().unwrap_or(serde_json::Value::Null),
                                                approved: None,
                                                error: None,
                                                timestamp: Timestamp::now(),
                                            },
                                        );
                                        let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                                        *guard = SessionState::Working {
                                            started_at: Timestamp::now(),
                                            current_tool: Some(tool.name),
                                        };
                                    }
                                }
                                ChunkType::ToolDone => {
                                    if let Some(tool) = tool.clone() {
                                        let tool_use_id = {
                                            let mut guard = current_tool_id_clone
                                                .lock()
                                                .unwrap_or_else(|e| e.into_inner());
                                            guard.take().unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
                                        };
                                        tool_log_clone.lock().unwrap_or_else(|e| e.into_inner()).push(
                                            ToolLogEntry {
                                                tool_use_id,
                                                tool: tool.name.clone(),
                                                params: tool.params.clone().unwrap_or(serde_json::Value::Null),
                                                approved: Some(tool.error.is_none()),
                                                error: tool.error.clone(),
                                                timestamp: Timestamp::now(),
                                            },
                                        );
                                        let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                                        *guard = SessionState::Working {
                                            started_at: Timestamp::now(),
                                            current_tool: None,
                                        };
                                    }
                                }
                                ChunkType::Done => {
                                    let response_text = {
                                        let guard = response_clone.lock().unwrap_or_else(|e| e.into_inner());
                                        if guard.is_empty() {
                                            None
                                        } else {
                                            Some(guard.clone())
                                        }
                                    };
                                    let usage_value = usage_clone.lock().unwrap_or_else(|e| e.into_inner()).clone();
                                    let cost_usd = request_clone.max_cost_usd.unwrap_or(0);
                                    let response = ClaudeResponse {
                                        session_id: session_id_clone.clone(),
                                        status: ClaudeSessionStatus::Complete,
                                        response: response_text,
                                        usage: usage_value,
                                        cost_usd,
                                        reserved_usd: cost_usd,
                                        provider_id: "tunnel".to_string(),
                                        model: request_clone.model.clone(),
                                        tunnel_endpoint: request_clone.tunnel_endpoint.clone(),
                                    };
                                    let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Complete(response);
                                    *pending_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                    *current_tool_id_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                }
                                ChunkType::Error => {
                                    let error = delta.unwrap_or_else(|| "error".to_string());
                                    let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                                    *guard = SessionState::Failed {
                                        error,
                                        at: Timestamp::now(),
                                    };
                                    *pending_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                    *current_tool_id_clone.lock().unwrap_or_else(|e| e.into_inner()) = None;
                                }
                                _ => {}
                            }

                            let _ = output_tx.send(chunk).await;
                        }
                        TunnelMessage::ToolApproval { tool, params, .. } => {
                            let tool_use_id = uuid::Uuid::new_v4().to_string();
                            let info = PendingToolInfo {
                                tool_use_id: tool_use_id.clone(),
                                tool: tool.clone(),
                                params: params.clone(),
                                requested_at: Timestamp::now(),
                            };
                            *pending_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(info.clone());
                            let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                            *guard = SessionState::PendingApproval { tool, params, since: Timestamp::now() };
                            tool_log_clone.lock().unwrap_or_else(|e| e.into_inner()).push(ToolLogEntry {
                                tool_use_id,
                                tool: info.tool.clone(),
                                params: info.params.clone(),
                                approved: None,
                                error: None,
                                timestamp: Timestamp::now(),
                            });
                        }
                        TunnelMessage::Error { error, .. } => {
                            let mut guard = state_clone.write().unwrap_or_else(|e| e.into_inner());
                            *guard = SessionState::Failed { error, at: Timestamp::now() };
                        }
                        _ => {}
                    }
                }
                write_task.abort();
            });

            Ok(TunnelSession {
                state,
                output_rx: Arc::new(Mutex::new(output_rx)),
                sender: sender_tx,
                tool_log,
                pending,
                current_tool_id,
                request,
            })
        }
    }

    impl ClaudeProvider for TunnelProvider {
        fn id(&self) -> &str {
            "tunnel"
        }

        fn info(&self) -> ClaudeProviderInfo {
            ClaudeProviderInfo {
                id: "tunnel".to_string(),
                name: "Tunnel".to_string(),
                models: Vec::new(),
                capabilities: ClaudeCapabilities {
                    streaming: true,
                    resume: true,
                    fork: true,
                    tools: true,
                    vision: true,
                },
                pricing: None,
                status: ClaudeProviderStatus::Available,
            }
        }

        fn is_available(&self) -> bool {
            !self.endpoints.read().unwrap_or_else(|e| e.into_inner()).is_empty()
        }

        fn supports_model(&self, _model: &str) -> bool {
            true
        }

        fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError> {
            let endpoint_id = request
                .tunnel_endpoint
                .as_ref()
                .ok_or(ClaudeError::TunnelRequired)?;
            let endpoint = self.endpoint_for(endpoint_id)?;
            if matches!(endpoint.auth, TunnelAuth::Nostr { .. }) && self.auth_for(&endpoint.id).is_none() {
                return Err(ClaudeError::TunnelAuthRequired);
            }
            let session_id = uuid::Uuid::new_v4().to_string();
            let session = self.spawn_connection(endpoint, session_id.clone(), request)?;
            self.sessions
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .insert(session_id.clone(), session);
            Ok(session_id)
        }

        fn get_session(&self, session_id: &str) -> Option<SessionState> {
            let session = self.session(session_id).ok()?;
            Some(session.state.read().unwrap_or_else(|e| e.into_inner()).clone())
        }

        fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session
                .sender
                .try_send(TunnelMessage::Prompt {
                    session_id: session_id.to_string(),
                    content: prompt.to_string(),
                });
            let mut guard = session.state.write().unwrap_or_else(|e| e.into_inner());
            *guard = SessionState::Working {
                started_at: Timestamp::now(),
                current_tool: None,
            };
            Ok(())
        }

        fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
            let session = self.session(session_id)?;
            let mut rx = session.output_rx.lock().unwrap_or_else(|e| e.into_inner());
            Ok(rx.try_recv().ok())
        }

        fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::ToolApprovalResponse {
                session_id: session_id.to_string(),
                approved,
            });
            if let Some(info) = session
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                session
                    .tool_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(ToolLogEntry {
                        tool_use_id: info.tool_use_id.clone(),
                        tool: info.tool.clone(),
                        params: info.params.clone(),
                        approved: Some(approved),
                        error: if approved { None } else { Some("denied".to_string()) },
                        timestamp: Timestamp::now(),
                    });
                if approved {
                    *session
                        .current_tool_id
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(info.tool_use_id);
                    let mut guard = session.state.write().unwrap_or_else(|e| e.into_inner());
                    *guard = SessionState::Working {
                        started_at: Timestamp::now(),
                        current_tool: Some(info.tool),
                    };
                } else {
                    let mut guard = session.state.write().unwrap_or_else(|e| e.into_inner());
                    *guard = SessionState::Failed {
                        error: "tool denied".to_string(),
                        at: Timestamp::now(),
                    };
                }
            }
            Ok(())
        }

        fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError> {
            let session = self.session(session_id)?;
            let mut request = session.request.clone();
            request.resume_session_id = Some(session_id.to_string());
            request.internal.fork = true;
            self.create_session(request)
        }

        fn stop(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Stop {
                session_id: session_id.to_string(),
            });
            Ok(())
        }

        fn pause(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Pause {
                session_id: session_id.to_string(),
            });
            Ok(())
        }

        fn resume(&self, session_id: &str) -> Result<(), ClaudeError> {
            let session = self.session(session_id)?;
            let _ = session.sender.try_send(TunnelMessage::Resume {
                session_id: session_id.to_string(),
            });
            Ok(())
        }

        fn tool_log(&self, session_id: &str) -> Option<Vec<ToolLogEntry>> {
            let session = self.session(session_id).ok()?;
            Some(session.tool_log.lock().unwrap_or_else(|e| e.into_inner()).clone())
        }

        fn pending_tool(&self, session_id: &str) -> Option<PendingToolInfo> {
            let session = self.session(session_id).ok()?;
            session.pending.lock().unwrap_or_else(|e| e.into_inner()).clone()
        }
    }

}

#[cfg(not(target_arch = "wasm32"))]
pub use providers::{CloudProvider, LocalProvider, TunnelProvider};
