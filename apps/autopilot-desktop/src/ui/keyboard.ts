// Keyboard interpretation (#5465) — PURE, unit-testable, no DOM.
//
// The keyboard subscription (subscriptions.ts) emits a raw `PressedKey`; this
// module decides what it MEANS against the active pane + palette state and
// returns a `KeyIntent`. The reducer (update.ts) then re-dispatches the existing
// Message named by the intent, so the shortcut layer reuses real handlers and
// never invents a new control verb (audit §5.2).

import { groupForPane } from "./nav"
import type { Model, PaneId } from "./model"

// Raw key event the subscription forwards (mirrors the PressedKey payload).
export type KeyEvent = Readonly<{
  key: string
  meta: boolean
  ctrl: boolean
  shift: boolean
  inEditable: boolean
}>

// What a key press resolves to. The reducer maps each to a real Message.
export type KeyIntent =
  | Readonly<{ kind: "none" }>
  | Readonly<{ kind: "open-palette" }>
  | Readonly<{ kind: "close-palette" }>
  | Readonly<{ kind: "palette-move"; delta: number }>
  | Readonly<{ kind: "palette-run" }>
  | Readonly<{ kind: "submit-turn"; pane: PaneId }>
  // Move to the previous/next sub-pane within the current nav group (a real,
  // never-no-op navigation action available in any grouped pane). `pane` is the
  // resolved destination the reducer should navigate to.
  | Readonly<{ kind: "navigate-pane"; pane: PaneId }>
  // Escape from any non-shell pane returns to the zero-base shell — the
  // explicit "open panes" can always be undone, so you never get trapped in
  // the full UI (owner directive 2026-06-19).
  | Readonly<{ kind: "back-to-shell" }>

const isModified = (event: KeyEvent): boolean => event.meta || event.ctrl

// Resolve the destination `delta` steps from `pane` within its group, clamped
// to the group bounds (no wrap, so the ends are stable). Returns null when the
// move would leave the group (or the group has a single destination).
const adjacentPaneInGroup = (pane: PaneId, delta: number): PaneId | null => {
  const group = groupForPane(pane)
  if (!group) return null
  const dests = group.destinations
  const index = dests.findIndex((d) => d.pane === pane)
  if (index === -1) return null
  const next = index + delta
  if (next < 0 || next >= dests.length) return null
  const target = dests[next]?.pane ?? null
  return target === pane ? null : target
}

export const interpretKey = (model: Model, event: KeyEvent): KeyIntent => {
  const key = event.key

  // ── Palette is open: keys drive the palette, nothing else ─────────────────
  if (model.commandPaletteOpen) {
    if (key === "Escape") return { kind: "close-palette" }
    if (key === "ArrowDown") return { kind: "palette-move", delta: 1 }
    if (key === "ArrowUp") return { kind: "palette-move", delta: -1 }
    // Cmd/Ctrl-K toggles the palette closed too.
    if (key.toLowerCase() === "k" && isModified(event)) return { kind: "close-palette" }
    if (key === "Enter") return { kind: "palette-run" }
    return { kind: "none" }
  }

  // ── Escape returns to the zero-base shell from anywhere in the full UI ────
  // The shell is the home surface; once you've opened the panes (Cmd-K or the
  // affordance), Escape always takes you back so you can never get stranded.
  // Works even while typing (inEditable) — it's a deliberate global escape.
  if (key === "Escape" && model.pane !== "shell") return { kind: "back-to-shell" }

  // ── Cmd/Ctrl-K opens the palette from anywhere (even while typing) ────────
  if (key.toLowerCase() === "k" && isModified(event)) return { kind: "open-palette" }

  // ── Cmd/Ctrl-Enter submits the chat/composer turn ─────────────────────────
  if (key === "Enter" && isModified(event)) {
    if (model.pane === "chat" || model.pane === "composer") {
      return { kind: "submit-turn", pane: model.pane }
    }
    return { kind: "none" }
  }

  // ── Bare nav keys are IGNORED while typing in an input/textarea (#5465) ────
  if (event.inEditable) return { kind: "none" }
  if (isModified(event)) return { kind: "none" }

  // ── j / k move between sub-panes of the current group ─────────────────────
  if (key === "j") {
    const target = adjacentPaneInGroup(model.pane, 1)
    return target ? { kind: "navigate-pane", pane: target } : { kind: "none" }
  }
  if (key === "k") {
    const target = adjacentPaneInGroup(model.pane, -1)
    return target ? { kind: "navigate-pane", pane: target } : { kind: "none" }
  }

  return { kind: "none" }
}
