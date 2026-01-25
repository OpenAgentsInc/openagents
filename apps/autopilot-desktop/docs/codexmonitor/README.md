# CodexMonitor Feature Study

Scope:
- Source: /Users/christopherdavid/code/CodexMonitor
- Goal: inventory working Tauri/Codex features and decide what to reuse vs skip for Autopilot Desktop.

## Feature inventory (observed)

Backend / Tauri
- App-server orchestration per workspace, JSON-RPC init and event forwarding (`src-tauri/src/codex.rs`, `src-tauri/src/backend/app_server.rs`).
- Workspace persistence, grouping, and worktree/clone agents (`src-tauri/src/workspaces.rs`, `src/features/workspaces`).
- Thread lifecycle and approvals: start/resume/list/archive, interrupt, approval rules, review runs (`src-tauri/src/codex.rs`, `src-tauri/src/rules.rs`).
- Git plumbing: status, diffs, commit, branch, sync, GH issues/PRs via `gh` (`src-tauri/src/git.rs`).
- Files + prompt library: list/read files, manage custom prompts (`src-tauri/src/workspaces.rs`, `src-tauri/src/prompts.rs`).
- Terminal dock (PTY) with streaming output (`src-tauri/src/terminal.rs`).
- Dictation (Whisper) flow with download/manage, audio level, transcript events (`src-tauri/src/dictation.rs`).
- Local usage scanning from Codex session logs (`src-tauri/src/local_usage.rs`).
- Settings + Codex config toggle sync (`src-tauri/src/settings.rs`, `src-tauri/src/codex_config.rs`).
- Optional remote backend daemon mode (JSON-RPC over TCP) (`src-tauri/src/remote_backend.rs`, `REMOTE_BACKEND_POC.md`).
- In-app updater plumbing (Tauri updater) (`src/features/update`).

Frontend / UX
- Feature-sliced architecture, hooks own side effects; presentational components (`src/features/*`).
- Event hub to ensure one Tauri listener per event and fan-out to React (`src/services/events.ts`).
- Resizable panels (sidebar, right panel, plan, terminal, debug) with persisted sizes (`src/features/layout`).
- Workspaces: groups, recent activity, drop-zone add, auto reconnect on focus (`src/features/workspaces`).
- Threads: pinned/rename/archive, drafts, item normalization and truncation, plan panel, approvals (`src/features/threads`, `src/utils/threadItems.ts`, `src/features/plan`).
- Composer: queueing, model/access mode, reasoning effort, attachments, autocomplete for skills/prompts/paths (`src/features/composer`).
- Git UI: diffs, stage/revert, branches, PR composer, GitHub panels (`src/features/git`).
- Prompt editor library (global/workspace), file tree with search and reveal (`src/features/prompts`, `src/features/files`).
- Update toasts, debug log, notifications + sounds (`src/features/update`, `src/features/debug`, `src/features/notifications`).
- Dictation controls and live waveform UI (`src/features/dictation`).
- Liquid glass title bar + reduced transparency toggle (`src-tauri/src/window.rs`, `src/features/layout`).

## Adapt for Autopilot Desktop (high value, low UI cost)

Core runtime
- App-server boot/health check flow and initialization pattern (Codex + app-server readiness).
- Event hub pattern for app-server events so we keep a single native `listen`.
- Workspace persistence (minimal) and auto reconnect per workspace.
- Thread resume/list logic and event normalization for streaming items (reused as a backend utility even if UI is minimal).

Operational visibility
- Local usage snapshot (helpful for value accounting without UI knobs).
- Basic approval policy handling for safe defaults (auto-approve only when configured).

Engineering patterns
- Feature-sliced event wiring (hooks handle side effects; components stay dumb).
- Small, explicit IPC surface between UI and Tauri backend.

## Do not adapt (for v1 minimal UI)

UX surface area
- Multi-panel UI (resizable sidebar/right/plan/terminal/debug), tabbed layouts, and heavy settings screens.
- Full composer UI with attachments, autocomplete, or model/effort controls.
- Thread management UX (pin/rename/archive, drafts, diff viewer, review UI).
- File tree, prompt editor, and Git UI panels.
- GitHub issues/PR integration and PR composer.

Backend scope
- Dictation and audio pipeline.
- Terminal PTY dock.
- Remote backend daemon mode.
- Collaboration modes and experimental Codex feature toggles.
- In-app updater flows.

## Notes / cautions

- CodexMonitor has extensive controls and manual toggles; Autopilot Desktop should prefer automation and minimal decisions.
- Keep event flow compatible with Codex app-server (`initialize` before any other request, request/notification separation).
- The event hub pattern is worth adopting early to avoid multiple listeners as the UI grows.
