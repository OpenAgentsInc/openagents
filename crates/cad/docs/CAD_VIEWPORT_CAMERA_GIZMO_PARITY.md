# CAD Viewport Camera/Gizmo Parity

Issue coverage: `VCAD-PARITY-093`

## Goal

Lock a deterministic parity baseline for viewport camera defaults, camera interaction semantics, and orientation/transform gizmo contracts against the pinned vcad reference.

## Contracts

- Default viewport camera matches vcad baseline:
  - azimuth `45deg`
  - elevation `30deg`
  - distance `100`
  - target `[0, 0, 0]`
  - fov `60deg`
- Camera interactions preserve vcad semantics:
  - orbit adjusts azimuth/elevation
  - vertical orbit clamps to `[-89deg, 89deg]`
  - pan uses camera-relative XZ and world-up Y movement
  - zoom uses multiplicative factors (`0.8` in, `1.25` out) and stable round trip
  - reset returns to default state
- Orientation gizmo exposes snap views:
  - `front`, `back`, `right`, `left`, `top`, `bottom`, `iso`, `hero`
- Transform gizmo modes are parity-locked:
  - `translate`, `rotate`, `scale`
- Grid snap increment stays parity-locked at `5mm`.

## Parity Artifacts

- vcad reference fixture:
  - `crates/cad/parity/fixtures/viewport_camera_gizmo_vcad_reference.json`
- Generated parity manifest:
  - `crates/cad/parity/viewport_camera_gizmo_parity_manifest.json`

## Validation

```bash
scripts/cad/parity-viewport-camera-gizmo-ci.sh
cargo run -p openagents-cad --bin parity-viewport-camera-gizmo
```
