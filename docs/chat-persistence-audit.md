# Codex Chat Persistence Audit (including Codex Exec)

This document surveys how Codex persists chat data across the Rust codebase, how sessions are resumed, and how `codex exec` integrates with that persistence. It also highlights likely causes when chats appear “not saved in the usual Codex location,” and what parts of the code control that behavior.

## Summary

- Session transcripts are persisted as JSON Lines “rollout” files under `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<UUID>.jsonl`.
- The first line includes `SessionMeta` (session id, cwd, originator, CLI version, instructions, and `source`). Subsequent lines contain structured items (response items, compaction markers, turn context, select event messages).
- A separate global message history file `$CODEX_HOME/history.jsonl` stores user-entered message text across sessions.
- `codex exec` uses the same persistence mechanism and path as the TUI/CLI; however it labels sessions with `SessionSource::Exec`. The TUI’s “resume last/picker” filters only interactive sources (CLI, VS Code) by default, so Exec sessions are saved but not listed there.
- `CODEX_HOME` determines where files are written. If `codex exec` runs with a different `CODEX_HOME` from the TUI/CLI, sessions will be saved elsewhere.

## Where data is stored

- Rollouts (session transcripts)
  - Path pattern: `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-YYYY-MM-DDThh-mm-ss-<UUID>.jsonl`
  - Created and written by `RolloutRecorder`.
  - Includes: `SessionMeta`, `ResponseItem`s, compaction markers, `TurnContext`, and a curated subset of event messages.
- Global message history
  - Path: `$CODEX_HOME/history.jsonl`
  - One JSON object per line: `{ "session_id": "<uuid>", "ts": <unix_seconds>, "text": "<message>" }`
  - Appended to when UIs send `Op::AddToHistory`.

## Key modules and responsibilities

- Rollout recording and listing
  - File layout and recorder API: codex-rs/core/src/rollout/recorder.rs
  - Listing/pagination and file scanning: codex-rs/core/src/rollout/list.rs
  - Persistence policy (what is saved): codex-rs/core/src/rollout/policy.rs
  - Constants and re-exports: codex-rs/core/src/rollout/mod.rs
- Conversation lifecycle (writes into rollout + in-memory history)
  - Core session orchestration and persistence hooks: codex-rs/core/src/codex.rs
  - In-memory history structure used to rebuild/compact: codex-rs/core/src/conversation_history.rs
- Global message history (cross-session)
  - File API for `$CODEX_HOME/history.jsonl`: codex-rs/core/src/message_history.rs
- Front-ends
  - TUI entry + resume filtering via `INTERACTIVE_SESSION_SOURCES`: codex-rs/tui/src/lib.rs
  - Exec (non-interactive) front-end: codex-rs/exec/src/lib.rs

## Rollout files in detail

- Creation
  - On session start, `Session::new` initializes a `RolloutRecorder` with params that include the `SessionSource` (CLI/VSCode/Exec/MCP).
  - The recorder writes a first item with `SessionMeta` (id, timestamp (UTC RFC3339), cwd, originator, CLI version, optional instructions, source) and collected Git info.
- Writing model and system events
  - The recorder accepts items as `RolloutItem` and persists those permitted by policy:
    - Always persisted: `ResponseItem`s (assistant/user messages, reasoning, tool calls and outputs, web search calls), compaction markers, turn context snapshots, `SessionMeta`.
    - Event messages: only a curated subset are written (e.g., `UserMessage`, `AgentMessage`, `AgentReasoning`, `TokenCount`, review-mode enter/exit, turn abort). Noisy or internal streaming deltas are skipped.
- Flushing and shutdown
  - Before replying to `Op::GetPath`, the recorder is flushed for consistency.
  - On `Op::Shutdown`, Codex requests a graceful recorder shutdown so tests and tooling see a finalized file.
- Listing and pagination
  - `RolloutRecorder::list_conversations` scans `$CODEX_HOME/sessions/**` newest-first using a stable ordering (timestamp desc, then UUID desc), returning a `ConversationsPage` with head/tail JSON, created/updated timestamps, and a `Cursor` for pagination.
  - Callers can filter by `SessionSource`. The TUI uses a narrow filter; see “Exec-specific behavior” below.

## Global message history in detail

- Path and schema: `$CODEX_HOME/history.jsonl` with owner-only permissions on Unix.
- Writing: triggered by `Op::AddToHistory` (front-ends decide when to send; the TUI does this for user-typed text). Writes are append-only JSONL with advisory file locks to avoid interleaving across processes.
- Reading:
  - `history_metadata` yields a file identifier (`log_id`, inode on Unix) and current line count for UIs to page backward safely.
  - `lookup(log_id, offset)` returns a single entry by offset for the specific file identity.
- Config: `[history]` supports `persistence = "save-all" | "none"` (default `save-all`), which affects the history file only, not rollout recording.

## Resuming and reconstructing a conversation

- Resume inputs
  - `ConversationManager::resume_conversation_from_rollout` takes a rollout path and spawns a session with `InitialHistory::Resumed` populated from the JSONL file.
  - The initial `SessionConfigured` event includes `rollout_path` so UIs know which file backs the session.
- Rebuilding in-memory history
  - On resume or fork, rollout items are converted back to `ResponseItem`s. Compaction markers are expanded via `build_compacted_history` using the initial context and observed user messages to reconstruct an equivalent history.

## Exec-specific behavior (codex exec)

- Source tagging
  - Exec constructs its `ConversationManager` with `SessionSource::Exec`. This is saved in `SessionMeta`.
- Persistence location
  - Exec loads the same `Config` as TUI/CLI (unless CLI override flags differ) and writes to the same `$CODEX_HOME/sessions/...` path.
  - If `CODEX_HOME` differs between processes (environment differences, wrapper scripts, CI), they will write to different trees. Confirm with `echo $CODEX_HOME` for each process; default is `~/.codex`.
- Resume flows
  - `codex exec resume --last`/`--id` use `list_conversations(..., allowed_sources = [])` (no filtering) or direct id lookup, so Exec can resume Exec/CLI/VS Code sessions.
  - The interactive TUI “resume last/picker” uses `INTERACTIVE_SESSION_SOURCES = [Cli, VSCode]` and therefore does not show Exec sessions by default. Sessions created via Exec are present on disk but intentionally filtered out of the picker and “resume last” in TUI.
- Output convenience files
  - Exec optionally writes the final agent message to a user-specified file (`--output-last-message`); this is not a persistence format for resuming, just a convenience artifact.

## Why chats may appear “not saved in the usual location”

- Session source filter in interactive UI
  - The TUI’s resume picker and “resume last” filter to interactive sources only. Exec sessions (`source = Exec`) are excluded even though they exist in `$CODEX_HOME/sessions/...`.
  - Workarounds:
    - Use `codex resume <SESSION_ID>` (TUI path by id does not filter by source).
    - Modify the filter to include `Exec` (see next section) if desired.
- Different `CODEX_HOME`
  - If `codex exec` and the interactive Codex run with different `CODEX_HOME` (explicit env var or home directory differences), they will write to different trees. Confirm with `echo $CODEX_HOME` for each process; default is `~/.codex`.
- Recorder initialization failure
  - Recorder creation errors cause session start to fail (with an error surfaced in logs); a running session implies the recorder initialized successfully.
- Archiving behavior
  - App server can move session files to `$CODEX_HOME/archived_sessions/`. If a file was archived, it won’t appear under `sessions/`.

## Integration points to surface Exec sessions in interactive Codex

- Include Exec in interactive filters
  - The TUI uses `INTERACTIVE_SESSION_SOURCES = [Cli, VSCode]` when listing. Including `Exec` here (or exposing a user preference to opt in) would make Exec sessions show up in the resume picker and “resume last”.
- Use direct id resumption where possible
  - Any front-end can resume via a known rollout path or session id using `find_conversation_path_by_id_str` to avoid source filtering.
- Ensure consistent `CODEX_HOME`
  - Align environment so all front-ends share the same `$CODEX_HOME`.

## Code reference map

- Session persistence entry points
  - `Session::record_conversation_items` → writes to in-memory history and rollout: codex-rs/core/src/codex.rs:860
  - Flush on `GetPath` and graceful shutdown: codex-rs/core/src/codex.rs:1368, codex-rs/core/src/codex.rs:1328
- Rollout recorder
  - Create and write JSONL with `SessionMeta`/`RolloutLine`: codex-rs/core/src/rollout/recorder.rs:100
  - Directory layout and filename format: codex-rs/core/src/rollout/recorder.rs:308
  - Persisted item policy: codex-rs/core/src/rollout/policy.rs
  - Listing/pagination and source filtering: codex-rs/core/src/rollout/list.rs
- Message history (global)
  - Append/metadata/lookup API: codex-rs/core/src/message_history.rs
- Exec front-end
  - Startup + resume logic (source = Exec): codex-rs/exec/src/lib.rs
- TUI filtering behavior
  - Resume filter (`INTERACTIVE_SESSION_SOURCES`): codex-rs/tui/src/lib.rs

## Practical checks

- Confirm the session file exists
  - Look under `$CODEX_HOME/sessions/**` for `rollout-*.jsonl`. The `SessionConfigured` event includes the full `rollout_path`.
- Verify `SessionMeta.source`
  - Inspect the first JSONL line to see if `source` is `Exec`, `Cli`, or `VSCode`.
- Check `CODEX_HOME`
  - Compare values for processes running Exec vs. interactive Codex.
- Resume by id
  - Use `codex resume <SESSION_ID>` to confirm the file is readable and resumable even if the picker omits it.

---

If you want me to propose a change set to include Exec sessions in the TUI’s resume picker, or to add a config knob to control the filter, I can draft that next.

