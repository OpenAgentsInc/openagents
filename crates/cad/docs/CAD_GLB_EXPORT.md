# CAD GLB Export

Issue coverage: `VCAD-PARITY-082`

## Scope

In scope:
- Deterministic binary GLB (glTF 2.0) export from `CadMeshPayload`.
- Single-mesh GLB payload with POSITION + index buffers.
- vcad-aligned `asset.generator` metadata (`"vcad"`).
- Stable export receipt for replay and parity automation.

Out of scope:
- Material/texture export.
- Scene graph export with multiple mesh nodes.
- File I/O wrappers in this crate (API exports byte buffers).

## API

`crates/cad/src/glb.rs`

- `export_glb_from_mesh(document_id, document_revision, variant_id, mesh) -> CadGlbExportArtifact`
- `CadGlbExportArtifact { receipt, bytes }`
- `CadGlbExportReceipt` includes:
  - `document_id`, `document_revision`, `variant_id`, `mesh_id`
  - `vertex_count`, `index_count`, `byte_count`, `deterministic_hash`

## Determinism Rules

- GLB header always emits magic `glTF` and version `2`.
- JSON chunk is padded with spaces (`0x20`) to 4-byte alignment.
- BIN chunk is padded with zero bytes to 4-byte alignment.
- Vertex and index byte ordering follows mesh payload order.
- Identical mesh input produces identical GLB bytes and `deterministic_hash`.

## Failure Handling

- Export failures map to `CadError::ExportFailed { format: "glb", reason }`.
- Contract/identity failures include:
  - empty `document_id` or `variant_id`
  - invalid mesh payload contract
  - variant mismatch between requested `variant_id` and mesh payload
