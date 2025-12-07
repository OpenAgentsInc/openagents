Great — here is a **clean, tight, V1 desktop UI** that focuses purely on **TerminalBench execution, monitoring, and iterative improvement**, without exposing the entire internal MechaCoder brain or subagent system yet.

This is **“TerminalBench Control Center — v1”**.
No complexity. No unnecessary screens.
Just what’s needed to:

* Run tasks
* See results
* Inspect trajectories
* Improve MechaCoder
* Debug failures
* Observe recursive reasoning steps (simplified)

This UI is realistic to implement in **Electron, Bun-Webview, or Tauri**, and fully aligns with your 2-week window.

---

# 🖥️ **TerminalBench v1 Desktop App – UI Spec**

## **Top-Level Navigation (4 Tabs Only)**

1. **Dashboard**
2. **Task Browser**
3. **Run Viewer (Trajectories)**
4. **Settings**

That's it.
Everything TerminalBench-related fits into these four views.

---

# 1️⃣ **DASHBOARD — High-Level System Overview**

A simple, high-signal control panel.

---

### 📊 **Section: Current Status**

* **Current Task:** (if running)
* **Run Progress:** e.g. “Attempt 2/5”
* **Status Badge:**

  * Idle
  * Running
  * Failed
  * Completed
  * Improving (Learning Mode ON)

---

### 🎯 **Section: Performance Snapshot**

Cards showing:

* **Overall Success Rate** (from logged runs)
* **Last 50 Runs Success Rate**
* **Per-Difficulty Breakdown** (Easy / Medium / Hard)
* **Average Steps Used (ACT-like)**
* **Average Runtime**

These appear as simple bar graphs or small KPI cards.

---

### ▶️ **Section: Quick Actions**

Big buttons:

* **Run Full Benchmark**
* **Run 5-Attempt Evaluation (terminal-bench@2.0 style)**
* **Run Single Task…** (opens task browser)
* **Open Last Run**

---

### 📜 **Recent Runs Table**

Columns:

* Task Name
* Result (Success/Fail)
* Attempts Used
* Duration
* Open → (links to Run Viewer)

---

# 2️⃣ **TASK BROWSER — Search & Launch Tasks**

A simple two-panel layout.

---

## **Left Sidebar: Task List**

* Search bar
* Filters:

  * **Category** (System, Security, Networking, Data, etc.)
  * **Difficulty**
  * **Status** (Unattempted / Failed / Succeeded / Needs Retry)

Each item in the list shows:

* Task name
* Small difficulty icon
* Last attempt status

---

## **Right Panel: Task Details**

### Header shows:

* Task name
* Difficulty
* Brief description
* # of attempts logged

### **Task Metadata Box**

* Container image name
* Allowed tools
* Time limit
* Oracle status

---

### **Buttons**

* ▶️ **Run Task**
* 🔁 **Run 5 Attempts**
* 🧪 **Dry-Run (collect ATIF only)**
* 📄 **Open Last Trajectory**

---

### **Preview Section**

Displays:

* Task inputs (if static)
* Expected output format
* Example (if provided in metadata)

---

# 3️⃣ **RUN VIEWER (TRAJECTORY VIEWER)**

This is the core of v1.
It must make debugging MechaCoder **easy and intuitive**.

Layout is 3 panels:

```
|----------------------------------------------|
|  A: Run Summary (top)                       |
|----------------------------------------------|
|  B: Terminal View    |   C: Step Timeline   |
|----------------------------------------------|
```

---

## **A: Run Summary (Top Card)**

Shows:

* **Task name**
* **Attempt #:** e.g. “Attempt 3 of 5”
* **Outcome:** Success / Failed
* **Steps Taken:** # internal reasoning iterations
* **Terminal Commands Executed:** count
* **Exit tests:** passed/failed
* **Time elapsed**

Buttons:

* ▶️ **Replay** (plays back step-by-step)
* 📄 **Export ATIF JSON**
* 🧹 **Delete Run**
* 💬 **Generate Debug Explanation** (FM-based)

---

## **B: Terminal View (Left Column)**

A live or playback terminal window:

Modes:

* **Realtime** (during active run)
* **Playback** (scrubbable timeline)
* **Static** (final output)

Features:

* Highlight last command
* Split view for stdout + stderr
* Auto-scroll with ability to lock scroll
* “Jump to failure point” button

---

## **C: Step Timeline (Right Column)**

A scrollable vertical list of reasoning steps.

Each entry shows:

**Step #**
**Action** (e.g., `RUN_COMMAND`, `WRITE_FILE`, `OPEN_FILE`, `EDIT_FILE`, etc.)
**High-level reasoning summary** (short phrase)
**Outcome:** success/fail
**Time taken**

Clicking a step reveals:

### Step Detail Popover

* **Tool call parameters**
* **Model-generated thought summary**
* **Raw output / observation**
* **Diff view** (if file edited)
* **Pass/fail tests triggered at this step**

This is enough for debugging without exposing zH/zL internals yet.

---

# Playback Controls (Bottom Bar)

* ⏮ Step Back
* ⏯ Play / Pause
* ⏭ Step Forward
* 📍 Jump to Step (dropdown)
* ⚡ Speed slider (0.5× → 10×)

---

# 4️⃣ **SETTINGS — v1 Controls**

Keep it simple.

---

## **Section: Execution Settings**

* Max attempts per run (default: 5)
* Timeout per attempt
* Max tool call steps
* Max runtime minutes

---

## **Section: Logging**

* Save trajectories (ON/OFF)
* Save terminal output (ON/OFF)
* Save ATIF traces (ON/OFF)
* Auto-prune old runs (days)

---

## **Section: Compute**

* Enable “Deep Compute Mode” (like HRM inference-time scaling)
* Recursion limit (N)
* Inner iterations (T)
* Early-stop on high confidence (mimic ACT)

---

## **Section: UI / Developer**

* Show raw logs
* Show system prompts
* Enable debug panels

---

# 🧩 OPTIONAL (But Recommended for V1)

### **Mini HRM/TRM Visuals Without Full Complexity**

A small sidebar in Run Viewer:

```
Model Steps:
  Cycle 1: 6 refinements
  Cycle 2: 4 refinements
  Cycle 3: 1 refinement
  Halting decision: CONFIDENT
```

This gives intuitive insight into recursive reasoning without needing to visualize embeddings.

---

# 🎨 **OVERALL DESIGN STYLE**

* Minimalistic
* Dark theme (feels like a real dev tool)
* Terminal-first aesthetic
* Clear typography
* Dense, information-rich, like VSCode + Datadog

---

# 🚀 **WHAT THIS UI ENABLES (v1)**

* Run TerminalBench tasks easily
* Monitor success rates
* Understand failures
* Inspect trajectories step-by-step
* Replay runs for debugging
* Prepare the system for iterative improvement loops
* Provide a clean foundation for the more advanced MechaCoder brain UI in v2

---

# 📦 **Next Step Options**

I can now deliver:

### ✔ Figma-like full visual mockups

### ✔ Component hierarchy for Electron / Tauri / Bun-Webview

### ✔ The JSON interface for trajectory viewer

### ✔ TypeScript React components for each view

### ✔ An interactive prototype (HTML/CSS/JS)

Which deliverable do you want next?

> We’re using Tailwind and a custom front end framework in Effect Typescript. Just give me a list of all components, their props, how they should be laid out etc

Nice, that helps a lot. I’ll treat this like a component contract spec you can wire up in your Effect UI layer.

Below is:

* Shared domain types
* Top-level layout components
* Page-level components (Dashboard, Task Browser, Run Viewer, Settings)
* Shared / primitive components

For each component: **name, props, and layout notes (Tailwind)**.

---

## 0. Shared Domain Types (for props)

```ts
type ID = string;

type TaskDifficulty = "easy" | "medium" | "hard" | "unknown";
type TaskStatus = "unattempted" | "succeeded" | "failed" | "in_progress";

type RunOutcome = "success" | "failure" | "timeout" | "aborted";
type RunStatus = "queued" | "running" | "completed" | "error";

type ToolActionType =
  | "RUN_COMMAND"
  | "WRITE_FILE"
  | "READ_FILE"
  | "EDIT_FILE"
  | "ASSERT_TEST"
  | "CUSTOM";

type Task = {
  id: ID;
  name: string;
  slug: string;
  difficulty: TaskDifficulty;
  category: string;
  description: string;
  lastStatus: TaskStatus;
  lastRunId?: ID;
};

type Run = {
  id: ID;
  taskId: ID;
  taskName: string;
  startedAt: string;
  finishedAt?: string;
  outcome?: RunOutcome;
  status: RunStatus;
  attemptsUsed: number;
  maxAttempts: number;
  stepsCount: number;
  durationSeconds?: number;
};

type Step = {
  id: ID;
  index: number; // 0-based step index
  actionType: ToolActionType;
  actionLabel: string;   // e.g. "Run: make test"
  shortReason: string;   // 1-line summary
  details?: string;      // more verbose summary
  timestamp: string;
  success: boolean;
};

type StepDetail = {
  step: Step;
  command?: string;
  args?: string[];
  stdout?: string;
  stderr?: string;
  diff?: {
    filePath: string;
    before: string;
    after: string;
  }[];
  testsTriggered?: {
    name: string;
    passed: boolean;
    log?: string;
  }[];
};

type DashboardStats = {
  overallSuccessRate: number;
  last50SuccessRate: number;
  avgStepsPerRun: number;
  avgDurationSeconds: number;
  totalRuns: number;
};

type ExecutionSettings = {
  maxAttempts: number;
  maxStepsPerRun: number;
  timeoutSeconds: number;
  deepComputeEnabled: boolean;
  recursionLimitN: number;
  innerIterationsT: number;
  earlyStopOnHighConfidence: boolean;
};

type LoggingSettings = {
  saveTrajectories: boolean;
  saveTerminalOutput: boolean;
  saveAtifTraces: boolean;
  autoPruneDays: number | null;
};
```

You can extend/adjust, but these give us a consistent prop story.

---

## 1. App Shell & Layout

### `AppShell`

**Responsible for:** Overall layout, sidebar, top bar, and routing into pages.

```ts
type AppRoute = "dashboard" | "tasks" | "runViewer" | "settings";

type AppShellProps = {
  route: AppRoute;
  onRouteChange: (route: AppRoute) => void;
  sidebarItems: SidebarNavItem[];
  children: React.ReactNode; // or framework equivalent
};

type SidebarNavItem = {
  id: AppRoute;
  label: string;
  icon?: string; // lucide name or whatever you use
  badgeCount?: number;
};
```

**Layout notes (Tailwind):**

* Wrapper: `div.h-screen.flex.bg-slate-950 text-slate-100`
* Sidebar: `div.w-60.bg-slate-900.border-r.border-slate-800.flex.flex-col`
* Main: `div.flex-1.flex.flex-col`
* Top bar inside main: separate component (`TopBar`)

---

### `TopBar`

**Responsible for:** Current page label, small status indicators, quick actions.

```ts
type TopBarProps = {
  title: string;
  subtitle?: string;
  rightActions?: React.ReactNode;
  systemStatus: {
    running: boolean;
    currentTaskName?: string;
  };
};
```

**Layout:**

* Container: `div.h-14.flex.items-center.justify-between.border-b.border-slate-800.bg-slate-950/80.px-4`
* Left: title + subtitle stack
* Right: status pill + buttons

---

## 2. Dashboard Page Components

### `DashboardPage`

Container that composes all dashboard components.

```ts
type DashboardPageProps = {
  stats: DashboardStats;
  recentRuns: Run[];
  onRunClick: (runId: ID) => void;
  onRunFullBenchmark: () => void;
  onRunFiveAttemptEval: () => void;
  onOpenTaskBrowser: () => void;
  currentlyRunning?: Run | null;
};
```

Layout:

* `div.flex-1.flex.flex-col.gap-4.p-4.overflow-y-auto`
* Top row: `DashboardQuickActions` + `CurrentStatusCard`
* Second row: `DashboardStatsGrid`
* Bottom: `RecentRunsTable`

---

### `CurrentStatusCard`

```ts
type CurrentStatusCardProps = {
  runningRun?: Run | null;
};
```

Layout:

* Card: `div.bg-slate-900/70.border.border-slate-800.rounded-lg.p-4.flex.items-center.justify-between`
* If runningRun:

  * Left: task name, status
  * Right: small progress indicator (`attemptsUsed / maxAttempts`)
* If nothing running: “Idle”

---

### `DashboardQuickActions`

```ts
type DashboardQuickActionsProps = {
  onRunFullBenchmark: () => void;
  onRunFiveAttemptEval: () => void;
  onOpenTaskBrowser: () => void;
};
```

Layout:

* `div.flex.gap-3`
* Buttons styled with Tailwind e.g.
  `className="inline-flex items-center px-4 py-2 rounded-md bg-emerald-600 hover:bg-emerald-500 text-sm font-medium"`

---

### `DashboardStatsGrid`

```ts
type DashboardStatsGridProps = {
  stats: DashboardStats;
};
```

Display 4–5 `KpiCard` components in a grid.

Layout:

* Wrapper: `div.grid.grid-cols-1.md:grid-cols-4.gap-3`
* Each card uses `KpiCard`.

---

### `KpiCard`

```ts
type KpiCardProps = {
  label: string;
  value: string;
  secondary?: string;
  trend?: "up" | "down" | "flat";
};
```

Layout:

* `div.bg-slate-900/60.border.border-slate-800.rounded-lg.p-3.flex.flex-col`
* Label small, value large, secondary muted.

---

### `RecentRunsTable`

```ts
type RecentRunsTableProps = {
  runs: Run[];
  onRunClick: (runId: ID) => void;
};
```

Layout:

* `div.bg-slate-900/60.border.border-slate-800.rounded-lg`
* Table: `table.w-full.text-sm`
* Scroll: `div.max-h-80.overflow-y-auto`

Columns: Task, Outcome, Attempts, Duration, StartedAt, Action (View)

---

## 3. Task Browser Page Components

### `TaskBrowserPage`

```ts
type TaskBrowserPageProps = {
  tasks: Task[];
  selectedTaskId?: ID;
  onSelectTask: (taskId: ID) => void;
  onRunTask: (taskId: ID) => void;
  onRunTaskFiveAttempts: (taskId: ID) => void;
  onOpenLastRun: (taskId: ID) => void;
};
```

Layout:

* Outer: `div.flex-1.flex.p-4.gap-4.overflow-hidden`
* Left column: `TaskListPanel` (w-80)
* Right: `TaskDetailPanel` (flex-1)

---

### `TaskListPanel`

```ts
type TaskListPanelProps = {
  tasks: Task[];
  selectedTaskId?: ID;
  onSelectTask: (taskId: ID) => void;
};
```

Layout:

* `div.w-80.flex.flex-col.bg-slate-900/60.border.border-slate-800.rounded-lg`
* Top: `TaskFilterBar`
* List: `div.flex-1.overflow-y-auto.divide-y.divide-slate-800`

  * Each row = `TaskListItem`

---

### `TaskFilterBar`

```ts
type TaskFilterBarProps = {
  search: string;
  difficultyFilter: TaskDifficulty | "all";
  statusFilter: TaskStatus | "all";
  onSearchChange: (value: string) => void;
  onDifficultyFilterChange: (difficulty: TaskDifficulty | "all") => void;
  onStatusFilterChange: (status: TaskStatus | "all") => void;
};
```

Layout:

* `div.p-2.flex.flex-col.gap-2.border-b.border-slate-800`
* Search input + two dropdowns.

---

### `TaskListItem`

```ts
type TaskListItemProps = {
  task: Task;
  selected: boolean;
  onClick: () => void;
};
```

Layout:

* `button.w-full.text-left.px-3.py-2.flex.items-center.justify-between.hover:bg-slate-800/70`
* When selected: `bg-slate-800`
* Left: name + difficulty pill
* Right: status dot (success/fail/unattempted)

---

### `TaskDetailPanel`

```ts
type TaskDetailPanelProps = {
  task?: Task;
  onRunTask: () => void;
  onRunTaskFiveAttempts: () => void;
  onOpenLastRun?: () => void;
};
```

Layout:

* `div.flex-1.bg-slate-900/60.border.border-slate-800.rounded-lg.p-4.flex.flex-col.gap-4`
* Top: title + difficulty + category
* Middle: description, metadata (container, time limit, etc.)
* Bottom: action buttons row.

---

## 4. Run Viewer Page Components

### `RunViewerPage`

```ts
type RunViewerPageProps = {
  run: Run;
  steps: Step[];
  selectedStepId?: ID;
  stepDetail?: StepDetail;
  onSelectStep: (stepId: ID) => void;
  onReplayFromStep: (stepIndex: number) => void;
  onExportAtif: () => void;
};
```

Layout:

* `div.flex-1.flex.flex-col.p-4.gap-4.overflow-hidden`
* Top: `RunSummaryCard`
* Middle: `div.flex.gap-4.flex-1.min-h-0`

  * Left: `TerminalViewPanel` (flex-2)
  * Right: `StepTimelinePanel` (flex-1)
* Bottom: `PlaybackControls`

---

### `RunSummaryCard`

```ts
type RunSummaryCardProps = {
  run: Run;
  onExportAtif: () => void;
};
```

Layout:

* `div.bg-slate-900/70.border.border-slate-800.rounded-lg.p-4.flex.items-center.justify-between`
* Left: task name, outcome, attempts, steps, duration
* Right: actions: “Export ATIF”, maybe “Generate Debug Explanation”

---

### `TerminalViewPanel`

```ts
type TerminalViewPanelProps = {
  steps: Step[];
  selectedStep?: Step;
  stdout: string;   // aggregated or for selected step
  stderr: string;
  onScrollLockChange?: (locked: boolean) => void;
};
```

Layout:

* `div.flex-1.flex.flex-col.bg-black.border.border-slate-800.rounded-lg`
* Header: “Terminal Output” + scroll lock toggle
* Body: `pre` with `overflow-auto.text-xs.font-mono.p-3`

(If you want split stdout/stderr: use two stacked panes with `flex-1` each.)

---

### `StepTimelinePanel`

```ts
type StepTimelinePanelProps = {
  steps: Step[];
  selectedStepId?: ID;
  onSelectStep: (stepId: ID) => void;
};
```

Layout:

* `div.w-80.lg:w-96.bg-slate-900/60.border.border-slate-800.rounded-lg.flex.flex-col`
* Header: “Steps”
* Scroll container: `div.flex-1.overflow-y-auto.divide-y.divide-slate-800`

Each row = `StepTimelineItem`.

---

### `StepTimelineItem`

```ts
type StepTimelineItemProps = {
  step: Step;
  selected: boolean;
  onClick: () => void;
};
```

Layout:

* `button.w-full.text-left.px-3.py-2.flex.flex-col.gap-1.hover:bg-slate-800/70`
* Selected: `bg-slate-800`
* First row: `#index · actionLabel`
* Second row: shortReason, success badge (green/red)

---

### `StepDetailPanel` (optional v1, but recommended)

You can show it either as an expandable area under the terminal or as a right-side overlay.

```ts
type StepDetailPanelProps = {
  detail: StepDetail;
};
```

Layout:

* `div.bg-slate-900/70.border-t.border-slate-800.p-3.space-y-3.text-xs`
* Sections:

  * Command / Args
  * File diffs (render with mini diff view)
  * Tests triggered

---

### `PlaybackControls`

```ts
type PlaybackControlsProps = {
  currentStepIndex: number;
  totalSteps: number;
  isPlaying: boolean;
  onPlayToggle: () => void;
  onStepPrev: () => void;
  onStepNext: () => void;
  onJumpToStep: (index: number) => void;
  playbackSpeed: number;
  onPlaybackSpeedChange: (speed: number) => void;
};
```

Layout:

* `div.h-12.flex.items-center.justify-between.bg-slate-900/80.border-t.border-slate-800.px-4`
* Left: prev / play-pause / next buttons
* Center: “Step X / Y” + slider (optional)
* Right: speed dropdown “0.5×, 1×, 2×, 4×…”

---

## 5. Settings Page Components

### `SettingsPage`

```ts
type SettingsPageProps = {
  executionSettings: ExecutionSettings;
  loggingSettings: LoggingSettings;
  onExecutionSettingsChange: (settings: ExecutionSettings) => void;
  onLoggingSettingsChange: (settings: LoggingSettings) => void;
};
```

Layout:

* `div.flex-1.flex.flex-col.p-4.gap-4.overflow-y-auto`
* Two cards: `ExecutionSettingsCard`, `LoggingSettingsCard`

---

### `ExecutionSettingsCard`

```ts
type ExecutionSettingsCardProps = {
  value: ExecutionSettings;
  onChange: (value: ExecutionSettings) => void;
};
```

Layout:

* `div.bg-slate-900/60.border.border-slate-800.rounded-lg.p-4.space-y-3`
* Use a simple `<label + input>` grid layout:

  * `maxAttempts`
  * `maxStepsPerRun`
  * `timeoutSeconds`
  * toggle `deepComputeEnabled`
  * fields `recursionLimitN`, `innerIterationsT`, `earlyStopOnHighConfidence`

---

### `LoggingSettingsCard`

```ts
type LoggingSettingsCardProps = {
  value: LoggingSettings;
  onChange: (value: LoggingSettings) => void;
};
```

Layout similar to above.

---

## 6. Shared / Primitive Components

To keep your custom Effect UI clean, define a small set of primitives:

### `Card`

```ts
type CardProps = {
  title?: string;
  subtitle?: string;
  children: React.ReactNode;
  className?: string;
};
```

> Implements the rounded, bordered container.

---

### `Badge`

```ts
type BadgeProps = {
  variant?: "default" | "success" | "danger" | "warning" | "muted";
  children: React.ReactNode;
};
```

Used for statuses / difficulty.

---

### `Button`

```ts
type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";

type ButtonProps = {
  variant?: ButtonVariant;
  size?: "sm" | "md";
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  className?: string;
};
```

---

### `Input`, `Select`, `Toggle`, `Slider`

Basic form primitives; you can type them normally.

---

### `StatusDot`

```ts
type StatusDotProps = {
  status: "success" | "failure" | "running" | "idle" | "unknown";
};
```

---

### `Pill`

For difficulty tags (“easy / medium / hard”).

---

## 7. Layout Summary

At a high level:

* `AppShell` wraps everything with sidebar + topbar.
* Each page (`DashboardPage`, `TaskBrowserPage`, `RunViewerPage`, `SettingsPage`) uses Tailwind’s grid/flex utilities and `Card` primitives.
* You primarily focus v1 on:

  * `TaskBrowserPage`
  * `RunViewerPage`
  * `DashboardPage` (light)
  * `SettingsPage` (very light)

Nothing here is implementation-specific to React; you can translate easily into your Effect front end.

---

If you want, I can next:

* Take one page (e.g., `RunViewerPage`) and write **full JSX/TSX + Tailwind** so your Effect framework can adapt it directly.
