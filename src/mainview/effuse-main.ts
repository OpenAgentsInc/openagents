/**
 * Effuse Mainview Entry Point
 *
 * Mounts all Effuse widgets and provides the runtime layer.
 * This replaces the monolithic index.ts with a clean Effect-based architecture.
 */

import { Effect, Layer, Stream } from "effect"
import { getSocketClient } from "./socket-client.js"
import {
  mountWidgetById,
  DomServiceLive,
  StateServiceLive,
  SocketServiceFromClient,
  // TBCC Widgets
  TBCCShellWidget,
  TBCCDashboardWidget,
  TBCCTaskBrowserWidget,
  TBCCRunBrowserWidget,
  TBCCSettingsWidget,
  // Streaming output widget (fixed overlay)
  TBOutputWidget,
} from "../effuse/index.js"

console.log("[Effuse] Loading mainview...")
if ((window as any).bunLog) {
  (window as any).bunLog("[Effuse] ========== EFFUSE-MAIN.TS IS EXECUTING ==========")
}

// Add visible error display
const showError = (msg: string) => {
  document.body.innerHTML = `<div style="padding:20px;color:red;font-family:monospace;background:#1a1a1a;">
    <h2>Effuse Error</h2>
    <pre>${msg}</pre>
  </div>`
}

// Global error handler
window.onerror = (msg, src, line, col, error) => {
  console.error("[Effuse] Global error:", msg, src, line, col, error)
  showError(`${msg}\n\nSource: ${src}:${line}:${col}\n\n${error?.stack || ""}`)
  return false
}

window.onunhandledrejection = (event) => {
  // Ignore empty Error objects (webview-bun internal artifacts)
  if (
    event.reason &&
    event.reason.constructor?.name === "Error" &&
    Object.keys(event.reason).length === 0
  ) {
    // Silently ignore - these are harmless webview-bun artifacts
    event.preventDefault()
    return
  }

  // Log actual errors
  console.error("[Effuse] Unhandled rejection:", event.reason)
  showError(`Unhandled Promise rejection:\n\n${event.reason?.stack || event.reason}`)
}

// ============================================================================
// Layer Setup
// ============================================================================

/**
 * Create the Effuse runtime layer with the socket client
 */
const createEffuseLayer = () => {
  const socketClient = getSocketClient()

  return Layer.mergeAll(
    DomServiceLive,
    StateServiceLive,
    SocketServiceFromClient(socketClient)
  )
}

// ============================================================================
// Widget Mounting
// ============================================================================

/**
 * Mount all widgets to their respective containers
 */
const mountAllWidgets = Effect.gen(function* () {
  console.log("[Effuse] Mounting TB Command Center...")
  if ((window as any).bunLog) {
    (window as any).bunLog("[Effuse] ========== MOUNTING TBCC WIDGETS ==========")
  }

  // 1. Mount Shell
  const shellWidget = yield* mountWidgetById(TBCCShellWidget, "tbcc-shell-widget").pipe(
    Effect.tap(() => console.log("[Effuse] Shell mounted")),
    Effect.catchAll((e) => {
      console.error("[Effuse] Failed to mount Shell widget:", e)
      return Effect.die(e)
    })
  )

  // 2. Mount Child Widgets
  const dashboardWidget = yield* mountWidgetById(TBCCDashboardWidget, "tbcc-tab-dashboard")
  const taskBrowserWidget = yield* mountWidgetById(TBCCTaskBrowserWidget, "tbcc-tab-tasks")
  const runBrowserWidget = yield* mountWidgetById(TBCCRunBrowserWidget, "tbcc-tab-runs")
  const settingsWidget = yield* mountWidgetById(TBCCSettingsWidget, "tbcc-tab-settings")

  // Mark as used (widgets are mounted but not directly referenced)
  void taskBrowserWidget
  void settingsWidget

  console.log("[Effuse] Child widgets mounted")

  // 3. Wire up events

  // Dashboard "View Run" -> Switch to Runs tab & Select Run
  yield* Stream.runForEach(dashboardWidget.events, (event) =>
    Effect.gen(function* () {
      if (event.type === "viewRun") {
        // Switch to Runs tab
        yield* shellWidget.emit({ type: "changeTab", tab: "runs" })
        // Select the run in Run Browser
        // We assume local source for now if coming from dashboard recent runs,
        // but dashboard should probably pass source too.
        // For now, let's try local first.
        yield* runBrowserWidget.emit({ type: "selectRun", runId: event.runId, source: "local" })
      }
    })
  ).pipe(Effect.forkScoped)

  console.log("[Effuse] TB Command Center ready")
})

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize the Effuse mainview
 *
 * We mount widgets and then keep the program alive with Effect.never
 * so the scope stays open and event handlers keep running.
 */
const initEffuse = () => {
  console.log("[Effuse] Creating layer...")
  if ((window as any).bunLog) {
    (window as any).bunLog("[Effuse] ========== INIT EFFUSE CALLED ==========")
  }

  // Set up HMR reload handler (before widget mounting)
  const socketClient = getSocketClient()
  socketClient.onMessage((message) => {
    if (message.type === "dev_reload") {
      console.log("[Effuse] HMR: Reload triggered by", (message as any).changedFile)
      if ((window as any).bunLog) {
        (window as any).bunLog(`[Effuse] HMR: Reloading due to ${(message as any).changedFile}`)
      }
      location.reload()
    }
  })
  console.log("[Effuse] HMR handler registered")

  let layer
  try {
    layer = createEffuseLayer()
    console.log("[Effuse] Layer created")
  } catch (e) {
    console.error("[Effuse] Failed to create layer:", e)
    return
  }

  // Mount widgets then wait forever (keeps scope open for event handlers)
  const program = Effect.gen(function* () {
    yield* mountAllWidgets
    console.log("[Effuse] Widgets mounted, keeping scope alive...")
    // Never complete - keeps the scope open so forked fibers keep running
    yield* Effect.never
  })

  console.log("[Effuse] Starting Effect runtime...")

  Effect.runFork(
    program.pipe(
      Effect.provide(layer),
      Effect.scoped,
      Effect.catchAllDefect((defect) => {
        console.error("[Effuse] Defect caught:", defect)
        console.error("[Effuse] Defect type:", typeof defect)
        console.error("[Effuse] Defect constructor:", defect?.constructor?.name)
        if (defect instanceof Error) {
          console.error("[Effuse] Error message:", defect.message)
          console.error("[Effuse] Error stack:", defect.stack)
        }
        try {
          console.error("[Effuse] Defect stringified:", JSON.stringify(defect, null, 2))
        } catch {
          console.error("[Effuse] Could not stringify defect")
        }
        return Effect.void
      })
    )
  )

  console.log("[Effuse] Mainview initialized")
}

// Initialize when DOM is ready
if ((window as any).bunLog) {
  (window as any).bunLog(`[Effuse] document.readyState = ${document.readyState}`)
}

if (document.readyState === "loading") {
  if ((window as any).bunLog) {
    (window as any).bunLog("[Effuse] Waiting for DOMContentLoaded...")
  }
  document.addEventListener("DOMContentLoaded", () => {
    if ((window as any).bunLog) {
      (window as any).bunLog("[Effuse] DOMContentLoaded fired!")
    }
    try {
      initEffuse()
    } catch (e) {
      console.error("[Effuse] Init error:", e)
      if ((window as any).bunLog) {
        (window as any).bunLog(`[Effuse] Init error: ${e}`)
      }
    }
  })
} else {
  if ((window as any).bunLog) {
    (window as any).bunLog("[Effuse] DOM already ready, initializing immediately")
  }
  try {
    initEffuse()
  } catch (e) {
    console.error("[Effuse] Init error:", e)
    if ((window as any).bunLog) {
      (window as any).bunLog(`[Effuse] Init error: ${e}`)
    }
  }
}

// Export for debugging
export { initEffuse }
