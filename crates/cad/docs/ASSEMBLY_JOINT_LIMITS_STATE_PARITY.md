# Assembly Joint Limits/State Parity (`VCAD-PARITY-060`)

`VCAD-PARITY-060` implements deterministic joint limits/state semantics parity for assembly joints.

## Scope

- Added deterministic state-semantics APIs on `CadAssemblyJoint`:
- `joint_dof`
- `joint_limits`
- `convert_state_to_physics_units`
- `convert_state_from_physics_units`
- `resolve_state_semantics`
- `set_state_with_limits`
- Revolute/slider limits are normalized and clamped before physics conversion.
- Fixed joints force state/physics state to zero.
- Unit semantics align with vcad mapping:
- `deg <-> rad` for revolute/cylindrical/ball
- `mm <-> m` for slider
- `fixed` for fixed joints

## vcad References

- `~/code/vcad/crates/vcad-kernel-physics/src/joints.rs`
- `~/code/vcad/crates/vcad-eval/src/kinematics.rs`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_joint_limits_state_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_joint_limits_state_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-joint-limits-state -- --check`
- CI script: `scripts/cad/parity-assembly-joint-limits-state-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_joint_limits_state_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-joint-limits-state`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_joint_limits_state_vcad_reference.json`
