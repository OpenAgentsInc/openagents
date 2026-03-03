# Primitive Contracts Parity

Issue coverage: `VCAD-PARITY-026`

## Purpose

Establish deterministic primitive-op parity contracts for core modeling primitives:

- cube (`box`)
- cylinder
- sphere
- cone/frustum

This lane validates parity at the primitive operation contract layer (`primitives` + kernel adapter routing), distinct from substrate BRep constructor parity.

## Implemented Primitive Contract Layer

- Extended primitive specs in `crates/cad/src/primitives.rs`:
  - `SpherePrimitive`
  - `ConePrimitive`
  - `PrimitiveSpec::{Sphere, Cone}`
- Extended kernel adapter contracts in `crates/cad/src/kernel.rs`:
  - v1 adapter defaults: `create_sphere`, `create_cone`
  - v2 bridge support: `create_sphere_v2`, `create_cone_v2`
  - capability set includes `PrimitiveSphere`, `PrimitiveCone`
- Added deterministic primitive contract parity lane:
  - `crates/cad/src/parity/primitive_contracts_parity.rs`
  - `crates/cad/src/bin/parity-primitive-contracts.rs`
  - `crates/cad/tests/parity_primitive_contracts.rs`
  - `scripts/cad/parity-primitive-contracts-ci.sh`
  - `crates/cad/parity/primitive_contracts_parity_manifest.json`

## Error Contracts

- invalid sphere radius maps to `CadError::InvalidPrimitive`
- invalid cone top radius (`< 0`) maps to `CadError::InvalidPrimitive`

## Parity Artifact

- `crates/cad/parity/primitive_contracts_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-primitive-contracts
scripts/cad/parity-primitive-contracts-ci.sh
```

## Determinism Contract

- primitive routing call counts are fixture-locked (`box=1`, `cylinder=1`, `sphere=1`, `cone=2`).
- primitive handle snapshots are deterministic for fixed dimensions.
- replaying the same primitive sequence yields identical snapshots.
- v2 bridge emits deterministic receipts for sphere/cone operations.
