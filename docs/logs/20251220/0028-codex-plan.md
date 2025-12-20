# Codex Integration for Autopilot

## Overview

Add support for OpenAI's Codex CLI as a first-class agent in Autopilot, enabling bi-directional delegation between Claude and Codex.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Autopilot                             │
│  ┌─────────────────────┐     ┌─────────────────────┐        │
│  │  claude-agent-sdk   │ ←→  │  codex-agent-sdk    │        │
│  │  (existing)         │     │  (new crate)        │        │
│  └─────────┬───────────┘     └─────────┬───────────┘        │
│            │                           │                     │
│            ▼                           ▼                     │
│   claude --output-format         codex exec --json          │
│       stream-json                                            │
└─────────────────────────────────────────────────────────────┘
```

Both agents can delegate to each other:
- Autopilot can spawn either as the primary agent
- Claude can invoke Codex via a skill/subagent
- Codex can invoke Claude via MCP or shell

## Key Files

| Purpose | Path |
|---------|------|
| Codex event types (Rust) | `/Users/christopherdavid/code/codex/codex-rs/exec/src/exec_events.rs` |
| TypeScript SDK reference | `/Users/christopherdavid/code/codex/sdk/typescript/src/` |
| Claude Agent SDK | `crates/claude-agent-sdk/src/` |
| Autopilot | `crates/autopilot/src/` |

---

## Implementation Plan

### Phase 1: Core Codex Agent SDK Crate

Create `crates/codex-agent-sdk/` mirroring the TypeScript SDK.

### Phase 2: Autopilot Integration

Add Codex as an agent backend option in Autopilot.

### Phase 3: Bi-directional Delegation

Enable Claude ↔ Codex delegation via skills/MCP.

---

## Detailed Issues

### Issue 1: Create codex-agent-sdk crate scaffold

**Priority:** P0
**Estimate:** Small

Create the basic crate structure for `crates/codex-agent-sdk`.

**Files to create:**
- `crates/codex-agent-sdk/Cargo.toml`
- `crates/codex-agent-sdk/src/lib.rs`

**Cargo.toml contents:**
```toml
[package]
name = "codex-agent-sdk"
version = "0.1.0"
edition = "2024"
description = "Rust SDK for Codex CLI"
license = "MIT"

[dependencies]
tokio = { version = "1", features = ["process", "io-util", "sync", "rt-multi-thread", "macros"] }
futures = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
which = "7"
```

**Acceptance criteria:**
- [ ] Crate compiles with `cargo build -p codex-agent-sdk`
- [ ] Added to workspace `Cargo.toml`
- [ ] Basic lib.rs with module declarations

---

### Issue 2: Port ThreadEvent and event types

**Priority:** P0
**Estimate:** Small
**Depends on:** Issue 1

Port the event types from `codex-rs/exec/src/exec_events.rs`.

**File to create:** `crates/codex-agent-sdk/src/events.rs`

**Types to port:**
```rust
pub enum ThreadEvent {
    ThreadStarted(ThreadStartedEvent),
    TurnStarted(TurnStartedEvent),
    TurnCompleted(TurnCompletedEvent),
    TurnFailed(TurnFailedEvent),
    ItemStarted(ItemStartedEvent),
    ItemUpdated(ItemUpdatedEvent),
    ItemCompleted(ItemCompletedEvent),
    Error(ThreadErrorEvent),
}

pub struct ThreadStartedEvent { pub thread_id: String }
pub struct TurnStartedEvent {}
pub struct TurnCompletedEvent { pub usage: Usage }
pub struct TurnFailedEvent { pub error: ThreadErrorEvent }
pub struct ItemStartedEvent { pub item: ThreadItem }
pub struct ItemUpdatedEvent { pub item: ThreadItem }
pub struct ItemCompletedEvent { pub item: ThreadItem }
pub struct ThreadErrorEvent { pub message: String }
pub struct Usage { pub input_tokens: i64, pub cached_input_tokens: i64, pub output_tokens: i64 }
```

**Acceptance criteria:**
- [ ] All event types with serde attributes for JSONL parsing
- [ ] `#[serde(tag = "type")]` for ThreadEvent
- [ ] Unit tests for deserializing sample JSONL

---

### Issue 3: Port ThreadItem and item detail types

**Priority:** P0
**Estimate:** Small
**Depends on:** Issue 2

Port the thread item types from `exec_events.rs`.

**File to create:** `crates/codex-agent-sdk/src/items.rs`

**Types to port:**
```rust
pub struct ThreadItem {
    pub id: String,
    pub details: ThreadItemDetails,
}

pub enum ThreadItemDetails {
    AgentMessage(AgentMessageItem),
    Reasoning(ReasoningItem),
    CommandExecution(CommandExecutionItem),
    FileChange(FileChangeItem),
    McpToolCall(McpToolCallItem),
    WebSearch(WebSearchItem),
    TodoList(TodoListItem),
    Error(ErrorItem),
}

// Plus all the item structs and status enums
```

**Acceptance criteria:**
- [ ] All item types with correct serde attributes
- [ ] Status enums: `CommandExecutionStatus`, `PatchApplyStatus`, `McpToolCallStatus`
- [ ] Helper structs: `FileUpdateChange`, `TodoItem`, etc.
- [ ] Unit tests for deserializing sample items

---

### Issue 4: Define CodexOptions and ThreadOptions

**Priority:** P0
**Estimate:** Small
**Depends on:** Issue 1

Create the configuration types.

**File to create:** `crates/codex-agent-sdk/src/options.rs`

**Types to define:**
```rust
pub struct CodexOptions {
    pub codex_path_override: Option<PathBuf>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

pub struct ThreadOptions {
    pub model: Option<String>,
    pub sandbox_mode: Option<SandboxMode>,
    pub working_directory: Option<PathBuf>,
    pub skip_git_repo_check: bool,
    pub model_reasoning_effort: Option<ModelReasoningEffort>,
    pub network_access_enabled: Option<bool>,
    pub web_search_enabled: Option<bool>,
    pub approval_policy: Option<ApprovalMode>,
    pub additional_directories: Vec<PathBuf>,
}

pub struct TurnOptions {
    pub output_schema: Option<serde_json::Value>,
}

pub enum SandboxMode { ReadOnly, WorkspaceWrite, DangerFullAccess }
pub enum ApprovalMode { Never, OnRequest, OnFailure, Untrusted }
pub enum ModelReasoningEffort { Minimal, Low, Medium, High, Xhigh }
```

**Acceptance criteria:**
- [ ] All option structs with `Default` implementations
- [ ] Builder pattern methods on `ThreadOptions`
- [ ] Serde attributes for CLI arg generation

---

### Issue 5: Define error types

**Priority:** P0
**Estimate:** Small
**Depends on:** Issue 1

Create the error module.

**File to create:** `crates/codex-agent-sdk/src/error.rs`

**Types to define:**
```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to spawn codex process: {0}")]
    SpawnFailed(#[from] std::io::Error),

    #[error("codex executable not found: {0}")]
    ExecutableNotFound(String),

    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("process exited unexpectedly: {0:?}")]
    ProcessExited(Option<i32>),

    #[error("turn failed: {0}")]
    TurnFailed(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

**Acceptance criteria:**
- [ ] thiserror derive for all variants
- [ ] From impls for common error types
- [ ] Result type alias

---

### Issue 6: Implement ProcessTransport for Codex CLI

**Priority:** P0
**Estimate:** Medium
**Depends on:** Issues 2, 5

Implement the process spawning and JSONL communication layer.

**Files to create:**
- `crates/codex-agent-sdk/src/transport/mod.rs`
- `crates/codex-agent-sdk/src/transport/process.rs`

**Key implementation:**
```rust
pub struct ProcessTransport {
    child: Child,
    stdin: ChildStdin,
    stdout_rx: mpsc::Receiver<Result<ThreadEvent>>,
}

impl ProcessTransport {
    pub async fn spawn(
        executable: PathBuf,
        args: Vec<String>,
        cwd: Option<PathBuf>,
        env: Option<HashMap<String, String>>,
    ) -> Result<Self>;

    pub async fn recv(&mut self) -> Option<Result<ThreadEvent>>;

    pub async fn kill(&mut self) -> Result<()>;
}

fn find_codex_executable() -> Result<PathBuf>;
```

**Implementation notes:**
- Spawn `codex exec --experimental-json <prompt>`
- Background task reads stdout line-by-line, parses JSONL
- Handle process exit and cleanup

**Acceptance criteria:**
- [ ] Spawns codex process with correct args
- [ ] Parses JSONL from stdout into ThreadEvent
- [ ] Handles process termination gracefully
- [ ] find_codex_executable checks PATH and common locations

---

### Issue 7: Implement Thread struct with run/runStreamed

**Priority:** P0
**Estimate:** Medium
**Depends on:** Issues 3, 4, 6

Implement the main Thread API.

**File to create:** `crates/codex-agent-sdk/src/thread.rs`

**Types and methods:**
```rust
pub enum Input {
    Text(String),
    Structured(Vec<UserInput>),
}

pub enum UserInput {
    Text { text: String },
    LocalImage { path: PathBuf },
}

pub struct Turn {
    pub items: Vec<ThreadItem>,
    pub final_response: String,
    pub usage: Option<Usage>,
}

pub struct StreamedTurn {
    inner: Pin<Box<dyn Stream<Item = Result<ThreadEvent>> + Send>>,
}

pub struct Thread {
    options: CodexOptions,
    thread_options: ThreadOptions,
    id: Option<String>,
}

impl Thread {
    pub fn id(&self) -> Option<&str>;

    pub async fn run(&mut self, input: impl Into<Input>, options: TurnOptions) -> Result<Turn>;

    pub async fn run_streamed(&mut self, input: impl Into<Input>, options: TurnOptions)
        -> Result<StreamedTurn>;
}

impl Stream for StreamedTurn {
    type Item = Result<ThreadEvent>;
}
```

**Implementation notes:**
- `run()` buffers all events and returns final Turn
- `run_streamed()` returns async stream of events
- Handle output_schema by writing temp file
- Track thread_id from ThreadStarted event

**Acceptance criteria:**
- [ ] run() returns Turn with items and final_response
- [ ] run_streamed() yields ThreadEvent stream
- [ ] Thread ID captured from first turn
- [ ] Output schema file creation/cleanup

---

### Issue 8: Implement Codex entry point struct

**Priority:** P0
**Estimate:** Small
**Depends on:** Issue 7

Create the main Codex struct as SDK entry point.

**Update:** `crates/codex-agent-sdk/src/lib.rs`

**API:**
```rust
pub struct Codex {
    options: CodexOptions,
}

impl Codex {
    pub fn new() -> Self;
    pub fn with_options(options: CodexOptions) -> Self;
    pub fn start_thread(&self, options: ThreadOptions) -> Thread;
    pub fn resume_thread(&self, id: &str, options: ThreadOptions) -> Thread;
}

// Convenience function
pub fn thread(options: ThreadOptions) -> Thread;
```

**Acceptance criteria:**
- [ ] Codex::new() with defaults
- [ ] start_thread creates new Thread
- [ ] resume_thread sets thread ID for continuation
- [ ] All types re-exported from lib.rs

---

### Issue 9: Add integration test with real Codex CLI

**Priority:** P1
**Estimate:** Small
**Depends on:** Issue 8

Add integration tests that run against the real Codex CLI.

**File to create:** `crates/codex-agent-sdk/tests/integration.rs`

**Tests:**
```rust
#[tokio::test]
#[ignore] // Requires codex installed and API key
async fn test_simple_query() {
    let codex = Codex::new();
    let mut thread = codex.start_thread(ThreadOptions {
        sandbox_mode: Some(SandboxMode::ReadOnly),
        ..Default::default()
    });

    let turn = thread.run("What is 2 + 2?", TurnOptions::default()).await.unwrap();
    assert!(!turn.final_response.is_empty());
}

#[tokio::test]
#[ignore]
async fn test_streaming() {
    let codex = Codex::new();
    let mut thread = codex.start_thread(ThreadOptions::default());

    let streamed = thread.run_streamed("Say hello", TurnOptions::default()).await.unwrap();
    let events: Vec<_> = streamed.collect().await;

    assert!(events.iter().any(|e| matches!(e, Ok(ThreadEvent::TurnCompleted(_)))));
}
```

**Acceptance criteria:**
- [ ] Tests pass with `cargo test -p codex-agent-sdk -- --ignored`
- [ ] Clear skip message when codex not installed
- [ ] Test both run() and run_streamed()

---

### Issue 10: Add Codex agent backend to Autopilot

**Priority:** P1
**Estimate:** Medium
**Depends on:** Issue 8

Integrate codex-agent-sdk into Autopilot as an alternative backend.

**Files to modify:**
- `crates/autopilot/Cargo.toml` - add codex-agent-sdk dependency
- `crates/autopilot/src/main.rs` - add --agent flag

**Changes:**
```rust
// New CLI flag
#[arg(long, default_value = "claude")]
agent: String,  // "claude" or "codex"

// Agent dispatch
match args.agent.as_str() {
    "claude" => run_claude_agent(&prompt, &options).await,
    "codex" => run_codex_agent(&prompt, &options).await,
    _ => bail!("Unknown agent: {}", args.agent),
}
```

**Acceptance criteria:**
- [ ] `cargo autopilot --agent codex "prompt"` works
- [ ] Codex events logged via TrajectoryCollector
- [ ] Token usage tracked similarly to Claude

---

### Issue 11: Create Codex TrajectoryCollector adapter

**Priority:** P1
**Estimate:** Small
**Depends on:** Issue 10

Adapt TrajectoryCollector to handle Codex events.

**File to modify:** `crates/autopilot/src/lib.rs`

**Changes:**
- Add `CodexEvent` variant to trajectory types or
- Create adapter that maps Codex events to existing format

```rust
impl From<codex_agent_sdk::ThreadEvent> for TrajectoryEvent {
    fn from(event: ThreadEvent) -> Self {
        match event {
            ThreadEvent::ItemCompleted(item) => {
                match item.item.details {
                    ThreadItemDetails::AgentMessage(msg) =>
                        TrajectoryEvent::AssistantMessage(msg.text),
                    ThreadItemDetails::CommandExecution(cmd) =>
                        TrajectoryEvent::ToolCall { name: "Bash", ... },
                    // etc
                }
            }
            ThreadEvent::TurnCompleted(tc) =>
                TrajectoryEvent::Usage(tc.usage.into()),
            // etc
        }
    }
}
```

**Acceptance criteria:**
- [ ] Codex runs generate .rlog files
- [ ] Token usage captured in trajectory
- [ ] Tool calls (commands, file changes) logged

---

### Issue 12: Create Claude skill for Codex delegation

**Priority:** P2
**Estimate:** Small
**Depends on:** Issue 8

Create a skill that allows Claude to delegate to Codex.

**File to create:** `.claude/skills/codex/SKILL.md`

**Contents:**
```yaml
---
description: Delegate complex code analysis or generation tasks to Codex (OpenAI's coding agent)
allowed_tools:
  - Bash
---

# Codex Delegation Skill

When the user's task would benefit from Codex's capabilities (e.g., complex refactoring,
code generation with a different style), you can delegate to Codex.

## Usage

Run Codex with a focused prompt:

```bash
codex exec --sandbox workspace-write "Your prompt here"
```

## When to Use

- Complex multi-file refactoring
- Code generation in unfamiliar languages
- When user explicitly requests Codex
- Performance-critical code optimization
```

**Acceptance criteria:**
- [ ] Skill discovered by Claude when setting_sources includes project
- [ ] Claude can invoke `codex exec` via Bash tool
- [ ] Results flow back to Claude's context

---

### Issue 13: Create MCP server for Claude invocation from Codex

**Priority:** P2
**Estimate:** Medium
**Depends on:** Issue 8

Create an MCP server that Codex can use to invoke Claude.

**Files to create:**
- `crates/claude-mcp/Cargo.toml`
- `crates/claude-mcp/src/main.rs`

**MCP Tools to expose:**
```json
{
  "tools": [
    {
      "name": "claude_query",
      "description": "Run a query with Claude Code",
      "inputSchema": {
        "type": "object",
        "properties": {
          "prompt": { "type": "string" },
          "model": { "type": "string" },
          "max_turns": { "type": "integer" }
        },
        "required": ["prompt"]
      }
    }
  ]
}
```

**Implementation:**
- JSON-RPC 2.0 over stdio
- Tool invokes claude-agent-sdk
- Streams results back as tool response

**Acceptance criteria:**
- [ ] MCP server runs with `cargo run -p claude-mcp`
- [ ] Codex can call `claude_query` tool
- [ ] Response includes Claude's final answer

---

### Issue 14: Add agent selection to issue metadata

**Priority:** P2
**Estimate:** Small
**Depends on:** Issue 10

Allow issues to specify which agent should work on them.

**File to modify:** `crates/issues/src/db.rs`

**Changes:**
- Add `agent` column to issues table (default: "claude")
- Update issue creation to accept agent parameter
- Update `issue_ready` to filter by agent or return agent preference

```sql
ALTER TABLE issues ADD COLUMN agent TEXT DEFAULT 'claude';
```

```rust
pub struct Issue {
    // existing fields...
    pub agent: String,  // "claude" or "codex"
}
```

**Acceptance criteria:**
- [ ] `cargo autopilot issue create --agent codex "..."` works
- [ ] Issues display agent in list view
- [ ] `--agent` flag on autopilot respects issue preference

---

### Issue 15: Documentation and examples

**Priority:** P2
**Estimate:** Small
**Depends on:** Issue 8

Add documentation for the Codex Agent SDK.

**Files to create:**
- `crates/codex-agent-sdk/README.md`
- `crates/codex-agent-sdk/examples/simple.rs`
- `crates/codex-agent-sdk/examples/streaming.rs`

**README sections:**
- Quick start
- API overview
- Configuration options
- Comparison with Claude Agent SDK
- Bi-directional delegation patterns

**Acceptance criteria:**
- [ ] README with usage examples
- [ ] Working example binaries
- [ ] Doc comments on all public types

---

## Implementation Order

```
Phase 1 - Core SDK (Issues 1-8)
├── Issue 1: Crate scaffold
├── Issue 2: Event types
├── Issue 3: Item types
├── Issue 4: Options
├── Issue 5: Error types
├── Issue 6: ProcessTransport
├── Issue 7: Thread API
└── Issue 8: Codex entry point

Phase 2 - Autopilot Integration (Issues 9-11)
├── Issue 9: Integration tests
├── Issue 10: Autopilot backend
└── Issue 11: Trajectory adapter

Phase 3 - Bi-directional Delegation (Issues 12-14)
├── Issue 12: Claude skill for Codex
├── Issue 13: MCP server for Claude
└── Issue 14: Issue agent metadata

Phase 4 - Polish (Issue 15)
└── Issue 15: Documentation
```

## Summary

15 modular issues total:
- **P0 (Core):** 8 issues - Must have for basic functionality
- **P1 (Integration):** 3 issues - Autopilot integration
- **P2 (Delegation):** 4 issues - Bi-directional delegation + docs
