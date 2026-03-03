# CAD STEP Import (Wave 2 Kickoff Scope)

This document defines the deterministic STEP import pipeline implemented for CAD backlog item 92.

## Scope

In scope:
- Deterministic STEP text import into `CadDocument`.
- Structural validation using the STEP checker before document creation.
- vcad-style entity coverage for `MANIFOLD_SOLID_BREP` + `OPEN/CLOSED_SHELL` +
  `FACE/ADVANCED_FACE` payloads.
- Stable semantic ID mapping for imported solids (`imported_solid_###` -> `solid.###`).
- `.apcad` persistence compatibility (stable IDs survive save/reload).
- Structured import result payload for observability (`CadStepImportResult`).

Out of scope:
- Full analytic B-Rep reconstruction and advanced topology healing.
- Assembly hierarchy import.
- STEP color/PMI/material parsing.
- UI-level import workflows in `apps/autopilot-desktop`.

## API

Implementation: `crates/cad/src/step_import.rs`

- `import_step_text_to_document(step_text, document_id) -> CadResult<CadStepImportResult>`
- `CadStepImportResult` includes:
  - `document: CadDocument`
  - `envelope: ApcadDocumentEnvelope`
  - `checker_report: CadStepCheckerReport`
  - `entity_coverage: CadStepImportEntityCoverage`
  - `import_hash: String`
  - `imported_feature_ids: Vec<String>`
  - `stable_ids: BTreeMap<String, String>`

## Determinism Rules

- Import hash uses deterministic FNV-1a over source STEP bytes.
- Solid-to-feature mapping is ordinal and stable:
  - solid #1 -> `feature.imported.solid.001`
  - semantic ref `imported_solid_001` -> `solid.001`
- Entity coverage is deterministic:
  - `entity_type_counts` uses lexicographically sorted `BTreeMap` keys.
  - `supported_entity_types_present` and `unsupported_entity_types_present`
    are stable sorted vectors.
- Imported document metadata keys are deterministic:
  - `import.format`
  - `import.hash`
  - `import.solid_count`
  - `import.shell_count`
  - `import.face_count`
- `.apcad` serialization preserves stable IDs via deterministic `BTreeMap` ordering.

## Failure Handling

Failures return explicit `CadError::ParseFailed` with actionable reason text.

Current failure classes:
- Empty `document_id`.
- Empty STEP payload.
- No solid entities detected by checker.
- Checker diagnostics marked as errors (`passed == false`).

Remediation hint:
- Re-export a valid STEP solid payload, then retry import.
- For checker shell diagnostics, both `CLOSED_SHELL` and `OPEN_SHELL` are valid.

## Observability

Import results include both structural checker data and deterministic import identity:
- `checker_report` captures solids/shells/faces, diagnostics, and checker backend metadata.
- `entity_coverage` captures supported vs unsupported STEP entity types seen in payload.
- `import_hash` is suitable for dedupe and replay-safe telemetry keys.
- Imported metadata is mirrored into `.apcad` envelope for downstream tooling.

## Validation Commands

- `cargo test -p openagents-cad step_import --quiet`
- `cargo test -p openagents-cad step_import_pipeline --quiet`
- `cargo check -p openagents-cad --quiet`
- `cargo check -p autopilot-desktop --quiet`
