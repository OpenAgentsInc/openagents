# CAD MCP Simulation Tools Parity

Issue coverage: `VCAD-PARITY-112`

## Goal

Lock deterministic parity contracts for CAD MCP Simulation Tools Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/mcp_simulation_tools_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/mcp_simulation_tools_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-mcp-simulation-tools-ci.sh
cargo run -p openagents-cad --bin parity-mcp-simulation-tools
```
