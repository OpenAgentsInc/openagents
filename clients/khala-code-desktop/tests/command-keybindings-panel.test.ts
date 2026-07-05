import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  createKhalaCodeCommandKeybindingsSection,
  readKhalaCodeCommandKeybindingOverrides,
  writeKhalaCodeCommandKeybindingOverrides,
} from "../src/ui/command-keybindings-panel"
import {
  createKhalaCodeCommandRegistry,
  type KhalaCodeCommandDefinition,
  type KhalaCodeCommandId,
} from "../src/ui/command-registry"

let previousDocument: typeof globalThis.document | undefined
let previousWindow: typeof globalThis.window | undefined
let previousRequestAnimationFrame: typeof globalThis.requestAnimationFrame | undefined

const command = (
  input: Partial<KhalaCodeCommandDefinition> & Pick<KhalaCodeCommandDefinition, "id" | "title">,
): KhalaCodeCommandDefinition => ({
  analyticsRef: `test.${input.id}`,
  category: "navigation",
  execute: () => undefined,
  ...input,
})

beforeEach(() => {
  previousDocument = globalThis.document
  previousWindow = globalThis.window
  previousRequestAnimationFrame = globalThis.requestAnimationFrame
  const win = new Window()
  Object.assign(globalThis, {
    document: win.document,
    requestAnimationFrame: (callback: FrameRequestCallback) => {
      callback(0)
      return 0
    },
    window: win,
  })
})

afterEach(() => {
  Object.assign(globalThis, {
    document: previousDocument,
    requestAnimationFrame: previousRequestAnimationFrame,
    window: previousWindow,
  })
})

describe("Khala Code command keybindings settings", () => {
  test("persists overrides, captures keys, blocks conflicts, and restores defaults", () => {
    const storage = window.localStorage
    writeKhalaCodeCommandKeybindingOverrides(storage, { "view.chat": "ctrl+j" })
    expect(readKhalaCodeCommandKeybindingOverrides(storage)).toEqual({ "view.chat": "ctrl+j" })

    const overrides: Partial<Record<KhalaCodeCommandId, string>> = {}
    const registry = createKhalaCodeCommandRegistry([
      command({
        defaultKeybindings: [{ alt: true, key: "1" }],
        id: "view.chat",
        title: "Open Chat",
      }),
      command({
        defaultKeybindings: [{ alt: true, key: "2" }],
        id: "view.fleet",
        title: "Open Fleet",
      }),
    ], {
      getKeybindingOverrides: () => overrides,
    })
    let changes = 0
    const section = createKhalaCodeCommandKeybindingsSection({
      getOverrides: () => overrides,
      onChanged: () => {
        changes += 1
      },
      registry: () => registry,
      resetAll: () => {
        for (const key of Object.keys(overrides)) delete overrides[key as KhalaCodeCommandId]
      },
      setOverride: (id, value) => {
        if (value === null) delete overrides[id]
        else overrides[id] = value
      },
    })

    const container = document.createElement("div")
    container.append(section.render())
    document.body.append(container)

    const chatCapture = container.querySelector<HTMLButtonElement>(
      '[data-command-keybinding-capture="view.chat"]',
    )
    chatCapture?.click()
    chatCapture?.dispatchEvent(new window.KeyboardEvent("keydown", {
      bubbles: true,
      ctrlKey: true,
      key: "j",
    }))
    expect(overrides["view.chat"]).toBe("ctrl+j")
    expect(changes).toBe(1)

    const fleetCapture = container.querySelector<HTMLButtonElement>(
      '[data-command-keybinding-capture="view.fleet"]',
    )
    fleetCapture?.click()
    fleetCapture?.dispatchEvent(new window.KeyboardEvent("keydown", {
      bubbles: true,
      ctrlKey: true,
      key: "j",
    }))
    expect(overrides["view.fleet"]).toBeUndefined()
    expect(container.textContent).toContain("conflicts with Open Chat")
    expect(container.querySelector('[data-command-id="view.chat"]')?.getAttribute("data-conflict")).toBe("true")
    expect(container.querySelector('[data-command-id="view.fleet"]')?.getAttribute("data-conflict")).toBe("true")

    container
      .querySelector<HTMLElement>('[data-command-id="view.chat"] .khala-keybindings-action')
      ?.dispatchEvent(new window.MouseEvent("click", { bubbles: true }))
    expect(overrides["view.chat"]).toBe("none")

    container
      .querySelectorAll<HTMLElement>('[data-command-id="view.chat"] .khala-keybindings-action')[1]
      ?.dispatchEvent(new window.MouseEvent("click", { bubbles: true }))
    expect(overrides["view.chat"]).toBeUndefined()

    overrides["view.chat"] = "ctrl+j"
    container.replaceChildren(section.render())
    container.querySelector<HTMLButtonElement>(".khala-keybindings-toolbar .khala-settings-refresh")?.click()
    expect(overrides).toEqual({})
  })
})
