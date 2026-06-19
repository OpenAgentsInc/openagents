import { describe, expect, test } from "bun:test"

import { interpretKey, type KeyEvent } from "../src/ui/keyboard"
import {
  ChangedCommandPaletteQuery,
  ClosedCommandPalette,
  MovedCommandPaletteSelection,
  NavigatedTo,
  NavigatedToGroup,
  OpenedCommandPalette,
  PressedKey,
  RanPaletteCommand,
} from "../src/ui/message"
import { initialModel, Model, PaneId, type Model as ModelType } from "../src/ui/model"
import {
  NAV_DESTINATIONS,
  NAV_GROUPS,
  NAV_LEAF_PANES,
  SHORTCUTS,
  filterPaletteCommands,
  groupByAccel,
  groupForPane,
  paletteCommands,
} from "../src/ui/nav"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"

const key = (over: Partial<KeyEvent>): KeyEvent => ({
  key: "",
  meta: false,
  ctrl: false,
  shift: false,
  inEditable: false,
  ...over,
})

// ── #5463: grouped nav, ~5 destinations, all panes reachable ─────────────────
describe("#5463 grouped nav", () => {
  test("the primary sidebar is ~5 grouped destinations, not 13 flat buttons", () => {
    expect(NAV_GROUPS.length).toBeGreaterThanOrEqual(4)
    expect(NAV_GROUPS.length).toBeLessThanOrEqual(6)
    expect(NAV_GROUPS.map((g) => g.label)).toEqual([
      "Chat",
      "Code",
      "Supervise",
      "Explore",
      "Settings",
    ])
  })

  test("Chat is the default post-onboarding home (group 1, defaultPane chat)", () => {
    const chat = NAV_GROUPS[0]
    expect(chat?.id).toBe("chat")
    expect(chat?.accel).toBe(1)
    expect(chat?.defaultPane).toBe("chat")
  })

  test("EVERY PaneId is reachable: a group destination or an intentional leaf", () => {
    const reachable = new Set<string>([
      ...NAV_DESTINATIONS.map((d) => d.pane),
      ...NAV_LEAF_PANES,
    ])
    for (const pane of PaneId.literals) {
      expect({ pane, reachable: reachable.has(pane) }).toEqual({ pane, reachable: true })
    }
  })

  test("no pane is registered in two groups (clean ownership)", () => {
    const seen = new Set<string>()
    for (const dest of NAV_DESTINATIONS) {
      expect(seen.has(dest.pane)).toBe(false)
      seen.add(dest.pane)
    }
  })

  test("groupForPane maps every reachable pane (session-detail → Code leaf)", () => {
    expect(groupForPane("chat")?.id).toBe("chat")
    expect(groupForPane("composer")?.id).toBe("code")
    expect(groupForPane("decisions")?.id).toBe("supervise")
    expect(groupForPane("session-detail")?.id).toBe("code")
  })

  test("accel 1..5 each resolve to a distinct group", () => {
    const ids = [1, 2, 3, 4, 5].map((n) => groupByAccel(n)?.id)
    expect(new Set(ids).size).toBe(5)
    expect(ids).not.toContain(undefined)
  })

  test("NavigatedToGroup lands on the group's default pane", () => {
    const [model] = update(initialModel, NavigatedToGroup({ group: "code" }))
    expect(model.pane).toBe("composer")
    const [supervise] = update(initialModel, NavigatedToGroup({ group: "supervise" }))
    expect(supervise.pane).toBe("decisions")
  })

  test("NavigatedToGroup with an unknown group is a no-op", () => {
    const [model, commands] = update(initialModel, NavigatedToGroup({ group: "nope" }))
    expect(model.pane).toBe(initialModel.pane)
    expect(commands).toHaveLength(0)
  })
})

// ── #5464: command palette over the typed registry ──────────────────────────
describe("#5464 command palette + registry", () => {
  test("every navigable destination is registered as a 'Go to' command", () => {
    for (const dest of NAV_DESTINATIONS) {
      const cmd = paletteCommands.find(
        (c) => c.kind === "navigate" && c.pane === dest.pane,
      )
      expect(cmd, `missing navigate command for ${dest.pane}`).toBeDefined()
    }
  })

  test("key actions are registered (spawn, intent, approvals, coordinator, replay)", () => {
    const ids = new Set(paletteCommands.map((c) => c.id))
    for (const id of [
      "action.spawn",
      "action.submit-intent",
      "action.resolve-next-approval",
      "action.coordinator-pause",
      "action.coordinator-resume",
      "action.open-replay",
    ]) {
      expect(ids.has(id), `missing ${id}`).toBe(true)
    }
  })

  test("empty query returns all commands; a query fuzzy-filters", () => {
    expect(filterPaletteCommands("")).toHaveLength(paletteCommands.length)
    const matches = filterPaletteCommands("swarm")
    expect(matches.length).toBeGreaterThan(0)
    expect(matches[0]?.command.label.toLowerCase()).toContain("swarm")
  })

  test("a non-matching query returns nothing", () => {
    expect(filterPaletteCommands("zzqqxx")).toHaveLength(0)
  })

  test("Open/Close palette toggles model state", () => {
    const [open] = update(initialModel, OpenedCommandPalette())
    expect(open.commandPaletteOpen).toBe(true)
    expect(open.commandPaletteQuery).toBe("")
    const [closed] = update(open, ClosedCommandPalette())
    expect(closed.commandPaletteOpen).toBe(false)
  })

  test("query change resets the highlight to the top", () => {
    const [open] = update(initialModel, OpenedCommandPalette())
    const [moved] = update(open, MovedCommandPaletteSelection({ delta: 3 }))
    expect(moved.commandPaletteIndex).toBeGreaterThan(0)
    const [queried] = update(moved, ChangedCommandPaletteQuery({ value: "go" }))
    expect(queried.commandPaletteIndex).toBe(0)
  })

  test("selection move is clamped to the filtered list bounds", () => {
    const [open] = update(initialModel, OpenedCommandPalette())
    const [up] = update(open, MovedCommandPaletteSelection({ delta: -5 }))
    expect(up.commandPaletteIndex).toBe(0)
    const [down] = update(open, MovedCommandPaletteSelection({ delta: 9999 }))
    expect(down.commandPaletteIndex).toBe(paletteCommands.length - 1)
  })

  test("running a navigate command navigates AND closes the palette", () => {
    const [open] = update(initialModel, OpenedCommandPalette())
    const [model] = update(open, RanPaletteCommand({ commandId: "go.swarm" }))
    expect(model.pane).toBe("swarm")
    expect(model.commandPaletteOpen).toBe(false)
  })

  test("running an action command maps to the real existing message", () => {
    // coordinator-pause → ClickedCoordinatorToggle({paused:true}) → emits a
    // SetCoordinatorPaused command (the real handler), not a no-op.
    const [open] = update(initialModel, OpenedCommandPalette())
    const [model, commands] = update(open, RanPaletteCommand({ commandId: "action.coordinator-pause" }))
    expect(model.commandPaletteOpen).toBe(false)
    expect(commands.length).toBeGreaterThan(0)
  })

  test("running the highlighted command (commandId null) uses the index", () => {
    const [open] = update(initialModel, OpenedCommandPalette())
    const [queried] = update(open, ChangedCommandPaletteQuery({ value: "swarm" }))
    const [model] = update(queried, RanPaletteCommand({ commandId: null }))
    expect(model.pane).toBe("swarm")
  })
})

// ── #5465: keyboard layer (pure interpretation + reducer wiring) ─────────────
describe("#5465 keyboard layer", () => {
  test("Cmd-K and Ctrl-K open the palette from anywhere (even while typing)", () => {
    expect(interpretKey(initialModel, key({ key: "k", meta: true })).kind).toBe("open-palette")
    expect(interpretKey(initialModel, key({ key: "k", ctrl: true })).kind).toBe("open-palette")
    expect(
      interpretKey(initialModel, key({ key: "k", meta: true, inEditable: true })).kind,
    ).toBe("open-palette")
  })

  test("while the palette is open, arrows move, Enter runs, Esc closes", () => {
    const open = Model.make({ ...initialModel, commandPaletteOpen: true })
    expect(interpretKey(open, key({ key: "ArrowDown" })).kind).toBe("palette-move")
    expect(interpretKey(open, key({ key: "ArrowUp" })).kind).toBe("palette-move")
    expect(interpretKey(open, key({ key: "Enter" })).kind).toBe("palette-run")
    expect(interpretKey(open, key({ key: "Escape" })).kind).toBe("close-palette")
  })

  test("Cmd-1..5 jump to the primary groups", () => {
    for (const n of [1, 2, 3, 4, 5]) {
      const intent = interpretKey(initialModel, key({ key: String(n), meta: true }))
      expect(intent.kind).toBe("navigate-group")
    }
  })

  test("Cmd-Enter submits in chat/composer, and is a no-op elsewhere", () => {
    const chat = Model.make({ ...initialModel, pane: "chat" })
    expect(interpretKey(chat, key({ key: "Enter", meta: true })).kind).toBe("submit-turn")
    const settings = Model.make({ ...initialModel, pane: "settings" })
    expect(interpretKey(settings, key({ key: "Enter", meta: true })).kind).toBe("none")
  })

  test("bare nav keys (j/k) are ignored while typing in an input", () => {
    const code = Model.make({ ...initialModel, pane: "composer" })
    expect(interpretKey(code, key({ key: "j", inEditable: true })).kind).toBe("none")
    expect(interpretKey(code, key({ key: "j", inEditable: false })).kind).toBe("navigate-pane")
  })

  test("j/k move between sub-panes within the current group (no wrap at ends)", () => {
    const composer = Model.make({ ...initialModel, pane: "composer" })
    // Code group order: composer, swarm, sessions, spawn.
    const down = interpretKey(composer, key({ key: "j" }))
    expect(down).toEqual({ kind: "navigate-pane", pane: "swarm" })
    // At the top, k cannot move up.
    expect(interpretKey(composer, key({ key: "k" })).kind).toBe("none")
  })

  test("PressedKey through the reducer opens the palette (full wiring)", () => {
    const [model] = update(initialModel, PressedKey({
      key: "k",
      meta: true,
      ctrl: false,
      shift: false,
      inEditable: false,
    }))
    expect(model.commandPaletteOpen).toBe(true)
  })

  test("the Settings shortcut listing is non-empty and includes the palette + groups", () => {
    expect(SHORTCUTS.length).toBeGreaterThan(0)
    const text = SHORTCUTS.map((s) => `${s.chord} ${s.description}`).join(" | ")
    expect(text).toContain("command palette")
    expect(text).toContain("Jump to Chat")
  })
})

// ── Integration: shell still renders a Document for every pane (black-screen) ─
describe("nav shell keeps the view mountable (black-screen guard holds)", () => {
  const isMountable = (doc: unknown): boolean => {
    if (typeof doc !== "object" || doc === null) return false
    const record = doc as Record<string, unknown>
    return typeof record.title === "string" && "body" in record && record.body != null
  }

  test("every pane renders a Document with the grouped sidebar", () => {
    for (const pane of PaneId.literals) {
      const model: ModelType = Model.make({ ...initialModel, pane })
      expect({ pane, ok: isMountable(view(model)) }).toEqual({ pane, ok: true })
    }
  })

  test("the open command palette renders a Document (overlay never blanks)", () => {
    const model = Model.make({ ...initialModel, pane: "chat", commandPaletteOpen: true })
    expect(isMountable(view(model))).toBe(true)
    const queried = Model.make({ ...model, commandPaletteQuery: "swarm" })
    expect(isMountable(view(queried))).toBe(true)
  })
})
