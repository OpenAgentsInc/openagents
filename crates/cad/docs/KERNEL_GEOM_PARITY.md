# Kernel Geom Parity

Issue coverage: `VCAD-PARITY-014`

## Purpose

Integrate `vcad-kernel-geom` parity surface types and evaluation contracts into `openagents-cad`.

## Implemented Surface Layer

`crates/cad/src/kernel_geom.rs` now provides:

- `SurfaceKind` and trait-object `Surface` contract
- analytic surface implementations:
  - `Plane`
  - `CylinderSurface`
  - `ConeSurface`
  - `SphereSurface`
  - `TorusSurface`
  - `BilinearSurface`
- `SurfaceRecord` enum and `GeometryStore` for deterministic storage/indexing

## Parity Artifact

- `crates/cad/parity/kernel_geom_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-geom
scripts/cad/parity-kernel-geom-ci.sh
```

## Determinism Contract

- manifest captures stable sample evaluations across all supported surface kinds.
- `crates/cad/tests/parity_kernel_geom.rs` enforces fixture equivalence.
- lane is integrated into `scripts/cad/parity_check.sh`.
