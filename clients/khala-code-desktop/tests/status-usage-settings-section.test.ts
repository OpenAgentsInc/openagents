import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeStatusUsageProjection } from "../src/shared/status-usage"
import { mountKhalaCodeStatusUsageSettingsSection } from "../src/ui/status-usage-settings-section"

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

describe("Khala Code status usage settings section", () => {
  test("renders timeline, usage, runtime, and provider error states", async () => {
    const { cleanup, container } = installDom()
    const projection: KhalaCodeStatusUsageProjection = {
      timeline: {
        anchorIds: ["m1", "m2"],
        assistantMessageCount: 1,
        estimatedVirtualizationUseful: false,
        messageCount: 2,
        toolCallCount: 1,
        userMessageCount: 1,
      },
      usage: {
        auditRows: 1,
        available: true,
        codexStateTokens: 0,
        leaderboardSyncedTokens: 20,
        missingUsageTurns: 1,
        pendingSyncTokens: 5,
        status: "needs_attention",
        totalTokens: 42,
        usageEventRows: 1,
      },
      runtime: {
        degradedCount: 1,
        readyCount: 1,
        unavailableCount: 0,
        rows: [{
          detail: "session catalog degraded",
          id: "boot:sessionCatalog",
          label: "sessionCatalog",
          retryable: true,
          state: "degraded",
        }],
      },
      errors: [{
        detail: "OpenAI API key missing",
        kind: "provider_auth",
        retryable: true,
        settingsEntryPoint: "provider",
        title: "Provider authentication required",
      }],
    }

    try {
      const section = mountKhalaCodeStatusUsageSettingsSection({
        fetch: async () => projection,
      })
      container.append(section.render())
      await section.refresh()

      expect(container.textContent).toContain("Status, Errors, Usage")
      expect(container.textContent).toContain("42")
      expect(container.textContent).toContain("session catalog degraded")
      expect(container.querySelector('.khala-status-usage-row[data-state="provider_auth"]')).not.toBeNull()
      expect(container.textContent).toContain("Provider authentication required")
    } finally {
      cleanup()
    }
  })
})
