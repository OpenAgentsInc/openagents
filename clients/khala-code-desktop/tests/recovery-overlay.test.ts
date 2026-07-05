import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"

import {
  khalaCodeDesktopLoadFailureState,
  khalaCodeDesktopUnresponsiveState,
  type KhalaCodeDesktopRecoveryActionDispatch,
} from "../src/shared/recovery-state"

type DomHarness = Readonly<{
  cleanup: () => Promise<void>
  container: HTMLElement
}>

const withDom = async (
  run: (harness: DomHarness) => Promise<void>,
): Promise<void> => {
  const window = new Window()
  const previousWindow = globalThis.window
  const previousDocument = globalThis.document
  const previousNavigator = globalThis.navigator
  Object.defineProperty(globalThis, "window", { configurable: true, value: window, writable: true })
  Object.defineProperty(globalThis, "document", { configurable: true, value: window.document })
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: window.navigator })
  const container = document.createElement("div")
  document.body.append(container)
  try {
    await run({
      cleanup: async () => {
        await new Promise(resolve => setTimeout(resolve, 0))
      },
      container,
    })
  } finally {
    await new Promise(resolve => setTimeout(resolve, 0))
    Object.defineProperty(globalThis, "window", { configurable: true, value: previousWindow, writable: true })
    Object.defineProperty(globalThis, "document", { configurable: true, value: previousDocument })
    Object.defineProperty(globalThis, "navigator", { configurable: true, value: previousNavigator })
    window.close()
  }
}

const recordingDispatch = (): {
  readonly calls: string[]
  readonly dispatch: KhalaCodeDesktopRecoveryActionDispatch
} => {
  const calls: string[] = []
  return {
    calls,
    dispatch: {
      exportDebugLogs: async () => {
        calls.push("exportDebugLogs")
        return { path: "/tmp/khala-code-debug-logs.zip" }
      },
      quit: async () => {
        calls.push("quit")
      },
      relaunch: async () => {
        calls.push("relaunch")
      },
    },
  }
}

// Oracle for khala_code.diagnostics.debug_log_export_public_safe_and_recovery_visible.v1
describe("mountKhalaCodeRecoveryOverlay", () => {
  test("renders nothing when the recovery state is none", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      mountKhalaCodeRecoveryOverlay(container, { dispatch })
      expect(container.querySelector("[data-khala-code-recovery-overlay]")).toBeNull()
      await cleanup()
    })
  })

  test("shows the unresponsive overlay with relaunch/export/keep-waiting/quit actions", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, { dispatch })
      overlay.show(khalaCodeDesktopUnresponsiveState("no heartbeat for 16s", "2026-07-05T00:00:00.000Z"))

      const root = container.querySelector("[data-khala-code-recovery-overlay]")
      expect(root).not.toBeNull()
      expect(root?.getAttribute("data-khala-code-recovery-kind")).toBe("unresponsive")
      const actionButtons = [...container.querySelectorAll("[data-khala-code-recovery-action]")]
      expect(actionButtons.map(button => button.getAttribute("data-khala-code-recovery-action"))).toEqual([
        "relaunch",
        "export_logs",
        "keep_waiting",
        "quit",
      ])
      await cleanup()
    })
  })

  test("shows the load_failure overlay without a keep-waiting choice", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, { dispatch })
      overlay.show(khalaCodeDesktopLoadFailureState("bundled view failed to load", "2026-07-05T00:00:00.000Z"))

      const actionButtons = [...container.querySelectorAll("[data-khala-code-recovery-action]")]
      expect(actionButtons.map(button => button.getAttribute("data-khala-code-recovery-action"))).toEqual([
        "relaunch",
        "export_logs",
        "quit",
      ])
      await cleanup()
    })
  })

  test("clicking export_logs calls the dispatch and renders the exported path without dismissing", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { calls, dispatch } = recordingDispatch()
      const exported: string[] = []
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, {
        dispatch,
        onExported: path => exported.push(path),
      })
      overlay.show(khalaCodeDesktopUnresponsiveState("no heartbeat", "2026-07-05T00:00:00.000Z"))

      const exportButton = container.querySelector<HTMLButtonElement>(
        '[data-khala-code-recovery-action="export_logs"]',
      )
      exportButton?.click()
      await new Promise(resolve => setTimeout(resolve, 0))

      expect(calls).toEqual(["exportDebugLogs"])
      expect(exported).toEqual(["/tmp/khala-code-debug-logs.zip"])
      expect(
        container.querySelector("[data-khala-code-recovery-exported-path]")?.textContent,
      ).toContain("/tmp/khala-code-debug-logs.zip")
      // Overlay remains visible — exporting logs is not a dismissal action.
      expect(container.querySelector("[data-khala-code-recovery-overlay]")).not.toBeNull()
      await cleanup()
    })
  })

  test("clicking keep_waiting dismisses the overlay without calling relaunch or quit", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { calls, dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, { dispatch })
      overlay.show(khalaCodeDesktopUnresponsiveState("no heartbeat", "2026-07-05T00:00:00.000Z"))

      const keepWaitingButton = container.querySelector<HTMLButtonElement>(
        '[data-khala-code-recovery-action="keep_waiting"]',
      )
      keepWaitingButton?.click()
      await new Promise(resolve => setTimeout(resolve, 0))

      expect(calls).toEqual([])
      expect(container.querySelector("[data-khala-code-recovery-overlay]")).toBeNull()
      await cleanup()
    })
  })

  test("clicking relaunch calls dispatch.relaunch exactly once", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { calls, dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, { dispatch })
      overlay.show(khalaCodeDesktopUnresponsiveState("no heartbeat", "2026-07-05T00:00:00.000Z"))

      container
        .querySelector<HTMLButtonElement>('[data-khala-code-recovery-action="relaunch"]')
        ?.click()
      await new Promise(resolve => setTimeout(resolve, 0))

      expect(calls).toEqual(["relaunch"])
      await cleanup()
    })
  })

  test("clicking quit calls dispatch.quit exactly once", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { calls, dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, { dispatch })
      overlay.show(khalaCodeDesktopUnresponsiveState("no heartbeat", "2026-07-05T00:00:00.000Z"))

      container.querySelector<HTMLButtonElement>('[data-khala-code-recovery-action="quit"]')?.click()
      await new Promise(resolve => setTimeout(resolve, 0))

      expect(calls).toEqual(["quit"])
      await cleanup()
    })
  })

  test("hide() clears the overlay back to the none state", async () => {
    await withDom(async ({ cleanup, container }) => {
      const { dispatch } = recordingDispatch()
      const { mountKhalaCodeRecoveryOverlay } = await import("../src/ui/recovery-overlay-react")
      const overlay = mountKhalaCodeRecoveryOverlay(container, { dispatch })
      overlay.show(khalaCodeDesktopUnresponsiveState("no heartbeat", "2026-07-05T00:00:00.000Z"))
      expect(container.querySelector("[data-khala-code-recovery-overlay]")).not.toBeNull()
      overlay.hide()
      expect(container.querySelector("[data-khala-code-recovery-overlay]")).toBeNull()
      await cleanup()
    })
  })
})
