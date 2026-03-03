# Sketch Sweep Parity

Issue coverage: `VCAD-PARITY-049`

## Purpose

Align sketch-to-sweep conversion behavior with vcad semantics for path controls,
open-profile diagnostics, and deterministic validation failures.

## OpenAgents Sketch Sweep Contract

- `convert_sketch_profile_to_feature_node` supports `SketchProfileFeatureKind::Sweep`:
  - requires `sweep_path_entity_ids`
  - validates that referenced path entities exist in the sketch model
  - supports deterministic `sweep_twist_deg`, `sweep_scale_start`, and `sweep_scale_end` params
  - rejects unrelated extrude/revolve-only fields (`source_feature_id`, `depth_mm`,
    `revolve_angle_deg`, `axis_anchor_ids`)
- Closed profiles convert to deterministic `sketch.sweep.v1` feature nodes.
- Open profiles convert but emit deterministic `CAD-WARN-NON-MANIFOLD` warnings.
- Invalid inputs fail deterministically:
  - empty sweep path list
  - non-positive scale values
  - unknown path entity ids
  - unsolved sketch constraints

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-sweep -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_sweep_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_sweep --quiet`

## Failure Modes

- Sweep path/control serialization drift fails fixture equivalence.
- Open-profile warning behavior drift fails parity checks.
- Missing-path, invalid-scale, or unknown-path guardrail regressions fail parity checks.
