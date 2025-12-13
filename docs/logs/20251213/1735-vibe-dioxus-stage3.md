# Vibe + OANIX — Dioxus Stage 3 (server-backed snapshot)

## What changed
- Added `VibeSnapshot` server functions and an in-memory store so the UI can pull and mutate shared state:
  - `get_vibe_snapshot` loads current state (mock-initialized for now).
  - `run_wasi_job`, `tail_logs`, `trigger_deploy` mutate the snapshot and return the new data.
- Vibe screen now hydrates from the snapshot and reapplies server updates to all panels; the action bar calls server functions instead of only local signals.
- All Vibe types now derive `Serialize/Deserialize` to support fullstack/server functions.
- Added project selection: Project cards now activate an “active project”, and server calls are scoped per project ID. The header shows the active project, and the snapshot store tracks state per project.

## How it’s structured
- `crates/dioxus/src/views/vibe/data.rs`: snapshot store + server functions (swap the in-memory state for real OANIX data later).
- `crates/dioxus/src/views/vibe/screen.rs`: uses `use_resource` to load snapshot and applies updates from server actions to files, logs, tasks, deployments, etc.

## Next integration steps
1) Replace the in-memory snapshot with real OANIX sources:
   - Namespace mounts → FileTree (WorkspaceFs/CowFs/TaskFs).
   - LogsFs stream → TerminalPanel.
   - Scheduler jobs / RunResult → AgentPanel.
   - Deploy pipeline → real server function / executor.
2) Add wgpui canvas preview for richer renders once data is live.
3) Add router + persisted state (disk/DB) keyed by project IDs.
