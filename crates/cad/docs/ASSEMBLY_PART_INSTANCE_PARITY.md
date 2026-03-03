# Assembly Part/Instance Parity (`VCAD-PARITY-057`)

`VCAD-PARITY-057` implements deterministic part definition + instance behavior parity on top of the assembly schema added in `VCAD-PARITY-056`.

## Scope

- Adds part/instance behavior methods on `CadAssemblySchema`:
- `create_part_def`
- `create_instance`
- `rename_instance`
- `set_instance_transform`
- `set_instance_material`
- `resolve_part_instances`
- Behavior contracts:
- Instance creation requires an existing part definition.
- Instance ids are deterministic (`<partDefId>-<n>`).
- Effective material fallback follows vcad semantics:
  - instance override
  - part default
  - fallback `default`
- Instances with missing part definitions are marked unresolved and skipped from resolved bindings.

## vcad References

- `~/code/vcad/packages/core/src/stores/document-store.ts`
- `~/code/vcad/packages/engine/src/evaluate.ts`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_part_instance_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_part_instance_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-part-instance -- --check`
- CI script: `scripts/cad/parity-assembly-part-instance-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_part_instance_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest after behavior changes:
  - `cargo run -p openagents-cad --bin parity-assembly-part-instance`
- If reference semantics change, update:
  - `crates/cad/parity/fixtures/assembly_part_instance_vcad_reference.json`
