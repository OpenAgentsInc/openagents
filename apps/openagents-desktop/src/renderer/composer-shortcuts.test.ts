/**
 * Composer Shift+Tab harness toggle (EP250 owner statement, verbatim: "i
 * want shift+tab to togle between modes in composer (fable / codex) in this
 * case"). Enforces: focused-composer Shift+Tab toggles BOTH directions and
 * preventDefaults (focus never moves); Shift+Tab with focus elsewhere never
 * toggles; plain Tab is untouched; toggling TO an unavailable lane is
 * allowed (capability truth lives on the chip/Send, never a silent block of
 * the gesture — the handler does not consult availability at all).
 */
import { describe, expect, test } from "vite-plus/test"

import {
  handleComposerShiftTab,
  nextComposerHarness,
} from "./composer-shortcuts.ts"
import type { DesktopHarnessName } from "./shell.ts"

type FakeEvent = {
  key: string
  shiftKey: boolean
  defaultPrevented: boolean
  target: unknown
  preventDefault: () => void
  prevented: boolean
}

const makeEvent = (overrides: Partial<FakeEvent> = {}): FakeEvent => {
  const event: FakeEvent = {
    key: "Tab",
    shiftKey: true,
    defaultPrevented: false,
    target: "composer",
    prevented: false,
    preventDefault: () => {
      event.prevented = true
    },
    ...overrides,
  }
  return event
}

const makeHooks = (selected: DesktopHarnessName) => {
  const calls: DesktopHarnessName[] = []
  let current = selected
  return {
    calls,
    current: () => current,
    hooks: {
      isComposerInput: (target: unknown) => target === "composer",
      selectedHarness: () => current,
      selectHarness: (harness: DesktopHarnessName) => {
        calls.push(harness)
        current = harness
      },
    },
  }
}

describe("nextComposerHarness", () => {
  test("toggles both directions", () => {
    expect(nextComposerHarness("fable")).toBe("codex")
    expect(nextComposerHarness("codex")).toBe("fable")
  })
})

describe("handleComposerShiftTab", () => {
  test("focused-composer Shift+Tab toggles fable -> codex and preventDefaults", () => {
    const { hooks, calls } = makeHooks("fable")
    const event = makeEvent()
    expect(handleComposerShiftTab(event, hooks)).toBe(true)
    expect(event.prevented).toBe(true)
    expect(calls).toEqual(["codex"])
  })

  test("focused-composer Shift+Tab toggles codex -> fable (both directions)", () => {
    const { hooks, calls, current } = makeHooks("codex")
    const first = makeEvent()
    expect(handleComposerShiftTab(first, hooks)).toBe(true)
    expect(calls).toEqual(["fable"])
    const second = makeEvent()
    expect(handleComposerShiftTab(second, hooks)).toBe(true)
    expect(calls).toEqual(["fable", "codex"])
    expect(current()).toBe("codex")
  })

  test("Shift+Tab with focus OUTSIDE the composer does NOT toggle and does NOT preventDefault (normal focus navigation)", () => {
    const { hooks, calls } = makeHooks("fable")
    const event = makeEvent({ target: "sidebar-button" })
    expect(handleComposerShiftTab(event, hooks)).toBe(false)
    expect(event.prevented).toBe(false)
    expect(calls).toEqual([])
  })

  test("plain Tab (no shift) in the composer is untouched", () => {
    const { hooks, calls } = makeHooks("fable")
    const event = makeEvent({ shiftKey: false })
    expect(handleComposerShiftTab(event, hooks)).toBe(false)
    expect(event.prevented).toBe(false)
    expect(calls).toEqual([])
  })

  test("an already-consumed event is left alone", () => {
    const { hooks, calls } = makeHooks("fable")
    const event = makeEvent({ defaultPrevented: true })
    expect(handleComposerShiftTab(event, hooks)).toBe(false)
    expect(calls).toEqual([])
  })

  test("toggling TO an unavailable lane is allowed — the handler never consults availability", () => {
    // The hooks expose no availability at all: selection moves regardless;
    // the disabled-reason popover and evidence-gated Send explain why the
    // lane cannot act. The gesture is never silently blocked.
    const { hooks, calls } = makeHooks("fable")
    expect(handleComposerShiftTab(makeEvent(), hooks)).toBe(true)
    expect(calls).toEqual(["codex"])
  })
})
