# Coder Service Layer

This document provides comprehensive documentation for the ChatService layer, which bridges the AI infrastructure with the UI layer.

## Overview

The **ChatService** implements the Service Layer + Event Bridge pattern, providing a simple stream-based API for the UI to consume while hiding the complexity of LLM providers, tool execution, and permission management.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              coder_app                                       │
│                    AppState consumes Stream<ChatUpdate>                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                     ▲
                                     │ ChatStream = Stream<ChatUpdate>
                                     │
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ChatService (coder_service)                        │
│                   Simple API: send_message() -> ChatStream                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                            Internal Bridge                                   │
│              SessionEvent/PermissionEvent → ChatUpdate                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                          AI Infrastructure                                   │
│         ProviderRegistry + ToolRegistry + PermissionManager + Storage        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Quick Start

### Basic Usage

```rust
use coder_service::{ChatService, ServiceConfig, ChatUpdate};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create service with default config
    let config = ServiceConfig::from_env();
    let service = ChatService::new(config).await?;

    // 2. Create a session
    let session = service.create_session(None).await?;
    println!("Session created: {}", session.id);

    // 3. Send a message and process updates
    let stream = service.send_message(session.id, "Hello, Claude!".into());
    futures::pin_mut!(stream);

    while let Some(update) = stream.next().await {
        match update {
            ChatUpdate::TextDelta { delta, .. } => print!("{}", delta),
            ChatUpdate::ToolStarted { tool_name, .. } => println!("\n[Using tool: {}]", tool_name),
            ChatUpdate::SessionEnded { .. } => println!("\n[Session ended]"),
            ChatUpdate::Error { message, .. } => eprintln!("\nError: {}", message),
            _ => {}
        }
    }

    Ok(())
}
```

### Feature Flags

The application supports two backend modes via Cargo features:

```toml
[features]
default = ["coder-service"]  # Use ChatService (built-in LLM providers)
legacy = []                   # Use mechacoder (Claude Code CLI)
```

Run with legacy backend:
```bash
cargo run -p coder_app --no-default-features --features legacy
```

---

## API Reference

### ChatService

The main service struct that orchestrates all AI operations.

```rust
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}
```

#### Constructor

```rust
impl ChatService {
    /// Create a new ChatService with the given configuration.
    ///
    /// Initializes:
    /// - ProviderRegistry (auto-detects available providers)
    /// - ToolRegistry (standard tools: bash, read, write, edit, grep, find)
    /// - PermissionManager
    /// - SqliteStorage
    /// - AgentRegistry (built-in agents)
    pub async fn new(config: ServiceConfig) -> Result<Self, ServiceError>;
}
```

#### Session Management

```rust
impl ChatService {
    /// Create a new session.
    ///
    /// # Arguments
    /// * `working_directory` - Optional working directory. Defaults to config.working_directory.
    ///
    /// # Returns
    /// A new Session with a unique ID and thread ID.
    pub async fn create_session(
        &self,
        working_directory: Option<PathBuf>,
    ) -> Result<Session, ServiceError>;

    /// Get a session by ID.
    pub async fn get_session(&self, session_id: SessionId) -> Option<Session>;

    /// List all active sessions.
    pub async fn list_sessions(&self) -> Vec<Session>;

    /// Cancel an active session.
    pub async fn cancel(&self, session_id: SessionId) -> Result<(), ServiceError>;
}
```

#### Messaging

```rust
impl ChatService {
    /// Send a message and get a stream of updates.
    ///
    /// This is the main entry point for chat operations. The returned stream
    /// emits `ChatUpdate` events as the AI processes the message.
    ///
    /// # Arguments
    /// * `session_id` - The session to send the message in
    /// * `content` - The user's message content
    ///
    /// # Returns
    /// A pinned stream of ChatUpdate events
    pub fn send_message(
        &self,
        session_id: SessionId,
        content: String,
    ) -> ChatStream;
}

/// Type alias for the chat update stream
pub type ChatStream = Pin<Box<dyn Stream<Item = ChatUpdate> + Send>>;
```

#### Permissions

```rust
impl ChatService {
    /// Respond to a permission request.
    ///
    /// Called when the UI receives a `ChatUpdate::PermissionRequired` event.
    ///
    /// # Arguments
    /// * `session_id` - The session the permission belongs to
    /// * `permission_id` - The ID of the pending permission
    /// * `response` - The user's response (Once, Always, or Reject)
    pub async fn respond_permission(
        &self,
        session_id: SessionId,
        permission_id: PermissionId,
        response: PermissionResponse,
    ) -> Result<(), ServiceError>;
}
```

#### Accessors

```rust
impl ChatService {
    /// Get the agent registry.
    pub fn agents(&self) -> &AgentRegistry;

    /// Get the storage backend.
    pub fn storage(&self) -> Arc<Storage>;
}
```

---

### ChatUpdate

All possible updates that can occur during a chat session.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatUpdate {
    // ═══════════════════════════════════════════════════════════════
    // Session Lifecycle
    // ═══════════════════════════════════════════════════════════════

    /// A new session has started.
    SessionStarted {
        session_id: SessionId,
        thread_id: ThreadId,
    },

    /// Session status changed.
    SessionStatusChanged {
        session_id: SessionId,
        status: SessionStatus,
    },

    /// Session has ended.
    SessionEnded {
        session_id: SessionId,
        success: bool,
        error: Option<String>,
    },

    // ═══════════════════════════════════════════════════════════════
    // Message Streaming
    // ═══════════════════════════════════════════════════════════════

    /// A new message has started (assistant response beginning).
    MessageStarted {
        session_id: SessionId,
        message_id: MessageId,
        role: MessageRole,
    },

    /// Text content delta (streaming text).
    TextDelta {
        session_id: SessionId,
        message_id: MessageId,
        delta: String,
    },

    /// Reasoning/thinking delta (for models with extended thinking).
    ReasoningDelta {
        session_id: SessionId,
        message_id: MessageId,
        delta: String,
    },

    /// Message has completed.
    MessageCompleted {
        session_id: SessionId,
        message_id: MessageId,
        finish_reason: String,
    },

    // ═══════════════════════════════════════════════════════════════
    // Tool Use
    // ═══════════════════════════════════════════════════════════════

    /// A tool use has started.
    ToolStarted {
        session_id: SessionId,
        message_id: MessageId,
        tool_call_id: String,
        tool_name: String,
    },

    /// Tool input is being streamed (partial JSON).
    ToolInputDelta {
        session_id: SessionId,
        tool_call_id: String,
        delta: String,
    },

    /// Tool input is complete, execution is starting.
    ToolExecuting {
        session_id: SessionId,
        tool_call_id: String,
        input: serde_json::Value,
    },

    /// Tool execution progress update.
    ToolProgress {
        session_id: SessionId,
        tool_call_id: String,
        message: String,
    },

    /// Tool execution has completed.
    ToolCompleted {
        session_id: SessionId,
        tool_call_id: String,
        output: String,
        is_error: bool,
        duration_ms: u64,
    },

    // ═══════════════════════════════════════════════════════════════
    // Permission
    // ═══════════════════════════════════════════════════════════════

    /// Permission is required before proceeding.
    PermissionRequired {
        session_id: SessionId,
        permission_id: PermissionId,
        request: PermissionRequest,
    },

    /// Permission has been resolved.
    PermissionResolved {
        session_id: SessionId,
        permission_id: PermissionId,
        granted: bool,
    },

    // ═══════════════════════════════════════════════════════════════
    // Errors
    // ═══════════════════════════════════════════════════════════════

    /// An error occurred.
    Error {
        session_id: SessionId,
        message: String,
        code: Option<String>,
        recoverable: bool,
    },

    // ═══════════════════════════════════════════════════════════════
    // Metadata
    // ═══════════════════════════════════════════════════════════════

    /// Token usage update.
    UsageUpdate {
        session_id: SessionId,
        total_tokens: u64,
        cost_usd: f64,
    },

    /// Agent information (sent at session start).
    AgentInfo {
        session_id: SessionId,
        agent_id: String,
        model_id: String,
        provider_id: String,
    },
}
```

#### Utility Methods

```rust
impl ChatUpdate {
    /// Get the session ID for this update.
    pub fn session_id(&self) -> SessionId;

    /// Check if this is an error update.
    pub fn is_error(&self) -> bool;

    /// Check if this update indicates the session has ended.
    pub fn is_terminal(&self) -> bool;
}
```

---

### ServiceConfig

Configuration for the ChatService.

```rust
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Default working directory for new sessions.
    pub working_directory: PathBuf,

    /// Database path for SQLite storage.
    pub database_path: PathBuf,

    /// Default agent ID (e.g., "build", "plan", "explore").
    pub default_agent: String,

    /// Default model ID (e.g., "claude-sonnet-4-20250514").
    pub default_model: String,

    /// Default provider ID (e.g., "anthropic").
    pub default_provider: String,

    /// Maximum turns in a conversation loop.
    pub max_turns: usize,

    /// Processor configuration.
    pub processor_config: ProcessorConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            database_path: PathBuf::from("coder.db"),
            default_agent: "build".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            default_provider: "anthropic".to_string(),
            max_turns: 50,
            processor_config: ProcessorConfig::default(),
        }
    }
}

impl ServiceConfig {
    /// Create config from environment variables.
    ///
    /// Reads:
    /// - CODER_WORKING_DIR
    /// - CODER_DATABASE
    /// - CODER_DEFAULT_AGENT
    /// - CODER_DEFAULT_MODEL
    /// - CODER_DEFAULT_PROVIDER
    pub fn from_env() -> Self;
}
```

---

### ServiceError

Errors that can occur in the ChatService.

```rust
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Session is busy")]
    SessionBusy,

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Permission error: {0}")]
    Permission(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
```

---

### SessionStatus

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is idle, waiting for input.
    Idle,
    /// Session is processing a request.
    Processing,
    /// Session is waiting for permission.
    WaitingForPermission,
    /// Session is executing a tool.
    ExecutingTool,
    /// Session is in error state.
    Error,
}
```

---

## Internal Architecture

### Bridge Module

The internal Bridge handles event translation between the session layer and the UI-facing ChatUpdate type.

```rust
pub struct Bridge {
    session_id: SessionId,
    thread_id: ThreadId,
    update_tx: mpsc::UnboundedSender<ChatUpdate>,
    tool_start_times: HashMap<String, Instant>,
}

impl Bridge {
    /// Handle a session event and emit ChatUpdate(s).
    pub fn handle_session_event(&mut self, event: SessionEvent);

    /// Handle a permission event and emit ChatUpdate(s).
    pub fn handle_permission_event(&mut self, event: PermissionEvent);

    /// Emit lifecycle events
    pub fn emit_session_started(&self);
    pub fn emit_session_ended(&self, success: bool, error: Option<String>);
    pub fn emit_agent_info(&self, agent_id: &str, model_id: &str, provider_id: &str);
    pub fn emit_usage(&self, total_tokens: u64, cost_usd: f64);
}
```

### Event Translation

| SessionEvent | ChatUpdate |
|--------------|------------|
| `StatusChanged` | `SessionStatusChanged` |
| `MessageStarted` | `MessageStarted` |
| `TextDelta` | `TextDelta` |
| `ToolStarted` | `ToolStarted` |
| `ToolCompleted` | `ToolCompleted` (with duration) |
| `MessageCompleted` | `MessageCompleted` |
| `Error` | `Error` |

| PermissionEvent | ChatUpdate |
|-----------------|------------|
| `RequestPending` | `PermissionRequired` + `SessionStatusChanged(WaitingForPermission)` |
| `RequestResponded(granted)` | `PermissionResolved` + `SessionStatusChanged(Processing)` |
| `RequestResponded(rejected)` | `PermissionResolved` |

---

## UI Integration

### service_handler Module

The `service_handler` module in `coder_app` provides the integration between ChatService and the existing UI architecture.

```rust
/// Messages from the UI to the service handler.
#[derive(Debug, Clone)]
pub enum ServiceRequest {
    /// Send a chat message.
    SendMessage { content: String, cwd: String },
    /// Cancel the current operation.
    Cancel,
}

/// Spawns a background thread with a tokio runtime to handle chat via ChatService.
pub fn spawn_service_handler(
    request_rx: mpsc::UnboundedReceiver<ServiceRequest>,
    response_tx: mpsc::UnboundedSender<ServerMessage>,
) -> JoinHandle<()>;
```

### App Integration

The App struct conditionally uses the service handler based on feature flags:

```rust
// With coder-service feature (default)
pub struct App {
    request_tx: mpsc::UnboundedSender<ServiceRequest>,
    server_rx: mpsc::UnboundedReceiver<ServerMessage>,
    // ...
}

impl App {
    pub fn new_with_service(
        request_tx: mpsc::UnboundedSender<ServiceRequest>,
        server_rx: mpsc::UnboundedReceiver<ServerMessage>,
    ) -> Self;
}

// With legacy feature
pub struct App {
    client_tx: mpsc::UnboundedSender<ClientMessage>,
    server_rx: mpsc::UnboundedReceiver<ServerMessage>,
    // ...
}

impl App {
    pub fn new(
        client_tx: mpsc::UnboundedSender<ClientMessage>,
        server_rx: mpsc::UnboundedReceiver<ServerMessage>,
    ) -> Self;
}
```

### Handling Updates in the UI

```rust
// In App::update()
while let Ok(msg) = self.server_rx.try_recv() {
    self.handle_server_message(msg);
}

fn handle_server_message(&mut self, msg: ServerMessage) {
    let mut view = self.chat_view.get();

    match msg {
        ServerMessage::TextDelta { text } => {
            // Update streaming message
            if let Some(streaming) = &mut view.streaming_message {
                streaming.content_so_far.push_str(&text);
            } else {
                view.streaming_message = Some(StreamingMessage {
                    id: MessageId::new(),
                    content_so_far: text,
                    is_complete: false,
                    started_at: Utc::now(),
                });
            }
            self.chat_view.set(view);
        }
        ServerMessage::Done { error } => {
            // Complete the streaming message
            if let Some(streaming) = view.streaming_message.take() {
                view.entries.push(ChatEntry::Message(MessageView {
                    id: streaming.id,
                    content: streaming.content_so_far,
                    role: Role::Assistant,
                    timestamp: Utc::now(),
                    has_tool_uses: false,
                }));
            }
            self.chat_view.set(view);
        }
        // ... handle other message types
    }
}
```

---

## Testing

### Unit Tests

The service layer includes comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.default_agent, "build");
        assert_eq!(config.default_model, "claude-sonnet-4-20250514");
        assert_eq!(config.default_provider, "anthropic");
    }

    #[test]
    fn test_chat_update_session_id() {
        let session_id = SessionId::new();
        let thread_id = ThreadId::new();
        let update = ChatUpdate::SessionStarted { session_id, thread_id };
        assert_eq!(update.session_id(), session_id);
    }

    #[test]
    fn test_chat_update_is_terminal() {
        let session_id = SessionId::new();

        let ended = ChatUpdate::SessionEnded {
            session_id,
            success: true,
            error: None,
        };
        assert!(ended.is_terminal());

        let error = ChatUpdate::Error {
            session_id,
            message: "Fatal".into(),
            code: None,
            recoverable: false,
        };
        assert!(error.is_terminal());

        let recoverable = ChatUpdate::Error {
            session_id,
            message: "Recoverable".into(),
            code: None,
            recoverable: true,
        };
        assert!(!recoverable.is_terminal());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_chat_service_integration() {
    // Set ANTHROPIC_API_KEY for this test
    let config = ServiceConfig::default();
    let service = ChatService::new(config).await.unwrap();

    let session = service.create_session(None).await.unwrap();
    assert!(service.get_session(session.id).await.is_some());

    let sessions = service.list_sessions().await;
    assert_eq!(sessions.len(), 1);
}
```

---

## Performance Characteristics

### Memory
- ChatService: ~1KB per instance
- Session: ~500 bytes
- ChatUpdate events: Streamed, not buffered

### Latency
- Session creation: <1ms
- First token: Depends on provider (~100-500ms)
- Event processing: <1ms per event

### Concurrency
- Multiple sessions supported
- Async/await throughout
- No blocking operations in main path

---

## Future Enhancements

### Planned
- **Permission Dialog Widget**: Native UI for permission prompts
- **Cost Tracking**: Per-session and aggregate cost metrics
- **Session Persistence**: Auto-save/restore across app restarts
- **Multi-turn Caching**: Reuse conversation context

### Potential
- **OpenAI Provider**: GPT-4, o1/o3 support
- **Ollama Provider**: Local model execution
- **Parallel Tool Execution**: Run independent tools concurrently
- **Tool Result Streaming**: Stream long tool outputs

---

## Related Documentation

- [AI_INFRASTRUCTURE.md](./AI_INFRASTRUCTURE.md) - LLM providers, tools, sessions
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Six-layer UI architecture
- [DATA_FLOW.md](./DATA_FLOW.md) - Event and data flow diagrams
- [CONFIGURATION.md](./CONFIGURATION.md) - All configuration options
