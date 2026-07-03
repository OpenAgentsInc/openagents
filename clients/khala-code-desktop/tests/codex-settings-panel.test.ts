import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import { mountCodexSettingsPanel } from "../src/ui/codex-settings-panel"

describe("Codex settings panel", () => {
  test("filters hidden Codex models out of the primary model picker", async () => {
    const window = new Window()
    const previousWindow = globalThis.window
    const previousDocument = globalThis.document

    Object.defineProperty(globalThis, "window", { configurable: true, value: window })
    Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })

    try {
      const container = document.createElement("section")
      const settings = projectKhalaCodeDesktopCodexSettings({
        configRead: {
          config: {
            model: "gpt-5.5",
          },
        },
        modelList: {
          data: [
            {
              id: "gpt-5.5",
              model: "gpt-5.5",
              displayName: "GPT-5.5",
              hidden: false,
              isDefault: true,
            },
            {
              id: "codex-auto-review",
              model: "codex-auto-review",
              displayName: "Codex Auto Review",
              hidden: true,
              isDefault: false,
            },
          ],
        },
      })
      const writes: string[] = []
      const panel = mountCodexSettingsPanel(container, {
        fetch: async () => settings,
        write: async request => {
          writes.push(String(request.value))
          return { ok: true, settings }
        },
      })

      await panel.refresh()

      const modelSelect = container.querySelector<HTMLSelectElement>("select.khala-settings-select")
      expect(modelSelect).not.toBeNull()
      const labels = [...modelSelect!.options].map(option => option.textContent)
      const values = [...modelSelect!.options].map(option => option.value)

      expect(labels).toEqual(["GPT-5.5"])
      expect(values).toEqual(["gpt-5.5"])
      expect(container.textContent).not.toContain("Codex Auto Review")
      expect(container.textContent).not.toContain("(hidden)")
      expect(writes).toEqual([])
    } finally {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      window.close()
    }
  })
})
