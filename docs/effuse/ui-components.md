# UI Components & Layout Guide

> Guide for agents working on the OpenAgents frontend, covering available components, layout patterns, and styling conventions.

## Overview

The OpenAgents UI is built with:
- **Effuse Widget System** - Effect-based reactive widgets
- **Tailwind CSS** - Utility-first styling (built-in to Effuse)
- **Simple Layouts** - Clean, minimal structure for Terminal Bench
- **HUD Components** - Advanced visualizations (grids, SVGs, flow charts)

## Current UI State (December 2025)

### TerminalBench Homepage

We are **rebuilding the homepage from scratch** with a simple, clean layout focused on Terminal Bench functionality.

**Current Layout:**
```
┌─────────────┬─┬──────────────────────┐
│   Sidebar   │ │     Main Area        │
│   (260px)   │ │     (flex-1)         │
│             │ │                      │
│             │ │                      │
│             │ │                      │
│             │ │                      │
└─────────────┴─┴──────────────────────┘
```

**File:** `src/mainview/index.html`

```html
<div class="flex h-screen">
  <!-- Sidebar (260px) -->
  <aside class="w-[260px] flex-shrink-0 bg-zinc-950">
    <div class="h-full p-4">
      <!-- Sidebar content goes here -->
    </div>
  </aside>

  <!-- Border (#262626) -->
  <div class="w-px flex-shrink-0" style="background-color: #262626;"></div>

  <!-- Main Area -->
  <main class="flex-1 bg-zinc-950">
    <div class="h-full p-4">
      <!-- Main content goes here -->
    </div>
  </main>
</div>
```

**Key Points:**
- Simple SidebarLayout pattern
- Fixed 260px sidebar
- 1px border with `#262626` color
- Main area takes remaining space
- All existing widgets commented out (in migration)

### Next Steps

**Immediate Goal:** Create a sidebar widget for loading HuggingFace dataset trajectories
- Display available trajectory datasets
- Select and load trajectories
- Preview trajectory metadata
- Integration with ATIF Details widget

## Effuse Widget System

### What is Effuse?

Effuse is our custom widget framework built on Effect-TS:
- **Reactive State** - Effect-based state management
- **Event Handling** - Type-safe event system
- **Socket Integration** - Real-time WebSocket updates
- **Composable** - Widgets can be mounted anywhere

### Widget Structure

```typescript
import { Effect } from "effect"
import { html } from "../template/html.js"
import type { Widget } from "../widget/types.js"

export interface MyWidgetState {
  count: number
  collapsed: boolean
}

export type MyWidgetEvent =
  | { type: "increment" }
  | { type: "toggleCollapse" }

export const MyWidget: Widget<MyWidgetState, MyWidgetEvent> = {
  id: "my-widget",

  initialState: () => ({
    count: 0,
    collapsed: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      return html`
        <div class="rounded-xl border border-zinc-800 bg-zinc-950 p-4">
          <h3 class="text-sm font-bold text-zinc-100">My Widget</h3>
          <div class="text-zinc-400">${state.count}</div>
          <button data-action="increment">Increment</button>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        if (action === "increment") {
          Effect.runFork(ctx.emit({ type: "increment" }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "increment":
          yield* ctx.state.update((s) => ({ ...s, count: s.count + 1 }))
          break
        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break
      }
    }),

  subscriptions: (ctx) => {
    // Optional: Subscribe to socket messages
    return []
  },
}
```

### Mounting Widgets

**In HTML:**
```html
<div id="my-widget-container"></div>
```

**In TypeScript:**
```typescript
import { mountWidgetById } from "../effuse/index.js"
import { MyWidget } from "./my-widget.js"

yield* mountWidgetById(MyWidget, "my-widget-container")
```

## Tailwind CSS Styling

### Colors

**Background:**
- `bg-zinc-950` - Primary background (darkest)
- `bg-zinc-900` - Secondary background
- `bg-zinc-800` - Tertiary background

**Text:**
- `text-zinc-100` - Primary text (lightest)
- `text-zinc-200` - Secondary text
- `text-zinc-400` - Muted text
- `text-zinc-500` - Disabled text

**Borders:**
- `border-zinc-800` - Primary borders
- `border-zinc-700` - Secondary borders
- Custom: `#262626` for sidebar border

**Accent Colors:**
- `text-emerald-400` / `bg-emerald-900/40` - Success/passed
- `text-red-400` / `bg-red-900/40` - Error/failed
- `text-amber-400` / `bg-amber-900/40` - Warning/timeout
- `text-violet-400` / `bg-violet-900/40` - Info/tools
- `text-blue-400` / `bg-blue-900/40` - User/primary

### Common Patterns

**Card/Panel:**
```html
<div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
  <!-- Content -->
</div>
```

**Collapsible Header:**
```html
<div class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer bg-zinc-900/40">
  <h3 class="text-sm font-bold font-mono text-zinc-100">Title</h3>
  <span class="text-zinc-500">${collapsed ? "▼" : "▲"}</span>
</div>
```

**Badge:**
```html
<span class="text-xs px-2 py-0.5 rounded bg-emerald-900/40 text-emerald-300 border border-emerald-700/50 font-mono">
  Label
</span>
```

**Button:**
```html
<button class="px-3 py-1.5 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-sm font-mono transition-colors">
  Click Me
</button>
```

### Typography

**Font Stack:**
- `font-mono` - Berkeley Mono for code/data
- Default - System sans-serif for UI text

**Sizes:**
- `text-xs` - 0.75rem (12px) - Metadata, labels
- `text-sm` - 0.875rem (14px) - Body text
- `text-base` - 1rem (16px) - Headings
- `text-lg` - 1.125rem (18px) - Large headings

## HUD Components

### Available HUD Elements

**Grid Visualizations:**
- Task grids with status indicators
- Category trees with expand/collapse
- Sortable tables with column headers

**SVG Elements:**
- Status icons (✓, ✗, ⏱, ⚠, ▶)
- Flow chart connectors
- Progress indicators
- Timeline visualizations

**Flow Charts:**
- Trajectory step flows
- Agent decision trees
- Subtask hierarchies

**Panes:**
- Split panes with resize handles
- Tabbed panes for multiple views
- Floating panels with drag/drop
- Collapsible sections

### Example: Status Grid

```html
<div class="grid grid-cols-3 gap-4">
  <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
    <div class="text-zinc-500 mb-1">Pass Rate</div>
    <div class="text-2xl font-bold text-emerald-400">85%</div>
  </div>
  <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
    <div class="text-zinc-500 mb-1">Tasks</div>
    <div class="text-2xl font-bold text-zinc-200">24/30</div>
  </div>
  <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
    <div class="text-zinc-500 mb-1">Duration</div>
    <div class="text-2xl font-bold text-zinc-200">5m 23s</div>
  </div>
</div>
```

### Example: Flow Chart

```html
<div class="relative">
  <!-- Nodes -->
  <div class="absolute top-0 left-0 w-32 h-12 bg-blue-900/40 border border-blue-700/50 rounded flex items-center justify-center text-xs">
    User Input
  </div>
  <div class="absolute top-20 left-0 w-32 h-12 bg-violet-900/40 border border-violet-700/50 rounded flex items-center justify-center text-xs">
    Agent Process
  </div>
  <div class="absolute top-40 left-0 w-32 h-12 bg-emerald-900/40 border border-emerald-700/50 rounded flex items-center justify-center text-xs">
    Result
  </div>

  <!-- Connectors (SVG) -->
  <svg class="absolute inset-0 pointer-events-none">
    <line x1="64" y1="12" x2="64" y2="20" stroke="#3b82f6" stroke-width="2"/>
    <line x1="64" y1="32" x2="64" y2="40" stroke="#8b5cf6" stroke-width="2"/>
  </svg>
</div>
```

## Existing Widgets

### Terminal Bench Widgets

**TB Controls** (`src/effuse/widgets/tb-controls.ts`)
- Suite loading and path input
- Run execution controls
- Progress bar with pass/fail counts
- Duration timer (HH:MM:SS)
- Task filtering (difficulty, search)

**TB Results** (`src/effuse/widgets/tb-results.ts`)
- Run summary with pass rate
- Per-task results table (sortable)
- Outcome filtering
- Difficulty badges
- Token/duration metrics

**TB Learning** (`src/effuse/widgets/tb-learning.ts`)
- FM learning features display
- Skills used/learned tracking
- Memory and reflexion metrics
- Learning flags indicators
- Run summary stats

**TB Output** (`src/effuse/widgets/tb-output.ts`)
- Live streaming output
- Source filtering (agent/verification/system)
- Line numbers toggle
- Auto-scroll control
- Copy to clipboard

**Category Tree** (`src/effuse/widgets/category-tree.ts`)
- Task organization by category
- Expand/collapse categories
- Task selection checkboxes
- Status icons and badges
- Pass/fail counts per category

**ATIF Details** (`src/effuse/widgets/atif-details.ts`)
- Step-by-step trajectory viewer
- Accordion expansion
- Tool calls display (function + args)
- Observations/results
- Source badges (user/agent/system)

### System Widgets

**APM Widget** (`src/effuse/widgets/apm-widget.ts`)
- Actions per minute tracking
- Real-time metrics updates
- Historical comparison
- Efficiency ratios

**Trajectory Pane** (`src/effuse/widgets/trajectory-pane.ts`)
- Recent runs list
- TB + ATIF unified view
- Run selection
- Delete/refresh controls

**MC Tasks** (`src/effuse/widgets/mc-tasks.ts`)
- Ready tasks from `.openagents/tasks.jsonl`
- Priority and type badges
- Task assignment
- Collapse/expand

**Container Panes** (`src/effuse/widgets/container-panes.ts`)
- Sandbox execution logs
- Per-task container output
- stdout/stderr separation
- Exit status display

## Socket Integration

Widgets can subscribe to real-time WebSocket messages from the desktop server.

### Message Types

**Terminal Bench:**
- `tb_run_start` - Run begins
- `tb_task_start` - Task starts
- `tb_task_output` - Streaming output
- `tb_task_complete` - Task finishes
- `tb_run_complete` - Run finishes
- `tb_learning_metrics` - Learning features update
- `tb_learning_summary` - Run learning summary

**ATIF:**
- `atif_trajectory_start` - Trajectory begins
- `atif_step` - Step recorded
- `atif_trajectory_complete` - Trajectory finishes

**Container:**
- `container_start` - Container execution starts
- `container_output` - Streaming container output
- `container_complete` - Container execution finishes

**APM:**
- `apm_update` - Real-time metrics
- `apm_snapshot` - Historical snapshot

### Subscription Pattern

```typescript
subscriptions: (ctx) => {
  const socket = Effect.map(SocketServiceTag, (s) => s)

  return [
    pipe(
      Stream.unwrap(Effect.map(socket, (s) => s.getMessages())),
      Stream.filter((msg): msg is HudMessage => msg.type.startsWith("tb_")),
      Stream.map((msg) =>
        Effect.gen(function* () {
          if (msg.type === "tb_task_complete") {
            yield* ctx.state.update((s) => ({
              ...s,
              completedTasks: s.completedTasks + 1,
            }))
          }
        })
      )
    ),
  ]
}
```

## Layout Patterns

### SidebarLayout (Current)

**Use Case:** Clean, focused UI with navigation sidebar
- Fixed-width sidebar (260px)
- Border separator
- Flexible main area

### Split Panes

**Use Case:** Resizable sections for detailed views
```html
<div class="flex h-full">
  <div class="w-1/3 border-r border-zinc-800">Left</div>
  <div class="flex-1">Right</div>
</div>
```

### Tab Panels

**Use Case:** Multiple views in same space
```html
<div class="border-b border-zinc-800 flex gap-2 px-4">
  <button class="px-3 py-2 border-b-2 border-blue-500">Tab 1</button>
  <button class="px-3 py-2 border-b-2 border-transparent">Tab 2</button>
</div>
```

### Grid Layout

**Use Case:** Dashboard with multiple metrics
```html
<div class="grid grid-cols-2 gap-4 p-4">
  <div>Widget 1</div>
  <div>Widget 2</div>
  <div>Widget 3</div>
  <div>Widget 4</div>
</div>
```

## Development Workflow

### Creating a New Widget

1. **Create widget file** (`src/effuse/widgets/my-widget.ts`)
   - Define state interface
   - Define event types
   - Implement widget object
   - Use Tailwind for all styling

2. **Create test file** (`src/effuse/widgets/my-widget.test.ts`)
   - Test initial state
   - Test rendering
   - Test event handling
   - Test socket subscriptions

3. **Export from index** (`src/effuse/index.ts`)
   ```typescript
   export { MyWidget } from "./widgets/my-widget.js"
   ```

4. **Mount in mainview** (`src/mainview/effuse-main.ts`)
   ```typescript
   yield* mountWidgetById(MyWidget, "my-widget-container")
   ```

5. **Add container to HTML** (`src/mainview/index.html`)
   ```html
   <div id="my-widget-container"></div>
   ```

6. **Rebuild bundle**
   ```bash
   bun build src/mainview/effuse-main.ts --outfile src/mainview/effuse-main.js
   ```

### Testing Widgets

```bash
# Run widget tests
SKIP_WEBVIEW_TESTS=1 bun test src/effuse/widgets/my-widget.test.ts

# Run all widget tests
SKIP_WEBVIEW_TESTS=1 bun test src/effuse/widgets/
```

### Styling Guidelines

**DO:**
- ✅ Use Tailwind utility classes
- ✅ Use semantic color names (emerald for success, red for error)
- ✅ Use consistent spacing (p-4, gap-4)
- ✅ Use mono font for code/data
- ✅ Use rounded corners (rounded-xl, rounded-lg)

**DON'T:**
- ❌ Write custom CSS (unless absolutely necessary)
- ❌ Use arbitrary hex colors (use Tailwind colors)
- ❌ Mix different spacing scales
- ❌ Use inline styles (except for specific values like #262626)

## Roadmap

### Next Priorities

1. **HuggingFace Trajectory Loader Widget**
   - Sidebar widget for loading HF datasets
   - Display available trajectory datasets
   - Select and load trajectories
   - Preview metadata
   - Integration with ATIF Details

2. **Simple TB Run View**
   - Main area displays current TB run
   - Real-time task status
   - Output streaming
   - Results summary

3. **Trajectory Viewer**
   - Step-by-step replay
   - Tool call inspection
   - Agent state visualization
   - Timeline scrubbing

### Future Enhancements

- Drag-and-drop widget placement
- Custom dashboard layouts
- Widget configuration UI
- Keyboard shortcuts
- Dark/light theme toggle
- Export/import layouts

## Resources

**Documentation:**
- [Effuse Widget System](../src/effuse/README.md)
- [HUD Protocol](../src/hud/protocol.ts)
- [Terminal Bench User Stories](./testing/terminal-bench-user-stories.md)

**Examples:**
- [TB Controls Widget](../src/effuse/widgets/tb-controls.ts)
- [ATIF Details Widget](../src/effuse/widgets/atif-details.ts)
- [TB Results Widget](../src/effuse/widgets/tb-results.ts)

**Tailwind:**
- [Tailwind Documentation](https://tailwindcss.com/docs)
- [Tailwind Browser CDN](https://tailwindcss.com/blog/tailwindcss-browser)

---

**Note for AI Agents:** When editing frontend code, always use Tailwind CSS for styling, follow the Effuse widget pattern, and ensure changes are tested before committing. The UI is currently in a migration phase from complex layout to simple SidebarLayout for Terminal Bench focus.
