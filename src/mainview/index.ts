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
import type {
  HudMessage,
  APMUpdateMessage,
  APMSnapshotMessage,
  TBRunStartMessage,
  TBRunCompleteMessage,
  TBTaskStartMessage,
  TBTaskProgressMessage,
  TBTaskCompleteMessage,
} from "../hud/protocol.js"
import {
  isTBRunStart,
  isTBRunComplete,
  isTBTaskStart,
  isTBTaskProgress,
  isTBTaskComplete,
} from "../hud/protocol.js"

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
// Terminal-Bench Widget State
// ============================================================================

type TBTaskStatus = "pending" | "running" | "passed" | "failed" | "timeout" | "error"

interface TBTaskState {
  id: string
  name: string
  difficulty: string
  category: string
  status: TBTaskStatus
  durationMs?: number
  turns?: number
}

interface TBState {
  isRunning: boolean
  runId: string | null
  suiteName: string
  suiteVersion: string
  totalTasks: number
  tasks: Map<string, TBTaskState>
  currentTaskId: string | null
  currentPhase: string | null
  currentTurn: number
  passed: number
  failed: number
  timeout: number
  error: number
  passRate: number
  totalDurationMs: number
}

let tbState: TBState = {
  isRunning: false,
  runId: null,
  suiteName: "",
  suiteVersion: "",
  totalTasks: 0,
  tasks: new Map(),
  currentTaskId: null,
  currentPhase: null,
  currentTurn: 0,
  passed: 0,
  failed: 0,
  timeout: 0,
  error: 0,
  passRate: 0,
  totalDurationMs: 0,
}

function getTBStatusColor(status: TBTaskStatus): string {
  switch (status) {
    case "passed": return "#22c55e" // Green
    case "failed": return "#ef4444" // Red
    case "timeout": return "#f59e0b" // Amber
    case "error": return "#ef4444" // Red
    case "running": return "#3b82f6" // Blue
    default: return "#6b7280" // Gray
  }
}

function renderTBWidget(): string {
  // Don't render if no run has ever started
  if (!tbState.isRunning && tbState.tasks.size === 0) return ""

  const completed = tbState.passed + tbState.failed + tbState.timeout + tbState.error
  const progressPct = tbState.totalTasks > 0
    ? (completed / tbState.totalTasks) * 100
    : 0
  const progressWidth = (228 * progressPct) / 100

  // Status text
  let statusText = "Idle"
  if (tbState.isRunning && tbState.currentTaskId) {
    const task = tbState.tasks.get(tbState.currentTaskId)
    statusText = task ? `${task.name} (${tbState.currentPhase || "running"})` : tbState.currentTaskId
    if (tbState.currentTurn > 0) {
      statusText += ` | Turn ${tbState.currentTurn}`
    }
  } else if (!tbState.isRunning && completed > 0) {
    statusText = `Complete | ${(tbState.passRate * 100).toFixed(0)}% pass`
  }

  // Truncate status text if too long
  if (statusText.length > 35) {
    statusText = statusText.slice(0, 32) + "..."
  }

  const passColor = tbState.passed > 0 ? "#22c55e" : "#6b7280"
  const failColor = (tbState.failed + tbState.timeout + tbState.error) > 0 ? "#ef4444" : "#6b7280"

  return `
    <g transform="translate(20, 140)" class="tb-widget">
      <!-- Background -->
      <rect x="0" y="0" width="260" height="95" rx="8" ry="8"
            fill="#141017" stroke="rgba(34, 197, 94, 0.25)" stroke-width="1"/>

      <!-- Header: TB suite name -->
      <text x="16" y="24" fill="#22c55e" font-size="14" font-weight="bold" font-family="Berkeley Mono, monospace">
        TB: ${tbState.suiteName || "Terminal-Bench"}
      </text>
      <text x="200" y="24" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">
        ${completed}/${tbState.totalTasks}
      </text>

      <!-- Progress bar background -->
      <rect x="16" y="36" width="228" height="10" rx="5" fill="#1e1e2e"/>
      <!-- Progress bar fill -->
      <rect x="16" y="36" width="${progressWidth}" height="10" rx="5" fill="#22c55e"/>
      ${tbState.isRunning ? `
      <!-- Animated pulse for running state -->
      <rect x="16" y="36" width="${progressWidth}" height="10" rx="5" fill="#22c55e" opacity="0.5">
        <animate attributeName="opacity" values="0.5;0.2;0.5" dur="1.5s" repeatCount="indefinite"/>
      </rect>` : ""}

      <!-- Stats row -->
      <text x="16" y="64" fill="${passColor}" font-size="11" font-family="Berkeley Mono, monospace">
        ✓ ${tbState.passed}
      </text>
      <text x="60" y="64" fill="${failColor}" font-size="11" font-family="Berkeley Mono, monospace">
        ✗ ${tbState.failed + tbState.timeout + tbState.error}
      </text>
      <text x="100" y="64" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">
        ${tbState.isRunning ? "Running..." : tbState.passRate > 0 ? `${(tbState.passRate * 100).toFixed(1)}%` : ""}
      </text>

      <!-- Current task / status -->
      <text x="16" y="82" fill="#9ca3af" font-size="10" font-family="Berkeley Mono, monospace">
        ${statusText}
      </text>
    </g>
  `
}

// ============================================================================
// RPC Schema for HUD Messages (must match src/bun/index.ts)
// ============================================================================

interface TBRunOptions {
  suitePath: string
  taskIds?: string[]
  timeout?: number
  maxTurns?: number
  outputDir?: string
}

interface TBSuiteInfo {
  name: string
  version: string
  tasks: Array<{
    id: string
    name: string
    category: string
    difficulty: string
  }>
}

interface HudRpcSchema {
  bun: {
    requests: {
      loadTBSuite: (suitePath: string) => Promise<TBSuiteInfo>
      startTBRun: (options: TBRunOptions) => Promise<{ runId: string }>
      stopTBRun: () => Promise<{ stopped: boolean }>
    };
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

  // Handle Terminal-Bench messages
  if (isTBRunStart(message)) {
    tbState = {
      isRunning: true,
      runId: message.runId,
      suiteName: message.suiteName,
      suiteVersion: message.suiteVersion,
      totalTasks: message.totalTasks,
      tasks: new Map(message.taskIds.map(id => [id, {
        id,
        name: id,
        difficulty: "",
        category: "",
        status: "pending" as TBTaskStatus,
      }])),
      currentTaskId: null,
      currentPhase: null,
      currentTurn: 0,
      passed: 0,
      failed: 0,
      timeout: 0,
      error: 0,
      passRate: 0,
      totalDurationMs: 0,
    }
    render()
    return
  }

  if (isTBTaskStart(message)) {
    const task = tbState.tasks.get(message.taskId)
    if (task) {
      task.name = message.taskName
      task.difficulty = message.difficulty
      task.category = message.category
      task.status = "running"
    }
    tbState.currentTaskId = message.taskId
    tbState.currentPhase = "setup"
    tbState.currentTurn = 0
    render()
    return
  }

  if (isTBTaskProgress(message)) {
    tbState.currentPhase = message.phase
    if (message.currentTurn !== undefined) {
      tbState.currentTurn = message.currentTurn
    }
    render()
    return
  }

  if (isTBTaskComplete(message)) {
    const task = tbState.tasks.get(message.taskId)
    if (task) {
      task.status = message.outcome === "success" ? "passed" : message.outcome as TBTaskStatus
      task.durationMs = message.durationMs
      task.turns = message.turns
    }
    // Update counters
    switch (message.outcome) {
      case "success": tbState.passed++; break
      case "failure": tbState.failed++; break
      case "timeout": tbState.timeout++; break
      case "error": tbState.error++; break
    }
    tbState.currentTaskId = null
    tbState.currentPhase = null
    tbState.currentTurn = 0
    render()
    return
  }

  if (isTBRunComplete(message)) {
    tbState.isRunning = false
    tbState.passRate = message.passRate
    tbState.totalDurationMs = message.totalDurationMs
    tbState.currentTaskId = null
    tbState.currentPhase = null
    render()
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
  // Add APM and TB widgets as fixed overlays (not affected by pan/zoom)
  const apmOverlay = renderAPMWidget()
  const tbOverlay = renderTBWidget()
  svg.innerHTML = svgElementToString(flowGroup) + apmOverlay + tbOverlay

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

// ============================================================================
// TB Run Controls (exposed for UI interaction)
// ============================================================================

/** Load a TB suite and populate the task list */
async function loadTBSuite(suitePath: string): Promise<TBSuiteInfo> {
  console.log("[TB] Loading suite:", suitePath)
  const suiteInfo = await rpc.request.loadTBSuite(suitePath)
  console.log("[TB] Suite loaded:", suiteInfo.name, `(${suiteInfo.tasks.length} tasks)`)
  return suiteInfo
}

/** Start a TB run with the given options */
async function startTBRun(options: TBRunOptions): Promise<string> {
  console.log("[TB] Starting run:", options)
  const { runId } = await rpc.request.startTBRun(options)
  console.log("[TB] Run started:", runId)
  return runId
}

/** Stop the current TB run */
async function stopTBRun(): Promise<boolean> {
  console.log("[TB] Stopping run")
  const { stopped } = await rpc.request.stopTBRun()
  console.log("[TB] Stopped:", stopped)
  return stopped
}

// Expose for console access during development
declare global {
  interface Window {
    TB: {
      loadSuite: typeof loadTBSuite
      startRun: typeof startTBRun
      stopRun: typeof stopTBRun
    }
  }
}

window.TB = {
  loadSuite: loadTBSuite,
  startRun: startTBRun,
  stopRun: stopTBRun,
}

console.log("Flow HUD loaded with WebSocket support")
console.log("TB controls available: window.TB.loadSuite(), window.TB.startRun(), window.TB.stopRun()")
