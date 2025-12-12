# MechaCoder Gym Panel Migration

**Date:** 2025-12-11 21:12
**Status:** Phases 1-3, 5 Complete | Phase 4 Pending

## Overview

Migrated Terminal-Bench/HillClimber functionality from Commander's Gym pane to the MechaCoder binary with a collapsible panel-based UI. Created a new shared `terminalbench` crate to avoid code duplication.

## Design Decisions

- **Stream output**: Main chat timeline (TB2 events interleaved with chat, visual distinction)
- **Panel position**: Right side, 320px fixed width, toggle with Cmd+G / Ctrl+G
- **Concurrency**: Sequential only, one stream at a time
- **Aesthetic**: Bloomberg Terminal - white on black, Berkeley Mono, no emojis

## Architecture

```
+--------------------------------------------------+
|  MechaCoder                               [Gym]  |
+--------------------------------------------------+
|                                        | GymPanel|
|  Chat Thread (primary)                 | --------|
|  - Claude SDK streaming                | Task:   |
|  - TB2 events (visual distinction)     | [drop]  |
|  - Tool calls                          | --------|
|                                        | [TB2]   |
|  [User message input]                  | [TG]    |
|  ------------------------------------  | --------|
|  Status: Ready                         | Runs:   |
|                                        | ...     |
+--------------------------------------------------+
```

## Implementation Summary

### Phase 1: Create `crates/terminalbench` (COMPLETED)

Created a new shared crate for Terminal-Bench types and services.

**Files created:**
- `crates/terminalbench/Cargo.toml`
- `crates/terminalbench/src/lib.rs`
- `crates/terminalbench/src/types.rs`
- `crates/terminalbench/src/task_loader.rs`
- `crates/terminalbench/src/run_store.rs`

**Types extracted:**
```rust
// Core types (from gym/tbcc/types.rs)
pub struct TBTask { id, name, description, difficulty, timeout_ms, max_turns, tags }
pub enum TBDifficulty { Easy, Medium, Hard, Expert, Unknown }
pub struct TBRunSummary { id, task_id, task_name, status, outcome, ... }
pub enum TBRunStatus { Queued, Running, Completed, Error }
pub enum TBRunOutcome { Success, Failure, Timeout, Error, Aborted }
pub enum TBModelOption { ClaudeSonnet, ClaudeHaiku, Gpt4o, Gpt4oMini, AppleFM }
pub struct ExecutionSettings { model, max_attempts, timeout_ms, max_tokens, save_trajectories }
pub struct TBRunOptions { task, model, timeout_secs, max_turns }
pub struct DashboardStats { success_rate, last_50_success_rate, avg_steps, ... }
pub struct DifficultyStats { easy, medium, hard, expert }
pub struct DifficultyCount { passed, total }

// Helper functions
pub fn format_duration(ms: Option<u64>) -> String
pub fn format_percent(value: f32) -> String
```

**Services extracted:**
- `TaskLoader` - Loads TBTask from JSON suite files
- `RunStore` - Persists run history to disk (tb_runs.json)

**Re-exports from harbor:**
- `StreamEvent` - Real-time streaming events from tbench CLI
- `Trajectory`, `Agent`, `Step`, `StepSource`, `TBenchMetrics`

**Dependencies:**
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
harbor = { path = "../harbor" }
directories = "5"
```

### Phase 2: Add Panel System to MechaCoder (COMPLETED)

Added collapsible right-side panel infrastructure.

**Files modified:**
- `crates/mechacoder/src/actions.rs` - Added `ToggleGymPanel` action
- `crates/mechacoder/src/main.rs` - Added keybindings:
  ```rust
  KeyBinding::new("cmd-g", ToggleGymPanel, None),
  KeyBinding::new("ctrl-g", ToggleGymPanel, None), // Linux
  ```
- `crates/mechacoder/src/screen.rs` - Added panel layout:
  - `gym_panel: Entity<GymPanel>` field
  - `gym_panel_visible: bool` field
  - `toggle_gym_panel()` action handler
  - Render with flex row layout, 320px right panel when visible
- `crates/mechacoder/src/lib.rs` - Export `panels` module
- `crates/mechacoder/Cargo.toml` - Added dependencies:
  ```toml
  harbor = { path = "../harbor" }
  terminalbench = { path = "../terminalbench" }
  ```

### Phase 3: Implement GymPanel Component (COMPLETED)

Created the Gym panel UI with Bloomberg Terminal aesthetic.

**Files created:**
- `crates/mechacoder/src/panels/mod.rs`
- `crates/mechacoder/src/panels/gym_panel.rs`

**GymPanel structure:**
```rust
pub struct GymPanel {
    focus_handle: FocusHandle,
    task_loader: TaskLoader,
    tasks: Vec<TBTask>,
    selected_task_idx: Option<usize>,
    recent_runs: Vec<TBRunSummary>,
    active_run: Option<ActiveRunState>,
    selected_model: TBModelOption,
}
```

**UI Layout (Bloomberg style):**
```
+---------------------------+
| GYM               [close] |
+---------------------------+
| TASK                      |
| [dropdown: regex-log   v] |
+---------------------------+
| MODEL                     |
| [dropdown: Sonnet 4    v] |
+---------------------------+
| ACTIONS                   |
| [Run TB2]  [TestGen]      |
+---------------------------+
| ACTIVE                    |
| regex-log - Turn 3/30     |
| [####------] 30%          |
+---------------------------+
| RECENT                    |
| o regex-log  PASS  2m ago |
| x filter-js  FAIL  5m ago |
| o kv-store   PASS  1h ago |
+---------------------------+
```

**Visual symbols (no emojis):**
- `o` = success (green)
- `x` = failure (red)
- `t` = timeout (yellow)
- `!` = error (red)
- `-` = aborted (muted)
- `*` = running (secondary)

**Features implemented:**
- Task selector dropdown (click to cycle)
- Model selector dropdown (click to cycle through options)
- Action buttons (Run TB2, TestGen) - placeholders, log on click
- Active run progress bar with ASCII art `[####------]`
- Recent runs list with status symbols

### Phase 4: Integrate tbench Streaming (PENDING)

This phase would enable actual TB2 run execution with streaming output.

**Would require:**
1. Create `crates/mechacoder/src/panels/tbench_runner.rs`:
   ```rust
   pub struct TBenchRunner {
       project_root: PathBuf,
   }

   impl TBenchRunner {
       pub fn start_run(&self, task: &TBTask, options: &TBRunOptions) -> Task<()> {
           // 1. Spawn: tbench --instruction "..." --stream --output-dir /tmp/...
           // 2. Read stdout lines, parse as StreamEvent JSON
           // 3. Send updates via mpsc channel to GPUI
       }
   }
   ```

2. Extend `crates/mechacoder/src/sdk_thread.rs`:
   ```rust
   pub enum ThreadEntry {
       UserMessage(UserMessage),
       AssistantMessage(AssistantMessage),
       ToolUse(ToolUse),
       // New:
       TBenchRun(TBenchRunEntry),
       TBenchEvent(TBenchStreamEntry),
   }
   ```

3. Update `crates/mechacoder/src/ui/thread_view.rs` to render TBench entries

### Phase 5: Update gym to use terminalbench (COMPLETED)

Updated gym crate to depend on terminalbench and re-export types.

**Files modified:**
- `crates/gym/Cargo.toml` - Added dependency:
  ```toml
  terminalbench = { path = "../terminalbench" }
  ```
- `crates/gym/src/tbcc/types.rs` - Re-export from terminalbench:
  ```rust
  pub use terminalbench::{
      TBTask, TBDifficulty, TBRunSummary, TBRunStatus, TBRunOutcome,
      TBModelOption, ExecutionSettings, DashboardStats, DifficultyStats,
      DifficultyCount, format_duration, format_percent,
  };

  // Gym-specific types kept here:
  pub fn difficulty_color(diff: TBDifficulty) -> &'static str
  pub struct CurrentRunInfo { ... }
  pub enum TBCCTab { Dashboard, Tasks, Runs, Settings }
  pub struct ContainerSettings { ... }
  ```

**Files modified:**
- `Cargo.toml` (workspace) - Added `"crates/terminalbench"` to members

## File Changes Summary

### New Files (8)
| File | Lines | Purpose |
|------|-------|---------|
| `crates/terminalbench/Cargo.toml` | 12 | New shared crate |
| `crates/terminalbench/src/lib.rs` | 30 | Crate root, re-exports |
| `crates/terminalbench/src/types.rs` | 220 | Core TB types |
| `crates/terminalbench/src/task_loader.rs` | 200 | Task loading from JSON |
| `crates/terminalbench/src/run_store.rs` | 280 | Run history persistence |
| `crates/mechacoder/src/panels/mod.rs` | 8 | Panel system module |
| `crates/mechacoder/src/panels/gym_panel.rs` | 400 | Gym panel component |
| `docs/logs/20251211/2112-mechacoder-gym-panel-migration.md` | this | Documentation |

### Modified Files (8)
| File | Changes |
|------|---------|
| `Cargo.toml` | Added terminalbench to workspace members |
| `crates/mechacoder/Cargo.toml` | Added terminalbench, harbor deps |
| `crates/mechacoder/src/lib.rs` | Export panels module |
| `crates/mechacoder/src/main.rs` | Added Cmd+G / Ctrl+G keybindings |
| `crates/mechacoder/src/actions.rs` | Added ToggleGymPanel action |
| `crates/mechacoder/src/screen.rs` | Added panel layout, gym_panel field |
| `crates/gym/Cargo.toml` | Added terminalbench dependency |
| `crates/gym/src/tbcc/types.rs` | Re-exports from terminalbench |

## Usage

```bash
# Build mechacoder
cargo build -p mechacoder --release

# Run
./target/release/MechaCoder

# Toggle Gym panel
Cmd+G (macOS) or Ctrl+G (Linux)
```

## Testing

```bash
# Check all affected crates compile
cargo check -p terminalbench
cargo check -p mechacoder
cargo check -p gym

# Run terminalbench tests
cargo test -p terminalbench

# Build release binary
cargo build -p mechacoder --release
```

## Next Steps

1. **Complete Phase 4**: Implement tbench streaming integration
   - Wire up Run TB2 button to spawn `tbench --stream`
   - Parse StreamEvent JSON from stdout
   - Display events in main chat timeline

2. **Add TestGen integration**: Wire up TestGen button similarly

3. **Persist settings**: Save selected task/model preferences

4. **Add keyboard shortcuts**:
   - Up/Down arrows to navigate tasks in panel
   - Enter to start run

## Dependencies Graph

```
terminalbench
├── harbor (StreamEvent, ATIF types)
├── serde
├── chrono
├── uuid
└── directories

mechacoder
├── terminalbench
├── harbor
├── gpui
├── claude_agent_sdk
└── ... (existing deps)

gym
├── terminalbench (NEW)
├── testgen
├── hillclimber
└── ... (existing deps)
```

## Verification

Build completed successfully:
```
cargo build -p mechacoder --release
Finished `release` profile [optimized + debuginfo] target(s) in 2m 19s
```

All crates compile with only warnings (no errors).
