# Runtime Shadow Parity Harness

Status: active

This runbook defines the runtime authority shadow-mode parity harness used to gate authority cutover.

Historical control+Khala parity harness (archived):

- `apps/runtime/docs/archived/spacetime-cutover-2026-02-25/CONTROL_KHALA_SHADOW_PARITY.md`

Canonical retained sync parity lane:

- `docs/sync/SPACETIME_PARITY_HARNESS.md`

## Purpose

1. Compare legacy and Rust runtime outputs for the same run scenario.
2. Emit actionable parity diff reports (critical/warning classification).
3. Apply policy thresholds that block or allow cutover progression.

## Inputs

The harness compares two manifest files (`legacy` and `rust`) with identical schema:

```json
{
  "run_id": "run_123",
  "receipt_path": "receipt.json",
  "replay_path": "replay.jsonl",
  "summary_path": "summary.json",
  "checkpoint_path": "checkpoint.json"
}
```

Relative paths resolve from each manifest’s directory.

## Command

```bash
cargo run -p openagents-runtime-service --bin runtime-shadow-harness -- \
  --legacy-manifest /tmp/shadow/legacy/manifest.json \
  --rust-manifest /tmp/shadow/rust/manifest.json \
  --output /tmp/shadow/parity-report.json \
  --max-warnings 0 \
  --block-on-critical true
```

## Output

`parity-report.json` schema:

- `schema`: `openagents.runtime.shadow_parity.v1`
- `generated_at`
- `legacy_run_id`, `rust_run_id`
- `diffs[]`: `{severity, field, legacy, rust, message}`
- `totals`: `{critical, warning}`
- `gate`: `{decision: allow|block, reason}`

## Gate Policy

Default policy:

1. Any critical diff blocks cutover.
2. Warning diffs above `max_warnings` block cutover.

## Review Workflow

1. Run shadow harness for staged scenarios (happy path, cancellation, failure, replay/resume).
2. Archive parity reports per run in deployment artifacts.
3. Triage all critical diffs before cutover.
4. Keep warning count within policy threshold for release gate pass.
