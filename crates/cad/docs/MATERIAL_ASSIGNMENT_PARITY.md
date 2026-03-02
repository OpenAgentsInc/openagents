# Material Assignment Parity

Issue coverage: `VCAD-PARITY-036`

## Purpose

Lock deterministic material assignment parity contracts at part and feature scopes.

## Implemented Material Assignment Contract Layer

- Extended `crates/cad/src/materials.rs` with deterministic assignment state/contracts:
  - `CadMaterialAssignmentState`
  - `CadMaterialAssignmentReceipt`
  - `CadMaterialAssignmentScope` (`feature`, `part`, `default`)
  - `set_part_material`, `set_feature_material`, `resolve_assignment`
- Contract behavior:
  - assignment precedence is deterministic: feature override > part assignment > default material.
  - material IDs are validated against known preset table and canonicalized.
  - assignment receipts include deterministic 16-char stable hashes.
  - unknown materials and invalid IDs return stable assignment errors.
- Added deterministic parity lane:
  - `crates/cad/src/parity/material_assignment_parity.rs`
  - `crates/cad/src/bin/parity-material-assignment.rs`
  - `crates/cad/tests/parity_material_assignment.rs`
  - `scripts/cad/parity-material-assignment-ci.sh`
  - `crates/cad/parity/material_assignment_parity_manifest.json`

## Contracts Locked

- feature-level assignment wins over part-level assignment.
- part-level assignment wins over default material.
- assignment resolution is deterministic across repeated runs.
- assignment hashes are stable for identical part/feature/material scope combinations.
- unknown material IDs fail deterministically with stable validation messaging.

## Parity Artifact

- `crates/cad/parity/material_assignment_parity_manifest.json`

Generation/check commands:

```bash
cargo run -p openagents-cad --bin parity-material-assignment
scripts/cad/parity-material-assignment-ci.sh
```
