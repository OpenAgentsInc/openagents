// #5730 The Verse: the runtime toggle for the game-world view behind chat.
//
// Covers, without a live node:
//   1. the default — verseEnabled is ON so the Verse shows by default now.
//   2. the ToggleVerse reducer — flips the boolean and is its own inverse.
//   3. the ⌘⇧V keyboard path through PressedKey resolves to the same toggle.
//   4. the hotbar carries a labeled "Verse" toggle slot (lit when on).

import { afterEach, describe, expect, test } from "bun:test"

import { initialModel } from "../src/ui/model"
import { update } from "../src/ui/update"
import { interpretKey } from "../src/ui/keyboard"
import { HOTBAR_SLOTS } from "../src/ui/nav"
import { PressedKey, ToggleVerse } from "../src/ui/message"
import {
  agentCharacterCreationFlag,
  chatWorldBuildFlags,
} from "../src/shared/chat-world-flags"

const VERSE_ENV_KEYS = [
  "VITE_VERSE_ENABLED",
  "VITE_DISABLE_VERSE",
  "VITE_VERSE_DISABLED",
  "VITE_CHAT_WORLD_SCENE",
  "VITE_CHAT_WORLD_PAYMENTS",
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
  })

  test("launch build flags default the whole Verse bundle ON", () => {
    clearVerseEnv()
    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: true,
      CHAT_WORLD_PAYMENTS: true,
    })
    expect(agentCharacterCreationFlag()).toBe(true)
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
  })

  test("legacy per-feature flags can still enable a partial debug bundle", () => {
    clearVerseEnv()
    process.env.VITE_VERSE_ENABLED = "0"
    process.env.VITE_CHAT_WORLD_SCENE = "1"
    process.env.VITE_CHAT_WORLD_PAYMENTS = "0"
    process.env.VITE_AGENT_CHARACTER_CREATION = "0"

    expect(chatWorldBuildFlags()).toEqual({
      CHAT_WORLD_SCENE: true,
      CHAT_WORLD_PAYMENTS: false,
    })
    expect(agentCharacterCreationFlag()).toBe(false)
  })

  test("ToggleVerse flips the flag and is its own inverse", () => {
    const [off] = update(initialModel, ToggleVerse())
    expect(off.verseEnabled).toBe(false)
    const [backOn] = update(off, ToggleVerse())
    expect(backOn.verseEnabled).toBe(true)
  })

  test("⌘⇧V resolves to the toggle-verse intent and flips the flag", () => {
    const intent = interpretKey(initialModel, {
      key: "v",
      meta: true,
      ctrl: false,
      shift: true,
      inEditable: false,
    })
    expect(intent.kind).toBe("toggle-verse")

    // Through the reducer's PressedKey path (Ctrl variant, while typing — the
    // toggle is a deliberate global shortcut).
    const [next] = update(
      initialModel,
      PressedKey({
        key: "v",
        meta: false,
        ctrl: true,
        shift: true,
        inEditable: true,
      }),
    )
    expect(next.verseEnabled).toBe(false)
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

  test("the hotbar carries a labeled Verse toggle slot", () => {
    const verse = HOTBAR_SLOTS.find((slot) => slot.kind === "verse")
    expect(verse).toBeDefined()
    expect(verse?.kind === "verse" ? verse.label : "").toBe("Verse")
  })
})
