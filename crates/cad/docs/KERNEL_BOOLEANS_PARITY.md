# Kernel Booleans Parity

Issue coverage: `VCAD-PARITY-018`

## Purpose

Integrate the staged boolean pipeline contract from `vcad-kernel-booleans` into `openagents-cad` kernel substrate.

## Implemented Boolean Pipeline Layer

`crates/cad/src/kernel_booleans.rs` now provides:

- staged pipeline operation types:
  - `KernelBooleanOp` (`Union`, `Difference`, `Intersection`)
- stage model:
  - `AabbFilter`
  - `SurfaceSurfaceIntersection`
  - `Classification`
  - `Reconstruction`
- deterministic pipeline reports:
  - `BooleanPipelineStageReport`
  - `BooleanReconstructionSummary`
  - `BooleanMeshFallbackSummary`
  - `BooleanPipelineResult` with deterministic signature
- execution entrypoint:
  - `run_staged_boolean_pipeline(left, right, op, config)`
- BRep output is preserved in parity lane results.
- mesh-only fallback output is removed in parity lane (`VCAD-PARITY-020`).

## Parity Artifact

- `crates/cad/parity/kernel_booleans_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-booleans
scripts/cad/parity-kernel-booleans-ci.sh
```

## Determinism Contract

- manifest captures stable stage order and deterministic signatures for union/difference/intersection sample runs.
- `crates/cad/tests/parity_kernel_booleans.rs` enforces fixture equivalence.
- lane is integrated into parity orchestration and CI evidence artifacts.
