# Core: Rollout Persistence

Files:
- `codex-rs/core/src/rollout/mod.rs`
- `codex-rs/core/src/rollout/recorder.rs`
- `codex-rs/core/src/rollout/list.rs`

Records session activity in JSONL files under `~/.codex/sessions/YYYY/MM/DD/`.
Each line contains a timestamp and a `RolloutItem` variant (session meta,
response item, compaction entry, event message).

## Recorder

- `RolloutRecorder::new(config, params)` opens or resumes a file and spawns an
  async writer task.
- `record_items(&[RolloutItem])` filters to persisted types and appends.
- `flush()` and `shutdown()` provide backpressure and clean shutdown semantics.

## Session meta

- The first line includes `SessionMetaLine` with timestamp, cwd, CLI version,
  and optional git information (`collect_git_info`).

## Listing & resume

- `list_conversations(codex_home, page_size, cursor)` returns a page for a UI
  picker.
- `get_rollout_history(path)` parses a file to produce either `InitialHistory::New`
  or `InitialHistory::Resumed` with items and the conversation id.

