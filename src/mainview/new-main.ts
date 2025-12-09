/**
 * New Mode Entry Point
 *
 * Mounts the introduction card component for the TerminalBench Gym.
 */

import { Effect, Layer } from "effect"
import {
  DomServiceLive, mountComponent, SocketServiceFromClient,
  StateServiceLive, AgentGraphComponent
} from "../effuse/index.js"
import { getSocketClient } from "./socket-client.js"

console.log("[New Mode] Loading...")

// Add visible error display as a small toast in bottom-left
const showError = (msg: string) => {
  // Remove any existing error toasts
  const existing = document.getElementById("error-toast")
  if (existing) existing.remove()

  const toast = document.createElement("div")
  toast.id = "error-toast"
  toast.style.cssText = `
    position: fixed;
    bottom: 20px;
    left: 20px;
    max-width: 400px;
    max-height: 300px;
    padding: 12px 16px;
    background: rgba(26, 26, 26, 0.95);
    border: 2px solid #ef4444;
    border-radius: 8px;
    color: #fca5a5;
    font-family: 'Berkeley Mono', monospace;
    font-size: 11px;
    overflow: auto;
    z-index: 10000;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(4px);
  `

  const header = document.createElement("div")
  header.style.cssText = `
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
    padding-bottom: 8px;
    border-bottom: 1px solid #7f1d1d;
  `

  const title = document.createElement("strong")
  title.textContent = "Error"
  title.style.cssText = "color: #ef4444; font-size: 12px;"

  const closeBtn = document.createElement("button")
  closeBtn.textContent = "×"
  closeBtn.style.cssText = `
    background: none;
    border: none;
    color: #fca5a5;
    font-size: 20px;
    cursor: pointer;
    padding: 0;
    width: 20px;
    height: 20px;
    line-height: 1;
  `
  closeBtn.onclick = () => toast.remove()

  header.appendChild(title)
  header.appendChild(closeBtn)

  const content = document.createElement("pre")
  content.textContent = msg
  content.style.cssText = `
    margin: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
    font-size: 10px;
    line-height: 1.4;
  `

  toast.appendChild(header)
  toast.appendChild(content)
  document.body.appendChild(toast)

  // Auto-dismiss after 10 seconds
  setTimeout(() => {
    if (toast.parentElement) toast.remove()
  }, 10000)
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

const mountAgentGraph = Effect.gen(function* () {
  console.log("[New Mode] Mounting agent graph background...")

  // Find or create container
  let container = document.getElementById("three-background-container")
  if (!container) {
    container = document.createElement("div")
    container.id = "three-background-container"
    document.body.appendChild(container)
  }

  console.log("[New Mode] Agent graph container found/created:", container)
  yield* mountComponent(AgentGraphComponent, container)
  console.log("[New Mode] Agent graph background mounted")
})

// ============================================================================
// Initialize
// ============================================================================

const initNewMode = () => {
  console.log("[New Mode] Initializing...")

  try {
    const layer = createNewModeLayer()

    const program = Effect.gen(function* () {
      // Mount agent graph background
      yield* mountAgentGraph
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
