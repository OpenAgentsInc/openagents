# Overview

The HUD pane system is a "window manager" inside a single web page:

1. A full-screen canvas area (optionally dotted) that can be panned.
2. A stack of floating panes, ordered back-to-front.
3. A hotbar at the bottom center that triggers pane-related actions.

This mirrors the existing Autopilot Desktop behavior (Rust/WGPUI):

- `PaneStore` is the canonical source of pane state.
- Pointer interactions mutate the store (drag/resize/pan).
- Rendering reflects the store state.

## Core Concepts

### Pane geometry (`PaneRect`)

Each pane has a rectangle:

- `x`, `y`: top-left position in pixels relative to the root container
- `width`, `height`: size in pixels

The store enforces minimum sizes using `normalizePaneRect()`:

- min width: 200px
- min height: 100px

(These values match `PANE_MIN_WIDTH` / `PANE_MIN_HEIGHT` in `crates/autopilot_ui/src/lib.rs`.)

### Z-order = array order

In both Rust and this TS port:

- `PaneStore.panes()` returns panes in back-to-front order.
- The last element is the top-most pane.

Bringing a pane to the front is a pure list operation:

- Remove it from its index.
- Push it to the end.

### Active pane

`PaneStore.activePaneId` tracks focus.

Typical invariants:

- After opening or bringing-to-front, the pane becomes active.
- Closing the active pane makes the *new last pane* active.

### Close vs toggle vs restore

The store supports a specific "toggle" behavior:

- If pane exists and is active: closing it stores its last rectangle in `closedPositions`.
- If pane exists but is not active: activating it brings it to the front.
- If pane does not exist:
  - If there is a snapshot in `closedPositions`, the snapshot rectangle can be used.
  - Otherwise a new rectangle is computed.

This enables the user experience:

- "Open events pane"
- Move/resize it
- Close it
- Later reopen it and it comes back in the last position

### Canvas panning

When the user drags the empty background (not over any pane):

- All pane rects are offset by `dx/dy`.
- The dotted background offset is updated too.

This matches the WGPUI behavior in `MinimalRoot`:

- Panning is a global translation applied to the whole pane stack.

### Dragging a pane

Drag behavior:

- Only the title bar starts a drag (matching "drag by chrome").
- The close button is excluded.
- When a drag starts, the pane is brought to the front.
- While dragging, `PaneStore.updateRect()` is called with the updated rect.
- On drag end, `PaneStore.setLastPosition()` records the final rect for future placement.

### Resizing a pane

Resize behavior:

- Resizing can be initiated from any edge/corner.
- Hit-testing uses `ResizablePane.edgeAt(bounds, point)` (ported from Rust).
- While resizing, `PaneStore.updateRect()` is called with a normalized rect.
- On resize end, `PaneStore.setLastPosition()` records the final rect.

## Keyboard Interactions

The DOM adapter currently handles:

- `Escape`: close active pane (if dismissable).
- `Cmd/Ctrl + 0..9`: trigger hotbar slots (and flash the corresponding slot).

The hotbar key bindings mirror `docs/autopilot-old/HUD.md` and the desktop shortcut scheme.

## Why The DOM Adapter Is Intentionally Thin

The "pane system" is a platform primitive. Your app owns:

- what panes exist (types/kinds)
- what each pane renders
- what each hotbar slot does
- persistence beyond the in-memory store (optional)

The package owns:

- deterministic state transitions for pane geometry and ordering
- minimal chrome and pointer/keyboard plumbing

This makes it easier to:

- unit test core behavior without a browser
- add multiple renderers later (Effuse adapter, React adapter, canvas renderer)

