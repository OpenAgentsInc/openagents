# Control + Khala Shadow Parity Harness

Status: Active  
Issue: OA-RUST-091 (`#1926`)

This harness extends shadow parity coverage beyond runtime authority to include:

1. Rust control-service API response parity.
2. Khala replay/live snapshot parity.

## Purpose

1. Detect behavioral drift before control/Khala cutover.
2. Produce deterministic parity report artifacts per deploy.
3. Gate promotion on critical divergence and warning thresholds.

## Binary

```bash
cargo run -p openagents-runtime-service --bin control-khala-shadow-harness -- \
  --legacy-manifest /tmp/shadow/control-khala/legacy/manifest.json \
  --rust-manifest /tmp/shadow/control-khala/rust/manifest.json \
  --output /tmp/shadow/control-khala/report.json \
  --max-warnings 0 \
  --block-on-critical true
```

## Manifest Schema

Each side (`legacy` and `rust`) provides:

```json
{
  "scenario_id": "staging-codex-worker-read",
  "control_status_path": "control_status.json",
  "control_route_split_status_path": "route_split_status.json",
  "khala_poll_path": "khala_poll.json",
  "khala_metrics_path": "khala_metrics.json"
}
```

Paths may be relative to the manifest directory.

## Report Artifact

Output schema: `openagents.shadow.control_khala_parity.v1`

Fields:

1. `comparisons[]`: component-level hash parity (`matched`, `legacy_hash`, `rust_hash`).
2. `diffs[]`: divergence records with severity (`critical`/`warning`).
3. `totals`: critical/warning counts.
4. `gate`: `allow` or `block`, with explicit reason.

## Severity Classification

Critical components (promotion-blocking when divergent):

1. `control_status`
2. `khala_poll`

Warning components (threshold-based gate):

1. `control_route_split_status`
2. `khala_metrics`

## Normalization Rules

To avoid false positives, harness normalization removes or condenses volatile fields:

1. Control status:
   - strips `memberships` from `data` before hashing.
2. Khala poll:
   - strips per-message `published_at`.
3. Khala metrics:
   - compares contract subset (`driver` + ordered topic window sequence bounds).

## Gate Policy

1. If `--block-on-critical=true` and any critical diffs exist, gate blocks.
2. If warning diffs exceed `--max-warnings`, gate blocks.
3. Otherwise gate allows promotion.

## Operational Flow

1. Capture snapshot responses from legacy and Rust stacks for the same scenario.
2. Generate parity report using the harness.
3. Archive report with deploy artifacts.
4. Block promotion on gate failure and escalate by severity.

Escalation policy:

1. Critical mismatch:
   - block release, open incident, and revert route split/cutover.
2. Warning mismatch above threshold:
   - block release, triage drift owner, re-run parity after fix.

## Controlled Divergence Test

A controlled divergence drill should force one component mismatch and confirm:

1. report marks expected severity,
2. gate blocks/permits according to configured thresholds.
