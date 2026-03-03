# CAD Collision Shape Generation Parity

Issue coverage: `VCAD-PARITY-106`

## Goal

Lock deterministic parity contracts for CAD Collision Shape Generation Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/collision_shape_generation_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/collision_shape_generation_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-collision-shape-generation-ci.sh
cargo run -p openagents-cad --bin parity-collision-shape-generation
```
