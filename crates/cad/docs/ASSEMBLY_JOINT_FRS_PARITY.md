# Assembly Joint FRS Parity (`VCAD-PARITY-058`)

`VCAD-PARITY-058` implements fixed/revolute/slider joint motion parity behavior and deterministic fixtures.

## Scope

- Added joint motion solver for fixed/revolute/slider lane:
- `CadAssemblyJoint::solve_fixed_revolute_slider_motion`
- `CadJointMotion` result enum
- Behavior contracts:
- Fixed: translation uses `parentAnchor - childAnchor`
- Revolute: same anchor translation + angle state, with axis normalization
- Slider: anchor translation + linear offset along normalized axis
- Zero-length axis fallback to +Z for revolute/slider
- Cylindrical/Ball are explicit out-of-scope errors for this lane (handled in later issues)

## vcad References

- `~/code/vcad/docs/features/assembly-joints.md`
- `~/code/vcad/packages/engine/src/kinematics.ts`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_joint_frs_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_joint_frs_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-joint-frs -- --check`
- CI script: `scripts/cad/parity-assembly-joint-frs-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_joint_frs_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-joint-frs`
- Update expected cases/reference:
  - `crates/cad/parity/fixtures/assembly_joint_frs_vcad_reference.json`
