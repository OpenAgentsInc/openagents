# Auto: Full-Auto Mode for OpenAgents

**Auto** provides autonomous task execution for OpenAgents. It smartly detects available AI backends, discovers tasks from taskmaster or Claude plans, and executes them automatically with real-time progress tracking.

## Features

- **Smart Backend Detection**: Automatically detects Claude Code CLI, API keys, OpenRouter, Ollama
- **Credential Discovery**: Reads from environment variables and `.env.local` files
- **Task Discovery**: Finds tasks from taskmaster database or Claude plan files
- **Multiple Execution Modes**: Single, batch, or continuous execution
- **Progress Tracking**: Updates taskmaster with status, commits, and completion
- **Real-time Streaming**: Stream updates for UI integration

## Quick Start

### CLI Usage

```bash
# Run a single task (auto-detect everything)
cargo run -p auto

# Run up to 5 tasks
cargo run -p auto -- --batch 5

# Run continuously until stopped
cargo run -p auto -- --continuous

# Run a specific task
cargo run -p auto -- --task tm-123

# Use a specific directory
cargo run -p auto -- --dir /path/to/project
```

### Programmatic Usage

```rust
use auto::{AutoMode, AutoConfig, ExecutionMode};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), auto::AutoError> {
    // Auto-detect everything
    let mut auto = AutoMode::auto().await?;

    // Stream updates
    let mut updates = std::pin::pin!(auto.run());
    while let Some(update) = updates.next().await {
        println!("{:?}", update);
        if update.is_terminal() {
            break;
        }
    }

    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        AutoMode                              │
│  Entry point, orchestrates detection → discovery → execution │
├─────────────────────────────────────────────────────────────┤
│                       AutoEngine                             │
│  Manages task lifecycle, progress tracking                   │
├──────────────────────┬──────────────────────────────────────┤
│      Detection       │           Discovery                   │
│  Backend/credential  │  Task source enumeration              │
│  scanning            │  (taskmaster, plans)                  │
├──────────────────────┼──────────────────────────────────────┤
│    ProgressTracker   │          TaskRunner                   │
│  Taskmaster updates  │  Executes tasks via backend           │
└──────────────────────┴──────────────────────────────────────┘
```

## Configuration

### AutoConfig

```rust
pub struct AutoConfig {
    /// Working directory for execution
    pub working_directory: PathBuf,

    /// Execution mode (Single, Batch, Continuous)
    pub execution_mode: ExecutionMode,

    /// Source of tasks (Taskmaster, Plans, Explicit, Auto)
    pub task_source: TaskSource,

    /// Preferred backend (optional override)
    pub preferred_backend: Option<Backend>,

    /// Path to .env.local file
    pub env_local_path: Option<PathBuf>,

    /// Maximum turns per task (prevents infinite loops)
    pub max_turns_per_task: usize,

    /// Whether to auto-commit changes
    pub auto_commit: bool,

    /// Commit message prefix
    pub commit_prefix: Option<String>,

    /// Whether to update taskmaster with progress
    pub update_taskmaster: bool,
}
```

### Execution Modes

| Mode | Description |
|------|-------------|
| `Single` | Execute one task, then stop (default) |
| `Batch { count }` | Execute up to N tasks, then stop |
| `Continuous` | Execute until stopped or no more tasks |

### Task Sources

| Source | Description |
|--------|-------------|
| `Taskmaster { db_path }` | Use taskmaster database |
| `Plans { claude_dir }` | Use Claude plan files |
| `Explicit { task_ids }` | Use specific task IDs |
| `Auto` | Try taskmaster, fall back to plans |

## Backend Detection

### Priority Order

1. **Claude Code CLI** - Most capable, uses full Claude CLI
2. **Anthropic API** - Direct API access (ANTHROPIC_API_KEY)
3. **OpenRouter** - Multi-model gateway (OPENROUTER_API_KEY)
4. **OpenAI** - OpenAI API (OPENAI_API_KEY)
5. **Ollama** - Local models (localhost:11434)
6. **Pi** - Built-in agent (always available)

### Credential Sources

1. **Environment Variables**: Direct env var lookup
2. **.env.local Files**: Parsed from working directory

```bash
# .env.local
OPENROUTER_API_KEY=sk-or-v1-...
ANTHROPIC_API_KEY=sk-ant-...
```

## Task Discovery

### From Taskmaster

```rust
// Finds ready tasks (open, no blocking dependencies)
let discovery = Discovery::discover(&config)?;
let tasks = discovery.tasks();
```

Ready tasks are:
- Status is `open`
- Not tombstoned
- No blocking dependencies on non-closed issues

### From Claude Plans

Scans `~/.claude/plans/` or `.claude/plans/` for `.md` files:

```
~/.claude/plans/
├── implement-auth.md
├── fix-bug-123.md
└── refactor-api.md
```

Each plan file becomes a task with:
- ID: filename (without extension)
- Title: First `# Heading` or filename
- Description: Full file content

## AutoUpdate Events

Updates are streamed for real-time UI integration:

```rust
pub enum AutoUpdate {
    // Initialization
    Initialized { backends_detected, selected_backend, working_directory },
    BackendSelected { backend, reason },

    // Discovery
    TasksDiscovered { count, source },
    NoTasksFound { reason },

    // Task Execution
    TaskStarted { task_id, title, index, total },
    TextDelta { task_id, delta },
    ReasoningDelta { task_id, delta },
    ToolStarted { task_id, tool_name, tool_call_id, input },
    ToolCompleted { task_id, tool_call_id, output, is_error },
    CommitCreated { task_id, sha, message },
    TaskCompleted { task_id, success, commits },

    // Completion
    Finished { tasks_completed, tasks_failed },
    Error { error },
    Cancelled { reason },
}
```

## Taskmaster Integration

When `update_taskmaster` is enabled (default), the ProgressTracker:

1. **On task start**: `repo.start(task_id, "auto")`
2. **On commit**: `repo.add_commit(task_id, sha)`
3. **On success**: `repo.close(task_id, reason, commits, "auto")`
4. **On failure**: Adds comment with error details

## CLI Reference

```
Full-Auto Mode - Autonomous task execution for OpenAgents

USAGE:
    auto [OPTIONS]

OPTIONS:
    -b, --batch <N>      Run up to N tasks
    -c, --continuous     Run until stopped or no more tasks
    -t, --task <ID>      Run a specific task
    -d, --dir <PATH>     Set working directory
    --no-commit          Don't auto-commit changes
    --no-taskmaster      Don't update taskmaster
    -h, --help           Show this help

ENVIRONMENT:
    ANTHROPIC_API_KEY    Use Anthropic API directly
    OPENROUTER_API_KEY   Use OpenRouter (can be in .env.local)
    OPENAI_API_KEY       Use OpenAI API

EXAMPLES:
    auto                          # Run single task
    auto --batch 5               # Run up to 5 tasks
    auto --continuous            # Run until stopped
    auto --task tm-123           # Run specific task
    auto --dir /path/to/project  # Use specific directory
```

## Integration with OpenAgents

Auto mode integrates with:

- **taskmaster**: Task management and progress tracking
- **mechacoder**: Backend routing and detection
- **llm**: Provider abstraction (Anthropic, OpenRouter)
- **coder_service**: Session management (future)

## Error Handling

```rust
pub enum AutoError {
    NoBackend,           // No AI backend available
    NoTasks(String),     // No tasks found
    Backend(String),     // Backend execution error
    Taskmaster(Error),   // Taskmaster database error
    Io(Error),           // File system error
    Config(String),      // Configuration error
    Session(String),     // Session management error
    Cancelled,           // User cancellation
}
```

## Future Enhancements

- [ ] Full API backend execution (not just Claude CLI)
- [ ] Parallel task execution
- [ ] Task dependency resolution
- [ ] Webhook notifications
- [ ] Cost tracking and limits
- [ ] Resume interrupted sessions
