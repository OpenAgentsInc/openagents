# Assembly Acceptance Scenes Parity (`VCAD-PARITY-065`)

`VCAD-PARITY-065` adds deterministic acceptance-scene fixtures for assembly parity.

## Scope

- Added deterministic acceptance reporting API:
- `CadAssemblySchema::acceptance_scene_report`
- `CadAssemblyAcceptanceSceneReport` (scene validity, missing references, cycle detection, FK coverage)
- Added fixture-driven acceptance scene parity coverage for:
- valid robot arm scene
- valid world-grounded slider scene
- invalid missing-reference scene
- invalid cyclic-joint scene

## vcad References

- `~/code/vcad/packages/core/src/stores/document-store.ts`
- `~/code/vcad/docs/features/assembly-joints.md`

## Parity Lane

- Manifest: `crates/cad/parity/assembly_acceptance_scenes_parity_manifest.json`
- Builder: `crates/cad/src/parity/assembly_acceptance_scenes_parity.rs`
- CLI: `cargo run -p openagents-cad --bin parity-assembly-acceptance-scenes -- --check`
- CI script: `scripts/cad/parity-assembly-acceptance-scenes-ci.sh`
- Reference fixture: `crates/cad/parity/fixtures/assembly_acceptance_scenes_vcad_reference.json`

## Troubleshooting

- Regenerate parity manifest:
  - `cargo run -p openagents-cad --bin parity-assembly-acceptance-scenes`
- Update expected case fixture:
  - `crates/cad/parity/fixtures/assembly_acceptance_scenes_vcad_reference.json`
