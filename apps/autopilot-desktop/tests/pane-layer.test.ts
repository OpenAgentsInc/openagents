// HUD H3 (#5501): the managed pane layer over the base surfaces.
//
// Panes come back as a MANAGED layer (pane-as-data + a typed PaneManager reducer)
// opened from the hotbar / command palette — NOT the old free-floating default
// sprawl. These tests pin: (1) the pure PaneManager reducer (open/close/focus/
// move/resize/clamp); (2) anti-sprawl placement (a new pane never stacks exactly
// on the last one, and panes clamp into the viewport so none can be lost
// off-screen); (3) the Foldkit wiring (the H3 messages flow through update.ts
// and store back on the Model); (4) the registry seam (every nav destination is
// openable as a pane via the palette); (5) the render (windows + 8 resize
// handles render only when panes are open); and (6) the invariant that the Verse
// and fallback shell stay window-free until an explicit open.

import { describe, expect, test } from "bun:test"

import { initialModel, Model, modelPaneLayer, PaneId } from "../src/ui/model"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import { initialRuntimeState } from "../src/ui/initial-state"
import {
  applyDrag,
  clampRect,
  emptyPaneLayer,
  PANE_CASCADE_STEP,
  PANE_DEFAULT_HEIGHT,
  PANE_DEFAULT_WIDTH,
  PANE_MIN_HEIGHT,
  PANE_MIN_WIDTH,
  PANE_RESIZE_HANDLES,
  PANE_VIEWPORT_MARGIN,
  paneLayerText,
  placeNewPaneRect,
  reducePaneLayer,
  type PaneLayer,
  type PaneRect,
  type Viewport,
} from "../src/ui/pane-manager"
import {
  ClosedManagedPane,
  EndedPaneDrag,
  FocusedManagedPane,
  MovedPaneDragPointer,
  OpenedManagedPane,
  StartedPaneDrag,
} from "../src/ui/message"
import { paletteCommands } from "../src/ui/nav"

const VIEWPORT: Viewport = { width: 1440, height: 900 }

const fallbackShellModel = () => Model.make({ ...initialModel, pane: "shell" })

// Cycle-safe serialize (the foldkit Html tree is plain objects) to assert what
// the view does / does not render without a DOM.
const serialize = (node: unknown): string => {
  const seen = new WeakSet<object>()
  return JSON.stringify(node, (_k, v) => {
    if (typeof v === "function") return "[fn]"
    if (typeof v === "object" && v !== null) {
      if (seen.has(v)) return "[cycle]"
      seen.add(v)
    }
    return v
  })
}

describe("HUD H3 PaneManager reducer: open / close / focus", () => {
  test("open adds a pane-as-data record with a rect + z, never the shell", () => {
    const s1 = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    expect(s1.panes).toHaveLength(1)
    expect(s1.panes[0]).toMatchObject({ kind: "chat", z: 1 })
    expect(s1.panes[0].id).toBe("chat#1")
    expect(s1.panes[0].rect.w).toBe(PANE_DEFAULT_WIDTH)
    expect(s1.panes[0].rect.h).toBe(PANE_DEFAULT_HEIGHT)
    // The shell is the BASE surface, never a managed window (anti-sprawl).
    const s2 = reducePaneLayer(s1, { kind: "open", pane: "shell" }, VIEWPORT)
    expect(s2).toBe(s1) // no-op (same reference)
  })

  test("close removes only the targeted pane (by id)", () => {
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    s = reducePaneLayer(s, { kind: "open", pane: "composer" }, VIEWPORT)
    const composerId = s.panes[1].id
    const closed = reducePaneLayer(s, { kind: "close", paneId: composerId }, VIEWPORT)
    expect(closed.panes).toHaveLength(1)
    expect(closed.panes[0].kind).toBe("chat")
    // Closing an unknown id is a no-op.
    expect(reducePaneLayer(closed, { kind: "close", paneId: "nope#9" }, VIEWPORT)).toBe(closed)
  })

  test("close-all clears every pane (and any in-flight drag)", () => {
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    s = reducePaneLayer(s, { kind: "open", pane: "swarm" }, VIEWPORT)
    const cleared = reducePaneLayer(s, { kind: "close-all" }, VIEWPORT)
    expect(cleared.panes).toHaveLength(0)
    expect(cleared.drag).toBeNull()
  })

  test("focus brings the targeted pane to the front (highest z)", () => {
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    s = reducePaneLayer(s, { kind: "open", pane: "composer" }, VIEWPORT)
    const chatId = s.panes[0].id
    // composer (opened 2nd) is on top; focusing chat must lift it above composer.
    expect(s.panes[0].z).toBeLessThan(s.panes[1].z)
    const focused = reducePaneLayer(s, { kind: "focus", paneId: chatId }, VIEWPORT)
    const chat = focused.panes.find((p) => p.id === chatId)!
    const composer = focused.panes.find((p) => p.kind === "composer")!
    expect(chat.z).toBeGreaterThan(composer.z)
  })
})

describe("HUD H3 PaneManager: drag (move) + resize (8-handle)", () => {
  test("a move shifts the rect by the pointer delta and brings the pane to front", () => {
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    s = reducePaneLayer(s, { kind: "open", pane: "composer" }, VIEWPORT)
    const chatId = s.panes[0].id
    const start = s.panes[0].rect
    s = reducePaneLayer(
      s,
      { kind: "drag-start", paneId: chatId, drag: "move", handle: null, pointerX: 200, pointerY: 200 },
      VIEWPORT,
    )
    // drag-start focuses the dragged pane.
    expect(s.panes.find((p) => p.id === chatId)!.z).toBe(
      Math.max(...s.panes.map((p) => p.z)),
    )
    s = reducePaneLayer(s, { kind: "drag-move", pointerX: 260, pointerY: 240 }, VIEWPORT)
    const moved = s.panes.find((p) => p.id === chatId)!.rect
    expect(moved.x).toBe(start.x + 60)
    expect(moved.y).toBe(start.y + 40)
    expect(moved.w).toBe(start.w)
    expect(moved.h).toBe(start.h)
    // drag-end clears the gesture; a later move no-ops.
    s = reducePaneLayer(s, { kind: "drag-end" }, VIEWPORT)
    expect(s.drag).toBeNull()
    const afterEnd = reducePaneLayer(s, { kind: "drag-move", pointerX: 999, pointerY: 999 }, VIEWPORT)
    expect(afterEnd.panes.find((p) => p.id === chatId)!.rect).toEqual(moved)
  })

  test("all 8 resize handles exist and resize from the right edge", () => {
    expect(PANE_RESIZE_HANDLES).toHaveLength(8)
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    const id = s.panes[0].id
    const start = s.panes[0].rect
    s = reducePaneLayer(
      s,
      { kind: "drag-start", paneId: id, drag: "resize", handle: "right", pointerX: 0, pointerY: 0 },
      VIEWPORT,
    )
    s = reducePaneLayer(s, { kind: "drag-move", pointerX: 50, pointerY: 0 }, VIEWPORT)
    const r = s.panes[0].rect
    expect(r.w).toBe(start.w + 50)
    expect(r.x).toBe(start.x) // right-edge resize keeps the left edge anchored
  })

  test("applyDrag: a top-left resize moves the top-left corner, keeps bottom-right anchored", () => {
    const start: PaneRect = { x: 100, y: 100, w: 400, h: 300 }
    const out = applyDrag(start, "topleft", -40, -20)
    expect(out.x).toBe(60)
    expect(out.y).toBe(80)
    expect(out.w).toBe(440) // grew left
    expect(out.h).toBe(320) // grew up
    // The bottom-right corner is unchanged.
    expect(out.x + out.w).toBe(start.x + start.w)
    expect(out.y + out.h).toBe(start.y + start.h)
  })

  test("resize cannot shrink below the minimum (the edge pins, the rect never inverts)", () => {
    const start: PaneRect = { x: 100, y: 100, w: PANE_MIN_WIDTH + 10, h: PANE_MIN_HEIGHT + 10 }
    // Drag the left edge way past the right edge.
    const out = applyDrag(start, "left", 9999, 0)
    expect(out.w).toBe(PANE_MIN_WIDTH)
    expect(out.x).toBe(start.x + start.w - PANE_MIN_WIDTH)
    expect(out.x + out.w).toBe(start.x + start.w) // right edge still anchored
  })
})

describe("HUD H3 anti-sprawl: cascade placement + viewport clamp", () => {
  test("each newly opened pane CASCADES — it never stacks exactly on the previous one", () => {
    const r0 = placeNewPaneRect(0, VIEWPORT.width, VIEWPORT.height)
    const r1 = placeNewPaneRect(1, VIEWPORT.width, VIEWPORT.height)
    const r2 = placeNewPaneRect(2, VIEWPORT.width, VIEWPORT.height)
    expect(r1.x).toBe(r0.x + PANE_CASCADE_STEP)
    expect(r1.y).toBe(r0.y + PANE_CASCADE_STEP)
    expect(r2.x).toBe(r0.x + PANE_CASCADE_STEP * 2)
    // The whole point: no two consecutive opens share a top-left (no sprawl-on-open).
    expect({ x: r0.x, y: r0.y }).not.toEqual({ x: r1.x, y: r1.y })
  })

  test("opening many panes in a row keeps every one fully on-screen (clamped, never lost)", () => {
    let s: PaneLayer = emptyPaneLayer
    for (let i = 0; i < 12; i++) {
      s = reducePaneLayer(s, { kind: "open", pane: "chat" }, VIEWPORT)
    }
    expect(s.panes).toHaveLength(12)
    for (const p of s.panes) {
      expect(p.rect.x).toBeGreaterThanOrEqual(PANE_VIEWPORT_MARGIN)
      expect(p.rect.y).toBeGreaterThanOrEqual(PANE_VIEWPORT_MARGIN)
      expect(p.rect.x + p.rect.w).toBeLessThanOrEqual(VIEWPORT.width - PANE_VIEWPORT_MARGIN)
      expect(p.rect.y + p.rect.h).toBeLessThanOrEqual(VIEWPORT.height - PANE_VIEWPORT_MARGIN)
    }
  })

  test("clampRect pulls an off-screen pane back fully into the viewport", () => {
    const off: PaneRect = { x: 5000, y: 5000, w: 400, h: 300 }
    const c = clampRect(off, VIEWPORT.width, VIEWPORT.height)
    expect(c.x + c.w).toBeLessThanOrEqual(VIEWPORT.width - PANE_VIEWPORT_MARGIN)
    expect(c.y + c.h).toBeLessThanOrEqual(VIEWPORT.height - PANE_VIEWPORT_MARGIN)
    // A negative origin is pulled to the top-left margin.
    const neg = clampRect({ x: -500, y: -500, w: 400, h: 300 }, VIEWPORT.width, VIEWPORT.height)
    expect(neg.x).toBe(PANE_VIEWPORT_MARGIN)
    expect(neg.y).toBe(PANE_VIEWPORT_MARGIN)
  })

  test("a drag-move that would push a pane off-screen is clamped, not lost", () => {
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    const id = s.panes[0].id
    s = reducePaneLayer(
      s,
      { kind: "drag-start", paneId: id, drag: "move", handle: null, pointerX: 0, pointerY: 0 },
      VIEWPORT,
    )
    s = reducePaneLayer(s, { kind: "drag-move", pointerX: 99999, pointerY: 99999 }, VIEWPORT)
    const r = s.panes[0].rect
    expect(r.x + r.w).toBeLessThanOrEqual(VIEWPORT.width - PANE_VIEWPORT_MARGIN)
    expect(r.y + r.h).toBeLessThanOrEqual(VIEWPORT.height - PANE_VIEWPORT_MARGIN)
  })
})

describe("HUD H3 Foldkit wiring: messages flow through update.ts", () => {
  test("OpenedManagedPane stores a pane on the Model's opaque paneLayer field", () => {
    const [m0] = update(initialModel, OpenedManagedPane({ pane: "chat" }))
    const layer = modelPaneLayer(m0)
    expect(layer.panes).toHaveLength(1)
    expect(layer.panes[0].kind).toBe("chat")
    // It does NOT touch the single-pane router — the base surface is unchanged.
    expect(m0.pane).toBe(initialModel.pane)
  })

  test("the full open → focus → move → close lifecycle round-trips through update", () => {
    let m = initialModel
    ;[m] = update(m, OpenedManagedPane({ pane: "chat" }))
    ;[m] = update(m, OpenedManagedPane({ pane: "composer" }))
    const chatId = modelPaneLayer(m).panes[0].id
    ;[m] = update(m, FocusedManagedPane({ paneId: chatId }))
    ;[m] = update(
      m,
      StartedPaneDrag({ paneId: chatId, drag: "move", handle: null, pointerX: 100, pointerY: 100 }),
    )
    ;[m] = update(m, MovedPaneDragPointer({ pointerX: 130, pointerY: 120 }))
    ;[m] = update(m, EndedPaneDrag())
    const chat = modelPaneLayer(m).panes.find((p) => p.id === chatId)!
    // The move stuck (delta 30,20 from the cascade origin).
    expect(chat.rect.x).toBeGreaterThan(0)
    ;[m] = update(m, ClosedManagedPane({ paneId: chatId }))
    expect(modelPaneLayer(m).panes.map((p) => p.kind)).toEqual(["composer"])
  })

  test("MovedPaneDragPointer / EndedPaneDrag are no-ops when no drag is in flight", () => {
    const [m0] = update(initialModel, OpenedManagedPane({ pane: "chat" }))
    const [m1] = update(m0, MovedPaneDragPointer({ pointerX: 999, pointerY: 999 }))
    expect(modelPaneLayer(m1).panes[0].rect).toEqual(modelPaneLayer(m0).panes[0].rect)
    const [m2] = update(m1, EndedPaneDrag())
    expect(modelPaneLayer(m2)).toEqual(modelPaneLayer(m1))
  })
})

describe("HUD H3 registry seam: every nav destination is openable as a pane", () => {
  test("the palette carries an 'Open <X> as a pane' command for each destination", () => {
    const panes = paletteCommands.filter((c) => c.id.startsWith("pane."))
    expect(panes.length).toBeGreaterThan(0)
    for (const c of panes) {
      expect(c.kind).toBe("action")
      if (c.kind === "action") {
        expect(c.messageTag).toBe("OpenedManagedPane")
        // The arg is a real PaneId (never the shell base surface).
        expect(PaneId.literals).toContain(c.args?.pane)
        expect(c.args?.pane).not.toBe("shell")
      }
    }
  })
})

describe("HUD H3 render: windows + handles render only when panes are open", () => {
  test("with no panes open the Verse first paint has no pane layer in the tree", () => {
    const [model] = initialRuntimeState()
    expect(model.pane).toBe("chat")
    expect(modelPaneLayer(model).panes).toHaveLength(0)
    const tree = serialize(view(model).body)
    expect(tree).toContain("app-shell-verse")
    // The pane layer renders nothing until an explicit open.
    expect(tree).not.toContain("pane-layer")
    expect(tree).not.toContain("pane-window")
  })

  test("opening a pane on the fallback shell renders a managed window with a title bar + 8 resize handles", () => {
    const m0 = fallbackShellModel()
    const [m1] = update(m0, OpenedManagedPane({ pane: "chat" }))
    // Still on the black shell base — the pane FLOATS over it (not the old
    // free-floating default; the shell pane itself is unchanged underneath).
    expect(m1.pane).toBe("shell")
    const tree = serialize(view(m1).body)
    expect(tree).toContain("pane-layer")
    expect(tree).toContain("pane-window")
    expect(tree).toContain("pane-window-titlebar")
    expect(tree).toContain("pane-window-close")
    // All 8 resize handles render.
    for (const handle of PANE_RESIZE_HANDLES) {
      expect(tree).toContain(`pane-window-resize-${handle}`)
    }
    // The shell text bar is still the base surface beneath the pane.
    expect(tree).toContain("shell-bar")
  })

  test("the pane layer is hidden over the immersive Verse home until code mode", () => {
    const opened = Model.make({
      ...initialModel,
      pane: "chat",
      paneLayer: reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "swarm" }, VIEWPORT),
    })
    const tree = serialize(view(opened).body)
    expect(tree).toContain("app-shell-verse") // Verse base
    expect(tree).not.toContain("sidebar")
    expect(tree).not.toContain("pane-window")
  })

  test("the pane layer floats over the immersive Verse home in code mode", () => {
    const opened = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      paneLayer: reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "swarm" }, VIEWPORT),
    })
    const tree = serialize(view(opened).body)
    expect(tree).toContain("app-shell-verse") // Verse base
    expect(tree).toContain("data-verse-mode")
    expect(tree).toContain("code")
    expect(tree).not.toContain("sidebar")
    expect(tree).toContain("pane-window") // + the floating managed pane
  })

  test("the pane layer also floats over the advanced full UI", () => {
    const opened = Model.make({
      ...initialModel,
      pane: "composer",
      paneLayer: reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "swarm" }, VIEWPORT),
    })
    const tree = serialize(view(opened).body)
    expect(tree).toContain("sidebar") // advanced full UI base
    expect(tree).toContain("pane-window") // + the floating managed pane
  })
})

describe("HUD H3 programmatic-control parity", () => {
  test("paneLayerText projects the open panes the same way the view renders them", () => {
    let s = reducePaneLayer(emptyPaneLayer, { kind: "open", pane: "chat" }, VIEWPORT)
    s = reducePaneLayer(s, { kind: "open", pane: "composer" }, VIEWPORT)
    const text = paneLayerText(s)
    // Top (focused) pane first; one line per pane with kind + geometry.
    expect(text.split("\n")).toHaveLength(2)
    expect(text.split("\n")[0]).toContain("composer") // opened last → on top
    expect(text).toContain("chat")
  })
})
