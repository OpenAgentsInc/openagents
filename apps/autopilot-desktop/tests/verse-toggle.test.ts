// #5730 The Verse: the runtime toggle for the game-world view behind chat.
//
// Covers, without a live node:
//   1. the default — verseEnabled is ON so the Verse shows by default now.
//   2. the ToggleVerse reducer — flips the boolean and is its own inverse.
//   3. the ⌘⇧V keyboard path through PressedKey resolves to the same toggle.
//   4. the hotbar carries a labeled "Verse" toggle slot (lit when on).

import { afterEach, describe, expect, test } from "bun:test"

import { initialModel, Model, modelPaneLayer } from "../src/ui/model"
import { update } from "../src/ui/update"
import { interpretKey } from "../src/ui/keyboard"
import { HOTBAR_SLOTS } from "../src/ui/nav"
import { verseSceneVisualization } from "../src/ui/view"
import {
  ChangedVerseMode,
  OpenedManagedPane,
  PressedKey,
  ToggleVerse,
} from "../src/ui/message"
import {
  agentCharacterCreationFlag,
  chatWorldBuildFlags,
  chatWorldHudFlag,
  chatWorldMultiplayerFlag,
} from "../src/shared/chat-world-flags"

const VERSE_ENV_KEYS = [
  "VITE_VERSE_ENABLED",
  "VITE_DISABLE_VERSE",
  "VITE_VERSE_DISABLED",
  "VITE_CHAT_WORLD_SCENE",
  "VITE_CHAT_WORLD_PAYMENTS",
  "VITE_CHAT_WORLD_MULTIPLAYER",
  "VITE_CHAT_WORLD_HUD",
  "VITE_AGENT_CHARACTER_CREATION",
] as const

const savedEnv = new Map<string, string | undefined>(
  VERSE_ENV_KEYS.map((key) => [key, process.env[key]]),
)

afterEach(() => {
  for (const key of VERSE_ENV_KEYS) {
    const value = savedEnv.get(key)
    if (value === undefined) delete process.env[key]
    else process.env[key] = value
  }
})

const clearVerseEnv = (): void => {
  for (const key of VERSE_ENV_KEYS) delete process.env[key]
}

describe("The Verse runtime toggle (#5730)", () => {
  test("defaults ON so the Verse shows by default", () => {
    expect(initialModel.verseEnabled).toBe(true)
    expect(initialModel.verseMode).toBe("explore")
  })

  test("launch build flags default the whole Verse bundle ON", () => {
    clearVerseEnv()
    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: true,
      CHAT_WORLD_PAYMENTS: true,
    })
    expect(chatWorldHudFlag()).toBe(false)
    expect(agentCharacterCreationFlag()).toBe(true)
    expect(chatWorldMultiplayerFlag()).toBe(true)
  })

  test("launch build keeps an explicit Verse kill switch", () => {
    clearVerseEnv()
    process.env.VITE_DISABLE_VERSE = "1"
    process.env.VITE_CHAT_WORLD_SCENE = "1"
    process.env.VITE_CHAT_WORLD_PAYMENTS = "1"
    process.env.VITE_AGENT_CHARACTER_CREATION = "1"

    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: false,
      CHAT_WORLD_PAYMENTS: false,
    })
    expect(agentCharacterCreationFlag()).toBe(false)
    expect(chatWorldMultiplayerFlag()).toBe(false)
  })

  test("legacy per-feature flags can still enable a partial debug bundle", () => {
    clearVerseEnv()
    process.env.VITE_VERSE_ENABLED = "0"
    process.env.VITE_CHAT_WORLD_SCENE = "1"
    process.env.VITE_CHAT_WORLD_PAYMENTS = "0"
    process.env.VITE_CHAT_WORLD_MULTIPLAYER = "0"
    process.env.VITE_AGENT_CHARACTER_CREATION = "0"

    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: true,
      CHAT_WORLD_PAYMENTS: false,
    })
    expect(agentCharacterCreationFlag()).toBe(false)
    expect(chatWorldMultiplayerFlag()).toBe(false)
    expect(chatWorldHudFlag()).toBe(false)
  })

  test("the Verse HUD/actions flag is an explicit opt-in", () => {
    clearVerseEnv()
    expect(chatWorldHudFlag()).toBe(false)

    process.env.VITE_CHAT_WORLD_HUD = "1"
    expect(chatWorldHudFlag()).toBe(true)
  })

  test("code mode ToggleVerse flips the flag and is its own inverse", () => {
    clearVerseEnv()
    const [codeMode] = update(initialModel, ChangedVerseMode({ mode: "code" }))
    const [off] = update(codeMode, ToggleVerse())
    expect(off.verseEnabled).toBe(false)
    const [backOn] = update(off, ToggleVerse())
    expect(backOn.verseEnabled).toBe(true)
  })

  test("explore mode ignores palette, pane, and Verse toggle shortcuts", () => {
    clearVerseEnv()
    const verseModel = Model.make({ ...initialModel, pane: "chat" })
    const [palette] = update(verseModel, PressedKey({
      key: "k",
      meta: true,
      ctrl: false,
      shift: false,
      inEditable: false,
    }))
    expect(palette.commandPaletteOpen).toBe(false)

    const [toggle] = update(verseModel, PressedKey({
      key: "v",
      meta: true,
      ctrl: false,
      shift: true,
      inEditable: false,
    }))
    expect(toggle.verseEnabled).toBe(true)

    const [directToggle] = update(verseModel, ToggleVerse())
    expect(directToggle.verseEnabled).toBe(true)

    const [paneAttempt] = update(verseModel, OpenedManagedPane({ pane: "composer" }))
    expect(modelPaneLayer(paneAttempt).panes).toHaveLength(0)
  })

  test("code mode permits palette, pane, and Verse toggle shortcuts", () => {
    clearVerseEnv()
    const verseModel = Model.make({ ...initialModel, pane: "chat", verseMode: "code" })
    const [palette] = update(verseModel, PressedKey({
      key: "k",
      meta: true,
      ctrl: false,
      shift: false,
      inEditable: false,
    }))
    expect(palette.commandPaletteOpen).toBe(true)

    const [withPane] = update(verseModel, OpenedManagedPane({ pane: "composer" }))
    expect(modelPaneLayer(withPane).panes.map((pane) => pane.kind)).toEqual(["composer"])

    const [toggle] = update(verseModel, PressedKey({
      key: "v",
      meta: true,
      ctrl: false,
      shift: true,
      inEditable: false,
    }))
    expect(toggle.verseEnabled).toBe(false)
  })

  test("switching Verse modes does not change scene visualization or restore pose", () => {
    clearVerseEnv()
    const pose = {
      regionRef: "world.region.tassadar",
      x: 7.25,
      y: 0,
      z: -3.5,
      yaw: 1.2,
      animation: "run",
      capturedAtMs: 12345,
    } as const
    const explore = Model.make({
      ...initialModel,
      pane: "chat",
      verseSceneRestorePose: pose,
    })
    const before = verseSceneVisualization(explore)
    const [code] = update(explore, ChangedVerseMode({ mode: "code" }))
    expect(code.verseSceneRestorePose).toEqual(pose)
    expect(verseSceneVisualization(code)).toEqual(before)

    const [backToExplore] = update(code, ChangedVerseMode({ mode: "explore" }))
    expect(backToExplore.verseSceneRestorePose).toEqual(pose)
    expect(verseSceneVisualization(backToExplore)).toEqual(before)
  })

  test("⌘⇧V resolves to the toggle-verse intent and flips the flag", () => {
    clearVerseEnv()
    const intent = interpretKey(initialModel, {
      key: "v",
      meta: true,
      ctrl: false,
      shift: true,
      inEditable: false,
    })
    expect(intent.kind).toBe("toggle-verse")

    // Through the reducer's PressedKey path (Ctrl variant). Editable fields own
    // their keys in code mode, so the shortcut only fires outside text entry.
    const [next] = update(
      Model.make({ ...initialModel, verseMode: "code" }),
      PressedKey({
        key: "v",
        meta: false,
        ctrl: true,
        shift: true,
        inEditable: false,
      }),
    )
    expect(next.verseEnabled).toBe(false)

    const [typing] = update(
      Model.make({ ...initialModel, verseMode: "code" }),
      PressedKey({
        key: "v",
        meta: false,
        ctrl: true,
        shift: true,
        inEditable: true,
      }),
    )
    expect(typing.verseEnabled).toBe(true)
  })

  test("bare V (no modifier) does not toggle the Verse", () => {
    const intent = interpretKey(initialModel, {
      key: "v",
      meta: false,
      ctrl: false,
      shift: false,
      inEditable: false,
    })
    expect(intent.kind).not.toBe("toggle-verse")
  })

  test("the hotbar carries a default coder action slot", () => {
    expect(HOTBAR_SLOTS[0]).toMatchObject({
      actionId: "action_bar.slot_1",
      iconName: "OpenaiLogoRegular",
      label: "New Coder Session",
      number: 1,
    })
  })
})
