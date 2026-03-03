# Kernel Boolean BRep Parity

Issue coverage: `VCAD-PARITY-020`

## Purpose

Preserve BRep outputs after boolean operations in the parity lane and remove mesh-only fallback output.

## Implemented BRep Preservation Layer

`crates/cad/src/kernel_booleans.rs` now:

- preserves BRep results in `BooleanPipelineResult::brep_result` for:
  - overlapping `Union`
  - overlapping `Difference`
  - overlapping `Intersection`
- emits `BooleanPipelineOutcome::BrepReconstruction` when BRep output is present.
- emits `BooleanPipelineOutcome::EmptyResult` for disjoint intersection instead of mesh fallback output.
- keeps deterministic stage sequencing without mesh fallback stage output artifacts.

## Parity Artifact

- `crates/cad/parity/kernel_boolean_brep_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-boolean-brep
scripts/cad/parity-kernel-boolean-brep-ci.sh
```

## Determinism Contract

- manifest captures `BrepReconstruction` outcomes for overlapping union/difference with `has_brep_result=true`.
- disjoint intersection is locked to `EmptyResult` with `mesh_fallback_present=false`.
- `crates/cad/tests/parity_kernel_boolean_brep.rs` enforces fixture equivalence.
