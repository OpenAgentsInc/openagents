# CAD Slicer Gcode Lane Parity

Issue coverage: `VCAD-PARITY-125`

## Goal

Lock deterministic parity contracts for CAD Slicer Gcode Lane Parity in Phase J - Full workspace parity lanes using the pinned vcad baseline.

## Contracts

- Capability scope parity is tracked for this issue ID and lane label.
- vcad source references used for this capability remain pinned and explicit.
- Generated parity manifests are deterministic across replay.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/slicer_gcode_lane_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/slicer_gcode_lane_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-slicer-gcode-lane-ci.sh
cargo run -p openagents-cad --bin parity-slicer-gcode-lane
```
