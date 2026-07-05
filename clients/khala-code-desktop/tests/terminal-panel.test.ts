import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import type { KhalaCodeTerminalWorkbenchProjection } from "../src/shared/terminal-workbench"
import { mountKhalaCodeTerminalPanel } from "../src/ui/terminal-panel"

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

const projection: KhalaCodeTerminalWorkbenchProjection = {
  activeProcessId: "proc-1",
  activeThreadId: "thread-1",
  boundary: "active_thread",
  tabs: [
    {
      command: "bun test",
      cwd: "/work/openagents",
      outputPreview: "pass",
      processId: "proc-1",
      status: "running",
      title: "bun test",
    },
    {
      command: "git status",
      cwd: "/work/openagents",
      outputPreview: "clean",
      processId: "proc-2",
      status: "exited",
      title: "git status",
    },
  ],
  transport: "codex_background_terminal",
}

const flush = async (): Promise<void> => {
  await Promise.resolve()
  await Promise.resolve()
  await Promise.resolve()
}

describe("Khala Code terminal panel", () => {
  test("renders tabs and routes copy, clean, and terminate actions", async () => {
    const { cleanup, container } = installDom()
    const copied: string[] = []
    const terminated: string[] = []
    let cleaned = 0

    try {
      const panel = mountKhalaCodeTerminalPanel(container, {
        clean: async () => {
          cleaned += 1
        },
        copy: async text => {
          copied.push(text)
        },
        fetch: async () => projection,
        terminate: async processId => {
          terminated.push(processId)
        },
      })
      panel.setVisible(true)
      await panel.refresh()

      expect(container.textContent).toContain("Bound to active session thread-1")
      expect(container.querySelectorAll(".khala-terminal-tab")).toHaveLength(2)
      expect(container.querySelector<HTMLButtonElement>(".khala-terminal-panel-action")?.disabled).toBe(true)

      container.querySelector<HTMLButtonElement>(".khala-terminal-body .khala-terminal-panel-action")?.click()
      await flush()
      expect(copied).toEqual(["pass"])

      const terminate = container.querySelectorAll<HTMLButtonElement>(".khala-terminal-body .khala-terminal-panel-action")[1]
      terminate?.click()
      await flush()
      expect(terminated).toEqual(["proc-1"])

      const clean = [...container.querySelectorAll<HTMLButtonElement>(".khala-terminal-panel-header .khala-terminal-panel-action")]
        .find(button => button.textContent === "Clean Exited")
      clean?.click()
      await flush()
      expect(cleaned).toBe(1)

      container.querySelector<HTMLButtonElement>('[data-process-id="proc-2"]')?.click()
      expect(container.querySelector<HTMLElement>(".khala-terminal-body")?.dataset.processId).toBe("proc-2")
    } finally {
      cleanup()
    }
  })
})
