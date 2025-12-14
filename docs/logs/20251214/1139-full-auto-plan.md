# Full-Auto Mode Implementation Plan

## Overview

Implement autonomous task execution for OpenAgents with smart backend detection, task discovery, and real-time progress tracking.

**Two components:**
1. **`crates/auto/`** - New crate for full-auto orchestration
2. **OpenRouter provider** - Add to `crates/llm/` for OPENROUTER_API_KEY support

---

## Part 1: OpenRouter Provider (`crates/llm/`)

### Files to Create/Modify

| File | Action |
|------|--------|
| `crates/llm/src/provider/openrouter.rs` | CREATE - Main provider implementation |
| `crates/llm/src/provider/mod.rs` | MODIFY - Add module, export, registry |
| `crates/llm/src/model/mod.rs` | MODIFY - Add openrouter model definitions |
| `crates/mechacoder/src/router.rs` | MODIFY - Add Backend::OpenRouter |

### Implementation

**1. OpenRouterProvider struct:**
```rust
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,  // from OPENROUTER_API_KEY
    base_url: String, // https://openrouter.ai/api/v1
}
```

**2. Key differences from Anthropic:**
- Uses OpenAI-compatible chat completions format
- `Authorization: Bearer <key>` header
- Model IDs include provider prefix: `anthropic/claude-3.5-sonnet`
- SSE format uses OpenAI delta structure

**3. Add Backend::OpenRouter to mechacoder:**
```rust
// In router.rs detect()
if std::env::var("OPENROUTER_API_KEY").is_ok() {
    backends.push(Backend::OpenRouter);
}
```

---

## Part 2: Auto Mode Crate (`crates/auto/`)

### Crate Structure

```
crates/auto/
├── Cargo.toml
└── src/
    ├── lib.rs              # AutoMode, exports
    ├── config.rs           # AutoConfig, ExecutionMode
    ├── detection.rs        # Enhanced credential detection
    ├── discovery.rs        # Task/plan discovery
    ├── update.rs           # AutoUpdate enum for UI streaming
    ├── progress.rs         # ProgressTracker (updates taskmaster)
    └── engine/
        ├── mod.rs          # AutoEngine orchestrator
        ├── task_runner.rs  # Execute single task
        └── loop_controller.rs  # Batch/continuous modes
```

### Core Types

**ExecutionMode:**
```rust
pub enum ExecutionMode {
    Single,                    // One task, then stop
    Batch { count: usize },    // N tasks
    Continuous,                // Until stopped or empty
}
```

**TaskSource:**
```rust
pub enum TaskSource {
    Taskmaster { db_path: PathBuf },
    Plans { claude_dir: PathBuf },
    Explicit { task_ids: Vec<String> },
    Auto,  // Try taskmaster, fall back to plans
}
```

**AutoUpdate (streams to UI):**
```rust
pub enum AutoUpdate {
    // Initialization
    Initialized { detection, config },
    BackendSelected { backend, reason },

    // Discovery
    TasksDiscovered { count, source },
    NoTasksFound { reason },

    // Task execution
    TaskStarted { task_id, title, backend },
    TextDelta { task_id, delta },
    ToolStarted { task_id, tool_name, tool_call_id },
    ToolCompleted { task_id, tool_call_id, output, is_error },
    TaskCompleted { task_id, result },
    CommitCreated { task_id, sha, message },

    // Completion
    Finished { tasks_completed, tasks_failed },
}
```

### Task Lifecycle Flow

```
1. DETECTION
   - Scan env + .env.local for API keys
   - Check Claude CLI, Ollama TCP
   - Select best backend

2. DISCOVERY
   - taskmaster.pick_next() → ready tasks
   - Or scan ~/.claude/plans/

3. EXECUTION LOOP (per task)
   a. taskmaster.start(task_id)
      → status = in_progress
      → execution_state = Running

   b. TaskRunner.run(task)
      → Build prompt from task details
      → Run agentic loop (stream TextDelta, ToolStarted, etc.)
      → Track commits made

   c. On success:
      → taskmaster.add_commit() for each SHA
      → taskmaster.close(task_id)
      → execution_state = Succeeded

   d. On failure:
      → execution_state = Failed
      → Keep issue open for retry

4. COMPLETION
   → Emit Finished with stats
```

### ProgressTracker Integration

```rust
pub struct ProgressTracker {
    repo: Arc<dyn IssueRepository>,
}

impl ProgressTracker {
    pub fn task_started(&self, task_id: &str, agent_id: &str) {
        self.repo.start(task_id, Some(agent_id));
        // Update execution_state = Running
    }

    pub fn add_commit(&self, task_id: &str, sha: &str) {
        self.repo.add_commit(task_id, sha);
    }

    pub fn task_completed(&self, task_id: &str, success: bool, commits: Vec<String>) {
        if success {
            self.repo.close(task_id, None, commits, Some("auto"));
        }
        // Update execution_state = Succeeded/Failed
    }
}
```

### Main API

```rust
pub struct AutoMode {
    engine: AutoEngine,
    config: AutoConfig,
}

impl AutoMode {
    /// Auto-detect everything and run
    pub async fn auto() -> Result<Self>;

    /// Run with custom config
    pub fn with_config(config: AutoConfig) -> Self;

    /// Execute, returning stream of updates
    pub fn run(&self) -> impl Stream<Item = AutoUpdate>;

    /// Stop gracefully
    pub async fn stop(&self);
}
```

---

## Dependencies

```toml
# crates/auto/Cargo.toml
[dependencies]
taskmaster = { path = "../taskmaster" }
mechacoder = { path = "../mechacoder", features = ["server"] }
coder_service = { path = "../coder/service" }
llm = { path = "../llm" }
tool_registry = { path = "../tool_registry" }
tokio = { version = "1", features = ["sync", "time", "rt-multi-thread"] }
tokio-stream = "0.1"
futures = "0.3"
async-stream = "0.3"
serde = { version = "1.0", features = ["derive"] }
thiserror = "2.0"
tracing = "0.1"
chrono = "0.4"
dotenv = "0.15"  # For parsing .env.local
```

---

## Implementation Order

### Phase 1: OpenRouter Provider
1. Create `crates/llm/src/provider/openrouter.rs`
2. Add OpenAI-compatible request/response transforms
3. Implement streaming adapter for OpenAI SSE format
4. Add to ProviderRegistry in `mod.rs`
5. Add `Backend::OpenRouter` to mechacoder router
6. Test with OPENROUTER_API_KEY

### Phase 2: Auto Crate Core
1. Create `crates/auto/` structure
2. Implement `config.rs` (AutoConfig, ExecutionMode)
3. Implement `detection.rs` (scan env + .env.local)
4. Implement `discovery.rs` (taskmaster + plans)
5. Implement `update.rs` (AutoUpdate enum)

### Phase 3: Execution Engine
1. Implement `progress.rs` (ProgressTracker)
2. Implement `engine/task_runner.rs`
3. Implement `engine/loop_controller.rs`
4. Implement `engine/mod.rs` (AutoEngine)
5. Wire up in `lib.rs`

### Phase 4: Integration
1. Add CLI binary `crates/auto/src/bin/auto.rs`
2. Test single task execution
3. Test batch mode
4. Test continuous mode
5. Verify taskmaster updates (status, commits, execution times)

---

## Critical Files Reference

**Patterns to follow:**
- `crates/llm/src/provider/anthropic.rs` - Provider structure, streaming
- `crates/coder/service/src/update.rs` - Update enum pattern
- `crates/taskmaster/src/repository/trait.rs` - IssueRepository API

**Files to modify:**
- `crates/llm/src/provider/mod.rs` - Add openrouter
- `crates/mechacoder/src/router.rs` - Add Backend::OpenRouter
- `Cargo.toml` (workspace) - Add auto crate

**Taskmaster methods to use:**
- `pick_next()` - Get next ready task
- `start(task_id, agent_id)` - Mark in progress
- `add_commit(task_id, sha)` - Track commits
- `close(task_id, reason, commits, actor)` - Complete task
