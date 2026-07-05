import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeLocalServerContractProjection } from "../src/shared/local-server-runtime"
import { mountKhalaCodeLocalServerManagerSettingsSection } from "../src/ui/local-server-manager-settings-section"

const installDom = (): { readonly container: HTMLElement; readonly cleanup: () => void } => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousHTMLElement = globalThis.HTMLElement

  Object.defineProperty(globalThis, "window", { configurable: true, value: window })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "HTMLElement", { configurable: true, value: window.HTMLElement })

  const container = window.document.createElement("section")
  window.document.body.append(container)

  return {
    container: container as unknown as HTMLElement,
    cleanup: () => {
      Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow })
      Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
      Object.defineProperty(globalThis, "HTMLElement", { configurable: true, value: previousHTMLElement })
      window.close()
    },
  }
}

const projection: KhalaCodeLocalServerContractProjection = {
  actions: [
    {
      commandId: "server.refresh",
      enabled: true,
      label: "Refresh Health",
      reason: "Reload runtime status.",
    },
    {
      commandId: "server.restart_local",
      enabled: false,
      label: "Restart Local Server",
      reason: "Lifecycle controller is not wired.",
    },
  ],
  capabilities: [
    { id: "health", label: "Health and readiness", required: true },
    { id: "stream_events", label: "AI SDK/OpenAgents stream events", required: true },
  ],
  credentialPolicy: "Remote server credentials stay out of renderer logs.",
  defaultRuntime: "khala_local_server",
  ownershipBoundary: "Khala owns the local server contract; Codex app-server remains a bridge.",
  rows: [
    {
      detail: "Contract defined.",
      isDefault: true,
      kind: "khala_local_server",
      label: "Khala Local Server",
      reason: "First pass.",
      state: "planned",
    },
    {
      detail: "Pylon ready.",
      isDefault: false,
      kind: "pylon",
      label: "Pylon Runtime",
      reason: "Candidate host.",
      state: "ready",
    },
  ],
}

describe("Khala Code local server manager settings section", () => {
  test("renders runtime rows, capabilities, and disabled lifecycle action", async () => {
    const { cleanup, container } = installDom()
    const actions: string[] = []

    try {
      const section = mountKhalaCodeLocalServerManagerSettingsSection({
        fetch: async () => projection,
        runAction: async id => {
          actions.push(id)
        },
      })
      container.append(section.render())
      await section.refresh()

      expect(container.textContent).toContain("Local Server Runtime")
      expect(container.textContent).toContain("Khala Local Server / default")
      expect(container.textContent).toContain("Pylon ready")
      expect(container.textContent).toContain("AI SDK/OpenAgents stream events")
      expect(container.textContent).toContain("Remote server credentials stay out")
      expect(container.querySelector('.khala-local-server-row[data-state="planned"]')).not.toBeNull()

      container.querySelector<HTMLButtonElement>('[data-command-id="server.refresh"]')?.click()
      await Promise.resolve()
      expect(actions).toEqual(["server.refresh"])

      const restart = container.querySelector<HTMLButtonElement>('[data-command-id="server.restart_local"]')
      expect(restart?.disabled).toBe(true)
    } finally {
      cleanup()
    }
  })
})
