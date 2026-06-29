# IDE And Editor Integration Audit

Date: 2026-06-11

This is system #36 from the Bun/Effect terminal-agent systems list. It captures
how a terminal coding agent should integrate with local editors and IDEs:
discovery, workspace matching, dynamic connection, file opening, diff display,
selection context, and language diagnostics.

The core idea is that IDE integration is an optional context and control plane.
It should enrich the agent without becoming the authoritative runtime.

## Target

Build an IDE/editor integration system that can:

- Detect running editors.
- Match editor workspaces to the current agent workspace.
- Connect over a local authenticated transport.
- Open files and worktrees in the selected editor.
- Send file update notifications.
- Receive diagnostics and selected context.
- Fall back to the user's external editor.
- Work across macOS, Linux, Windows, and WSL where possible.

The system should be useful to a Bun/Effect terminal UI, a hosted worker, and
OpenAgents workrooms that need editor-adjacent evidence.

## User-Visible Capability

Users should be able to:

- See whether an editor integration is connected.
- Select among multiple running editors.
- Disconnect cleanly.
- Open the current project or worktree in an editor.
- Open a specific file at a line.
- See language diagnostics as context for the next agent turn.
- Use a terminal editor fallback when no integration exists.

The agent should not silently connect to an unrelated editor window.

## Core Model

```ts
interface IdeConnection {
  readonly connectionId: string
  readonly displayName: string
  readonly transport: "http_sse" | "websocket" | "stdio" | "none"
  readonly endpoint?: string
  readonly authTokenRef?: string
  readonly workspaceFolders: readonly string[]
  readonly matchedWorkspaceId?: string
  readonly status: "available" | "connected" | "failed" | "stale"
  readonly platformHints: readonly string[]
}

interface EditorAction {
  readonly actionId: string
  readonly kind: "open_file" | "open_workspace" | "show_diff" | "notify_file_updated"
  readonly workspaceId: string
  readonly path: string
  readonly line?: number
  readonly connectionId?: string
}

interface DiagnosticBatch {
  readonly batchId: string
  readonly receivedAt: string
  readonly source: string
  readonly files: readonly DiagnosticFile[]
  readonly truncated: boolean
}
```

Store connection and diagnostic records as runtime-private or team-private
evidence unless the user explicitly publishes them.

## Discovery

Discovery should use multiple signals:

- Editor-provided lockfiles or registration records.
- Workspace folder declarations.
- Local port liveness checks.
- Process identity.
- Parent-process ancestry when launched from an integrated terminal.
- Environment variables supplied by editor terminals.
- Platform path conversion for WSL and Windows.

Stale records should be cleaned up:

- Remove records for dead processes.
- Remove records whose local transport no longer responds.
- Handle inaccessible platform paths without blocking startup.
- Sort candidates by freshness.

When more than one editor matches, ask the user or use explicit auto-connect
policy. Do not guess.

## Workspace Matching

Workspace matching should be strict enough to prevent accidental cross-project
control.

Rules:

- Normalize Unicode paths.
- Resolve case-insensitive drive letters on Windows.
- In WSL, convert Windows paths to WSL paths when the editor runs on Windows.
- Reject WSL workspace paths for a different distro.
- Treat current working directory inside an editor workspace as a match.
- Allow explicit environment override for test or emergency cases.
- Prefer the editor whose process is an ancestor when running in that editor's
  terminal.

Invalid but running editors may be shown as unavailable with their workspace
folders summarized.

## Dynamic Connection

The runtime should treat an editor as a dynamic connector:

- A selected editor creates a dynamic connector config.
- The connector includes transport, URL, auth token reference, editor name, and
  platform hints.
- Disconnect removes connector tools and commands from runtime state.
- Connection attempts have a timeout.
- Connection status updates are reflected in the UI.
- Reconnect should clear stale transport cache.

The transport protocol can be MCP, local HTTP, WebSocket, or another small RPC
protocol, but the domain model should stay transport-neutral.

## File And Diff Actions

Editor actions should be structured:

- `open_workspace`: open a project or worktree root.
- `open_file`: open a file and optional line.
- `show_diff`: show proposed or applied diff.
- `notify_file_updated`: tell the editor after a runtime file edit.

When no connected editor exists, fall back to an external editor:

- Prefer `VISUAL`.
- Then `EDITOR`.
- Then platform defaults.
- Classify GUI editors separately from terminal editors.
- Use safe argv arrays on POSIX.
- Use platform-specific shell handling only where necessary.
- For terminal editors, hand the terminal screen to the editor and restore it
  afterward.

Do not shell-interpolate file paths on POSIX. Repository file names are
attacker-controlled input.

## Selection Context

The editor connector should optionally provide:

- Active file.
- Cursor line and column.
- Selected range.
- Visible range.
- Unsaved buffer content hash or excerpt.
- Project diagnostics.
- Editor command availability.

The runtime should treat this as context, not authority. If a file edit is
about to happen, read the file through the workspace service and check current
content before patching.

## Language Diagnostics

Language diagnostics should enter through a bounded registry.

Flow:

1. Language server or editor sends diagnostic notification.
2. Runtime validates the notification shape.
3. Runtime normalizes file URIs to local paths.
4. Diagnostics are deduplicated within the batch.
5. Diagnostics already delivered in prior turns are suppressed.
6. Errors are sorted ahead of warnings, info, and hints.
7. Per-file and total diagnostic counts are capped.
8. Batch is attached to the next turn as context.

Use an LRU for cross-turn deduplication so long sessions do not grow without
bound. Clear delivered diagnostics for a file when the agent edits that file.

## Language Server Manager

If the terminal runtime can start language servers directly:

- Initialize asynchronously after startup.
- Do not block basic terminal operation if initialization fails.
- Keep initialization idempotent.
- Use generation counters so stale async initialization cannot overwrite
  newer state.
- Support reinitialization after plugin refresh.
- Shut down child processes on exit.
- Isolate handler failures per server.

The language server system should be a plugin extension point, not a hard-coded
language matrix.

## Effect Services

Recommended split:

- `IdeDiscovery`: finds editor candidates.
- `WorkspaceMatcher`: validates workspace alignment.
- `IdeConnectionService`: manages dynamic connector state.
- `EditorActionService`: opens files, workspaces, and diffs.
- `PathConversionService`: local and editor path translation.
- `DiagnosticsRegistry`: bounded diagnostic delivery.
- `LanguageServerManager`: optional direct LSP lifecycle.
- `EditorFallback`: external editor launch.
- `IdeProjection`: status UI and API projection.

All services should fail soft. IDE integration should enhance the run, not make
core agent execution unavailable.

## Safety Rules

- Do not auto-connect to an unmatched workspace.
- Do not persist editor auth tokens in public records.
- Do not expose private local paths in public artifacts.
- Do not trust editor selection context without verifying file state.
- Do not run terminal editors without restoring terminal state.
- Do not let one language server failure break diagnostics from other servers.
- Do not keep stale dynamic connector tools after disconnect.

## Tests

Minimum coverage:

- Editor record parsing for old and new formats.
- Stale record cleanup.
- Workspace match and mismatch cases.
- WSL path conversion and distro mismatch.
- Multiple-editor selection behavior.
- Dynamic connect, disconnect, and timeout.
- External editor argv safety.
- Terminal editor screen handoff.
- Diagnostic deduplication and volume limiting.
- File-edit clearing of delivered diagnostics.
- Language server initialization retry and shutdown.

## OpenAgents Translation Notes

Checked the open OpenAgents issue list on 2026-06-11.

Related live roadmap issues:

- #4769 covers repo connect and per-mission data-scope UX.
- #4773 covers API parity.
- #4779 covers writeback symmetry through artifacts and authority to PR drafts.
- #4786 is the Autopilot MVP ladder epic.

No open issue explicitly names IDE/editor integration. That means OpenAgents
should treat this as a proposed subsystem unless a new issue lands or it is
folded into repo connect and workroom implementation.

Recommended OpenAgents shape:

- Keep IDE state optional and private by default.
- Add `EditorContextSnapshot` records only when a user connects an editor.
- Attach diagnostics to missions as private evidence unless published.
- Use editor integration to enrich PR writeback but never as the sole source of
  repository truth.
- Provide the same capabilities through the agent API so browser-only workroom
  features do not become privileged.

## Decision

Build IDE/editor integration after the workspace and Git systems, but before
high-polish local developer workflows. It materially improves context quality,
but it must remain optional and bounded.
