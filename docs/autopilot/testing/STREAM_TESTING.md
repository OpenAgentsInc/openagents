# Stream Testing (Rust WS / Khala)

Active stream testing lanes:

1. Runtime websocket/sync contract tests in `apps/runtime/tests/`.
2. Cross-surface contract harness:
```bash
scripts/run-cross-surface-contract-harness.sh
```
3. iOS and desktop handshake validation via active runbooks under `apps/autopilot-ios/docs/`.

## Core invariants

1. Delivery is at-least-once.
2. Client apply is idempotent by `(topic, seq)`.
3. `stale_cursor` forces replay/bootstrap before live tail.

## Release gate

Any stream behavior change requires updated report evidence in `docs/reports/`.
