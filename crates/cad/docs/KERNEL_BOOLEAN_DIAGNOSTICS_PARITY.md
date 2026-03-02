# Kernel Boolean Diagnostics Parity

Issue coverage: `VCAD-PARITY-019`

## Purpose

Map staged `vcad-kernel-booleans` diagnostics into the OpenAgents CAD error model.

## Implemented Diagnostics Mapping Layer

`crates/cad/src/kernel_booleans.rs` now provides:

- deterministic boolean diagnostics:
  - `BooleanDiagnostic`
  - `BooleanDiagnosticCode`
  - `BooleanDiagnosticSeverity`
- mapping entrypoints:
  - `map_boolean_diagnostic_to_cad_error`
  - `boolean_diagnostics_to_cad_errors`
  - `primary_boolean_cad_error`
- staged diagnostics emitted from the parity pipeline for:
  - disjoint AABB operands
  - empty SSI/classification stages
  - staged reconstruction fallback behavior
  - empty intersection outcomes

`crates/cad/src/error.rs` now includes:

- `CadErrorCode::BooleanDiagnostic`
- `CadError::BooleanDiagnostic { diagnostic_code, stage, severity, reason, retryable }`

## Parity Artifact

- `crates/cad/parity/kernel_boolean_diagnostics_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-kernel-boolean-diagnostics
scripts/cad/parity-kernel-boolean-diagnostics-ci.sh
```

## Determinism Contract

- manifest captures deterministic diagnostic-code sequences and mapped error-code sequences.
- `crates/cad/tests/parity_kernel_boolean_diagnostics.rs` enforces fixture equivalence.
- lane is integrated into parity orchestration and CI evidence artifacts.
