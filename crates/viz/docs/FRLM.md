# FRLM Visualization Components

This module provides visualization components for FRLM (Federated Recursive Language Models) trace events and execution state.

## Components

### FrlmPanel

A composite panel that shows the complete FRLM execution state:

```rust
use viz::frlm::FrlmPanel;
use viz::frlm::QueryStatus;

let mut panel = FrlmPanel::new();

// Set run ID
panel.set_run_id("run-abc123");

// Update budget display
panel.set_budget(
    500,   // spent_sats
    200,   // reserved_sats
    1000,  // limit_sats
);

// Update sub-query status
panel.update_query(
    "sq-1",
    QueryStatus::Executing,
    0,      // start_ms
    None,   // end_ms (still running)
    Some("provider-xyz".to_string()),
);

// Paint the panel
panel.paint(bounds, &mut cx);
```

**Features:**
- Collapsible title bar with stats summary
- Budget meter showing spend vs limit
- Timeline view with scrollable sub-query lanes
- Event handling for clicks and scrolling

### BudgetMeter

A horizontal progress bar showing budget usage:

```rust
use viz::frlm::BudgetMeter;

let mut meter = BudgetMeter::new()
    .with_limit(1000);

meter.set_budget(
    300,  // spent_sats
    100,  // reserved_sats
    1000, // limit_sats
);

meter.paint(bounds, &mut cx);
```

**Visual Elements:**
- Background track (dark)
- Reserved portion (blue, semi-transparent)
- Spent portion (green → orange → red based on usage)
- Text label: "X / Y sats"

### FrlmTimeline

A horizontal timeline showing sub-queries over time:

```rust
use viz::frlm::{FrlmTimeline, TimelineEntry, QueryStatus};

let mut timeline = FrlmTimeline::new();
timeline.set_run_id("run-abc123");

// Add entries
timeline.update_entry(TimelineEntry {
    query_id: "sq-1".to_string(),
    status: QueryStatus::Complete,
    start_ms: 0,
    end_ms: Some(1500),
    provider_id: Some("provider-xyz".to_string()),
});

// Scroll the timeline
timeline.scroll(20.0);

timeline.paint(bounds, &mut cx);
```

**Visual Elements:**
- Header with run ID and stats
- Time markers with grid lines
- Lane per sub-query with time spans
- Color-coded status bars
- Pulsing effect for executing queries
- Scrollbar for many queries

### QueryLane

Individual sub-query progress visualization:

```rust
use viz::frlm::{QueryLane, QueryStatus};

let mut lane = QueryLane::new("sq-1")
    .with_status(QueryStatus::Executing);

lane.set_progress(0.7);  // 70% complete
lane.set_provider("provider-xyz");

lane.paint(bounds, &mut cx);
```

**Visual Elements:**
- Status dot with pulse animation
- Progress bar track and fill
- Query ID label
- Duration display (when complete)

### QueryStatus

Status enum with color mappings:

```rust
use viz::frlm::QueryStatus;

let status = QueryStatus::Executing;
let color = status.color();  // Orange
let label = status.label();  // "executing"
```

| Status | Color | Description |
|--------|-------|-------------|
| `Pending` | Gray | Not yet submitted |
| `Submitted` | Blue | Submitted to provider |
| `Executing` | Orange | Provider is processing |
| `Complete` | Green | Successfully completed |
| `Failed` | Red | Error occurred |
| `Timeout` | Purple | Timed out |

## Layout

The FRLM panel uses this layout:

```
┌─────────────────────────────────────────────────────────┐
│ FRLM CONDUCTOR                    2/5 ✓  2 ⏳      ▼   │  <- Title bar
├─────────────────────────────────────────────────────────┤
│ ████████░░░░░░░░░░░░░░░░  300 / 1000 sats              │  <- Budget meter
├─────────────────────────────────────────────────────────┤
│ FRLM Run abc123 | 2/5 complete | 2 active              │  <- Timeline header
│─────────────────────────────────────────────────────────│
│ 0s        1s        2s        3s        4s        5s   │  <- Time markers
│─────────────────────────────────────────────────────────│
│ sq-1...  ████████████████                        1.5s  │  <- Query lanes
│ sq-2...  ██████████████████████████                    │
│ sq-3...  ████████                                      │
│ sq-4...  ○ pending                                     │
│ sq-5...  ○ pending                                     │
└─────────────────────────────────────────────────────────┘
```

## Integration with Pylon

The Pylon desktop app uses a direct-draw version in `ui/frlm_panel.rs`:

```rust
// In crates/pylon-desktop/src/ui/mod.rs
if has_frlm {
    // 3-column layout: Jobs | FRLM | Chat
    frlm_panel::draw_frlm_panel(scene, text, state, x, y, width, height);
}
```

The panel automatically appears in a 3-column layout when:
- `state.frlm_active_run` is `Some(_)`, OR
- `state.frlm_subquery_status` is non-empty

## State Types

### FrlmRunState

```rust
pub struct FrlmRunState {
    pub run_id: String,
    pub program: String,
    pub fragment_count: usize,
    pub pending_queries: usize,
    pub completed_queries: usize,
    pub budget_used_sats: u64,
    pub budget_remaining_sats: u64,
    pub started_at: u64,
}
```

### SubQueryDisplayStatus

```rust
pub enum SubQueryDisplayStatus {
    Pending,
    Submitted { job_id: String },
    Executing { provider_id: String },
    Complete { duration_ms: u64 },
    Failed { error: String },
    Timeout,
}
```

## Animation

Components use delta-based animation for smooth transitions:

```rust
// In paint() method
let delta = (self.target - self.value) * 0.15;
if delta.abs() > 0.0001 {
    self.value += delta;
}
```

This provides:
- Smooth interpolation (15% per frame)
- Natural easing
- No explicit duration management

## Colors

The module uses consistent colors matching the Pylon theme:

| Element | Color | Hsla |
|---------|-------|------|
| Panel background | Dark blue-gray | `(220°, 15%, 8%)` |
| Header background | Darker | `(0°, 0%, 12%)` |
| Text (dim) | Gray | `(0°, 0%, 50%)` |
| Text (bright) | White | `(0°, 0%, 90%)` |
| Accent (cyan) | Cyan | `(180°, 80%, 50%)` |
| Accent (green) | Green | `(145°, 70%, 45%)` |
| Warning | Orange | `(45°, 90%, 55%)` |
| Critical | Red | `(0°, 85%, 50%)` |

## See Also

- [FRLM Crate](../../frlm/docs/README.md) - Core FRLM conductor
- [Viz Grammar](./GRAMMAR.md) - Base visualization grammar
- [WGPUI Components](../../wgpui/docs/README.md) - UI component system
