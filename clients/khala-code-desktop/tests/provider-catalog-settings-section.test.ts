import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import { mountKhalaCodeProviderCatalogSettingsSection } from "../src/ui/provider-catalog-settings-section"

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

const click = async (button: HTMLButtonElement): Promise<void> => {
  button.click()
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}

describe("Khala Code provider catalog settings section", () => {
  test("renders provider states and writes composer provider selection through model_provider", async () => {
    const { cleanup, container } = installDom()
    const writes: (string | null)[] = []
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model_provider: "openai",
        },
      },
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            provider: "openai",
            providerDisplayName: "OpenAI",
          },
          {
            id: "ollama/qwen",
            model: "ollama/qwen",
            provider: "ollama",
            providerDisplayName: "Ollama",
          },
        ],
      },
    })

    try {
      const section = mountKhalaCodeProviderCatalogSettingsSection({
        fetch: async () => settings,
        writeModelProvider: async providerId => {
          writes.push(providerId)
          return { ok: true, settings }
        },
      })
      container.append(section.render())
      await section.refresh()

      const rows = Array.from(container.querySelectorAll(".khala-provider-catalog-row"))
      expect(rows.map(row => [row.getAttribute("data-provider-id"), row.getAttribute("data-state")])).toEqual([
        ["openai", "connected"],
        ["ollama", "env_configured"],
      ])

      const ollamaConnect = rows[1]?.querySelector("button") as HTMLButtonElement | null
      expect(ollamaConnect?.textContent).toBe("Connect")
      await click(ollamaConnect!)
      expect(writes).toEqual(["ollama"])
      expect(container.textContent).toContain("Selected Ollama.")
    } finally {
      cleanup()
    }
  })

  test("validates custom OpenAI-compatible provider form without rendering credential values", async () => {
    const { cleanup, container, window } = installDom()
    const settings = projectKhalaCodeDesktopCodexSettings({
      modelList: { data: [] },
    })

    try {
      const section = mountKhalaCodeProviderCatalogSettingsSection({
        fetch: async () => settings,
        writeModelProvider: async () => ({ ok: true, settings }),
      })
      container.append(section.render())
      await section.refresh()

      const input = (name: string): HTMLInputElement =>
        container.querySelector(`input[name="${name}"]`) as HTMLInputElement
      input("custom-provider-id").value = "Private_Local"
      input("custom-provider-id").dispatchEvent(new window.Event("input") as unknown as Event)
      input("custom-provider-name").value = "Private Local"
      input("custom-provider-name").dispatchEvent(new window.Event("input") as unknown as Event)
      input("custom-provider-base-url").value = "http://user:sk-renderer-secret@localhost:8080/v1"
      input("custom-provider-base-url").dispatchEvent(new window.Event("input") as unknown as Event)
      input("custom-provider-models").value = "qwen, llama"
      input("custom-provider-models").dispatchEvent(new window.Event("input") as unknown as Event)

      const form = container.querySelector(".khala-provider-custom-form") as HTMLFormElement
      form.dispatchEvent(new window.Event("submit") as unknown as Event)
      await Promise.resolve()

      expect(container.textContent).toContain("Private Local")
      expect(container.textContent).toContain("private_local")
      expect(container.textContent).not.toContain("sk-renderer-secret")
      expect(container.textContent).toContain("API key is not stored in the renderer")
    } finally {
      cleanup()
    }
  })
})
