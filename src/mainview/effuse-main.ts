/**
 * Effuse Mainview Entry Point
 *
 * Mounts all Effuse widgets and provides the runtime layer.
 * This replaces the monolithic index.ts with a clean Effect-based architecture.
 */

import { Effect, Layer } from "effect"
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
} from "../effuse/index.js"

console.log("[Effuse] Loading mainview...")

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
  console.log("[Effuse] Mounting widgets...")

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

  console.log("[Effuse] All widgets mounted successfully")
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
  const layer = createEffuseLayer()

  // Mount widgets then wait forever (keeps scope open for event handlers)
  const program = Effect.gen(function* () {
    yield* mountAllWidgets
    console.log("[Effuse] Widgets mounted, keeping scope alive...")
    // Never complete - keeps the scope open so forked fibers keep running
    yield* Effect.never
  })

  Effect.runFork(
    program.pipe(
      Effect.provide(layer),
      Effect.scoped
    )
  )

  console.log("[Effuse] Mainview initialized")
}

// Initialize when DOM is ready
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initEffuse)
} else {
  initEffuse()
}

// Export for debugging
export { initEffuse }
