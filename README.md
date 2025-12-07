# OpenAgents

Your agent command center. (wip)

## Tech stack

- [Effect](https://effect.website/)
- [webview-bun](https://github.com/tr1ckydev/webview-bun)
- [Effuse](./src/effuse/README.md) - Effect-based reactive widget system
- [Tailwind CSS](https://tailwindcss.com/) - Utility-first styling

## Getting Started

1. Install dependencies:
   ```bash
   bun install
   ```

2. Run in development mode:
   ```bash
   bun start
   ```

3. Build for production:
   ```bash
   bun run build
   ```

## UI Development

The OpenAgents frontend is built with the **Effuse widget system** and styled with **Tailwind CSS**.

### Current UI: TerminalBench

We're building a simple, focused UI for Terminal Bench:
- **SidebarLayout** - 260px sidebar + main area
- **Effuse Widgets** - Reactive, composable UI components
- **Tailwind Styling** - Utility-first CSS (no custom stylesheets)
- **HUD Components** - Grids, SVGs, flow charts for visualizations

### Documentation

- **[UI Components & Layout Guide](./docs/effuse/ui-components.md)** - Complete guide for frontend development
- **[Effuse Widget System](./src/effuse/README.md)** - Widget architecture and patterns
- **[Terminal Bench User Stories](./docs/testing/terminal-bench-user-stories.md)** - Feature requirements

### Available Widgets

**Terminal Bench:**
- TB Controls - Suite loading, run execution
- TB Results - Summary stats and per-task results
- TB Learning - FM learning metrics
- TB Output - Live streaming output
- Category Tree - Task organization
- ATIF Details - Trajectory step viewer

**System:**
- APM Widget - Actions per minute tracking
- Trajectory Pane - Run history
- MC Tasks - Ready tasks list
- Container Panes - Sandbox logs

### Next Steps

1. **HuggingFace Trajectory Loader** - Sidebar widget for loading HF datasets
2. **Simple TB Run View** - Main area for current run display
3. **Trajectory Viewer** - Step-by-step replay and inspection
