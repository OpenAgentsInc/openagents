# Kernel Precision Parity

Issue coverage: `VCAD-PARITY-017`

## Purpose

Align tolerance defaults and exact-predicate strategy with `vcad-kernel-math` behavior.

## Implemented Precision Layer

- Added robust predicate module: `crates/cad/src/kernel_predicates.rs`
  - `Sign`
  - `orient2d`, `orient3d`
  - `incircle`, `insphere`
  - derived checks:
    - `point_on_segment_2d`
    - `point_on_plane`
    - `are_collinear_2d`
    - `are_coplanar`
    - `point_side_of_line`
- Updated tolerance policy defaults in `crates/cad/src/policy.rs`:
  - linear tolerance: `1e-6 mm`
  - angular tolerance: `1e-9 rad`
  - default predicate strategy: `AdaptiveExact`
  - precision helpers for linear/angular tolerance checks
- Added `robust` crate dependency for adaptive-precision geometric predicates.

## Parity Artifact

- `crates/cad/parity/kernel_precision_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-precision
scripts/cad/parity-kernel-precision-ci.sh
```

## Determinism Contract

- manifest captures exact-predicate sign outcomes for near-collinear and near-coplanar inputs.
- `crates/cad/tests/parity_kernel_precision.rs` enforces fixture equivalence.
- lane is integrated into parity orchestration and CI evidence artifacts.
