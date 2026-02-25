# Spacetime Parity Harness

Status: active
Updated: 2026-02-25

This is the active parity/replay harness reference for retained Spacetime sync lanes.

Archived historical Spacetime shadow harness:
- `docs/sync/archived/2026-02-25-spacetime-shadow-parity-harness.md`

## Objective

Validate runtime publish/delivery metrics, retained route retirement guards, and client replay/resume correctness in one deterministic lane.

## Canonical Harness

```bash
./scripts/spacetime/replay-resume-parity-harness.sh
```

## Runtime Checks

- `cargo test -p openagents-runtime-service spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture`
- `cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture`

## Shared Client Checks

- `cargo test -p autopilot-spacetime client::tests -- --nocapture`

## Desktop Checks

- `cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture`
- `cargo test -p autopilot-desktop sync_apply_engine -- --nocapture`
- `cargo test -p autopilot-desktop sync_lifecycle -- --nocapture`

## Promotion Use

Attach harness output as evidence for staging/prod gate reviews and rollback readiness records.
