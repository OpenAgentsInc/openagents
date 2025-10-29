# Claude Code Integration Plan (Headless CLI)

This document outlines how to integrate Claude Code’s Headless CLI mode into the OpenAgents mobile app and Rust WebSocket bridge, similar to our Codex and OpenCode adapters. It analyzes the stream‑JSON output format and proposes an adapter that preserves the app’s existing WS contract and UI components. All bridge output will conform to our unified, canonical ThreadEvent envelope (see docs/integrations/README.md and crates/oa-bridge/src/events.rs).

## Goals

- Allow switching the provider to “Claude Code (headless CLI)” without app changes.
- Keep the bridge as the single integration point; the app must not call local HTTP/CLI directly.
- Map Claude’s stream‑JSON events to our established JSONL row kinds so the current feed renders naturally.
- Maintain per‑prompt respawn semantics (like Codex) with an option to reuse session context when available.

## Claude Code Headless Overview

- CLI: `claude` (Claude Code) supports a headless, non‑interactive mode with structured output.
- Invocation (one‑shot): `claude -p "<prompt>" --output-format stream-json [--verbose]`
  - Emits a sequence of JSON objects per line (“stream JSON”).
- Initialization event: first object typically has `{"type":"system","subtype":"init",...,"cwd":"<dir>","session_id":"<uuid>","tools":[...],"model":"claude-sonnet-..."}`.
- Assistant messages: `{"type":"assistant","message":{...,"content":[{type:"text",...}|{type:"tool_use",name:"...",input:{...}}],"usage":{...}}}`.
- Tool results are streamed as `{"type":"user","message":{...,"content":[{type:"tool_result",tool_use_id:"...",content:"..."}]}}` entries that match prior `tool_use` ids.
- Typical tool names include `Bash`, `Grep`, `Read`, `Write`, `WebSearch`, etc. A single run may involve multiple tool_call/tool_result pairs before the assistant finalizes.

Note: API keys and configuration are handled by the local CLI (e.g., ANTHROPIC_API_KEY); our bridge should not manage these secrets.

## Current Bridge Baseline

- Bridge launches a child process and fans out stdout/stderr lines to all WS clients.
- For Codex, we stream Codex JSONL directly; for providers that don’t speak our JSONL, we translate to our internal event envelope before broadcasting.

## Integration Strategy

Introduce a “Claude” runner in the bridge that spawns the `claude` CLI per prompt, parses stream‑JSON, and translates events to the canonical ThreadEvent JSONL so the app can render without change.

### Process lifecycle

- Spawn per prompt (default), cwd = detected repo root.
  - Command: `claude -p <prompt> --output-format stream-json --verbose`
  - Optionally add flags from bridge toggles (e.g., model selection, output style) if available.
- Capture stdout line‑by‑line; parse as JSON objects. Ignore non‑JSON if any.
- On process error/exit: emit an error event to clients and mark turn failed.

### Session reuse (optional)

- Track the last seen `session_id` from the init object. If Claude CLI supports passing a session identifier (or implicitly resumes by `cwd`), expose a bridge toggle “Resume last Claude session” to carry context across prompts. Otherwise, default to fresh one‑shots.

## Event Mapping (Claude stream‑JSON → canonical ThreadEvent)

Emit canonical ThreadEvent JSON lines. The app’s current parser will render these without change.

- Thread
  - Claude `system/init` → `{"type":"thread.started","thread_id": session_id || <random>}`

- Assistant text
  - Claude `assistant.content[{type:"text", text}]` → `item.completed` with `agent_message` (`{"type":"item.completed","item":{"id":"...","type":"agent_message","text":"..."}}`).

- Reasoning (if surfaced distinctly)
  - If the CLI differentiates reasoning, map to `reasoning` → UI `reason` row.

- Tool calls
  - For each `assistant.content[{type:"tool_use", name, input, id}]` emit an item start:
    - `{"type":"item.started","item":{"id":id,"type":"command_execution","command": name, "aggregated_output":"","status":"in_progress"}}`
  - For matching `user.content[{type:"tool_result", tool_use_id, content}]` emit completion:
    - `{"type":"item.completed","item":{"id":tool_use_id,"type":"command_execution","command": name, "aggregated_output": summarize(content), "exit_code": 0, "status":"completed"}}`
  - If an error is reported for a tool, set `status":"failed"` and `exit_code:1` and include error text in `aggregated_output`.

- Exec/binary output deltas
  - When tool output is very large, summarize with byte length to keep logs readable (similar to our Codex `exec_command_output_delta` handling).

- Turn lifecycle and usage
  - When `assistant.message.usage` becomes available, emit turn completion:
    - `{"type":"turn.completed","usage":{"input_tokens": n, "cached_input_tokens": 0, "output_tokens": m}}`
  - If the run aborts or returns an error at top‑level, emit `{"type":"turn.failed","error":{"message":"..."}}`.

- Errors
  - Any top‑level `type:"error"` object or unparseable line → bridge emits `{"type":"error","message":"..."}` and marks the turn failed.

- Housekeeping
  - The adapter should coalesce adjacent text segments to minimize UI spam and assign monotonic item ids per process to preserve ordering.

## App Behavior

- No app changes required in phase 1; the adapter feeds Codex‑style events over WS.
- Settings adds a Provider selector with options: `Codex (CLI)`, `OpenCode (server)`, `Claude Code (CLI)`.
- History/log store works unchanged; we can enrich entries with Claude’s `session_id` in metadata for deep‑linking later.

## Controls (WebSocket)

Reuse existing controls; keep the contract stable.

- `run.submit` — Bridge spawns `claude` with the provided prompt string.
- `run.abort` — Sends SIGINT to the child and marks the turn as failed/aborted.
- Optional future controls:
  - `provider.select` — switch runner mode (`codex|opencode|claude`).
  - `claude.session.resume` — opt‑in to session reuse where supported.

## Security & Permissions

- The bridge does not handle Anthropic credentials; the CLI loads them from the environment or its config store.
- All file/network actions are executed locally by the CLI as the current user; the bridge does not elevate privileges.

## Rollout Plan

1) Phase 1 — Bridge adapter
- Implement `claude` runner: spawn per prompt, parse stream‑JSON, map to Codex‑style events, and broadcast to WS clients.
- Add provider toggle in Settings; default remains Codex.

2) Phase 2 — Richer mapping (optional)
- Recognize additional Claude content (e.g., structured plan/todo, cost breakdown) and render with dedicated cards.
- Add attachment handling if tool outputs reference local files (show as links or inline previews).

3) Phase 3 — Session continuity
- If the CLI supports explicit session identifiers or project‑scoped context, allow opt‑in session reuse across prompts in the same repo.

## Testing

- Local sanity checks
  - Run prompts that trigger common tools (`Grep`, `Read`, `Bash`) and verify we see `cmd_item` rows with start/complete and output summaries.
  - Verify `assistant` text segments render as markdown rows.
  - Confirm usage totals appear on `turn.completed` and that aborts yield `turn.failed`.
- Failure and recovery
  - Kill the child process mid‑run to confirm error surfacing and clean reset on next prompt.

## Risks & Open Questions

- Session reuse: CLI guarantees for resuming by `session_id` vs implicit cwd linkage are unclear; ship one‑shot first.
- Tool taxonomy: Some tools may be non‑shell and better represented as `mcp_tool_call`; we start with `command_execution` for consistent UI.
- Output volume: Tools like `Read` can emit very large content; summarize to avoid UI slowdown and truncate stored logs.
- Model/config drift: Expose minimal knobs initially (model id, verbosity) and rely on CLI defaults.

## Example Mapping From Given Stream

Input (abridged):
- `{"type":"system","subtype":"init",...,"session_id":"5c0d3cff-..."}`
- `{"type":"assistant","message":{...,"content":[{"type":"text","text":"I'll search ..."}]}}`
- `{"type":"assistant","message":{...,"content":[{"type":"tool_use","id":"toolu_...","name":"Grep","input":{...}}]}}`
- `{"type":"user","message":{...,"content":[{"type":"tool_result","tool_use_id":"toolu_...","content":"Found 6 files..."}]}}`
- `{"type":"assistant","message":{...,"content":[{"type":"text","text":"Perfect! I found ..."}]}}`

Emitted to app:
- `{"type":"thread.started","thread_id":"5c0d3cff-..."}`
- `{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"I'll search ..."}}`
- `{"type":"item.started","item":{"id":"toolu_...","type":"command_execution","command":"Grep","aggregated_output":"","status":"in_progress"}}`
- `{"type":"item.completed","item":{"id":"toolu_...","type":"command_execution","command":"Grep","aggregated_output":"Found 6 files\n...","exit_code":0,"status":"completed"}}`
- `{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Perfect! I found ..."}}`

When `assistant.message.usage` appears, also emit `{"type":"turn.completed","usage":{...}}`.

## File References

- Canonical event types: `crates/oa-bridge/src/events.rs:1`
- App parser: `expo/lib/codex-events.ts:1`
- Codex JSONL schema (historical reference): `docs/exec-jsonl-schema.md:1`
- Example Claude headless stream (user provided): `claude --output-format stream-json`
