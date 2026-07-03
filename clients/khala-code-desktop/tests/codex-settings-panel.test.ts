import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import { mountCodexSettingsPanel } from "../src/ui/codex-settings-panel"

const setGlobal = (key: string, value: unknown): void => {
  Object.defineProperty(globalThis, key, {
    configurable: true,
    value,
    writable: true,
  })
}

const installDom = (): {
  readonly container: HTMLElement
  readonly cleanup: () => void
  readonly window: Window
} => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousHTMLElement = globalThis.HTMLElement

  setGlobal("window", window)
  setGlobal("document", window.document)
  setGlobal("HTMLElement", window.HTMLElement)

  const container = window.document.createElement("section")
  window.document.body.append(container)

  return {
    container: container as unknown as HTMLElement,
    window,
    cleanup: () => {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      Object.defineProperty(globalThis, "HTMLElement", { configurable: true, value: previousHTMLElement })
      window.close()
    },
  }
}

describe("Codex settings panel", () => {
  test("does not expose hidden Codex models as selectable chat models", async () => {
    const { cleanup, container, window } = installDom()
    const writes: unknown[] = []
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model: "gpt-5.5-codex",
        },
      },
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            displayName: "GPT-5.5",
            hidden: false,
          },
          {
            id: "codex-auto-review",
            model: "codex-auto-review",
            displayName: "Codex Auto Review",
            hidden: true,
          },
          {
            id: "gpt-5.4-mini",
            model: "gpt-5.4-mini",
            displayName: "GPT-5.4-Mini",
            hidden: false,
          },
        ],
      },
    })

    try {
      const panel = mountCodexSettingsPanel(container, {
        fetch: async () => settings,
        write: async request => {
          writes.push(request)
          return { ok: true, settings }
        },
      })

      await panel.refresh()

      const modelSelect = container.querySelector("select")
      expect(modelSelect).not.toBeNull()
      const options = Array.from(modelSelect?.options ?? [])

      expect(options.map(option => option.textContent)).toEqual(["GPT-5.5", "GPT-5.4-Mini"])
      expect(options.some(option => option.value === "codex-auto-review")).toBe(false)
      expect(container.textContent).not.toContain("Codex Auto Review")
      expect(container.textContent).not.toContain("(hidden)")

      if (modelSelect !== null) {
        modelSelect.value = "gpt-5.4-mini"
        modelSelect.dispatchEvent(new window.Event("change") as unknown as Event)
      }

      await Promise.resolve()

      expect(writes).toEqual([{ keyPath: "model", value: "gpt-5.4-mini" }])
    } finally {
      cleanup()
    }
  })
})
