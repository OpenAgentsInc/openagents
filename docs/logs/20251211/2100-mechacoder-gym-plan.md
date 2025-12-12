# MechaCoder Gym Panel Migration

Migrate Terminal-Bench/HillClimber from Commander's Gym pane to mechacoder binary with collapsible panel UI.

## Design Decisions

- **Stream output**: Main chat timeline (TB2 events interleaved with chat, visual distinction)
- **Panel position**: Right side, 320px fixed width, toggle with Cmd+G
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

## Implementation Phases

### Phase 1: Create `crates/terminalbench`

Extract shared types from gym to new crate.

**Create files:**
- `crates/terminalbench/Cargo.toml`
- `crates/terminalbench/src/lib.rs`
- `crates/terminalbench/src/types.rs`
- `crates/terminalbench/src/task_loader.rs`
- `crates/terminalbench/src/run_store.rs`

**Extract from `crates/gym/src/tbcc/types.rs`:**
```rust
pub struct TBTask { id, name, description, difficulty, timeout_ms, max_turns, tags }
pub enum TBDifficulty { Easy, Medium, Hard, Expert, Unknown }
pub struct TBRunSummary { id, task_id, task_name, status, outcome, ... }
pub enum TBRunStatus { Queued, Running, Completed, Error }
pub enum TBRunOutcome { Success, Failure, Timeout, Error, Aborted }
pub enum TBModelOption { ClaudeSonnet, ClaudeHaiku, Gpt4o, Gpt4oMini, AppleFM }
```

**Extract from `crates/gym/src/services/`:**
- `task_loader.rs` - loads TBTask from JSON files
- `run_store.rs` - persists run history

**Cargo.toml:**
```toml
[package]
name = "terminalbench"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
chrono = "0.4"
uuid = { version = "1", features = ["v4"] }
harbor = { path = "../harbor" }
```

### Phase 2: Add Panel System to MechaCoder

**Create files:**
- `crates/mechacoder/src/panels/mod.rs`
- `crates/mechacoder/src/panels/gym_panel.rs`

**Modify `crates/mechacoder/src/screen.rs`:**

Current:
```rust
div().size_full().child(thread_view)
```

New:
```rust
div()
    .size_full()
    .flex()
    .flex_row()
    .child(div().flex_1().child(thread_view))
    .when(self.gym_panel_visible, |el| {
        el.child(
            div()
                .w(px(320.0))
                .border_l_1()
                .border_color(border::DEFAULT)
                .child(self.gym_panel.clone())
        )
    })
```

**Modify `crates/mechacoder/src/actions.rs`:**
```rust
actions!(mechacoder, [
    // ... existing
    ToggleGymPanel,
]);
```

**Modify `crates/mechacoder/src/main.rs`:**
```rust
KeyBinding::new("cmd-g", ToggleGymPanel, None),
KeyBinding::new("ctrl-g", ToggleGymPanel, None), // Linux
```

### Phase 3: Implement GymPanel Component

**`crates/mechacoder/src/panels/gym_panel.rs`:**

```rust
pub struct GymPanel {
    focus_handle: FocusHandle,
    task_loader: TaskLoader,
    selected_task: Option<TBTask>,
    tasks: Vec<TBTask>,
    recent_runs: Vec<TBRunSummary>,
    active_run: Option<ActiveRunState>,
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
| ACTIONS                   |
| [Run TB2]  [Run TestGen]  |
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
- `o` = success (lowercase o)
- `x` = failure
- `*` = running

### Phase 4: Integrate tbench Streaming

**Create `crates/mechacoder/src/panels/tbench_runner.rs`:**

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

**Extend `crates/mechacoder/src/sdk_thread.rs`:**

```rust
pub enum ThreadEntry {
    UserMessage(UserMessage),
    AssistantMessage(AssistantMessage),
    ToolUse(ToolUse),
    // New:
    TBenchRun(TBenchRunEntry),
    TBenchEvent(TBenchStreamEntry),
}

pub struct TBenchRunEntry {
    pub run_id: String,
    pub task_id: String,
    pub task_name: String,
    pub status: TBRunStatus,
}

pub struct TBenchStreamEntry {
    pub run_id: String,
    pub event: StreamEvent, // From harbor crate
}
```

**Modify `crates/mechacoder/src/ui/thread_view.rs`:**

Add rendering for TBench entries:
```rust
ThreadEntry::TBenchRun(run) => {
    // Render as header: [TB2] regex-log
}
ThreadEntry::TBenchEvent(entry) => {
    match &entry.event {
        StreamEvent::Assistant { turn, text } => // Render turn
        StreamEvent::ToolUse { tool, id } => // Render tool call
        StreamEvent::Complete { success, turns, cost, .. } => // Render result
    }
}
```

### Phase 5: Update gym to use terminalbench

**Modify `crates/gym/Cargo.toml`:**
```toml
terminalbench = { path = "../terminalbench" }
```

**Modify `crates/gym/src/tbcc/types.rs`:**
```rust
// Re-export from terminalbench
pub use terminalbench::{TBTask, TBDifficulty, TBRunSummary, ...};

// Keep gym-specific types here:
pub struct DashboardStats { ... }
pub enum TBCCTab { ... }
```

## Files to Create

| File | Purpose |
|------|---------|
| `crates/terminalbench/Cargo.toml` | New shared crate |
| `crates/terminalbench/src/lib.rs` | Crate root |
| `crates/terminalbench/src/types.rs` | Core TB types |
| `crates/terminalbench/src/task_loader.rs` | Task loading |
| `crates/terminalbench/src/run_store.rs` | Run history |
| `crates/mechacoder/src/panels/mod.rs` | Panel system |
| `crates/mechacoder/src/panels/gym_panel.rs` | Gym panel |
| `crates/mechacoder/src/panels/tbench_runner.rs` | TB2 execution |

## Files to Modify

| File | Changes |
|------|---------|
| `crates/mechacoder/Cargo.toml` | Add terminalbench, harbor deps |
| `crates/mechacoder/src/lib.rs` | Export panels module |
| `crates/mechacoder/src/main.rs` | Add Cmd+G keybinding |
| `crates/mechacoder/src/actions.rs` | Add ToggleGymPanel |
| `crates/mechacoder/src/screen.rs` | Add panel layout |
| `crates/mechacoder/src/sdk_thread.rs` | Extend ThreadEntry enum |
| `crates/mechacoder/src/ui/thread_view.rs` | Render TBench entries |
| `crates/gym/Cargo.toml` | Depend on terminalbench |
| `crates/gym/src/tbcc/types.rs` | Import from terminalbench |

## Sequence: Start TB2 Run

```
1. User: Cmd+G (toggle Gym panel)
2. GymPanel visible, shows task dropdown
3. User: Select "regex-log"
4. User: Click [Run TB2]
5. GymPanel: Check SdkThread not streaming
6. TBenchRunner: Spawn `tbench --stream`
7. For each StreamEvent from stdout:
   - Parse JSON -> StreamEvent
   - Create ThreadEntry::TBenchEvent
   - ThreadView renders in main chat
   - GymPanel updates progress
8. On Complete: Update recent_runs, clear active_run
```

## Notes

- Edition 2024 for all new Rust crates
- Use `cargo add` for dependencies (per project rules)
- No emojis in UI - use ASCII/Unicode symbols
- Sequential streams only (no concurrent chat+tbench)


