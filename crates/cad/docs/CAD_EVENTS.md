# CAD Events

`CadEvent` is the typed event contract for CAD-to-UI observability and activity feed integration.

## Event Schema

Module: `openagents_cad::events`

- `CadEventKind`
  - `DocumentCreated`
  - `VariantGenerated`
  - `SelectionChanged`
  - `WarningRaised`
  - `ParameterUpdated`
  - `RebuildCompleted`
  - `AnalysisUpdated`
  - `ExportCompleted`
  - `ExportFailed`
- `CadEvent`
  - `event_id`
  - `kind`
  - `session_id`
  - `document_id`
  - `document_revision`
  - `variant_id`
  - `summary`
  - `detail`

## Deterministic Dedupe

- Event IDs are built with deterministic keys via `build_cad_event_id(...)`.
- IDs include kind/session/document/revision/variant and a caller-provided stable key.
- Replayed emissions with the same key upsert instead of duplicating rows.

## Activity Feed Mapping

- CAD reducer emits `CadEvent` and mirrors each event into activity feed rows.
- Activity feed domain: `cad`.
- Source tag format: `cad.<event_kind>`.
- CAD pane also retains a bounded in-memory event history (`CadDemoPaneState::cad_events`) for refresh/replay behavior.
