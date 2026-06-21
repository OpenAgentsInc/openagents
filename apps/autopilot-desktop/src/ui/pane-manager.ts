// HUD H3 (#5501) — the managed pane layer over the zero-base shell.
//
// THE DURABLE PATTERN (audit §4.1, the one that survived three rewrites):
// a pane is plain data — `{ id, kind, rect{x,y,w,h}, z }` — a store holds the
// open set, and a typed PaneManager reducer (open/close/focus/move/resize +
// cascade/clamp) is the ONLY thing that mutates it. The render is a pure
// function of that data (view.ts), and the body of each pane reuses the EXISTING
// `paneView` content keyed by `kind` (a `PaneId`), so the kept full-UI panes
// render inside a managed window with zero new content code.
//
// What this is NOT (the §5.2 anti-sprawl learning): it is not the old
// free-floating window sprawl that opens on top of itself. Panes open ONLY on an
// explicit open (hotbar / command palette), cascade so a new pane never stacks
// exactly on the last one, and clamp into the viewport so a pane can never be
// dragged off-screen and lost. The base surface (the black shell) stays black
// and empty until a pane is opened.
//
// This module is PURE + DOM-free so the whole layer is unit-testable: the
// reducer takes `(state, action, viewport) → state` and the geometry helpers are
// plain math. The Foldkit Model embeds `PaneLayer` as a field; update.ts routes
// the pane-layer messages straight through here.

import { Schema as S } from "effect"

import { PaneId } from "./model.js"

// A managed pane's content is one of the EXISTING panes (reuse `paneView`). We
// alias `PaneId` as the kind so adding a windowable surface needs no new union —
// the registry/`paneView` switch is still the single source of pane content.
// `shell` is deliberately excluded as a managed kind (it is the base surface, not
// a window); the opener guards against it.
export const ManagedPaneKind = PaneId
export type ManagedPaneKind = typeof ManagedPaneKind.Type

// A rectangle in webview CSS pixels (top-left origin). All geometry is integer
// pixels so the projection + tests are exact.
export const PaneRect = S.Struct({
  x: S.Number,
  y: S.Number,
  w: S.Number,
  h: S.Number,
})
export type PaneRect = typeof PaneRect.Type

// Pane-as-data (audit §4.1). `id` is a stable per-open identifier (kind + seq);
// `z` is the stacking order (higher = in front, focused). `rect` is the live
// geometry the drag/resize reducer mutates.
export const ManagedPane = S.Struct({
  id: S.String,
  kind: ManagedPaneKind,
  rect: PaneRect,
  z: S.Number,
})
export type ManagedPane = typeof ManagedPane.Type

// The transient drag/resize gesture in flight. `kind` distinguishes a title-bar
// move from one of the 8 resize handles. `handle` is the active resize handle
// (Commander's 8-handle set, audit §4.6). `startPointer` + `startRect` are
// captured at pointer-down so each move applies an absolute delta (no drift).
export const PaneDragKind = S.Literals(["move", "resize"])
export type PaneDragKind = typeof PaneDragKind.Type

export const PaneResizeHandle = S.Literals([
  "topleft",
  "top",
  "topright",
  "right",
  "bottomright",
  "bottom",
  "bottomleft",
  "left",
])
export type PaneResizeHandle = typeof PaneResizeHandle.Type

export const PaneDrag = S.Struct({
  paneId: S.String,
  kind: PaneDragKind,
  // Null for a move; the active handle for a resize.
  handle: S.NullOr(PaneResizeHandle),
  startPointerX: S.Number,
  startPointerY: S.Number,
  startRect: PaneRect,
})
export type PaneDrag = typeof PaneDrag.Type

// The whole managed-pane layer state. Embedded as one Model field so the rest of
// the Model is untouched (no regression to the single-pane router or the shell).
export const PaneLayer = S.Struct({
  panes: S.Array(ManagedPane),
  // Monotonic counter: backs both unique ids and the next z-order, so a freshly
  // opened/focused pane is always in front and ids never collide.
  seq: S.Number,
  // The gesture in flight, or null when idle.
  drag: S.NullOr(PaneDrag),
})
export type PaneLayer = typeof PaneLayer.Type

export const emptyPaneLayer: PaneLayer = {
  panes: [],
  seq: 0,
  drag: null,
}

// ── Geometry constants ──────────────────────────────────────────────────────
// Default opened-pane size + the cascade step + min size + the viewport margin
// the clamp keeps a pane fully inside. Conservative defaults that look good on
// the desktop webview; the clamp re-fits if the window is smaller.
export const PANE_DEFAULT_WIDTH = 720
export const PANE_DEFAULT_HEIGHT = 480
export const PANE_MIN_WIDTH = 280
export const PANE_MIN_HEIGHT = 180
// Cascade offset so a new pane never lands exactly on the previous one (the
// anti-sprawl core: opens are visibly staggered, never stacked, audit §4.2/§4.6).
export const PANE_CASCADE_STEP = 32
// Keep panes off the very top (the ← Shell / hotbar chrome) and edges.
export const PANE_VIEWPORT_MARGIN = 16
// The first pane's top-left anchor (below the top chrome).
const PANE_ORIGIN_X = 64
const PANE_ORIGIN_Y = 64

// ── Pure geometry helpers ───────────────────────────────────────────────────

// Clamp a rect fully inside a viewport (audit §4.6 — Commander's
// `ensurePaneIsVisible`). Width/height are first floored to the minimums and
// capped to the viewport (minus margins) so a pane is always at least partly
// grabbable and never larger than the screen; then the position is pulled in so
// the whole pane stays visible. A pane can never be dragged off-screen and lost.
export const clampRect = (
  rect: PaneRect,
  viewportWidth: number,
  viewportHeight: number,
): PaneRect => {
  const maxW = Math.max(PANE_MIN_WIDTH, viewportWidth - PANE_VIEWPORT_MARGIN * 2)
  const maxH = Math.max(PANE_MIN_HEIGHT, viewportHeight - PANE_VIEWPORT_MARGIN * 2)
  const w = Math.min(Math.max(rect.w, PANE_MIN_WIDTH), maxW)
  const h = Math.min(Math.max(rect.h, PANE_MIN_HEIGHT), maxH)
  const minX = PANE_VIEWPORT_MARGIN
  const minY = PANE_VIEWPORT_MARGIN
  const maxX = Math.max(minX, viewportWidth - PANE_VIEWPORT_MARGIN - w)
  const maxY = Math.max(minY, viewportHeight - PANE_VIEWPORT_MARGIN - h)
  const x = Math.min(Math.max(rect.x, minX), maxX)
  const y = Math.min(Math.max(rect.y, minY), maxY)
  return { x, y, w, h }
}

// Cascade placement for a NEW pane (audit §4.6 — Commander's
// `calculatePanePosition`). Offsets each successive open by `PANE_CASCADE_STEP`
// from the origin, wrapping back near the origin once the cascade would push a
// pane off the bottom/right, so opens stay staggered (never stacked) and stay on
// screen. `openCount` is how many panes are already open.
export const placeNewPaneRect = (
  openCount: number,
  viewportWidth: number,
  viewportHeight: number,
): PaneRect => {
  const w = PANE_DEFAULT_WIDTH
  const h = PANE_DEFAULT_HEIGHT
  // How many cascade steps fit before we'd run off the usable area.
  const usableW = Math.max(1, viewportWidth - PANE_VIEWPORT_MARGIN - PANE_ORIGIN_X - w)
  const usableH = Math.max(1, viewportHeight - PANE_VIEWPORT_MARGIN - PANE_ORIGIN_Y - h)
  const stepsX = Math.max(1, Math.floor(usableW / PANE_CASCADE_STEP) + 1)
  const stepsY = Math.max(1, Math.floor(usableH / PANE_CASCADE_STEP) + 1)
  const ringSize = Math.max(1, Math.min(stepsX, stepsY))
  const step = openCount % ringSize
  const rect: PaneRect = {
    x: PANE_ORIGIN_X + step * PANE_CASCADE_STEP,
    y: PANE_ORIGIN_Y + step * PANE_CASCADE_STEP,
    w,
    h,
  }
  return clampRect(rect, viewportWidth, viewportHeight)
}

// Apply a move/resize delta to a rect for the given handle. A move shifts x/y; a
// resize grows/shrinks the active edge(s), keeping the opposite edge anchored and
// enforcing the minimum size (so an aggressive drag past the min pins the edge
// rather than inverting the rect). Pure: the reducer clamps the result to the
// viewport afterwards.
export const applyDrag = (
  startRect: PaneRect,
  handle: PaneResizeHandle | null,
  dx: number,
  dy: number,
): PaneRect => {
  if (handle === null) {
    // Move.
    return { x: startRect.x + dx, y: startRect.y + dy, w: startRect.w, h: startRect.h }
  }
  let { x, y, w, h } = startRect
  const right = startRect.x + startRect.w
  const bottom = startRect.y + startRect.h
  const touchesLeft = handle === "left" || handle === "topleft" || handle === "bottomleft"
  const touchesRight = handle === "right" || handle === "topright" || handle === "bottomright"
  const touchesTop = handle === "top" || handle === "topleft" || handle === "topright"
  const touchesBottom = handle === "bottom" || handle === "bottomleft" || handle === "bottomright"

  if (touchesLeft) {
    // New left edge, but never past (right - min): pin x so width >= min.
    const newX = Math.min(startRect.x + dx, right - PANE_MIN_WIDTH)
    x = newX
    w = right - newX
  }
  if (touchesRight) {
    w = Math.max(PANE_MIN_WIDTH, startRect.w + dx)
  }
  if (touchesTop) {
    const newY = Math.min(startRect.y + dy, bottom - PANE_MIN_HEIGHT)
    y = newY
    h = bottom - newY
  }
  if (touchesBottom) {
    h = Math.max(PANE_MIN_HEIGHT, startRect.h + dy)
  }
  return { x, y, w, h }
}

// ── The typed PaneManager reducer ───────────────────────────────────────────
// One closed action union, one pure transition. update.ts maps each pane-layer
// Message to one of these and stores the result back in the Model. The viewport
// is passed in (the view measures it at dispatch time) so placement/clamp use
// the real window size; tests pass a fixed viewport for determinism.
export type PaneLayerAction =
  | Readonly<{ kind: "open"; pane: ManagedPaneKind }>
  | Readonly<{ kind: "close"; paneId: string }>
  | Readonly<{ kind: "focus"; paneId: string }>
  | Readonly<{
      kind: "drag-start"
      paneId: string
      drag: PaneDragKind
      handle: PaneResizeHandle | null
      pointerX: number
      pointerY: number
    }>
  | Readonly<{ kind: "drag-move"; pointerX: number; pointerY: number }>
  | Readonly<{ kind: "drag-end" }>
  | Readonly<{ kind: "close-all" }>

export type Viewport = Readonly<{ width: number; height: number }>

const findPane = (
  panes: ReadonlyArray<ManagedPane>,
  paneId: string,
): ManagedPane | null => panes.find((p) => p.id === paneId) ?? null

export const reducePaneLayer = (
  state: PaneLayer,
  action: PaneLayerAction,
  viewport: Viewport,
): PaneLayer => {
  switch (action.kind) {
    case "open": {
      // The shell is the base surface, never a managed window (anti-sprawl: the
      // black default stays the base, not a pane stacked on itself).
      if (action.pane === "shell") return state
      const seq = state.seq + 1
      const rect = placeNewPaneRect(state.panes.length, viewport.width, viewport.height)
      const pane: ManagedPane = {
        id: `${action.pane}#${seq}`,
        kind: action.pane,
        rect,
        z: seq,
      }
      return { ...state, panes: [...state.panes, pane], seq }
    }
    case "close": {
      const panes = state.panes.filter((p) => p.id !== action.paneId)
      if (panes.length === state.panes.length) return state
      // A close also ends any drag on the removed pane.
      const drag = state.drag?.paneId === action.paneId ? null : state.drag
      return { ...state, panes, drag }
    }
    case "close-all":
      return { ...state, panes: [], drag: null }
    case "focus": {
      const pane = findPane(state.panes, action.paneId)
      if (!pane) return state
      // Already the sole/top pane → bumping z is harmless but keep it cheap.
      const seq = state.seq + 1
      const panes = state.panes.map((p) =>
        p.id === action.paneId ? { ...p, z: seq } : p,
      )
      return { ...state, panes, seq }
    }
    case "drag-start": {
      const pane = findPane(state.panes, action.paneId)
      if (!pane) return state
      // Starting a drag focuses the pane (brings it to front) — same as Commander.
      const seq = state.seq + 1
      const panes = state.panes.map((p) =>
        p.id === action.paneId ? { ...p, z: seq } : p,
      )
      const drag: PaneDrag = {
        paneId: action.paneId,
        kind: action.drag,
        handle: action.handle,
        startPointerX: action.pointerX,
        startPointerY: action.pointerY,
        startRect: pane.rect,
      }
      return { ...state, panes, seq, drag }
    }
    case "drag-move": {
      const drag = state.drag
      if (!drag) return state
      const dx = action.pointerX - drag.startPointerX
      const dy = action.pointerY - drag.startPointerY
      const moved = applyDrag(drag.startRect, drag.handle, dx, dy)
      const clamped = clampRect(moved, viewport.width, viewport.height)
      const panes = state.panes.map((p) =>
        p.id === drag.paneId ? { ...p, rect: clamped } : p,
      )
      return { ...state, panes }
    }
    case "drag-end":
      return state.drag === null ? state : { ...state, drag: null }
  }
}

// ── Pure projection for programmatic-control parity ─────────────────────────
// A driver (Claude) reads the open managed panes as plain text — the SAME set
// the view renders — so headless control sees exactly what the owner sees. One
// line per pane, top (focused) first: "kind  x,y wxh".
export const paneLayerText = (layer: PaneLayer): string =>
  [...layer.panes]
    .sort((a, b) => b.z - a.z)
    .map((p) => `${p.kind}  ${p.rect.x},${p.rect.y} ${p.rect.w}x${p.rect.h}`)
    .join("\n")

// The 8 resize handles in render order (also the test source of truth).
export const PANE_RESIZE_HANDLES: ReadonlyArray<PaneResizeHandle> = [
  "topleft",
  "top",
  "topright",
  "right",
  "bottomright",
  "bottom",
  "bottomleft",
  "left",
]
