# Assembly Schema Parity (`VCAD-PARITY-056`)

`VCAD-PARITY-056` adds the baseline assembly schema in `openagents-cad` aligned to the pinned vcad baseline (`1b59e7948efcdb848d8dba6848785d57aa310e81`).

## Scope

- Adds core assembly schema types in `crates/cad/src/assembly.rs`:
- `CadPartDef`
- `CadPartInstance` (`CadInstance` alias)
- `CadAssemblyJoint`
- `CadJointKind` (`Fixed`, `Revolute`, `Slider`, `Cylindrical`, `Ball`)
- `CadTransform3D`
- Adds document-level assembly fields in `CadDocument`:
- `partDefs`
- `instances`
- `joints`
- `groundInstanceId`

## vcad References

- `~/code/vcad/crates/vcad-ir/src/lib.rs`
- `~/code/vcad/docs/features/assembly-joints.md`

## Determinism Contracts

- Assembly fields serialize with vcad-compatible camelCase keys.
- Joint kind tags and limited-joint semantics (`Revolute`, `Slider`) are deterministic.
- Assembly schema parity fixture generation is byte-stable across replays.

## Parity Lane

- Manifest: `crates/cad/parity/assembly_schema_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_schema_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-schema -- --check`
- CI script: `scripts/cad/parity-assembly-schema-ci.sh`

## Troubleshooting

- If the parity lane reports schema drift, regenerate the manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-schema`
- If vcad reference fixture changes, update:
  - `crates/cad/parity/fixtures/assembly_schema_vcad_reference.json`
