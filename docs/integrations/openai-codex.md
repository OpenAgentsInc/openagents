# OpenAI Codex Integration Plan (Exec JSONL)

This document details our current integration with OpenAI Codex’s `exec --json` mode and clarifies adapter behavior in the Rust bridge and app UI. It cross‑references Codex’s own docs and our internal JSONL schema and resume behavior.

## Goals

- Treat Codex as a first‑class provider alongside OpenCode and Claude Code.
- Keep the app’s WebSocket‑only contract; all provider I/O is bridged on desktop.
- Maintain compatibility with the current feed renderer by adhering to the JSONL envelope in `docs/exec-jsonl-schema.md`.

## Codex Exec Overview

- CLI: `codex exec` supports JSONL streaming via `--json` (see `/Users/christopherdavid/code/codex-openai/docs/exec.md:1`).
- Event envelope (one JSON object per line): `thread.started`, `turn.started`, `item.started|updated|completed`, `turn.completed|failed`, `error`.
- Item kinds: `agent_message`, `reasoning`, `command_execution`, `file_change`, `mcp_tool_call`, `web_search`, `todo_list`.
- Resume: newer builds support `codex exec resume --last|<id>`; older builds do not (see `docs/exec-resume-json.md:1`).

## Bridge Adapter (today)

- Process lifecycle
  - Spawn per prompt, cwd = repo root heuristic.
  - When supported, subsequent prompts add `resume --last` (or a captured `thread_id`).
  - Write the prompt to stdin and close stdin to signal EOF.

- Stream handling
  - Forward each stdout JSONL line to all WS clients.
  - Suppress or summarize very large `exec_command_output_delta` payloads in console logs.
  - Track `thread.started.thread_id` to correlate sessions across spawns.

- Error and recovery
  - If stdin is consumed (single prompt), respawn for the next message.
  - On process exit or parse error, emit `error` and start fresh on next prompt.

## JSONL Contract (canonical)

See `docs/exec-jsonl-schema.md:1` for full details. Highlights:
- Envelope: `type` tagged union; items flatten their `type` under `item`.
- Usage is included only in `turn.completed`.
- `ThreadItem.id` is unique per stream and monotonically increasing (`item_0`, `item_1`, ...).

## Resume Behavior

- Preferred: `codex exec --json resume --last -` (stdin prompts) once a first run has established state.
- Fallback: if the installed Codex lacks `exec resume`, always run fresh; app UX remains stable but thread continuity is not preserved.
- The bridge auto‑detects support and logs its choice (see `docs/exec-resume-json.md:1`).

## App Mapping

- The app parser (`expo/lib/codex-events.ts:1`) already understands these events and renders:
  - `agent_message` → markdown rows.
  - `reasoning` → reasoning rows.
  - `command_execution` → command rows (begin/complete, exit codes, output sample).
  - `file_change`, `mcp_tool_call`, `web_search`, `todo_list` → dedicated cards.

## Testing Checklist

- Fresh run emits `thread.started`, items, final `turn.completed`.
- Resume run emits only new items and a final `turn.completed`.
- Failure path emits `error` and a `turn.failed` sentinel.
- Large command output is summarized but preserved in the JSONL line.

## References

- Codex docs: `/Users/christopherdavid/code/codex-openai/docs/exec.md:1`
- JSONL schema: `docs/exec-jsonl-schema.md:1`
- Resume details: `docs/exec-resume-json.md:1`
