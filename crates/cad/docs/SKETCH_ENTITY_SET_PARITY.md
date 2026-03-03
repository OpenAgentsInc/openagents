# Sketch Entity Set Parity

Issue coverage: `VCAD-PARITY-041`

## Purpose

Expand and lock deterministic sketch entity schema parity for:

- `line`
- `rectangle`
- `circle`
- `arc`
- `spline`

## OpenAgents Entity Contracts

- `CadSketchEntity` now carries first-class variants for rectangle, circle, and spline in addition
  to existing line/arc support.
- Entity validation enforces deterministic anchor mapping and finite geometry invariants:
  - rectangle requires strict `min_mm < max_mm` on both axes
  - circle requires finite positive `radius_mm`
  - spline requires anchor/control-point count parity and minimum point count
- Sketch profile conversion parity tracks closed-loop behavior by entity type:
  - rectangle/circle/closed-spline map to closed-loop profile semantics
  - line/arc keep open-profile warning semantics

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-entity-set -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_entity_set_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_entity_set --quiet`

## Failure Modes

- Entity schema drift (missing or renamed kinds) fails manifest regeneration checks.
- Closed/open profile classification drift for entity kinds fails summary parity.
- Non-deterministic replay of sample sketch entity corpus fails deterministic signature checks.
