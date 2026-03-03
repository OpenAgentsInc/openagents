# Sketch Loft Parity

Issue coverage: `VCAD-PARITY-050`

## Purpose

Align sketch-to-loft conversion behavior with vcad semantics for multi-profile
inputs, closed-loft mode, and deterministic validation failures.

## OpenAgents Sketch Loft Contract

- `convert_sketch_profile_to_feature_node` supports `SketchProfileFeatureKind::Loft`:
  - requires `loft_profile_ids` with at least one secondary profile
  - rejects invalid self-reference (`loft_profile_ids` must not include primary `profile_id`)
  - supports deterministic `loft_closed` serialization for tube-style closed lofts
  - rejects unrelated extrude/revolve/sweep fields
- Closed profiles convert to deterministic `sketch.loft.v1` feature nodes.
- Open profiles convert but emit deterministic `CAD-WARN-NON-MANIFOLD` warnings.
- Invalid inputs fail deterministically:
  - empty `loft_profile_ids`
  - primary profile id in `loft_profile_ids`
  - unsolved sketch constraints

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-loft -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_loft_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_loft --quiet`

## Failure Modes

- Loft profile-id contract drift fails fixture equivalence.
- Closed-loft mode serialization drift fails parity checks.
- Open-profile warning behavior or invalid-input guardrail regressions fail parity checks.
