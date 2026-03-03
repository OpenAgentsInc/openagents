# Kernel NURBS Parity

Issue coverage: `VCAD-PARITY-021`

## Purpose

Integrate `vcad-kernel-nurbs` parity support for B-spline/NURBS curves and surfaces.

## Implemented NURBS Layer

`crates/cad/src/kernel_nurbs.rs` now provides:

- `BSplineCurve`
  - `new`
  - `clamped_uniform`
  - `evaluate`
  - `parameter_domain`
  - `insert_knot`
- `WeightedPoint`
- `NurbsCurve`
  - `new`
  - `evaluate`
  - `parameter_domain`
  - `circle`
- `BSplineSurface`
  - `new`
  - `evaluate`
  - `parameter_domain`
- `NurbsSurface`
  - `new`
  - `evaluate`
  - `parameter_domain`

Error contracts:

- invalid knot vectors map to `CadError::InvalidParameter`
- invalid weights map to `CadError::InvalidParameter`

## Parity Artifact

- `crates/cad/parity/kernel_nurbs_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-nurbs
scripts/cad/parity-kernel-nurbs-ci.sh
```

## Determinism Contract

- manifest locks deterministic curve/surface sample snapshots and signatures.
- manifest includes knot-insertion drift checks for B-spline curves.
- `crates/cad/tests/parity_kernel_nurbs.rs` enforces fixture equivalence.
