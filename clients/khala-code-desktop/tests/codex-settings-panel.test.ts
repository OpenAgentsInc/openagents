import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import { projectKhalaCodeDesktopCodexEcosystem } from "../src/shared/codex-ecosystem"
import { defaultKhalaCodeModelRoleRegistry } from "../src/shared/model-roles"
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

  // Oracle for the Promise.all landmine class documented in
  // docs/2026-07-05-promise-all-cron-landmine-audit.md: refreshSettings runs
  // three independent IPC fetches (settings, ecosystem, model roles). One
  // fetch rejecting must not hide the OTHER fetches' already-succeeded fresh
  // data behind a generic all-or-nothing error banner with stale state.
  test("keeps a sibling fetch's fresh data when one of the three settings-refresh IPC calls rejects", async () => {
    const { cleanup, container } = installDom()

    const settingsV1 = projectKhalaCodeDesktopCodexSettings({
      configRead: { config: { model: "gpt-5.5-codex" } },
      modelList: {
        data: [
          { id: "gpt-5.5-codex", model: "gpt-5.5-codex", displayName: "GPT-5.5", hidden: false },
        ],
      },
    })
    const settingsV2 = projectKhalaCodeDesktopCodexSettings({
      configRead: { config: { model: "gpt-5.4-mini" } },
      modelList: {
        data: [
          { id: "gpt-5.5-codex", model: "gpt-5.5-codex", displayName: "GPT-5.5", hidden: false },
          { id: "gpt-5.4-mini", model: "gpt-5.4-mini", displayName: "GPT-5.4-Mini", hidden: false },
        ],
      },
    })
    const ecosystemV1 = projectKhalaCodeDesktopCodexEcosystem({})
    const modelRolesV1 = defaultKhalaCodeModelRoleRegistry()

    let fetchCount = 0
    let ecosystemCount = 0
    let modelRolesCount = 0

    try {
      const panel = mountCodexSettingsPanel(container, {
        fetch: async () => {
          fetchCount += 1
          return fetchCount === 1 ? settingsV1 : settingsV2
        },
        fetchEcosystem: async () => {
          ecosystemCount += 1
          if (ecosystemCount === 1) return ecosystemV1
          throw new Error("ecosystem IPC channel closed")
        },
        fetchModelRoles: async () => {
          modelRolesCount += 1
          return { ok: true as const, path: "/tmp/model-roles.json", registry: modelRolesV1 }
        },
        write: async () => ({ ok: true, settings: settingsV1 }),
      })

      // First refresh: all three fetches succeed and establish a baseline.
      await panel.refresh()
      expect(selectForLabel(container, "Model").value).toBe("gpt-5.5-codex")
      expect(container.querySelector(".khala-settings-status")).toBeNull()

      // Second refresh: settings and model roles resolve with FRESH data,
      // but the ecosystem fetch rejects. The fresh settings data must still
      // render (not get discarded/hidden behind a generic error), and the
      // status banner must scope the error to the ecosystem group only.
      await panel.refresh()

      expect(selectForLabel(container, "Model").value).toBe("gpt-5.4-mini")
      expect(Array.from(selectForLabel(container, "Model").options).map(option => option.textContent))
        .toEqual(["GPT-5.5", "GPT-5.4-Mini"])

      const status = container.querySelector(".khala-settings-status")
      expect(status).not.toBeNull()
      expect(status?.textContent).toContain("Ecosystem refresh failed")
      expect(status?.textContent).toContain("ecosystem IPC channel closed")
      expect(status?.textContent).not.toContain("Settings refresh failed")
      expect(status?.textContent).not.toContain("Model roles refresh failed")

      // The ecosystem section keeps rendering the last successfully-fetched
      // ecosystem snapshot instead of blanking to "has not been loaded yet".
      expect(container.textContent).not.toContain("Ecosystem state has not been loaded yet.")
    } finally {
      cleanup()
    }
  })
})
