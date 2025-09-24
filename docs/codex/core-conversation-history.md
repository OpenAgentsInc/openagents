# Core: Conversation History

Manages the in‑memory view of a conversation (messages, tools, reasoning) and
provides compacted views when token budgets are tight.

Files:
- `codex-rs/core/src/conversation_history.rs`
- `codex-rs/core/src/codex/compact.rs`

## Responsibilities

- Keep the current list of `ResponseItem`s across turns.
- Insert summaries and history bridges when compaction runs.
- Provide a stable input list for `Prompt` assembly.

## Compaction triggers

- Based on `Config.model_auto_compact_token_limit` and the model’s token usage
  snapshots, compaction inserts a summary chunk with a bridge template.

## Bridges and templates

- Templates under `core/templates/compact/` include `prompt.md` and
  `history_bridge.md`; these guide the model to continue coherently after
  summarization.

