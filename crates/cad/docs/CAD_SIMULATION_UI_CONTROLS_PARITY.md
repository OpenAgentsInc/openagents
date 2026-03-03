# CAD Simulation UI Controls Parity

Issue coverage: `VCAD-PARITY-110`

## Goal

Lock deterministic parity contracts for CAD Simulation UI Controls Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/simulation_ui_controls_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/simulation_ui_controls_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-simulation-ui-controls-ci.sh
cargo run -p openagents-cad --bin parity-simulation-ui-controls
```
