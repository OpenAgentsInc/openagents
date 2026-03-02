# Parity Gap Matrix

Issue coverage: `VCAD-PARITY-004`

## Purpose

Generate a deterministic, machine-readable gap matrix from:

- `crates/cad/parity/vcad_capabilities_inventory.json`
- `crates/cad/parity/openagents_capabilities_inventory.json`

The matrix classifies each `vcad` reference capability row as:

- `matched`
- `missing`

using deterministic key normalization and token-similarity matching per surface
(`docs`, `crates`, `commands`).

## Output Artifact

- `crates/cad/parity/vcad_openagents_gap_matrix.json`

## Commands

Generate/update matrix:

```bash
cargo run -p openagents-cad --bin parity-gap-matrix --
```

Full drift check (inputs + matrix):

```bash
scripts/cad/parity-gap-matrix-ci.sh
```

## Test Coverage

- `crates/cad/tests/parity_gap_matrix.rs`
  - fixture schema/invariant checks
  - generation parity check against committed fixture
- unit tests live in:
  - `crates/cad/src/parity/gap_matrix.rs`
