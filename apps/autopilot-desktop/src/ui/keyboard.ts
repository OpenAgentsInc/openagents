// Keyboard interpretation (#5465) — PURE, unit-testable, no DOM.
//
// The keyboard subscription (subscriptions.ts) emits a raw `PressedKey`; this
// module decides what it MEANS against the active pane + palette state and
// returns a `KeyIntent`. The reducer (update.ts) then re-dispatches the existing
// Message named by the intent, so the shortcut layer reuses real handlers and
// never invents a new control verb (audit §5.2).

import {
  openAgentsInputActionMapFromProfile,
  openAgentsInputActionSpecById,
  parseOpenAgentsInputProfileOrDefault,
  resolveOpenAgentsKeyboardEventActionBindings,
} from "@openagentsinc/input-bindings"

import { groupForPane } from "./nav.js"
import { modelPaneLayer, type Model, type PaneId } from "./model.js"

// Raw key event the subscription forwards (mirrors the PressedKey payload).
export type KeyEvent = Readonly<{
  key: string
  code?: string
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
  // Explicit controls handle shell navigation. Escape is reserved for local UI
  // dismissal (palette/input/canvas capture) and must never drop the Verse back
  // to the zero-base shell.
  | Readonly<{ kind: "back-to-shell" }>
  // #5730 The Verse: ⌘⇧V / Ctrl-⇧V toggles the game-world view on/off.
  | Readonly<{ kind: "toggle-verse" }>
  // HUD H1: action-bar slot 1 opens a fresh coder-session surface.
  | Readonly<{ kind: "open-coder-session" }>
  | Readonly<{ kind: "close-managed-panes" }>
  | Readonly<{ kind: "hide-code-dock" }>

const isModified = (event: KeyEvent): boolean => event.meta || event.ctrl

const isVerseExploreActionContext = (model: Model): boolean =>
  model.pane === "chat" &&
  model.verseEnabled &&
  model.verseMode === "explore"

const actionIdsForKey = (
  model: Model,
  event: KeyEvent,
): ReadonlyArray<string> => {
  const profile = parseOpenAgentsInputProfileOrDefault(model.inputProfile)
  const actionMap = openAgentsInputActionMapFromProfile(profile)
  return resolveOpenAgentsKeyboardEventActionBindings(
    actionMap,
    {
      key: event.key,
      ...(event.code === undefined ? {} : { code: event.code }),
      metaKey: event.meta,
      ctrlKey: event.ctrl,
      shiftKey: event.shift,
    },
    { allowExtraModifiers: false },
  )
    .map((match) => match.actionId)
    .filter((actionId) => {
      const spec = openAgentsInputActionSpecById.get(actionId)
      return (
        spec?.contexts.includes("verse_explore") === true &&
        isVerseExploreActionContext(model)
      )
    })
}

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

  // ── Escape outside the palette dismisses local overlays only ──────────────
  // In the Verse app the canvas owns navigation. Pressing Escape must not
  // reveal the old shell or coding target selector; it closes active managed
  // panes first, then the code dock, then becomes a stable no-op.
  if (key === "Escape") {
    if (modelPaneLayer(model).panes.length > 0) return { kind: "close-managed-panes" }
    if (model.pane === "chat" && model.verseMode === "code") return { kind: "hide-code-dock" }
    return { kind: "none" }
  }

  // Focused editor/composer/terminal fields own their keys. They must never
  // trigger global commands, submit turns, or j/k pane movement while text entry
  // is active; the palette-open branch above is the only exception.
  if (event.inEditable) return { kind: "none" }

  // ── Cmd/Ctrl-K opens the palette when text entry is not focused ───────────
  if (key.toLowerCase() === "k" && isModified(event)) return { kind: "open-palette" }

  // ── Cmd/Ctrl-Shift-V toggles the Verse (game-world view) from anywhere ────
  if (key.toLowerCase() === "v" && isModified(event) && event.shift) {
    return { kind: "toggle-verse" }
  }

  // ── Cmd/Ctrl-Enter submits the chat/composer turn ─────────────────────────
  if (key === "Enter" && isModified(event)) {
    if (model.pane === "chat" || model.pane === "composer") {
      return { kind: "submit-turn", pane: model.pane }
    }
    return { kind: "none" }
  }

  if (isModified(event)) return { kind: "none" }

  if (actionIdsForKey(model, event).includes("action_bar.slot_1")) {
    return { kind: "open-coder-session" }
  }

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
