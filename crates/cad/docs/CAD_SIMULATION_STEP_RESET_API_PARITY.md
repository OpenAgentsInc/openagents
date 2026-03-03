# CAD Simulation Step Reset API Parity

Issue coverage: `VCAD-PARITY-109`

## Goal

Lock deterministic parity contracts for CAD Simulation Step Reset API Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/simulation_step_reset_api_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/simulation_step_reset_api_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-simulation-step-reset-api-ci.sh
cargo run -p openagents-cad --bin parity-simulation-step-reset-api
```
