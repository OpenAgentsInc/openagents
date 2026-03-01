# CAD Warnings Panel and Markers

CAD warning UI integration is implemented in:

- `apps/autopilot-desktop/src/input/reducers/cad.rs`
- `apps/autopilot-desktop/src/panes/cad.rs`
- `apps/autopilot-desktop/src/pane_system.rs`

## Behavior

- Warning panel shows rebuild-linked model validity warnings.
- Filters:
  - severity (`all -> critical -> warning -> info`)
  - code (`all` + discovered warning codes)
- Clicking a warning row focuses implicated geometry.
- Clicking a marker focuses the mapped warning.
- Fallback focus path is feature-level (`cad://feature/<feature_id>`) when entity deep link is unavailable.

## Marker Lifecycle

- Marker overlays are regenerated from current warning set after each rebuild commit.
- Stale hover/focus marker state is cleared on rebuild commit.
- Marker and panel bounds are clamped to pane content.

## Verification

- `cargo test -p autopilot-desktop cad -- --nocapture`
