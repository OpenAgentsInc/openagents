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

const selectForLabel = (
  container: HTMLElement,
  label: string,
): HTMLSelectElement => {
  const controls = Array.from(container.querySelectorAll(".khala-settings-control"))
  const control = controls.find(node =>
    node.querySelector(".khala-settings-control-label")?.textContent === label
  )
  const select = control?.querySelector("select")
  if (select === null || select === undefined) {
    throw new Error(`missing select for ${label}`)
  }
  return select as HTMLSelectElement
}

const changeSelect = async (
  select: HTMLSelectElement,
  value: string,
  window: Window,
): Promise<void> => {
  select.value = value
  select.dispatchEvent(new window.Event("change") as unknown as Event)
  await Promise.resolve()
  await Promise.resolve()
}

describe("Codex settings panel", () => {
  // Oracle for khala_code.settings.hidden_models_excluded_from_picker.v1
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

  // Oracle for khala_code.settings.no_bare_unset_labels.v1
  test("renders unset read-only config fields as 'Default', never the bare word 'Unset'", async () => {
    const { cleanup, container } = installDom()
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model: "gpt-5.5-codex",
        },
      },
      modelList: {
        data: [{
          id: "gpt-5.5-codex",
          model: "gpt-5.5-codex",
          displayName: "GPT-5.5",
          hidden: false,
        }],
      },
    })

    try {
      const panel = mountCodexSettingsPanel(container, {
        fetch: async () => settings,
        write: async () => ({ ok: true, settings }),
      })

      await panel.refresh()

      const metricValues = Array.from(container.querySelectorAll(".khala-settings-metric-value"))
        .map(node => node.textContent)
      expect(metricValues).not.toContain("Unset")
      expect(metricValues).toContain("Default")
    } finally {
      cleanup()
    }
  })

  // Oracle for the first #8254 slice: enum-backed Codex config metrics are
  // editable through the existing config/value/write RPC surface.
  test("writes enum-backed config selects through the Codex config-value RPC", async () => {
    const { cleanup, container, window } = installDom()
    const writes: unknown[] = []
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model: "gpt-5.5-codex",
          model_reasoning_summary: null,
          model_verbosity: null,
          approval_policy: null,
          sandbox_mode: null,
        },
      },
      modelList: {
        data: [{
          id: "gpt-5.5-codex",
          model: "gpt-5.5-codex",
          displayName: "GPT-5.5",
          hidden: false,
        }],
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

      await changeSelect(selectForLabel(container, "Summary"), "detailed", window)
      await changeSelect(selectForLabel(container, "Verbosity"), "high", window)
      await changeSelect(selectForLabel(container, "Approval"), "on-request", window)
      await changeSelect(selectForLabel(container, "Sandbox"), "workspace-write", window)
      await changeSelect(selectForLabel(container, "Summary"), "", window)

      expect(writes).toEqual([
        { keyPath: "model_reasoning_summary", value: "detailed" },
        { keyPath: "model_verbosity", value: "high" },
        { keyPath: "approval_policy", value: "on-request" },
        { keyPath: "sandbox_mode", value: "workspace-write" },
        { keyPath: "model_reasoning_summary", value: null },
      ])
    } finally {
      cleanup()
    }
  })

  // Oracle for khala_code.settings.editable_not_env_var_only.v1: provider
  // editability uses provider options sourced from model/list, never free text.
  test("writes the model provider select through the Codex config-value RPC", async () => {
    const { cleanup, container, window } = installDom()
    const writes: unknown[] = []
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          model: "gpt-5.5-codex",
          model_provider: "openai",
        },
      },
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            displayName: "GPT-5.5",
            provider: "openai",
            providerDisplayName: "OpenAI",
            hidden: false,
          },
          {
            id: "openrouter/sonoma-sky",
            model: "openrouter/sonoma-sky",
            displayName: "Sonoma Sky",
            provider: "openrouter",
            providerDisplayName: "OpenRouter",
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

      const providerSelect = selectForLabel(container, "Provider")
      expect(providerSelect.disabled).toBe(false)
      expect(providerSelect.name).toBe("model_provider")
      expect(Array.from(providerSelect.options).map(option => option.textContent)).toEqual([
        "Default",
        "OpenAI",
        "OpenRouter",
      ])

      await changeSelect(providerSelect, "openrouter", window)
      await changeSelect(providerSelect, "", window)

      expect(writes).toEqual([
        { keyPath: "model_provider", value: "openrouter" },
        { keyPath: "model_provider", value: null },
      ])
    } finally {
      cleanup()
    }
  })
})
