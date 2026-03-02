# Sketch Extrude Parity

Issue coverage: `VCAD-PARITY-047`

## Purpose

Align sketch extrude conversion behavior to vcad semantics for closed/open profiles,
parameter guards, and deterministic diagnostics.

## OpenAgents Extrude Contract

- `convert_sketch_profile_to_feature_node` for `SketchProfileFeatureKind::Extrude`:
  - requires positive `depth_mm`
  - rejects `source_feature_id`, `revolve_angle_deg`, and `axis_anchor_ids`
- Closed loops produce deterministic `sketch.extrude.v1` feature nodes with stable
  `profile_hash` independent of input entity order.
- Open profiles convert but emit deterministic `CAD-WARN-NON-MANIFOLD` warnings.
- Invalid inputs fail deterministically:
  - zero/negative extrusion depth
  - empty profile entity list
  - unsolved sketch constraints

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-extrude -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_extrude_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_extrude --quiet`

## Failure Modes

- Closed/open profile classification drift fails fixture equivalence.
- Non-deterministic `profile_hash` under reordered entity IDs fails parity checks.
- Missing guardrails for zero-depth, empty profiles, or unsolved constraints fails parity tests.
