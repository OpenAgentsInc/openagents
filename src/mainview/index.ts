import * as BunContext from "@effect/platform-bun/BunContext"
import { Effect } from "effect"
import { calculateLayout } from "../flow/layout.js"
import { buildMechaCoderFlowTree, generateNodeSizes } from "../flow/mechacoder-map.js"
import { loadMechaCoderState } from "../flow/mechacoder-state.js"
import { sampleMechaCoderTree, sampleNodeSizes } from "../flow/sample-data.js"
import {
  initialCanvasState,
  reduceCanvasState,
  DEFAULT_CONFIG,
  type CanvasEvent,
} from "../flow/canvas.js"
import {
  renderFlowSVG,
  svgElementToString,
  DEFAULT_RENDER_CONFIG,
} from "../flow-host-svg/render.js"
import Electrobun, { Electroview } from "electrobun/view"
import type { HudMessage } from "../hud/protocol.js"

// ============================================================================
// RPC Schema for HUD Messages (must match src/bun/index.ts)
// ============================================================================

interface HudRpcSchema {
  bun: {
    requests: {};
    messages: {
      hudMessage: HudMessage;
    };
  };
  webview: {
    requests: {};
    messages: {};
  };
}

// ============================================================================
// HUD Event State
// ============================================================================

/** Store recent HUD events for display */
const hudEventHistory: HudMessage[] = []
const MAX_HUD_HISTORY = 50

/** Events that should trigger an immediate refresh */
const REFRESH_TRIGGER_EVENTS = new Set([
  "task_selected",
  "task_decomposed",
  "subtask_complete",
  "subtask_failed",
  "session_complete",
  "commit_created",
])

function handleHudMessage(message: HudMessage): void {
  // Store in history
  hudEventHistory.push(message)
  if (hudEventHistory.length > MAX_HUD_HISTORY) {
    hudEventHistory.shift()
  }

  console.log("[HUD] Received:", message.type, message)

  // Trigger immediate refresh for important state changes
  if (REFRESH_TRIGGER_EVENTS.has(message.type)) {
    void refreshLayoutFromState()
  }
}

// Larger padding/spacing to keep stacked agent->repo->task columns readable
const LAYOUT_CONFIG = { padding: 16, spacing: 280 }
const REFRESH_INTERVAL_MS = 5000

// Calculate layout once from sample data as a placeholder until live data loads
let layout = calculateLayout({
  root: sampleMechaCoderTree,
  nodeSizes: sampleNodeSizes,
  config: LAYOUT_CONFIG,
})
let hasLiveLayout = false
let isRefreshing = false

function getLayoutBounds() {
  const minX = Math.min(...layout.nodes.map(n => n.x))
  const minY = Math.min(...layout.nodes.map(n => n.y))
  const maxX = Math.max(...layout.nodes.map(n => n.x + n.size.width))
  const maxY = Math.max(...layout.nodes.map(n => n.y + n.size.height))
  return { minX, minY, maxX, maxY, width: maxX - minX, height: maxY - minY }
}

function getCenteredPan(viewWidth: number, viewHeight: number) {
  const bounds = getLayoutBounds()
  const centerX = viewWidth / 2 - (bounds.minX + bounds.width / 2)
  const centerY = viewHeight / 2 - (bounds.minY + bounds.height / 2)
  return { panX: centerX, panY: centerY }
}

async function refreshLayoutFromState(): Promise<void> {
  if (isRefreshing) return
  isRefreshing = true
  try {
    const state = await Effect.runPromise(
      loadMechaCoderState({ rootDir: "." }).pipe(Effect.provide(BunContext.layer)),
    )
    const tree = buildMechaCoderFlowTree(state)
    const nodeSizes = generateNodeSizes(tree)
    layout = calculateLayout({
      root: tree,
      nodeSizes,
      config: LAYOUT_CONFIG,
    })

    if (!hasLiveLayout) {
      const recentered = getCenteredPan(canvasState.viewportWidth, canvasState.viewportHeight)
      canvasState = { ...canvasState, ...recentered }
      hasLiveLayout = true
    }

    render()
  } catch (error) {
    console.error("Failed to load MechaCoder state", error)
  } finally {
    isRefreshing = false
  }
}

// Get DOM elements
const container = document.getElementById("flow-container")!
const svg = document.getElementById("flow-svg")!
const resetBtn = document.getElementById("reset-btn")!
const zoomLevel = document.getElementById("zoom-level")!

// Initialize canvas state with viewport size
let canvasState = initialCanvasState(window.innerWidth, window.innerHeight)
const initialPan = getCenteredPan(window.innerWidth, window.innerHeight)
canvasState = { ...canvasState, ...initialPan }

// Render SVG content
function render(): void {
  const flowGroup = renderFlowSVG(layout, canvasState, DEFAULT_RENDER_CONFIG)
  svg.innerHTML = svgElementToString(flowGroup)
  
  // Update zoom display
  zoomLevel.textContent = `${Math.round(canvasState.scale * 100)}%`
}

// Apply canvas event and re-render
function dispatch(event: CanvasEvent): void {
  canvasState = reduceCanvasState(canvasState, event, DEFAULT_CONFIG)
  render()
}

// Mouse/pointer event handlers
container.addEventListener("mousedown", (e) => {
  container.classList.add("dragging")
  dispatch({
    type: "PAN_START",
    pointer: { x: e.clientX, y: e.clientY },
    timestamp: e.timeStamp,
  })
})

container.addEventListener("mousemove", (e) => {
  if (canvasState.isDragging) {
    dispatch({
      type: "PAN_MOVE",
      pointer: { x: e.clientX, y: e.clientY },
      timestamp: e.timeStamp,
    })
  }
})

container.addEventListener("mouseup", (e) => {
  container.classList.remove("dragging")
  dispatch({
    type: "PAN_END",
    timestamp: e.timeStamp,
  })
})

container.addEventListener("mouseleave", (e) => {
  if (canvasState.isDragging) {
    container.classList.remove("dragging")
    dispatch({
      type: "PAN_END",
      timestamp: e.timeStamp,
    })
  }
})

// Wheel zoom
container.addEventListener("wheel", (e) => {
  e.preventDefault()
  dispatch({
    type: "ZOOM",
    pointer: { x: e.clientX, y: e.clientY },
    delta: e.deltaY,
  })
}, { passive: false })

// Reset button
resetBtn.addEventListener("click", () => {
  dispatch({ type: "RESET" })
  // Re-center after reset
  const recentered = getCenteredPan(canvasState.viewportWidth, canvasState.viewportHeight)
  canvasState = { ...canvasState, ...recentered }
  render()
})

// Handle window resize
window.addEventListener("resize", () => {
  dispatch({
    type: "RESIZE",
    width: window.innerWidth,
    height: window.innerHeight,
  })
  const recentered = getCenteredPan(window.innerWidth, window.innerHeight)
  canvasState = { ...canvasState, ...recentered }
  render()
})

// Inertial animation loop
let animationId: number | null = null

function tick(): void {
  if (canvasState.velocityX !== 0 || canvasState.velocityY !== 0) {
    dispatch({ type: "TICK" })
    animationId = requestAnimationFrame(tick)
  } else {
    animationId = null
  }
}

// Start inertia after pan ends
const originalDispatch = dispatch
function dispatchWithInertia(event: CanvasEvent): void {
  originalDispatch(event)
  
  if (event.type === "PAN_END" && !animationId) {
    if (canvasState.velocityX !== 0 || canvasState.velocityY !== 0) {
      animationId = requestAnimationFrame(tick)
    }
  }
}

// Replace dispatch to use inertia version
container.removeEventListener("mouseup", () => {})
container.addEventListener("mouseup", (e) => {
  container.classList.remove("dragging")
  dispatchWithInertia({
    type: "PAN_END",
    timestamp: e.timeStamp,
  })
})

// Initial render
render()

// Load live data and refresh periodically (fallback polling)
void refreshLayoutFromState()
setInterval(refreshLayoutFromState, REFRESH_INTERVAL_MS)

// ============================================================================
// Electrobun RPC Setup for Real-time HUD Events
// ============================================================================

// Set up RPC to receive hudMessage events from the Bun process
const rpc = Electroview.defineRPC<HudRpcSchema>({
  maxRequestTime: 10000,
  handlers: {
    requests: {},
    messages: {
      hudMessage: (message: HudMessage) => {
        handleHudMessage(message)
      },
    },
  },
})

// Initialize Electrobun with RPC
const electrobunInstance = new Electrobun.Electroview({ rpc })
void electrobunInstance // Keep reference to avoid GC

console.log("Flow HUD loaded with WebSocket support")
