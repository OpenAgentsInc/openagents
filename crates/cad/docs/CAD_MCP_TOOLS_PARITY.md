# CAD MCP Tools Parity

Issue coverage: `VCAD-PARITY-085`

## Goal

Lock OpenAgents MCP CAD tool behavior to vcad-compatible contracts for the base `create/export/inspect` workflow.

## Contracts

Tool identity parity:
- `create_cad_document`
- `export_cad`
- `inspect_cad`

Case set:
- create deterministic IR from primitive input
- inspect deterministic geometry summary from created IR
- export deterministic STL bytes
- export deterministic GLB bytes

Expected checks:
- create snapshot has at least 2 nodes and 1 root
- inspect reports 1 part, valid positive volume/triangle counts
- mass data is present when material density is known
- STL/GLB exports exceed minimum valid byte thresholds

## Parity Artifacts

- Reference corpus:
  - `crates/cad/parity/fixtures/cad_mcp_tools_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/cad_mcp_tools_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-cad-mcp-tools-ci.sh
cargo run -p openagents-cad --bin parity-cad-mcp-tools -- --check
```
