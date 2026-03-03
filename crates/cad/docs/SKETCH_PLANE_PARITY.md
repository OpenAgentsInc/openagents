# Sketch Plane Parity

Issue coverage: `VCAD-PARITY-042`

## Purpose

Lock deterministic sketch plane parity for:

- Standard sketch plane presets: `XY`, `XZ`, `YZ`
- Planar face selection from existing BRep geometry

## OpenAgents Plane Contracts

- `CadSketchPlane` now has explicit preset constructors:
  - `CadSketchPlane::xy()`
  - `CadSketchPlane::xz()`
  - `CadSketchPlane::yz()`
  - `CadSketchPlane::from_preset(CadSketchPlanePreset::{Xy|Xz|Yz})`
- Preset bases match vcad reference semantics:
  - `XY` -> origin `[0,0,0]`, x `[1,0,0]`, y `[0,1,0]`, normal `[0,0,1]`
  - `XZ` -> origin `[0,0,0]`, x `[1,0,0]`, y `[0,0,1]`, normal `[0,-1,0]`
  - `YZ` -> origin `[0,0,0]`, x `[0,1,0]`, y `[0,0,1]`, normal `[1,0,0]`
- Planar face selection is provided through:
  - `CadSketchPlane::from_planar_face(&brep, "face.<id>")`
  - `CadSketchPlane::from_planar_face_with_identity(...)`
- Face selection behavior:
  - accepts only `face.<id>` refs present in `BRepSolid.topology.faces`
  - requires `SurfaceRecord::Plane` at the face surface index
  - applies face orientation to sketch-plane normal (`Reversed` flips normal and y-axis)
  - rejects non-planar faces with deterministic parse diagnostics

## Parity Evidence

- Manifest generator/check:
  - `cargo run -p openagents-cad --bin parity-sketch-plane -- --check`
- Manifest fixture:
  - `crates/cad/parity/sketch_plane_parity_manifest.json`
- Integration test:
  - `cargo test -p openagents-cad --test parity_sketch_plane --quiet`

## Failure Modes

- Preset basis drift (IDs, axes, normals) fails manifest parity checks.
- Planar-face orientation mapping drift fails face-case summaries.
- Non-planar face acceptance (instead of rejection) fails diagnostics parity checks.
- Non-deterministic replay of selection corpus fails deterministic signature checks.
