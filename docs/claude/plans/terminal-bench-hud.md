# Terminal-Bench HUD Visualization Plan

## Overview

Add real-time Terminal-Bench visualization to the existing HUD infrastructure. Visualize TB runs as they happen using the flow graph / node-based SVG system, with ability to trigger runs from the UI.

**Approach**: Start minimal (widget + events), expand to full view mode later.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     tbench-local.ts                              │
│   - Emits HudMessages during run lifecycle                       │
│   - tb_run_start, tb_task_start, tb_task_complete, etc.         │
└─────────────────────────────────────────────────────────────────┘
                              │
                         WebSocket
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    HudServer (port 4242)                         │
│   src/bun/index.ts                                               │
│   - Receives TB events                                           │
│   - Handles run trigger requests from UI                         │
│   - Spawns tbench-local.ts subprocess                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                         Electrobun RPC
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Mainview                                   │
│   src/mainview/index.ts                                          │
│   - View mode switcher: Flow | TB                                │
│   - TB widget (compact) or TB view (full)                        │
│   - Real-time progress, output streaming                         │
│   - Run trigger UI                                               │
└─────────────────────────────────────────────────────────────────┘
```

## HUD Message Types

Add to `src/hud/protocol.ts`:

```typescript
// TB Run Lifecycle
interface TBRunStartMessage {
  type: "tb_run_start";
  runId: string;
  suiteName: string;
  suiteVersion: string;
  totalTasks: number;
  taskIds: string[];
}

interface TBRunCompleteMessage {
  type: "tb_run_complete";
  runId: string;
  passRate: number;
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  totalDurationMs: number;
}

// Task Progress
interface TBTaskStartMessage {
  type: "tb_task_start";
  runId: string;
  taskId: string;
  taskName: string;
  category: string;
  difficulty: "easy" | "medium" | "hard" | "expert";
  taskIndex: number;
  totalTasks: number;
}

interface TBTaskProgressMessage {
  type: "tb_task_progress";
  runId: string;
  taskId: string;
  phase: "setup" | "agent" | "verification";
  currentTurn?: number;
  elapsedMs: number;
}

interface TBTaskOutputMessage {
  type: "tb_task_output";
  runId: string;
  taskId: string;
  text: string;
  source: "agent" | "verification" | "system";
}

interface TBTaskCompleteMessage {
  type: "tb_task_complete";
  runId: string;
  taskId: string;
  outcome: "success" | "failure" | "timeout" | "error";
  durationMs: number;
  turns: number;
  tokens: number;
}

// Suite Info (for UI)
interface TBSuiteInfoMessage {
  type: "tb_suite_info";
  name: string;
  version: string;
  tasks: Array<{ id: string; name: string; category: string; difficulty: string }>;
}

// UI Requests (bidirectional)
interface TBRunRequestMessage {
  type: "tb_run_request";
  suitePath: string;
  taskIds?: string[];
  timeout?: number;
  maxTurns?: number;
}
```

## Critical Files

| File | Changes |
|------|---------|
| `src/hud/protocol.ts` | Add 8 TB message types to HudMessage union |
| `src/cli/tbench-local.ts` | Import HudClient, emit events at lifecycle points |
| `src/mainview/index.ts` | Add TBState, renderTBWidget(), handle TB messages |
| `src/bun/index.ts` | Add RPC handler for startTBRun, spawn subprocess |
| `src/tbench-view/` | **NEW** - Full TB view mode (Phase 2) |

---

## Detailed Task Breakdown

### Phase 1: Protocol & Events (~175 lines)

#### Task: oa-tbhud-01 - Add TB message types to HUD protocol
**Priority**: P1
**Type**: feature
**Description**: Add Terminal-Bench specific message types to `src/hud/protocol.ts`

**Acceptance Criteria**:
- Add 8 new interfaces: TBRunStartMessage, TBRunCompleteMessage, TBTaskStartMessage, TBTaskProgressMessage, TBTaskOutputMessage, TBTaskCompleteMessage, TBSuiteInfoMessage, TBRunRequestMessage
- Add all to HudMessage union type
- Add type guards: `isTBRunStart()`, `isTBTaskComplete()`, etc.

**Files**: `src/hud/protocol.ts`

---

#### Task: oa-tbhud-02 - Emit HUD events from tbench-local.ts
**Priority**: P1
**Type**: feature
**Description**: Modify `src/cli/tbench-local.ts` to emit HUD messages during run lifecycle

**Acceptance Criteria**:
- Import and initialize HudClient at start
- Emit `tb_run_start` after loading suite (~line 545)
- Emit `tb_task_start` before each task (~line 559)
- Emit `tb_task_progress` during agent execution (hook into onOutput)
- Emit `tb_task_complete` after each task (~line 567)
- Emit `tb_run_complete` at end (~line 580)
- Silent failure if HUD server not running

**Files**: `src/cli/tbench-local.ts`

**Dependencies**: oa-tbhud-01

---

#### Task: oa-tbhud-03 - Create TB emit helpers
**Priority**: P1
**Type**: feature
**Description**: Create `src/tbench-hud/emit.ts` with helper functions for emitting TB events (like `src/hud/emit.ts` for APM)

**Acceptance Criteria**:
- `createTBEmitter(hudClient)` factory function
- `emitRunStart(suite, taskIds)`
- `emitTaskStart(task, index, total)`
- `emitTaskProgress(taskId, phase, turn, elapsed)`
- `emitTaskOutput(taskId, text, source)`
- `emitTaskComplete(taskId, result)`
- `emitRunComplete(summary)`
- Auto-generates runId using nanoid

**Files**: `src/tbench-hud/emit.ts` (new)

**Dependencies**: oa-tbhud-01

---

### Phase 2: UI Widget (~100 lines)

#### Task: oa-tbhud-04 - Add TB state management to mainview
**Priority**: P1
**Type**: feature
**Description**: Add TBState interface and state variable to `src/mainview/index.ts`

**Acceptance Criteria**:
- Define TBState interface with: isRunning, suiteName, totalTasks, tasks Map, currentTaskId, passed, failed, passRate
- Initialize `let tbState: TBState`
- Handle all TB message types in handleHudMessage()
- Update state on each event
- Call render() after state changes

**Files**: `src/mainview/index.ts`

**Dependencies**: oa-tbhud-01

---

#### Task: oa-tbhud-05 - Render TB progress widget
**Priority**: P1
**Type**: feature
**Description**: Add `renderTBWidget()` function to display compact TB progress overlay

**Acceptance Criteria**:
- Fixed position SVG overlay (like APM widget, below it)
- Shows: suite name, progress bar, pass/fail counts, current task
- Progress bar fills as tasks complete
- Green for pass, red for fail
- Shows "Idle" when no run active
- Include in main render() function

**Files**: `src/mainview/index.ts`

**Dependencies**: oa-tbhud-04

---

### Phase 3: Run Triggering (~50 lines)

#### Task: oa-tbhud-06 - Add RPC handler for TB run triggering
**Priority**: P2
**Type**: feature
**Description**: Add bidirectional RPC to `src/bun/index.ts` to trigger TB runs from UI

**Acceptance Criteria**:
- Add `startTBRun` request handler to RPC schema
- Accept: suitePath, taskIds (optional), timeout, maxTurns
- Spawn `bun src/cli/tbench-local.ts` as subprocess
- Return { started: true, runId } or { started: false, error }
- Add `loadTBSuite` request to load suite info for UI

**Files**: `src/bun/index.ts`

**Dependencies**: oa-tbhud-02

---

#### Task: oa-tbhud-07 - Add keyboard shortcut for demo run
**Priority**: P2
**Type**: feature
**Description**: Add Ctrl+T keyboard shortcut to trigger a demo TB run from mainview

**Acceptance Criteria**:
- Listen for Ctrl+T in mainview
- Call RPC startTBRun with demo config (single easy task like "regex-log")
- Show toast/notification that run started
- Can be expanded to full UI later

**Files**: `src/mainview/index.ts`

**Dependencies**: oa-tbhud-06

---

### Phase 4: Full TB View Mode (~400 lines)

#### Task: oa-tbhud-08 - Add view mode switcher
**Priority**: P2
**Type**: feature
**Description**: Add ability to switch between Flow view and TB view in mainview

**Acceptance Criteria**:
- Add viewMode state: "flow" | "tbench"
- Add view switcher UI (tabs or keyboard shortcut)
- Ctrl+1 for Flow, Ctrl+2 for TB
- Conditionally render Flow graph or TB view based on mode
- Persist view mode preference

**Files**: `src/mainview/index.ts`, `src/mainview/view-switcher.ts` (new)

---

#### Task: oa-tbhud-09 - Create TB view HTML/CSS structure
**Priority**: P2
**Type**: feature
**Description**: Create the TB view layout with header, main panel, and sidebar

**Acceptance Criteria**:
- Suite header with name, version, task count
- Main panel area for progress/output
- Sidebar for task list
- Stats bar at bottom
- Dark theme matching existing HUD style
- Responsive layout

**Files**: `src/tbench-view/index.html` (new), `src/tbench-view/index.css` (new)

**Dependencies**: oa-tbhud-08

---

#### Task: oa-tbhud-10 - Implement category/task tree component
**Priority**: P2
**Type**: feature
**Description**: Create collapsible tree view showing categories and tasks with status

**Acceptance Criteria**:
- Group tasks by category
- Collapsible category sections
- Show task status icons: pending (○), running (◐), passed (✓), failed (✗)
- Show difficulty badge per task
- Click task to select for detail view
- Show category pass rate

**Files**: `src/tbench-view/components/category-tree.ts` (new)

**Dependencies**: oa-tbhud-09

---

#### Task: oa-tbhud-11 - Implement real-time output viewer
**Priority**: P2
**Type**: feature
**Description**: Create streaming output viewer for current task

**Acceptance Criteria**:
- Auto-scrolling text area
- Show agent output in real-time (from tb_task_output events)
- Color-code by source (agent=white, verification=yellow, system=gray)
- Max buffer of 1000 lines
- Copy to clipboard button
- Clear on task change

**Files**: `src/tbench-view/components/output-viewer.ts` (new)

**Dependencies**: oa-tbhud-09

---

#### Task: oa-tbhud-12 - Implement run progress panel
**Priority**: P2
**Type**: feature
**Description**: Create progress panel showing overall run status and current task

**Acceptance Criteria**:
- Overall progress bar with percentage
- Current task info: name, category, difficulty, phase, turn count
- Elapsed time display
- Pass/fail/timeout/error counters
- Abort button
- Auto-updates from HUD events

**Files**: `src/tbench-view/components/progress-panel.ts` (new)

**Dependencies**: oa-tbhud-09

---

### Phase 5: Run Configuration UI (~150 lines)

#### Task: oa-tbhud-13 - Implement run trigger dialog
**Priority**: P3
**Type**: feature
**Description**: Create modal dialog for configuring and starting TB runs

**Acceptance Criteria**:
- Suite selector dropdown
- Task selection: all, by category, by difficulty, or manual selection
- Task multi-select with search
- Options: timeout, max turns
- Baseline comparison selector (from history)
- Start/Cancel buttons
- Validates inputs before starting

**Files**: `src/tbench-view/components/run-trigger.ts` (new)

**Dependencies**: oa-tbhud-06, oa-tbhud-09

---

#### Task: oa-tbhud-14 - Implement run history storage
**Priority**: P3
**Type**: feature
**Description**: Store and retrieve historical TB run results

**Acceptance Criteria**:
- Store runs in `.openagents/tbench/runs/` as JSON
- Filename: `bench-{timestamp}.json`
- Include: runId, suite, config, results, summary
- Load history on startup
- Limit to last 50 runs
- Cleanup old runs

**Files**: `src/tbench-hud/storage.ts` (new)

---

#### Task: oa-tbhud-15 - Implement run comparison view
**Priority**: P3
**Type**: feature
**Description**: Show comparison between current run and baseline

**Acceptance Criteria**:
- Pass rate delta with arrow
- Avg duration delta
- Improved/regressed/unchanged task lists
- Side-by-side task results
- Delta highlighting (green=better, red=worse)

**Files**: `src/tbench-view/components/comparison-chart.ts` (new)

**Dependencies**: oa-tbhud-14

---

### Phase 6: Polish & Testing

#### Task: oa-tbhud-16 - Add TB node visualization in flow graph
**Priority**: P3
**Type**: feature
**Description**: Optionally show TB tasks as nodes in the flow graph (alternative to separate view)

**Acceptance Criteria**:
- New node type "tbTask" in flow model
- TB theme colors (green accent)
- Build TB flow tree from state
- Show: suite root → category nodes → task nodes
- Status-based styling
- Can toggle between flow-integrated and separate view

**Files**: `src/flow/model.ts`, `src/flow-host-svg/render.ts`, `src/tbench-view/flow-adapter.ts` (new)

---

#### Task: oa-tbhud-17 - Write tests for TB HUD components
**Priority**: P3
**Type**: task
**Description**: Add tests for TB state management and event handling

**Acceptance Criteria**:
- Test TB message type guards
- Test state reducer for all event types
- Test emit helpers
- Test RPC handlers
- Mock HudClient for testing

**Files**: `src/tbench-hud/__tests__/*.test.ts` (new)

---

#### Task: oa-tbhud-18 - Document TB HUD visualization
**Priority**: P3
**Type**: task
**Description**: Document how to use the TB visualization features

**Acceptance Criteria**:
- Add to docs/tbench/README.md
- Explain view modes
- Keyboard shortcuts
- How to trigger runs from UI
- Screenshots of UI
- Troubleshooting section

**Files**: `docs/tbench/README.md`

---

## Implementation Order

**Week 1 - Core (P1)**:
1. oa-tbhud-01 - Protocol types
2. oa-tbhud-03 - Emit helpers
3. oa-tbhud-02 - tbench-local.ts integration
4. oa-tbhud-04 - Mainview state
5. oa-tbhud-05 - Widget render

**Week 2 - Triggering + View (P2)**:
6. oa-tbhud-06 - RPC handler
7. oa-tbhud-07 - Keyboard shortcut
8. oa-tbhud-08 - View switcher
9. oa-tbhud-09 - TB view structure
10. oa-tbhud-10 - Category tree
11. oa-tbhud-11 - Output viewer
12. oa-tbhud-12 - Progress panel

**Week 3 - Polish (P3)**:
13. oa-tbhud-13 - Run dialog
14. oa-tbhud-14 - History storage
15. oa-tbhud-15 - Comparison view
16. oa-tbhud-16 - Flow integration
17. oa-tbhud-17 - Tests
18. oa-tbhud-18 - Docs

## Success Criteria

- [ ] Can see TB widget in HUD when running `tbench-local.ts`
- [ ] Widget shows real-time progress (tasks completing, pass/fail)
- [ ] Can trigger demo run with Ctrl+T
- [ ] Can switch to full TB view mode with Ctrl+2
- [ ] Can configure and start runs from UI
- [ ] Can compare runs against baseline
