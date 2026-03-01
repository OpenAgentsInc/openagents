# CAD Feature Timeline UI

Timeline implementation lives in:

- `apps/autopilot-desktop/src/input/reducers/cad.rs`
- `apps/autopilot-desktop/src/panes/cad.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `crates/cad/src/history.rs`

## Scope

- Ordered feature rows (topological order from active graph).
- Row metadata:
  - feature name
  - operation type
  - status badge (`ok|warn|fail`)
  - provenance (`manual|ai`)
- Selected row parameter inspector.

## Interaction

- Mouse: click timeline rows to select.
- Keyboard: with CAD pane active:
  - `ArrowUp` selects previous row
  - `ArrowDown` selects next row
- Auto-scroll keeps selected row visible in a fixed-size visible window.

## History Binding

- Each rebuild commit pushes a command transition into `CadHistoryStack`.
- Timeline/inspector refresh and selection state are updated on rebuild commit.

## Verification

- `cargo test -p autopilot-desktop cad -- --nocapture`
