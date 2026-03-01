# CAD Dimension Editing

Backlog scope: issue #2526 (plan item 75).

## Interaction Model

- Dimension labels are rendered in the CAD pane as a dedicated `Dimensions` overlay.
- Click a row to begin edit mode for that dimension.
- Input is typed-only (numeric characters, `.` and optional leading `-`).
- Key handling while editing:
  - `Enter`: commit value
  - `Backspace`: remove last typed character
  - `Escape`: cancel edit session

## Deterministic Behavior

- Dimensions are bounded with min/max ranges per field.
- Commit applies a deterministic parameter mutation, increments `document_revision`, and queues rebuild.
- Rebuild trigger key format: `edit-dimension:<dimension_id>`.
- Undo/redo timeline receives rebuild transitions through existing history integration.

## Current Dimension Set

- `width_mm`
- `depth_mm`
- `height_mm`
- `wall_mm`

These values are mirrored into the demo feature graph so overlays/rebuild hashes/mesh output stay aligned after edit commits.
