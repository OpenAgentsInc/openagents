# CAD Sketch Constraints (Wave 2 MVP)

This document defines the deterministic MVP constraint solver integrated into
`crates/cad/src/sketch.rs`.

## Supported Constraints

- `coincident`
- `horizontal`
- `vertical`
- `tangent`
- `dimension` (`length`, `radius`)

## Deterministic Solver Contract

- Entry point: `CadSketchModel::solve_constraints_deterministic()`
- Constraint evaluation order: lexical order of `constraint_id` (BTreeMap)
- Iteration count: fixed at `1` for MVP deterministic pass
- Output report:
  - `passed`
  - `solved_constraints`
  - `unsolved_constraints`
  - `constraint_status` map (`solved`/`unsolved`)
  - `residuals_mm` map per constraint id
  - structured diagnostics

## Diagnostic Codes

- `SKETCH_CONSTRAINT_COINCIDENT_UNSATISFIED`
- `SKETCH_CONSTRAINT_HORIZONTAL_UNSATISFIED`
- `SKETCH_CONSTRAINT_VERTICAL_UNSATISFIED`
- `SKETCH_CONSTRAINT_TANGENT_UNSATISFIED`
- `SKETCH_CONSTRAINT_DIMENSION_LENGTH_UNSATISFIED`
- `SKETCH_CONSTRAINT_DIMENSION_RADIUS_UNSATISFIED`

Each diagnostic includes:

- severity
- constraint id
- remediation hint

## Validation Rules

- Stable IDs are required for planes/entities/anchors/constraints.
- Constraint id must match map key.
- Constraints must reference existing entities/anchors.
- Constraint kind/entity type must match (e.g. `radius` only on arc entities).
- All tolerance values must be finite and > 0.

## Test Coverage

- Deterministic pass for common scenarios:
  - coincident + horizontal + vertical + tangent + length/radius dimensions
- Deterministic solver report across repeated runs
- Explicit unsolved tangent diagnostic behavior
- Reference validation failures for missing entities
