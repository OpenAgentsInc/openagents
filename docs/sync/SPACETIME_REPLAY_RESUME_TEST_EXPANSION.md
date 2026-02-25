# Spacetime Replay/Resume Test Expansion

Status: active  
Updated: 2026-02-25

## Objective

Provide one cross-surface replay/resume/reconnect verification lane that covers runtime mirror parity, shared Spacetime client behavior, and desktop apply/checkpoint lifecycle.

## Verification Entry Point

```bash
./scripts/spacetime/replay-resume-parity-harness.sh
```

This script is also integrated into local CI:

```bash
./scripts/local-ci.sh spacetime-replay-resume
```

## Cross-Surface Matrix

Runtime surface:
- `cargo test -p openagents-runtime-service shadow_control_khala -- --nocapture`
- `cargo test -p openagents-runtime-service shadow::tests -- --nocapture`

Shared client surface:
- `cargo test -p autopilot-spacetime client::tests -- --nocapture`
- includes stale-cursor, reconnect/backoff helpers, duplicate replay behavior, and multi-client ordering consistency for shared streams.

Desktop surface:
- `cargo test -p autopilot-desktop sync_checkpoint_store -- --nocapture`
- `cargo test -p autopilot-desktop sync_apply_engine -- --nocapture`
- `cargo test -p autopilot-desktop sync_lifecycle -- --nocapture`

## Coverage Goals by Failure Class

1. Stale cursor and checkpoint recovery:
   - stale detection and rebootstrap path in shared client.
   - local checkpoint restore/clamp/rewind behavior in desktop.
2. Reconnect storm and duplicate delivery:
   - deterministic duplicate replay batches in shared client resubscribe loops.
   - desktop idempotent apply engine and reconnect backoff policy.
3. Multi-client ordering consistency:
   - equivalent ordered snapshots across clients subscribed to the same stream state.

## Promotion Use

Use this harness as required evidence for `OA-SPACETIME-029` gates before canary/production promotion.
