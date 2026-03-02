# Assembly Serialization + Replay Parity (`VCAD-PARITY-064`)

`VCAD-PARITY-064` implements deterministic assembly serialization and replay parity.

## Scope

- Added JSON helper APIs for assembly state payloads:
- `CadAssemblySchema::{to_json,to_pretty_json,from_json}`
- `CadAssemblyUiState::{to_json,to_pretty_json,from_json}`
- Added deterministic replay coverage for serialized assembly mutation sequences:
- selection + rename + joint-state clamp replay
- delete + sync replay cleanup behavior
- deterministic error contracts for invalid replay operations

## vcad References

- `~/code/vcad/packages/core/src/stores/document-store.ts`
- `~/code/vcad/packages/core/src/utils/save-load.ts`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_serialization_replay_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_serialization_replay_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-serialization-replay -- --check`
- CI script: `scripts/cad/parity-assembly-serialization-replay-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_serialization_replay_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-serialization-replay`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_serialization_replay_vcad_reference.json`
