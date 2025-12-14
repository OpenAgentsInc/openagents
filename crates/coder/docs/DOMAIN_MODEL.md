# Domain Model: Event Sourcing in Coder

The domain model is the foundation of Coder's architecture. It implements **event sourcing**: all state changes are captured as immutable events, and current state is derived by replaying those events.

## Why Event Sourcing?

Traditional applications store **current state** in a database. Event sourcing instead stores **all state changes** as an append-only log of events.

### Benefits

1. **Complete Audit Trail**: Every state change is recorded with timestamp and context
2. **Time-Travel Debugging**: Replay events to any point in history
3. **Replay-ability**: Reconstruct state by replaying events
4. **Distributed Systems**: Events can be streamed to multiple consumers
5. **Flexibility**: Multiple projections from same event stream
6. **Testability**: Pure functions (events → state changes)

### Trade-Offs

- **Storage**: Events accumulate over time (but compress well)
- **Complexity**: Must maintain projections alongside events
- **Performance**: Replaying many events can be slow (use snapshots)

## Core Concepts

### Entities

Domain entities represent core business objects. Each has a strongly-typed UUID:

```rust
// ID macro generates consistent types
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct ThreadId(Uuid);

impl ThreadId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

**All Entity IDs**:
- `ThreadId` - A conversation thread
- `MessageId` - A chat message
- `RunId` - A workflow execution
- `StepId` - A single step in a run
- `ToolUseId` - A tool invocation
- `ProjectId` - A code project
- `FileId` - A file in a project

### Events

All state changes are captured as `DomainEvent`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DomainEvent {
    // Thread lifecycle
    ThreadCreated {
        thread_id: ThreadId,
        title: String,
        timestamp: DateTime<Utc>,
    },

    ThreadUpdated {
        thread_id: ThreadId,
        title: Option<String>,
        archived: Option<bool>,
        timestamp: DateTime<Utc>,
    },

    // Message events
    MessageAdded {
        thread_id: ThreadId,
        message_id: MessageId,
        content: String,
        role: Role,
        tool_uses: SmallVec<[ToolUseId; 4]>,
        timestamp: DateTime<Utc>,
    },

    MessageStreaming {
        thread_id: ThreadId,
        message_id: MessageId,
        content: String, // Partial content
        timestamp: DateTime<Utc>,
    },

    MessageComplete {
        thread_id: ThreadId,
        message_id: MessageId,
        final_content: String,
        timestamp: DateTime<Utc>,
    },

    MessageDeleted {
        thread_id: ThreadId,
        message_id: MessageId,
        timestamp: DateTime<Utc>,
    },

    // Tool execution
    ToolUseStarted {
        tool_use_id: ToolUseId,
        run_id: RunId,
        message_id: MessageId,
        tool_name: String,
        input: serde_json::Value,
        timestamp: DateTime<Utc>,
    },

    ToolUseProgress {
        tool_use_id: ToolUseId,
        progress: f32, // 0.0-1.0
        status_message: String,
        timestamp: DateTime<Utc>,
    },

    ToolUseComplete {
        tool_use_id: ToolUseId,
        result: ToolOutput,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    ToolUseFailed {
        tool_use_id: ToolUseId,
        error: String,
        timestamp: DateTime<Utc>,
    },

    ToolUseCancelled {
        tool_use_id: ToolUseId,
        timestamp: DateTime<Utc>,
    },

    // Run lifecycle
    RunStarted {
        run_id: RunId,
        thread_id: ThreadId,
        agent_name: String,
        timestamp: DateTime<Utc>,
    },

    RunStepStarted {
        run_id: RunId,
        step_id: StepId,
        step_type: StepType,
        description: String,
        timestamp: DateTime<Utc>,
    },

    RunStepUpdated {
        run_id: RunId,
        step_id: StepId,
        status: StepStatus,
        progress: Option<f32>,
        timestamp: DateTime<Utc>,
    },

    RunStepComplete {
        run_id: RunId,
        step_id: StepId,
        result: StepResult,
        timestamp: DateTime<Utc>,
    },

    RunArtifactAdded {
        run_id: RunId,
        artifact_id: String,
        artifact_type: ArtifactType,
        data: Vec<u8>,
        timestamp: DateTime<Utc>,
    },

    RunFinished {
        run_id: RunId,
        status: RunStatus,
        cost: Cost,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // Project management
    ProjectCreated {
        project_id: ProjectId,
        name: String,
        path: String,
        timestamp: DateTime<Utc>,
    },

    ProjectUpdated {
        project_id: ProjectId,
        changes: ProjectChanges,
        timestamp: DateTime<Utc>,
    },

    FileChanged {
        project_id: ProjectId,
        file_id: FileId,
        path: String,
        change_type: FileChangeType,
        timestamp: DateTime<Utc>,
    },
}
```

### Event Envelope

Events are wrapped in an envelope for ordering and metadata:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Sequential event number (1, 2, 3, ...)
    pub sequence: u64,

    /// The actual event
    pub event: DomainEvent,

    /// Causation ID (which event caused this one)
    pub causation_id: Option<Uuid>,

    /// Correlation ID (groups related events)
    pub correlation_id: Uuid,

    /// Metadata (user_id, session_id, etc.)
    pub metadata: HashMap<String, String>,
}
```

**Sequence numbers** ensure total ordering of events. Even in distributed systems, sequence numbers guarantee a canonical event order.

**Causation/Correlation IDs** track relationships between events:
- Causation: "Event A caused Event B"
- Correlation: "Events A, B, C are all part of the same user action"

## Entities in Detail

### Message

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub thread_id: ThreadId,
    pub content: String,
    pub role: Role,
    pub tool_uses: SmallVec<[ToolUseId; 4]>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub streaming: bool, // Is this message still being streamed?
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Message {
    pub fn new_user(thread_id: ThreadId, content: String) -> Self {
        Self {
            id: MessageId::new(),
            thread_id,
            content,
            role: Role::User,
            tool_uses: SmallVec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            streaming: false,
        }
    }

    pub fn new_assistant(thread_id: ThreadId) -> Self {
        Self {
            id: MessageId::new(),
            thread_id,
            content: String::new(),
            role: Role::Assistant,
            tool_uses: SmallVec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            streaming: true, // Start streaming
        }
    }

    pub fn append_content(&mut self, delta: &str) {
        self.content.push_str(delta);
        self.updated_at = Utc::now();
    }

    pub fn complete(&mut self) {
        self.streaming = false;
        self.updated_at = Utc::now();
    }
}
```

**Streaming**: Assistant messages start empty and stream in content deltas. The `streaming` flag indicates incomplete messages.

### ToolUse

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolUse {
    pub id: ToolUseId,
    pub run_id: RunId,
    pub message_id: MessageId,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: Option<ToolOutput>,
    pub status: ToolStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToolOutput {
    Text(String),
    Json(serde_json::Value),
    Binary(Vec<u8>),
    File { path: String, mime_type: String },
    Empty,
}

impl ToolUse {
    pub fn start(&mut self) {
        self.status = ToolStatus::Running;
        self.started_at = Utc::now();
    }

    pub fn complete(&mut self, output: ToolOutput) {
        self.status = ToolStatus::Success;
        self.output = Some(output);
        self.completed_at = Some(Utc::now());
        self.duration_ms = Some((Utc::now() - self.started_at).num_milliseconds() as u64);
    }

    pub fn fail(&mut self, error: String) {
        self.status = ToolStatus::Failed;
        self.error = Some(error);
        self.completed_at = Some(Utc::now());
        self.duration_ms = Some((Utc::now() - self.started_at).num_milliseconds() as u64);
    }
}
```

**Tool lifecycle**:
1. Created in `Pending` status
2. Transition to `Running` when execution starts
3. Complete with `Success` (with output) or `Failed` (with error)
4. Can be `Cancelled` at any time

### Run

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub thread_id: ThreadId,
    pub agent_name: String,
    pub steps: Vec<StepRun>,
    pub status: RunStatus,
    pub cost: Cost,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepRun {
    pub id: StepId,
    pub step_type: StepType,
    pub description: String,
    pub status: StepStatus,
    pub progress: f32, // 0.0-1.0
    pub result: Option<StepResult>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RunStatus {
    Queued,
    Running,
    WaitingForApproval,
    Success,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Cost {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
    pub usd_cents: u32, // Cost in cents
}
```

**Run lifecycle**:
```
Queued → Running → [WaitingForApproval] → Success/Failed/Cancelled
```

Each run contains multiple steps. Steps execute sequentially or in parallel (depending on the agent's workflow).

## Projections

Projections are **read-optimized views** derived from events. They trade off write complexity (apply events) for read simplicity (direct access).

### ChatView

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatView {
    pub thread_id: ThreadId,
    pub entries: Vec<ChatEntry>,
    pub streaming_message: Option<(MessageId, String)>,
    pub message_count: usize,
    pub last_updated: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChatEntry {
    Message(MessageView),
    ToolUse(ToolUseView),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageView {
    pub id: MessageId,
    pub content: String,
    pub role: Role,
    pub timestamp: DateTime<Utc>,
    pub tool_uses: Vec<ToolUseId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolUseView {
    pub id: ToolUseId,
    pub tool_name: String,
    pub input_summary: String,
    pub output_summary: Option<String>,
    pub status: ToolStatus,
    pub duration_ms: Option<u64>,
}

impl ChatView {
    pub fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            entries: Vec::new(),
            streaming_message: None,
            message_count: 0,
            last_updated: Utc::now(),
        }
    }

    /// Apply a domain event to update this view
    pub fn apply(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::MessageAdded { message_id, content, role, timestamp, .. } => {
                self.entries.push(ChatEntry::Message(MessageView {
                    id: *message_id,
                    content: content.clone(),
                    role: *role,
                    timestamp: *timestamp,
                    tool_uses: Vec::new(),
                }));
                self.message_count += 1;
                self.last_updated = *timestamp;
            }

            DomainEvent::MessageStreaming { message_id, content, timestamp } => {
                self.streaming_message = Some((*message_id, content.clone()));
                self.last_updated = *timestamp;
            }

            DomainEvent::MessageComplete { message_id, final_content, timestamp } => {
                self.streaming_message = None;
                // Update the message in entries
                for entry in &mut self.entries {
                    if let ChatEntry::Message(msg) = entry {
                        if msg.id == *message_id {
                            msg.content = final_content.clone();
                            msg.timestamp = *timestamp;
                            break;
                        }
                    }
                }
                self.last_updated = *timestamp;
            }

            DomainEvent::ToolUseStarted { tool_use_id, tool_name, input, timestamp, .. } => {
                let input_summary = summarize_json(input, 100);
                self.entries.push(ChatEntry::ToolUse(ToolUseView {
                    id: *tool_use_id,
                    tool_name: tool_name.clone(),
                    input_summary,
                    output_summary: None,
                    status: ToolStatus::Running,
                    duration_ms: None,
                }));
                self.last_updated = *timestamp;
            }

            DomainEvent::ToolUseComplete { tool_use_id, result, duration_ms, timestamp } => {
                for entry in &mut self.entries {
                    if let ChatEntry::ToolUse(tool) = entry {
                        if tool.id == *tool_use_id {
                            tool.output_summary = Some(summarize_output(result, 200));
                            tool.status = ToolStatus::Success;
                            tool.duration_ms = Some(*duration_ms);
                            break;
                        }
                    }
                }
                self.last_updated = *timestamp;
            }

            // Handle other events...
            _ => {}
        }
    }

    /// Get visible entries for virtual scrolling
    pub fn visible_entries(&self, start: usize, count: usize) -> &[ChatEntry] {
        let end = (start + count).min(self.entries.len());
        &self.entries[start..end]
    }
}
```

**Why ChatView?**
- **UI-Optimized**: Linear list of entries for virtual scrolling
- **Summarized**: Tool inputs/outputs are summarized for performance
- **Streaming State**: Separate field for in-progress message
- **Fast Reads**: Direct access without querying event log

### ThreadSummary

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: ThreadId,
    pub title: String,
    pub last_message_preview: String,
    pub last_updated: DateTime<Utc>,
    pub message_count: usize,
    pub unread_count: usize,
    pub archived: bool,
}

impl ThreadSummary {
    pub fn apply(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::MessageAdded { content, timestamp, .. } => {
                self.last_message_preview = truncate(content, 100);
                self.last_updated = *timestamp;
                self.message_count += 1;
                self.unread_count += 1;
            }

            DomainEvent::ThreadUpdated { title, archived, timestamp, .. } => {
                if let Some(new_title) = title {
                    self.title = new_title.clone();
                }
                if let Some(new_archived) = archived {
                    self.archived = *new_archived;
                }
                self.last_updated = *timestamp;
            }

            _ => {}
        }
    }
}
```

**Use Case**: Thread list view (inbox, sidebar). Shows preview and metadata without loading full chat.

## Event Patterns

### Command-Event Flow

1. **Command**: User intent ("Send message")
2. **Handler**: Validates and executes command
3. **Event**: Records what happened ("MessageAdded")
4. **Projection**: Updates read model ("ChatView")
5. **UI**: Re-renders with new state

```rust
// 1. Command
commands.dispatch(Command::SendMessage {
    thread_id,
    content: "Hello!".into(),
});

// 2. Handler
fn handle_send_message(cmd: &Command) {
    if let Command::SendMessage { thread_id, content } = cmd {
        // Validate
        if content.trim().is_empty() {
            return Err("Message cannot be empty");
        }

        // Generate event
        let event = DomainEvent::MessageAdded {
            thread_id: *thread_id,
            message_id: MessageId::new(),
            content: content.clone(),
            role: Role::User,
            tool_uses: SmallVec::new(),
            timestamp: Utc::now(),
        };

        // Emit event
        emit_event(event);
    }
}

// 3. Event handler
fn on_event(event: DomainEvent) {
    // Apply to all projections
    for view in chat_views.values_mut() {
        view.apply(&event);
    }

    for summary in thread_summaries.values_mut() {
        summary.apply(&event);
    }
}
```

### Streaming Pattern

Assistant messages stream in character-by-character:

```rust
// Start streaming
DomainEvent::MessageAdded {
    message_id,
    content: "".into(), // Empty initially
    streaming: true,
    ...
}

// Stream deltas
DomainEvent::MessageStreaming {
    message_id,
    content: "Hello".into(),
    ...
}

DomainEvent::MessageStreaming {
    message_id,
    content: "Hello, world".into(), // Cumulative
    ...
}

// Complete
DomainEvent::MessageComplete {
    message_id,
    final_content: "Hello, world!".into(),
    ...
}
```

**UI Behavior**:
- `MessageStreaming`: Show content with typing indicator
- `MessageComplete`: Remove typing indicator, finalize content

### Saga Pattern

Complex workflows span multiple events:

```rust
// User approves a step
Command::ApproveStep { run_id, step_id }

// Emits event
DomainEvent::RunStepUpdated {
    run_id,
    step_id,
    status: StepStatus::Running,
    ...
}

// Backend executes step
DomainEvent::RunStepComplete {
    run_id,
    step_id,
    result: StepResult::Success,
    ...
}

// Next step starts
DomainEvent::RunStepStarted {
    run_id,
    step_id: next_step_id,
    ...
}
```

**Correlation ID** ties these events together. The UI can filter events by correlation ID to show a single workflow's progress.

## Testing

Event sourcing makes testing straightforward:

```rust
#[test]
fn test_message_added() {
    let mut view = ChatView::new(thread_id);

    let event = DomainEvent::MessageAdded {
        thread_id,
        message_id: MessageId::new(),
        content: "Test message".into(),
        role: Role::User,
        tool_uses: SmallVec::new(),
        timestamp: Utc::now(),
    };

    view.apply(&event);

    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.message_count, 1);

    if let ChatEntry::Message(msg) = &view.entries[0] {
        assert_eq!(msg.content, "Test message");
        assert_eq!(msg.role, Role::User);
    } else {
        panic!("Expected message entry");
    }
}

#[test]
fn test_streaming_message() {
    let mut view = ChatView::new(thread_id);
    let message_id = MessageId::new();

    // Add empty message
    view.apply(&DomainEvent::MessageAdded {
        thread_id,
        message_id,
        content: "".into(),
        role: Role::Assistant,
        tool_uses: SmallVec::new(),
        timestamp: Utc::now(),
    });

    // Stream content
    view.apply(&DomainEvent::MessageStreaming {
        thread_id,
        message_id,
        content: "Hello".into(),
        timestamp: Utc::now(),
    });

    assert_eq!(view.streaming_message, Some((message_id, "Hello".into())));

    // Complete
    view.apply(&DomainEvent::MessageComplete {
        thread_id,
        message_id,
        final_content: "Hello, world!".into(),
        timestamp: Utc::now(),
    });

    assert_eq!(view.streaming_message, None);

    if let ChatEntry::Message(msg) = &view.entries[0] {
        assert_eq!(msg.content, "Hello, world!");
    }
}
```

## Snapshots

Replaying thousands of events is slow. **Snapshots** cache projection state:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub sequence: u64, // Last event sequence included
    pub timestamp: DateTime<Utc>,
    pub projection: ChatView,
}

impl Snapshot {
    pub fn restore(snapshot: Snapshot, events: &[DomainEvent]) -> ChatView {
        let mut view = snapshot.projection;

        // Apply events after snapshot
        for event in events {
            view.apply(event);
        }

        view
    }
}
```

**Strategy**:
- Take snapshot every N events (e.g., 1000)
- Restore from most recent snapshot + replay remaining events
- Trade off storage (snapshots) for speed (fewer events to replay)

## Summary

Event sourcing provides:

1. **Immutable History**: All changes are recorded
2. **Derived State**: Projections are computed from events
3. **Time Travel**: Replay to any point in history
4. **Testability**: Pure functions for event application
5. **Flexibility**: Multiple projections from same event stream

The domain model is the **source of truth**. All other layers derive their state from domain events.
