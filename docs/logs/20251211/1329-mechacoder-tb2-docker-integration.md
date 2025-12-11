# MechaCoder TB2 Docker Integration

**Date:** 2024-12-11 13:29
**Status:** Complete

## Goal

Switch MechaCoder Gym screen to run Claude Code inside Docker containers matching the exact Terminal-Bench 2 environment, ready for official leaderboard submission via Harbor.

## Problem Statement

MechaCoder was stuck in the wrong working directory:
1. Used `std::env::current_dir()` which returned the openagents directory
2. Claude CLI path was broken (`~/.claude/local/claude` doesn't exist on Linux)
3. Tasks hardcoded `/app/regex.txt` but agent ran elsewhere
4. No Docker container setup to match TB2 environment

## Solution Architecture

```
MechaCoder Screen
       |
       v
TB2 Task Loader  ----> Load from ~/code/terminal-bench-2/
       |
       v
Docker Runner    ----> sandbox::DockerBackend (existing)
       |
       v
Verification     ----> Run tests/test.sh, parse reward.txt
       |
       v
ATIF Export      ----> Save trajectory to store
```

## Files Created

### 1. `crates/gym/src/mechacoder/tb2_loader.rs` (~280 lines)

Parses TB2 tasks from `~/code/terminal-bench-2/`:

```rust
pub struct TaskToml {
    pub version: String,
    pub metadata: TaskMetadata,
    pub verifier: VerifierConfig,
    pub agent: AgentConfig,
    pub environment: EnvironmentConfig,
}

pub struct TB2Task {
    pub id: String,
    pub name: String,
    pub instruction: String,
    pub config: TaskToml,
    pub task_dir: PathBuf,
    pub dockerfile_path: PathBuf,
    pub tests_dir: PathBuf,
}

pub struct TB2TaskLoader {
    pub tb2_root: PathBuf,
}
```

Key functions:
- `discover_tasks()` - Scans TB2 directory for task folders with task.toml
- `load_task(id)` - Loads full task including instruction.md
- `parse_task_toml()` - Parses task configuration

### 2. `crates/gym/src/mechacoder/docker_runner.rs` (~450 lines)

Executes Claude Code inside Docker containers:

```rust
pub struct DockerRunConfig {
    pub task: TB2Task,
    pub workspace_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub max_turns: u32,
    pub model: Option<String>,
}

pub enum DockerEvent {
    ContainerStarting { image: String },
    ContainerStarted { container_id: String },
    ClaudeOutput { line: String },
    ToolUse { tool_name: String, tool_id: String },
    AssistantMessage { text: String, turn: u32 },
    TurnComplete { turn: u32 },
    ContainerStopped { exit_code: i32 },
    Error { message: String },
}

pub struct DockerRunner {
    backend: DockerBackend,
}
```

Key functions:
- `run_claude()` - Runs Claude CLI in container, streams events
- `ensure_image()` - Pulls Docker image if needed
- `setup_directories()` - Creates /app, /logs, /tests mounts
- `build_claude_command()` - Builds Harbor-compatible command

Claude command (matching Harbor):
```bash
claude --verbose --output-format stream-json \
  -p "$INSTRUCTION" \
  --allowedTools Bash,Edit,Write,Read,Glob,Grep,LS,WebFetch,NotebookEdit,NotebookRead,TodoRead,TodoWrite,Agent \
  2>&1 | tee /logs/agent/claude-code.txt
```

### 3. `crates/gym/src/mechacoder/verifier.rs` (~200 lines)

Runs TB2 verification tests:

```rust
pub struct VerificationResult {
    pub passed: bool,
    pub tests_passed: u32,
    pub tests_total: u32,
    pub reward: f64,
    pub output: String,
    pub error: Option<String>,
}

pub struct TB2Verifier {
    backend: DockerBackend,
}
```

Key functions:
- `run_tests()` - Runs tests/test.sh in container
- `parse_reward()` - Reads /logs/verifier/reward.txt (1=pass, 0=fail)
- `parse_ctrf()` - Parses /logs/verifier/ctrf.json for test details

## Files Modified

### 4. `crates/gym/src/mechacoder/types.rs`

Added TB2 fields to MechaTask:

```rust
pub struct MechaTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub verification_cmd: Option<String>,
    // NEW: TB2-specific fields
    pub docker_image: Option<String>,
    pub timeout_sec: Option<u64>,
    pub task_dir: Option<PathBuf>,
    pub tests_dir: Option<PathBuf>,
    pub difficulty: Option<String>,
    pub category: Option<String>,
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<u32>,
}
```

Updated task loading:
```rust
pub mod tasks {
    pub fn load_tb2_tasks() -> Vec<MechaTask> {
        let loader = TB2TaskLoader::new_default();
        loader.discover_tasks()
            .iter()
            .filter_map(|s| loader.load_task(&s.id).ok().map(MechaTask::from))
            .collect()
    }
}
```

### 5. `crates/gym/src/mechacoder/mod.rs`

Added Docker-based execution path:

```rust
// In on_start():
if task.is_tb2_task() {
    Self::run_docker_task(task, max_turns, tx, store_clone, session_id, abort_rx).await;
} else {
    // Fall back to SDK-based approach
    Self::run_cc_query(prompt, tx, store_clone, session_id, abort_rx).await;
}
```

Added `run_docker_task()` method (~200 lines):
1. Creates temp workspace and logs directories
2. Converts MechaTask to TB2Task
3. Spawns DockerRunner
4. Forwards DockerEvents to UI as RunnerEvents
5. Runs verification after Claude completes
6. Reports pass/fail and saves ATIF trajectory

### 6. `crates/gym/src/mechacoder/task_panel.rs`

Added task selector dropdown:

```rust
pub struct SelectTask(pub MechaTask);

struct TaskPanel {
    available_tasks: Vec<MechaTask>,
    task_selector_expanded: bool,
}
```

Updated working directory display:
- Shows "/app" with "Docker: image-name" badge for TB2 tasks
- Shows host path with "Local" badge for non-TB2 tasks

### 7. `crates/gym/Cargo.toml`

Added dependencies:
```toml
sandbox = { path = "../sandbox" }
toml = "0.8"
tempfile = "3"
dirs = "5"
thiserror = "1"
```

## Container Setup

Directory structure inside container:
```
/app/                    # Working directory, agent produces solutions here
/logs/
  agent/
    sessions/            # Claude session JSONL files
    claude-code.txt      # Streamed output
  verifier/
    reward.txt           # "1" or "0"
    ctrf.json            # Detailed test results
/tests/
  test.sh                # Test runner script
  test_outputs.py        # Pytest tests
```

Environment variables:
- `ANTHROPIC_API_KEY` - from host environment
- `CLAUDE_CONFIG_DIR=/logs/agent/sessions`
- `FORCE_AUTO_BACKGROUND_TASKS=1`

## Data Flow

1. User selects task from dropdown (loaded from `~/code/terminal-bench-2/`)
2. Click Start -> create temp workspace + logs dirs
3. DockerRunner pulls/verifies TB2 docker_image from task.toml
4. Start container with mounts: workspace->/app, logs->/logs, tests->/tests
5. Run Claude CLI, stream JSON output to UI via mpsc channel
6. On Claude completion, VerificationRunner runs test.sh
7. Parse reward.txt, update UI with pass/fail
8. Save ATIF trajectory to store

## Testing

Compilation verified:
```bash
cargo check -p gym    # Compiles with warnings only
cargo check -p hud    # Full build succeeds
```

## Usage

1. Start Gym: `cargo run -p hud`
2. Open MechaCoder screen
3. Select a task from dropdown (e.g., "Regex Log [medium]")
4. Click Start
5. Watch Docker container start, Claude execute, and verification run
6. See pass/fail result

## Notes

- Uses existing `sandbox::DockerBackend` for container management
- Falls back to SDK-based execution for non-TB2 tasks
- TB2 tasks automatically detected by presence of `docker_image` field
- All TB2 tasks from `~/code/terminal-bench-2/` loaded dynamically
- Verifier uses same container image as agent for consistency

## Related Files

- Plan: `/home/christopherdavid/.claude/plans/keen-yawning-blanket.md`
- Harbor reference: `~/code/harbor/src/harbor/agents/installed/claude_code.py`
- TB2 tasks: `~/code/terminal-bench-2/*/`
