# Kernel Fillet Parity

Issue coverage: `VCAD-PARITY-023`

## Purpose

Integrate deterministic parity support aligned to `vcad-kernel-fillet` for edge classification and all-edge chamfer/fillet substrate paths.

## Implemented Fillet Layer

`crates/cad/src/kernel_fillet.rs` now provides:

- `FilletCase` classification (`PlanePlane`, `PlaneCylinder`, `CylinderCylinderCoaxial`, `CylinderCylinderSkew`, `GeneralCurved`, `Unsupported`)
- `FilletResult` edge-level status contract (`Success`, `Unsupported`, `RadiusTooLarge`, `DegenerateGeometry`)
- `classify_fillet_case(surface_a, surface_b)`
- `extract_manifold_edge_adjacency(brep)` and `classify_manifold_edges(brep)`
- `chamfer_all_edges(brep, distance)`
- `fillet_all_edges(brep, radius)`
- `fillet_edges_detailed(brep, edge_ids, radius)`
- `closest_point_uv(surface, point, tolerance)` for plane/cylinder subset parity

Error contracts:

- invalid `distance` / `radius` maps to `CadError::InvalidParameter`
- oversized detailed-edge radii map to `FilletResult::RadiusTooLarge`

## Parity Artifact

- `crates/cad/parity/kernel_fillet_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-fillet
scripts/cad/parity-kernel-fillet-ci.sh
```

## Determinism Contract

- cube baseline contracts lock:
  - chamfer/fillet face count = `26`
  - fillet cylinder-surface count = `12`
- edge-case classification breakdowns are fixture-locked for cube and cylinder baselines.
- detailed-edge fixtures lock unsupported and radius-too-large result mapping.
- `crates/cad/tests/parity_kernel_fillet.rs` enforces fixture equivalence.
