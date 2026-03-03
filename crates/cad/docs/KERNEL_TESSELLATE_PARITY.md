# Kernel Tessellate Parity

Issue coverage: `VCAD-PARITY-016`

## Purpose

Integrate `vcad-kernel-tessellate` parity pipeline contracts into `openagents-cad` substrate APIs.

## Implemented Tessellation Layer

`crates/cad/src/kernel_tessellate.rs` now provides:

- `TriangleMesh` contract (`vertices`, `indices`, `normals`)
- `TessellationParams` (`circle_segments`, `height_segments`, `latitude_segments`)
- parity tessellation entrypoints:
  - `tessellate_solid(brep, params)`
  - `tessellate_brep(brep, segments)`
- deterministic primitive classification + tessellation for:
  - cube
  - cylinder
  - sphere
  - cone/frustum
- explicit diagnostics for missing/unsupported primitive topology.

## Parity Artifact

- `crates/cad/parity/kernel_tessellate_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-tessellate
scripts/cad/parity-kernel-tessellate-ci.sh
```

## Determinism Contract

- manifest captures stable mesh counts and signatures for primitive constructors.
- `crates/cad/tests/parity_kernel_tessellate.rs` enforces fixture equivalence.
- lane is integrated into `scripts/cad/parity_check.sh` and CI artifact evidence.
