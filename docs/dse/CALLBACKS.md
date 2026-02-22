# Callbacks (Internal Observability)

Callbacks are in-process observability events, distinct from replay logs.

## Layering

1. Callback layer: local/in-process telemetry.
2. Replay layer: durable session event log (`REPLAY.jsonl`).

## Requirements

1. Callback failure must not crash execution.
2. Callback handling should be non-blocking.
3. Callback data may be richer than externally published replay artifacts.

See `docs/execution/REPLAY.md` for durable replay semantics.
