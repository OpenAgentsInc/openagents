# Pane Dragging + Resizing Plan

This is the follow-on plan to add dragging/resizing to the HUD pane system. The current implementation is intentionally static; the structure already mirrors Commander’s store + pane renderer.

## Goals
- Drag panes by title bar.
- Resize from edges/corners.
- Keep panes within viewport bounds.
- Persist last position for tiling and reopen placement.

## Implementation Outline
1. **Pane Store**
   - Add `update_rect(id, rect)` back into `PaneStore` for drag/resize updates.
   - Preserve `last_pane_position` on every drag/resize end.

2. **Pane Frame Interaction**
   - Use the existing `PaneFrame` title bar as the drag handle.
   - Track drag start (`mouse_pos`, `pane_rect`) inside the pane UI layer.
   - On drag end, call `ensure_pane_visible` and update the store.

3. **Resize Handles**
   - Leverage `wgpui::components::hud::ResizablePane` to implement edge/corner handles.
   - Minimum size targets: 200x100 (match Commander).
   - Clamp to viewport margins (`PANE_MARGIN`).

4. **Z-order + Focus**
   - On drag start, call `bring_to_front(id)` to keep the moving pane on top.
   - Update `active_pane_id` accordingly.

5. **Persistence**
   - (Optional) add serialization of pane state once we introduce storage.

## Notes
- Commander uses `use-gesture` to handle drag + resize. In WGPUI we can mirror the same behavior via `InputEvent::MouseDown/Move/Up` and the `ResizablePane` component.
- The current HUD layer already centralizes all input routing in `MinimalRoot`; this makes it straightforward to add drag/resize without changing the architecture.

---

## Work Log
- Added pane drag state tracking and store updates in `MinimalRoot` so title-bar drags move panes.
- Implemented `PaneStore::update_rect` + `set_last_position` to persist the latest drag position.
- Routed drag start through `PaneFrame` title bounds (excluding close button) and clamped positions with `ensure_pane_visible`.
- Added resize hit-testing via `ResizablePane` with edge/corner drags that update pane rects.
- Enforced min size (200x100) + viewport clamping during resize and persisted last rect on release.

## Next Steps
- Persist pane positions to storage and restore on launch.
- Add drag cursor feedback and optional snap/tiling guides.
- Add visual resize affordances (optional handles/hover) and resize cursor feedback.
