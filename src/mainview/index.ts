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
import type { HudMessage, APMUpdateMessage, APMSnapshotMessage } from "../hud/protocol.js"

// ============================================================================
// APM Widget State
// ============================================================================

interface APMState {
  sessionAPM: number
  recentAPM: number
  totalActions: number
  durationMinutes: number
  // Historical snapshot data
  apm1h: number
  apm6h: number
  apm1d: number
  apmLifetime: number
  claudeCodeAPM: number
  mechaCoderAPM: number
  efficiencyRatio: number
}

let apmState: APMState = {
  sessionAPM: 0,
  recentAPM: 0,
  totalActions: 0,
  durationMinutes: 0,
  apm1h: 0,
  apm6h: 0,
  apm1d: 0,
  apmLifetime: 0,
  claudeCodeAPM: 0,
  mechaCoderAPM: 0,
  efficiencyRatio: 0,
}

function getAPMColor(apm: number): string {
  if (apm >= 30) return "#f59e0b" // Gold - Elite
  if (apm >= 15) return "#22c55e" // Green - High velocity
  if (apm >= 5) return "#3b82f6" // Blue - Active
  return "#6b7280" // Gray - Baseline
}

function renderAPMWidget(): string {
  const color = getAPMColor(apmState.sessionAPM)
  const efficiencyText = apmState.efficiencyRatio > 0
    ? `${apmState.efficiencyRatio.toFixed(1)}x faster`
    : ""
  const deltaPercent = apmState.efficiencyRatio > 0
    ? `+${((apmState.efficiencyRatio - 1) * 100).toFixed(0)}%`
    : ""

  return `
    <g transform="translate(20, 20)" class="apm-widget">
      <!-- Background -->
      <rect x="0" y="0" width="260" height="110" rx="8" ry="8"
            fill="#141017" stroke="rgba(245, 158, 11, 0.25)" stroke-width="1"/>

      <!-- Header: APM value -->
      <text x="16" y="32" fill="${color}" font-size="24" font-weight="bold" font-family="Berkeley Mono, monospace">
        APM: ${apmState.sessionAPM.toFixed(1)}
      </text>
      ${efficiencyText ? `
      <text x="140" y="32" fill="#22c55e" font-size="14" font-family="Berkeley Mono, monospace">
        ▲ ${efficiencyText}
      </text>` : ""}

      <!-- Session stats -->
      <text x="16" y="54" fill="#9ca3af" font-size="12" font-family="Berkeley Mono, monospace">
        Session: ${apmState.totalActions} actions | ${apmState.durationMinutes.toFixed(0)}m
      </text>

      <!-- Time windows -->
      <text x="16" y="74" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">
        1h: ${apmState.apm1h.toFixed(1)} | 6h: ${apmState.apm6h.toFixed(1)} | 24h: ${apmState.apm1d.toFixed(1)}
      </text>

      <!-- Comparison -->
      ${apmState.mechaCoderAPM > 0 ? `
      <text x="16" y="94" fill="#f59e0b" font-size="11" font-family="Berkeley Mono, monospace">
        MechaCoder vs Claude Code: ${deltaPercent}
      </text>` : ""}
    </g>
  `
}

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

  // Handle APM-specific messages
  if (message.type === "apm_update") {
    const apmMsg = message as APMUpdateMessage
    apmState = {
      ...apmState,
      sessionAPM: apmMsg.sessionAPM,
      recentAPM: apmMsg.recentAPM,
      totalActions: apmMsg.totalActions,
      durationMinutes: apmMsg.durationMinutes,
    }
    render() // Update APM widget
    return
  }

  if (message.type === "apm_snapshot") {
    const snapMsg = message as APMSnapshotMessage
    apmState = {
      ...apmState,
      apm1h: snapMsg.combined.apm1h,
      apm6h: snapMsg.combined.apm6h,
      apm1d: snapMsg.combined.apm1d,
      apmLifetime: snapMsg.combined.apmLifetime,
      claudeCodeAPM: snapMsg.comparison.claudeCodeAPM,
      mechaCoderAPM: snapMsg.comparison.mechaCoderAPM,
      efficiencyRatio: snapMsg.comparison.efficiencyRatio,
    }
    render() // Update APM widget
    return
  }

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
  // Add APM widget as fixed overlay (not affected by pan/zoom)
  const apmOverlay = renderAPMWidget()
  svg.innerHTML = svgElementToString(flowGroup) + apmOverlay

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
