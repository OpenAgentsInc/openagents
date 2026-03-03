# CAD Embroidery Core Lane Parity

Issue coverage: `VCAD-PARITY-128`

## Goal

Lock deterministic parity contracts for CAD Embroidery Core Lane Parity in Phase J - Full workspace parity lanes using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/embroidery_core_lane_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/embroidery_core_lane_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-embroidery-core-lane-ci.sh
cargo run -p openagents-cad --bin parity-embroidery-core-lane
```
