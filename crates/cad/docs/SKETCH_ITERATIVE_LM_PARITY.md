# Sketch Iterative LM Solver Parity

Issue coverage: `VCAD-PARITY-044`

## Purpose

Upgrade sketch solving from fixed one-pass behavior to deterministic iterative
Levenberg-Marquardt-style solving.

## OpenAgents Iterative Solver Contract

- Entry point remains: `CadSketchModel::solve_constraints_deterministic()`
- Solver now executes iterative LM-style passes with adaptive damping (`lambda`)
  instead of a hard-coded single pass.
- Default LM configuration is exposed through `CadSketchLmConfig` and currently uses:
  - `max_iterations = 100`
  - `residual_tolerance_mm = 1e-6`
  - `initial_lambda = 1e-3`
  - `lambda_increase = 10`
  - `lambda_decrease = 0.1`
  - `min_lambda = 1e-12`
  - `max_lambda = 1e12`
- Report semantics:
  - `iteration_count` now reflects actual iterative solve passes
  - deterministic replay from identical model seed produces identical report payload

## Solver Scope For This Issue

- Iterative solving is applied to currently implemented solve paths:
  - `coincident`, `horizontal`, `vertical`, `tangent`, `length`, `radius`, legacy `dimension`
- Constraint kinds not yet implemented in solve path are still validated and emitted as
  deterministic unsolved warnings (`SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED`).

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-iterative-lm -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_iterative_lm_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_iterative_lm --quiet`

## Failure Modes

- Regression back to one-pass (`iteration_count == 1` for coupled scenario) fails parity checks.
- Non-deterministic replay of iterative solve report or solved model hash fails fixture checks.
- LM configuration drift without fixture updates fails manifest lock checks.
