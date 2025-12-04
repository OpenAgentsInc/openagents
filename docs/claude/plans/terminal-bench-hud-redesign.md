# Terminal Bench HUD Redesign Plan

## Overview

Transform the Terminal Bench UI from floating panes with token-by-token streaming into a unified grid-based canvas with proper ATIF-formatted output, persistent run history, and expandable run nodes.

## Architecture Summary

```
┌──────────────────────────────────────────────────────────────┐
│                     Grid Background Canvas                    │
│                                                              │
│  ┌─────────────────┐  ┌──────────────────────────────────┐  │
│  │ TB Controls     │  │ Past Runs Timeline (Nodes)        │  │
│  │ ─────────────── │  │                                   │  │
│  │ Suite: [path]   │  │ ┌──────┐ ┌──────┐ ┌──────┐       │  │
│  │ [Load][Start]   │  │ │Run 1 │→│Run 2 │→│Run 3 │       │  │
│  │ Status: Ready   │  │ │85%✓  │ │92%✓  │ │LIVE  │       │  │
│  │                 │  │ └──────┘ └──────┘ └──────┘       │  │
│  │ Progress:       │  │      ↓ (expanded)                 │  │
│  │ 3/89 (3.4%)    │  │ ┌────────────────────────────┐    │  │
│  │ ████░░░░░░░░░░ │  │ │ task-1  ✓ 45s              │    │  │
│  └─────────────────┘  │ │ task-2  ✗ 120s             │    │  │
│                       │ │ task-3  ⏳ running...       │    │  │
│  ┌─────────────────┐  │ └────────────────────────────┘    │  │
│  │ Live Output     │  └──────────────────────────────────┘  │
│  │ (ATIF-formatted)│                                        │
│  │ ─────────────── │                                        │
│  │ [Agent] Fix...  │                                        │
│  │ [Tool] Edit...  │                                        │
│  │ [Obs] Success   │                                        │
│  └─────────────────┘                                        │
└──────────────────────────────────────────────────────────────┘
```

## Part 1: Fix Token-by-Token Streaming

### Problem
The TB output viewer shows tokens one-by-one instead of aggregated lines. This is because `tbench-hud/emit.ts` sends each chunk as a separate `tb_task_output` message.

### Solution
Create an output buffer that aggregates tokens into lines before emitting, using patterns from `tool-log-buffer.ts`.

### Files to Modify
- `src/tbench-hud/emit.ts` - Add line buffering to taskOutput method
- `src/tbench-hud/output-buffer.ts` - New file for buffer logic

### Implementation
```typescript
// src/tbench-hud/output-buffer.ts
export interface TBOutputBuffer {
  taskId: string;
  source: TBOutputSource;
  buffer: string;
  lastFlush: number;
}

// Flush on newline or every 500ms
export const appendAndFlush = (
  buffer: TBOutputBuffer,
  chunk: string,
  emit: (line: string) => void
): void => {
  buffer.buffer += chunk;

  // Flush complete lines
  const lines = buffer.buffer.split('\n');
  if (lines.length > 1) {
    for (let i = 0; i < lines.length - 1; i++) {
      emit(lines[i]);
    }
    buffer.buffer = lines[lines.length - 1];
  }

  // Force flush if buffer is large or stale
  if (buffer.buffer.length > 500 || Date.now() - buffer.lastFlush > 500) {
    if (buffer.buffer.length > 0) {
      emit(buffer.buffer);
      buffer.buffer = '';
      buffer.lastFlush = Date.now();
    }
  }
};
```

---

## Part 2: ATIF-Formatted Output Display

### Problem
Tool calls and observations are shown as raw text instead of structured ATIF format.

### Solution
Parse streaming output into ATIF-compatible steps and render with distinct styling.

### Files to Modify
- `src/mainview/index.ts` - Update output rendering logic
- `src/mainview/index.css` - Add ATIF message styles

### Output Line Types
```css
.atif-line.agent { color: #e5e5e5; }        /* Agent reasoning */
.atif-line.tool-call { color: #3b82f6; }    /* [Tool] Read file.ts */
.atif-line.observation { color: #22c55e; }  /* [Obs] Content... */
.atif-line.error { color: #ef4444; }        /* [Error] Failed... */
.atif-line.system { color: #6b7280; }       /* [System] Setup... */
```

---

## Part 3: TB Run Persistence

### Storage Format
Single JSON file per run with ATIF trajectory + metadata header:

```
.openagents/tb-runs/              (gitignored by default)
├── 20251204-tb-103245-abc123.json
├── 20251204-tb-142301-def456.json
└── ...
```

### Run File Schema
```typescript
interface TBRunFile {
  // Metadata header (for quick loading without parsing trajectory)
  meta: {
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
  };

  // Task-level results
  tasks: Array<{
    id: string;
    name: string;
    category: string;
    difficulty: string;
    outcome: "success" | "failure" | "timeout" | "error";
    durationMs: number;
    turns: number;
    tokens: number;
  }>;

  // Full ATIF trajectory for detailed analysis
  trajectory: ATIFTrajectory;
}
```

### Files to Create
- `src/tbench-hud/persistence.ts` - Save/load run files
- Update `.gitignore` to include `.openagents/tb-runs/`

### Persistence Implementation
```typescript
// src/tbench-hud/persistence.ts
export const saveTBRun = async (
  run: TBRunFile,
  baseDir = ".openagents/tb-runs"
): Promise<string> => {
  const date = new Date().toISOString().slice(0, 10).replace(/-/g, "");
  const time = new Date().toISOString().slice(11, 19).replace(/:/g, "");
  const shortId = run.meta.runId.slice(-6);
  const filename = `${date}-tb-${time}-${shortId}.json`;
  const filepath = join(baseDir, filename);

  await Bun.file(filepath).write(JSON.stringify(run, null, 2));
  return filepath;
};

export const loadTBRuns = async (
  baseDir = ".openagents/tb-runs"
): Promise<TBRunMeta[]> => {
  // Load only metadata for quick listing
  const files = readdirSync(baseDir).filter(f => f.endsWith(".json"));
  return Promise.all(files.map(async f => {
    const content = await Bun.file(join(baseDir, f)).json();
    return { ...content.meta, filepath: join(baseDir, f) };
  }));
};
```

---

## Part 4: TB Run Nodes on Canvas

### Node Types
Add new flow node types for TB runs:

1. **`tb-run-summary`** - Compact node showing run pass rate
2. **`tb-run-expanded`** - Expanded view with task list
3. **`tb-task`** - Individual task within expanded run

### Files to Modify
- `src/flow/model.ts` - Add TB node type definitions
- `src/flow/tb-map.ts` - New file: build TB flow tree from run history
- `src/flow-host-svg/render.ts` - Add TB node themes and rendering
- `src/mainview/index.ts` - Load and display TB runs as nodes

### Node Hierarchy
```
Root: "Terminal Bench" (horizontal)
├── Controls Node (static, always visible)
├── Run Timeline (horizontal)
│   ├── Run 1 [tb-run-summary]
│   │   └── (expanded) [tb-run-expanded]
│   │       ├── Task 1 [tb-task]
│   │       ├── Task 2 [tb-task]
│   │       └── ...
│   ├── Run 2 [tb-run-summary]
│   └── Current Run [tb-run-summary, status=running]
└── Live Output Pane (static, bottom-left)
```

### Node Themes
```typescript
const TB_NODE_THEMES = {
  "tb-run-summary": {
    fill: "#0a1520",
    stroke: "rgba(34, 197, 94, 0.3)",
    header: "rgba(34, 197, 94, 0.1)",
    accent: (status) => status === "running" ? "#3b82f6" :
                        status === "success" ? "#22c55e" : "#ef4444",
    glow: "rgba(34, 197, 94, 0.2)",
  },
  "tb-task": {
    fill: "#0a0f15",
    stroke: "rgba(255, 255, 255, 0.1)",
    accent: (outcome) => outcome === "success" ? "#22c55e" :
                         outcome === "failure" ? "#ef4444" :
                         outcome === "timeout" ? "#f59e0b" : "#8b5cf6",
  },
};
```

### Expand/Collapse Interaction
```typescript
// In mainview/index.ts
const expandedRuns = new Set<string>();

const toggleRunExpand = (runId: string) => {
  if (expandedRuns.has(runId)) {
    expandedRuns.delete(runId);
  } else {
    expandedRuns.add(runId);
  }
  rebuildTBFlowTree();
  rerenderCanvas();
};

// On node click
svgElement.addEventListener('click', (e) => {
  const node = findNodeAtPoint(e.clientX, e.clientY);
  if (node?.type === 'tb-run-summary') {
    toggleRunExpand(node.metadata.runId);
  }
});
```

---

## Part 5: Remove Demo Content, Default to TB Mode

### Files to Modify
- `src/mainview/index.ts` - Comment out MechaCoder demo tree, default to TB view

### Changes
1. Set default view mode to "tb" instead of "flow"
2. Comment out `buildMechaCoderFlowTree()` call
3. Replace with `buildTBFlowTree()` for run history
4. Hide MechaCoder-specific UI elements

---

## Part 6: Consolidate HUD Design Doc

### Create New Doc
`docs/hud/HUD-DESIGN.md` - Definitive HUD design document

### Content Structure
1. **Vision**: Factorio-inspired agent factory management
2. **Architecture**: Grid canvas, flow nodes, floating panes
3. **Terminal Bench UI**: Run controls, history nodes, output viewer
4. **Visual Language**: Colors, typography, status indicators
5. **Interaction Patterns**: Pan/zoom, expand/collapse, keyboard shortcuts

### Deprecation
Add note to old Factorio docs pointing to new location:
```markdown
> **Note**: This document is deprecated. See [docs/hud/HUD-DESIGN.md](../hud/HUD-DESIGN.md) for the current design.
```

---

## Implementation Tasks (for .openagents/tasks.jsonl)

### Phase 1: Core Infrastructure (Priority 0)

1. **oa-tb-output-buffer** - Create output buffer for line aggregation
   - `src/tbench-hud/output-buffer.ts`
   - Fix token-by-token streaming

2. **oa-tb-persistence** - Implement run persistence
   - `src/tbench-hud/persistence.ts`
   - `.openagents/tb-runs/` storage

3. **oa-tb-atif-format** - ATIF output formatting
   - Update `src/mainview/index.ts` output rendering
   - Add ATIF line styles to CSS

### Phase 2: Canvas Integration (Priority 1)

4. **oa-tb-flow-nodes** - Add TB node types to flow system
   - `src/flow/model.ts` - type definitions
   - `src/flow/tb-map.ts` - tree builder
   - `src/flow-host-svg/render.ts` - rendering

5. **oa-tb-expand-collapse** - Implement expand/collapse for run nodes
   - Click handler in mainview
   - State management for expanded runs

6. **oa-tb-default-view** - Default to TB mode, remove demo content
   - Comment out MechaCoder flow tree
   - Set default view to "tb"

### Phase 3: Documentation (Priority 2)

7. **oa-hud-design-doc** - Create consolidated HUD design doc
   - `docs/hud/HUD-DESIGN.md`
   - Deprecation notes in old docs

---

## Critical Files Summary

| File | Action | Purpose |
|------|--------|---------|
| `src/tbench-hud/output-buffer.ts` | Create | Line buffering |
| `src/tbench-hud/emit.ts` | Modify | Use buffer in taskOutput |
| `src/tbench-hud/persistence.ts` | Create | Save/load runs |
| `src/flow/tb-map.ts` | Create | Build TB flow tree |
| `src/flow/model.ts` | Modify | Add TB node types |
| `src/flow-host-svg/render.ts` | Modify | TB node themes |
| `src/mainview/index.ts` | Modify | TB default, ATIF rendering |
| `src/mainview/index.css` | Modify | ATIF line styles |
| `docs/hud/HUD-DESIGN.md` | Create | Consolidated design doc |
| `.gitignore` | Modify | Add `.openagents/tb-runs/` |

---

## Testing Strategy

1. **Unit Tests**: Output buffer line aggregation
2. **Integration Tests**: Run persistence save/load
3. **Visual Tests**: Verify canvas renders TB nodes correctly
4. **E2E Tests**: Start TB run, verify output streaming and persistence
