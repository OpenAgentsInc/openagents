# MechaCoder Screen Implementation

**Date:** 2025-12-11
**Session:** Continuation from previous context (Claude Agent SDK + HillClimber architecture)

## Overview

This session implemented the MechaCoder screen - a new Gym tab that provides a flexible Terminal-Bench solver with support for both FM (Apple Foundation Model) and CC (Claude Code SDK) backends. This follows the architecture designed in `docs/claude/agent-sdk/HILLCLIMBER-CLAUDE-ARCHITECTURE.md`.

## Prior Context

The previous session:
1. Built the Rust Claude Agent SDK with 100% parity to the Node.js SDK
2. Fixed a bug with `--setting-sources` flag (was passing singular `--setting-source` multiple times)
3. Created the architecture design doc for integrating Claude Agent SDK with HillClimber

## Implementation Summary

### 1. HillClimber Backend Enum

**File:** `crates/hillclimber/src/types.rs`

Added `HillClimberBackend` enum to support backend selection:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HillClimberBackend {
    #[default]
    FM,  // Apple Foundation Model (local inference)
    CC,  // Claude Code SDK (cloud API)
}
```

### 2. Claude Code HillClimber Runner

**File:** `crates/hillclimber/src/cc_runner.rs` (new, ~450 lines)

Created a new runner that uses the Claude Agent SDK instead of the FM-based MAP orchestrator:

**Key Types:**
- `CCRunnerOptions` - Configuration (model, max_turns, budget, skills, workspace)
- `CCIterationResult` - Result from a single iteration
- `CCHillClimberRunner` - Main runner struct

**Key Differences from FM Runner:**
- FM Runner: Manual orchestration via `MAPOrchestrator` with `FMClient` trait + `ToolExecutor` trait
- CC Runner: Claude Code handles everything - just send prompt and stream results

**Flow:**
```
1. Build prompt from task description
2. Configure QueryOptions (model, turns, budget, setting_sources for skills)
3. Call query() to spawn Claude Code CLI
4. Stream SdkMessage events:
   - Assistant messages → track turns
   - ToolProgress → log tool usage
   - Result → extract final status (num_turns, cost, success)
5. Save run to HillClimberStore
```

**Options Builder Pattern:**
```rust
let options = CCRunnerOptions::new()
    .model("claude-sonnet-4-5-20250929")
    .max_turns(30)
    .max_budget_usd(5.0)
    .use_skills(true)
    .workspace("/path/to/task");
```

### 3. MechaCoder Screen

**Directory:** `crates/gym/src/mechacoder/`

Created a complete new screen with four files:

#### `types.rs` (~230 lines)
Domain types for the MechaCoder screen:
- `MechaStatus` - Idle, GeneratingTests, Running, WaitingInput, Solved, Failed
- `MechaTask` - Task definition (id, name, description, verification_cmd)
- `MechaSession` - Full session state (status, backend, task, turn, progress, cost)
- `LogKind` - Info, Progress, Tool, Thinking, TestResult, Success, Error
- `LogEntry` - Timestamped log entry with kind, message, optional details
- `MechaEvent` - Events from the runner (Started, TurnStart, ToolUse, VerifyResult, Completed, Error)
- `tasks::regex_log()` - Built-in regex-log task definition

#### `task_panel.rs` (~350 lines)
Left panel showing:
- Task header (name + "TARGET: 100%" badge)
- Status badge (color-coded by status)
- **Backend toggle** - Two clickable buttons to switch between FM and CC
- Progress display (large percentage + bar)
- Cost display (for CC backend)
- Current solution preview
- Task description

Emits `SwitchBackend(HillClimberBackend)` event when user clicks toggle.

#### `log_panel.rs` (~210 lines)
Right panel showing:
- Header with event count and tool count badges
- Scrollable list of log entries
- Each entry shows: timestamp, kind badge (color-coded), message
- Optional expandable details section

#### `mod.rs` (~290 lines)
Main screen component:
- Creates and manages TaskPanel and LogPanel entities
- Subscribes to SwitchBackend events from TaskPanel
- Emits StartRun/StopRun events for parent to handle
- Renders top control bar with Start/Stop button + backend indicator
- Two-panel layout (280px left panel + flex right panel)

### 4. Gym Integration

**Files Modified:**

#### `crates/gym/Cargo.toml`
Added dependency:
```toml
hillclimber = { path = "../hillclimber" }
```

#### `crates/gym/src/lib.rs`
Added module:
```rust
pub mod mechacoder;
```

#### `crates/gym/src/types.rs`
Added tab variant:
```rust
pub enum GymTab {
    // ... existing variants ...
    MechaCoder,  // New
}
```

Updated `label()` and `all()` methods. MechaCoder is now first in tab order.

#### `crates/gym/src/actions.rs`
Added action:
```rust
actions!(gym, [
    // ...
    SwitchToMechaCoder,
]);
```

#### `crates/gym/src/gym_screen.rs`
- Added import for `MechaCoderScreen`
- Added `mechacoder_view: Entity<MechaCoderScreen>` field
- Created view in `with_store()`
- Added `switch_to_mechacoder()` handler
- Added to `render_active_tab_content()` match
- Registered action handler in `render()`

## Technical Decisions

### Why Separate CC Runner?

The FM runner uses a fundamentally different architecture:
- **FM:** Manual tool orchestration via traits (`FMClient`, `ToolExecutor`)
- **CC:** Claude Code handles tools internally, we just stream results

Rather than trying to unify these, a separate runner keeps the code clean and allows each to be optimized for its paradigm.

### Skills Integration

The CC runner uses the `setting_sources` option to load skills:
```rust
if options.use_skills {
    query_options = query_options.setting_sources(vec![
        SettingSource::Project,
        SettingSource::User,
    ]);
}
```

This loads `.claude/skills/` from the project and user directories, enabling domain-specific procedural knowledge.

### Backend Toggle UX

The toggle is in the task panel (left side) rather than the control bar because:
1. It's a session-level configuration, not a run-level action
2. Keeps related info together (backend + cost display)
3. Disabled when session is busy (prevents mid-run switching)

### Scrolling Workaround

The initial implementation used `overflow_y_scroll()` which doesn't exist in the GPUI version. Changed to `overflow_hidden()` as a temporary fix. Regex Crusade uses `overflow_y_scroll()` but compiles - this suggests a version mismatch or conditional compilation. Can be fixed later.

## Files Created/Modified

### New Files
- `crates/hillclimber/src/cc_runner.rs` (~450 lines)
- `crates/gym/src/mechacoder/mod.rs` (~290 lines)
- `crates/gym/src/mechacoder/types.rs` (~230 lines)
- `crates/gym/src/mechacoder/task_panel.rs` (~350 lines)
- `crates/gym/src/mechacoder/log_panel.rs` (~210 lines)

### Modified Files
- `crates/hillclimber/src/types.rs` - Added `HillClimberBackend` enum
- `crates/hillclimber/src/lib.rs` - Added exports for cc_runner module
- `crates/hillclimber/Cargo.toml` - Added claude_agent_sdk dependency
- `crates/gym/Cargo.toml` - Added hillclimber dependency
- `crates/gym/src/lib.rs` - Added mechacoder module
- `crates/gym/src/types.rs` - Added MechaCoder tab
- `crates/gym/src/actions.rs` - Added SwitchToMechaCoder action
- `crates/gym/src/gym_screen.rs` - Wired up MechaCoder screen

## Session 2: Runner Execution + Thorough Logging (continued)

**Fixed Issue:** User reported "I SEE NO FUCKING LOGGING" - the UI showed "Running..." but nothing was actually happening.

### Root Cause

The UI was built but the actual Claude Code SDK query was never executed. The spawn closure signatures were wrong for GPUI.

### Fixes Applied

#### 1. Spawn Closure Signature Fix

The GPUI `cx.spawn()` method uses async closures. The pattern is:
```rust
// WRONG - causes type mismatch
cx.spawn(|this: WeakEntity<Self>, mut cx: gpui::AsyncApp| async move { ... }).detach();

// CORRECT - use async closure syntax
cx.spawn(async move |this, cx| { ... }).detach();
```

#### 2. Added Thorough Logging (tracing)

Added `telemetry` and `tracing` dependencies to gym's Cargo.toml.

**All logging uses `target: "mechacoder"` or `target: "mechacoder::cc"` for filtering.**

Key log points added:

| Function | Level | What's Logged |
|----------|-------|---------------|
| `new()` | info | Screen creation, default session config |
| `poll_events()` | trace/debug | Event count, each event type |
| `switch_backend()` | info/warn | Backend changes, blocked if busy |
| `set_task()` | info | Task changes |
| `on_start()` | info/debug | Start button click, session reset, channel creation, spawn |
| `on_stop()` | info | Stop button click with turn/cost stats |
| `run_cc_query()` | info/debug/trace/warn/error | Every step of CC SDK execution |

**Detailed CC Query Logging:**
- Query options (model, max_turns)
- `query()` call success/failure
- Every stream message (message count tracked)
- Assistant turns with content length
- Tool progress with name and duration
- Result types (Success, ErrorDuringExecution, ErrorMaxTurns, ErrorMaxBudget)
- Stream errors
- Unexpected stream end

#### 3. Handled Missing SdkMessage Variants

Added handling for:
- `SdkMessage::StreamEvent`
- `SdkMessage::AuthStatus`

### Dependencies Updated

`crates/gym/Cargo.toml`:
```toml
telemetry = { path = "../telemetry" }
tracing = "0.1"
tokio = { version = "1", features = ["sync", "time"] }  # added "time" for sleep
```

## Remaining Work

### Not Yet Implemented
1. ~~**Actual runner execution**~~ - ✅ FIXED - CC backend now executes properly
2. **FM backend integration** - Only CC runner exists; FM should use existing `HillClimberRunner`
3. **Task selection UI** - Currently hardcoded to regex-log
4. **TestGen integration** - GeneratingTests status exists but not wired up
5. **Scrolling fix** - `overflow_y_scroll()` needs investigation
6. **Workspace directory** - Currently runs in cwd, should be configurable

### Future Enhancements
1. **Workspace picker** - Let user select task workspace directory
2. **History view** - Show past runs from HillClimberStore
3. **Solution diff** - Show what changed between iterations
4. **Real-time test results** - Parse pytest output and show pass/fail per test

## Verification

Both crates compile successfully:
```bash
cargo check -p hillclimber  # OK (2 warnings)
cargo check -p gym          # OK (no warnings)
cargo build -p gym          # OK
```

## References

- Architecture doc: `docs/claude/agent-sdk/HILLCLIMBER-CLAUDE-ARCHITECTURE.md`
- Claude Agent SDK: `crates/claude_agent_sdk/`
- Existing FM runner: `crates/hillclimber/src/runner.rs`
- RegexCrusade (reference UI): `crates/gym/src/regex_crusade/`
