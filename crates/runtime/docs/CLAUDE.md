# Claude

Portable Claude Agent SDK abstraction for agents.

---

## Overview

Agents access Claude Agent SDK instances through the `/claude` mount. The runtime provides:

- **Provider abstraction** — Same interface for local proxies, cloud API, and tunnel endpoints
- **Tunnel-based auth** — Credentials stay on user's machine; remote agents connect via tunnel
- **Policy enforcement** — Per-agent tool permissions, model restrictions, and autonomy levels
- **Budget tracking** — USD-denominated accounting (consistent with `/compute` and `/containers`)
- **Session management** — Resume sessions, fork from checkpoints, preserve context
- **Streaming output** — Token-by-token output via `watch()` on session streams

This enables agents to spawn and control Claude Agent SDK instances without hardcoding providers or exposing credentials.

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Budget units | **USD** (micro-USD for precision) | Consistent with `/compute` and `/containers` |
| Execution model | **Always returns session_id** | Non-blocking; works on all backends including Cloudflare |
| Auth model | **Tunnel-based proxy** | Credentials stay on user's machine; sandboxes only know tunnel URL |
| Session management | **Resume + Fork** | Claude sessions have valuable context worth preserving |
| Tool permissions | **Policy-controlled** | Agent owner controls what tools Claude can use |
| Interface | **`/claude/new` → session_id** | Avoids blocking; agent polls status or watches output |

---

## Tunnel-Based Proxy Architecture

The core innovation: Claude Agent SDK instances run in sandboxed environments (containers, cloud, DVMs) but connect to Claude through authenticated tunnels pointing to the user's machine.

### User's Local Setup

```
User's Machine
├── Local Claude Proxy
│   ├── Handles authentication (API key, OAuth, etc.)
│   ├── Routes to Claude (Anthropic API or local model)
│   └── Tracks usage and enforces limits
└── Tunnel Endpoint
    ├── Exposed via ngrok, Cloudflare Tunnel, Nostr relay, etc.
    └── Accepts connections from remote agents
```

### Remote Agent Flow

```
Remote Container/Agent
        │
        ▼
Tunnel URL (wss://abc123.ngrok.io/claude)
        │
        ▼
User's Local Proxy
        │
        ▼
Claude (Anthropic API or local model)
```

### Key Benefits

- **Credentials never leave user's machine** — API keys, OAuth tokens stay local
- **User controls all Claude access** — Rate limits, model selection, tool permissions
- **Works with any hosting** — Containers, cloud, DVMs, browser WASM
- **Supports local Claude** — Use local models via proxy for privacy/cost
- **Nostr-native auth** — Remote agent signs requests to prove identity

### Local Proxy Types

| Type | Description |
|------|-------------|
| `claude-code` | User's local claude-code installation with authenticated API key |
| `anthropic-api` | Direct Anthropic API key for self-hosted proxy |
| `local-llm` | Local Claude-compatible model (e.g., via Ollama, llama.cpp) |
| `openrouter` | OpenRouter proxy for model selection |

---

## Security Model

The `/claude` mount implements defense-in-depth security. The core principle: **put sensitive stuff outside the boundary where the agent runs**.

### Network Isolation Pattern

For maximum security, Claude workers run with **no direct network access**:

```
Container (--network none)
├── Claude Agent SDK process
├── Read-only repo (filtered)
├── tmpfs workspace
└── Unix socket mount (/var/run/anthropic-proxy.sock)
         │
         ▼
Host Proxy (outside container)
├── Domain allowlist (api.anthropic.com only)
├── Request logging (redacted)
├── Credential injection
└── Rate limiting
```

The container physically cannot exfiltrate data—even if prompt injection convinces Claude to try.

**Container configuration:**

```bash
docker run \
  --network none \
  --cap-drop ALL \
  --read-only \
  --tmpfs /workspace:rw,noexec,nosuid \
  --pids-limit 100 \
  --memory 4g \
  -v /var/run/anthropic-proxy.sock:/var/run/anthropic-proxy.sock:ro \
  -v /filtered-repo:/repo:ro \
  claude-worker
```

**Inside container, Claude uses the proxy:**

```bash
# Configure Claude to use Unix socket proxy
export ANTHROPIC_BASE_URL="http://unix:/var/run/anthropic-proxy.sock"

# Or via HTTP proxy
export HTTPS_PROXY="http://localhost:8080"

# IMPORTANT: For Node.js-based Claude Code, ensure fetch() respects proxy env vars
export NODE_USE_ENV_PROXY=1
```

**Note:** Claude Code is Node.js-based. Without `NODE_USE_ENV_PROXY=1`, Node's `fetch()` ignores `HTTP_PROXY`/`HTTPS_PROXY` environment variables. This is a common "why isn't my proxy working?" trap in hardened container setups.

### Credential Injection via Proxy

Credentials never enter the container. The proxy injects them:

```
Claude Request (from container)     Host Proxy                    Anthropic API
        │                               │                              │
        │ POST /v1/messages             │                              │
        │ (no auth header)              │                              │
        ├──────────────────────────────►│                              │
        │                               │ + Authorization: Bearer sk-  │
        │                               ├─────────────────────────────►│
        │                               │                              │
        │                               │◄─────────────────────────────┤
        │◄──────────────────────────────┤                              │
```

This matches the pattern from Claude's secure deployment guide: "inject via proxy/tools, don't expose."

### Policy is Admin-Only

**Critical security rule:** Agents can READ policies but NEVER write them.

| Path | Agent Access | Admin Access |
|------|--------------|--------------|
| `/claude/policy` | Read | Read/Write |
| `/claude/auth/tunnels` | Read (summary only) | Read/Write |
| `/compute/policy` | Read | Read/Write |
| `/containers/policy` | Read | Read/Write |

This prevents prompt injection from reconfiguring security boundaries.

### Repo Exposure Filtering

Even read-only mounts can leak secrets. Filter before mounting:

```rust
/// Files to exclude from repo mount
const REPO_DENYLIST: &[&str] = &[
    ".env",
    ".env.*",
    "*.pem",
    "*.key",
    "**/credentials.json",
    "**/secrets.yaml",
    "**/.git/config",  // May contain tokens
];

/// Create filtered repo snapshot for container
fn create_filtered_repo(repo_path: &Path, denylist: &[&str]) -> Result<PathBuf> {
    let filtered = tempdir()?;
    // Copy repo excluding denylist patterns
    // ...
}
```

### Worker Isolation Modes

| Mode | Isolation | Network | Use Case |
|------|-----------|---------|----------|
| `local` | Process | Host network | Dev, trusted agents |
| `container` | Docker | None (proxy socket) | Production, untrusted |
| `gvisor` | gVisor sandbox | None (proxy socket) | High security |
| `firecracker` | microVM | None (proxy socket) | Maximum isolation |

Configure per-agent:

```yaml
mounts:
  /claude:
    type: claude
    policy:
      isolation_mode: container  # or gvisor, firecracker
      network_mode: proxy_only   # no direct egress
      repo_filter: strict        # apply denylist
```

### Containerized CLI Configuration

To run the Claude Agent SDK inside a container, set the following environment variables on the runtime host:

- `OPENAGENTS_CLAUDE_CONTAINER_IMAGE` (required): OCI image that includes the `claude` CLI.
- `OPENAGENTS_CLAUDE_CONTAINER_RUNTIME` (optional): `apple`, `docker`, or `auto` (default). `apple` requires macOS 26+ and the `apple-container` feature.
- `OPENAGENTS_CLAUDE_CONTAINER_COMMAND` (optional): command inside the image (default: `claude`).
- `OPENAGENTS_CLAUDE_PROXY_URL` (optional): HTTP proxy URL; sets `HTTP_PROXY`, `HTTPS_PROXY`, and `NODE_USE_ENV_PROXY=1`.

Container isolation is applied to the `local` and `cloud` providers when `policy.isolation_mode = container`. `network_mode = none` disables networking with `--network none`. `proxy_only` uses the proxy env vars if provided but does not yet hard-block egress on its own.

Default image definition:

- Dockerfile: `docker/claude/Dockerfile`
- Docs: `docs/claude/container-image.md`

Build example:

```bash
container build -t openagents/claude-code:latest -f docker/claude/Dockerfile .
```

### Multi-Instance Worker Pool

For production, run a pool of Claude workers:

```
/claude/
├── workers/
│   ├── <worker_id>/
│   │   ├── status      # idle|busy|unhealthy
│   │   ├── isolation   # local|container|gvisor|firecracker
│   │   ├── sessions    # active session count
│   │   └── metrics     # requests, latency, errors
│   └── ...
├── pool/
│   ├── config          # min/max workers, scaling policy
│   ├── status          # pool health, capacity
│   └── metrics         # aggregate stats
```

The runtime scheduler routes requests to available workers based on:
- Worker health and load
- Requested isolation level
- Agent identity (can pin agents to specific workers)

### Defense in Depth Summary

| Layer | Protection |
|-------|------------|
| **Mount table** | Least privilege, capability allowlist |
| **AccessLevel** | Budget enforcement at boundary |
| **Container isolation** | Code runs in sandbox, not host |
| **Network none** | No direct egress possible |
| **Proxy** | Domain allowlist, credential injection, logging |
| **Repo filtering** | Secrets stripped before mount |
| **Policy isolation** | Agents can't reconfigure security |
| **Trajectories** | Full audit trail of all actions |

This makes prompt injection attacks significantly harder—even if Claude is tricked, the sandbox prevents escalation.

---

## Filesystem Layout

```
/claude/
├── providers/           # Available Claude providers
│   ├── tunnel/         # User's tunnel endpoints
│   │   ├── info        # Read: provider info JSON
│   │   ├── endpoints   # Read: configured tunnel endpoints
│   │   └── health      # Read: provider health status
│   ├── cloud/          # Direct Anthropic API (requires API key)
│   │   ├── info
│   │   ├── models      # Read: available models
│   │   └── health
│   └── local/          # Local Claude proxy (same machine)
│       ├── info
│       ├── models
│       └── health
├── workers/            # Claude worker pool (admin-managed)
│   └── <worker_id>/
│       ├── status      # Read: idle|busy|unhealthy
│       ├── isolation   # Read: local|container|gvisor|firecracker
│       ├── sessions    # Read: active session count
│       └── metrics     # Read: requests, latency, errors
├── pool/               # Worker pool management (admin-only)
│   ├── config          # Read/Write (admin): min/max workers, scaling
│   ├── status          # Read: pool health, capacity
│   └── metrics         # Read: aggregate stats
├── new                  # Call (write+read same handle) → session_id (always async)
├── policy               # Read-only to agents (control plane updates)
├── usage                # Read: current tick/day usage (reserved/spent USD)
├── proxy/              # Host proxy status (for network-none containers)
│   ├── status          # Read: proxy health
│   ├── allowlist       # Read (admin-only write): allowed domains
│   └── metrics         # Read: request counts, blocked attempts
├── auth/               # Tunnel authentication
│   ├── tunnels         # Read (agents) / Write (admin): configured endpoints
│   ├── status          # Read: auth state per tunnel
│   └── challenge       # Read: Nostr auth challenge, Write: signed response
└── sessions/
    └── <session_id>/   # Individual Claude session
        ├── status      # Read: creating|ready|working|idle|complete|failed
        ├── prompt      # Write: send prompt/message
        ├── response    # Read: latest response (when idle/complete)
        ├── output      # Watch: streaming tokens
        ├── context     # Read: conversation context summary
        ├── tools/      # Tool execution tracking
        │   ├── log     # Read: tool execution log (append-only)
        │   ├── pending # Read: tools awaiting approval
        │   └── approve # Write: approve/reject pending tool use
        ├── usage       # Read: session token usage + cost
        ├── fork        # Write: fork session from current state → new session_id
        └── ctl         # Write: "stop", "pause", "resume"
```

**Key principles:**

1. **`/claude/new` always returns a session_id immediately.** Use `env.call("/claude/new", ...)`
   or open a read/write handle so the response is read from the same handle. The agent then reads
   `/claude/sessions/<id>/status` to poll or watches `/claude/sessions/<id>/output` for streaming.

2. **Sessions are stateful.** Unlike `/compute` jobs, Claude sessions maintain conversation history
   and can be resumed or forked.

3. **Tool execution is transparent.** All tool calls are logged to `/tools/log` for auditability.

---

## Core Types

### ClaudeRequest

```rust
/// A Claude session request (provider-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeRequest {
    /// Model identifier (e.g., "claude-sonnet-4-20250514", "claude-opus-4-5-20251101")
    pub model: String,

    /// System prompt for the session
    pub system_prompt: Option<String>,

    /// Initial prompt/message to send
    pub initial_prompt: Option<String>,

    /// Tools available to Claude in this session
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    /// Maximum context tokens (for budget control)
    pub max_context_tokens: Option<u64>,

    /// Tunnel endpoint to use (required for remote execution)
    pub tunnel_endpoint: Option<String>,

    /// Maximum cost in micro-USD the caller is willing to pay.
    /// Budget check reserves this amount; actual cost reconciled on completion.
    pub max_cost_usd: Option<u64>,

    /// Idempotency key for deduplication
    /// Full key is scoped: {agent_id}:{provider_id}:{idempotency_key}
    pub idempotency_key: Option<String>,

    /// Session to resume (if continuing previous work)
    pub resume_session_id: Option<String>,

    /// Timeout in milliseconds for session creation (default: 60000)
    pub timeout_ms: Option<u64>,

    /// Autonomy level for this session (tool-approval mode, distinct from runtime autonomy)
    /// If omitted, the runtime uses policy.default_autonomy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<ClaudeSessionAutonomy>,
}

/// Tool definition for Claude Agent SDK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g., "Read", "Write", "Bash")
    pub name: String,
    /// Tool description
    pub description: Option<String>,
    /// Tool-specific configuration
    pub config: Option<serde_json::Value>,
}

/// Claude session autonomy level (tool-approval mode)
/// Note: This is distinct from runtime AutonomyLevel (Supervised/SemiAutonomous/Autonomous)
/// which controls mount-level access grants. This controls tool approval within a Claude session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeSessionAutonomy {
    /// Full autonomy - Claude executes tools without approval
    Full,
    /// Supervised - Certain tools require approval
    #[default]
    Supervised,
    /// Restricted - All tool use requires approval
    Restricted,
    /// Read-only - No tool execution allowed
    ReadOnly,
}
```

### ClaudeResponse

```rust
/// Claude session response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeResponse {
    /// Session identifier
    pub session_id: String,

    /// Session status
    pub status: ClaudeSessionStatus,

    /// Latest response text (if any)
    pub response: Option<String>,

    /// Token usage
    pub usage: Option<ClaudeUsage>,

    /// Actual cost in micro-USD (post-execution)
    pub cost_usd: u64,

    /// Reserved cost (from max_cost_usd at submission)
    pub reserved_usd: u64,

    /// Provider that handled the request
    pub provider_id: String,

    /// Model used
    pub model: String,

    /// Tunnel endpoint used (if applicable)
    pub tunnel_endpoint: Option<String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
}
```

### ClaudeSessionStatus

```rust
/// Claude session status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaudeSessionStatus {
    /// Session is being created (connecting to tunnel, initializing)
    Creating,
    /// Session is ready, waiting for prompts
    Ready,
    /// Claude is actively working (generating response, using tools)
    Working,
    /// Claude finished current task, waiting for next prompt
    Idle,
    /// Session completed (explicitly ended)
    Complete,
    /// Session failed
    Failed { error: String },
    /// Waiting for tool approval
    PendingApproval { tool: String, params: serde_json::Value },
}
```

### ClaudeChunk (Streaming)

```rust
/// A streaming chunk from Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeChunk {
    /// Session identifier
    pub session_id: String,

    /// Chunk type
    pub chunk_type: ChunkType,

    /// Delta content (for text chunks)
    pub delta: Option<String>,

    /// Tool information (for tool chunks)
    pub tool: Option<ToolChunk>,

    /// Accumulated usage
    pub usage: Option<ClaudeUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    /// Text output from Claude
    Text,
    /// Tool execution starting
    ToolStart,
    /// Tool execution output
    ToolOutput,
    /// Tool execution complete
    ToolDone,
    /// Claude finished response
    Done,
    /// Error occurred
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChunk {
    pub name: String,
    pub params: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
```

---

## ClaudePolicy

Per-agent policy configured via mount table:

```rust
/// Policy for Claude sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePolicy {
    /// Allowed provider IDs (empty = all available)
    #[serde(default)]
    pub allowed_providers: Vec<String>,

    /// Allowed models (empty = all)
    /// Supports glob patterns: "claude-sonnet-4-*", "claude-haiku-*"
    #[serde(default)]
    pub allowed_models: Vec<String>,

    /// Blocked models (takes precedence over allowed)
    /// Supports glob patterns: "claude-opus-*"
    #[serde(default)]
    pub blocked_models: Vec<String>,

    /// Allowed tools for Claude to use
    #[serde(default)]
    pub allowed_tools: Vec<String>,

    /// Blocked tools (takes precedence)
    #[serde(default)]
    pub blocked_tools: Vec<String>,

    /// Tools that require approval before execution
    #[serde(default)]
    pub approval_required_tools: Vec<String>,

    /// Maximum cost per tick in micro-USD
    pub max_cost_usd_per_tick: Option<u64>,

    /// Maximum cost per day in micro-USD
    pub max_cost_usd_per_day: Option<u64>,

    /// Default max_cost_usd if request doesn't specify
    pub default_max_cost_usd: Option<u64>,

    /// Require requests to specify max_cost_usd
    #[serde(default)]
    pub require_max_cost: bool,

    /// Maximum concurrent Claude sessions per agent
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,

    /// Maximum context tokens per session
    pub max_context_tokens: Option<u64>,

    /// Default autonomy level for sessions
    #[serde(default)]
    pub default_autonomy: ClaudeSessionAutonomy,

    /// Require idempotency keys for all requests
    #[serde(default)]
    pub require_idempotency: bool,

    /// Allowed tunnel endpoints (empty = all configured)
    #[serde(default)]
    pub allowed_tunnels: Vec<String>,

    // --- Security settings (admin-controlled) ---

    /// Worker isolation mode
    #[serde(default)]
    pub isolation_mode: IsolationMode,

    /// Network access mode
    #[serde(default)]
    pub network_mode: NetworkMode,

    /// Repo filtering mode
    #[serde(default)]
    pub repo_filter: RepoFilterMode,

    /// Proxy domain allowlist (for network_mode: proxy_only)
    #[serde(default)]
    pub proxy_allowlist: Vec<String>,
}

/// Worker isolation mode
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    /// Process on host (dev only)
    Local,
    /// Docker container with hardening
    #[default]
    Container,
    /// gVisor sandbox (stronger)
    Gvisor,
    /// Firecracker microVM (strongest)
    Firecracker,
}

/// Network access mode
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    /// No network restrictions (dev only, dangerous)
    Host,
    /// Network disabled, only proxy socket (secure)
    #[default]
    ProxyOnly,
    /// Completely air-gapped (no external access)
    None,
}

/// Repo filtering mode
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoFilterMode {
    /// No filtering (dangerous)
    None,
    /// Standard denylist (.env, *.pem, etc.)
    #[default]
    Standard,
    /// Strict denylist + allowlist
    Strict,
    /// Custom filter rules
    Custom { denylist: Vec<String>, allowlist: Vec<String> },
}

fn default_max_concurrent() -> u32 { 3 }
```

### Mount Configuration

```yaml
mounts:
  /claude:
    type: claude
    access: budgeted  # REQUIRED: Claude API costs money
    budget:
      per_tick_usd: 1000000       # $1/tick (in micro-USD)
      per_day_usd: 100000000      # $100/day (in micro-USD)
      approval_threshold_usd: 10000000  # $10 requires approval
    policy:
      # Provider and model restrictions
      allowed_providers: ["tunnel", "cloud"]
      allowed_models: ["claude-sonnet-4-*", "claude-haiku-*"]
      blocked_models: ["claude-opus-*"]  # Opus too expensive for auto

      # Tool permissions
      allowed_tools: ["Read", "Write", "Edit", "Bash", "Glob", "Grep"]
      blocked_tools: ["WebFetch"]  # No external network
      approval_required_tools: ["Bash"]  # Bash needs approval

      # Limits
      max_concurrent: 3
      max_context_tokens: 200000
      default_autonomy: supervised
      require_max_cost: true

      # Security (admin-controlled, agents can't modify)
      isolation_mode: container        # container|gvisor|firecracker|local
      network_mode: proxy_only         # proxy_only|none|host
      repo_filter: standard            # standard|strict|none
      proxy_allowlist:
        - api.anthropic.com
        - api.openai.com  # If using OpenRouter

    tunnels:
      - id: local-proxy
        url: wss://my-tunnel.ngrok.io/claude
        auth: nostr

    # Worker pool configuration (optional)
    pool:
      min_workers: 1
      max_workers: 5
      idle_timeout_secs: 300
      scale_up_threshold: 0.8  # 80% utilization
```

---

## ClaudeProvider Trait

Sync trait for FileService compatibility:

```rust
/// Information about a Claude provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeProviderInfo {
    pub id: String,
    pub name: String,
    pub models: Vec<ClaudeModelInfo>,
    pub capabilities: ClaudeCapabilities,
    pub pricing: Option<ClaudePricing>,
    pub status: ProviderStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeModelInfo {
    pub id: String,
    pub name: String,
    pub context_length: u64,
    pub output_limit: u64,
    pub pricing: ModelPricing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCapabilities {
    /// Supports streaming output
    pub streaming: bool,
    /// Supports session resume
    pub resume: bool,
    /// Supports session fork
    pub fork: bool,
    /// Supports tool use
    pub tools: bool,
    /// Supports vision (image input)
    pub vision: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePricing {
    /// Input tokens per 1k (micro-USD)
    pub input_per_1k_microusd: u64,
    /// Output tokens per 1k (micro-USD)
    pub output_per_1k_microusd: u64,
    /// Cache read per 1k (micro-USD)
    pub cache_read_per_1k_microusd: u64,
    /// Cache write per 1k (micro-USD)
    pub cache_write_per_1k_microusd: u64,
}

/// Claude provider trait (sync for FileService compatibility)
pub trait ClaudeProvider: Send + Sync {
    /// Provider identifier
    fn id(&self) -> &str;

    /// Provider info (models, capabilities, pricing)
    fn info(&self) -> ClaudeProviderInfo;

    /// Check if provider is available
    fn is_available(&self) -> bool;

    /// Check if provider supports a model
    fn supports_model(&self, model: &str) -> bool;

    /// Create a new Claude session. ALWAYS returns session_id immediately.
    fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError>;

    /// Get current state of a session
    fn get_session(&self, session_id: &str) -> Option<SessionState>;

    /// Send a prompt to a session (non-blocking, returns immediately)
    fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError>;

    /// Poll for streaming output
    fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError>;

    /// Approve a pending tool execution
    fn approve_tool(&self, session_id: &str, approved: bool) -> Result<(), ClaudeError>;

    /// Fork a session from current state
    fn fork_session(&self, session_id: &str) -> Result<String, ClaudeError>;

    /// Stop a session
    fn stop(&self, session_id: &str) -> Result<(), ClaudeError>;

    /// Pause a session (preserve state for later resume)
    fn pause(&self, session_id: &str) -> Result<(), ClaudeError>;

    /// Resume a paused session
    fn resume(&self, session_id: &str) -> Result<(), ClaudeError>;
}

/// Session state for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionState {
    /// Session being created
    Creating { started_at: Timestamp },
    /// Ready for prompts
    Ready { created_at: Timestamp },
    /// Actively working
    Working { started_at: Timestamp, current_tool: Option<String> },
    /// Waiting for next prompt
    Idle { last_response_at: Timestamp },
    /// Completed
    Complete(ClaudeResponse),
    /// Failed
    Failed { error: String, at: Timestamp },
    /// Waiting for tool approval
    PendingApproval { tool: String, params: serde_json::Value, since: Timestamp },
}
```

---

## Tunnel Authentication

Remote Claude sessions authenticate with the user's local proxy using Nostr signatures.

### Tunnel Configuration

```rust
/// Tunnel endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelEndpoint {
    /// Unique identifier
    pub id: String,
    /// WebSocket URL for tunnel
    pub url: String,
    /// Authentication method
    pub auth: TunnelAuth,
    /// Allowed agent pubkeys (empty = all)
    pub allowed_agents: Vec<String>,
    /// Rate limits
    pub rate_limit: Option<RateLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TunnelAuth {
    /// No authentication (dangerous, local only)
    None,
    /// Nostr signature authentication
    Nostr {
        /// Expected relay for key verification
        relay: Option<String>,
    },
    /// Pre-shared key
    Psk {
        /// Path in secret store
        secret_path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_minute: u32,
    pub tokens_per_minute: u64,
}
```

### Authentication Flow

```rust
/// Nostr tunnel authentication
///
/// 1. Agent reads challenge from /claude/auth/challenge
/// 2. Agent signs challenge with its keypair (via /identity/sign)
/// 3. Agent writes signed response to /claude/auth/challenge
/// 4. Proxy validates signature against known agent pubkey
/// 5. Session authorized, tunnel connection established

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelAuthChallenge {
    /// Random challenge string
    pub challenge: String,
    /// Challenge expiry
    pub expires_at: Timestamp,
    /// Tunnel endpoint ID
    pub tunnel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelAuthResponse {
    /// The challenge that was signed
    pub challenge: String,
    /// Schnorr signature over challenge
    pub signature: String,
    /// Agent's pubkey
    pub pubkey: String,
    /// Tunnel endpoint ID
    pub tunnel_id: String,
}
```

`/claude/auth/status` provides a per-tunnel auth summary (authorized flag, pubkey for Nostr auth,
challenge expiry). Tunnels with `auth: none` are treated as authorized by default.

---

## Backend Implementations

### Tunnel Provider

The primary provider for remote Claude execution:

```rust
/// Tunnel-based Claude provider (connects to user's local proxy)
pub struct TunnelProvider {
    endpoints: Arc<RwLock<Vec<TunnelEndpoint>>>,
    sessions: Arc<RwLock<HashMap<String, TunnelSession>>>,
    executor: Arc<ExecutorManager>,
    signer: Arc<dyn SigningService>,
}

struct TunnelSession {
    session_id: String,
    endpoint_id: String,
    ws_connection: Option<WebSocketConnection>,
    state: SessionState,
    buffer: Vec<ClaudeChunk>,
}

impl ClaudeProvider for TunnelProvider {
    fn id(&self) -> &str { "tunnel" }

    fn info(&self) -> ClaudeProviderInfo {
        ClaudeProviderInfo {
            id: "tunnel".to_string(),
            name: "Tunnel to Local Proxy".to_string(),
            models: self.query_endpoint_models(),
            capabilities: ClaudeCapabilities {
                streaming: true,
                resume: true,
                fork: true,
                tools: true,
                vision: true,
            },
            pricing: None,  // Depends on underlying provider
            status: ProviderStatus::Available,
        }
    }

    fn create_session(&self, request: ClaudeRequest) -> Result<String, ClaudeError> {
        let session_id = generate_session_id();
        let endpoint_id = request.tunnel_endpoint
            .as_ref()
            .ok_or(ClaudeError::TunnelRequired)?;

        // Find endpoint
        let endpoints = self.endpoints.read();
        let endpoint = endpoints.iter()
            .find(|e| &e.id == endpoint_id)
            .ok_or(ClaudeError::TunnelNotFound)?;

        // Authenticate with tunnel
        let auth_result = self.executor.execute(async {
            self.authenticate_tunnel(endpoint).await
        })?;

        // Establish WebSocket connection
        let ws = self.executor.execute(async {
            self.connect_tunnel(endpoint, &auth_result).await
        })?;

        // Send session creation request over tunnel
        let create_msg = TunnelMessage::CreateSession {
            request: request.clone(),
            session_id: session_id.clone(),
        };
        ws.send(&serde_json::to_vec(&create_msg)?)?;

        // Track session
        self.sessions.write().insert(session_id.clone(), TunnelSession {
            session_id: session_id.clone(),
            endpoint_id: endpoint_id.clone(),
            ws_connection: Some(ws),
            state: SessionState::Creating { started_at: Timestamp::now() },
            buffer: vec![],
        });

        Ok(session_id)
    }

    fn send_prompt(&self, session_id: &str, prompt: &str) -> Result<(), ClaudeError> {
        let mut sessions = self.sessions.write();
        let session = sessions.get_mut(session_id)
            .ok_or(ClaudeError::SessionNotFound)?;

        if let Some(ws) = &session.ws_connection {
            let msg = TunnelMessage::Prompt {
                session_id: session_id.to_string(),
                content: prompt.to_string(),
            };
            ws.send(&serde_json::to_vec(&msg)?)?;
            session.state = SessionState::Working {
                started_at: Timestamp::now(),
                current_tool: None,
            };
        }

        Ok(())
    }

    fn poll_output(&self, session_id: &str) -> Result<Option<ClaudeChunk>, ClaudeError> {
        let mut sessions = self.sessions.write();
        let session = sessions.get_mut(session_id)
            .ok_or(ClaudeError::SessionNotFound)?;

        // Check buffer first
        if !session.buffer.is_empty() {
            return Ok(Some(session.buffer.remove(0)));
        }

        // Poll WebSocket for new chunks
        if let Some(ws) = &session.ws_connection {
            if let Some(data) = ws.try_recv()? {
                let msg: TunnelMessage = serde_json::from_slice(&data)?;
                if let TunnelMessage::Chunk(chunk) = msg {
                    return Ok(Some(chunk));
                }
            }
        }

        Ok(None)
    }
}

/// Messages sent over tunnel WebSocket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TunnelMessage {
    /// Create a new Claude session
    CreateSession {
        request: ClaudeRequest,
        session_id: String,
    },
    /// Session created successfully
    SessionCreated {
        session_id: String,
    },
    /// Send prompt to Claude
    Prompt {
        session_id: String,
        content: String,
    },
    /// Streaming chunk from Claude
    Chunk(ClaudeChunk),
    /// Tool approval request
    ToolApproval {
        session_id: String,
        tool: String,
        params: serde_json::Value,
    },
    /// Tool approval response
    ToolApprovalResponse {
        session_id: String,
        approved: bool,
    },
    /// Stop session
    Stop { session_id: String },
    /// Error
    Error { session_id: String, error: String },
}
```

### Cloud Provider

Direct Anthropic API access (requires API key in secret store):

```rust
/// Direct Anthropic API provider
pub struct CloudProvider {
    executor: Arc<ExecutorManager>,
    sessions: Arc<RwLock<HashMap<String, CloudSession>>>,
    api_key_path: String,  // Path in secret store
}

impl ClaudeProvider for CloudProvider {
    fn id(&self) -> &str { "cloud" }

    fn info(&self) -> ClaudeProviderInfo {
        ClaudeProviderInfo {
            id: "cloud".to_string(),
            name: "Anthropic Cloud API".to_string(),
            models: vec![
                ClaudeModelInfo {
                    id: "claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4".to_string(),
                    context_length: 200_000,
                    output_limit: 16_000,
                    pricing: ModelPricing {
                        input_per_1k_microusd: 3000,   // $3/MTok
                        output_per_1k_microusd: 15000,  // $15/MTok
                    },
                },
                ClaudeModelInfo {
                    id: "claude-opus-4-5-20251101".to_string(),
                    name: "Claude Opus 4.5".to_string(),
                    context_length: 200_000,
                    output_limit: 32_000,
                    pricing: ModelPricing {
                        input_per_1k_microusd: 15000,   // $15/MTok
                        output_per_1k_microusd: 75000,  // $75/MTok
                    },
                },
                // ... more models
            ],
            capabilities: ClaudeCapabilities {
                streaming: true,
                resume: true,
                fork: true,
                tools: true,
                vision: true,
            },
            pricing: Some(ClaudePricing {
                input_per_1k_microusd: 3000,
                output_per_1k_microusd: 15000,
                cache_read_per_1k_microusd: 300,
                cache_write_per_1k_microusd: 3750,
            }),
            status: ProviderStatus::Available,
        }
    }

    // Implementation uses Claude Agent SDK directly via API
}
```

### Local Provider

For same-machine Claude proxy (no tunnel needed):

```rust
/// Local Claude provider (same machine, no tunnel)
pub struct LocalProvider {
    proxy_port: u16,
    sessions: Arc<RwLock<HashMap<String, LocalSession>>>,
    executor: Arc<ExecutorManager>,
}

impl ClaudeProvider for LocalProvider {
    fn id(&self) -> &str { "local" }

    fn info(&self) -> ClaudeProviderInfo {
        ClaudeProviderInfo {
            id: "local".to_string(),
            name: "Local Claude Proxy".to_string(),
            models: self.query_local_models(),
            capabilities: ClaudeCapabilities {
                streaming: true,
                resume: true,
                fork: true,
                tools: true,
                vision: true,
            },
            pricing: None,  // Depends on underlying provider
            status: self.check_local_health(),
        }
    }

    // Implementation connects to local proxy via localhost
}
```

---

## ClaudeFs FileService

The `/claude` FileService implementation:

```rust
/// Claude capability as a filesystem
pub struct ClaudeFs {
    agent_id: String,
    router: Arc<RwLock<ClaudeRouter>>,
    policy: Arc<RwLock<ClaudePolicy>>,
    usage: Arc<RwLock<ClaudeUsageState>>,
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    tunnels: Arc<RwLock<Vec<TunnelEndpoint>>>,
    journal: Arc<dyn IdempotencyJournal>,
}

impl FileService for ClaudeFs {
    fn name(&self) -> &str { "claude" }

    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        match parts.as_slice() {
            // /claude/new — create Claude session, returns session_id
            ["new"] if flags.write => {
                Ok(Box::new(ClaudeNewHandle::new(
                    self.agent_id.clone(),
                    self.router.clone(),
                    self.policy.clone(),
                    self.usage.clone(),
                    self.sessions.clone(),
                    self.journal.clone(),
                )))
            }

            // /claude/policy — read-only to agents
            ["policy"] => {
                let policy = self.policy.read();
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&*policy)?)))
            }

            // /claude/usage — current usage stats
            ["usage"] => {
                let usage = self.usage.read();
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&*usage)?)))
            }

            // /claude/auth/tunnels — configured tunnel endpoints
            ["auth", "tunnels"] => {
                let tunnels = self.tunnels.read();
                // Never expose secrets, only IDs and URLs
                let safe: Vec<_> = tunnels.iter().map(|t| TunnelSummary {
                    id: t.id.clone(),
                    url: t.url.clone(),
                    auth_type: t.auth.type_name(),
                }).collect();
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(&safe)?)))
            }

            // /claude/sessions/<session_id>/status
            ["sessions", session_id, "status"] => {
                let sessions = self.sessions.read();
                let state = sessions.get(*session_id).ok_or(Error::NotFound)?;
                Ok(Box::new(StringHandle::new(serde_json::to_string_pretty(state)?)))
            }

            // /claude/sessions/<session_id>/prompt — write to send prompt
            ["sessions", session_id, "prompt"] if flags.write => {
                Ok(Box::new(PromptHandle::new(
                    self.router.clone(),
                    session_id.to_string(),
                )))
            }

            // /claude/sessions/<session_id>/response — read latest response
            ["sessions", session_id, "response"] => {
                let router = self.router.read();
                if let Some(provider) = router.get_session_provider(*session_id) {
                    if let Some(SessionState::Idle { .. } | SessionState::Complete(_)) =
                        provider.get_session(*session_id)
                    {
                        // Return latest response
                        // ...
                    }
                }
                Err(Error::NotReady)
            }

            // /claude/sessions/<session_id>/tools/log — tool execution log
            ["sessions", session_id, "tools", "log"] => {
                // Return append-only tool execution log
                // ...
                todo!()
            }

            // /claude/sessions/<session_id>/tools/approve — approve pending tool
            ["sessions", session_id, "tools", "approve"] if flags.write => {
                Ok(Box::new(ToolApprovalHandle::new(
                    self.router.clone(),
                    session_id.to_string(),
                )))
            }

            // /claude/sessions/<session_id>/fork — fork session
            ["sessions", session_id, "fork"] if flags.write => {
                Ok(Box::new(ForkHandle::new(
                    self.router.clone(),
                    self.sessions.clone(),
                    session_id.to_string(),
                )))
            }

            // /claude/sessions/<session_id>/ctl — control session
            ["sessions", session_id, "ctl"] if flags.write => {
                Ok(Box::new(SessionCtlHandle::new(
                    self.router.clone(),
                    session_id.to_string(),
                )))
            }

            _ => Err(Error::NotFound),
        }
    }

    fn watch(&self, path: &str) -> Result<Option<Box<dyn WatchHandle>>> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();

        // /claude/sessions/<session_id>/output — streaming output
        if let ["sessions", session_id, "output"] = parts.as_slice() {
            return Ok(Some(Box::new(OutputWatchHandle::new(
                self.router.clone(),
                session_id.to_string(),
            ))));
        }

        Ok(None)
    }
}
```

---

## Budget Integration

**REQUIRED: `/claude` MUST be mounted with `AccessLevel::Budgeted`.**

```rust
// Mount with budget policy (amounts in micro-USD)
namespace.mount("/claude", claude_fs, AccessLevel::Budgeted(BudgetPolicy {
    per_tick_usd: 1_000_000,          // $1/tick
    per_day_usd: 100_000_000,         // $100/day
    approval_threshold_usd: 10_000_000, // $10 requires approval
    approvers: vec![owner_pubkey],
}));
```

### Reserve/Reconcile Model

1. **At session creation:** Reserve `max_cost_usd` from budget
2. **During session:** Tokens consumed, cost accrues
3. **At session end:** Reconcile `actual_cost_usd`, refund difference

### Cost Calculation

```rust
/// Calculate session cost from usage
fn calculate_cost(usage: &ClaudeUsage, pricing: &ClaudePricing) -> u64 {
    let input_cost = (usage.input_tokens * pricing.input_per_1k_microusd) / 1000;
    let output_cost = (usage.output_tokens * pricing.output_per_1k_microusd) / 1000;
    let cache_read_cost = (usage.cache_read_tokens * pricing.cache_read_per_1k_microusd) / 1000;
    let cache_write_cost = (usage.cache_write_tokens * pricing.cache_write_per_1k_microusd) / 1000;

    input_cost + output_cost + cache_read_cost + cache_write_cost
}
```

---

## Implementation per Backend

| Feature | Tunnel | Cloud | Local |
|---------|--------|-------|-------|
| **Provider** | `TunnelProvider` | `CloudProvider` | `LocalProvider` |
| **Auth** | Nostr signature | API key | None/PSK |
| **Credentials Location** | User's machine | Secret store | Local |
| **Session Resume** | Yes | Yes | Yes |
| **Session Fork** | Yes | Yes | Yes |
| **Streaming** | WebSocket | SSE | WebSocket |
| **Tool Support** | Full | Full | Full |
| **Use Case** | Remote agents | Direct API | Same machine |

---

## Usage Examples

### Create Claude Session via Tunnel

```rust
// Agent code - create Claude session through tunnel
let request = serde_json::json!({
    "model": "claude-sonnet-4-20250514",
    "system_prompt": "You are a helpful coding assistant.",
    "initial_prompt": "Review this code and suggest improvements.",
    "tools": [
        {"name": "Read"},
        {"name": "Glob"},
        {"name": "Grep"}
    ],
    "tunnel_endpoint": "local-proxy",
    "max_cost_usd": 5000000,  // $5 budget (micro-USD)
    "autonomy": "supervised",
    "idempotency_key": "review_session_123"
});

// Create session (returns immediately)
let session_info: serde_json::Value =
    serde_json::from_slice(&env.call("/claude/new", &serde_json::to_vec(&request)?)?)?;
let session_id = session_info["session_id"].as_str().unwrap();

// Wait for ready
loop {
    let status_bytes = env.read(&format!("/claude/sessions/{}/status", session_id))?;
    let status: serde_json::Value = serde_json::from_slice(&status_bytes)?;

    if status["status"] == "ready" { break; }
    if status["status"] == "failed" {
        return Err(format!("Session failed: {}", status["error"]));
    }
    std::thread::sleep(Duration::from_millis(100));
}

// Send additional prompts
env.write(
    &format!("/claude/sessions/{}/prompt", session_id),
    b"Now fix the bugs you found"
)?;
```

### Watch Streaming Output

```rust
// Watch for streaming output
if let Some(mut watch) = env.watch(&format!("/claude/sessions/{}/output", session_id))? {
    while let Some(event) = watch.next(Some(Duration::from_secs(60)))? {
        if let WatchEvent::Data(data) = event {
            let chunk: ClaudeChunk = serde_json::from_slice(&data)?;

            match chunk.chunk_type {
                ChunkType::Text => {
                    print!("{}", chunk.delta.unwrap_or_default());
                }
                ChunkType::ToolStart => {
                    println!("\n[Tool: {}]", chunk.tool.as_ref().unwrap().name);
                }
                ChunkType::ToolDone => {
                    println!("[Tool complete]");
                }
                ChunkType::Done => {
                    break;
                }
                ChunkType::Error => {
                    eprintln!("Error: {:?}", chunk);
                }
                _ => {}
            }
        }
    }
}
```

### Handle Tool Approval

```rust
// For supervised autonomy, check for pending approvals
let status_bytes = env.read(&format!("/claude/sessions/{}/status", session_id))?;
let status: ClaudeSessionStatus = serde_json::from_slice(&status_bytes)?;

if let ClaudeSessionStatus::PendingApproval { tool, params } = status {
    println!("Claude wants to use tool: {}", tool);
    println!("With params: {}", serde_json::to_string_pretty(&params)?);

    // Decide whether to approve
    let approve = should_approve(&tool, &params);

    // Send approval
    let response = serde_json::json!({ "approved": approve });
    env.write(
        &format!("/claude/sessions/{}/tools/approve", session_id),
        &serde_json::to_vec(&response)?
    )?;
}
```

### Fork a Session

```rust
// Fork session to try a different approach
let fork_result: serde_json::Value = serde_json::from_slice(&env.call(
    &format!("/claude/sessions/{}/fork", session_id),
    b"{}",
)?)?;
let forked_session_id = fork_result["session_id"].as_str().unwrap();

// Send different prompt to forked session
env.write(
    &format!("/claude/sessions/{}/prompt", forked_session_id),
    b"Try a completely different approach: use functional programming"
)?;
```

### Configure Tunnel Endpoints

```rust
// Read configured tunnels
let tunnels_bytes = env.read("/claude/auth/tunnels")?;
let tunnels: Vec<TunnelSummary> = serde_json::from_slice(&tunnels_bytes)?;

for tunnel in tunnels {
    println!("Tunnel: {} at {}", tunnel.id, tunnel.url);
}
```

### Check Session Cost

```rust
// Check session usage and cost
let usage_bytes = env.read(&format!("/claude/sessions/{}/usage", session_id))?;
let usage: serde_json::Value = serde_json::from_slice(&usage_bytes)?;

println!("Tokens - Input: {}, Output: {}",
    usage["input_tokens"], usage["output_tokens"]);
println!("Cost: ${:.4}", usage["cost_usd"].as_u64().unwrap_or(0) as f64 / 1_000_000.0);
```

---

## Integration with Compute and Containers

The `/claude` mount complements `/compute` and `/containers`:

| Mount | Purpose |
|-------|---------|
| `/compute` | AI inference (LLM calls, embeddings, simple queries) |
| `/containers` | Code execution (build, test, run sandboxed code) |
| `/claude` | Autonomous Claude agents (complex multi-step tasks with tool use) |

**When to use `/claude` vs `/compute`:**
- `/compute`: Simple LLM calls, embeddings, quick questions
- `/claude`: Multi-turn conversations, tool use, autonomous work sessions

**Integration pattern:**

```rust
// Agent spawns Claude to handle complex task
let claude_session = create_claude_session(&env, ClaudeRequest {
    model: "claude-sonnet-4-20250514".into(),
    system_prompt: Some("You are a code reviewer. Use tools to analyze the codebase.".into()),
    tools: vec![
        tool("Read"),
        tool("Glob"),
        tool("Grep"),
    ],
    tunnel_endpoint: Some("local-proxy".into()),
    ..Default::default()
})?;

// Claude uses tools via the runtime
// Tool calls appear in /claude/sessions/<id>/tools/log
// Parent agent can monitor and intervene if needed
```

---

## References

- [COMPUTE.md](COMPUTE.md) — AI compute abstraction (`/compute` mount)
- [CONTAINERS.md](CONTAINERS.md) — Container abstraction (`/containers` mount)
- [FILESYSTEM.md](FILESYSTEM.md) — FileService trait, watch semantics
- [Claude Agent SDK docs](/docs/claude/) — SDK overview, hosting, security
