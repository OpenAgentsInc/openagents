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
  // Widgets
  APMWidget,
  TrajectoryPaneWidget,
  ContainerPanesWidget,
  TBOutputWidget,
  MCTasksWidget,
  TBControlsWidget,
  CategoryTreeWidget,
  // HF Trajectory Browser Widgets
  HFTrajectoryListWidget,
  HFTrajectoryDetailWidget,
} from "../effuse/index.js"
import type { Trajectory } from "../atif/schema.js"

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
  console.log("[Effuse] Mounting HF Trajectory Browser widgets...")
  if ((window as any).bunLog) {
    (window as any).bunLog("[Effuse] ========== MOUNTING HF TRAJECTORY WIDGETS ==========")
  }

  // Mount HF Trajectory List Widget (sidebar)
  const listWidget = yield* mountWidgetById(HFTrajectoryListWidget, "hf-trajectory-list-widget").pipe(
    Effect.tap(() => console.log("[Effuse] HF Trajectory List widget mounted")),
    Effect.catchAll((e) => {
      console.error("[Effuse] Failed to mount HF Trajectory List widget:", e)
      return Effect.die(e)
    })
  )

  // Mount HF Trajectory Detail Widget (main area)
  const detailWidget = yield* mountWidgetById(HFTrajectoryDetailWidget, "hf-trajectory-detail-widget").pipe(
    Effect.tap(() => console.log("[Effuse] HF Trajectory Detail widget mounted")),
    Effect.catchAll((e) => {
      console.error("[Effuse] Failed to mount HF Trajectory Detail widget:", e)
      return Effect.die(e)
    })
  )

  // Wire up event forwarding: List selection -> Detail load
  // When user selects a trajectory in the list, fetch full trajectory via RPC and send to detail widget
  yield* Stream.runForEach(listWidget.events, (event) =>
    Effect.gen(function* () {
      if (event.type === "select") {
        console.log("[Effuse] Loading trajectory:", event.sessionId, "at index:", event.index)

        // Set detail widget to loading state
        yield* detailWidget.emit({ type: "clear" })

        try {
          // Fetch full trajectory via RPC using socket client
          const socketClient = getSocketClient()
          const trajectory = (yield* Effect.promise(() =>
            socketClient.getHFTrajectory(event.index)
          )) as Trajectory | null

          if (trajectory) {
            // Send to detail widget
            yield* detailWidget.emit({
              type: "load",
              sessionId: event.sessionId,
              trajectory,
            })
            console.log("[Effuse] Trajectory loaded successfully")
          } else {
            console.warn("[Effuse] Trajectory not found at index:", event.index)
          }
        } catch (error) {
          console.error("[Effuse] Failed to load trajectory:", error)
          // Detail widget will show error state
        }
      }
    })
  ).pipe(Effect.forkScoped)

  console.log("[Effuse] HF Trajectory Browser ready")

  /* OLD WIDGETS DISABLED FOR SIMPLE LAYOUT
  // Mount APM Widget (bottom-right corner)
  yield* mountWidgetById(APMWidget, "apm-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] APM widget container not found:", e)
      return Effect.void
    })
  )

  // Mount Trajectory Pane (left sidebar)
  yield* mountWidgetById(TrajectoryPaneWidget, "trajectory-pane-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] Trajectory pane container not found:", e)
      return Effect.void
    })
  )

  // Mount Container Panes (execution output grid)
  yield* mountWidgetById(ContainerPanesWidget, "container-panes-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] Container panes container not found:", e)
      return Effect.void
    })
  )

  // Mount TB Output (streaming output viewer)
  yield* mountWidgetById(TBOutputWidget, "tb-output-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] TB output container not found:", e)
      return Effect.void
    })
  )

  // Mount MC Tasks (ready tasks list)
  yield* mountWidgetById(MCTasksWidget, "mc-tasks-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] MC tasks container not found:", e)
      return Effect.void
    })
  )

  // Mount TB Controls (suite loading, run control)
  yield* mountWidgetById(TBControlsWidget, "tb-controls-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] TB controls container not found:", e)
      return Effect.void
    })
  )

  // Mount Category Tree (task categories)
  yield* mountWidgetById(CategoryTreeWidget, "category-tree-widget").pipe(
    Effect.catchAll((e) => {
      console.warn("[Effuse] Category tree container not found:", e)
      return Effect.void
    })
  )
  */
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
