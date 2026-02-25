# Spacetime Shadow Parity Harness

Status: active  
Updated: 2026-02-25

## Objective

Run Spacetime and Spacetime in shadow mode, compare parity outputs before cutover, and block rollout when divergence exceeds configured thresholds.

## Implementation Surfaces

- Runtime dual-write parity validation:
  - `apps/runtime/src/spacetime_publisher.rs`
  - runtime mirror append validates stream id/seq/payload hash/payload bytes/durable offset.
- Control + Spacetime dual-read parity harness:
  - `apps/runtime/src/shadow_control_spacetime.rs`
  - `apps/runtime/src/bin/control-spacetime-shadow-harness.rs`
- Run-level replay parity harness:
  - `apps/runtime/src/shadow.rs`

## Harness Inputs and Outputs

Control/Spacetime harness compares normalized snapshots from legacy lane vs Rust/Spacetime lane:

- `control_status`
- `control_route_split_status`
- `spacetime_poll`
- `spacetime_metrics`

Output report schema:

- `openagents.shadow.control_spacetime_parity.v1`
- includes component hashes, diffs, totals, and gate decision (`allow` or `block`)

## Gate Thresholds

Policy fields:

- `max_warning_count`
- `block_on_critical`

Gate behavior:

1. `critical > 0` and `block_on_critical=true` -> block.
2. `warning > max_warning_count` -> block.
3. otherwise -> allow.

## Runbook Commands

Generate and enforce control/spacetime shadow parity gate:

```bash
cargo run -p openagents-runtime-service --bin control-spacetime-shadow-harness -- \
  --legacy-manifest <legacy_manifest.json> \
  --rust-manifest <rust_manifest.json> \
  --output <parity_report.json> \
  --max-warnings 0 \
  --block-on-critical true
```

The command exits non-zero when gate decision is `block`.

## Mismatch Resolution Workflow

1. Inspect `comparisons` and `diffs` in parity report; identify first critical divergence.
2. Confirm whether divergence is expected volatility or contract drift.
3. If volatility, update normalizer in harness (`strip_*_volatiles`/contract extractors) with test coverage.
4. If contract drift, fix producer lane (control or runtime) and regenerate report.
5. Re-run parity harness until decision is `allow`.
6. Attach report artifact to promotion evidence (`SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`).

## Verification

```bash
cargo test -p openagents-runtime-service shadow_control_spacetime -- --nocapture
cargo test -p openagents-runtime-service shadow::tests -- --nocapture
```
