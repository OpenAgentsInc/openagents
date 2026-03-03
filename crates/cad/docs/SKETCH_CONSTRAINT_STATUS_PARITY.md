# Sketch Constraint Status Parity

Issue coverage: `VCAD-PARITY-046`

## Purpose

Align under/fully/over-constrained status semantics with vcad’s DOF model for
deterministic sketch-state reporting.

## OpenAgents Constraint-Status Contract

- `CadSketchModel::degrees_of_freedom()` now computes:
  - `DOF = parameter_count - constraint_equation_count`
- `parameter_count` is modeled as `2 * anchor_count` (x/y per anchor parameter).
- `constraint_equation_count` is the sum of `residual_component_count()` across constraints.
- Status mapping follows vcad semantics:
  - `DOF > 0` => `under_constrained`
  - `DOF == 0` => `fully_constrained`
  - `DOF < 0` => `over_constrained`
- Helper/report APIs:
  - `constraint_status()`
  - `constraint_status_report()`
  - `is_under_constrained()`
  - `is_fully_constrained()`
  - `is_over_constrained()`

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-constraint-status -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_constraint_status_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_constraint_status --quiet`

## Failure Modes

- DOF sign mismatches against status classification fail parity checks.
- Non-deterministic under/fully/over snapshots across replay fail fixture checks.
- Helper API drift (`is_under/is_fully/is_over`) fails parity fixture equivalence.
