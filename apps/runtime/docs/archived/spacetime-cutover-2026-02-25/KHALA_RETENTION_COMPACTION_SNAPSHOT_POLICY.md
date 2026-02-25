# Khala Retention, Compaction, and Snapshot Policy (Rust Runtime)

Policy authority: `apps/runtime/src/fanout.rs` and `apps/runtime/src/config.rs`.

## Guarantees

1. Runtime is the ordering authority for Khala topic streams via per-topic `seq`.
2. Replay is bounded by topic-class replay budgets.
3. Stale cursor outcomes are deterministic and machine-readable.

## Topic Classes

Configured in Rust runtime fanout limits:

1. `run_events` (`run:{run_id}:events`)
2. `worker_lifecycle` (`worker:{worker_id}:lifecycle`)
3. `codex_worker_events` (`runtime.codex_worker_events`)
4. `fallback` (all other topics)

## Stale Cursor Policy

Runtime returns deterministic stale cursor reasons:

1. `retention_floor_breach`
2. `replay_budget_exceeded`

Clients must reset local watermarks and replay bootstrap when stale cursor is returned.

## Operator Controls

Environment knobs:

- `RUNTIME_KHALA_*_REPLAY_BUDGET_EVENTS`
- `RUNTIME_KHALA_*_PUBLISH_RATE_PER_SECOND`
- `RUNTIME_KHALA_*_MAX_PAYLOAD_BYTES`

See `apps/runtime/src/config.rs` for the full matrix.

## Verification

Core Rust tests:

- `cargo test --manifest-path apps/runtime/Cargo.toml memory_fanout_rejects_stale_cursor`
- `cargo test --manifest-path apps/runtime/Cargo.toml memory_fanout_rejects_cursor_when_replay_budget_is_exceeded`
- `cargo test --manifest-path apps/runtime/Cargo.toml khala_topic_messages_returns_stale_cursor_when_replay_floor_is_missed`
