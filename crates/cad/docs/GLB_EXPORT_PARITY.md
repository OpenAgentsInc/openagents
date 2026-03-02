# GLB Export Parity

Issue coverage: `VCAD-PARITY-082`

## Goal

Match vcad GLB behavior for deterministic mesh export:

- valid GLB 2.0 header/chunk structure
- vcad-aligned JSON generator metadata
- deterministic bytes + stable hash replay
- stable diagnostics for invalid mesh contracts and variant mismatch

## Contracts

- GLB header starts with `glTF` and version `2`.
- Chunk layout is deterministic:
  - JSON chunk type `JSON`
  - BIN chunk type `BIN`
- Exported JSON includes `asset.generator = "vcad"`.
- Invalid mesh contract fails with:
  - `mesh payload is invalid: mesh payload must include triangle indices`
- Variant mismatch fails with:
  - `mesh variant_id mismatch: payload=<payload> requested=<requested>`

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/glb_export_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/glb_export_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-glb-export-ci.sh
cargo test -p openagents-cad glb::tests --quiet
```
