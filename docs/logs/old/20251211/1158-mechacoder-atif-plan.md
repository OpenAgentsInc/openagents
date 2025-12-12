# Plan: Save MechaCoder Conversations as ATIF Trajectories

## Goal
Save every MechaCoder Claude Code SDK conversation as an ATIF trajectory in `~/.openagents/trajectories.db`.

## Key Files to Modify

| File | Changes |
|------|---------|
| `crates/gym/src/mechacoder/mod.rs` | Add TrajectoryStore, create trajectory on start, save steps |
| `crates/gym/src/mechacoder/types.rs` | Add session_id to MechaSession |
| `crates/gym/src/gym_screen.rs` | Pass TrajectoryStore to MechaCoderScreen |
| `crates/gym/Cargo.toml` | Already has atif, atif-store deps |

## Implementation Steps

### Step 1: Add TrajectoryStore to MechaCoderScreen

```rust
// In mod.rs
use atif::{Agent, Step, ToolCall, FinalMetrics};
use atif_store::TrajectoryStore;
use std::sync::{Arc, Mutex};

pub struct MechaCoderScreen {
    // ... existing fields ...
    store: Option<Arc<Mutex<TrajectoryStore>>>,
}
```

### Step 2: Add session_id to MechaSession

```rust
// In types.rs
pub struct MechaSession {
    // ... existing fields ...
    pub session_id: Option<String>,  // ATIF trajectory session ID
}
```

### Step 3: Create trajectory on Start

In `on_start()`:
1. Create Agent with name="mechacoder", version="0.1.0", model="claude-sonnet-4"
2. Call `store.create_trajectory(&agent)` → get session_id
3. Store session_id in MechaSession
4. Add initial system step with task prompt (Step::system)
5. Pass session_id and store clone to runner thread

### Step 4: Save steps during run_cc_query()

For each SDK message:

**Assistant messages → Step::agent()**
- Extract text content as message
- Extract tool_use blocks as ToolCall structs
- Add reasoning_content if thinking block present
- Save immediately with `store.add_step()`

**Tool results (from next Assistant's observation)**
- Create Observation with ObservationResult
- Attach to the agent step via `with_observation()`

**ToolProgress messages**
- Update step metrics with elapsed time (optional)

### Step 5: Complete/fail trajectory on Result

When `SdkMessage::Result` received:

**Success:**
```rust
let final_metrics = FinalMetrics {
    total_cost_usd: Some(cost),
    total_steps: Some(turn as i64),
    ..Default::default()
};
store.complete_trajectory(&session_id, Some(&final_metrics))?;
```

**Error:**
```rust
store.fail_trajectory(&session_id)?;
```

### Step 6: Wire up TrajectoryStore from GymScreen

In `gym_screen.rs`, pass the store to MechaCoderScreen:
```rust
let mechacoder_screen = cx.new(|cx| {
    MechaCoderScreen::with_store(cx, store.clone())
});
```

## Data Mapping: SDK → ATIF

| SDK Message | ATIF Step |
|-------------|-----------|
| Initial prompt | `Step::system(1, prompt)` |
| `SdkMessage::Assistant` | `Step::agent(n, text).with_tool_calls(...)` |
| `SdkMessage::User` (tool result) | Attach as `Observation` to previous agent step |
| `SdkMessage::Result(Success)` | `complete_trajectory()` with FinalMetrics |
| `SdkMessage::Result(Error*)` | `fail_trajectory()` |

## ToolCall Extraction

From assistant message content:
```rust
if item.get("type") == Some("tool_use") {
    let tool_call = ToolCall::new(
        item.get("id").as_str(),      // tool_call_id
        item.get("name").as_str(),    // function_name
        item.get("input").clone(),    // arguments (JSON)
    );
    tool_calls.push(tool_call);
}
```

## Thread Safety

- TrajectoryStore wrapped in `Arc<Mutex<TrajectoryStore>>`
- Clone Arc for runner thread
- Lock briefly for each add_step() call
- Runner thread has its own tokio runtime, store access is sync

## Verification

After implementation:
```bash
sqlite3 ~/.openagents/trajectories.db "SELECT session_id, agent_name, status, total_steps FROM trajectories WHERE agent_name='mechacoder' ORDER BY created_at DESC LIMIT 5;"
```

## Not In Scope
- Streaming step updates (append_to_step) - full steps are fine
- Per-step token metrics - only final cost tracked
- Subagent references - MechaCoder is single agent
