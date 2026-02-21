# Cross-Surface Contract Harness (OA-RUST-069)

Status: active
Owner: `owner:infra`

## Purpose

Run one shared scenario suite across web, desktop, and iOS adapters and emit normalized artifacts for release gating.

This harness validates contract behavior (not visual parity).

## Scenario Catalog

- `docs/autopilot/testing/cross-surface-contract-scenarios.json`

Current shared scenarios:

1. `worker_event_envelope_decode`
2. `sync_replay_dedupe`
3. `system_noise_suppression`

## Command

From repo root:

```bash
scripts/run-cross-surface-contract-harness.sh
```

Default iOS destination:

- `platform=iOS Simulator,name=iPhone 17 Pro`

Override if needed:

```bash
IOS_DESTINATION='platform=iOS Simulator,name=iPhone 16' \
scripts/run-cross-surface-contract-harness.sh
```

## Outputs

Per-run output directory:

- `docs/autopilot/testing/reports/cross-surface/<timestamp>/`

Artifacts:

- `surface-runs.jsonl` (raw adapter run records)
- `summary.json` (normalized pass/fail summary)
- `SUMMARY.md` (human-readable report)
- `logs/*.log` (surface command logs)

## Release Gate Use

This harness is required evidence for OA-RUST-069 and should be attached to release readiness checks:

1. Run harness against release candidate code.
2. Confirm `overall_status == "passed"` in `summary.json`.
3. Link `SUMMARY.md` and logs in release notes / issue comments.
4. If any surface fails, block promotion until fixed or formally waived.
