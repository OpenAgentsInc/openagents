# Architecture

The package is split into a small "core" plus an optional DOM adapter.

```
packages/effuse-panes/
  src/
    types.ts           geometry primitives (Point/Bounds) + helpers
    paneStore.ts       PaneStore + rect normalization + placement helpers
    resizablePane.ts   ResizeEdge + hit test + pure resize math
    hotbar.ts          HotbarModel + HotbarSlot data model
    paneSystemDom.ts   mountPaneSystemDom(): pointer+keyboard -> store mutations -> DOM
  test/
    paneStore.test.ts
    resizablePane.test.ts
```

## Module Responsibilities

### `src/types.ts`

- Defines the "geometry vocabulary":
  - `Point`, `Size`, `Bounds`, `PaneRect`
- Includes `boundsContains(bounds, point)` which matches the "bounds.contains" style checks in Rust.

### `src/paneStore.ts`

Port of the `PaneStore` in `crates/autopilot_ui/src/lib.rs`.

Owns:

- pane list (z-order)
- active pane id
- last pane position (for "new pane placement")
- closed snapshots (for restore-on-toggle)

Does not own:

- pointer state (drag/resizing are orchestrated by adapter)
- rendering
- persistence to disk

Key API:

- `addPane()`
- `removePane(storePosition)`
- `bringToFront()`
- `updateRect()`
- `togglePane(create(snapshot))`

Placement helpers:

- `calculateNewPanePosition(last, screen, width, height)`
- `normalizePaneRect(rect)`

### `src/resizablePane.ts`

Port of `crates/wgpui/src/components/hud/resizable_pane.rs` "math core":

- `ResizeEdge` enum
- `hitTestResizeEdge()` (edge/corner hit test)
- `ResizablePane.resizeBounds()` (pure bounds calculation given drag state)

Important: this is intentionally UI-agnostic:

- it does not paint resize handles
- it does not track hover state
- it only answers: "which edge is this point over?" and "what bounds result from this drag?"

### `src/hotbar.ts`

Data-only model for:

- hotbar items (`HotbarSlot`)
- temporary flash state (`flashSlot()`)
- click collection (`takeClickedSlots()`)

This mirrors the Rust `Hotbar` *state* behavior, but does not attempt to replicate its exact visuals.

### `src/paneSystemDom.ts`

This is the "thin adapter" that makes the system usable in a browser.

It:

- injects minimal CSS for panes/hotbar (no Tailwind required)
- creates DOM nodes for:
  - a pane layer container
  - a hotbar container
- wires events:
  - pointer down/move/up for drag/resize/pan
  - keydown for Escape + Cmd/Ctrl+0..9
  - click for hotbar slots
- schedules render work:
  - pointer-move rendering is batched through `requestAnimationFrame`
  - hotbar flash clear render uses a short timeout
- holds the transient pointer states:
  - `paneDrag`, `paneResize`, `canvasPan`
- cleans up all adapter-owned resources in `destroy()`:
  - DOM listeners
  - injected adapter nodes (`style`, pane layer, hotbar)
  - pending animation frames and pending timeout callbacks

It intentionally does **not** render any pane contents.

## Design Constraints

### 1) Do not couple to app content

The pane system should not need to know:

- what a "chat pane" is
- what a "threads pane" is
- what your tool-call cards look like

That knowledge belongs to the host app.

### 2) Keep the "core" unit-testable

`PaneStore` + `ResizablePane` are pure enough that tests do not need a browser.

### 3) Preserve the WGPUI semantics

Even if the DOM adapter visuals differ, the *behavioral contract* should match:

- store ordering and active pane rules
- toggle close/restore semantics
- drag/resize math and constraints

See `PARITY_CHECKLIST.md` for details.
