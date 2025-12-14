# Coder Integration Plan

> **Status: COMPLETED**
>
> This plan was implemented. The recommended hybrid approach (D + A: Service Layer + Event Bridge)
> has been fully integrated. See [SERVICE_LAYER.md](./SERVICE_LAYER.md) for the implementation details.

This document outlines proposed approaches for integrating the AI infrastructure (llm, session, permission, agent, storage, tool_registry) with the coder_app UI layer.

## Current State

### What We Have

**AI Infrastructure (Backend)**
```
llm/                  → LlmProvider trait, Anthropic streaming, ProviderRegistry
tool_registry/        → Tool trait, standard tools (bash, read, write, edit, grep, find)
coder/permission/     → PermissionManager with async ask/respond
coder/session/        → Processor, PromptBuilder, Session state
coder/agent/          → AgentRegistry, built-in agents (general, explore, plan, build)
coder/storage/        → SqliteStorage for persistence
mechacoder/           → Router, Backend detection, ServerMessage types
```

**UI Layer (Frontend)**
```
coder/domain/         → DomainEvent, entities, projections (ChatView)
coder/ui_runtime/     → Signal<T>, Memo<T>, Effect, Scheduler
coder/widgets/        → Widget trait, Div, Text, Button, Input
coder/surfaces_chat/  → ChatThread, MessageBubble, ToolUseIndicator
coder/app/            → Application entry, AppState
```

### The Gap

The AI infrastructure produces `SessionEvent` and `PermissionEvent`.
The UI layer consumes `DomainEvent` and renders via `Signal<T>`.

**We need a bridge** that:
1. Converts `SessionEvent` → `DomainEvent`
2. Routes `PermissionEvent` → UI dialog signals
3. Handles user input → `CompletionRequest`
4. Manages the conversation loop

---

## Proposed Integration Approaches

### Approach A: Event Bridge Pattern

**Concept**: Create a dedicated bridge layer that translates between AI events and domain events.

```
┌─────────────────────────────────────────────────────────────┐
│                        coder_app                             │
│                     (AppState + UI)                          │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │ DomainEvent
                              │
┌─────────────────────────────────────────────────────────────┐
│                     coder_bridge (NEW)                       │
│  SessionEvent → DomainEvent                                  │
│  PermissionEvent → PermissionDialogSignal                    │
│  UserInput → CompletionRequest                               │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │ SessionEvent, PermissionEvent
                              │
┌─────────────────────────────────────────────────────────────┐
│                    coder_session                             │
│                    (Processor)                               │
└─────────────────────────────────────────────────────────────┘
```

**New Crate: `coder_bridge`**

```rust
pub struct Bridge {
    // AI infrastructure
    provider_registry: Arc<ProviderRegistry>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    storage: Arc<dyn Storage>,
    agent_registry: AgentRegistry,

    // Event channels
    domain_event_tx: mpsc::UnboundedSender<DomainEvent>,
    session_event_rx: mpsc::UnboundedReceiver<SessionEvent>,
    permission_event_rx: mpsc::UnboundedReceiver<PermissionEvent>,
}

impl Bridge {
    /// Start a new conversation
    pub async fn send_message(&self, thread_id: ThreadId, content: String);

    /// Respond to permission request
    pub async fn respond_permission(&self, permission_id: PermissionId, response: Response);

    /// Cancel current operation
    pub async fn cancel(&self);

    /// Run the event processing loop (spawned as tokio task)
    pub async fn run(&mut self);
}
```

**Pros:**
- Clean separation of concerns
- AI infrastructure remains UI-agnostic
- Easy to test bridge in isolation
- Could support multiple frontends (CLI, TUI, GUI)

**Cons:**
- Additional crate to maintain
- Extra event translation layer
- Slightly more complex data flow

---

### Approach B: Direct Integration in AppState

**Concept**: Embed AI infrastructure directly into AppState, using signals for reactivity.

```rust
// In coder_app/src/state.rs
pub struct AppState {
    // Existing UI state
    pub threads: HashMap<ThreadId, Signal<ChatView>>,
    pub active_thread: Signal<Option<ThreadId>>,

    // AI infrastructure (NEW)
    provider_registry: Arc<ProviderRegistry>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    storage: Arc<SqliteStorage>,
    agent_registry: AgentRegistry,

    // Active sessions
    sessions: HashMap<SessionId, Session>,
    active_processor: Option<JoinHandle<()>>,

    // Permission UI state
    pending_permission: Signal<Option<(PermissionId, PermissionRequest)>>,
}

impl AppState {
    pub async fn send_message(&mut self, thread_id: ThreadId, content: String) {
        // 1. Create/get session
        // 2. Build request with PromptBuilder
        // 3. Spawn processor task
        // 4. SessionEvents update signals directly
    }
}
```

**Pros:**
- Simpler architecture (fewer crates)
- Direct access to signals for updates
- Less boilerplate

**Cons:**
- AppState becomes large and complex
- Harder to test AI logic in isolation
- Tighter coupling between UI and AI

---

### Approach C: Actor Model with Message Passing

**Concept**: Each component is an actor that communicates via typed messages.

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   UIActor    │◄──►│ SessionActor │◄──►│ ProviderActor│
│  (signals)   │    │  (processor) │    │   (llm)      │
└──────────────┘    └──────────────┘    └──────────────┘
        ▲                   ▲
        │                   │
        ▼                   ▼
┌──────────────┐    ┌──────────────┐
│PermissionActor│   │ StorageActor │
│  (ask/respond)│   │  (sqlite)    │
└──────────────┘    └──────────────┘
```

**Message Types:**
```rust
enum UIMessage {
    SendMessage { thread_id, content },
    RespondPermission { permission_id, response },
    Cancel,
    Navigate(Route),
}

enum SessionMessage {
    Start { thread_id, content },
    StreamEvent(StreamEvent),
    ToolResult { tool_call_id, result },
    Abort,
}

enum ProviderMessage {
    Stream { request: CompletionRequest },
    Cancel,
}
```

**Pros:**
- Highly decoupled
- Easy to add new actors
- Natural concurrency model
- Good for complex multi-agent scenarios

**Cons:**
- More complex to implement
- Requires actor framework (tokio actors or custom)
- Debugging message flows is harder
- Overkill for current needs?

---

### Approach D: Service Layer Pattern

**Concept**: Create a `ChatService` that encapsulates all AI operations behind a simple API.

```rust
// New: coder_service crate or module
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}

struct ChatServiceInner {
    provider_registry: ProviderRegistry,
    tool_registry: ToolRegistry,
    permission_manager: PermissionManager,
    storage: SqliteStorage,
    agent_registry: AgentRegistry,
}

impl ChatService {
    /// Send a message and get a stream of UI updates
    pub fn send_message(
        &self,
        thread_id: ThreadId,
        content: String,
    ) -> impl Stream<Item = ChatUpdate> {
        // Returns a stream that the UI can consume
    }

    /// Respond to permission
    pub async fn respond_permission(&self, id: PermissionId, response: Response);

    /// List available agents
    pub fn agents(&self) -> Vec<AgentDefinition>;

    /// Get conversation history
    pub async fn get_thread(&self, id: ThreadId) -> Option<Thread>;
}

pub enum ChatUpdate {
    MessageStarted { message_id: MessageId },
    TextDelta { delta: String },
    ToolStarted { tool_name: String, tool_call_id: String },
    ToolProgress { tool_call_id: String, progress: String },
    ToolCompleted { tool_call_id: String, success: bool },
    MessageCompleted { finish_reason: String },
    PermissionRequired { permission_id: PermissionId, request: PermissionRequest },
    Error { message: String },
}
```

**Usage in AppState:**
```rust
impl AppState {
    pub fn send_message(&mut self, thread_id: ThreadId, content: String) {
        let stream = self.chat_service.send_message(thread_id, content);

        // Spawn task to process updates
        let chat_view = self.threads.get(&thread_id).unwrap().clone();
        let permission_signal = self.pending_permission.clone();

        tokio::spawn(async move {
            pin_mut!(stream);
            while let Some(update) = stream.next().await {
                match update {
                    ChatUpdate::TextDelta { delta } => {
                        chat_view.update(|v| v.append_streaming(&delta));
                    }
                    ChatUpdate::PermissionRequired { permission_id, request } => {
                        permission_signal.set(Some((permission_id, request)));
                    }
                    // ... handle other updates
                }
            }
        });
    }
}
```

**Pros:**
- Simple, clean API
- Stream-based updates fit well with reactive UI
- Service is testable in isolation
- Hides complexity from UI layer

**Cons:**
- Service becomes a "god object" over time
- Stream processing logic still lives in AppState

---

## Recommended Approach: D (Service Layer) + A (Event Bridge)

**Hybrid approach** combining the best of both:

1. **ChatService** provides the public API (simple, stream-based)
2. **Internal bridge** handles event translation
3. **AppState** just consumes the stream

```
┌─────────────────────────────────────────────────────────────┐
│                        coder_app                             │
│                  AppState consumes Stream                    │
└─────────────────────────────────────────────────────────────┘
                              ▲
                              │ Stream<ChatUpdate>
                              │
┌─────────────────────────────────────────────────────────────┐
│                      ChatService                             │
│            Simple API: send_message() -> Stream              │
├─────────────────────────────────────────────────────────────┤
│                   Internal Bridge                            │
│       SessionEvent/PermissionEvent → ChatUpdate              │
├─────────────────────────────────────────────────────────────┤
│              AI Infrastructure                               │
│    Processor + PermissionManager + ProviderRegistry          │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Plan

#### Phase 1: Create ChatService (coder_service)

```
crates/coder/service/
├── Cargo.toml
├── src/
│   ├── lib.rs           # ChatService struct
│   ├── update.rs        # ChatUpdate enum
│   ├── bridge.rs        # Internal event translation
│   └── config.rs        # Service configuration
```

**Key Files:**

```rust
// lib.rs
pub struct ChatService { /* ... */ }

impl ChatService {
    pub async fn new(config: ServiceConfig) -> Result<Self>;
    pub fn send_message(&self, thread_id: ThreadId, content: &str) -> ChatStream;
    pub async fn respond_permission(&self, id: PermissionId, response: Response);
    pub async fn cancel(&self, session_id: SessionId);
    pub fn agents(&self) -> &AgentRegistry;
    pub fn storage(&self) -> &dyn Storage;
}

pub type ChatStream = Pin<Box<dyn Stream<Item = ChatUpdate> + Send>>;
```

```rust
// update.rs
pub enum ChatUpdate {
    // Session lifecycle
    SessionStarted { session_id: SessionId },
    SessionEnded { session_id: SessionId },

    // Message streaming
    MessageStarted { message_id: MessageId, role: Role },
    TextDelta { message_id: MessageId, delta: String },
    MessageCompleted { message_id: MessageId, finish_reason: String },

    // Tool use
    ToolStarted { message_id: MessageId, tool_call_id: String, tool_name: String },
    ToolInput { tool_call_id: String, partial_json: String },
    ToolCompleted { tool_call_id: String, output: String, is_error: bool },

    // Permission
    PermissionRequired { permission_id: PermissionId, request: PermissionRequest },
    PermissionResolved { permission_id: PermissionId, granted: bool },

    // Errors
    Error { message: String, recoverable: bool },

    // Metadata
    UsageUpdate { tokens: u64, cost: f64 },
}
```

#### Phase 2: Integrate with AppState

```rust
// coder_app/src/state.rs
pub struct AppState {
    // Service
    chat_service: ChatService,

    // UI state (existing)
    threads: HashMap<ThreadId, Signal<ChatView>>,
    active_thread: Signal<Option<ThreadId>>,

    // Permission dialog
    permission_dialog: Signal<Option<PermissionDialog>>,

    // Status
    status: Signal<AppStatus>,
}

struct PermissionDialog {
    permission_id: PermissionId,
    request: PermissionRequest,
}

impl AppState {
    pub async fn init() -> Self {
        let config = ServiceConfig::from_env();
        let chat_service = ChatService::new(config).await.unwrap();
        // ...
    }

    pub fn send_message(&self, thread_id: ThreadId, content: String) {
        let stream = self.chat_service.send_message(thread_id, &content);
        self.spawn_update_handler(thread_id, stream);
    }

    fn spawn_update_handler(&self, thread_id: ThreadId, stream: ChatStream) {
        let chat_view = self.threads.get(&thread_id).cloned();
        let permission_dialog = self.permission_dialog.clone();
        let status = self.status.clone();

        tokio::spawn(async move {
            pin_mut!(stream);
            while let Some(update) = stream.next().await {
                Self::handle_update(update, &chat_view, &permission_dialog, &status);
            }
        });
    }

    fn handle_update(
        update: ChatUpdate,
        chat_view: &Option<Signal<ChatView>>,
        permission_dialog: &Signal<Option<PermissionDialog>>,
        status: &Signal<AppStatus>,
    ) {
        match update {
            ChatUpdate::TextDelta { delta, .. } => {
                if let Some(view) = chat_view {
                    view.update(|v| v.append_streaming(&delta));
                }
            }
            ChatUpdate::PermissionRequired { permission_id, request } => {
                permission_dialog.set(Some(PermissionDialog { permission_id, request }));
            }
            ChatUpdate::Error { message, .. } => {
                status.set(AppStatus::Error(message));
            }
            // ... other handlers
        }
    }
}
```

#### Phase 3: Wire Up UI Components

**PermissionDialog Widget:**
```rust
// surfaces_chat/permission_dialog.rs
pub struct PermissionDialogWidget {
    dialog: Signal<Option<PermissionDialog>>,
    on_respond: Box<dyn Fn(PermissionId, Response)>,
}

impl Widget for PermissionDialogWidget {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        if let Some(dialog) = self.dialog.get() {
            // Render modal with:
            // - Title: dialog.request.title
            // - Description: dialog.request.description
            // - Buttons: "Allow Once", "Always Allow", "Deny"
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Handle button clicks, call on_respond
    }
}
```

**ChatInput Integration:**
```rust
// surfaces_chat/input.rs
impl ChatInput {
    fn on_submit(&mut self) {
        let content = self.input_value.get();
        if !content.is_empty() {
            // Dispatch command
            self.commands.dispatch(Command::SendMessage {
                thread_id: self.thread_id,
                content,
            });
            self.input_value.set(String::new());
        }
    }
}
```

---

## Additional Components Needed

### 1. Missing Tool Wrappers

```rust
// tool_registry/src/wrappers/glob.rs
pub struct GlobTool;
impl Tool for GlobTool { /* ... */ }

// tool_registry/src/wrappers/webfetch.rs
pub struct WebFetchTool;
impl Tool for WebFetchTool { /* ... */ }
```

### 2. Additional Providers

```rust
// llm/src/provider/openai.rs
pub struct OpenAIProvider { /* ... */ }

// llm/src/provider/ollama.rs
pub struct OllamaProvider { /* ... */ }
```

### 3. Error Recovery

```rust
// coder_service/src/retry.rs
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f32,
}

impl ChatService {
    async fn with_retry<F, T>(&self, policy: &RetryPolicy, f: F) -> Result<T>
    where
        F: Fn() -> Future<Output = Result<T>>;
}
```

### 4. Cost Tracking

```rust
// coder_service/src/cost.rs
pub struct CostTracker {
    pub session_costs: HashMap<SessionId, f64>,
    pub total_cost: f64,
    pub budget_limit: Option<f64>,
}

impl CostTracker {
    pub fn track(&mut self, session_id: SessionId, usage: &Usage, pricing: &ModelPricing);
    pub fn is_over_budget(&self) -> bool;
}
```

---

## Migration Path

### Step 1: Create coder_service (no UI changes)
- Implement ChatService
- Add unit tests
- Verify with integration test (no UI)

### Step 2: Add to AppState (minimal UI changes)
- Import ChatService
- Add initialization
- Keep existing DomainEvent flow working

### Step 3: Wire send_message (partial integration)
- Connect ChatInput → ChatService
- Process ChatUpdate → ChatView
- Test message sending

### Step 4: Add PermissionDialog
- Create widget
- Wire to permission_dialog signal
- Test permission flow

### Step 5: Remove legacy paths
- Remove any direct mechacoder usage
- Clean up unused code

---

## Open Questions

1. **Threading Model**: Should ChatService own a tokio runtime, or use the app's runtime?

2. **Persistence**: When to auto-save conversations? On every message? On session end?

3. **Multi-Agent**: How to handle subagent spawning? Nested streams? Separate sessions?

4. **Cancellation**: How to handle cancel mid-tool-execution? Graceful vs immediate?

5. **Offline Mode**: Cache provider responses? Queue messages for later?

---

## Timeline Estimate

| Phase | Scope | Complexity |
|-------|-------|------------|
| Phase 1: ChatService | New crate, bridge logic | Medium |
| Phase 2: AppState Integration | Wire to existing state | Low |
| Phase 3: UI Widgets | Permission dialog, status | Medium |
| Phase 4: Missing Tools | glob, webfetch | Low |
| Phase 5: Additional Providers | OpenAI, Ollama | Medium |
| Phase 6: Production Hardening | Retry, cost, recovery | Medium |

---

## Summary

**Recommended**: Approach D (Service Layer) with internal bridge

**Key Benefits**:
- Simple API for UI layer
- Stream-based updates fit reactive model
- Testable in isolation
- Hides AI complexity from AppState

**New Crate**: `coder_service` (~500-800 lines)

**Files to Modify**:
- `coder_app/src/state.rs` - Add ChatService, update handlers
- `coder_app/src/lib.rs` - Initialize service
- `surfaces_chat/` - Add PermissionDialog widget

---

## Implementation Status

> **Completed: December 2024**

### What Was Implemented

| Component | Status | Location |
|-----------|--------|----------|
| ChatService crate | Done | `crates/coder/service/` |
| ChatUpdate enum (17 variants) | Done | `crates/coder/service/src/update.rs` |
| Internal Bridge | Done | `crates/coder/service/src/bridge.rs` |
| ServiceConfig | Done | `crates/coder/service/src/service.rs` |
| service_handler integration | Done | `crates/coder/app/src/service_handler.rs` |
| Feature flags | Done | `crates/coder/app/Cargo.toml` |
| App integration | Done | `crates/coder/app/src/app.rs` |

### Crate Structure

```
crates/coder/service/
├── Cargo.toml
└── src/
    ├── lib.rs           # Module exports
    ├── service.rs       # ChatService implementation
    ├── update.rs        # ChatUpdate enum
    └── bridge.rs        # Event translation
```

### Test Results

63 tests passing across 9 AI infrastructure crates:
- `coder_domain`: 9 tests
- `coder_storage`: 12 tests
- `coder_permission`: 3 tests
- `coder_session`: 5 tests
- `coder_agent`: 4 tests
- `tool_registry`: 3 tests
- `llm`: 3 tests
- `coder_service`: 15 tests
- `coder_app`: 9 tests

### Feature Flags

```toml
[features]
default = ["coder-service"]  # Use ChatService
legacy = []                   # Use mechacoder (Claude Code CLI)
```

### Usage

```bash
# Run with ChatService (default)
cargo run -p coder_app

# Run with legacy Claude Code CLI
cargo run -p coder_app --no-default-features --features legacy
```

### Remaining Work

The following items were identified but not implemented:

- [ ] Permission dialog widget in UI
- [ ] OpenAI provider
- [ ] Ollama provider
- [ ] Session persistence (auto-save)
- [ ] Cost tracking UI
- [ ] Parallel tool execution

See [SERVICE_LAYER.md](./SERVICE_LAYER.md) for comprehensive API documentation.
