import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeSessionActionProjection } from "../src/shared/session-actions"
import { mountKhalaCodeSessionActionsSettingsSection } from "../src/ui/session-actions-settings-section"

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

const projection: KhalaCodeSessionActionProjection = {
  activeThreadId: "thread-1",
  activeThreadTitle: "Audit thread",
  closedTabs: [],
  messageCount: 2,
  sessionCount: 3,
  intents: [
    {
      action: "fork",
      commandId: "session.fork",
      enabled: true,
      reason: "Fork through the active Codex-compatible runtime",
      runtimeBoundary: "codex_app_server",
      threadId: "thread-1",
    },
    {
      action: "share",
      commandId: "session.share",
      enabled: false,
      reason: "Sharing requires an explicit safe backing path; private local transcripts and files stay local.",
      runtimeBoundary: "khala_owned_server",
      threadId: "thread-1",
    },
    {
      action: "archive",
      commandId: "session.archive",
      enabled: true,
      reason: "Archive the active session",
      runtimeBoundary: "codex_app_server",
      threadId: "thread-1",
    },
  ],
}

describe("Khala Code session actions settings section", () => {
  test("renders enabled actions, disabled share state, and runs commands", async () => {
    const { cleanup, container } = installDom()
    const actions: string[] = []

    try {
      const section = mountKhalaCodeSessionActionsSettingsSection({
        fetch: async () => projection,
        runAction: async action => {
          actions.push(action)
          return { ok: true, message: `${action} done` }
        },
      })
      container.append(section.render())
      await section.refresh()

      expect(container.textContent).toContain("Session Actions")
      expect(container.textContent).toContain("Audit thread")
      expect(container.textContent).toContain("session.fork")
      expect(container.querySelector('[data-action="share"]')?.getAttribute("data-enabled")).toBe("false")
      expect(container.textContent).toContain("private local transcripts")

      const forkButton = container.querySelector<HTMLButtonElement>('[data-action="fork"] button')
      expect(forkButton?.disabled).toBe(false)
      forkButton?.click()
      await new Promise(resolve => setTimeout(resolve, 0))

      expect(actions).toEqual(["fork"])
      expect(container.textContent).toContain("fork done")
    } finally {
      cleanup()
    }
  })
})
