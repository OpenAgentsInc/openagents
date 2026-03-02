# Sketch Constraint Enum Parity

Issue coverage: `VCAD-PARITY-043`

## Purpose

Lock deterministic parity for the full sketch constraint enum surface across:

- Geometric constraints
- Dimensional constraints
- Legacy compatibility constraint shape (`dimension`)

## OpenAgents Constraint Contracts

`CadSketchConstraint` now includes the full parity enum set:

- Geometric:
  - `coincident`
  - `point_on_line`
  - `parallel`
  - `perpendicular`
  - `horizontal`
  - `vertical`
  - `tangent`
  - `equal_length`
  - `equal_radius`
  - `concentric`
  - `fixed`
  - `point_on_circle`
  - `line_through_center`
  - `midpoint`
  - `symmetric`
- Dimensional:
  - `distance`
  - `point_line_distance`
  - `angle`
  - `radius`
  - `length`
  - `horizontal_distance`
  - `vertical_distance`
  - `diameter`
- Legacy compatibility:
  - `dimension` (`CadDimensionConstraintKind::{Length, Radius}`)

Validation contract updates:

- Model-level validation now checks anchor/entity reference integrity for every constraint kind.
- Curve constraints accept `arc` or `circle` entities where applicable.
- `tangent` supports optional explicit tangency anchor (`at_anchor_id`).

Solver behavior for this issue:

- Deterministic solver continues to solve current implemented subset:
  - `coincident`, `horizontal`, `vertical`, `tangent`, `length`, `radius`, legacy `dimension`
- Non-implemented kinds are reported deterministically as unsolved with warning diagnostic:
  - code: `SKETCH_CONSTRAINT_KIND_NOT_IMPLEMENTED`

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-constraint-enum -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_constraint_enum_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_constraint_enum --quiet`

## Failure Modes

- Enum/schema drift (missing/renamed constraint kinds) fails manifest parity checks.
- Reference validation regressions (anchor/entity resolution) fail sample corpus validation.
- Non-deterministic unsupported-kind diagnostics fail deterministic replay signature checks.
