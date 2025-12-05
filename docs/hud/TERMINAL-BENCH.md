# Terminal-Bench in the HUD

> **Terminal-Bench** - Benchmark suite for evaluating AI coding agents on real-world terminal tasks.

This document describes how Terminal-Bench integrates with the HUD WebSocket protocol for real-time visualization and control in the Electrobun desktop app.

## Overview

The Terminal-Bench HUD integration provides:
1. **Real-time progress tracking** - Watch tasks execute with live status updates
2. **Run controls** - Start, stop, and configure benchmark runs from the UI
3. **Task selection** - Load suites and select specific tasks to run
4. **Results visualization** - Pass/fail rates, timing, and comparison metrics

## Architecture

```
┌─────────────────┐     WebSocket      ┌─────────────────┐
│  tbench-local   │ ──────────────────▶│   HUD Server    │
│   (CLI runner)  │     port 4242      │  (Electrobun)   │
└─────────────────┘                    └────────┬────────┘
        │                                       │
        │ spawns                                │ RPC
        ▼                                       ▼
┌─────────────────┐                    ┌─────────────────┐
│  Claude Code    │                    │    Mainview     │
│   (subagent)    │                    │   (UI/SVG)      │
└─────────────────┘                    └─────────────────┘
```

### Components

- **`src/cli/tbench-local.ts`** - CLI runner that executes TB tasks and emits HUD events
- **`src/tbench-hud/emit.ts`** - TBEmitter factory for sending events to HUD
- **`src/bun/index.ts`** - Electrobun main process with RPC handlers
- **`src/mainview/index.ts`** - UI with TB state management and controls
- **`src/hud/protocol.ts`** - Message type definitions

## HUD Message Types

### Run Lifecycle

#### TB Run Start

Emitted when a benchmark run begins:

```typescript
interface TBRunStartMessage {
  type: "tb_run_start";
  runId: string;           // Unique run ID (e.g., "tb-20241204-abc123")
  suiteName: string;       // Suite name from JSON
  suiteVersion: string;    // Suite version
  totalTasks: number;      // Number of tasks to run
  taskIds: string[];       // Task IDs in execution order
  timestamp: string;       // ISO timestamp
}
```

#### TB Run Complete

Emitted when a benchmark run finishes:

```typescript
interface TBRunCompleteMessage {
  type: "tb_run_complete";
  runId: string;
  passRate: number;        // 0.0 - 1.0
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  totalDurationMs: number;
}
```

#### TB Run History Update

Pushed by the desktop server when run history changes (e.g., after a run completes) so the HUD can refresh without polling:

```typescript
interface TBRunHistoryMessage {
  type: "tb_run_history";
  runs: Array<{
    runId: string;
    suiteName: string;
    suiteVersion: string;
    timestamp: string;
    passRate: number;
    passed: number;
    failed: number;
    timeout: number;
    error: number;
    totalDurationMs: number;
    totalTokens: number;
    taskCount: number;
    filepath: string;
  }>;
}
```

### Task Lifecycle

#### TB Task Start

Emitted when a task begins execution:

```typescript
interface TBTaskStartMessage {
  type: "tb_task_start";
  runId: string;
  taskId: string;
  taskName: string;
  category: string;        // e.g., "file_operations", "git"
  difficulty: TBDifficulty; // "easy" | "medium" | "hard" | "expert"
  taskIndex: number;       // 0-based index in run
  totalTasks: number;
}
```

#### TB Task Progress

Emitted during task execution to show phase/turn updates:

```typescript
interface TBTaskProgressMessage {
  type: "tb_task_progress";
  runId: string;
  taskId: string;
  phase: TBTaskPhase;      // "setup" | "agent" | "verification"
  currentTurn?: number;    // Agent turn count
  elapsedMs: number;
}
```

#### TB Task Output

Emitted for streaming output from the agent:

```typescript
interface TBTaskOutputMessage {
  type: "tb_task_output";
  runId: string;
  taskId: string;
  text: string;
  source: TBOutputSource;  // "agent" | "verification" | "system"
}
```

#### TB Task Complete

Emitted when a task finishes:

```typescript
interface TBTaskCompleteMessage {
  type: "tb_task_complete";
  runId: string;
  taskId: string;
  outcome: TBTaskOutcome;  // "success" | "failure" | "timeout" | "error"
  durationMs: number;
  turns: number;
  tokens: number;
  verificationOutput?: string;
}
```

### Suite Info

#### TB Suite Info

Emitted when a suite is loaded (for UI display):

```typescript
interface TBSuiteInfoMessage {
  type: "tb_suite_info";
  name: string;
  version: string;
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: TBDifficulty;
  }>;
}
```

## Event Flow

```
User clicks "Load" in UI
         │
         ▼
    RPC: loadTBSuite(path)
         │
         ▼
    Parse suite JSON
         │
         ▼
    Return TBSuiteInfo to UI
         │
         ▼
    UI renders task selector
         │
         ▼
User selects tasks, clicks "Start"
         │
         ▼
    RPC: startTBRun(options)
         │
         ▼
    Spawn tbench-local.ts subprocess
         │
         ├──▶ tb_run_start ──────────▶ HUD displays "Running..."
         │
         │    For each task:
         │    ├──▶ tb_task_start ────▶ HUD shows current task
         │    ├──▶ tb_task_progress ─▶ HUD updates phase/turn
         │    ├──▶ tb_task_output ───▶ (future: streaming viewer)
         │    └──▶ tb_task_complete ─▶ HUD updates pass/fail count
         │
         └──▶ tb_run_complete ───────▶ HUD shows final results
```

## UI Controls

### Control Panel

Located in the top-right corner of the HUD:

```
┌─────────────────────────────────────┐
│ TERMINAL-BENCH              Ready   │
├─────────────────────────────────────┤
│ [./tasks/terminal-bench.json    ]   │
│                                     │
│ [Load]  [Start]  [Stop]             │
├─────────────────────────────────────┤
│ Suite Name (12 tasks)   [All][None] │
│ ┌─────────────────────────────────┐ │
│ │ ☑ task-1-name           EASY    │ │
│ │ ☑ task-2-name           MEDIUM  │ │
│ │ ☐ task-3-name           HARD    │ │
│ │ ...                             │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+L` | Load suite from path |
| `Ctrl+T` | Start run with selected tasks |
| `Ctrl+X` | Stop current run |

### Progress Widget (SVG Overlay)

Displays below the APM widget during runs:

```
┌─────────────────────────────────────┐
│ TB: Terminal-Bench           5/12   │
│ ████████████░░░░░░░░░░░░░░░  41.6%  │
│ ✓ 4  ✗ 1                            │
│ regex-log (agent) | Turn 15         │
└─────────────────────────────────────┘
```

## RPC Handlers

### loadTBSuite

Load and parse a Terminal-Bench suite JSON file:

```typescript
// Request
rpc.request.loadTBSuite("./tasks/terminal-bench.json")

// Response
{
  name: "Terminal-Bench 2.0",
  version: "2.0.0",
  tasks: [
    { id: "regex-log", name: "Regex Log Parser", category: "text", difficulty: "easy" },
    // ...
  ]
}
```

### startTBRun

Start a benchmark run:

```typescript
// Request
rpc.request.startTBRun({
  suitePath: "./tasks/terminal-bench.json",
  taskIds: ["regex-log", "git-branch"],  // Optional: specific tasks
  timeout: 3600,                          // Optional: seconds
  maxTurns: 300,                          // Optional
  outputDir: "./results/run-001",         // Optional
})

// Response
{ runId: "tb-20241204-abc123" }
```

### stopTBRun

Stop the currently running benchmark:

```typescript
// Request
rpc.request.stopTBRun()

// Response
{ stopped: true }
```

## Using the TBEmitter

For custom runners that want to emit TB events:

```typescript
import { createTBEmitter } from "../tbench-hud/emit.js";

const tbEmit = createTBEmitter();

// Start run
const runId = tbEmit.runStart(suiteInfo, taskIds);

// For each task
tbEmit.taskStart(task, index, total);
tbEmit.taskProgress(taskId, "agent", turn, elapsed);
tbEmit.taskOutput(taskId, "Agent output...", "agent");
tbEmit.taskComplete(taskId, result);

// End run
tbEmit.runComplete(summary);

// Clean up
tbEmit.close();
```

## State Management

The mainview maintains TB state for rendering:

```typescript
interface TBState {
  isRunning: boolean;
  runId: string | null;
  suiteName: string;
  suiteVersion: string;
  totalTasks: number;
  tasks: Map<string, TBTaskState>;
  currentTaskId: string | null;
  currentPhase: string | null;
  currentTurn: number;
  passed: number;
  failed: number;
  timeout: number;
  error: number;
  passRate: number;
  totalDurationMs: number;
}

interface TBTaskState {
  id: string;
  name: string;
  difficulty: string;
  category: string;
  status: "pending" | "running" | "passed" | "failed" | "timeout" | "error";
  durationMs?: number;
  turns?: number;
}
```

## Suite JSON Format

Terminal-Bench suites are JSON files:

```json
{
  "name": "Terminal-Bench 2.0",
  "version": "2.0.0",
  "description": "AI coding agent benchmark suite",
  "tasks": [
    {
      "id": "regex-log",
      "name": "Regex Log Parser",
      "category": "text_processing",
      "difficulty": "easy",
      "prompt": "Create a script that parses log files...",
      "setup": { "files": { "logs/app.log": "..." } },
      "verification": { "type": "script", "script": "verify.sh" }
    }
  ]
}
```

## Future Enhancements

Planned features (see `.openagents/tasks.jsonl` for tracking):

- **oa-8b6113**: Real-time output viewer with streaming text
- **oa-9aaa01**: Enhanced progress panel with elapsed time and abort
- **oa-bd4cc3**: Category/task tree view with status icons
- **oa-50dc74**: View mode switcher (Flow/TB views)
- **oa-f4ef9d**: Run history storage and retrieval
- **oa-b86a0a**: Baseline comparison view

## Related Documentation

- [HUD Events](./HUD-EVENTS.md) - General HUD WebSocket protocol
- [APM](./APM.md) - Actions Per Minute metrics
- [Flow Visualization](./flow.md) - MechaCoder flow graph
