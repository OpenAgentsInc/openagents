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
    PendingApproval {
        tool: String,
        params: serde_json::Value,
    },
}

/// Session state for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionState {
    /// Session being created.
    Creating { started_at: Timestamp },
    /// Ready for prompts.
    Ready { created_at: Timestamp },
    /// Actively working.
    Working {
        started_at: Timestamp,
        current_tool: Option<String>,
    },
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
            SessionState::Failed { error, .. } => ClaudeSessionStatus::Failed {
                error: error.clone(),
            },
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
    Custom {
        denylist: Vec<String>,
        allowlist: Vec<String>,
    },
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

