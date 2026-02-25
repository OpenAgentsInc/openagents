# Spacetime Replay/Resume Test Expansion

Status: active
Updated: 2026-02-25

## Objective

Provide one cross-surface replay/resume/reconnect verification lane that covers runtime sync metrics/retirement guards, shared Spacetime client behavior, and desktop apply/checkpoint lifecycle.

## Verification Entry Point

```bash
./scripts/spacetime/replay-resume-parity-harness.sh
```

Integrated local CI lane:

```bash
./scripts/local-ci.sh spacetime-replay-resume
```

## Cross-Surface Matrix

Runtime surface:

- `cargo test -p openagents-runtime-service spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture`
- `cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture`

Shared client surface:

- `cargo test -p autopilot-spacetime client::tests -- --nocapture`
- includes stale-cursor behavior, reconnect/backoff helpers, duplicate replay behavior, and multi-client ordering consistency for retained streams.

Desktop surface:

- `cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture`
- `cargo test -p autopilot-desktop sync_apply_engine -- --nocapture`
- `cargo test -p autopilot-desktop sync_lifecycle -- --nocapture`

## Coverage Goals by Failure Class

1. Stale cursor and checkpoint recovery.
2. Reconnect storm and deterministic duplicate handling.
3. Multi-client ordering consistency.

## Promotion Use

Use this harness as required evidence for staging and production sync promotion gates.
