# Assembly Forward Kinematics Parity (`VCAD-PARITY-061`)

`VCAD-PARITY-061` implements vcad forward-kinematics solve parity for assembly instance world transforms.

## Scope

- Added `CadAssemblySchema::solve_forward_kinematics`.
- Solver behavior follows vcad eval semantics:
- root instances are instances not referenced as a joint child
- BFS traversal over parent->child edges
- world-grounded (`parentInstanceId = null`) edges use identity parent world transform
- composition order is `parent_world * joint_transform * instance_local`
- visited guard stabilizes cyclic chains without infinite traversal
- Added deterministic transform composition path (Euler/matrix conversion and axis-angle rotation)

## vcad References

- `~/code/vcad/crates/vcad-eval/src/kinematics.rs`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_fk_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_fk_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-fk -- --check`
- CI script: `scripts/cad/parity-assembly-fk-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_fk_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-fk`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_fk_vcad_reference.json`
