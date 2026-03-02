# Kernel Math Parity

Issue coverage: `VCAD-PARITY-012`

## Purpose

Integrate `vcad-kernel-math` parity types and adapters into `openagents-cad` with deterministic behavior.

## Implemented Surface

`crates/cad/src/kernel_math.rs` now provides:

- scalar/type aliases and primitives: `Point3`, `Vec3`, `Dir3`, `Point2`, `Vec2`
- affine `Transform` with translation/scale/axis rotations/composition/inverse
- `Tolerance` contract matching vcad defaults:
  - `linear = 1e-6`
  - `angular = 1e-9`
- adapters to existing CAD measurement type:
  - `From<CadMeasurePoint3> for Point3/Vec3`
  - `From<Point3/Vec3> for CadMeasurePoint3`
  - transform matrix adapters: `From<[f64;16]> for Transform` and reverse

## Parity Artifact

- `crates/cad/parity/kernel_math_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-math
scripts/cad/parity-kernel-math-ci.sh
```

## Determinism Contract

- manifest captures stable sample results for translation, rotation, axis rotation, composition, inverse round-trip error, tolerance equality checks, and adapter round-trip values.
- `crates/cad/tests/parity_kernel_math.rs` enforces fixture equivalence.
- lane is integrated into `scripts/cad/parity_check.sh`.
