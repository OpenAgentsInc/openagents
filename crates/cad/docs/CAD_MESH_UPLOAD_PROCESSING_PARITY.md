# CAD Mesh Upload/Processing Parity

Issue coverage: `VCAD-PARITY-096`

## Goal

Lock deterministic parity contracts for mesh upload validation and GPU processing entrypoints (`processGeometryGpu`, `computeCreasedNormalsGpu`, `decimateMeshGpu`) against the pinned vcad baseline.

## Contracts

- Mesh upload validation enforces deterministic input constraints:
  - `positions.len()` divisible by `3`
  - `indices.len()` divisible by `3`
  - every index in-bounds of uploaded vertex count
- Processing shape contracts are deterministic:
  - without LOD: one geometry result
  - with LOD: three results at ratios `[1.0, 0.5, 0.25]`
- Every processed level returns stable buffer classes:
  - `positions`, `indices`, `normals`
  - normals length tracks uploaded position length
- GPU-disabled fallback contract remains stable:
  - error message: `GPU feature not enabled`

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/mesh_upload_processing_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/mesh_upload_processing_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-mesh-upload-processing-ci.sh
cargo run -p openagents-cad --bin parity-mesh-upload-processing
```
