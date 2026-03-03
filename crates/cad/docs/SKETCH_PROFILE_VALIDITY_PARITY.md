# Sketch Profile Validity Parity

Issue coverage: `VCAD-PARITY-051`

## Purpose

Align sketch-profile validity behavior with vcad semantics for deterministic
profile rejection and conversion safety checks.

## OpenAgents Sketch Profile Validity Contract

- `SketchProfileFeatureSpec::validate` now rejects duplicate `profile_entity_ids`.
- `convert_sketch_profile_to_feature_node` rejects profile entities with invalid
  geometry before node emission:
  - zero-length lines
  - rectangles with non-positive extents
- Open profiles remain convertible but emit deterministic
  `CAD-WARN-NON-MANIFOLD` warnings.
- Invalid inputs fail deterministically:
  - duplicate profile entity ids
  - degenerate profile geometry
  - unknown referenced entities
  - unsolved constraints

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-profile-validity -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_profile_validity_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_profile_validity --quiet`

## Failure Modes

- Profile validation contract drift fails fixture equivalence.
- Degenerate profile-geometry guardrail regressions fail parity checks.
- Open-profile warning behavior or unsolved/unknown-entity regressions fail parity checks.
