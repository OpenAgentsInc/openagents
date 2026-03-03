# CAD Joint Physics Mapping Parity

Issue coverage: `VCAD-PARITY-108`

## Goal

Lock deterministic parity contracts for CAD Joint Physics Mapping Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/joint_physics_mapping_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/joint_physics_mapping_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-joint-physics-mapping-ci.sh
cargo run -p openagents-cad --bin parity-joint-physics-mapping
```
