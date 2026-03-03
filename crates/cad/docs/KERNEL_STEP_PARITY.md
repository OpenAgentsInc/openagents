# Kernel STEP Parity

Issue coverage: `VCAD-PARITY-025`

## Purpose

Integrate a deterministic `vcad-kernel-step` parity adapter path for BRep STEP read/write behavior in the kernel substrate lane.

## Implemented STEP Adapter Layer

`crates/cad/src/kernel_step.rs` now provides:

- `StepAdapterError` with vcad-aligned categories (`Io`, `Parse`, `MissingEntity`, `UnsupportedEntity`, `InvalidGeometry`, `InvalidTopology`, `TypeMismatch`, `NoSolids`)
- `write_step(solid, path)` and `write_step_to_buffer(solid)`
- `read_step(path)` and `read_step_from_buffer(bytes)`
- `tokenize_step(bytes)` and `parse_step_entity_ids(bytes)` deterministic parser metadata helpers
- `read_step_to_cad` / `write_step_to_cad` wrappers with `CadError` mapping

Adapter scope for this parity lane:

- deterministic STEP envelope emission for kernel BRep snapshots
- deterministic summary payload round-trip for adapter-authored STEP data
- stable parse/error semantics for malformed or empty-solid inputs

## Parity Artifact

- `crates/cad/parity/kernel_step_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-step
scripts/cad/parity-kernel-step-ci.sh
```

## Determinism Contract

- writing the same BRep twice yields byte-identical STEP bytes.
- adapter-authored STEP buffers round-trip to stable topology counts.
- token/entity-id extraction is fixture-locked for deterministic parser metadata.
- no-solid and invalid-UTF8 error paths are fixture-locked.
- `crates/cad/tests/parity_kernel_step.rs` enforces fixture equivalence.
