# STL Import/Export Parity

Issue coverage: `VCAD-PARITY-081`

## Goal

Match vcad STL behavior across export and import paths:

- deterministic binary STL export with vcad header label
- STL import support for both binary and ASCII payloads
- deterministic vertex deduplication and mesh reconstruction
- stable diagnostics for malformed STL input

## Contracts

- Binary export header starts with `vcad binary STL export`.
- Import format detection:
  - ASCII when payload starts with `solid` and sample contains `facet`.
  - otherwise binary.
- ASCII/Binary import returns deterministic `triangle_count` and `unique_vertex_count`.
- Truncated binary STL fails with:
  - `Invalid STL: expected at least 84 bytes, got <N>`

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/stl_import_export_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/stl_import_export_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-stl-import-export-ci.sh
cargo test -p openagents-cad stl::tests --quiet
```
