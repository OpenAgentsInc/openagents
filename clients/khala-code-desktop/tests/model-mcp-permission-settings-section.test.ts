import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import { projectKhalaCodeDesktopCodexEcosystem } from "../src/shared/codex-ecosystem"
import { projectKhalaCodeDesktopCodexSettings } from "../src/shared/codex-settings"
import { mountKhalaCodeModelMcpPermissionSettingsSection } from "../src/ui/model-mcp-permission-settings-section"

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

describe("Khala Code model/MCP/permission settings section", () => {
  test("filters models and toggles local model visibility", async () => {
    const { cleanup, container, window } = installDom()
    const settings = projectKhalaCodeDesktopCodexSettings({
      modelList: {
        data: [
          {
            id: "gpt-5.5-codex",
            model: "gpt-5.5-codex",
            displayName: "GPT-5.5 Codex",
            provider: "openai",
            providerDisplayName: "OpenAI",
          },
          {
            id: "anthropic/opus",
            model: "anthropic/opus",
            displayName: "Opus",
            provider: "anthropic",
            providerDisplayName: "Anthropic",
          },
        ],
      },
    })

    try {
      const section = mountKhalaCodeModelMcpPermissionSettingsSection({
        fetchEcosystem: async () => projectKhalaCodeDesktopCodexEcosystem({}),
        fetchSettings: async () => settings,
        writePermissionProfile: async () => ({ ok: true, settings }),
      })
      container.append(section.render())
      await section.refresh()

      const search = container.querySelector('input[name="model-manager-search"]') as HTMLInputElement
      search.value = "opus"
      search.dispatchEvent(new window.Event("input") as unknown as Event)
      expect(Array.from(container.querySelectorAll(".khala-model-manager-row")).map(row => row.getAttribute("data-model-id"))).toEqual([
        "anthropic/opus",
      ])

      const hide = container.querySelector(".khala-model-manager-action") as HTMLButtonElement
      await click(hide)
      expect(container.querySelector(".khala-model-manager-row")?.getAttribute("data-visible")).toBe("false")
      expect(container.textContent).toContain("visibility hidden locally")
    } finally {
      cleanup()
    }
  })

  test("renders MCP intent and writes permission profile", async () => {
    const { cleanup, container, window } = installDom()
    const writes: string[] = []
    const settings = projectKhalaCodeDesktopCodexSettings({
      configRead: {
        config: {
          default_permissions: "read-only",
        },
      },
      permissionProfileList: {
        data: [
          { id: "read-only", description: "Read only", allowed: true },
          { id: "workspace-write", description: "Workspace write", allowed: true },
        ],
      },
    })
    const ecosystem = projectKhalaCodeDesktopCodexEcosystem({
      mcpServerStatusList: {
        data: [{
          name: "private_oauth",
          authStatus: "notLoggedIn",
          tools: { private_tool: { bearer: "raw-secret" } },
        }],
      },
    })

    try {
      const section = mountKhalaCodeModelMcpPermissionSettingsSection({
        fetchEcosystem: async () => ecosystem,
        fetchSettings: async () => settings,
        writePermissionProfile: async profileId => {
          writes.push(profileId)
          return { ok: true, settings }
        },
      })
      container.append(section.render())
      await section.refresh()

      expect(container.querySelector(".khala-mcp-manager-row")?.getAttribute("data-state")).toBe("needs_auth")
      const login = container.querySelector(".khala-mcp-manager-action") as HTMLButtonElement
      await click(login)
      expect(container.textContent).toContain("needs MCP OAuth/login")
      expect(container.textContent).not.toContain("raw-secret")

      const select = container.querySelector('select[name="permission-manager-profile"]') as HTMLSelectElement
      select.value = "workspace-write"
      select.dispatchEvent(new window.Event("change") as unknown as Event)
      await Promise.resolve()
      await Promise.resolve()
      expect(writes).toEqual(["workspace-write"])
    } finally {
      cleanup()
    }
  })
})
