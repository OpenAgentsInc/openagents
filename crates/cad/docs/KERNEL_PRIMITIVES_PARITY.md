# Kernel Primitives Parity

Issue coverage: `VCAD-PARITY-015`

## Purpose

Integrate `vcad-kernel-primitives` constructor parity into `openagents-cad` substrate APIs.

## Implemented Constructor Layer

`crates/cad/src/kernel_primitives.rs` now provides:

- `BRepSolid` payload (`topology`, `geometry`, `solid_id`)
- constructor parity functions:
  - `make_cube(sx, sy, sz)`
  - `make_cylinder(radius, height, segments)`
  - `make_sphere(radius, segments)`
  - `make_cone(radius_bottom, radius_top, height, segments)`
- equal-radii cone fallback contract: cone routes to cylinder constructor
- deterministic dimension validation and `CadError::InvalidPrimitive` diagnostics

## Parity Artifact

- `crates/cad/parity/kernel_primitives_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-primitives
scripts/cad/parity-kernel-primitives-ci.sh
```

## Determinism Contract

- manifest captures stable topology and surface counts for cube/cylinder/sphere/cone variants.
- `crates/cad/tests/parity_kernel_primitives.rs` enforces fixture equivalence.
- lane is integrated into `scripts/cad/parity_check.sh` and CI artifact evidence.
