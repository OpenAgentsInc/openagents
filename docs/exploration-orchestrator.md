# OpenAgents Exploration Flow (iOS ↔ macOS) — Architecture & Implementation Guide

This document describes the end‑to‑end exploration flow implemented in OpenAgents v0.3+ across iOS and macOS, including the bridge, orchestration, streaming updates, UI behaviors, path normalization, and developer controls.

## High‑Level Overview

- iOS app (SwiftUI) connects to the macOS companion via WebSocket (JSON‑RPC 2.0).
- macOS hosts the Desktop WebSocket Server and runs the on‑device Foundation Models (FM) orchestrator for exploration.
- The orchestrator plans a few bounded steps (session listing/searches, workspace scans), executes them, streams ACP updates (plan + tool calls), and finally prepares a short intent summary.
- The iOS UI renders
  - a single sticky plan header with status icons,
  - a compact timeline of streamed updates (tool calls/updates, agent content),
  - a transient “Working (Ns)” indicator while the server is preparing the first update,
  - a visible fm.analysis tool when the summary is being prepared,
  - and an on‑tap JSON inspector for tool calls.

## Components & Key Files

- Bridge (macOS server and iOS client)
  - Desktop server: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
  - iOS client/manager: `ios/OpenAgents/Bridge/BridgeManager.swift`

- Orchestration (macOS only)
  - Orchestrator: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift`
  - Tool execution hub: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ToolExecutor.swift`
  - Session tools: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SessionTools.swift`
  - Filesystem/code tools: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/{WorkspaceScanner.swift,GrepTool.swift,ContentSpanTool.swift}`
  - FM tool wrappers: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/FMTools.swift`
  - Common types: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationTypes.swift`
  - Path normalization: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/PathUtils.swift`

- iOS UI
  - Home view: `ios/OpenAgents/Views/ChatHomeView.swift`
  - Plan renderer: `ios/OpenAgents/ACP/Renderers/PlanStateView.swift`
  - Theme: `ios/OpenAgents/Theme.swift`

## Bridge Protocol (JSON‑RPC 2.0)

Server methods (macOS):
- `initialize` — handshake and capabilities exchange.
- `orchestrate.explore.start` — start an exploration session for a given workspace root + goals. Returns `{ session_id, plan_id, status }` and then streams updates via `session/update` notifications.

Notifications (server → client):
- `session/update` with ACP payloads, including:
  - `plan` — plan entries with `_meta` (op_id, op_hash, tool) and status.
  - `tool_call` — tool invocation announcement (contains `call_id`, name, optional `_meta`).
  - `tool_call_update` — started/progress/completed/error with optional `_meta` (`progress`, `note`) and `output`.
  - `agent_message_chunk` — agent text chunks (final summary uses this).

Client methods (iOS):
- Nothing custom for this flow; iOS issues `orchestrate.explore.start` and renders streamed updates.

## Orchestrator Flow

Entrypoint: `ExploreOrchestrator.startExploration()`

1) Validate FM availability and workspace readability.
2) Generate an initial FM‑assisted plan (3–5 operations) within strict bounds:
   - Allowed ops: `sessionList`, `sessionSearch`, `sessionRead`, `sessionAnalyze`, `listDir`, `readSpan`, `grep`.
   - Post‑process: ensure a `sessionAnalyze` step is present when session ops appear.
3) Stream plan as ACP `plan` with unique `op_id` per step.
4) Execute operations sequentially through `ToolExecutor` and stream:
   - `tool_call` (with name and `call_id`),
   - optional `tool_call_update` with `status=started` and `_meta.progress`,
   - `tool_call_update` with `status=completed` or `error` and bounded `output`.
5) Produce a concise final intent summary.
   - Prefer `session.analyze.userIntent` when present (deterministic, cleaned).
   - Otherwise synthesize via FM using compact context.
   - Stream a visible `fm.analysis` tool call (started → completed) to show that the summary stage is running.
   - Send the final text as an `agent_message_chunk`.

### Tools Summary

- Session tools (bounded, read from local session JSONL files)
  - `session.list` — recent sessions with metadata (truncated sample).
  - `session.search` — regex search within recent sessions (bounded results and byte caps).
  - `session.read` — small ranges, file references extraction.
  - `session.analyze` — aggregates: file frequency, tool frequency, goal patterns, avg length, and a `userIntent` string.

- Filesystem/code tools
  - `fs.list_dir` — directory listing (non‑recursive); respects workspace boundaries.
  - `content.get_span` — small slices of files.
  - `search.grep` — bounded regex search with time cap, cooperative yields, result caps, binary‑file skip, and common ignores; reports `matches`, `total_matches`, and `truncated` flag.

### Progress Semantics

- For long‑running operations (e.g., `session.analyze`, `search.grep`), the orchestrator emits a `tool_call_update` with `status=started` and `_meta.progress`, followed by zero or more progress updates, then `completed`.

## Path Normalization

Many FM plans provide path aliases; we normalize all inbound paths to workspace‑relative forms:

Handled aliases/placeholders → normalized
- `.` `/` `workspace` `/workspace` → `.`
- `/workspace/...` → `...`
- `/path/to` `path/to` → `.`
- `/path/to/...` `path/to/...` → `...`
- `/<workspaceName>` → `.`
- `/<workspaceName>/...` → `...`
- Absolute paths inside the workspace → relative to root.

Single source of truth:
- `PathUtils.normalizeToWorkspaceRelative(...)`
- Used by `GrepTool`, `WorkspaceScanner`, and `ExploreOrchestrator` (for parsed plan path parameters).

Workspace enforcement:
- All tools verify resolved paths remain inside the workspace; otherwise emit `pathOutsideWorkspace` errors.

## Desktop Server (macOS)

File: `DesktopWebSocketServer.swift`
- Accepts connections, completes WebSocket handshake, and receives JSON‑RPC requests.
- On `orchestrate.explore.start`, constructs an `ExploreOrchestrator` with a stream handler that encodes ACP updates as JSON‑RPC `session/update` notifications.
- Streams a visible `fm.analysis` tool around summary generation:
  - `tool_call` (`name=fm.analysis`), `tool_call_update status=started`, `tool_call_update status=completed` with `{ summary_bytes, source }`.
- Final summary is an `agent_message_chunk` with:
  - `**Repository:** <name>`
  - `**Intent From Session History:**` or `**Inferred Intent (FM):**` followed by a single concise sentence.

## iOS UI (SwiftUI)

File: `ChatHomeView.swift`
- Connection banner shows bridge status and number of streamed updates.
- “Start” triggers `orchestrate.explore.start` with a fixed workspace path (dev/local).
- A transient `Working (Ns)` indicator appears immediately after start and hides on the first streamed update.
- Sticky plan header (`PlanStateView`) shows a single, deduplicated plan summary with per‑step status icons:
  - pending ○, in‑progress ⏰, completed ✓
  - “Plan Complete” when all steps complete
- Timeline list excludes inline `plan` updates (they appear only in the sticky header) and includes tool call/update rows and messages.
- fm.analysis progress also appears as a tool row while the intent summary is prepared.
- JSON Inspector: tap any tool row to open a sheet showing the raw (re‑encoded) ACP notification JSON.
- Agent message chunks render directly as markdown (header label/icon removed for a cleaner feed).

File: `PlanStateView.swift`
- Renders plan status dot and title (`Plan Idle / Running / Complete / Failed`).
- Shows steps with numeric labels and status icons; respects theming for readability.

File: `Theme.swift`
- Uses dynamic system colors for backgrounds, borders, and text to ensure readable contrast in both light and dark mode.

## Intent Generation Rules

- Use `session.analyze.userIntent` when available; clean it:
  - Collapse bullets into a single sentence.
  - Normalize paths to repo‑relative (using `PathUtils`).
- If `userIntent` is missing, synthesize via FM using a compact JSON context with bounded size.
- Server labels the intent section to reflect the true source:
  - “Intent From Session History” (deterministic)
  - “Inferred Intent (FM)” (synthesized)

## Boundedness & Safeguards

- Grep/search operations include time caps, cooperative yielding, binary detection, ignore lists, and result caps.
- Session reads enforce byte caps and per‑operation limits.
- All streamed payloads are pruned/summarized where necessary before sending to iOS to avoid oversized messages.

## Developer Notes

- Start the macOS app (server) first; then run iOS (simulator/device).
- For iOS exploration, update the default workspace path in `ChatHomeView.startExploreFlow()` if needed.
- To debug paths, search for usages of `PathUtils.normalizeToWorkspaceRelative(...)`.
- To verify streaming, watch the Xcode console for `[Bridge] ...` and `[Orchestrator] ...` logs on both sides.

## Troubleshooting

- “Path outside workspace” on FM‑suggested paths:
  - Ensure PathUtils handles the alias (e.g., `/workspace`, `/path/to`, `/<workspaceName>`). It already does for the common cases above.
- Summary shows FM label when a deterministic intent exists:
  - Check `session.analyze` produced `userIntent`; otherwise FM fallback is used.
- UI shows no progress during summary:
  - fm.analysis tool call should stream (`started` → `completed`); see server code around that call.
- Light mode readability issues:
  - Confirm text uses `OATheme.Colors.text*` semantics and avoid hard‑coded gray values.

---

For design decisions and rationale, see ADRs under `docs/adr/` (especially ADR‑0004 and ADR‑0006).
