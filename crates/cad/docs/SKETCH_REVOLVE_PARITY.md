# Sketch Revolve Parity

Issue coverage: `VCAD-PARITY-048`

## Purpose

Align sketch revolve conversion behavior to vcad semantics for full and partial
angles, axis requirements, and deterministic validation failures.

## OpenAgents Revolve Contract

- `convert_sketch_profile_to_feature_node` for `SketchProfileFeatureKind::Revolve`:
  - requires `revolve_angle_deg` in `(0, 360]`
  - requires `axis_anchor_ids` with two distinct anchor IDs
  - rejects `depth_mm` and `source_feature_id`
- Full-angle (`360`) revolve emits deterministic `sketch.revolve.v1` nodes with no
  partial-angle seam warning.
- Partial-angle revolve emits deterministic `CAD-WARN-SLIVER-FACE` advisory warnings.
- Invalid inputs fail deterministically:
  - zero angle
  - angle above 360
  - missing axis anchors
  - unsolved sketch constraints

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-revolve -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_revolve_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_revolve --quiet`

## Failure Modes

- Full vs partial-angle warning behavior drift fails fixture equivalence.
- Angle-bound or axis-anchor validation drift fails parity checks.
- Non-deterministic `profile_hash` under reordered entity IDs fails parity checks.
