# CAD Overflow Invariants (Backlog 80)

This note tracks render-time clipping invariants and regression coverage for the CAD pane.

## Invariants

- The entire CAD pane body is clipped to `content_bounds`.
- Engineering overlay is clipped to the viewport bounds.
- Warning panel rows/text are clipped to warning panel bounds.
- Dimension panel rows/edit errors are clipped to dimension panel bounds.
- Timeline rows/inspect panel are clipped to timeline panel bounds.
- Context menu rows/labels are clipped to context menu bounds.
- Footer summary is truncated to a bounded character length before rendering.

## Regression Coverage

Bounds/layout regression tests already present in `pane_system.rs`:
- warning panel + markers within content
- timeline panel rows within content
- dimension panel rows within content

Render/readability regression tests in `panes/cad.rs`:
- `footer_summary_line_truncates_for_long_tokens`
- `truncate_with_ellipsis_respects_limit`
- `tile_caption_is_compact_and_readable`

## Verification Commands

```bash
cargo test -p autopilot-desktop cad_warning_panel_and_markers_stay_within_content_no_overflow --quiet
cargo test -p autopilot-desktop cad_timeline_panel_rows_stay_within_content_in_small_panes --quiet
cargo test -p autopilot-desktop cad_dimension_panel_rows_stay_within_content --quiet
cargo test -p autopilot-desktop footer_summary_line_truncates_for_long_tokens --quiet
```
