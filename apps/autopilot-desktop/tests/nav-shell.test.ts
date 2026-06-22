import { describe, expect, test } from "bun:test"
import { openAgentsDefaultInputProfile, type OpenAgentsInputProfile } from "@openagentsinc/input-bindings"

import { interpretKey, type KeyEvent } from "../src/ui/keyboard"
import { initialRuntimeState } from "../src/ui/initial-state"
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
import { initialModel, Model, modelPaneLayer, PaneId, type Model as ModelType } from "../src/ui/model"
import {
  CODE_MODE_SHORTCUTS,
  CommandScope,
  HOTBAR_SLOTS,
  NAV_DESTINATIONS,
  NAV_GROUPS,
  NAV_LEAF_PANES,
  SHORTCUTS,
  codeModePaletteCommands,
  filterPaletteCommands,
  groupByAccel,
  groupForPane,
  paletteCommands,
} from "../src/ui/nav"
import { update } from "../src/ui/update"
import { view } from "../src/ui/view"
import { keyboardForwardDecision } from "../src/ui/subscriptions"

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

  test("key actions are registered (spawn, Blueprint, intent, approvals, coordinator, replay)", () => {
    const ids = new Set(paletteCommands.map((c) => c.id))
    for (const id of [
      "action.spawn",
      "action.blueprint-chat",
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

  test("code-mode command registry is scoped, keybound, and separate from global nav", () => {
    expect(codeModePaletteCommands.length).toBeGreaterThan(0)
    const scopeSet = new Set(CommandScope)
    for (const command of codeModePaletteCommands) {
      expect(command.group).toBe("code")
      expect(command.keybinding).toContain("⌘K")
      expect(command.scopes.length).toBeGreaterThan(0)
      for (const scope of command.scopes) expect(scopeSet.has(scope)).toBe(true)
    }
    expect(codeModePaletteCommands.some((command) => command.scopes.includes("accounts"))).toBe(true)
    expect(codeModePaletteCommands.some((command) => command.scopes.includes("approvals"))).toBe(true)
    expect(codeModePaletteCommands.some((command) => command.scopes.includes("diffs"))).toBe(true)
    expect(codeModePaletteCommands.some((command) => command.scopes.includes("diagnostics"))).toBe(true)
    expect(codeModePaletteCommands.map((command) => command.id)).not.toContain("go.network")
  })

  test("code-mode command shortcuts are generated from registry rows", () => {
    expect(CODE_MODE_SHORTCUTS).toHaveLength(codeModePaletteCommands.length)
    expect(CODE_MODE_SHORTCUTS.map((shortcut) => shortcut.chord)).toEqual(
      codeModePaletteCommands.map((command) => command.keybinding),
    )
    expect(CODE_MODE_SHORTCUTS.map((shortcut) => shortcut.description)).toEqual(
      codeModePaletteCommands.map((command) => command.label),
    )
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

  test("Verse code mode palette runs the scoped code command set", () => {
    const open = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      commandPaletteOpen: true,
    })
    const tree = serializeView(view(open).body)
    expect(tree).toContain("Open Accounts pane")
    expect(tree).toContain("⌘K accounts")
    expect(tree).not.toContain("Go to Network")

    const [model] = update(open, RanPaletteCommand({ commandId: "code.pane.accounts" }))
    expect(model.commandPaletteOpen).toBe(false)
    expect(modelPaneLayer(model).panes.map((pane) => pane.kind)).toEqual(["accounts"])
  })

  test("Verse code mode ignores global palette command ids", () => {
    const open = Model.make({
      ...initialModel,
      pane: "chat",
      verseMode: "code",
      commandPaletteOpen: true,
    })
    const [model, commands] = update(open, RanPaletteCommand({ commandId: "go.network" }))
    expect(model.pane).toBe("chat")
    expect(model.commandPaletteOpen).toBe(false)
    expect(commands).toHaveLength(0)
  })
})

// ── #5465: keyboard layer (pure interpretation + reducer wiring) ─────────────
describe("#5465 keyboard layer", () => {
  test("Cmd-K and Ctrl-K open the palette unless an editor owns focus", () => {
    expect(interpretKey(initialModel, key({ key: "k", meta: true })).kind).toBe("open-palette")
    expect(interpretKey(initialModel, key({ key: "k", ctrl: true })).kind).toBe("open-palette")
    expect(
      interpretKey(initialModel, key({ key: "k", meta: true, inEditable: true })).kind,
    ).toBe("none")
  })

  test("while the palette is open, arrows move, Enter runs, Esc closes", () => {
    const open = Model.make({ ...initialModel, commandPaletteOpen: true })
    expect(interpretKey(open, key({ key: "ArrowDown" })).kind).toBe("palette-move")
    expect(interpretKey(open, key({ key: "ArrowUp" })).kind).toBe("palette-move")
    expect(interpretKey(open, key({ key: "Enter" })).kind).toBe("palette-run")
    expect(interpretKey(open, key({ key: "Escape" })).kind).toBe("close-palette")
  })

  test("Cmd-number no longer jumps to hotbar groups", () => {
    for (const n of [1, 2, 3, 4, 5]) {
      const intent = interpretKey(initialModel, key({ key: String(n), meta: true }))
      expect(intent.kind).toBe("none")
    }
  })

  test("native edit shortcuts are not intercepted by the webview shortcut filter", () => {
    for (const key of ["c", "v", "x", "a", "z"]) {
      expect(
        keyboardForwardDecision({ key, meta: true, ctrl: false, inEditable: true }),
      ).toEqual({ forward: false, preventDefault: false })
    }
    expect(
      keyboardForwardDecision({
        key: "ArrowUp",
        meta: true,
        ctrl: false,
        inEditable: true,
      }),
    ).toEqual({ forward: false, preventDefault: false })
    expect(
      keyboardForwardDecision({ key: "k", meta: true, ctrl: false, inEditable: true }),
    ).toEqual({ forward: false, preventDefault: false })
    expect(
      keyboardForwardDecision({
        key: "Escape",
        meta: false,
        ctrl: false,
        inEditable: true,
      }),
    ).toEqual({ forward: true, preventDefault: true })
  })

  test("Cmd/Ctrl-Shift-V is forwarded through the real subscription decision path", () => {
    expect(
      keyboardForwardDecision({
        key: "v",
        meta: true,
        ctrl: false,
        shift: true,
        inEditable: false,
      }),
    ).toEqual({ forward: true, preventDefault: true })
    expect(
      keyboardForwardDecision({
        key: "v",
        meta: false,
        ctrl: true,
        shift: true,
        inEditable: false,
      }),
    ).toEqual({ forward: true, preventDefault: true })
    expect(
      keyboardForwardDecision({
        key: "v",
        meta: true,
        ctrl: false,
        shift: true,
        inEditable: true,
      }),
    ).toEqual({ forward: false, preventDefault: false })
  })

  test("Verse action-bar number keys are consumed so the host never dings", () => {
    for (const [keyValue, code] of [
      ["1", "Digit1"],
      ["2", "Digit2"],
      ["0", "Digit0"],
    ] as const) {
      expect(
        keyboardForwardDecision({
          key: keyValue,
          code,
          meta: false,
          ctrl: false,
          shift: false,
          inEditable: false,
        }),
      ).toEqual({ forward: true, preventDefault: true })
    }

    expect(
      keyboardForwardDecision({
        key: "1",
        code: "Digit1",
        meta: false,
        ctrl: false,
        shift: false,
        inEditable: true,
      }),
    ).toEqual({ forward: false, preventDefault: false })
  })

  test("desktop shortcut matching is exact about unbound extra modifiers", () => {
    expect(
      keyboardForwardDecision({
        key: "k",
        meta: true,
        ctrl: false,
        shift: false,
        inEditable: false,
      }),
    ).toEqual({ forward: true, preventDefault: true })
    expect(
      keyboardForwardDecision({
        key: "k",
        meta: true,
        ctrl: false,
        shift: true,
        inEditable: false,
      }),
    ).toEqual({ forward: false, preventDefault: false })
  })

  test("desktop forwarding follows a custom action map instead of a raw key whitelist", () => {
    const profile: OpenAgentsInputProfile = {
      ...openAgentsDefaultInputProfile,
      profileId: "test-custom-palette",
      bindings: {
        ...openAgentsDefaultInputProfile.bindings,
        "app.command_palette": [
          { type: "keyboard_key", key: "p", modifiers: { primary: true } },
        ],
      },
    }

    expect(
      keyboardForwardDecision({
        key: "k",
        meta: true,
        ctrl: false,
        inEditable: false,
      }, profile.bindings),
    ).toEqual({ forward: false, preventDefault: false })
    expect(
      keyboardForwardDecision({
        key: "p",
        meta: true,
        ctrl: false,
        inEditable: false,
      }, profile.bindings),
    ).toEqual({ forward: true, preventDefault: true })
  })

  test("Cmd-Enter submits in chat/composer, and is a no-op elsewhere", () => {
    const chat = Model.make({ ...initialModel, pane: "chat" })
    expect(interpretKey(chat, key({ key: "Enter", meta: true })).kind).toBe("submit-turn")
    expect(interpretKey(chat, key({ key: "Enter", meta: true, inEditable: true })).kind).toBe("none")
    const settings = Model.make({ ...initialModel, pane: "settings" })
    expect(interpretKey(settings, key({ key: "Enter", meta: true })).kind).toBe("none")
  })

  test("composer/editor/terminal editable focus suspends command shortcuts", () => {
    const code = Model.make({ ...initialModel, pane: "composer" })
    expect(interpretKey(code, key({ key: "j", inEditable: true })).kind).toBe("none")
    expect(interpretKey(code, key({ key: "j", inEditable: false })).kind).toBe("navigate-pane")
    expect(interpretKey(code, key({ key: "k", meta: true, inEditable: true })).kind).toBe("none")
    expect(interpretKey(code, key({ key: "v", meta: true, shift: true, inEditable: true })).kind).toBe("none")
    expect(keyboardForwardDecision({ key: "w", meta: false, ctrl: false, inEditable: true })).toEqual({
      forward: false,
      preventDefault: false,
    })
    expect(keyboardForwardDecision({ key: "k", meta: true, ctrl: false, inEditable: true })).toEqual({
      forward: false,
      preventDefault: false,
    })
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

  test("the Settings shortcut listing is non-empty and does not advertise hotbar number jumps", () => {
    expect(SHORTCUTS.length).toBeGreaterThan(0)
    const text = SHORTCUTS.map((s) => `${s.chord} ${s.description}`).join(" | ")
    expect(text).toContain("command palette")
    expect(text).not.toContain("Jump to Chat")
    expect(text).not.toContain("1 … 9")
  })
})

// ── Integration: shell still renders a Document for every pane (black-screen) ─
describe("nav shell keeps the view mountable (black-screen guard holds)", () => {
  const isMountable = (doc: unknown): boolean => {
    if (typeof doc !== "object" || doc === null) return false
    const record = doc as Record<string, unknown>
    return typeof record.title === "string" && "body" in record && record.body != null
  }

  test("every pane renders a mountable Document", () => {
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

  test("fresh runtime first paint is the immersive Verse, not advanced code chrome", () => {
    const [model] = initialRuntimeState()
    const tree = serializeView(view(model).body)

    expect(model.pane).toBe("chat")
    expect(tree).toContain("app-shell-verse")
    expect(tree).toContain("data-verse-focus-root")
    expect(tree).toContain("chat-pane-world")
    expect(tree).toContain("verse-run-hud")
    expect(tree).toContain("Tassadar")
    expect(tree).toContain("Pylon")
    expect(tree).not.toContain("The Verse")
    expect(tree).not.toContain("Advanced")
    expect(tree).not.toContain("Send message")
    expect(tree).not.toContain("chat-composer-verse")
    expect(tree).not.toContain("Command palette")
    expect(tree).not.toContain("⌘K")
    expect(tree).not.toContain("chat-thread-shell")
    expect(tree).not.toContain("chat-message-list")
    expect(tree).not.toContain("pylon-base-status")
    expect(tree).not.toContain("character-creation-overlay")
    expect(tree).toContain("hotbar-slot")
    expect(tree).toContain("hotbar-slot-coder")
    expect(tree).toContain("data-hotbar-icon")
    expect(tree).toContain("OpenaiLogoRegular")
    expect(tree).toContain("New Coder Session")
    expect(tree).not.toContain("sidebar")
    expect(tree).not.toContain("status-hud-overlay")
    expect(tree).not.toContain("shell-target-tabs")
    expect(tree).not.toContain("Go to Composer")
    expect(tree).not.toContain("Spawn a session")
    expect(tree).not.toContain("Sessions")
    expect(tree).not.toContain("Swarm")
    expect(tree).not.toContain("Deploy")
    expect(tree).not.toContain("Claude Code")
    expect(tree).not.toContain("Codex")
  })

  test("fresh Verse first paint disables Cmd-K advanced paths while HUD actions are off", () => {
    const [start] = initialRuntimeState()
    const [palette] = update(start, OpenedCommandPalette())
    const tree = serializeView(view(palette).body)

    expect(palette.commandPaletteOpen).toBe(false)
    expect(tree).not.toContain("Go to Composer")
    expect(tree).not.toContain("Go to Sessions")
    expect(tree).not.toContain("Go to Swarm")
    expect(tree).not.toContain("Go to Spawn")
    expect(tree).not.toContain("Spawn a session")
  })
})

// ── HUD H1: the Verse action hotbar (#5499) ──────────────────────────────────
// Cycle-safe serialize so we can assert what the hotbar renders without a DOM
// (plain foldkit Html objects — same approach as zero-base-shell.test.ts).
const serializeView = (node: unknown): string => {
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

describe("#5499 HUD H1 hotbar — Verse action bindings", () => {
  const exploreModel = Model.make({
    ...initialModel,
    pane: "chat",
    verseEnabled: true,
    verseMode: "explore",
  })

  test("the hotbar exposes ten action-bar slots in keyboard order", () => {
    expect(HOTBAR_SLOTS).toHaveLength(10)
    expect(HOTBAR_SLOTS.map((slot) => slot.actionId)).toEqual([
      "action_bar.slot_1",
      "action_bar.slot_2",
      "action_bar.slot_3",
      "action_bar.slot_4",
      "action_bar.slot_5",
      "action_bar.slot_6",
      "action_bar.slot_7",
      "action_bar.slot_8",
      "action_bar.slot_9",
      "action_bar.slot_10",
    ])
    expect(HOTBAR_SLOTS[0]).toMatchObject({
      actionId: "action_bar.slot_1",
      iconName: "OpenaiLogoRegular",
      label: "New Coder Session",
      number: 1,
    })
  })

  test("slot 1 opens a fresh coder-session surface", () => {
    expect(interpretKey(exploreModel, key({ key: "1", code: "Digit1" }))).toEqual({
      kind: "open-coder-session",
    })
    expect(interpretKey(exploreModel, key({ key: "2", code: "Digit2" }))).toEqual({
      kind: "none",
    })

    const [model, commands] = update(
      exploreModel,
      PressedKey({
        key: "1",
        code: "Digit1",
        meta: false,
        ctrl: false,
        shift: false,
        inEditable: false,
      }),
    )
    expect(model.pane).toBe("chat")
    expect(model.verseEnabled).toBe(true)
    expect(model.verseMode).toBe("code")
    expect(model.composerSessionRef).toBeNull()
    expect(model.spawnObjective).toBe("")
    expect(commands.map((command) => command.name)).toContain("LoadManagedAccounts")
  })

  test("bare number keys remain ignored while typing", () => {
    expect(interpretKey(exploreModel, key({
      key: "1",
      code: "Digit1",
      inEditable: true,
    })).kind).toBe("none")
  })

  test("the shell renders the bottom-left hotbar with the text input to its right", () => {
    const tree = serializeView(view(Model.make({ ...initialModel, pane: "shell" })).body)
    expect(tree).toContain("shell-bar")
    expect(tree).toContain("hotbar")
    expect(tree).toContain("hotbar-inline")
    expect(tree).toContain("shell-target-tabs")
    expect(tree.indexOf("hotbar-inline")).toBeLessThan(tree.indexOf("shell-target-tabs"))
    expect(tree.indexOf("shell-target-tabs")).toBeLessThan(tree.indexOf("shell-input"))
    expect(tree).toContain("shell-input")
    expect(tree.indexOf("hotbar-inline")).toBeLessThan(tree.indexOf("shell-input"))
    expect(tree).toContain("hotbar-slot")
    expect(tree).toContain("hotbar-slot-coder")
    expect(tree).toContain("hotbar-slot-icon")
    expect(tree).toContain("hotbar-slot-key")
    expect(tree).toContain("OpenaiLogoRegular")
    expect(tree).toContain("New Coder Session (1)")
    expect(tree).not.toContain("⌘K")
  })

  test("the hotbar face is concise and does not inline raw SVG markup", () => {
    const tree = serializeView(view(Model.make({ ...initialModel, pane: "composer" })).body)
    expect(tree).toContain("hotbar-slot-empty")
    expect(tree).toContain("hotbar-slot-coder")
    expect(tree).toContain("New Coder Session (1)")
    expect(tree).toContain("Action Slot 10 (0)")
    expect(tree).toContain("hotbar-slot-tooltip")
    expect(tree).not.toContain("hotbar-slot-palette")
    expect(tree).not.toContain("hotbar-slot-verse")
    expect(tree).not.toContain("<svg")
    expect(tree).not.toContain("Command palette")
  })

  test("the hotbar renders on the full UI as a bottom-left floating action strip", () => {
    const tree = serializeView(view(Model.make({ ...initialModel, pane: "composer" })).body)
    expect(tree).toContain("hotbar")
    expect(tree).toContain("hotbar-floating")
    expect(tree).not.toContain("hotbar-slot-group")
  })

  test("the hotbar is hidden in immersive training fullscreen (does not occlude the scene)", () => {
    const tree = serializeView(
      view(Model.make({ ...initialModel, pane: "training-fullscreen" })).body,
    )
    expect(tree).not.toContain("hotbar-slot")
  })
})

// ── HUD H7: the live status/meters overlay placement (#5504) ─────────────────
describe("#5504 HUD H7 status/meters overlay — placed on the full UI only", () => {
  test("renders the status HUD overlay element on the full multi-pane UI", () => {
    const tree = serializeView(
      view(Model.make({ ...initialModel, pane: "composer" })).body,
    )
    expect(tree).toContain("status-hud-overlay")
    expect(tree).toContain("oa-desktop-status-hud")
  })

  test("is NOT on the black zero-base shell (keep the launch screen quiet/black)", () => {
    const tree = serializeView(
      view(Model.make({ ...initialModel, pane: "shell" })).body,
    )
    expect(tree).not.toContain("status-hud-overlay")
  })

  test("is hidden in immersive training fullscreen (does not occlude the scene)", () => {
    const tree = serializeView(
      view(Model.make({ ...initialModel, pane: "training-fullscreen" })).body,
    )
    expect(tree).not.toContain("status-hud-overlay")
  })
})
