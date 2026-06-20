// #5730 The Verse: the runtime toggle for the game-world view behind chat.
//
// Covers, without a live node:
//   1. the default — verseEnabled is ON so the Verse shows by default now.
//   2. the ToggleVerse reducer — flips the boolean and is its own inverse.
//   3. the ⌘⇧V keyboard path through PressedKey resolves to the same toggle.
//   4. the hotbar carries a labeled "Verse" toggle slot (lit when on).

import { describe, expect, test } from "bun:test"

import { initialModel } from "../src/ui/model"
import { update } from "../src/ui/update"
import { interpretKey } from "../src/ui/keyboard"
import { HOTBAR_SLOTS } from "../src/ui/nav"
import { PressedKey, ToggleVerse } from "../src/ui/message"

describe("The Verse runtime toggle (#5730)", () => {
  test("defaults ON so the Verse shows by default", () => {
    expect(initialModel.verseEnabled).toBe(true)
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
