# Coder AI Infrastructure

This document describes the AI and session management infrastructure that powers Coder's intelligent features. This layer sits between the domain model and LLM providers, handling conversation management, tool execution, permissions, and persistence.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              coder/app                                       │
│                         (Application Entry)                                  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            coder/session                                     │
│                  (Session Processor & Prompt Builder)                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         │                          │                          │
         ▼                          ▼                          ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│  coder/agent    │      │ coder/permission│      │  coder/storage  │
│  (Definitions)  │      │  (Ask/Respond)  │      │   (SQLite DB)   │
└─────────────────┘      └─────────────────┘      └─────────────────┘
         │                          │
         │                          │
         ▼                          │
┌─────────────────┐                 │
│  tool_registry  │◄────────────────┘
│    (Tools)      │
└─────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                                  llm                                         │
│            (Provider Abstraction: Anthropic, OpenAI, Ollama)                │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Crate Dependency Flow

```
llm                   # LLM provider abstraction (no dependencies on coder)
  ↑
tool_registry         # Tool execution framework
  ↑
coder_permission      # Permission ask/respond system
  ↑
coder_session         # Session processor & prompt building
  ↑
coder_agent          # Agent definitions & registry
  ↑
coder_app            # Application entry point
```

---

## 1. LLM Provider Layer (`crates/llm/`)

The `llm` crate provides a unified interface for interacting with multiple LLM providers.

### Core Trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider identifier (e.g., "anthropic", "openai")
    fn id(&self) -> &'static str;

    /// Display name for UI
    fn display_name(&self) -> &'static str;

    /// Check if provider is available (has credentials)
    async fn is_available(&self) -> bool;

    /// Get available models for this provider
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Create a streaming completion
    async fn stream(&self, request: CompletionRequest) -> Result<CompletionStream, ProviderError>;

    /// Provider-specific capabilities
    fn capabilities(&self) -> ProviderCapabilities;
}
```

### Provider Registry

The `ProviderRegistry` manages multiple providers and routes requests:

```rust
impl ProviderRegistry {
    /// Initialize with default providers (auto-detects available credentials)
    pub async fn init_defaults(&self) -> Result<(), ProviderError>;

    /// Register a custom provider
    pub async fn register(&self, provider: Arc<dyn LlmProvider>);

    /// Get provider for a model spec (e.g., "anthropic/claude-sonnet-4-20250514")
    pub async fn provider_for_model(&self, model_spec: &str) -> Option<Arc<dyn LlmProvider>>;

    /// Stream a completion
    pub async fn stream(
        &self,
        model_spec: &str,
        request: CompletionRequest,
    ) -> Result<CompletionStream, ProviderError>;
}
```

### Streaming Events

The streaming interface follows a structured event pattern:

```rust
pub enum StreamEvent {
    /// Stream started
    Start { model: String, provider: String },

    /// Text content delta
    TextDelta { id: String, delta: String },

    /// Reasoning/thinking delta (extended thinking)
    ReasoningDelta { id: String, delta: String, provider_metadata: Option<Value> },

    /// Tool call input streaming
    ToolInputStart { id: String, tool_name: String },
    ToolInputDelta { id: String, delta: String },
    ToolInputEnd { id: String },

    /// Complete tool call (ready for execution)
    ToolCall { tool_call_id: String, tool_name: String, input: Value },

    /// Tool result (after execution)
    ToolResult { tool_call_id: String, result: ToolResultContent, is_error: bool },

    /// Stream finished
    Finish { finish_reason: FinishReason, usage: Usage },

    /// Error occurred
    Error { error: StreamError },
}
```

### Message Types

```rust
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub system: Option<String>,
    pub tools: Vec<Tool>,
    pub tool_choice: Option<ToolChoice>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub stop: Vec<String>,
    pub provider_options: ProviderOptions,
}

pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

pub enum ContentBlock {
    Text { text: String },
    Image { source: ImageSource, media_type: Option<String> },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: ToolResultContent, is_error: bool },
    Reasoning { id: String, text: String },
}
```

### Anthropic Provider

The Anthropic provider supports:
- SSE streaming via `/messages` API
- Extended thinking (interleaved thinking beta)
- Fine-grained tool streaming
- Prompt caching
- Vision (image input)

```rust
let provider = AnthropicProvider::new()?; // Reads ANTHROPIC_API_KEY
let request = CompletionRequest::new("claude-sonnet-4-20250514")
    .system("You are a helpful assistant.")
    .message(Message::user("Hello!"));

let stream = provider.stream(request).await?;
```

---

## 2. Tool Registry (`crates/tool_registry/`)

The tool registry provides a unified interface for defining and executing tools.

### Tool Trait

```rust
#[async_trait]
pub trait Tool: Send + Sync + Debug {
    /// Associated input type for this tool
    type Input: DeserializeOwned + Send;

    /// Get tool metadata (name, description, schema)
    fn info(&self) -> ToolInfo;

    /// Execute the tool with parsed input
    async fn execute(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult<ToolOutput>;

    /// Check if permission is needed (return Some(request) if yes)
    fn check_permission(&self, input: &Self::Input, ctx: &ToolContext) -> Option<PermissionRequest>;

    /// Validate input before execution
    fn validate(&self, input: &Self::Input, ctx: &ToolContext) -> ToolResult<()>;
}
```

### Tool Context

Every tool execution receives a context with cancellation support:

```rust
pub struct ToolContext {
    /// Working directory for file operations
    pub working_dir: PathBuf,

    /// Cancellation token for long-running operations
    pub cancellation: CancellationToken,

    /// Session ID (for logging/tracking)
    pub session_id: Option<String>,

    /// Additional metadata
    pub metadata: Value,
}

impl ToolContext {
    /// Check if cancellation was requested
    pub fn is_cancelled(&self) -> bool;

    /// Wait for cancellation (async)
    pub async fn cancelled(&self);
}
```

### Tool Registry

```rust
impl ToolRegistry {
    /// Create with standard tools (bash, read, write, edit, grep, find)
    pub fn with_standard_tools() -> Self;

    /// Register a custom tool
    pub fn register<T: Tool + 'static>(&mut self, tool: T);

    /// Execute a tool by name
    pub async fn execute(
        &self,
        name: &str,
        input: Value,
        ctx: &ToolContext,
    ) -> ToolResult<ToolOutput>;

    /// Check permission for a tool
    pub fn check_permission(
        &self,
        name: &str,
        input: &Value,
        ctx: &ToolContext,
    ) -> Option<PermissionRequest>;

    /// Generate Anthropic-compatible tool definitions
    pub fn to_anthropic_tools(&self) -> Vec<Value>;
}
```

### Standard Tools

| Tool | Description | Input |
|------|-------------|-------|
| `bash` | Execute shell commands | `{ command, timeout_ms?, working_dir? }` |
| `read` | Read file contents | `{ path, offset?, limit? }` |
| `write` | Write/create files | `{ path, content }` |
| `edit` | Edit files with old/new string replacement | `{ path, old_string, new_string }` |
| `grep` | Search file contents with regex | `{ pattern, path?, ignore_case?, max_results? }` |
| `find` | Find files by glob pattern | `{ pattern, path?, max_results? }` |

### Permission Requests

Tools can request permission before execution:

```rust
pub struct PermissionRequest {
    pub permission_type: String,  // e.g., "bash", "file_write"
    pub title: String,
    pub description: String,
    pub patterns: Vec<String>,    // Patterns for "always allow"
    pub metadata: HashMap<String, Value>,
}
```

---

## 3. Permission System (`crates/coder/permission/`)

The permission system provides an async ask/respond pattern for tool authorization.

### Permission Manager

```rust
impl PermissionManager {
    /// Ask for permission (blocks until responded)
    pub async fn ask(&self, request: PermissionRequest) -> Result<(), PermissionError>;

    /// Respond to a pending permission request
    pub async fn respond(
        &self,
        session_id: SessionId,
        permission_id: PermissionId,
        response: Response,
    ) -> Result<(), PermissionError>;

    /// Subscribe to permission events
    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<PermissionEvent>;
}

pub enum Response {
    /// Allow once
    Once,
    /// Allow and remember for matching patterns
    Always,
    /// Reject with optional reason
    Reject,
}
```

### Session-Scoped State

Each session maintains its own permission state:

```rust
struct SessionState {
    /// Pending permission requests
    pending: HashMap<PermissionId, PendingRequest>,

    /// "Always allow" patterns by permission type
    always_allow: HashMap<String, Vec<CompiledPattern>>,
}
```

### Always Allow Patterns

The system supports glob-based pattern matching for "always allow" rules:

```rust
// User selects "Always Allow" for pattern "git *"
permission_manager.respond(session_id, permission_id, Response::Always).await?;

// Future "git status", "git diff", etc. are auto-allowed
let result = permission_manager.ask(request_for_git_log).await;
// -> Ok(()) immediately, no UI prompt
```

### Permission Events

The UI subscribes to permission events:

```rust
pub enum PermissionEvent {
    /// New permission request needs user input
    RequestCreated {
        session_id: SessionId,
        permission_id: PermissionId,
        request: PermissionRequest,
    },

    /// Permission was granted
    Granted { session_id, permission_id },

    /// Permission was rejected
    Rejected { session_id, permission_id, reason: String },
}
```

---

## 4. Session Management (`crates/coder/session/`)

The session crate manages conversation state and the main processing loop.

### Session State

```rust
pub struct Session {
    pub id: SessionId,
    pub thread_id: ThreadId,
    pub working_directory: PathBuf,
    pub title: Option<String>,
    pub status: SessionStatus,
    pub agent_config: AgentConfig,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_cost: f64,
    pub total_tokens: u64,
    pub current_message_id: Option<MessageId>,
}

pub enum SessionStatus {
    Idle,                                    // Waiting for user input
    Busy,                                    // Processing
    WaitingForPermission,                    // Blocked on permission
    Retrying { attempt: u32, next_retry_at: i64 }, // Retrying after error
    Error,                                   // In error state
}
```

### Processor

The `Processor` handles the main conversation loop:

```rust
impl Processor {
    pub fn new(
        config: ProcessorConfig,
        tool_registry: Arc<ToolRegistry>,
        permission_manager: Arc<PermissionManager>,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Self;

    /// Process a streaming response from the LLM
    pub async fn process_stream(
        &self,
        session: &mut Session,
        stream: CompletionStream,
    ) -> Result<ProcessResult, SessionError>;
}

pub enum ProcessResult {
    Continue,  // Tool use requires another turn
    Stop,      // Assistant finished or error
}
```

### Processing Flow

```
1. Receive CompletionStream from provider
2. Process StreamEvents:
   - TextDelta → Emit SessionEvent::TextDelta
   - ToolCall → Check permission, execute tool
   - Finish → Determine if continue or stop
3. If tool_use finish_reason → Return ProcessResult::Continue
4. Otherwise → Return ProcessResult::Stop
```

### Session Events

```rust
pub enum SessionEvent {
    StatusChanged { session_id, status: SessionStatus },
    MessageStarted { session_id, message_id },
    TextDelta { session_id, message_id, delta: String },
    ToolStarted { session_id, message_id, tool_name, tool_call_id },
    ToolCompleted { session_id, message_id, tool_call_id, success: bool },
    MessageCompleted { session_id, message_id, finish_reason: String },
    Error { session_id, message_id, error: String },
}
```

### System Prompt Builder

The `PromptBuilder` constructs system prompts with context:

```rust
impl PromptBuilder {
    pub fn new(working_dir: impl AsRef<Path>, agent_config: AgentConfig) -> Self;

    /// Add custom instructions (e.g., from CLAUDE.md)
    pub fn with_instructions(self, instructions: impl Into<String>) -> Self;

    /// Add git status information
    pub fn with_git_status(self, status: impl Into<String>) -> Self;

    /// Build the complete system prompt
    pub fn build(&self) -> String;
}

/// Load CLAUDE.md files from directory hierarchy
pub fn load_instructions(working_dir: &Path) -> Vec<String>;
```

---

## 5. Agent Definitions (`crates/coder/agent/`)

The agent crate defines agent configurations and built-in agents.

### Agent Definition

```rust
pub struct AgentDefinition {
    pub name: String,
    pub description: Option<String>,
    pub mode: AgentMode,
    pub built_in: bool,
    pub model: Option<AgentModelConfig>,
    pub prompt: Option<String>,
    pub tools: IndexMap<String, bool>,
    pub permission: AgentPermission,
    pub temperature: Option<f32>,
    pub max_steps: Option<u32>,
}

pub enum AgentMode {
    Subagent,  // Spawned by primary agents
    Primary,   // User-selectable
    All,       // Any context
}
```

### Agent Permissions

Each agent has its own permission configuration:

```rust
pub struct AgentPermission {
    pub edit: Permission,
    pub bash: IndexMap<String, Permission>,
    pub webfetch: Permission,
    pub doom_loop: Permission,
    pub external_directory: Permission,
}

pub enum Permission {
    Allow,  // Auto-allow
    Ask,    // Ask user
    Deny,   // Auto-deny
}

impl AgentPermission {
    /// Permissive preset (allow everything)
    pub fn permissive() -> Self;

    /// Read-only preset (no writes, limited bash)
    pub fn read_only() -> Self;

    /// Plan mode preset (read + safe commands)
    pub fn plan_mode() -> Self;
}
```

### Built-in Agents

| Agent | Mode | Description |
|-------|------|-------------|
| `general` | Subagent | General-purpose for complex tasks and parallel execution |
| `explore` | Subagent | Read-only file search specialist |
| `plan` | Primary | Planning mode with restricted permissions |
| `build` | Primary | Full-capability agent for implementation |

### Agent Registry

```rust
impl AgentRegistry {
    pub fn with_builtin_agents() -> Self;
    pub fn register(&mut self, agent: AgentDefinition);
    pub fn get(&self, name: &str) -> Option<Arc<AgentDefinition>>;
    pub fn list_primary(&self) -> Vec<Arc<AgentDefinition>>;
    pub fn list_subagents(&self) -> Vec<Arc<AgentDefinition>>;
}
```

---

## 6. Storage Layer (`crates/coder/storage/`)

SQLite-based persistence for sessions and messages.

### Storage Trait

```rust
#[async_trait]
pub trait Storage: Send + Sync {
    // Thread operations
    async fn create_thread(&self, title: &str) -> Result<ThreadId>;
    async fn get_thread(&self, id: ThreadId) -> Result<Option<ThreadRecord>>;
    async fn list_threads(&self, limit: usize, offset: usize) -> Result<Vec<ThreadRecord>>;
    async fn delete_thread(&self, id: ThreadId) -> Result<()>;

    // Message operations
    async fn add_message(&self, thread_id: ThreadId, msg: &MessageRecord) -> Result<MessageId>;
    async fn get_messages(&self, thread_id: ThreadId) -> Result<Vec<MessageRecord>>;
    async fn update_message(&self, id: MessageId, content: &str) -> Result<()>;

    // Session operations
    async fn save_session(&self, session: &SessionRecord) -> Result<()>;
    async fn get_session(&self, id: SessionId) -> Result<Option<SessionRecord>>;
    async fn list_sessions(&self) -> Result<Vec<SessionRecord>>;
}
```

### SQLite Implementation

```rust
impl SqliteStorage {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self>;
    pub async fn in_memory() -> Result<Self>;
}
```

### Schema

```sql
CREATE TABLE threads (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL REFERENCES threads(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    working_directory TEXT NOT NULL,
    title TEXT,
    status TEXT NOT NULL,
    agent_config TEXT NOT NULL,  -- JSON
    total_cost REAL NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

## Integration Example

### Complete Session Flow

```rust
// 1. Initialize components
let provider_registry = Arc::new(ProviderRegistry::new());
provider_registry.init_defaults().await?;

let tool_registry = Arc::new(ToolRegistry::with_standard_tools());
let (perm_tx, perm_rx) = mpsc::unbounded_channel();
let permission_manager = Arc::new(PermissionManager::new(perm_tx));
let (event_tx, event_rx) = mpsc::unbounded_channel();

// 2. Create session
let mut session = Session::new("/path/to/project");
session.agent_config = AgentConfig {
    agent_id: "build".into(),
    model_id: "claude-sonnet-4-20250514".into(),
    provider_id: "anthropic".into(),
    max_tokens: Some(8192),
    temperature: None,
};

// 3. Build system prompt
let instructions = load_instructions(&session.working_directory);
let prompt = PromptBuilder::new(&session.working_directory, session.agent_config.clone())
    .with_instructions(instructions.join("\n"))
    .with_git_status("On branch main\nnothing to commit")
    .build();

// 4. Create completion request
let request = CompletionRequest::new(&session.agent_config.model_id)
    .system(&prompt)
    .message(Message::user("Help me fix the bug in auth.rs"))
    .tools(tool_registry.to_anthropic_tools());

// 5. Get stream from provider
let provider = provider_registry.get("anthropic").await.unwrap();
let stream = provider.stream(request).await?;

// 6. Process with session processor
let processor = Processor::new(
    ProcessorConfig::default(),
    tool_registry,
    permission_manager,
    event_tx,
);

loop {
    let result = processor.process_stream(&mut session, stream).await?;
    match result {
        ProcessResult::Continue => {
            // Tool use - continue conversation with tool results
            let request = /* build request with tool results */;
            stream = provider.stream(request).await?;
        }
        ProcessResult::Stop => break,
    }
}
```

### UI Integration

```rust
// Spawn task to handle session events
tokio::spawn(async move {
    while let Some(event) = event_rx.recv().await {
        match event {
            SessionEvent::TextDelta { delta, .. } => {
                // Update UI with streaming text
                chat_view_signal.update(|v| v.append_streaming(delta));
            }
            SessionEvent::ToolStarted { tool_name, .. } => {
                // Show tool indicator
                chat_view_signal.update(|v| v.add_tool_indicator(tool_name));
            }
            SessionEvent::Error { error, .. } => {
                // Show error toast
                error_signal.set(Some(error));
            }
            // ...
        }
    }
});

// Spawn task to handle permission requests
tokio::spawn(async move {
    while let Some(event) = perm_rx.recv().await {
        if let PermissionEvent::RequestCreated { permission_id, request, .. } = event {
            // Show permission dialog
            permission_dialog_signal.set(Some((permission_id, request)));
        }
    }
});
```

---

## Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `ANTHROPIC_API_KEY` | Anthropic API key | For Anthropic provider |
| `OPENAI_API_KEY` | OpenAI API key | For OpenAI provider |
| `ANTHROPIC_BASE_URL` | Custom API endpoint | No (defaults to api.anthropic.com) |

### CLAUDE.md Files

The system automatically loads `CLAUDE.md` files from the project directory hierarchy:

```
/home/user/
  └── CLAUDE.md           # User-level instructions
/home/user/projects/
  └── myproject/
      └── CLAUDE.md       # Project-level instructions
```

Instructions are loaded from root to leaf, with more specific instructions coming later.

---

## Error Handling

### Error Types

```rust
// LLM Provider errors
pub enum ProviderError {
    MissingCredentials(String),
    Network(String),
    ApiError { status: u16, message: String },
    RateLimit(String),
    InvalidRequest(String),
    NotFound(String),
    Stream(String),
}

// Tool errors
pub enum ToolError {
    NotFound(String),
    InvalidInput(String),
    Execution(String),
    PermissionDenied(String),
    Timeout,
    Cancelled,
}

// Session errors
pub enum SessionError {
    NotFound(SessionId),
    Busy,
    Aborted,
    Storage(StorageError),
    Llm(String),
    Tool(String),
    Permission(String),
}

// Permission errors
pub enum PermissionError {
    SessionNotFound(SessionId),
    PermissionNotFound(PermissionId),
    Rejected { permission_id, reason: String },
    Timeout,
}
```

---

## Performance Considerations

### Streaming

- Events are processed as they arrive (no buffering)
- Text deltas are emitted immediately for responsive UI
- Tool calls are executed asynchronously

### Memory

- Sessions are lightweight (~500 bytes)
- Messages are stored in SQLite, not kept in memory
- Tool registry uses type-erased `Arc<dyn Tool>` for efficiency

### Concurrency

- Permission requests use async channels (no blocking)
- Tool execution supports cancellation via `CancellationToken`
- Multiple sessions can run concurrently

---

## 7. Service Layer (`crates/coder/service/`)

The service layer provides a unified API that bridges the AI infrastructure with the UI layer.

### ChatService

```rust
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}

impl ChatService {
    /// Create a new ChatService
    pub async fn new(config: ServiceConfig) -> Result<Self, ServiceError>;

    /// Create a new session
    pub async fn create_session(&self, working_directory: Option<PathBuf>) -> Result<Session, ServiceError>;

    /// Send a message and get a stream of updates
    pub fn send_message(&self, session_id: SessionId, content: String) -> ChatStream;

    /// Respond to a permission request
    pub async fn respond_permission(
        &self,
        session_id: SessionId,
        permission_id: PermissionId,
        response: PermissionResponse,
    ) -> Result<(), ServiceError>;

    /// Cancel an active session
    pub async fn cancel(&self, session_id: SessionId) -> Result<(), ServiceError>;

    /// Get the agent registry
    pub fn agents(&self) -> &AgentRegistry;

    /// Get the storage
    pub fn storage(&self) -> Arc<Storage>;
}

pub type ChatStream = Pin<Box<dyn Stream<Item = ChatUpdate> + Send>>;
```

### ChatUpdate Events

The service emits a stream of `ChatUpdate` events for the UI to consume:

```rust
pub enum ChatUpdate {
    // Session lifecycle
    SessionStarted { session_id, thread_id },
    SessionStatusChanged { session_id, status },
    SessionEnded { session_id, success, error },

    // Message streaming
    MessageStarted { session_id, message_id, role },
    TextDelta { session_id, message_id, delta },
    ReasoningDelta { session_id, message_id, delta },
    MessageCompleted { session_id, message_id, finish_reason },

    // Tool use
    ToolStarted { session_id, message_id, tool_call_id, tool_name },
    ToolInputDelta { session_id, tool_call_id, delta },
    ToolExecuting { session_id, tool_call_id, input },
    ToolProgress { session_id, tool_call_id, message },
    ToolCompleted { session_id, tool_call_id, output, is_error, duration_ms },

    // Permission
    PermissionRequired { session_id, permission_id, request },
    PermissionResolved { session_id, permission_id, granted },

    // Errors and metadata
    Error { session_id, message, code, recoverable },
    UsageUpdate { session_id, total_tokens, cost_usd },
    AgentInfo { session_id, agent_id, model_id, provider_id },
}
```

### Internal Bridge

The service uses an internal bridge to translate between `SessionEvent`/`PermissionEvent` and `ChatUpdate`:

```rust
struct Bridge {
    session_id: SessionId,
    thread_id: ThreadId,
    update_tx: mpsc::UnboundedSender<ChatUpdate>,
    tool_start_times: HashMap<String, Instant>,
}

impl Bridge {
    fn handle_session_event(&mut self, event: SessionEvent);
    fn handle_permission_event(&mut self, event: PermissionEvent);
}
```

### Usage Example

```rust
use coder_service::{ChatService, ServiceConfig, ChatUpdate};
use futures::StreamExt;

let config = ServiceConfig::from_env();
let service = ChatService::new(config).await?;

let session = service.create_session(None).await?;
let stream = service.send_message(session.id, "Hello!".into());

futures::pin_mut!(stream);
while let Some(update) = stream.next().await {
    match update {
        ChatUpdate::TextDelta { delta, .. } => print!("{}", delta),
        ChatUpdate::ToolStarted { tool_name, .. } => println!("[Using: {}]", tool_name),
        ChatUpdate::SessionEnded { .. } => break,
        _ => {}
    }
}
```

For comprehensive service layer documentation, see [SERVICE_LAYER.md](./SERVICE_LAYER.md).

---

## Future Enhancements

- **OpenAI Provider**: GPT-4, o1/o3 series with reasoning tokens
- **Ollama Provider**: Local model execution
- **Multi-turn Caching**: Reuse previous conversation context
- **Tool Result Streaming**: Stream tool output as it's generated
- **Parallel Tool Execution**: Execute independent tools concurrently
- **Session Persistence**: Auto-save/restore session state
- **Cost Tracking**: Per-session and aggregate cost metrics
