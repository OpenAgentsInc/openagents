import { afterEach, beforeEach, describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { mountKhalaCodeCommandPalette } from "../src/ui/command-palette"
import {
  createKhalaCodeCommandRegistry,
  type KhalaCodeCommandDefinition,
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

describe("Khala Code command palette", () => {
  test("opens, searches, navigates, executes, and renders disabled and empty states", () => {
    const executed: string[] = []
    const registry = createKhalaCodeCommandRegistry([
      command({
        defaultKeybindings: [{ alt: true, key: "1" }],
        execute: () => {
          executed.push("view.chat")
        },
        id: "view.chat",
        title: "Open Chat",
      }),
      command({
        defaultKeybindings: [{ alt: true, key: "2" }],
        execute: () => {
          executed.push("view.fleet")
        },
        id: "view.fleet",
        title: "Open Fleet",
      }),
      command({
        available: () => false,
        disabledReason: () => "No active turn is running",
        id: "composer.stop_turn",
        title: "Stop Active Turn",
      }),
    ])
    const container = document.createElement("div")
    document.body.append(container)
    const palette = mountKhalaCodeCommandPalette(container, {
      getRecords: () => [
        {
          group: "provider",
          id: "provider:openai",
          kind: "provider",
          metadataRef: "test.provider.openai",
          subtitle: "Selected provider",
          title: "OpenAI",
        },
      ],
      onExecute: result => {
        if (result.kind !== "command") return
        void registry.execute(result.id as "view.chat" | "view.fleet")
      },
      registry,
    })

    palette.open()
    expect(container.querySelector(".khala-code-command-palette")?.hasAttribute("hidden")).toBe(false)
    expect(container.querySelectorAll(".khala-code-command-palette-result")).toHaveLength(4)
    expect(container.textContent).toContain("Providers")

    palette.setQuery("fleet")
    expect(palette.selectedResultId()).toBe("view.fleet")
    const input = container.querySelector<HTMLInputElement>(".khala-code-command-palette-input")
    if (input === null) throw new Error("missing command palette input")
    input.dispatchEvent(new window.KeyboardEvent("keydown", { bubbles: true, key: "Enter" }))
    expect(executed).toEqual(["view.fleet"])
    expect(palette.isOpen()).toBe(false)

    palette.open("stop")
    const disabled = container.querySelector<HTMLButtonElement>(".khala-code-command-palette-result")
    expect(disabled?.disabled).toBe(true)
    expect(disabled?.textContent).toContain("No active turn is running")

    palette.setQuery("no such command")
    expect(container.textContent).toContain("No matching commands")
  })

  test("renders loading state while async command sources refresh", () => {
    const registry = createKhalaCodeCommandRegistry([
      command({ id: "view.chat", title: "Open Chat" }),
    ])
    const container = document.createElement("div")
    document.body.append(container)
    const palette = mountKhalaCodeCommandPalette(container, {
      getLoading: () => true,
      registry,
    })

    palette.open()

    expect(container.textContent).toContain("Loading commands")
  })
})
