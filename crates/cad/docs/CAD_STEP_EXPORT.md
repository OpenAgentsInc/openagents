# CAD STEP Export (Wave 1 Demo Scope)

This document defines the deterministic STEP export contract implemented for CAD backlog item 76.

## Scope

In scope:
- Deterministic STEP text generation from `CadMeshPayload`.
- Solid-only export path (`FACETED_BREP` + closed shell faces).
- Deterministic post-boolean BRep export path for `BooleanPipelineOutcome::BrepReconstruction`.
- No assembly graph, PMI, color styling, or manufacturing metadata.
- Stable export receipt with byte count + deterministic hash for observability.

Out of scope:
- STEP import (implemented separately in `crates/cad/docs/CAD_STEP_IMPORT.md`).
- Multi-body assembly structure.
- High-fidelity analytic B-Rep surfaces beyond faceted export.
- Non-STEP export formats.

## API

`crates/cad/src/export.rs`

- `export_step_from_mesh(document_id, document_revision, variant_id, mesh) -> CadStepExportArtifact`
- `export_step_from_brep(document_id, document_revision, variant_id, brep) -> CadStepBrepExportArtifact`
- `export_step_from_post_boolean_brep(document_id, document_revision, variant_id, brep_result, outcome) -> CadStepBrepExportArtifact`
- `can_export_post_boolean_step(brep_result) -> bool`
- `CadStepExportArtifact { receipt, bytes }`
- `CadStepExportReceipt` includes:
  - `document_id`
  - `document_revision`
  - `variant_id`
  - `mesh_id`
  - `file_name`
  - `triangle_count`
  - `byte_count`
  - `deterministic_hash`
- `CadStepBrepExportArtifact { receipt, bytes }`
- `CadStepBrepExportReceipt` includes:
  - `document_id`
  - `document_revision`
  - `variant_id`
  - `file_name`
  - `solid_count`
  - `shell_count`
  - `face_count`
  - `byte_count`
  - `deterministic_hash`

## Determinism Rules

- Header timestamp is fixed to `1970-01-01T00:00:00`.
- Entity IDs are allocated in stable insertion order.
- Triangle traversal follows the incoming `triangle_indices` order.
- Coordinate formatting is fixed to six decimal places (`mm` units).
- File naming uses deterministic segment sanitization:
  - `<document>-<variant>-r<revision>.step`

Given identical inputs, exports must be byte-identical and produce the same receipt hash.

## Failure Handling

All export failures return `CadError::ExportFailed { format: "step", reason }`.

Current failure classes:
- Empty document ID.
- Empty variant ID.
- Mesh contract invalid.
- Variant mismatch between request and payload.
- Zero triangles.
- Degenerate triangle face.
- Non-finite vertex coordinates.
- Post-boolean mesh-only export attempts:
  - `cannot export to STEP: solid has been converted to mesh (B-rep data lost after boolean operations)`
- Post-boolean empty export attempts:
  - `cannot export to STEP: solid is empty`

Remediation hint:
- Rebuild the active variant to refresh a valid mesh payload.
- Retry export with the active variant ID.
- For post-boolean export, ensure the operation preserved a BRep result.
- Fix invalid geometry conditions before export.

## Observability

Desktop CAD reducer emits:
- `CadEventKind::ExportCompleted` with file name, bytes, and deterministic hash.
- `CadEventKind::ExportFailed` with error and remediation hint.

This enables activity lane verification and deterministic replay checks for export flows.
