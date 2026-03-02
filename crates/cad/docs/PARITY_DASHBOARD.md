# Parity Dashboard Workflow

Issue coverage: `VCAD-PARITY-010`

## Purpose

Generate and publish the baseline parity dashboard in repo docs from deterministic
parity artifacts (`scorecard`, `risk register`, `CI artifact manifest`).

## Outputs

- `crates/cad/parity/parity_dashboard.json`
- `crates/cad/docs/PARITY_BASELINE_DASHBOARD.md`

## Commands

Generate dashboard outputs:

```bash
cargo run -p openagents-cad --bin parity-dashboard
```

Check fixture/doc lock in CI:

```bash
scripts/cad/parity-dashboard-ci.sh
```

## Determinism Contract

- `parity-dashboard --check` fails on JSON or markdown drift.
- `scripts/cad/parity_check.sh` enforces dashboard freshness.
- `crates/cad/tests/parity_dashboard.rs` verifies regeneration equivalence for both outputs.
