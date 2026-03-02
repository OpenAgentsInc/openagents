# CAD Compact IR

Issue coverage: `VCAD-PARITY-086`

## Purpose

Provide vcad-aligned compact CAD IR parse/serialize support for token-efficient AI and headless flows.

## Surface

`crates/cad/src/compact_ir.rs` exposes:

- `to_compact(&CadMcpDocument) -> CadResult<String>`
- `from_compact(&str) -> CadResult<CadMcpDocument>`
- `looks_like_compact_ir(&str) -> bool`

`crates/cad/src/mcp_tools.rs` exposes adapters:

- `cad_document_to_compact(&CadMcpDocument) -> CadResult<String>`
- `cad_document_from_compact(&str) -> CadResult<CadMcpDocument>`
- `cad_document_from_text(&str) -> CadResult<CadMcpDocument>`

## Grammar (implemented subset)

- Header/comments: `# ...`
- Materials: `M name r g b metallic roughness [density]`
- Geometry opcodes:
  - `C sx sy sz ["name"]`
  - `Y r h ["name"]`
  - `S r ["name"]`
  - `K rb rt h ["name"]`
  - `U left right ["name"]`
  - `D left right ["name"]`
  - `I left right ["name"]`
  - `T child x y z ["name"]`
  - `R child rx ry rz ["name"]`
  - `X child sx sy sz ["name"]`
- Scene roots: `ROOT node material [hidden]`

Node references are line-indexed in compact text (0-based) and validated during parse.

## MCP + CLI Integration

- `create_cad_document(format=compact)` now returns compact IR text.
- `export_cad`/`inspect_cad` value adapters accept `ir` as either JSON object or compact IR string.
- `openagents-cad-cli import` accepts `*.cad0` and `*.vcadc`.
- `openagents-cad-cli info` recognizes compact IR files.

## Error Contract

Compact parse failures map to `CadError::ParseFailed` with line-specific diagnostics, including:

- unknown opcode
- wrong argument count
- invalid number/node id
- invalid node reference (forward/out-of-range)
