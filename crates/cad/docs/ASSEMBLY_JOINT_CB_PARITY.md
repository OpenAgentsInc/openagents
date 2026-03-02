# Assembly Joint Cylindrical/Ball Parity (`VCAD-PARITY-059`)

`VCAD-PARITY-059` implements cylindrical and ball joint motion parity semantics.

## Scope

- Added full-joint motion solver:
- `CadAssemblyJoint::solve_motion`
- `CadJointMotion::{Cylindrical,Ball}` variants
- Cylindrical behavior (vcad eval parity):
- axis normalization with zero-axis fallback to +Z
- state interpreted as rotation angle
- translation uses `parentAnchor - rotated(childAnchor)`
- Ball behavior (vcad eval parity):
- rotation axis fixed to +Z
- translation uses `parentAnchor - rotated(childAnchor)`

## vcad References

- `~/code/vcad/crates/vcad-eval/src/kinematics.rs`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_joint_cb_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_joint_cb_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-joint-cb -- --check`
- CI script: `scripts/cad/parity-assembly-joint-cb-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_joint_cb_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-joint-cb`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_joint_cb_vcad_reference.json`
