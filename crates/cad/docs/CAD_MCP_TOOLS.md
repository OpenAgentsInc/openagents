# CAD MCP Tools

Issue coverage: `VCAD-PARITY-085`

## Scope

In scope:
- Implement vcad-aligned MCP CAD tool contracts in `crates/cad/src/mcp_tools.rs`:
  - `create_cad_document`
  - `export_cad`
  - `inspect_cad`
- Maintain deterministic response envelopes: `{ "content": [{"type":"text","text":"..."}] }`.
- Support deterministic primitive-driven modeling flow (`cube`, `cylinder`, `sphere`, `cone`) with ordered transform/boolean operation handling.

Out of scope:
- Full vcad sketch/extrude/revolve/sweep/loft MCP surface (covered in later parity issues).
- MCP transport/server runtime wiring.

## Tool Contracts

### `create_cad_document`

Input:
- `parts[]` with `name`, `primitive`, optional `operations[]`, optional `material`.
- `format` (`json` or `compact`, default `compact`).

Output:
- One text content item containing serialized IR document JSON (`version=0.1`).

### `export_cad`

Input:
- `ir` (document from `create_cad_document`)
- `filename` ending in `.stl` or `.glb`

Behavior:
- Evaluates part meshes deterministically.
- Writes bytes to disk at resolved output path.
- Returns text JSON payload with:
  - `path`
  - `bytes`
  - `format`
  - `parts`

### `inspect_cad`

Input:
- `ir` (document from `create_cad_document`)

Behavior:
- Evaluates part meshes deterministically.
- Returns pretty JSON text with:
  - `volume_mm3`
  - `surface_area_mm2`
  - `bounding_box`
  - `center_of_mass`
  - `triangles`
  - `parts`
  - optional `mass_g`, `part_masses` when density data is present.

## Determinism

Deterministic behavior is validated by:
- identical create document hash on repeated input
- stable inspect/export response hashes in parity lane replay
- byte-stable STL/GLB exports for unchanged input
