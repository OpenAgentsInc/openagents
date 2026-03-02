# Sketch Jacobian + Residual Parity

Issue coverage: `VCAD-PARITY-045`

## Purpose

Add deterministic Jacobian/residual pipeline parity for sketch constraints and
surface solver diagnostics aligned with vcad LM reference behavior.

## OpenAgents Jacobian/Residual Contract

- `CadSketchModel::lm_pipeline_summary()` computes deterministic LM pipeline metadata:
  - residual vector size + L2 norm
  - finite-difference Jacobian shape + non-zero count
  - Jacobian rank and rank-deficiency flag
  - stable residual/Jacobian hashes
  - per-constraint residual component counts
- Finite-difference epsilon is currently `1e-8` (`LM_JACOBIAN_EPSILON`).
- Residual formulas cover the full sketch constraint enum, including
  geometric + dimensional variants and legacy `dimension`.

## Solver Diagnostics Contract

- Iterative solve now emits deterministic LM pipeline diagnostics:
  - `SKETCH_LM_PIPELINE_NON_FINITE` (error): Jacobian/residual pipeline produced non-finite values.
  - `SKETCH_LM_JACOBIAN_RANK_DEFICIENT` (warning): unsolved residual state with rank-deficient Jacobian.
  - `SKETCH_LM_PIPELINE_BUILD_FAILED` (warning): snapshot build failure during solve iteration.
- Pipeline diagnostics are attached to `constraint_id = "lm.pipeline"`.

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-jacobian-residual -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_jacobian_residual_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_jacobian_residual --quiet`

## Failure Modes

- Residual/Jacobian hash drift without fixture updates fails parity checks.
- Missing rank-deficiency warning in deterministic conflicting-dimensions case fails parity checks.
- Non-deterministic replay of pipeline summary/report snapshots fails fixture checks.
