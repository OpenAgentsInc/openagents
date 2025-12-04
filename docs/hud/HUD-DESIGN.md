# OpenAgents HUD Design

> **The Definitive Guide to the Factorio-Inspired Agent Factory Interface**

This document consolidates the vision, architecture, and implementation details for the OpenAgents HUD - a game-like interface for monitoring and controlling AI coding agents.

## Vision

The HUD presents AI agent activity as a **factory management game**. Inspired by Factorio's industrial aesthetic:

- **Nodes = Entities** - Agents, repos, tasks, benchmark runs
- **Edges = Conveyor Belts** - Data and job flow between nodes
- **Canvas = Factory Floor** - Infinite grid you can pan and zoom
- **Feeds = Production Logs** - Streaming output from agents

The goal is **glanceability**: understand what agents are doing at a glance, like watching a factory humming along.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     OpenAgents Desktop                          │
│                      (Electrobun)                               │
├─────────────────────────────────────────────────────────────────┤
│                         Mainview                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   SVG Canvas                             │   │
│  │  ┌─────────┐    ┌─────────┐    ┌─────────┐              │   │
│  │  │  Node   │───▶│  Node   │───▶│  Node   │              │   │
│  │  └─────────┘    └─────────┘    └─────────┘              │   │
│  │       Pan/Zoom • Grid Background • Animated Edges       │   │
│  └─────────────────────────────────────────────────────────┘   │
│  ┌────────────┐  ┌────────────┐  ┌────────────────────────┐   │
│  │ APM Widget │  │ TB Widget  │  │    Output Feed         │   │
│  └────────────┘  └────────────┘  └────────────────────────┘   │
├─────────────────────────────────────────────────────────────────┤
│                    WebSocket (port 4242)                        │
├─────────────────────────────────────────────────────────────────┤
│  MechaCoder    │  Terminal-Bench   │   Claude Code             │
│  Orchestrator  │  Runner           │   Subagent                │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Breakdown

| Layer | Module | Purpose |
|-------|--------|---------|
| **Model** | `src/flow/model.ts` | Pure types for nodes, edges, positions |
| **Layout** | `src/flow/layout.ts` | Tree → positioned nodes + connection paths |
| **Canvas** | `src/flow/canvas.ts` | Pan/zoom state machine |
| **TB Map** | `src/flow/tb-map.ts` | Terminal-Bench state → flow tree |
| **Renderer** | `src/flow-host-svg/render.ts` | PositionedNodes → SVG elements |
| **Host** | `src/mainview/index.ts` | Event handling, state management, UI |

## View Modes

The HUD supports multiple view modes, toggled via the View Mode selector:

### 1. Terminal-Bench Mode (Default)

Shows benchmark runs with:
- **Run History Timeline** - Horizontal row of past runs
- **Run Summary Nodes** - Pass rate, task count, timing
- **Expanded Run View** - Click to see individual tasks
- **Live Output Feed** - Streaming agent output during runs

### 2. Flow Mode

Shows MechaCoder activity with:
- **Agent Nodes** - MechaCoder process status
- **Task Nodes** - Current and queued tasks
- **Phase Indicators** - Read/Plan/Edit/Test/Commit flow
- **Repo Nodes** - Active repositories

### 3. Combined Mode (Future)

Both TB runs and MechaCoder flow on the same canvas.

## Flow Node Types

### Core Types

```typescript
interface FlowNode {
  id: NodeId;
  type: string;
  label: string;
  direction?: "horizontal" | "vertical";
  children?: FlowNode[];
  metadata?: Record<string, unknown>;
}

type Status = "idle" | "busy" | "completed" | "error" | "blocked";
```

### Terminal-Bench Types

| Type | Description | Size |
|------|-------------|------|
| `tb-root` | Root container for TB view | 280×80 |
| `tb-controls` | Start/Stop/Config panel | 260×100 |
| `tb-timeline` | Horizontal run history | 200×60 |
| `tb-run-summary` | Collapsed run (pass rate) | 160×70 |
| `tb-run-expanded` | Expanded run with tasks | 280×100 |
| `tb-task` | Individual task result | 240×50 |

### MechaCoder Types

| Type | Description | Size |
|------|-------------|------|
| `mechacoder` | Agent process node | 282×100 |
| `repo` | Repository being worked | 240×80 |
| `task` | Task from .openagents | 240×80 |
| `phase` | Golden Loop phase | 160×40 |

## Visual Language

### Color Palette

**Status Colors:**
| Status | Color | Hex |
|--------|-------|-----|
| Idle | Gray | `#6b7280` |
| Busy | Blue | `#3b82f6` |
| Completed | Green | `#22c55e` |
| Error | Red | `#ef4444` |
| Blocked | Amber | `#f59e0b` |

**Node Themes:**
```typescript
const themes = {
  "tb-root": { fill: "#0f1a12", stroke: "rgba(34, 197, 94, 0.35)" },
  "tb-run-summary": { fill: "#141017", stroke: "rgba(59, 130, 246, 0.3)" },
  "tb-task": { fill: "#0f1117", stroke: "rgba(100, 116, 139, 0.25)" },
  "mechacoder": { fill: "#1a1625", stroke: "rgba(139, 92, 246, 0.5)" },
};
```

### Typography

- **Node Labels**: 14px semi-bold, white
- **Subtitles**: 11px regular, gray-400
- **Metrics**: 12px monospace, status-colored
- **Feed Output**: 12px monospace, green/blue/gray by source

### Grid & Canvas

- **Grid**: 48px spacing, subtle gray lines (`rgba(100, 116, 139, 0.08)`)
- **Background**: Dark (`#0a0a0f`)
- **Zoom Range**: 0.25x to 2.0x
- **Pan**: Drag to pan, momentum-based

## Terminal-Bench Integration

### State Model

```typescript
interface TBFlowState {
  runs: readonly TBRunWithPath[];      // Past runs (metadata)
  currentRunId: string | null;          // Live run ID
  currentTaskId: string | null;         // Currently executing task
  expandedRunIds: ReadonlySet<string>;  // Which runs show details
  currentTasks?: ReadonlyMap<string, TBTaskResult>;
}
```

### Run Persistence

Runs are saved to `.openagents/tb-runs/` (gitignored):

```
.openagents/tb-runs/
├── 20241204-tb-103045-abc123.json
├── 20241204-tb-142312-def456.json
└── ...
```

Each file contains:
```typescript
interface TBRunFile {
  meta: TBRunMeta;           // Quick-loadable header
  tasks: TBTaskResult[];     // Per-task results
  trajectory?: Trajectory;   // Full ATIF trajectory (optional)
}
```

### Tree Structure

```
tb-root (Terminal-Bench)
├── tb-controls (Start/Stop)
└── tb-timeline (Run History)
    ├── tb-run-summary (Run 1: 85%)
    ├── tb-run-expanded (Run 2: LIVE)
    │   ├── tb-task (regex-log ✓)
    │   ├── tb-task (git-branch ✗)
    │   └── tb-task (file-search ⏳)
    └── tb-run-summary (Run 3: 70%)
```

### Output Display

Agent output streams in ATIF-formatted lines:

```css
.atif-line.tool-call { color: #3b82f6; }     /* Blue for tool calls */
.atif-line.observation { color: #22c55e; }   /* Green for results */
.atif-line.error { color: #ef4444; }         /* Red for errors */
```

Tokens aggregate on the same line until a newline character, then start a new line.

## APM Widget

Displays agent velocity metrics:

```
┌─────────────────────────────────────┐
│  APM: 18.97  ▲ 4.2x faster          │
│  Session: 12.3 APM | 47 actions     │
│  1h: 12.9 | 6h: 20.1 | 24h: 12.3    │
└─────────────────────────────────────┘
```

**APM = (messages + tool_calls) / duration_minutes**

| APM Range | Color | Performance |
|-----------|-------|-------------|
| 0-5 | Gray | Baseline |
| 5-15 | Blue | Active |
| 15-30 | Green | High velocity |
| 30+ | Gold | Elite |

## WebSocket Protocol

### Message Flow

```
Agent → ws://localhost:4242 → HUD Server → RPC → Mainview
```

### Key Message Types

**Session Lifecycle:**
- `session_start` - New session with ID
- `session_complete` - Session finished

**Task Flow:**
- `task_selected` - Task picked for work
- `task_decomposed` - Broken into subtasks
- `subtask_start/complete/failed` - Subtask lifecycle

**Terminal-Bench:**
- `tb_run_start/complete` - Run lifecycle
- `tb_task_start/progress/output/complete` - Task lifecycle
- `tb_suite_info` - Suite loaded

**Streaming:**
- `text_output` - Agent reasoning text
- `tool_call` - Tool invocation
- `tool_result` - Tool response

**Metrics:**
- `apm_update` - Real-time APM
- `apm_snapshot` - Historical comparison

## Interaction Patterns

### Canvas Navigation

| Action | Input |
|--------|-------|
| Pan | Click + drag |
| Zoom | Scroll wheel |
| Reset view | Double-click |

### Node Interaction

| Action | Input |
|--------|-------|
| Expand run | Click run summary node |
| Collapse run | Click expanded run header |
| View task details | Click task node |
| Start TB run | Click controls → Start |

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `1` | Terminal-Bench view |
| `2` | Flow view |
| `Ctrl+L` | Load TB suite |
| `Ctrl+T` | Start TB run |
| `Ctrl+X` | Stop current run |

## Implementation Files

| File | Purpose |
|------|---------|
| `src/flow/model.ts` | Core types |
| `src/flow/layout.ts` | Layout engine |
| `src/flow/tb-map.ts` | TB tree builder |
| `src/flow-host-svg/render.ts` | SVG renderer |
| `src/mainview/index.ts` | Host controller |
| `src/mainview/index.css` | Styles |
| `src/tbench-hud/persistence.ts` | Run storage |
| `src/tbench-hud/emit.ts` | TB event emitter |
| `src/hud/protocol.ts` | Message types |
| `src/hud/emit.ts` | HUD emitter |

## Related Documentation

- [TERMINAL-BENCH.md](./TERMINAL-BENCH.md) - TB protocol details
- [HUD-EVENTS.md](./HUD-EVENTS.md) - Event mapping reference
- [APM.md](./APM.md) - APM metrics specification
- [flow.md](./flow.md) - Flow editor spec v1
