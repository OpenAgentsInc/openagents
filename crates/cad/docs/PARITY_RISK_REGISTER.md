# Parity Risk Register And Blocker Workflow

Issue coverage: `VCAD-PARITY-009`

## Purpose

Track parity blockers in a deterministic register generated from the fixture corpus and scorecard,
with enforceable blocker profiles for CI and local workflows.

## Artifacts

- `crates/cad/parity/parity_risk_register.json`

The register records:

- open parity risks derived from missing fixture seeds
- blocker priority (`p0`) and severity mapping
- blocker profile evaluations (`phase_a_baseline_v1`, `parity_complete_v1`)

## Commands

Generate risk register fixture:

```bash
cargo run -p openagents-cad --bin parity-risk-register
```

Check fixture lock + baseline blocker profile:

```bash
scripts/cad/parity-risk-register-ci.sh
```

Run blocker workflow directly:

```bash
scripts/cad/parity-blocker-workflow.sh --check
scripts/cad/parity-blocker-workflow.sh --profile parity_complete_v1 --check
scripts/cad/parity-blocker-workflow.sh --list
```

## Blocker Workflow Contract

- `phase_a_baseline_v1` allows bounded open blockers while parity is incomplete.
- `parity_complete_v1` requires zero open blockers and zero open risks.
- CI parity orchestration enforces `phase_a_baseline_v1` via `scripts/cad/parity_check.sh`.

## Determinism Contract

- `parity-risk-register --check` fails on fixture drift.
- `crates/cad/tests/parity_risk_register.rs` verifies schema and generation equivalence.
- `crates/cad/tests/parity_blocker_workflow.rs` verifies blocker profile workflow discoverability.
