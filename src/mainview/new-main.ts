/**
 * New Mode Entry Point
 *
 * Mounts the introduction card component for the TerminalBench Gym.
 */

import { Effect, Layer } from "effect"
import {
  DomServiceLive, IntroCardComponent, mountComponent, SocketServiceFromClient,
  StateServiceLive, ThreeBackgroundComponent
} from "../effuse/index.js"
import { getSocketClient } from "./socket-client.js"

console.log("[New Mode] Loading...")

// Add visible error display
const showError = (msg: string) => {
  document.body.innerHTML = `<div style="padding:20px;color:red;font-family:monospace;background:#1a1a1a;">
    <h2>New Mode Error</h2>
    <pre>${msg}</pre>
  </div>`
}

// Global error handler
window.onerror = (msg, src, line, col, error) => {
  console.error("[New Mode] Global error:", msg, src, line, col, error)
  const errorMsg = `${msg}\n\nSource: ${src}:${line}:${col}\n\n${error?.stack || ""}`
  showError(errorMsg)
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

  // Ignore empty plain objects (also webview-bun artifacts)
  if (
    event.reason &&
    typeof event.reason === "object" &&
    Object.keys(event.reason).length === 0 &&
    !(event.reason instanceof Error)
  ) {
    // Silently ignore - these are harmless webview-bun artifacts
    event.preventDefault()
    return
  }

  // Log actual errors
  const reason = event.reason
  let errorMsg = "Unknown error"

  if (reason instanceof Error) {
    errorMsg = `${reason.name}: ${reason.message}\n\n${reason.stack || ""}`
  } else if (typeof reason === "string") {
    errorMsg = reason
  } else if (reason && typeof reason === "object") {
    try {
      errorMsg = JSON.stringify(reason, null, 2)
    } catch {
      errorMsg = String(reason)
    }
  } else {
    errorMsg = String(reason)
  }

  console.error("[New Mode] Unhandled rejection:", reason)
  showError(`Unhandled Promise rejection:\n\n${errorMsg}`)
  event.preventDefault()
}

// ============================================================================
// Socket Client Setup
// ============================================================================

const socketClient = getSocketClient()

// Set up HMR reload handler
socketClient.onMessage((message) => {
  if (message.type === "dev_reload") {
    console.log("[New Mode] HMR: Reload triggered by", (message as any).changedFile)
    location.reload()
  }
})
console.log("[New Mode] HMR handler registered")

// ============================================================================
// Layer Creation
// ============================================================================

const createNewModeLayer = () => {
  return Layer.mergeAll(
    DomServiceLive,
    StateServiceLive,
    SocketServiceFromClient(socketClient)
  )
}

// ============================================================================
// Component Mounting
// ============================================================================

const mountThreeBackground = Effect.gen(function* () {
  console.log("[New Mode] Mounting Three.js background...")

  // Find or create container
  let container = document.getElementById("three-background-container")
  if (!container) {
    container = document.createElement("div")
    container.id = "three-background-container"
    document.body.appendChild(container)
  }

  console.log("[New Mode] Three.js container found/created:", container)
  yield* mountComponent(ThreeBackgroundComponent, container)
  console.log("[New Mode] Three.js background mounted")
})

const mountIntroCard = Effect.gen(function* () {
  console.log("[New Mode] Mounting intro card...")

  // Find or create container
  let container = document.getElementById("intro-card-container")
  if (!container) {
    container = document.createElement("div")
    container.id = "intro-card-container"
    document.body.appendChild(container)
  }

  console.log("[New Mode] Container found/created:", container)

  console.log("[New Mode] About to mount component...")
  yield* mountComponent(IntroCardComponent, container)

  console.log("[New Mode] Intro card mounted")
})

// ============================================================================
// Initialize
// ============================================================================

const initNewMode = () => {
  console.log("[New Mode] Initializing...")

  try {
    const layer = createNewModeLayer()

    const program = Effect.gen(function* () {
      // Mount Three.js background first (lower z-index)
      yield* mountThreeBackground
      // Mount intro card on top (higher z-index)
      yield* mountIntroCard
      // Keep scope alive
      yield* Effect.never
    })

    Effect.runFork(
      program.pipe(
        Effect.provide(layer),
        Effect.scoped,
        Effect.catchAll((error: unknown) => {
          console.error("[New Mode] Effect error caught:", error)
          const errorMsg = error instanceof Error
            ? `${error.name}: ${error.message}\n\n${error.stack || ""}`
            : String(error)
          showError(`Effect error:\n\n${errorMsg}`)
          return Effect.void
        }),
        Effect.catchAllDefect((defect: unknown) => {
          console.error("[New Mode] Defect caught:", defect)
          const defectMsg = defect instanceof Error
            ? `${defect.name}: ${defect.message}\n\n${defect.stack || ""}`
            : String(defect)
          showError(`Defect:\n\n${defectMsg}`)
          return Effect.void
        })
      )
    )

    console.log("[New Mode] Initialized")
  } catch (error) {
    console.error("[New Mode] Initialization error:", error)
    const errorMsg = error instanceof Error
      ? `${error.name}: ${error.message}\n\n${error.stack || ""}`
      : String(error)
    showError(`Initialization error:\n\n${errorMsg}`)
  }
}

// Auto-initialize when DOM is ready
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initNewMode)
} else {
  initNewMode()
}
