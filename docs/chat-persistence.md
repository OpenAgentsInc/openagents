# Chat Persistence in Codex

This document explains how Codex persists chats today and what a GUI needs to list, resume, and render prior conversations. It also proposes a lightweight indexing schema if we decide to add a local DB later.

## Where chats are saved
- Root: `${CODEX_HOME:-~/.codex}`
- Directory layout: `sessions/YYYY/MM/DD/`
- File name: `rollout-YYYY-MM-DDThh-mm-ss-<conversation_id>.jsonl`
  - `<conversation_id>` is a UUID and the canonical chat ID.
  - Example: `~/.codex/sessions/2025/09/24/rollout-2025-09-24T07-04-31-01997961-c7ba-7fc0-aa32-e8996f295c1c.jsonl`

## File format (JSONL)
Each line is a `RolloutLine`:

```json
{ "timestamp": "2025-09-24T07:04:31.123Z", "type": "session_meta", "payload": { /* SessionMetaLine */ } }
{ "timestamp": "2025-09-24T07:04:31.456Z", "type": "event_msg",    "payload": { /* EventMsg::UserMessage */ } }
{ "timestamp": "2025-09-24T07:04:32.000Z", "type": "response_item", "payload": { /* assistant message, tool call/output, reasoning, … */ } }
```

Key variants persisted (see `codex-rs/protocol/src/protocol.rs` and `codex-rs/core/src/rollout/policy.rs`):
- `SessionMeta(SessionMetaLine)` — first line; includes:
  - `id` (UUID chat id), `timestamp`, `cwd`, `originator`, `cli_version`, optional `instructions`, and optional `git` info.
- `ResponseItem(...)` — durable items in the transcript: `Message` (user/assistant), `Reasoning`, `LocalShellCall`, `FunctionCall`, `FunctionCallOutput`, `CustomTool*`, `WebSearch*`.
- `EventMsg(...)` — only select high‑level events are saved: `UserMessage`, `AgentMessage`, `AgentReasoning` (not deltas), `TokenCount`, `Entered/ExitedReviewMode`, `TurnAborted`.
- `TurnContext` — per‑turn config snapshot (cwd, approval policy, sandbox policy, model, reasoning settings).
- `Compacted` — compaction marker with the assistant summary text.

Not persisted: streaming deltas (agent text/reasoning chunks), exec stdout/stderr chunks, and most low‑level events. The file contains finalized items sufficient to reconstruct the transcript.

## Listing and resuming
Core provides helpers in `codex-rs/core/src/rollout/`:
- `RolloutRecorder::list_conversations(codex_home, page_size, cursor)` returns newest‑first pages. Each item includes the absolute `path` and a `head` (up to 10 JSON objects from the top of the file). Only files with a `SessionMeta` and at least one `UserMessage` are listed.
- `RolloutRecorder::get_rollout_history(path)` parses a file and returns `InitialHistory::Resumed` with `conversation_id`, full `history` (all persisted `RolloutItem`s), and the `rollout_path`.
- At runtime the protocol emits `SessionConfigured { session_id, rollout_path, initial_messages?, … }` so a UI can remember the current file while streaming.

### Practical UI plan
- To build a “Chats” list, page via `list_conversations` and derive:
  - `id`: from `SessionMeta.meta.id` (UUID)
  - `started_at`: from `SessionMeta.meta.timestamp`
  - `title`: first line of first `UserMessage` (truncate ~80 chars) as a heuristic
  - `cwd` and optional `git` (branch/commit) from `SessionMetaLine`
  - `path`: absolute rollout file path for resume
- To resume, call `get_rollout_history(path)`, seed the transcript with the returned items, and continue streaming new turns into the same session.

## Conversation vs Session
- A “session” is the lifetime captured by one rollout file; its id is the UUID embedded in both the file name and the `SessionMeta`.
- A “conversation” in memory is the evolving list of items used to build the next prompt. Compaction may replace older segments with `Compacted`+bridge text; those markers are also persisted.

## Optional extras today
- Some builds mention an optional `history.jsonl` for alternate history logging, but the source of truth for resume is the rollout file under `sessions/`.

## If we add a local DB (optional)
You do not need a DB to list/resume chats. If we want faster search/filtering or robust titles, a minimal SQLite schema could mirror rollouts:

Tables
- `sessions(id TEXT PRIMARY KEY, started_at TEXT, cwd TEXT, cli_version TEXT, originator TEXT, git_branch TEXT, git_commit TEXT, title TEXT, rollout_path TEXT, last_event_at TEXT)`
- `messages(session_id TEXT, offset INTEGER, role TEXT, kind TEXT, text TEXT, tool_name TEXT, PRIMARY KEY(session_id, offset))`

Ingestion
- On startup (or periodically), scan `~/.codex/sessions/**/rollout-*.jsonl`:
  - Insert/refresh `sessions` from the `SessionMeta` line and file mtime.
  - Extract first `UserMessage` as `title`.
  - Optionally index `messages.text` for local search.

For now, the simplest path is to use `list_conversations` for the left sidebar (fast, paginated, no new storage) and `get_rollout_history` to resume.

