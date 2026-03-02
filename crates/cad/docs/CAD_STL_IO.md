# CAD STL Import/Export

Issue coverage: `VCAD-PARITY-081`

## Scope

In scope:
- Deterministic binary STL export from `CadMeshPayload`.
- STL import to `CadMeshPayload` from binary and ASCII STL payloads.
- vcad-aligned binary header label: `vcad binary STL export`.
- vcad-style ASCII detection (`solid` + `facet` sample) and per-face vertex deduplication.

Out of scope:
- STL color/material extensions.
- STL file I/O wrappers in this crate (APIs are byte-buffer based).

## API

`crates/cad/src/stl.rs`

- `export_stl_from_mesh(document_id, document_revision, variant_id, mesh) -> CadStlExportArtifact`
- `import_stl_to_mesh(document_revision, variant_id, stl_bytes) -> CadStlImportResult`
- `CadStlExportArtifact { receipt, bytes }`
- `CadStlExportReceipt` includes:
  - `document_id`, `document_revision`, `variant_id`, `mesh_id`
  - `triangle_count`, `byte_count`, `deterministic_hash`
- `CadStlImportResult` includes:
  - `source_format` (`binary` | `ascii`)
  - `triangle_count`, `unique_vertex_count`, `import_hash`
  - `mesh` (`CadMeshPayload`)

## Determinism Rules

- Binary STL header is fixed to `vcad binary STL export` with zero padding to 80 bytes.
- Triangle traversal follows `triangle_indices` order.
- Vertex deduplication on import uses exact float-bit keys.
- Identical input bytes produce identical `import_hash` and imported mesh payload.

## Failure Handling

- Export failures map to `CadError::ExportFailed { format: "stl", reason }`.
- Import failures map to `CadError::ParseFailed { reason }`.

Key parse failures:
- truncated binary payload (`expected at least 84 bytes`)
- mismatched binary byte count (`expected N bytes, got M`)
- ASCII payloads with no `vertex` lines
- ASCII payloads with malformed `vertex` coordinates
