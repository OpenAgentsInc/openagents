# ACP Adapter - Complete Documentation

This document provides comprehensive documentation for the ACP (Agent Client Protocol) adapter in OpenAgents, including architecture, API reference, usage examples, and troubleshooting.

## Table of Contents

- [Architecture](#architecture)
- [Core Concepts](#core-concepts)
- [API Reference](#api-reference)
  - [Rust API](#rust-api)
  - [REST API](#rest-api)
- [Usage Examples](#usage-examples)
- [Session Replay](#session-replay)
- [Real-time Streaming](#real-time-streaming)
- [Converters](#converters)
- [Permission System](#permission-system)
- [Integration Guide](#integration-guide)
- [Troubleshooting](#troubleshooting)

## Architecture

The ACP adapter sits between OpenAgents applications (desktop GUI, CLI) and AI coding agents (Codex Code, Codex), providing a standardized communication layer.

### Component Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                       OpenAgents Application                       │
│                    (Desktop GUI / CLI / Web UI)                    │
└────────────────────────┬───────────────────────────────────────────┘
                         │
                         │ Rust API or REST API
                         ▼
┌────────────────────────────────────────────────────────────────────┐
│                         ACP Adapter Layer                          │
│ ┌────────────────┐  ┌──────────────┐  ┌─────────────────────────┐ │
│ │ AcpAgent       │  │ OpenAgents   │  │ Permission              │ │
│ │ Connection     │  │ Client       │  │ Management              │ │
│ │                │  │              │  │                         │ │
│ │ - Session mgmt │  │ - File ops   │  │ - UI delegation         │ │
│ │ - Protocol     │  │ - Terminal   │  │ - Auto-approve          │ │
│ │   negotiation  │  │ - Notif hook │  │ - Custom handlers       │ │
│ └────────────────┘  └──────────────┘  └─────────────────────────┘ │
│                                                                    │
│ ┌────────────────────────────────────────────────────────────────┐ │
│ │                       Converters                               │ │
│ │  SDK ↔ ACP  │  ACP ↔ rlog  │  Codex ↔ ACP                     │ │
│ └────────────────────────────────────────────────────────────────┘ │
│                                                                    │
│ ┌─────────────────┐  ┌─────────────────┐  ┌───────────────────┐  │
│ │ RlogReplay      │  │ RlogStreamer    │  │ Agent Wrappers    │  │
│ │ (Playback)      │  │ (Recording)     │  │ (Codex, Codex)   │  │
│ └─────────────────┘  └─────────────────┘  └───────────────────┘  │
└────────────────────────┬───────────────────────────────────────────┘
                         │
                         │ JSON-RPC 2.0 over stdio
                         ▼
┌────────────────────────────────────────────────────────────────────┐
│                    Agent Subprocess                                │
│                  (Codex Code / Codex)                             │
│                                                                    │
│  Implements ACP Server:                                            │
│  - initialize → AgentCapabilities                                  │
│  - session/new → SessionId                                         │
│  - session/prompt → Starts agent execution                         │
│  - SessionNotification ← Agent sends updates                       │
└────────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Protocol-first**: All communication follows ACP JSON-RPC 2.0 specification
2. **Transport agnostic**: Currently stdio, but extensible to WebSocket, HTTP, etc.
3. **Recording by default**: All sessions can be recorded to rlog for replay
4. **Multi-agent**: Same API works with Codex Code, Codex, or custom agents
5. **Permission flexibility**: From auto-approve to full UI delegation

## Core Concepts

### ACP Protocol Lifecycle

Every agent interaction follows this sequence:

```
1. Spawn subprocess → 2. Initialize → 3. New session → 4. Prompt → 5. Updates → 6. Close
```

**1. Spawn subprocess**
```rust
let child = tokio::process::Command::new("codex")
    .args(["--output-format", "stream-json"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;
```

**2. Initialize** (negotiate capabilities)
```json
→ {"method": "initialize", "params": {
    "protocolVersion": "1.0",
    "clientCapabilities": {"fs": {"readTextFile": true, "writeTextFile": true}}
  }}
← {"result": {
    "protocolVersion": "1.0",
    "agentCapabilities": {"tools": ["bash", "read", "write"]}
  }}
```

**3. New session**
```json
→ {"method": "session/new", "params": {"cwd": "/path/to/workspace"}}
← {"result": {"sessionId": "session-abc123"}}
```

**4. Prompt**
```json
→ {"method": "session/prompt", "params": {
    "sessionId": "session-abc123",
    "content": [{"type": "text", "text": "Fix the bug"}]
  }}
← {"result": {"status": "running"}}
```

**5. Updates** (stream of notifications)
```json
← {"method": "session/notification", "params": {
    "sessionId": "session-abc123",
    "update": {
      "type": "agentMessageChunk",
      "chunk": {"type": "message", "block": {"markdown": "Let me examine..."}}
    }
  }}
```

**6. Close**
```rust
connection.close_session("session-abc123").await?;
```

### Session State Model

```rust
pub struct AcpAgentSession {
    pub session_id: SessionId,      // Unique identifier
    pub cwd: PathBuf,                // Working directory
    pub created_at: DateTime<Utc>,  // Creation time
    pub state: SessionState,         // running | paused | completed | error
    // Internal: transport, entries, etc.
}
```

Each session accumulates entries as the conversation progresses:

```rust
pub enum Entry {
    UserMessage(String),                    // Prompt from user
    AgentMessage(Vec<AssistantChunk>),      // Response chunks
    ToolCall(ToolCall),                     // Tool execution
}
```

### Permission Flow

When the agent needs permission (file write, shell command, etc.):

```
Agent → RequestPermission → PermissionHandler → UI/Auto → Response → Agent
```

Example flow for file write:

```
1. Agent: "I need to write to src/main.rs"
   → RequestPermissionRequest {
       operation: "write",
       file: "src/main.rs",
       content_preview: "fn main() { ... }"
     }

2. PermissionHandler receives request
   → If UiPermissionHandler: sends to UI channel
   → If AllowAllPermissions: auto-approves
   → If DenyAllPermissions: auto-denies

3. Response sent back to agent
   ← RequestPermissionResponse { approved: true }

4. Agent proceeds with write
```

## API Reference

### Rust API

#### AcpAgentConnection

Main type for managing agent connections.

```rust
pub struct AcpAgentConnection {
    pub agent_name: String,
    // ... internal fields
}

impl AcpAgentConnection {
    /// Create connection via stdio transport
    pub async fn stdio(
        agent_name: impl Into<String>,
        command: AgentCommand,
        root_dir: &Path
    ) -> Result<Self>;

    /// Get agent capabilities from initialization
    pub fn capabilities(&self) -> &acp::AgentCapabilities;

    /// Get negotiated protocol version
    pub fn protocol_version(&self) -> &acp::ProtocolVersion;

    /// Create a new session
    ///
    /// # Arguments
    /// * `cwd` - Working directory for this session
    ///
    /// # Returns
    /// New session with unique ID
    pub async fn new_session(&self, cwd: PathBuf) -> Result<AcpAgentSession>;

    /// Send a prompt to a session
    ///
    /// # Arguments
    /// * `session_id` - Target session ID
    /// * `content` - User prompt text
    ///
    /// # Returns
    /// PromptResponse indicating agent started processing
    pub async fn prompt(
        &self,
        session_id: &acp::SessionId,
        content: impl Into<String>
    ) -> Result<acp::PromptResponse>;

    /// Cancel ongoing work in a session
    ///
    /// Sends cancel notification to agent. Agent may or may not honor it.
    pub async fn cancel(&self, session_id: &acp::SessionId);

    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<AcpAgentSession>;

    /// List all active session IDs
    pub async fn list_sessions(&self) -> Vec<String>;

    /// Close a session and free resources
    pub async fn close_session(&self, session_id: &str) -> Result<()>;

    /// Check if agent process is still running
    pub fn is_running(&mut self) -> bool;
}
```

#### AgentCommand

Configuration for spawning an agent subprocess.

```rust
pub struct AgentCommand {
    pub path: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl AgentCommand {
    /// Create new command with executable path
    pub fn new(path: impl Into<PathBuf>) -> Self;

    /// Add a single argument
    pub fn arg(self, arg: impl Into<String>) -> Self;

    /// Add multiple arguments
    pub fn args<I, S>(self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>;

    /// Add environment variable
    pub fn env(self, key: impl Into<String>, value: impl Into<String>) -> Self;
}
```

Example:
```rust
let cmd = AgentCommand::new("/usr/local/bin/codex")
    .arg("--output-format").arg("stream-json")
    .arg("--model").arg("codex-sonnet-4-5")
    .env("OPENAI_API_KEY", "sk-ant-...");
```

#### OpenAgentsClient

Implements `acp::Client` trait to handle agent requests.

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
    /// Handle permission request from agent
    async fn request_permission(
        &self,
        req: acp::RequestPermissionRequest
    ) -> Result<acp::RequestPermissionResponse>;

    /// Read text file on behalf of agent
    async fn read_text_file(
        &self,
        req: acp::ReadTextFileRequest
    ) -> Result<acp::ReadTextFileResponse>;

    /// Write text file on behalf of agent
    async fn write_text_file(
        &self,
        req: acp::WriteTextFileRequest
    ) -> Result<acp::WriteTextFileResponse>;

    /// Receive session notification (updates from agent)
    async fn session_notification(
        &self,
        notification: acp::SessionNotification
    ) -> Result<()>;

    async fn create_terminal(
        &self,
        req: acp::CreateTerminalRequest
    ) -> Result<acp::CreateTerminalResponse>;

    async fn send_terminal_input(
        &self,
        req: acp::SendTerminalInputRequest
    ) -> Result<()>;

    async fn close_terminal(
        &self,
        req: acp::CloseTerminalRequest
    ) -> Result<()>;
}
```

#### PermissionHandler Trait

Implement this trait for custom permission logic.

```rust
#[async_trait]
pub trait PermissionHandler: Send + Sync {
    /// Handle a permission request
    ///
    /// # Arguments
    /// * `request` - Details about what agent wants to do
    ///
    /// # Returns
    /// Response indicating approval/denial and optional message
    async fn request_permission(
        &self,
        request: &acp::RequestPermissionRequest
    ) -> acp::RequestPermissionResponse;
}
```

**Built-in implementations:**

```rust
// Auto-approve all
pub struct AllowAllPermissions;

// Auto-deny all
pub struct DenyAllPermissions;

// Delegate to UI via channel
pub struct UiPermissionHandler {
    tx: mpsc::Sender<UiPermissionRequest>,
}
```

### REST API

When integrated with an Actix web server, the adapter can expose REST endpoints for session management.

#### Base URL

All endpoints are under `/api/acp`

#### Endpoints

##### List Sessions

```
GET /api/acp/sessions
```

**Response:**
```json
{
  "sessions": [
    {
      "sessionId": "session-abc123",
      "agent": "codex",
      "cwd": "/home/user/project",
      "state": "running",
      "createdAt": "2025-12-23T17:48:27Z"
    }
  ]
}
```

##### Create Session

```
POST /api/acp/sessions
```

**Request:**
```json
{
  "agent": "codex",          // "codex" | "codex"
  "model": "codex-sonnet-4-5",  // optional
  "cwd": "/home/user/project"    // optional, defaults to server cwd
}
```

**Response:**
```json
{
  "sessionId": "session-abc123",
  "agent": "codex",
  "cwd": "/home/user/project",
  "state": "ready",
  "createdAt": "2025-12-23T17:48:27Z"
}
```

**Error codes:**
- `400` - Invalid agent name or configuration
- `500` - Failed to spawn agent subprocess

##### Send Prompt

```
POST /api/acp/sessions/{sessionId}/prompt
```

**Request:**
```json
{
  "content": "Fix the authentication bug in src/auth.rs"
}
```

**Response:**
```json
{
  "status": "running",
  "sessionId": "session-abc123"
}
```

**Error codes:**
- `404` - Session not found
- `500` - Failed to send prompt

##### Cancel Session

```
POST /api/acp/sessions/{sessionId}/cancel
```

**Response:**
```json
{
  "status": "cancelled",
  "sessionId": "session-abc123"
}
```

##### Close Session

```
DELETE /api/acp/sessions/{sessionId}
```

**Response:**
```json
{
  "status": "closed",
  "sessionId": "session-abc123"
}
```

##### Get Session Details

```
GET /api/acp/sessions/{sessionId}
```

**Response:**
```json
{
  "sessionId": "session-abc123",
  "agent": "codex",
  "cwd": "/home/user/project",
  "state": "running",
  "createdAt": "2025-12-23T17:48:27Z",
  "stats": {
    "messageCount": 12,
    "toolCallCount": 8,
    "errorCount": 0
  }
}
```

#### Real-time Updates (Server-Sent Events)

```
GET /api/acp/sessions/{sessionId}/stream
```

**Response:** Server-Sent Events stream

```
event: agentMessageChunk
data: {"chunk":{"type":"message","block":{"markdown":"Let me examine the code..."}}}

event: toolCall
data: {"toolCall":{"id":"tool-1","name":"read","status":"running"}}

event: toolCallUpdate
data: {"toolCallId":"tool-1","status":"completed","content":"File contents..."}
```

## Usage Examples

### Example 1: Simple Codex Code Session

```rust
use acp_adapter::agents::codex::{connect_codex, CodexAgentConfig};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = PathBuf::from("/home/user/myproject");

    // Connect to Codex Code
    let config = CodexAgentConfig::new()
        .model("codex-sonnet-4-5")
        .max_turns(100);

    let connection = connect_codex(config, &cwd).await?;

    // Create session
    let session = connection.new_session(cwd.clone()).await?;
    println!("Session created: {}", session.session_id);

    // Send prompt
    let response = connection.prompt(
        &session.session_id,
        "Add error handling to the API endpoints"
    ).await?;

    println!("Agent started: {:?}", response.status);

    Ok(())
}
```

### Example 2: Multi-Agent with Permission Handling

```rust
use acp_adapter::{AcpAgentConnection, OpenAgentsClient, UiPermissionHandler};
use acp_adapter::agents::{codex::connect_codex, codex::connect_codex};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = PathBuf::from(".");

    // Set up permission handling
    let (perm_tx, mut perm_rx) = mpsc::channel(100);
    let permission_handler = UiPermissionHandler::new(perm_tx);
    let client = OpenAgentsClient::new(permission_handler, cwd.clone());

    // Spawn permission UI handler
    tokio::spawn(async move {
        while let Some(request) = perm_rx.recv().await {
            println!("Permission request: {:?}", request);
            // In real app: show UI dialog
            // For now: auto-approve
            // request.respond(true, None).await;
        }
    });

    // Connect to both agents
    let codex = connect_codex(Default::default(), &cwd).await?;
    let codex = connect_codex(Default::default(), &cwd).await?;

    // Create sessions
    let codex_session = codex.new_session(cwd.clone()).await?;
    let codex_session = codex.new_session(cwd.clone()).await?;

    // Race them on the same task
    let task = "Implement a binary search tree";

    tokio::try_join!(
        codex.prompt(&codex_session.session_id, task),
        codex.prompt(&codex_session.session_id, task)
    )?;

    println!("Both agents working...");

    Ok(())
}
```

### Example 3: Session Recording and Replay

```rust
use acp_adapter::{RlogStreamer, RlogReplay, StreamConfig, ReplayConfig};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = PathBuf::from(".");

    // === RECORDING ===
    let stream_config = StreamConfig {
        output_dir: PathBuf::from("docs/logs"),
        session_prefix: "20251223".to_string(),
        buffer_size: 1000,
        flush_interval_ms: 500,
    };

    let streamer = RlogStreamer::new(stream_config).await?;

    // Start session
    let connection = connect_codex(Default::default(), &cwd).await?;
    let session = connection.new_session(cwd.clone()).await?;

    streamer.start(&session.session_id, "Fix bugs").await?;

    // ... agent runs, notifications streamed to rlog ...

    streamer.finalize().await?;
    let log_path = streamer.log_path();

    // === REPLAY ===
    let (tx, mut rx) = mpsc::channel(100);

    let replay_config = ReplayConfig::realtime().speed(2.0);
    let replayer = RlogReplay::new(session.session_id.clone())
        .with_config(replay_config);

    // Start replay in background
    tokio::spawn(async move {
        replayer.replay_file(&log_path, tx).await.unwrap();
    });

    // Consume replayed notifications
    while let Some(notification) = rx.recv().await {
        println!("Replay: {:?}", notification.update);
    }

    Ok(())
}
```

### Example 4: Custom Agent Integration

```rust
use acp_adapter::{AgentCommand, AcpAgentConnection};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Custom agent that implements ACP protocol
    let command = AgentCommand::new("/usr/local/bin/my-custom-agent")
        .arg("--mode").arg("code")
        .arg("--verbose")
        .env("API_KEY", "secret-key")
        .env("MODEL", "custom-model-v1");

    let connection = AcpAgentConnection::stdio(
        "MyCustomAgent",
        command,
        &PathBuf::from(".")
    ).await?;

    println!("Custom agent capabilities: {:?}", connection.capabilities());

    let session = connection.new_session(PathBuf::from(".")).await?;
    connection.prompt(&session.session_id, "Analyze codebase").await?;

    Ok(())
}
```

## Session Replay

Session replay allows you to:
- **Debug** agent behavior from recorded sessions
- **Test** UI rendering without running live agents
- **Analyze** agent decision-making patterns
- **Demo** features using pre-recorded sessions

### Recording Sessions

Sessions are automatically recorded when using `RlogStreamer`:

```rust
use acp_adapter::{RlogStreamer, StreamConfig};

let config = StreamConfig {
    output_dir: PathBuf::from("docs/logs"),
    session_prefix: chrono::Utc::now().format("%Y%m%d").to_string(),
    buffer_size: 1000,
    flush_interval_ms: 1000,
};

let streamer = RlogStreamer::new(config).await?;

// Start recording
streamer.start(&session_id, "Initial prompt text").await?;

// Agent notifications automatically written to rlog
// ...

// Finalize (writes footer, flushes buffer)
streamer.finalize().await?;
```

**Rlog format:**
```
{"type":"SessionStart","sessionId":"session-abc","prompt":"Fix bugs","timestamp":"2025-12-23T17:48:27Z"}
{"type":"AgentMessageChunk","chunk":{"type":"message","block":{"markdown":"I'll help..."}}}
{"type":"ToolCall","toolCall":{"id":"t1","name":"read","path":"src/main.rs"}}
{"type":"SessionEnd","status":"completed","timestamp":"2025-12-23T17:49:15Z"}
```

### Replaying Sessions

```rust
use acp_adapter::{RlogReplay, ReplayConfig};
use tokio::sync::mpsc;

// Create notification channel
let (tx, mut rx) = mpsc::channel(100);

// Configure replay
let config = ReplayConfig {
    delay_ms: 50,              // 50ms between notifications
    use_timestamps: true,      // Use actual timestamps from log
    speed_multiplier: 1.0,     // Real-time
    skip_empty: true,          // Skip blank lines
};

let replayer = RlogReplay::new(session_id).with_config(config);

// Start replay
let stats = replayer.replay_file("docs/logs/20251223/session.rlog", tx).await?;

println!("Replayed {} lines, {} errors", stats.lines_processed, stats.errors);

// Consume notifications
while let Some(notification) = rx.recv().await {
    handle_notification(notification);
}
```

**Replay modes:**

```rust
// Instant (no delays)
let config = ReplayConfig::instant();

// Real-time with original timestamps
let config = ReplayConfig::realtime();

// 10x speed
let config = ReplayConfig::realtime().speed(10.0);

// Custom delay
let config = ReplayConfig::default().delay(100);
```

## Real-time Streaming

The `RlogStreamer` converts ACP notifications to rlog format in real-time, enabling:
- Session recording for later replay
- Live session monitoring in separate processes
- Trajectory data collection

### Basic Streaming

```rust
use acp_adapter::{RlogStreamer, StreamConfig};

let config = StreamConfig {
    output_dir: PathBuf::from("docs/logs"),
    session_prefix: "20251223".to_string(),
    buffer_size: 1000,
    flush_interval_ms: 1000,
};

let mut streamer = RlogStreamer::new(config).await?;

// Start session
streamer.start(&session_id, "Fix authentication bug").await?;

// Write notifications as they arrive
while let Some(notification) = notification_stream.recv().await {
    streamer.write_notification(&notification).await?;
}

// Finalize
streamer.finalize().await?;

// Get path to written log
let log_path = streamer.log_path();
println!("Session recorded to: {}", log_path.display());
```

### Streaming with Header Metadata

```rust
use acp_adapter::streaming::RlogHeaderInfo;

let header = RlogHeaderInfo {
    session_id: session_id.to_string(),
    agent: "codex".to_string(),
    model: Some("codex-sonnet-4-5".to_string()),
    cwd: "/home/user/project".to_string(),
    initial_prompt: "Implement feature X".to_string(),
    started_at: chrono::Utc::now(),
};

streamer.start_with_header(&header).await?;
```

### Tailing Logs in Real-time

Use the rlog path for live tailing:

```bash
# Get the log path
tail -f docs/logs/20251223/174827-session-abc123.rlog

# Or with jq for pretty printing
tail -f docs/logs/20251223/*.rlog | jq .
```

## Converters

The adapter includes bidirectional converters for various formats.

### SDK to ACP

Convert Codex SDK messages to ACP notifications:

```rust
use acp_adapter::converters::sdk_to_acp::message_to_notification;
use codex_agent_sdk::Message;

let sdk_message = Message::AgentMessage { content: "Hello" };
let notification = message_to_notification(&sdk_message, &session_id)?;
```

### ACP to rlog

Convert ACP notifications to rlog format:

```rust
use acp_adapter::converters::rlog::notification_to_rlog_line;

let rlog_line = notification_to_rlog_line(&notification)?;
println!("{}", rlog_line);
// Output: {"type":"AgentMessageChunk","chunk":{...}}
```

### rlog to ACP

Parse rlog line back to ACP notification:

```rust
use acp_adapter::converters::rlog::rlog_line_to_notification;

let line = r#"{"type":"ToolCall","toolCall":{"id":"t1","name":"read"}}"#;
let notification = rlog_line_to_notification(line, &session_id)?;
```

### Codex to ACP

Convert Codex ThreadEvent to ACP:

```rust
use acp_adapter::converters::codex::thread_event_to_notification;
use codex_agent_sdk::ThreadEvent;

let thread_event = ThreadEvent::CommandExecution { command: "ls -la" };
let notification = thread_event_to_notification(&thread_event, &session_id)?;
```

## Permission System

The permission system provides flexible control over what agents can do.

### Permission Request Structure

```rust
pub struct RequestPermissionRequest {
    pub operation: String,         // "read" | "write" | "execute" | "network"
    pub resource: String,          // File path, command, URL, etc.
    pub details: Option<String>,   // Additional context
}

pub struct RequestPermissionResponse {
    pub approved: bool,
    pub message: Option<String>,
}
```

### Built-in Handlers

#### AllowAllPermissions

Auto-approves everything (useful for trusted automation):

```rust
use acp_adapter::{OpenAgentsClient, AllowAllPermissions};

let client = OpenAgentsClient::new(AllowAllPermissions, cwd);
```

#### DenyAllPermissions

Auto-denies everything (useful for read-only analysis):

```rust
use acp_adapter::DenyAllPermissions;

let client = OpenAgentsClient::new(DenyAllPermissions, cwd);
```

#### UiPermissionHandler

Delegates to UI via channel:

```rust
use acp_adapter::{UiPermissionHandler, PermissionRequestManager};
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel(100);
let manager = PermissionRequestManager::new(rx);
let handler = UiPermissionHandler::new(tx);

// Use handler with client
let client = OpenAgentsClient::new(handler, cwd);

// Handle requests in UI loop
tokio::spawn(async move {
    while let Some(request) = manager.next_request().await {
        // Show UI dialog
        let approved = show_permission_dialog(&request);

        // Respond
        manager.respond(request.id, approved, None).await;
    }
});
```

### Custom Permission Handler

```rust
use acp_adapter::PermissionHandler;
use agent_client_protocol_schema as acp;
use async_trait::async_trait;

struct PolicyBasedHandler {
    allow_read: bool,
    allow_write: bool,
}

#[async_trait]
impl PermissionHandler for PolicyBasedHandler {
    async fn request_permission(
        &self,
        request: &acp::RequestPermissionRequest
    ) -> acp::RequestPermissionResponse {
        let approved = match request.operation.as_str() {
            "read" => self.allow_read,
            "write" => self.allow_write,
            "execute" => false,  // Never allow shell commands
            _ => false,
        };

        acp::RequestPermissionResponse::new(approved)
            .message(if approved {
                "Approved by policy"
            } else {
                "Denied by policy"
            })
    }
}
```

## Integration Guide

### Integrating with Actix Web Server

```rust
use actix_web::{web, App, HttpServer};
use acp_adapter::AcpAgentConnection;
use std::sync::Arc;
use tokio::sync::RwLock;

struct AppState {
    connections: Arc<RwLock<HashMap<String, AcpAgentConnection>>>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = web::Data::new(AppState {
        connections: Arc::new(RwLock::new(HashMap::new())),
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/api/acp/sessions", web::post().to(create_session))
            .route("/api/acp/sessions/{id}/prompt", web::post().to(send_prompt))
            .route("/api/acp/sessions/{id}", web::delete().to(close_session))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

### Integrating with Desktop GUI (wry)

```rust
use acp_adapter::agents::codex::connect_codex;
use wry::application::window::Window;

fn setup_acp_integration(window: &Window) {
    tauri::async_runtime::spawn(async move {
        let connection = connect_codex(Default::default(), &PathBuf::from(".")).await.unwrap();

        // Expose to webview
        window.eval(&format!(
            "window.acpConnection = {{ sessionId: '{}' }}",
            connection.agent_name
        ));
    });
}
```

## Troubleshooting

### Agent Not Found

**Error:** `AgentNotFound: Codex Code executable not found`

**Solution:**
```bash
# Install Codex Code
npm install -g @openai-ai/codex-code

# Or specify custom path
let config = CodexAgentConfig::new()
    .executable_path("/custom/path/to/codex");
```

### Initialization Timeout

**Error:** Agent doesn't respond to `initialize` request

**Causes:**
- Agent binary doesn't support ACP protocol
- Incorrect working directory
- Missing environment variables

**Debug:**
```rust
// Enable debug logging
std::env::set_var("RUST_LOG", "acp_adapter=debug");
tracing_subscriber::fmt::init();

// Check stderr from agent
let mut cmd = tokio::process::Command::new("codex");
cmd.stderr(std::process::Stdio::piped());
```

### Permission Deadlock

**Error:** Agent hangs waiting for permission response

**Cause:** `UiPermissionHandler` channel not being consumed

**Solution:**
```rust
// Ensure permission handler loop is running
let manager = PermissionRequestManager::new(rx);

tokio::spawn(async move {
    while let Some(req) = manager.next_request().await {
        manager.respond(req.id, true, None).await;  // Must respond!
    }
});
```

### Replay Errors

**Error:** `Failed to parse rlog line`

**Causes:**
- Incomplete session (crashed mid-recording)
- Manual editing of rlog file
- Version mismatch

**Solution:**
```rust
// Use lenient parsing
let config = ReplayConfig::instant().skip_empty(true);

// Handle errors gracefully
match replayer.replay_file(path, tx).await {
    Ok(stats) => {
        if stats.errors > 0 {
            eprintln!("Replayed with {} errors", stats.errors);
        }
    }
    Err(e) => eprintln!("Replay failed: {}", e),
}
```

### Memory Leak

**Error:** Memory usage grows unbounded

**Cause:** Sessions accumulate notifications without cleanup

**Solution:**
```rust
// Implement periodic cleanup
if session.notification_count() > 10000 {
    session.checkpoint().await?;
    session.clear_before(checkpoint_id).await?;
}

// Or use bounded channels
let (tx, rx) = mpsc::channel(100);  // Backpressure after 100
```

### Codex Events Not Showing

**Error:** Codex tool calls don't appear in UI

**Cause:** Codex uses different event types than Codex

**Solution:**
```rust
use acp_adapter::converters::codex::thread_event_to_notification;

// Convert Codex ThreadEvent to ACP
let notification = thread_event_to_notification(&event, &session_id)?;
```

---

## See Also

- [ACP Protocol Specification](https://agentclientprotocol.com)
- [Zed ACP Component Mapping](./zed-acp-component-mapping.md)
- [Crate README](../crates/acp-adapter/README.md)
- [OpenAgents Architecture](./architecture.md)
