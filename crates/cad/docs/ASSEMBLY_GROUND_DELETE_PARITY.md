# Assembly Ground + Deletion Parity (`VCAD-PARITY-062`)

`VCAD-PARITY-062` implements ground-instance and deletion-cleanup invariant parity for assembly editing semantics.

## Scope

- Added assembly mutation APIs:
- `CadAssemblySchema::set_ground_instance`
- `CadAssemblySchema::delete_joint`
- `CadAssemblySchema::delete_instance`
- Added deterministic deletion summary payload:
- `CadInstanceDeletionSummary`
- Enforced invariants:
- ground instance must reference an existing instance
- deleting an instance removes joints where parent or child references it
- deleting the grounded instance clears `ground_instance_id`
- unknown instance/joint ids return deterministic invalid-parameter errors

## vcad References

- `~/code/vcad/packages/core/src/stores/document-store.ts`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_ground_delete_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_ground_delete_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-ground-delete -- --check`
- CI script: `scripts/cad/parity-assembly-ground-delete-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_ground_delete_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-ground-delete`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_ground_delete_vcad_reference.json`
