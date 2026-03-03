# CAD Gym Style API Parity

Issue coverage: `VCAD-PARITY-111`

## Goal

Lock deterministic parity contracts for CAD Gym Style API Parity in Phase I - Physics + URDF parity using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/gym_style_api_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/gym_style_api_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-gym-style-api-ci.sh
cargo run -p openagents-cad --bin parity-gym-style-api
```
