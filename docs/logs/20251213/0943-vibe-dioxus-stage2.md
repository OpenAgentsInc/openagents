# Vibe + OANIX — Dioxus Stage 2 (interactive mocks)

## What changed
- Added interactive controls and live state to the Vibe screen (no backend yet, but wired for OANIX flows):
  - Action bar with buttons to **Run WASI**, **Tail Logs**, and **Deploy**.
  - Terminal panel now consumes a live log signal; actions append realistic OANIX/WASI log lines.
  - Agent feed now consumes dynamic tasks; WASI runs add new task entries.
  - Deploy panel now reads from a signal so new deploy versions show up instantly.
- Added a `VibeSnapshot` loader hook and server-side placeholder so the UI can swap from mocks to real OANIX data without layout changes.
- Switched mock data to owned `String`-backed structs so we can mutate state (projects, templates, files, DB rows, deployments, domains, analytics).
- Added mock agent tasks and terminal logs as reusable constructors.
- UI layout keeps MechaCoder toggle; Editor tab now stacks the action bar above the 3-column workspace.

## Next steps
1. Swap mocks for real data:
   - Bridge to OANIX `Namespace` mounts for FileTree; stream `LogsFs` into the terminal.
   - Surface `Scheduler` jobs / `RunResult` in the agent feed.
2. Wire actions:
   - `Run WASI` → call OanixEnv + Scheduler and stream stdout/stderr into panels.
   - `Deploy` → hit a server function and update deployment status transitions.
3. Integrate wgpui canvas for richer preview/graphs once data is live.
4. Add routing + server functions (Dioxus fullstack) for fetch/mutate of OANIX state.
