# Core: Compaction and Token Budgets

File: `codex-rs/core/src/codex/compact.rs`

Implements history compaction to fit within model context windows while
preserving recent turns and key context from earlier conversation.

## Inputs

- Model info: context window and `auto_compact_token_limit`.
- Config overrides: `model_auto_compact_token_limit`.
- Token usage snapshots from Responses API.

## Strategy

- Preserve the latest turns (user + assistant) intact.
- Summarize older content into a concise block using templates.
- Insert a history bridge that guides the model back into the flow.

## Output

- A new `ResponseItem::Message` structure containing the compacted summary.
- Optional `Compacted` rollout items to record the operation.

Tests in this module validate truncation, ordering, and retention rules.

