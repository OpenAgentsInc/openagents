import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeDesktopClaudeSettingsProjection } from "../src/shared/claude-settings"
import { mountClaudeSettingsSection } from "../src/ui/claude-settings-panel"

const installDom = (): {
  readonly container: HTMLElement
  readonly cleanup: () => void
} => {
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

// Oracle for khala_code.settings.no_bare_unset_labels.v1
describe("Claude settings section", () => {
  test("renders unset read-only account fields as 'Default', never the bare word 'Unset'", async () => {
    const { cleanup, container } = installDom()
    const settings: KhalaCodeDesktopClaudeSettingsProjection = {
      ok: true,
      observedAt: "2026-07-03T00:00:00.000Z",
      errors: [],
      account: {
        apiProvider: null,
        apiKeySource: null,
        email: null,
        organization: null,
        subscriptionType: null,
        tokenSource: null,
      },
      init: {
        permissionMode: null,
        model: "claude-live",
        system: null,
      },
      models: {
        options: [],
        selected: null,
      },
    }

    try {
      const section = mountClaudeSettingsSection(container, { fetch: async () => settings })
      await section.refresh()

      const metricValues = Array.from(container.querySelectorAll(".khala-settings-metric-value"))
        .map(node => node.textContent)
      expect(metricValues).not.toContain("Unset")
      expect(metricValues).toContain("Default")
    } finally {
      cleanup()
    }
  })
})
