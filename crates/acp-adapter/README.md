# ACP Adapter

Agent Client Protocol (ACP) adapter for OpenAgents, enabling standardized communication between OpenAgents applications and AI coding agents (Codex Code, Codex, and future agents).

## Overview

The ACP adapter wraps existing agent SDKs (`codex-agent-sdk`, `codex-agent-sdk`) to provide a unified protocol-based interface. It handles:

- **Multi-agent support** - Work with Codex Code, Codex, or any ACP-compatible agent
- **Protocol standardization** - JSON-RPC 2.0 communication following the ACP specification
- **Session management** - Create, track, and manage agent sessions
- **Session replay** - Record and replay sessions from rlog files
- **Real-time streaming** - Live conversion between ACP and rlog formats
- **Permission handling** - Flexible permission delegation to UI or auto-approval

## Architecture

```text
+-------------------+     +------------------+     +----------------------+
|   Desktop GUI     |     |   acp-adapter    |     |  Agent subprocess    |
|   (wry + actix)   |---->|   (this crate)   |---->|  (CC / Codex)        |
+-------------------+     +------------------+     +----------------------+
        |                        |
        | HTMX/WS               |
        v                        v
+-------------------+     +------------------+
|   rlog streamer   |<----|   converters     |
|   (docs/logs/)    |     |   (ACP <-> rlog) |
+-------------------+     +------------------+
```

**Key components:**

- `AcpAgentConnection` - Manages stdio subprocess connection with an agent
- `OpenAgentsClient` - Implements `acp::Client` trait for permission/file operations
- `RlogReplay` - Replays rlog files as ACP notifications
- `RlogStreamer` - Streams ACP to rlog in real-time
- Agent wrappers - Configuration and connection helpers for specific agents

## Quick Start

### Basic Usage

```rust
use acp_adapter::agents::codex::{connect_codex, CodexAgentConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to Codex Code
    let cwd = PathBuf::from("/path/to/workspace");
    let config = CodexAgentConfig::new()
        .model("codex-sonnet-4-5")
        .max_turns(100)
        .permission_mode("default");

    let connection = connect_codex(config, &cwd).await?;

    // Create a session
    let session = connection.new_session(cwd.clone()).await?;

    // Send a prompt
    connection.prompt(&session.session_id, "Fix the bug in src/main.rs").await?;

    Ok(())
}
```

### Using Codex

```rust
use acp_adapter::agents::codex::{connect_codex, CodexAgentConfig};

let config = CodexAgentConfig::new()
    .model("gpt-4o")
    .max_turns(50);

let connection = connect_codex(config, &cwd).await?;
let session = connection.new_session(cwd.clone()).await?;
connection.prompt(&session.session_id, "Add authentication").await?;
```

### Custom Agent Command

```rust
use acp_adapter::{AgentCommand, AcpAgentConnection};

let command = AgentCommand::new("/path/to/custom-agent")
    .arg("--mode").arg("code")
    .env("API_KEY", "sk-...");

let connection = AcpAgentConnection::stdio("Custom Agent", command, &cwd).await?;
```

## Session Replay

Replay recorded rlog files as ACP notifications for debugging, testing, or UI development:

```rust
use acp_adapter::{RlogReplay, ReplayConfig};
use agent_client_protocol_schema as acp;
use tokio::sync::mpsc;

// Create notification channel
let (tx, mut rx) = mpsc::channel(100);

// Replay at 2x speed
let config = ReplayConfig::realtime().speed(2.0);
let replayer = RlogReplay::new(acp::SessionId::new("session-123"))
    .with_config(config);

// Start replay
tokio::spawn(async move {
    replayer.replay_file("docs/logs/20251223/session.rlog", tx).await.unwrap();
});

// Receive notifications
while let Some(notification) = rx.recv().await {
    match notification.update {
        acp::SessionUpdate::AgentMessageChunk { chunk } => {
            println!("Agent: {:?}", chunk);
        }
        acp::SessionUpdate::ToolCall { tool_call } => {
            println!("Tool: {} ({})", tool_call.name, tool_call.id);
        }
        _ => {}
    }
}
```

## Real-time Streaming

Stream ACP notifications to rlog format for recording:

```rust
use acp_adapter::{RlogStreamer, StreamConfig};
use std::path::PathBuf;

let config = StreamConfig {
    output_dir: PathBuf::from("docs/logs"),
    session_prefix: "20251223".to_string(),
    buffer_size: 1000,
    flush_interval_ms: 1000,
};

let streamer = RlogStreamer::new(config).await?;

// Start streaming
streamer.start(&session_id, &initial_prompt).await?;

// Handle ACP notifications
let notification = acp::SessionNotification::new(
    session_id.clone(),
    acp::SessionUpdate::AgentMessageChunk { chunk }
);

streamer.write_notification(&notification).await?;

// Finalize when done
streamer.finalize().await?;
```

## Permission Handling

The adapter supports multiple permission handling strategies:

### Auto-approve All

```rust
use acp_adapter::{OpenAgentsClient, AllowAllPermissions};

let permission_handler = AllowAllPermissions;
let client = OpenAgentsClient::new(permission_handler, cwd);
```

### Auto-deny All

```rust
use acp_adapter::DenyAllPermissions;

let permission_handler = DenyAllPermissions;
let client = OpenAgentsClient::new(permission_handler, cwd);
```

### UI Permission Delegation

```rust
use acp_adapter::{UiPermissionHandler, PermissionRequestManager};
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel(100);
let manager = PermissionRequestManager::new(rx);
let permission_handler = UiPermissionHandler::new(tx);
let client = OpenAgentsClient::new(permission_handler, cwd);

// In your UI handler
tokio::spawn(async move {
    while let Some(request) = manager.next_request().await {
        // Show UI to user
        let approved = show_permission_ui(&request);

        // Respond
        manager.respond(request.id, approved, None).await;
    }
});
```

### Custom Permission Handler

Implement the `PermissionHandler` trait:

```rust
use acp_adapter::PermissionHandler;
use agent_client_protocol_schema as acp;
use async_trait::async_trait;

struct CustomPermissionHandler;

#[async_trait]
impl PermissionHandler for CustomPermissionHandler {
    async fn request_permission(
        &self,
        request: &acp::RequestPermissionRequest,
    ) -> acp::RequestPermissionResponse {
        // Your custom logic
        acp::RequestPermissionResponse::new(true)
    }
}
```

## API Reference

### AcpAgentConnection

Main connection type for communicating with an agent subprocess.

```rust
impl AcpAgentConnection {
    /// Create connection via stdio
    pub async fn stdio(
        name: impl Into<String>,
        command: AgentCommand,
        root_dir: &Path
    ) -> Result<Self>;

    /// Get agent capabilities
    pub fn capabilities(&self) -> &acp::AgentCapabilities;

    /// Get negotiated protocol version
    pub fn protocol_version(&self) -> &acp::ProtocolVersion;

    /// Create a new session
    pub async fn new_session(&self, cwd: PathBuf) -> Result<AcpAgentSession>;

    /// Send a prompt to a session
    pub async fn prompt(
        &self,
        session_id: &acp::SessionId,
        content: impl Into<String>
    ) -> Result<acp::PromptResponse>;

    /// Cancel ongoing work in a session
    pub async fn cancel(&self, session_id: &acp::SessionId);

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<AcpAgentSession>;

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<String>;

    /// Close a session
    pub async fn close_session(&self, session_id: &str) -> Result<()>;

    /// Check if agent process is still running
    pub fn is_running(&mut self) -> bool;
}
```

### OpenAgentsClient

Implements the `acp::Client` trait for handling agent requests.

```rust
pub struct OpenAgentsClient<P: PermissionHandler> {
    permission_handler: P,
    cwd: PathBuf,
}

impl<P: PermissionHandler> OpenAgentsClient<P> {
    pub fn new(permission_handler: P, cwd: PathBuf) -> Self;
}

#[async_trait]
impl<P: PermissionHandler> acp::Client for OpenAgentsClient<P> {
    async fn request_permission(
        &self,
        req: acp::RequestPermissionRequest
    ) -> Result<acp::RequestPermissionResponse>;

    async fn read_text_file(
        &self,
        req: acp::ReadTextFileRequest
    ) -> Result<acp::ReadTextFileResponse>;

    async fn write_text_file(
        &self,
        req: acp::WriteTextFileRequest
    ) -> Result<acp::WriteTextFileResponse>;

    async fn session_notification(
        &self,
        notification: acp::SessionNotification
    ) -> Result<()>;

    // ... other Client methods
}
```

### Agent Configuration

#### Codex Code

```rust
pub struct CodexAgentConfig {
    pub executable_path: Option<PathBuf>,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub permission_mode: Option<String>,
    pub system_prompt: Option<String>,
    pub max_budget_usd: Option<f64>,
}

impl CodexAgentConfig {
    pub fn new() -> Self;
    pub fn executable_path(self, path: impl Into<PathBuf>) -> Self;
    pub fn model(self, model: impl Into<String>) -> Self;
    pub fn max_turns(self, max_turns: u32) -> Self;
    pub fn permission_mode(self, mode: impl Into<String>) -> Self;
    pub fn system_prompt(self, prompt: impl Into<String>) -> Self;
    pub fn max_budget_usd(self, budget: f64) -> Self;
}

pub async fn connect_codex(
    config: CodexAgentConfig,
    root_dir: &Path
) -> Result<AcpAgentConnection>;
```

#### Codex

```rust
pub struct CodexAgentConfig {
    pub executable_path: Option<PathBuf>,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub temperature: Option<f32>,
}

impl CodexAgentConfig {
    pub fn new() -> Self;
    pub fn executable_path(self, path: impl Into<PathBuf>) -> Self;
    pub fn model(self, model: impl Into<String>) -> Self;
    pub fn max_turns(self, max_turns: u32) -> Self;
    pub fn temperature(self, temp: f32) -> Self;
}

pub async fn connect_codex(
    config: CodexAgentConfig,
    root_dir: &Path
) -> Result<AcpAgentConnection>;
```

## Converters

The adapter includes bidirectional converters for various formats:

### SDK <-> ACP

- `converters::sdk_to_acp` - Convert Codex SDK messages to ACP notifications
- `converters::acp_to_sdk` - Convert ACP requests to SDK types

### ACP <-> rlog

- `converters::rlog::notification_to_rlog_line` - Convert ACP notification to rlog format
- `converters::rlog::rlog_line_to_notification` - Parse rlog line to ACP notification

### Codex <-> ACP

- `converters::codex::thread_event_to_notification` - Convert Codex ThreadEvent to ACP
- `converters::codex::todo_list_to_plan` - Map Codex TodoList to ACP Plan

## REST API Integration

When integrating with a web UI (e.g., Actix server), you can expose ACP sessions via REST:

```rust
use actix_web::{get, post, delete, web, HttpResponse};
use acp_adapter::{AcpAgentConnection, agents::codex::connect_codex};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct CreateSessionRequest {
    agent: String,  // "codex" or "codex"
    model: Option<String>,
    cwd: Option<PathBuf>,
}

#[post("/api/acp/sessions")]
async fn create_session(
    req: web::Json<CreateSessionRequest>,
    state: web::Data<AppState>
) -> HttpResponse {
    // Create connection
    let connection = match req.agent.as_str() {
        "codex" => connect_codex(config, &cwd).await,
        "codex" => connect_codex(config, &cwd).await,
        _ => return HttpResponse::BadRequest().body("Unknown agent"),
    };

    // Create session
    let session = connection.new_session(cwd).await.unwrap();

    HttpResponse::Ok().json(session)
}

#[derive(Deserialize)]
struct PromptRequest {
    content: String,
}

#[post("/api/acp/sessions/{id}/prompt")]
async fn send_prompt(
    id: web::Path<String>,
    req: web::Json<PromptRequest>,
    state: web::Data<AppState>
) -> HttpResponse {
    let session_id = acp::SessionId::new(id.into_inner());

    // Send prompt
    state.connection.prompt(&session_id, &req.content).await.unwrap();

    HttpResponse::Ok().finish()
}

#[delete("/api/acp/sessions/{id}")]
async fn close_session(
    id: web::Path<String>,
    state: web::Data<AppState>
) -> HttpResponse {
    state.connection.close_session(&id).await.unwrap();
    HttpResponse::Ok().finish()
}
```

See `docs/acp-adapter.md` for complete REST API documentation.

## Troubleshooting

### Agent executable not found

**Problem**: `AgentNotFound` error when connecting

**Solution**:
```bash
# For Codex Code
npm install -g @openai-ai/codex-code

# For Codex
npm install -g codex-agent

# Or specify explicit path
let config = CodexAgentConfig::new()
    .executable_path("/custom/path/to/codex");
```

### Connection timeout during initialization

**Problem**: Agent subprocess doesn't respond to `initialize` request

**Causes**:
- Agent binary incompatible with ACP protocol
- Missing required environment variables
- Incorrect working directory

**Debug**:
```rust
// Enable tracing
use tracing_subscriber;
tracing_subscriber::fmt::init();

// Check stderr output from agent
let mut cmd = tokio::process::Command::new("codex");
cmd.stderr(std::process::Stdio::piped());
```

### Permission requests not appearing in UI

**Problem**: Permission requests are auto-denied or ignored

**Solution**: Ensure `UiPermissionHandler` is properly wired:

```rust
// Create manager BEFORE client
let (tx, rx) = mpsc::channel(100);
let manager = PermissionRequestManager::new(rx);
let handler = UiPermissionHandler::new(tx);
let client = OpenAgentsClient::new(handler, cwd);

// Ensure manager.next_request() loop is running
tokio::spawn(async move {
    while let Some(req) = manager.next_request().await {
        // Must call manager.respond()!
    }
});
```

### Session replay produces garbled output

**Problem**: Replayed rlog contains invalid JSON or protocol errors

**Causes**:
- rlog file was manually edited
- Incomplete session (crashed before finalization)
- Version mismatch between recorder and replayer

**Solution**:
```rust
// Use lenient parsing for partial logs
let config = ReplayConfig::instant().skip_empty(true);
let replayer = RlogReplay::new(session_id).with_config(config);

// Handle errors gracefully
match replayer.replay_file(path, tx).await {
    Ok(stats) => println!("Replayed {} lines", stats.lines_processed),
    Err(e) => eprintln!("Partial replay failed: {}", e),
}
```

### Memory leak with long-running sessions

**Problem**: Memory usage grows unbounded during long agent sessions

**Cause**: Session state accumulates all notifications without cleanup

**Solution**:
```rust
// Periodically checkpoint and clear old entries
if session.entry_count() > 1000 {
    session.checkpoint().await?;
    session.clear_before(checkpoint_id).await?;
}

// Or use bounded notification channels
let (tx, rx) = mpsc::channel(100);  // Backpressure after 100 pending
```

### Codex tool calls not appearing

**Problem**: Codex CommandExecution, FileChange, etc. don't show in UI

**Cause**: Codex uses different event types than Codex SDK

**Solution**: Ensure Codex converter is being used:

```rust
use acp_adapter::converters::codex::thread_event_to_notification;

// Convert Codex events
let notification = thread_event_to_notification(&thread_event, &session_id)?;
```

## Testing

Run the full test suite:

```bash
cd crates/acp-adapter
cargo test
```

Integration tests require agent binaries to be installed:

```bash
# Install test dependencies
npm install -g @openai-ai/codex-code codex-agent

# Run integration tests
cargo test --test integration_tests
```

## Dependencies

**External**:
- [`agent-client-protocol-schema`](https://crates.io/crates/agent-client-protocol-schema) - ACP protocol types
- `tokio` - Async runtime
- `serde` - Serialization
- `tracing` - Logging

**Internal**:
- `recorder` - rlog parsing and writing
- `autopilot` - Trajectory collection (optional)

## Related Documentation

- [ACP Protocol Specification](https://agentclientprotocol.com/protocol/overview)
- [Full REST API Documentation](../../docs/acp-adapter.md)
- [Zed ACP Component Mapping](../../docs/zed-acp-component-mapping.md)
- [OpenAgents Architecture](../../docs/architecture.md)

## License

Same as OpenAgents project.
