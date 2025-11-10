# Delegate Tool (`delegate.run`)

## Purpose

Route a concrete coding task to a configured agent provider (desktop or local) when execution is actually needed. Not for meta questions about capabilities; answer those inline.

## Name

`delegate.run`

## Summary

Delegates a specific task to an external provider (e.g., desktop coding agent or local LLM) with workspace context.

## Visibility

Emits an ACP `tool_call` in the transcript. Inspector shows arguments and result.

## Arguments

- `provider` (string, optional)
  - Allowed: `auto` (default), `codex`, `gptoss`, `local_fm`, `custom:<id>`
  - Notes: `auto` lets OpenAgents choose based on task; explicit values force a specific provider when available.
- `task` (string, optional)
  - Short verb for intent, e.g., `delegate`, `search`, `run`, `generate`.
- `description` (string, optional)
  - Human‑readable one‑liner for the delegate; shown in UI.
- `user_prompt` (string, required)
  - The exact instruction to pass through to the provider.
- `workspace_root` (string, optional)
  - Absolute path used as the working directory for the provider.
- `files_include_glob` ([string], optional)
  - Glob patterns to focus the provider’s operations (e.g., `src/**/*.swift`).
- `summarize` (bool, optional)
  - Request a succinct summary upon completion.
- `max_files` (int, optional)
  - Upper bound providers should respect when scanning/editing.
- `priority` (string, optional)
  - One of `low`, `normal`, `high` (advisory only).
- `time_budget_ms` (int, optional)
  - Soft budget; providers may adapt behavior to fit.
- `dry_run` (bool, optional)
  - If true, emit the `tool_call` without dispatching (for previews/instruction tuning).

## Result

- Returns a compact status string today (e.g., `"delegate.run dispatched"`).
- The authoritative record is the ACP timeline: the `tool_call` and any subsequent provider messages/results.
- Future: returns a structured object `{ ok, provider, dispatched, session_id, tool_call_id, notes }`.

## Routing Behavior

- `auto` selects the best available provider based on task hints, platform, and availability.
- Explicit `provider` forces routing when available; otherwise returns a clear unavailability message.
- Workspace context is applied if `workspace_root` is set. All resolved paths must remain inside the workspace.

## Safety & Guardrails

- Do not invoke for meta/capability questions (e.g., “what can you do?”, “who can you delegate to?”).
- Bound behavior via `max_files`, provider timeouts, and result caps; everything is logged to the ACP stream.
- File operations are limited to the workspace. Paths escaping the workspace are rejected.
- Users can cancel at any time; cancellation is propagated to the provider.

## Examples

1) Minimal, auto‑route
```json
{
  "tool": "delegate.run",
  "arguments": {
    "user_prompt": "Scan the repo for TODOs and summarize the top 5 actionable items"
  }
}
```

2) Explicit provider with workspace and focus globs
```json
{
  "tool": "delegate.run",
  "arguments": {
    "provider": "codex",
    "task": "delegate",
    "description": "Refactor SwiftUI view structure for readability",
    "user_prompt": "Extract subviews from ChatAreaView over ~100 lines; keep behavior identical",
    "workspace_root": "/Users/alex/code/openagents/ios",
    "files_include_glob": ["OpenAgents/**/*.swift"],
    "summarize": true,
    "max_files": 20
  }
}
```

3) Local LLM for heavy code generation
```json
{
  "tool": "delegate.run",
  "arguments": {
    "provider": "gptoss",
    "task": "generate",
    "description": "Draft an actor for WebSocket reconnection with exponential backoff",
    "user_prompt": "Implement a Swift actor that manages a WebSocket connection with jittered exponential backoff, cancellation support, and Combine publishers for status",
    "time_budget_ms": 120000
  }
}
```

## UI Integration

- The chat timeline shows a “tool call” row with name `delegate.run` and a compact argument summary.
- Clicking the row opens a detail sheet with pretty‑printed JSON for Arguments and Output.
- Provider activity (streams, tool sub‑calls) appears as subsequent messages in the same session.

## Notes

- Today, explicit `provider: "codex"` maps to the desktop provider if configured; other providers are enabled as they’re registered.
- The on‑device model can decide to emit `delegate.run` during a chat. When it does, the app records the `tool_call` and performs the dispatch automatically.
