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
import { DEFAULT_SPAWNABLE_SCENE_ID } from "../shared/verse-spawned-scene.js"

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
  // Dev affordance (#6033): ⌘⇧E / Ctrl-⇧E spawns/unspawns the isolated
  // crackling-energy scene station into the live Verse world; ⌘⇧P / Ctrl-⇧P
  // toggles that scene's optional gateway-portal variant. Explore mode only.
  | Readonly<{ kind: "spawn-verse-scene"; sceneId: string }>
  | Readonly<{ kind: "toggle-verse-scene-portal"; sceneId: string }>
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

  // ── Hotbar action slots fire even with a field focused (verse explore) ─────
  // The Verse hotbar number keys are the MMO action bar. The owner hit a bug
  // where, with the in-world Ask box focused, `2`/`3` typed a digit and the
  // hotbar "did nothing". The hotbar is a dedicated game surface in the verse, so
  // its slot keys must fire regardless of focus (the matching forward-gate change
  // already swallows the digit so it never pollutes the box). `actionIdsForKey`
  // is already scoped to verse_explore + isVerseExploreActionContext(model), so
  // this only affects the in-world Ask box, never the command palette, composer,
  // or any other field. Resolved BEFORE the generic in-editable no-op below.
  {
    const focusedSlotActionIds = actionIdsForKey(model, event)
    if (focusedSlotActionIds.includes("action_bar.slot_1")) {
      return { kind: "open-coder-session" }
    }
    if (focusedSlotActionIds.includes("action_bar.slot_2")) {
      return { kind: "spawn-verse-scene", sceneId: DEFAULT_SPAWNABLE_SCENE_ID }
    }
    if (focusedSlotActionIds.includes("action_bar.slot_3")) {
      return {
        kind: "toggle-verse-scene-portal",
        sceneId: DEFAULT_SPAWNABLE_SCENE_ID,
      }
    }
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

  // ── Dev affordance (#6033 / #6041): spawn an isolated scene station into the ─
  // live Verse, and toggle its portal. Explore mode only (so coding overlay keys
  // are untouched), and only while the Verse is on-screen. Driven by the
  // registered `verse.spawn_scene` / `verse.toggle_scene_portal` input bindings
  // (verse_explore context) rather than a hand-written raw-key check, so this
  // path stays aligned with the same action catalog the subscription forward
  // gate consults — the mismatch the #6041 bug came from. `actionIdsForKey`
  // already filters to verse_explore + isVerseExploreActionContext(model).
  {
    const actionIds = actionIdsForKey(model, event)
    if (actionIds.includes("verse.spawn_scene")) {
      return { kind: "spawn-verse-scene", sceneId: DEFAULT_SPAWNABLE_SCENE_ID }
    }
    if (actionIds.includes("verse.toggle_scene_portal")) {
      return {
        kind: "toggle-verse-scene-portal",
        sceneId: DEFAULT_SPAWNABLE_SCENE_ID,
      }
    }
  }

  // ── Cmd/Ctrl-Enter submits the chat/composer turn ─────────────────────────
  if (key === "Enter" && isModified(event)) {
    if (model.pane === "chat" || model.pane === "composer") {
      return { kind: "submit-turn", pane: model.pane }
    }
    return { kind: "none" }
  }

  if (isModified(event)) return { kind: "none" }

  // ── Hotbar action slots (verse_explore) ──────────────────────────────────
  // Slot 1 (key 1) opens a fresh coder session; slot 2 (key 2) spawns the
  // isolated crackling-energy Verse scene; slot 3 (key 3) toggles that scene's
  // gateway portal. These now resolve in the focus-independent hotbar block at
  // the TOP of this function (so they also fire with the in-world Ask box
  // focused); nothing extra is needed here for the unfocused case.

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
