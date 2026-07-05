import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeSupportProjection } from "../src/shared/support-entrypoints"
import { mountKhalaCodeSupportSettingsSection } from "../src/ui/support-settings-section"

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

const projection: KhalaCodeSupportProjection = {
  entries: [
    {
      id: "release_notes",
      label: "Release Notes",
      url: "https://github.com/OpenAgentsInc/openagents/releases/tag/test",
    },
    {
      id: "bug_report",
      label: "Bug Report",
      url: "https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml",
    },
  ],
  issueMetadata: "Khala Code support metadata\nprivateData=not_included",
}

describe("Khala Code support settings section", () => {
  test("renders support links, exports diagnostics, and copies issue metadata", async () => {
    const { cleanup, container } = installDom()
    const opened: string[] = []
    const copied: string[] = []
    let exported = 0

    try {
      const section = mountKhalaCodeSupportSettingsSection({
        copyIssueMetadata: async metadata => {
          copied.push(metadata)
        },
        exportDiagnostics: async () => {
          exported += 1
          return { ok: true, message: "Diagnostics exported (42 bytes)." }
        },
        fetch: async () => projection,
        open: async (_id, url) => {
          opened.push(url)
          return true
        },
      })

      container.append(section.render())
      await section.refresh()

      expect(container.textContent).toContain("Help And Support")
      expect(container.textContent).toContain("Release Notes")
      expect(container.textContent).toContain("privateData=not_included")

      container.querySelector<HTMLButtonElement>('[data-support-id="bug_report"]')?.click()
      await Promise.resolve()
      expect(opened).toEqual([
        "https://github.com/OpenAgentsInc/openagents/issues/new?template=strict-bug.yml",
      ])

      const actionButtons = container.querySelectorAll<HTMLButtonElement>(".khala-support-action")
      actionButtons[0]?.click()
      await Promise.resolve()
      expect(exported).toBe(1)
      expect(container.textContent).toContain("Diagnostics exported")

      actionButtons[1]?.click()
      await Promise.resolve()
      expect(copied).toEqual(["Khala Code support metadata\nprivateData=not_included"])
    } finally {
      cleanup()
    }
  })
})
