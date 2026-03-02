# STEP Export Post-Boolean Parity

Issue coverage: `VCAD-PARITY-080`

## Goal

Match vcad STEP export semantics for post-boolean results:

- BRep-backed boolean results are STEP-exportable.
- Mesh-only boolean results fail with the vcad `NotBRep`-style message.
- Empty boolean results fail with the vcad empty-solid message.

## Contracts

- `can_export_post_boolean_step(brep_result)` returns:
  - `true` when `brep_result.is_some()`
  - `false` when `brep_result.is_none()`
- `export_step_from_post_boolean_brep(...)` maps missing BRep as:
  - `BooleanPipelineOutcome::EmptyResult` -> `cannot export to STEP: solid is empty`
  - otherwise -> `cannot export to STEP: solid has been converted to mesh (B-rep data lost after boolean operations)`
- BRep export path uses deterministic kernel STEP adapter bytes and receipt hashes.

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/step_export_post_boolean_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/step_export_post_boolean_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-step-export-post-boolean-ci.sh
cargo test -p openagents-cad export::tests::step_post_boolean_brep_export_succeeds_for_brep_result --quiet
```
