# 2028: FRLM/RLM/Apple FM Visualization in Pylon Desktop

**Date:** 2026-01-04
**Author:** Claude (Opus 4.5)
**Scope:** Integrating RLM, FRLM, and Apple FM execution visualization into Pylon desktop using viz/wgpui component infrastructure

---

## Overview

This session implemented comprehensive visualization for FRLM (Federated RLM), RLM (Recursive Language Model), and Apple FM tool execution in the Pylon desktop application. The work bridges the existing `viz` crate components with `pylon-desktop`'s UI system.

## Problem Statement

Prior to this work:
- The `viz` crate had rich visualization components (`FrlmPanel`, `FrlmTimeline`, `BudgetMeter`, `QueryLane`) but they weren't wired to pylon-desktop
- FRLM trace events from the conductor weren't being consumed for visualization
- Apple FM tool calls (7 tools via Swift Bridge) had no UI representation
- RLM execution iterations weren't visible
- No topology graph showing execution venues (Local/Swarm/Datacenter)

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Pylon Desktop App                                │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │ HEADER: FM Bridge status | Nostr auth | Wallet balance | FRLM active││
│  ├─────────────────────────────────────────────────────────────────────┤│
│  │ TOKEN STREAM (FM output)                                            ││
│  ├─────────────────────────────────────────────────────────────────────┤│
│  │ PROMPT INPUT                                                        ││
│  ├───────────────┬───────────────────────────────┬─────────────────────┤│
│  │ JOBS PANEL    │ FRLM/RLM/TOOLS PANEL          │ CHAT PANEL          ││
│  │ (NIP-90 list) │ (viz::frlm::FrlmPanel)        │ (Nostr messages)    ││
│  │               │ + Apple FM Tools              │                     ││
│  │               │ + RLM Iterations              │                     ││
│  │               │ + Topology Graph              │                     ││
│  ├───────────────┴───────────────────────────────┴─────────────────────┤│
│  │ TOKEN RAIL: Token speed | FRLM events | Tool calls                  ││
│  └─────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────┘

Data Flow:
  FRLM Conductor → TraceEvent channel → frlm_integration.poll() → FmVizState
                                              ↓
                                      update_frlm_panel()
                                              ↓
                                       viz::FrlmPanel + VenueTopology
```

---

## Implementation Details

### 1. State Extensions (`crates/pylon-desktop/src/state.rs`)

Added new types for visualization:

```rust
// Tool call status tracking
pub enum ToolCallStatus {
    Pending,
    Executing,
    Complete,
    Failed,
}

// Apple FM tool call representation
pub struct AppleFmToolCall {
    pub tool_name: String,
    pub arguments: String,
    pub status: ToolCallStatus,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub result: Option<String>,
}

// RLM iteration tracking
pub struct RlmIteration {
    pub iteration: u32,
    pub command_type: String,  // "Run", "RunCode", "Final"
    pub executed: String,
    pub result: String,
    pub duration_ms: u64,
}

// Execution venue types
pub enum ExecutionVenue {
    Local,      // FM Bridge (green)
    Swarm,      // NIP-90 network (orange)
    Datacenter, // Datacenter providers (purple)
    Unknown,
}

// Venue topology graph wrapper
pub struct VenueTopology {
    pub graph: viz::topology::Graph,
    next_node_id: NodeId,
    conductor_id: NodeId,
    venue_nodes: HashMap<String, NodeId>,
}
```

Extended `FmVizState` with:
- `frlm_panel: Option<FrlmPanel>` - viz crate panel
- `apple_fm_tool_calls: Vec<AppleFmToolCall>` - tool call history
- `current_tool_call: Option<AppleFmToolCall>` - active tool
- `rlm_iterations: Vec<RlmIteration>` - iteration history
- `rlm_active: bool` - RLM running flag
- `venue_topology: VenueTopology` - execution graph

### 2. UI Panels

#### FRLM Panel (`crates/pylon-desktop/src/ui/frlm_panel.rs`)

Rewrote to use `viz::frlm::FrlmPanel` component:

```rust
pub fn draw_frlm_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &mut FmVizState,
    x: f32, y: f32, width: f32, height: f32,
) {
    // Initialize panel if needed
    if state.frlm_panel.is_none() {
        state.frlm_panel = Some(FrlmPanel::new());
    }

    let panel = state.frlm_panel.as_mut().unwrap();

    // Update from state
    if let Some(ref run) = state.frlm_active_run {
        panel.set_run_id(&run.run_id);
        panel.set_budget(run.budget_used_sats, 0, run.budget_remaining_sats + run.budget_used_sats);
    }

    // Update query statuses from frlm_subquery_status
    for (query_id, status) in &state.frlm_subquery_status {
        let query_status = match status { ... };
        panel.update_query(query_id, query_status, 0, duration_ms, provider_id);
    }

    // Paint using Component trait
    let mut paint_cx = PaintContext::new(scene, text, 1.0);
    panel.paint(bounds, &mut paint_cx);
}
```

#### Apple FM Tools Panel (`crates/pylon-desktop/src/ui/apple_fm_panel.rs`)

New file visualizing tool selection and execution:

- Active tool indicator with pulsing orange dot
- Recent tool calls as scrollable list
- Status-colored dots (pending=gray, executing=orange, complete=green, failed=red)
- Duration display for completed calls
- "... +N more" indicator for overflow

#### RLM Execution Panel (`crates/pylon-desktop/src/ui/rlm_panel.rs`)

New file visualizing RLM execution loop:

- Iteration counter with progress bar
- Color-coded command types:
  - `Run` = blue (LLM inference)
  - `RunCode` = orange (code execution)
  - `Final` = green (completion)
- Command timeline showing recent iterations
- Duration display per iteration

#### Topology Panel (`crates/pylon-desktop/src/ui/topology_panel.rs`)

New file for venue topology graph:

- Uses `viz::topology::Graph` component
- Central "FRLM Conductor" node
- Dynamic provider nodes added as queries execute
- Edge connections from conductor to providers
- Venue legend: Local (green), Swarm (orange), Datacenter (purple)

### 3. Layout Updates (`crates/pylon-desktop/src/ui/mod.rs`)

Updated `build_pylon_ui` with adaptive layout:

```rust
// Determine which visualization mode we're in
let has_frlm = state.frlm_active_run.is_some() || !state.frlm_subquery_status.is_empty();
let has_rlm = state.rlm_active || !state.rlm_iterations.is_empty();
let has_tools = state.current_tool_call.is_some() || !state.apple_fm_tool_calls.is_empty();

if has_frlm || has_rlm || has_tools {
    // 3-column layout: Jobs | Execution Viz | Chat
    let panel_width = (width - padding * 2.0 - gap * 2.0) / 3.0;

    // Left: Jobs
    jobs_panel::draw_jobs_panel(...);

    // Center: Visualization (stacked based on activity)
    if has_frlm {
        frlm_panel::draw_frlm_panel(...);
        if has_tools {
            apple_fm_panel::draw_apple_fm_tools_panel(...);
        }
    } else if has_rlm {
        rlm_panel::draw_rlm_panel(...);
    } else if has_tools {
        apple_fm_panel::draw_apple_fm_tools_panel(...);
        topology_panel::draw_topology_panel(...);
    }

    // Right: Chat
    chat_panel::draw_chat_panel(...);
} else {
    // 2-column layout: Jobs | Chat (original)
}
```

### 4. TraceEvent Processing (`crates/pylon-desktop/src/app.rs`)

Added `FrlmIntegration` to `RenderState`:

```rust
pub struct RenderState {
    // ... existing fields ...
    pub frlm_integration: FrlmIntegration,
}
```

Integrated into `about_to_wait()`:

```rust
fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    if let Some(state) = &mut self.state {
        // ... existing FM, Nostr, Wallet polling ...

        // Poll FRLM trace events (non-blocking)
        state.frlm_integration.poll(&mut state.fm_state);

        // Update FrlmPanel from state (sync viz panel with current state)
        update_frlm_panel(&mut state.fm_state);

        // ... rest of event loop ...
    }
}
```

Added `update_frlm_panel()` function:
- Ensures `FrlmPanel` exists
- Updates budget meter from active run
- Syncs query statuses with `QueryStatus` enum
- Updates topology with executing providers
- Sets current time for timeline animation

### 5. FRLM Integration Updates (`crates/pylon-desktop/src/frlm_integration.rs`)

Enhanced `update_state_from_trace` to track venue:

```rust
TraceEvent::SubQueryExecute { query_id, provider_id, venue, .. } => {
    state.frlm_subquery_status.insert(
        query_id,
        SubQueryDisplayStatus::Executing { provider_id: provider_id.clone() },
    );

    // Update topology with venue
    let execution_venue = match venue {
        frlm::types::Venue::Local => ExecutionVenue::Local,
        frlm::types::Venue::Swarm => ExecutionVenue::Swarm,
        frlm::types::Venue::Datacenter => ExecutionVenue::Datacenter,
        frlm::types::Venue::Unknown => ExecutionVenue::Unknown,
    };
    state.venue_topology.record_execution(execution_venue, Some(&provider_id));
}
```

---

## Files Modified/Created

| File | Action | Purpose |
|------|--------|---------|
| `crates/pylon-desktop/src/state.rs` | Modified | Added visualization types (ToolCallStatus, AppleFmToolCall, RlmIteration, ExecutionVenue, VenueTopology) and extended FmVizState |
| `crates/pylon-desktop/src/app.rs` | Modified | Added FrlmIntegration to RenderState, poll() call in about_to_wait, update_frlm_panel() function |
| `crates/pylon-desktop/src/frlm_integration.rs` | Modified | Added ExecutionVenue import, venue tracking in SubQueryExecute |
| `crates/pylon-desktop/src/ui/mod.rs` | Modified | Added 3-column layout logic with panel imports |
| `crates/pylon-desktop/src/ui/frlm_panel.rs` | Modified | Rewrote to use viz::frlm::FrlmPanel |
| `crates/pylon-desktop/src/ui/apple_fm_panel.rs` | Created | Apple FM tools visualization |
| `crates/pylon-desktop/src/ui/rlm_panel.rs` | Created | RLM execution visualization |
| `crates/pylon-desktop/src/ui/topology_panel.rs` | Created | Venue topology graph |

---

## TraceEvent Types Supported

The FRLM conductor emits these events (from `crates/frlm/src/trace.rs`):

| Event | Visualization |
|-------|---------------|
| `RunInit` | Initializes FrlmPanel, sets fragment count |
| `RunDone` | Clears active run, updates totals |
| `SubQuerySubmit` | Adds pending query to timeline |
| `SubQueryExecute` | Updates query to executing, adds to topology |
| `SubQueryReturn` | Marks complete, shows duration |
| `SubQueryTimeout` | Marks timeout status |
| `BudgetReserve` | Updates budget meter (reserved) |
| `BudgetSettle` | Updates budget meter (actual vs reserved) |
| `FallbackLocal` | Could show local fallback indicator |

---

## Build Status

```
$ cargo build -p pylon-desktop
   Compiling pylon-desktop v0.1.0
warning: `pylon-desktop` (bin "pylon-desktop") generated 14 warnings
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.37s
```

The 14 warnings are for dead code that is prepared for future features:
- FRLM run management methods (will be called when FRLM is actively running)
- State fields for tool results (will be populated when tools execute)
- Utility methods for clearing highlights, etc.

---

## Next Steps

1. **Wire Apple FM tool events** - Connect Swift Bridge tool calls to `apple_fm_tool_calls` state
2. **Wire RLM iteration events** - Connect RLM engine to `rlm_iterations` state
3. **Test with live FRLM run** - Verify timeline, budget meter, topology updates in real-time
4. **Add timeline scrubbing** - Allow replaying past execution with trace buffer
5. **Add execution receipts** - Show cost attribution per sub-query

---

## References

- Plan file: `/Users/christopherdavid/.claude/plans/refactored-jingling-sketch.md`
- viz crate: `crates/viz/` - FrlmPanel, BudgetMeter, QueryLane, Graph
- FRLM crate: `crates/frlm/` - TraceEvent, TraceEmitter, Conductor
- Previous work: Commit 090bc1c15 (RLM as Apple FM tools via FRLM conductor)
