# CodexMonitor Parity Plan for Autopilot UI

This document describes how to replicate CodexMonitor's feature set and layout inside
`crates/autopilot/` while adapting the visuals and behaviors to our WGPUI-based shell.
It assumes Autopilot phases 1-6 are complete and phase 7 (runtime adapters and
orchestration) is in progress.

## Goals

- Deliver the same UI layout and workflow model as CodexMonitor: workspace sidebar,
  chat/diff center, right-side git + approvals, bottom composer, and debug panel.
- Preserve Autopilot's existing backend architecture (Adjutant, Codex app-server adapter,
  tool telemetry) while adding multi-workspace orchestration.
- Keep the WGPUI rendering style and input model, but match CodexMonitor's visual hierarchy
  and interaction patterns.

## CodexMonitor Feature Inventory (What to Pull Over and Why)

### 1) Workspace and thread orchestration
What to pull over:
CodexMonitor persists a list of workspaces, spawns one `codex app-server` per workspace,
and restores thread lists on launch. Each workspace can be connected on-demand, and the
sidebar displays threads with a limited preview list, expand control, and status dot.
Thread rows support context actions (archive and copy id).

Why:
This is the core mental model of CodexMonitor: a workspace is a repository with its own
agent threads. The sidebar enables fast switching between projects and active agents, which
Autopilot does not currently expose in a workspace-first way. Parity here unlocks the exact
usage flow: add repo, start agent, switch threads, resume later.

### 2) App-server event stream to conversation items
What to pull over:
CodexMonitor translates app-server events into message items (user/assistant), reasoning
items (summary + full detail), tool items (command execution, file changes, MCP tool calls,
web search, image view), and review start/complete markers. File-change tools expand into
diff blocks. Items arrive via streamed deltas and are updated in place.

Why:
CodexMonitor is not only a chat surface; it is a live event timeline for the agent.
Autopilot already renders tool cards and DSPy stages, but to match CodexMonitor we need the
specific item taxonomy and streaming behavior that the app-server v2 protocol emits.
This makes the UI a faithful view of Codex activity and aligns with the CLI/TUI semantics.

### 3) Git status + diff panel + diff viewer
What to pull over:
CodexMonitor polls git status for the active workspace, shows branch name and file-level
change counts, and displays a per-file diff list in the right panel. Selecting a file
switches the center area into diff-view mode and scrolls to the selected diff. A full
diff viewer renders unified patches with line numbers and syntax highlighting.

Why:
The right-hand git panel is essential for navigating changes during agent work. It is also
a safety and review affordance: you can inspect file deltas before approving or continuing.
Parity here means the UI can serve as an at-a-glance change dashboard just like CodexMonitor.

### 4) Approvals surface and access mode
What to pull over:
CodexMonitor collects server-initiated approval requests (exec and apply patch) in a right
panel list. Each approval shows the method, params, and accept/decline actions. The
composer includes an access mode selector (read-only, current, full-access) that maps to
sandbox and approval policy when starting a turn.

Why:
Approvals are the practical autonomy control surface for local agents. CodexMonitor makes
these decisions explicit and always visible. Autopilot already has permission modes, but
this layout and the explicit panel mirror the app-server flow and reduce surprises.

### 5) Model, reasoning effort, and skill insertion controls
What to pull over:
The composer provides drop-down selectors for model and reasoning effort (from
`model/list`), an access mode selector, and a skills menu that inserts `$skill` tokens into
the input buffer. The model list defaults to a Codex model if present.

Why:
CodexMonitor treats these controls as first-class, inline with the prompt area. The
workflow is to set model, reasoning, and autonomy per message, not hidden in a modal.
Parity ensures Autopilot can behave the same way with app-server-backed sessions.

### 6) Review flow
What to pull over:
Typing `/review` starts a review turn (uncommitted changes by default), with support for
`/review base <branch>`, `/review commit <sha>`, or `/review custom <instructions>`.
Review start/complete markers appear in the timeline, and the composer is disabled while
review is in progress.

Why:
The review command is a key agent workflow in CodexMonitor. Matching its parsing and
timeline behavior will keep Autopilot aligned with the Codex CLI and avoid regressions in
review-driven task flows.

### 7) Debug panel and event logging
What to pull over:
A collapsible debug panel appears at the bottom and displays event logs filtered to errors
and warnings. It supports copy and clear actions, and is toggled via an icon in the top
bar when alerts are present.

Why:
CodexMonitor treats debug telemetry as a first-class UI feature. Autopilot has robust
telemetry panels already, but this lightweight, always-available debug log is crucial when
app-server protocol or runtime behavior goes wrong.

### 8) Layout and visual hierarchy
What to pull over:
The layout is a three-part grid: left sidebar (workspaces and threads), center main area
(chat or diff view), right panel (git + approvals). A top bar provides workspace context
(branch, repo name, path), while the bottom composer spans the main column. The look is a
translucent, blurred glass style with subtle borders and compact typography.

Why:
The spatial layout is the primary UX contract. Matching the geometry and visual hierarchy is
required to say we have implemented CodexMonitor features, even if the rendering surface is
WGPUI rather than HTML/CSS.

## Autopilot Architecture Mapping (Where This Fits)

- App state: extend `crates/autopilot/src/app/state.rs` with workspace and thread state,
  similar to CodexMonitor's `useWorkspaces` and `useThreads` hooks, but in Rust.
- App-server integration: reuse `crates/autopilot/src/app/codex_app_server/` to spawn and
  manage multiple app-server processes (one per workspace) and multiplex event streams.
- UI rendering: build a CodexMonitor-style layout in
  `crates/autopilot/src/app/ui/rendering/` (new layout functions plus new render passes for
  sidebar, top bar, right panel, and debug panel). This should sit alongside existing
  rendering code so we can switch layouts or merge them cleanly.
- Input/composer: adapt `crates/autopilot/src/app/ui/rendering/input.rs` to render a
  multi-line composer with inline selects and a send button, rather than a terminal prompt.
- Git status: add a `GitState` module and periodic refresh loop similar to the
  CodexMonitor backend, using `git2` in Autopilot to avoid app-server dependency for diffs.
- Thread and item mapping: add a CodexMonitor-style item model that is fed by
  app-server events in `crates/autopilot/src/app/events/response.rs` (or a new module).

## Implementation Plan

### Phase 1: Workspace manager + multi-app-server sessions
- Add `app/workspaces/` module that stores workspace entries and persists them to a
  `workspaces.json` under `~/.openagents/autopilot/`.
- Extend the app-server adapter to spawn and track multiple sessions keyed by workspace id.
  Each session should maintain its own JSONL transport and event channels.
- Implement operations: list workspaces, add workspace (folder picker), connect workspace,
  start thread, resume thread, list threads (paged), archive thread, and respond to approvals.
- Feed all app-server notifications into a unified event bus so the UI can update per
  workspace and thread.

### Phase 2: CodexMonitor layout skeleton in WGPUI
- Introduce a new layout helper for CodexMonitor geometry: left sidebar width 280px,
  right panel width 230px, top bar height ~40px, bottom composer height ~120px, plus
  an optional debug panel strip.
- Implement `render_monitor_layout` that draws the background layers and border lines
  matching the frosted glass look (translucent panels, subtle separators, soft typography).
- Add a main content switch that can render either chat or diff view based on UI state.

### Phase 3: Workspace + thread sidebar UI
- Render a workspaces list with add button, connect badge, and per-workspace thread list.
- Implement thread rows with status dots (processing, unread, reviewing, ready), limited
  to three with a "Show more" expander.
- Add context menu support for thread actions (archive, copy id) using WGPUI context menu
  components (similar to `ChatState` context menu infrastructure).
- Wire selection logic to update the active workspace and active thread.

### Phase 4: Conversation timeline and item rendering
- Define a `ConversationItem` struct mirroring CodexMonitor semantics and map app-server
  events into items (message, reasoning, tool, review).
- Render reasoning items as collapsible cards with summary title, and tool items with
  detail/output blocks. File change tools should expand into per-file diffs.
- Maintain streaming behavior: update existing items on delta events, append new items on
  completion. Show a "thinking" indicator when a turn is active.
- Keep the 30-item view window with automatic expand when the user scrolls up, to maintain
  the same performance and behavior profile.

### Phase 5: Git status, diff panel, and diff viewer
- Add a `GitState` that polls `git2` every 3 seconds for branch name, file statuses, and
  diff stats for the active workspace.
- Implement a right-side Git Diff panel with totals, branch name, and per-file counts.
- Implement a diff viewer in the main area that renders unified patches with line numbers
  and syntax highlighting, and auto-scrolls to the selected file.
- Provide a data flow to refresh diffs when the panel is open or a file is selected.

### Phase 6: Composer and inline controls
- Replace the terminal-style input rendering with a multi-line text area, send button, and
  a bottom control bar for model, reasoning effort, access mode, and skills.
- Implement Enter to send (Shift+Enter for newline) and disable input during review.
- Pull `model/list` and `skills/list` from app-server and update the selectors on workspace
  connect. Default to the Codex model when present.
- Insert `$skill` tokens into the text buffer on selection.

### Phase 7: Approvals panel and access mode wiring
- Surface pending approval requests in the right panel with method and JSON params,
  plus approve/decline actions.
- Map the access mode selector to sandbox and approval policy when starting a turn:
  read-only -> read-only sandbox, current -> workspace-write, full-access -> danger-full-access.
- Ensure app-server approval responses are routed through the session and update the
  approvals list in the UI.

### Phase 8: Review flow parity
- Add `/review` parsing and execution with target variants (base branch, commit, custom).
- Show review start/complete cards in the timeline and lock the composer while reviewing.
- Update thread state to indicate reviewing, so the sidebar status dot matches.

### Phase 9: Debug panel and event logging
- Implement a lightweight debug log that collects stderr, errors, and warnings from
  app-server and internal actions.
- Add a top-bar alert toggle and a bottom debug panel with copy/clear controls.
- Keep the log capped (200 entries) to match CodexMonitor behavior.

### Phase 10: Polish, tuning, and regressions
- Match CodexMonitor typography and sizing (compact 11-14px text, rounded buttons,
  translucent panel backgrounds) in WGPUI theme tokens.
- Validate the "diff view" switch behavior and ensure the right panel remains visible.
- Confirm session restore on startup and refresh on window focus.
- Add integration tests for multi-workspace app-server management and approval handling.

## Open Questions / Decisions

- Should CodexMonitor parity replace the existing Autopilot layout, or live behind a
  layout toggle (e.g., "Monitor Mode") so existing panels remain available?
- Do we want to keep CodexMonitor git status logic in Autopilot (git2) or rely on
  app-server `git/*` endpoints for status/diffs when available?
- How should we expose thread archive/copy-id context menu in WGPUI on macOS?

## Deliverable Checklist

- WGPUI layout matches CodexMonitor geometry and visual hierarchy.
- Workspace and thread management behave like CodexMonitor (including restore and focus
  refresh).
- App-server event stream drives conversation items with reasoning, tools, reviews, and
  approvals.
- Git diff panel and diff viewer match CodexMonitor behavior and styling.
- Composer controls (model, reasoning, access, skills) match CodexMonitor and drive
  app-server turn options.
- Debug panel and alert toggle behave like CodexMonitor.
