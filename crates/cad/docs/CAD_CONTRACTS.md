# CAD Contracts

This document defines the reusable CAD payload contracts consumed by pane/UI/event systems.

## Contracts

- `CadAnalysis`: deterministic analysis snapshot for a document revision + variant.
- `CadSelectionState`: selection payload with stable selected entities and revision.
- `CadWarning`: warning receipt with stable code/severity/message/remediation fields.

Implementation: `crates/cad/src/contracts.rs`.

## Determinism Rules

- All payloads derive serde and use deterministic map key ordering (`BTreeMap`).
- Warning codes and selection kinds are typed enums.
- Payloads are designed for persistence and activity feed emission.

## Schema Verification

- Unit tests assert JSON serialization for:
  - `CadWarning`
  - `CadSelectionState`
  - `CadAnalysis`

Command:

- `cargo test -p openagents-cad`
