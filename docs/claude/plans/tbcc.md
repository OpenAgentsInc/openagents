# TerminalBench Command Center - Implementation Plan

## Overview

Build a 4-tab Command Center in Effuse that consolidates TB execution, monitoring, and trajectory inspection. The existing HF trajectory browser widgets become a data source within the Run Viewer tab.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  tbcc-shell.ts (Tab navigation + event bus)                        │
├─────────────────────────────────────────────────────────────────────┤
│  Tab 1: Dashboard     │ Tab 2: Tasks   │ Tab 3: Runs  │ Tab 4: Settings │
│  ├── kpi-grid         │ ├── task-list  │ ├── run-list │ ├── exec-settings │
│  ├── quick-actions    │ └── task-detail│ └── run-detail│ └── log-settings  │
│  └── recent-runs      │                │              │              │
└─────────────────────────────────────────────────────────────────────┘
```

## File Structure

```
src/effuse/widgets/tb-command-center/
├── index.ts                    # Barrel export
├── types.ts                    # Shared types (TabId, Task, Run, Step)
├── tbcc-shell.ts               # Main shell widget (tab nav, status bar)
├── tbcc-dashboard.ts           # Dashboard page
├── tbcc-task-browser.ts        # Task browser page (list + detail panels)
├── tbcc-run-browser.ts         # Run browser page (list + detail panels)
└── tbcc-settings.ts            # Settings page
```

## Critical Files to Modify

| File | Changes |
|------|---------|
| `src/mainview/index.html` | Replace current layout with TBCC containers |
| `src/mainview/effuse-main.ts` | Mount TBCC shell, remove old HF widget mounting |
| `src/effuse/index.ts` | Export new TBCC widgets |

## Widgets to Reuse

| Existing Widget | Reuse In |
|----------------|----------|
| `hf-trajectory-list.ts` | Run browser list - adapt for both HF data + local TB runs |
| `hf-trajectory-detail.ts` | Run browser detail - step accordion pattern |
| `tb-controls.ts` | Task browser - suite loading, run control logic |
| `tb-output.ts` | Run detail - terminal output display pattern |

## Implementation Phases

### Phase 1: Shell + Dashboard

1. Create `types.ts` with shared types:
   - `TabId = "dashboard" | "tasks" | "runs" | "settings"`
   - `TBTask`, `TBRun`, `RunStep`, `DashboardStats`

2. Create `tbcc-shell.ts`:
   - State: `activeTab`, `systemStatus`, `sidebarCollapsed`
   - Render: Sidebar with tab nav + main content area
   - Events: `changeTab`, `toggleSidebar`

3. Create `tbcc-dashboard.ts`:
   - KPI grid: success rate, avg steps, avg duration, total runs
   - Quick actions: Run Benchmark, Run Single Task
   - Recent runs table (top 10)
   - Socket subscription for active run status

4. Update `index.html` and `effuse-main.ts` to mount shell

### Phase 2: Task Browser

5. Create `tbcc-task-browser.ts`:
   - Left panel: Task list from `tasks/terminal-bench-2.json`
   - Filters: difficulty, category, status (passed/failed/untried)
   - Right panel: Task detail + run buttons
   - Wire to `socket.startTBRun()` for execution

### Phase 3: Run Browser (HF + Local Data)

6. Create `tbcc-run-browser.ts`:
   - Left panel: Unified run list (HF trajectories + local `results/tb-*/`)
   - Data sources: `socket.getHFTrajectories()` + `socket.loadRecentTBRuns()`
   - Right panel: Run detail with step accordion (reuse hf-trajectory-detail pattern)
   - Terminal output panel (reuse tb-output pattern)

### Phase 4: Settings

7. Create `tbcc-settings.ts`:
   - Execution: maxAttempts, maxSteps, timeout
   - Logging: saveTrajectories, saveTerminalOutput, autoPrune
   - Persist to localStorage

## State Types

```typescript
// Shell
interface TBCCShellState {
  activeTab: TabId
  systemStatus: { isRunning: boolean; currentRunId: string | null }
  sidebarCollapsed: boolean
}

// Dashboard
interface TBCCDashboardState {
  stats: { successRate: number; avgSteps: number; avgDuration: number; totalRuns: number } | null
  recentRuns: TBRunSummary[]
  currentRun: { runId: string; taskName: string; status: string } | null
  loading: boolean
}

// Task Browser
interface TBCCTaskBrowserState {
  tasks: TBTask[]
  selectedTaskId: string | null
  searchQuery: string
  difficultyFilter: "all" | "easy" | "medium" | "hard"
  loading: boolean
}

// Run Browser
interface TBCCRunBrowserState {
  runs: TBRunSummary[]  // Unified: HF + local
  selectedRunId: string | null
  selectedRun: TBRunDetail | null  // Full trajectory
  dataSource: "all" | "hf" | "local"
  loading: boolean
}
```

## HTML Layout

```html
<div class="flex h-screen">
  <!-- Sidebar (260px) -->
  <aside id="tbcc-sidebar" class="w-[260px] bg-zinc-950 border-r border-zinc-800/60">
    <!-- Tab navigation rendered by tbcc-shell.ts -->
  </aside>

  <!-- Main Content -->
  <main id="tbcc-main" class="flex-1 bg-zinc-950">
    <!-- Tab content rendered by tbcc-shell.ts -->
  </main>
</div>
```

## Data Flow

1. **Dashboard stats**: `socket.loadRecentTBRuns()` → aggregate into KPIs
2. **Task list**: Load `tasks/terminal-bench-2.json` via socket RPC or fetch
3. **HF trajectories**: `socket.getHFTrajectories(offset, limit)` (existing RPC)
4. **Local runs**: `socket.loadRecentTBRuns()` (existing RPC)
5. **Run execution**: `socket.startTBRun({ taskIds })` → subscribe to `tb_*` messages
6. **Real-time updates**: Socket `getMessages()` stream → filter by `tb_run_start`, `tb_task_complete`, etc.

## Deliverables

- [ ] `src/effuse/widgets/tb-command-center/types.ts`
- [ ] `src/effuse/widgets/tb-command-center/tbcc-shell.ts`
- [ ] `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts`
- [ ] `src/effuse/widgets/tb-command-center/tbcc-task-browser.ts`
- [ ] `src/effuse/widgets/tb-command-center/tbcc-run-browser.ts`
- [ ] `src/effuse/widgets/tb-command-center/tbcc-settings.ts`
- [ ] `src/effuse/widgets/tb-command-center/index.ts`
- [ ] Updated `src/mainview/index.html`
- [ ] Updated `src/mainview/effuse-main.ts`
- [ ] Updated `src/effuse/index.ts`
