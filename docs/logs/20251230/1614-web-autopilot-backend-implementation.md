# Web Autopilot Backend Implementation

**Date:** 2024-12-30 16:14
**Status:** In Progress

## Summary

Implemented the backend infrastructure for connecting the web UI to autopilot functionality via a tunnel architecture. Users will run `openagents connect` on their machine to tunnel their local autopilot to the web UI.

## Problem Statement

The web UI app shell is complete (sidebars, docks, status bar), but the center pane is empty. The desktop autopilot has local access to the user's computer; the web version does not. We needed an architecture to bridge this gap.

## Chosen Architecture: Tunnel-First

```
┌─────────────┐                  ┌─────────────────┐                  ┌─────────────────┐
│   Browser   │◄───WebSocket────►│  CF Worker      │◄───WebSocket────►│  Tunnel Client  │
│  (WASM)     │                  │  (Relay DO)     │                  │  (CLI)          │
└─────────────┘                  └─────────────────┘                  └─────────────────┘
```

**Why this approach:**
- Full desktop access for autopilot (file system, git, etc.)
- Uses existing codebase (autopilot, claude-agent-sdk)
- User controls compute and API keys
- No separate deployment needed

## Implementation Details

### 1. Claude OAuth Integration

**Files Created:**
- `crates/web/worker/src/services/claude.rs`
- `crates/web/worker/src/routes/claude_auth.rs`

**OAuth Endpoints Discovered:**
- Authorization: `https://claude.ai/oauth/authorize`
- Token: `https://console.anthropic.com/v1/oauth/token`
- API Key Creation: `https://api.anthropic.com/api/oauth/claude_cli/create_api_key`

Source: [sst/opencode-anthropic-auth](https://github.com/sst/opencode-anthropic-auth)

**Features:**
- PKCE (Proof Key for Code Exchange) with S256 challenge
- Token storage linked to user session in KV
- Support for users not yet logged in (pending tokens)

**Routes Added:**
| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/auth/claude/start` | Start OAuth flow with PKCE |
| GET | `/api/auth/claude/callback` | Handle OAuth callback |
| GET | `/api/auth/claude/status` | Check if Claude connected |
| POST | `/api/auth/claude/disconnect` | Remove Claude tokens |
| POST | `/api/auth/claude/link` | Link pending tokens after GitHub login |

### 2. Relay Protocol Crate

**Location:** `crates/relay/`

Shared message types for browser ↔ worker ↔ tunnel communication.

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RelayMessage {
    // Connection Management
    TunnelConnected { version: String, capabilities: Vec<String> },
    TunnelDisconnected { reason: Option<String> },
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },

    // Browser → Tunnel
    StartTask { task_id: String, repo: String, task: String, use_own_key: bool },
    CancelTask { task_id: String },
    SendInput { task_id: String, text: String },

    // Tunnel → Browser (Autopilot Streaming)
    AutopilotChunk { task_id: String, chunk: String },
    ToolStart { task_id: String, tool_name: String, tool_id: String, params: Value },
    ToolDone { task_id: String, tool_id: String, output: String, is_error: bool },
    ToolProgress { task_id: String, tool_id: String, elapsed_secs: f32, message: Option<String> },
    Usage { task_id: String, input_tokens: u64, output_tokens: u64, ... },
    TaskDone { task_id: String, summary: String },
    TaskError { task_id: String, error: String, recoverable: bool },

    // Error Handling
    Error { code: String, message: String },
}
```

### 3. Durable Object WebSocket Relay

**File:** `crates/web/worker/src/relay.rs`

Uses Cloudflare Durable Objects with WebSocket Hibernation API for cost-effective long-lived connections.

**Key Features:**
- Tags for connection type (`browser` vs `tunnel`)
- Message relaying between browser and tunnel clients
- Automatic tunnel disconnect notification to browsers
- Session management in KV

```rust
#[durable_object]
pub struct TunnelRelay {
    state: State,
    env: Env,
}

impl DurableObject for TunnelRelay {
    async fn fetch(&self, req: Request) -> Result<Response> { ... }
    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> { ... }
    async fn websocket_close(&self, ws: WebSocket, code: usize, reason: String, was_clean: bool) -> Result<()> { ... }
    async fn websocket_error(&self, ws: WebSocket, error: Error) -> Result<()> { ... }
}
```

### 4. Tunnel Routes

**File:** `crates/web/worker/src/routes/tunnel.rs`

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/tunnel/register` | Create session, returns session_id + tunnel_token |
| GET | `/api/tunnel/status/:session_id` | Check if tunnel is connected |
| GET | `/api/tunnel/ws/browser?session_id=...` | Browser WebSocket connection |
| GET | `/api/tunnel/ws/tunnel?session_id=...&token=...` | Tunnel client WebSocket connection |

**Session Flow:**
1. Browser calls `POST /api/tunnel/register` with repo
2. Server returns `session_id`, `tunnel_token`, `tunnel_url`, `browser_url`
3. User runs `openagents connect` with tunnel_token
4. Tunnel client connects to `tunnel_url`
5. Browser connects to `browser_url`
6. Messages relay through Durable Object

### 5. Configuration Updates

**wrangler.toml additions:**

```toml
# Durable Objects for WebSocket relay
[durable_objects]
bindings = [
    { name = "TUNNEL_RELAY", class_name = "TunnelRelay" }
]

[[migrations]]
tag = "v1"
new_classes = ["TunnelRelay"]

[vars]
# CLAUDE_CLIENT_ID = "" # Set when available

# Secrets needed:
# CLAUDE_CLIENT_SECRET
```

## Files Created/Modified

### New Files
| File | Purpose |
|------|---------|
| `crates/relay/Cargo.toml` | Relay protocol crate config |
| `crates/relay/src/lib.rs` | Crate entry point |
| `crates/relay/src/protocol.rs` | Message type definitions |
| `crates/web/worker/src/services/claude.rs` | Claude OAuth service |
| `crates/web/worker/src/routes/claude_auth.rs` | Claude OAuth routes |
| `crates/web/worker/src/routes/tunnel.rs` | Tunnel session routes |
| `crates/web/worker/src/relay.rs` | Durable Object relay |

### Modified Files
| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Added `crates/relay` to members |
| `crates/web/worker/src/lib.rs` | Added Claude + tunnel routes |
| `crates/web/worker/src/routes/mod.rs` | Export claude_auth, tunnel |
| `crates/web/worker/src/services/mod.rs` | Export claude |
| `crates/web/wrangler.toml` | Durable Object bindings + migrations |

## Remaining Work

### 1. Connect CLI (`crates/connect/`)
CLI for users to run on their machine:
```bash
openagents connect --repo owner/repo
```

Will:
- Authenticate with web worker
- Connect WebSocket to relay
- Start local autopilot daemon
- Relay messages bidirectionally

### 2. Web Client Updates
- Add WebSocket connection to center pane
- Render ThreadView with streaming messages
- Show "Connect Your Machine" prompt when tunnel disconnected
- Add Claude OAuth button in UI

### 3. End-to-End Testing
- Test full flow: GitHub login → Claude OAuth → Tunnel connect → Task execution
- Test reconnection handling
- Test error cases

## Technical Notes

### Claude OAuth Discovery
The Claude OAuth flow is not publicly documented. Endpoints were discovered from:
- [opencode-anthropic-auth](https://github.com/sst/opencode-anthropic-auth) npm package
- GitHub issues mentioning OAuth tokens starting with `sk-ant-oat01-`

### WebSocket Hibernation
Using Cloudflare's WebSocket Hibernation API allows connections to stay alive while the Durable Object is idle, significantly reducing costs for long-lived connections.

### PKCE Implementation
Using S256 challenge method:
```rust
let verifier = random_string(64);  // A-Z, a-z, 0-9, -, ., _, ~
let challenge = base64url(sha256(verifier));
```

## References

- [workers-rs Durable Objects](https://docs.rs/worker/latest/worker/durable/)
- [Cloudflare WebSocket Hibernation](https://developers.cloudflare.com/durable-objects/best-practices/websockets/)
- [openauth OAuth2 provider](https://github.com/sst/openauth)
- [opencode-anthropic-auth](https://github.com/sst/opencode-anthropic-auth)
