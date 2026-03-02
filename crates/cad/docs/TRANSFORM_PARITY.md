# Transform Parity

Issue coverage: `VCAD-PARITY-027`

## Purpose

Validate deterministic transform parity for translate/rotate/scale composition semantics.

This lane locks parity contracts for canonical vcad transform ordering:
`Primitive -> Scale -> Rotate -> Translate` (matrix form `T * Rz * Ry * Rx * S`).

## Implemented Transform Parity Lane

- Added deterministic transform parity module:
  - `crates/cad/src/parity/transform_parity.rs`
- Added manifest CLI:
  - `crates/cad/src/bin/parity-transform.rs`
- Added deterministic fixture tests:
  - `crates/cad/tests/parity_transform.rs`
- Added CI check script:
  - `scripts/cad/parity-transform-ci.sh`
- Added fixture artifact:
  - `crates/cad/parity/transform_parity_manifest.json`

## Contracts Locked

- Transform feature matrix composition matches kernel-math reference composition.
- Transform sequence composition is deterministic for repeated runs.
- Transform sequence is order-sensitive (`A -> B` differs from `B -> A`).
- Invalid non-positive scale maps to stable `CadError::InvalidPrimitive` diagnostics.

## Parity Artifact

- `crates/cad/parity/transform_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-transform
scripts/cad/parity-transform-ci.sh
```
