import * as BunContext from "@effect/platform-bun/BunContext"
import { Effect } from "effect"
import { calculateLayout } from "../flow/layout.js"
import { buildMechaCoderFlowTree, generateNodeSizes } from "../flow/mechacoder-map.js"
import { loadMechaCoderState } from "../flow/mechacoder-state.js"
import { sampleMechaCoderTree, sampleNodeSizes } from "../flow/sample-data.js"
// TB flow tree imports
import {
  buildTBFlowTree,
  generateTBNodeSizes,
  createEmptyTBFlowState,
  toggleRunExpanded,
  type TBFlowState,
  type TBRunDetails,
} from "../flow/tb-map.js"
import { loadRecentRuns, loadTBRun } from "../tbench-hud/persistence.js"
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
} from "../hud/protocol.js"
import {
  isTBRunStart,
  isTBRunComplete,
  isTBTaskStart,
  isTBTaskProgress,
  isTBTaskOutput,
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

interface TBOutputLine {
  text: string
  source: "agent" | "verification" | "system"
  timestamp: number
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
  // Output streaming
  outputBuffer: TBOutputLine[]
  maxOutputLines: number
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
  outputBuffer: [],
  maxOutputLines: 500,
}

// ============================================================================
// View Mode State
// ============================================================================

type ViewMode = "flow" | "tbench"
// Default to TB mode (previously "flow")
let viewMode: ViewMode = (localStorage.getItem("hud-view-mode") as ViewMode) || "tbench"

function setViewMode(mode: ViewMode): void {
  viewMode = mode
  localStorage.setItem("hud-view-mode", mode)
  updateViewModeUI()
  render()
}

function updateViewModeUI(): void {
  const flowBtn = document.getElementById("view-flow-btn")
  const tbBtn = document.getElementById("view-tb-btn")
  if (flowBtn && tbBtn) {
    flowBtn.classList.toggle("active", viewMode === "flow")
    tbBtn.classList.toggle("active", viewMode === "tbench")
  }

  // TB controls always visible (user can trigger runs from any view)
  const tbControls = document.getElementById("tb-controls")
  if (tbControls) {
    tbControls.style.display = "block"
  }
}

// Initialize view mode on load
setTimeout(updateViewModeUI, 0)

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
// TB Dashboard View (Full-screen Terminal-Bench mode)
// ============================================================================

function renderTBDashboard(): string {
  const vw = window.innerWidth
  const vh = window.innerHeight
  const completed = tbState.passed + tbState.failed + tbState.timeout + tbState.error
  const progressPct = tbState.totalTasks > 0 ? (completed / tbState.totalTasks) * 100 : 0

  // Current task info
  let currentTaskName = "No task running"
  let currentTaskPhase = ""
  if (tbState.isRunning && tbState.currentTaskId) {
    const task = tbState.tasks.get(tbState.currentTaskId)
    currentTaskName = task?.name || tbState.currentTaskId
    currentTaskPhase = tbState.currentPhase || "running"
    if (tbState.currentTurn > 0) {
      currentTaskPhase += ` · Turn ${tbState.currentTurn}`
    }
  }

  // Build task list rows
  const taskRows = Array.from(tbState.tasks.values()).map((task, i) => {
    const y = 280 + i * 28
    const statusColor = getTBStatusColor(task.status)
    const statusIcon = task.status === "passed" ? "✓" : task.status === "failed" ? "✗" : task.status === "running" ? "▶" : "○"
    return `
      <text x="60" y="${y}" fill="${statusColor}" font-size="14" font-family="Berkeley Mono, monospace">${statusIcon}</text>
      <text x="90" y="${y}" fill="#e5e5e5" font-size="13" font-family="Berkeley Mono, monospace">${task.name}</text>
      <text x="${vw - 120}" y="${y}" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">${task.difficulty}</text>
      ${task.durationMs ? `<text x="${vw - 60}" y="${y}" fill="#6b7280" font-size="11" font-family="Berkeley Mono, monospace">${(task.durationMs / 1000).toFixed(1)}s</text>` : ""}
    `
  }).join("")

  return `
    <!-- TB Dashboard Background -->
    <rect x="0" y="0" width="${vw}" height="${vh}" fill="#0a0a0f"/>

    <!-- Header -->
    <text x="40" y="50" fill="#22c55e" font-size="28" font-weight="bold" font-family="Berkeley Mono, monospace">
      Terminal-Bench
    </text>
    <text x="40" y="80" fill="#6b7280" font-size="14" font-family="Berkeley Mono, monospace">
      ${tbState.suiteName || "No suite loaded"} ${tbState.suiteVersion ? `v${tbState.suiteVersion}` : ""}
    </text>

    <!-- Progress Section -->
    <rect x="40" y="110" width="${vw - 80}" height="100" rx="8" fill="#141017" stroke="rgba(34, 197, 94, 0.2)" stroke-width="1"/>

    <!-- Progress bar -->
    <rect x="60" y="130" width="${vw - 120}" height="20" rx="10" fill="#1e1e2e"/>
    <rect x="60" y="130" width="${(vw - 120) * progressPct / 100}" height="20" rx="10" fill="#22c55e"/>
    ${tbState.isRunning ? `
    <rect x="60" y="130" width="${(vw - 120) * progressPct / 100}" height="20" rx="10" fill="#22c55e" opacity="0.5">
      <animate attributeName="opacity" values="0.5;0.2;0.5" dur="1.5s" repeatCount="indefinite"/>
    </rect>` : ""}

    <!-- Stats -->
    <text x="60" y="175" fill="#22c55e" font-size="18" font-family="Berkeley Mono, monospace">
      ✓ ${tbState.passed}
    </text>
    <text x="140" y="175" fill="#ef4444" font-size="18" font-family="Berkeley Mono, monospace">
      ✗ ${tbState.failed}
    </text>
    <text x="220" y="175" fill="#f59e0b" font-size="18" font-family="Berkeley Mono, monospace">
      ⏱ ${tbState.timeout}
    </text>
    <text x="300" y="175" fill="#8b5cf6" font-size="18" font-family="Berkeley Mono, monospace">
      ⚠ ${tbState.error}
    </text>
    <text x="${vw - 200}" y="175" fill="#e5e5e5" font-size="18" font-family="Berkeley Mono, monospace">
      ${completed} / ${tbState.totalTasks} (${progressPct.toFixed(1)}%)
    </text>

    <!-- Current Task -->
    <text x="60" y="195" fill="#9ca3af" font-size="12" font-family="Berkeley Mono, monospace">
      ${tbState.isRunning ? `Current: ${currentTaskName} · ${currentTaskPhase}` : tbState.passRate > 0 ? `Completed · ${(tbState.passRate * 100).toFixed(1)}% pass rate` : "Ready to run"}
    </text>

    <!-- Task List Header -->
    <text x="40" y="250" fill="#6b7280" font-size="12" font-family="Berkeley Mono, monospace" text-transform="uppercase" letter-spacing="1">
      TASKS
    </text>
    <line x1="40" y1="260" x2="${vw - 40}" y2="260" stroke="rgba(255,255,255,0.1)" stroke-width="1"/>

    <!-- Task List -->
    ${taskRows}

    <!-- Footer hint -->
    <text x="40" y="${vh - 30}" fill="#4b5563" font-size="11" font-family="Berkeley Mono, monospace">
      Press Ctrl+1 for Flow view · Ctrl+2 for TB view · Ctrl+T to start · Ctrl+X to stop
    </text>
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
      outputBuffer: [],
      maxOutputLines: 500,
    }
    // Sync flow state with updated tbState
    syncTBFlowWithState()
    render()
    // Sync UI controls (elements set later, safe at runtime)
    document.getElementById("tb-status")!.textContent = "Running..."
    document.getElementById("tb-status")!.className = "tb-status running"
    ;(document.getElementById("tb-start-btn") as HTMLButtonElement).disabled = true
    ;(document.getElementById("tb-stop-btn") as HTMLButtonElement).disabled = false
    // Show category tree (functions defined later in file)
    ;(window as unknown as Record<string, () => void>).__showCategoryTree?.()
    requestAnimationFrame(() => (window as unknown as Record<string, () => void>).__renderCategoryTree?.())
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
    // Sync flow state with updated current task
    syncTBFlowWithState()
    render()
    requestAnimationFrame(() => (window as unknown as Record<string, () => void>).__renderCategoryTree?.())
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

  if (isTBTaskOutput(message)) {
    // Aggregate tokens into lines - only create new line on newline char
    const text = message.text
    const source = message.source
    const now = Date.now()

    // Split on newlines to handle multi-line output
    const parts = text.split("\n")

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]

      // Get last line in buffer (if same source and recent)
      const lastLine = tbState.outputBuffer[tbState.outputBuffer.length - 1]
      const canAppend = lastLine &&
        lastLine.source === source &&
        now - lastLine.timestamp < 5000 && // Same logical line if within 5s
        i === 0 // Only append to last line for the first part

      if (canAppend && part.length > 0) {
        // Append to existing line
        lastLine.text += part
        lastLine.timestamp = now
      } else if (part.length > 0 || i > 0) {
        // Create new line (either has content or is after a newline)
        tbState.outputBuffer.push({
          text: part,
          source,
          timestamp: now,
        })
      }
    }

    // Trim buffer if too large
    if (tbState.outputBuffer.length > tbState.maxOutputLines) {
      tbState.outputBuffer = tbState.outputBuffer.slice(-tbState.maxOutputLines)
    }
    // Update output viewer (throttled via requestAnimationFrame)
    requestAnimationFrame(() => updateOutputViewer())
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
    // Sync flow state (task no longer running)
    syncTBFlowWithState()
    render()
    requestAnimationFrame(() => (window as unknown as Record<string, () => void>).__renderCategoryTree?.())
    return
  }

  if (isTBRunComplete(message)) {
    tbState.isRunning = false
    tbState.passRate = message.passRate
    tbState.totalDurationMs = message.totalDurationMs
    tbState.currentTaskId = null
    tbState.currentPhase = null
    // Sync flow state (run no longer active)
    syncTBFlowWithState()
    render()
    // Sync UI controls
    document.getElementById("tb-status")!.textContent = `Done ${(message.passRate * 100).toFixed(0)}%`
    document.getElementById("tb-status")!.className = "tb-status"
    ;(document.getElementById("tb-start-btn") as HTMLButtonElement).disabled = false
    ;(document.getElementById("tb-stop-btn") as HTMLButtonElement).disabled = true
    // Reload run history to include the completed run
    void refreshTBLayout()
    return
  }

  // Trigger immediate refresh for important state changes
  if (REFRESH_TRIGGER_EVENTS.has(message.type)) {
    void refreshLayoutFromState()
  }
}

// Larger padding/spacing to keep stacked agent->repo->task columns readable
const LAYOUT_CONFIG = { padding: 16, spacing: 280 }
const TB_LAYOUT_CONFIG = { padding: 12, spacing: 180 }
const REFRESH_INTERVAL_MS = 5000

// Calculate layout once from sample data as a placeholder until live data loads
let layout = calculateLayout({
  root: sampleMechaCoderTree,
  nodeSizes: sampleNodeSizes,
  config: LAYOUT_CONFIG,
})
let hasLiveLayout = false
let isRefreshing = false

// ============================================================================
// TB Flow State
// ============================================================================

let tbFlowState: TBFlowState = createEmptyTBFlowState()
let tbRunDetails: Map<string, TBRunDetails> = new Map()
let tbLayout = calculateLayout({
  root: buildTBFlowTree(tbFlowState),
  nodeSizes: generateTBNodeSizes(buildTBFlowTree(tbFlowState)),
  config: TB_LAYOUT_CONFIG,
})

/**
 * Refresh TB flow layout from run history
 */
async function refreshTBLayout(): Promise<void> {
  try {
    // Load recent runs (metadata only)
    const runs = await loadRecentRuns(10)

    // Update TB flow state with runs
    tbFlowState = {
      ...tbFlowState,
      runs,
      currentRunId: tbState.isRunning ? tbState.runId : null,
      currentTaskId: tbState.currentTaskId,
    }

    // Build the flow tree
    const tree = buildTBFlowTree(tbFlowState, tbRunDetails)
    const nodeSizes = generateTBNodeSizes(tree)
    tbLayout = calculateLayout({
      root: tree,
      nodeSizes,
      config: TB_LAYOUT_CONFIG,
    })

    if (viewMode === "tbench") {
      render()
    }
  } catch (error) {
    console.error("[TB] Failed to refresh layout:", error)
  }
}

/**
 * Toggle expansion of a run node
 */
async function handleRunNodeClick(runId: string): Promise<void> {
  const isExpanding = !tbFlowState.expandedRunIds.has(runId)

  // Toggle expansion
  tbFlowState = toggleRunExpanded(tbFlowState, runId)

  // If expanding, load the run details if not already cached
  if (isExpanding && !tbRunDetails.has(runId)) {
    try {
      const run = tbFlowState.runs.find(r => r.runId === runId)
      if (run) {
        const fullRun = await loadTBRun(run.filepath)
        tbRunDetails.set(runId, {
          meta: fullRun.meta,
          tasks: fullRun.tasks,
        })
      }
    } catch (error) {
      console.error(`[TB] Failed to load run ${runId}:`, error)
    }
  }

  // Rebuild the tree
  const tree = buildTBFlowTree(tbFlowState, tbRunDetails)
  const nodeSizes = generateTBNodeSizes(tree)
  tbLayout = calculateLayout({
    root: tree,
    nodeSizes,
    config: TB_LAYOUT_CONFIG,
  })

  render()
}

/**
 * Sync TB flow state with current tbState and rebuild layout.
 * Call this whenever TB events update tbState.
 */
function syncTBFlowWithState(): void {
  tbFlowState = {
    ...tbFlowState,
    currentRunId: tbState.isRunning ? tbState.runId : null,
    currentTaskId: tbState.currentTaskId,
  }

  // Rebuild the flow tree with updated state
  const tree = buildTBFlowTree(tbFlowState, tbRunDetails)
  const nodeSizes = generateTBNodeSizes(tree)
  tbLayout = calculateLayout({
    root: tree,
    nodeSizes,
    config: TB_LAYOUT_CONFIG,
  })
}

function getLayoutBounds() {
  // Use appropriate layout based on view mode
  const currentLayout = viewMode === "tbench" ? tbLayout : layout
  const minX = Math.min(...currentLayout.nodes.map(n => n.x))
  const minY = Math.min(...currentLayout.nodes.map(n => n.y))
  const maxX = Math.max(...currentLayout.nodes.map(n => n.x + n.size.width))
  const maxY = Math.max(...currentLayout.nodes.map(n => n.y + n.size.height))
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

// TB Control DOM elements
const tbSuitePathInput = document.getElementById("tb-suite-path") as HTMLInputElement
const tbLoadBtn = document.getElementById("tb-load-btn")!
const tbStartBtn = document.getElementById("tb-start-btn")!
const tbStopBtn = document.getElementById("tb-stop-btn")!
const tbStatus = document.getElementById("tb-status")!
const tbTaskSelector = document.getElementById("tb-task-selector")!
const tbSuiteName = document.getElementById("tb-suite-name")!
const tbTaskList = document.getElementById("tb-task-list")!
const tbSelectAll = document.getElementById("tb-select-all")!
const tbSelectNone = document.getElementById("tb-select-none")!

// TB UI State
let selectedTaskIds: Set<string> = new Set()

// Initialize canvas state with viewport size
let canvasState = initialCanvasState(window.innerWidth, window.innerHeight)
const initialPan = getCenteredPan(window.innerWidth, window.innerHeight)
canvasState = { ...canvasState, ...initialPan }

// Render SVG content
function render(): void {
  if (viewMode === "flow") {
    // Flow view: MechaCoder graph with overlays
    const flowGroup = renderFlowSVG(layout, canvasState, DEFAULT_RENDER_CONFIG)
    const apmOverlay = renderAPMWidget()
    const tbOverlay = renderTBWidget()
    svg.innerHTML = svgElementToString(flowGroup) + apmOverlay + tbOverlay
  } else {
    // TB view: TB flow tree (run history nodes) on grid canvas + TB widget
    const tbFlowGroup = renderFlowSVG(tbLayout, canvasState, DEFAULT_RENDER_CONFIG)
    const tbOverlay = renderTBWidget()
    svg.innerHTML = svgElementToString(tbFlowGroup) + tbOverlay
  }

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

// Click handler for node interactions (TB run expand/collapse)
svg.addEventListener("click", (e) => {
  // Only handle clicks when not dragging
  if (canvasState.isDragging) return

  // Find clicked node by checking data-node-id on rect elements
  const target = e.target as SVGElement
  const nodeRect = target.closest("[data-node-id]") as SVGElement | null
  if (!nodeRect) return

  const nodeId = nodeRect.getAttribute("data-node-id")
  if (!nodeId) return

  // Handle TB run node clicks (expand/collapse)
  if (nodeId.startsWith("tb-run-")) {
    const runId = nodeId.replace("tb-run-", "")
    void handleRunNodeClick(runId)
  }
})

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
void refreshTBLayout()
setInterval(refreshLayoutFromState, REFRESH_INTERVAL_MS)
setInterval(refreshTBLayout, REFRESH_INTERVAL_MS)

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

// Type assertion for Electrobun RPC request interface
const rpcRequest = rpc as unknown as {
  request: HudRpcSchema["bun"]["requests"]
}

/** Load a TB suite and populate the task list */
async function loadTBSuiteRpc(suitePath: string): Promise<TBSuiteInfo> {
  console.log("[TB] Loading suite:", suitePath)
  const suiteInfo = await rpcRequest.request.loadTBSuite(suitePath)
  console.log("[TB] Suite loaded:", suiteInfo.name, `(${suiteInfo.tasks.length} tasks)`)
  return suiteInfo
}

/** Start a TB run with the given options */
async function startTBRunRpc(options: TBRunOptions): Promise<string> {
  console.log("[TB] Starting run:", options)
  const { runId } = await rpcRequest.request.startTBRun(options)
  console.log("[TB] Run started:", runId)
  return runId
}

/** Stop the current TB run */
async function stopTBRunRpc(): Promise<boolean> {
  console.log("[TB] Stopping run")
  const { stopped } = await rpcRequest.request.stopTBRun()
  console.log("[TB] Stopped:", stopped)
  return stopped
}

// ============================================================================
// TB UI Control Handlers
// ============================================================================

function updateTBStatus(status: string, className?: string): void {
  tbStatus.textContent = status
  tbStatus.className = "tb-status" + (className ? ` ${className}` : "")
}

function updateTBButtons(isRunning: boolean): void {
  (tbStartBtn as HTMLButtonElement).disabled = isRunning
  ;(tbStopBtn as HTMLButtonElement).disabled = !isRunning
  ;(tbLoadBtn as HTMLButtonElement).disabled = isRunning
  tbSuitePathInput.disabled = isRunning
}

function renderTaskList(suite: TBSuiteInfo): void {
  tbTaskList.innerHTML = ""
  selectedTaskIds.clear()

  for (const task of suite.tasks) {
    selectedTaskIds.add(task.id) // Select all by default

    const item = document.createElement("label")
    item.className = "tb-task-item"
    item.innerHTML = `
      <input type="checkbox" data-task-id="${task.id}" checked>
      <span class="task-name" title="${task.name}">${task.name}</span>
      <span class="task-difficulty ${task.difficulty}">${task.difficulty}</span>
    `

    const checkbox = item.querySelector("input")!
    checkbox.addEventListener("change", () => {
      if (checkbox.checked) {
        selectedTaskIds.add(task.id)
      } else {
        selectedTaskIds.delete(task.id)
      }
    })

    tbTaskList.appendChild(item)
  }

  tbSuiteName.textContent = `${suite.name} (${suite.tasks.length} tasks)`
  tbTaskSelector.classList.remove("hidden")
}

async function handleLoadSuite(): Promise<void> {
  const suitePath = tbSuitePathInput.value.trim()
  if (!suitePath) {
    updateTBStatus("No path", "error")
    return
  }

  try {
    updateTBStatus("Loading...")
    const suite = await loadTBSuiteRpc(suitePath)
    renderTaskList(suite)
    updateTBStatus("Ready")
  } catch (err) {
    console.error("[TB] Load failed:", err)
    updateTBStatus("Load failed", "error")
    tbTaskSelector.classList.add("hidden")
  }
}

async function handleStartRun(): Promise<void> {
  const suitePath = tbSuitePathInput.value.trim()
  if (!suitePath) {
    updateTBStatus("No path", "error")
    return
  }

  // Get selected task IDs (or all if none selected)
  const taskIds = selectedTaskIds.size > 0 ? Array.from(selectedTaskIds) : undefined

  try {
    updateTBStatus("Starting...", "running")
    updateTBButtons(true)

    await startTBRunRpc({
      suitePath,
      ...(taskIds !== undefined ? { taskIds } : {}),
    })

    updateTBStatus("Running...", "running")
  } catch (err) {
    console.error("[TB] Start failed:", err)
    updateTBStatus("Start failed", "error")
    updateTBButtons(false)
  }
}

async function handleStopRun(): Promise<void> {
  try {
    updateTBStatus("Stopping...")
    const stopped = await stopTBRunRpc()

    if (stopped) {
      updateTBStatus("Stopped")
    } else {
      updateTBStatus("No active run")
    }
    updateTBButtons(false)
  } catch (err) {
    console.error("[TB] Stop failed:", err)
    updateTBStatus("Stop failed", "error")
    updateTBButtons(false)
  }
}

function handleSelectAll(): void {
  const checkboxes = tbTaskList.querySelectorAll<HTMLInputElement>("input[type=checkbox]")
  checkboxes.forEach(cb => {
    cb.checked = true
    const taskId = cb.dataset.taskId
    if (taskId) selectedTaskIds.add(taskId)
  })
}

function handleSelectNone(): void {
  const checkboxes = tbTaskList.querySelectorAll<HTMLInputElement>("input[type=checkbox]")
  checkboxes.forEach(cb => {
    cb.checked = false
    const taskId = cb.dataset.taskId
    if (taskId) selectedTaskIds.delete(taskId)
  })
}

// Wire up button event handlers
tbLoadBtn.addEventListener("click", handleLoadSuite)
tbStartBtn.addEventListener("click", handleStartRun)
tbStopBtn.addEventListener("click", handleStopRun)
tbSelectAll.addEventListener("click", handleSelectAll)
tbSelectNone.addEventListener("click", handleSelectNone)

// View mode button handlers
document.getElementById("view-flow-btn")?.addEventListener("click", () => setViewMode("flow"))
document.getElementById("view-tb-btn")?.addEventListener("click", () => setViewMode("tbench"))

// ============================================================================
// Keyboard Shortcuts
// ============================================================================

document.addEventListener("keydown", (e) => {
  // Ignore if typing in input
  if (e.target instanceof HTMLInputElement) return

  // Ctrl+1: Flow view
  if (e.ctrlKey && e.key === "1") {
    e.preventDefault()
    setViewMode("flow")
    return
  }

  // Ctrl+2: TB view
  if (e.ctrlKey && e.key === "2") {
    e.preventDefault()
    setViewMode("tbench")
    return
  }

  // Ctrl+L: Load suite
  if (e.ctrlKey && e.key === "l") {
    e.preventDefault()
    handleLoadSuite()
    return
  }

  // Ctrl+T: Start run
  if (e.ctrlKey && e.key === "t") {
    e.preventDefault()
    if (!tbState.isRunning) {
      handleStartRun()
    }
    return
  }

  // Ctrl+X: Stop run (when not in input)
  if (e.ctrlKey && e.key === "x") {
    e.preventDefault()
    if (tbState.isRunning) {
      handleStopRun()
    }
    return
  }
})

// Expose for console access during development
declare global {
  interface Window {
    TB: {
      loadSuite: typeof loadTBSuiteRpc
      startRun: typeof startTBRunRpc
      stopRun: typeof stopTBRunRpc
      handleLoad: typeof handleLoadSuite
      handleStart: typeof handleStartRun
      handleStop: typeof handleStopRun
    }
  }
}

window.TB = {
  loadSuite: loadTBSuiteRpc,
  startRun: startTBRunRpc,
  stopRun: stopTBRunRpc,
  handleLoad: handleLoadSuite,
  handleStart: handleStartRun,
  handleStop: handleStopRun,
}

console.log("Flow HUD loaded with WebSocket support")
console.log("View modes: Ctrl+1 (Flow), Ctrl+2 (TB) | TB: Ctrl+L (load), Ctrl+T (start), Ctrl+X (stop)")

// ============================================================================
// TB Output Viewer
// ============================================================================

const outputViewer = document.getElementById("tb-output-viewer")
const outputContent = document.getElementById("tb-output-content")
const outputClearBtn = document.getElementById("tb-output-clear")
const outputCopyBtn = document.getElementById("tb-output-copy")
const outputCloseBtn = document.getElementById("tb-output-close")

function showOutputViewer(): void {
  outputViewer?.classList.remove("hidden")
}

function hideOutputViewer(): void {
  outputViewer?.classList.add("hidden")
}

function updateOutputViewer(): void {
  if (!outputContent) return

  // Show viewer when there's output during a run
  if (tbState.outputBuffer.length > 0 && tbState.isRunning) {
    showOutputViewer()
  }

  // Render last 100 lines to avoid DOM bloat
  const linesToShow = tbState.outputBuffer.slice(-100)
  const html = linesToShow.map(line => {
    const escaped = line.text
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
    return `<div class="tb-output-line ${line.source}">${escaped}</div>`
  }).join("")

  outputContent.innerHTML = html

  // Auto-scroll to bottom
  outputContent.scrollTop = outputContent.scrollHeight
}

function clearOutput(): void {
  tbState.outputBuffer = []
  if (outputContent) outputContent.innerHTML = ""
}

function copyOutput(): void {
  const text = tbState.outputBuffer.map(l => l.text).join("\n")
  navigator.clipboard.writeText(text).then(() => {
    console.log("[TB] Output copied to clipboard")
  })
}

// Wire up output viewer buttons
outputClearBtn?.addEventListener("click", clearOutput)
outputCopyBtn?.addEventListener("click", copyOutput)
outputCloseBtn?.addEventListener("click", hideOutputViewer)

// ============================================================================
// TB Category Tree
// ============================================================================

const categoryTree = document.getElementById("tb-category-tree")
const treeContent = document.getElementById("tb-tree-content")
const treeExpandBtn = document.getElementById("tb-tree-expand")
const treeCollapseBtn = document.getElementById("tb-tree-collapse")
const treeCloseBtn = document.getElementById("tb-tree-close")

// Track collapsed categories
const collapsedCategories = new Set<string>()

function showCategoryTree(): void {
  categoryTree?.classList.remove("hidden")
}

function hideCategoryTree(): void {
  categoryTree?.classList.add("hidden")
}

interface CategoryData {
  name: string
  tasks: TBTaskState[]
  passed: number
  failed: number
  total: number
}

function groupTasksByCategory(): Map<string, CategoryData> {
  const categories = new Map<string, CategoryData>()

  for (const task of tbState.tasks.values()) {
    const cat = task.category || "uncategorized"
    if (!categories.has(cat)) {
      categories.set(cat, { name: cat, tasks: [], passed: 0, failed: 0, total: 0 })
    }
    const catData = categories.get(cat)!
    catData.tasks.push(task)
    catData.total++
    if (task.status === "passed") catData.passed++
    if (task.status === "failed" || task.status === "error" || task.status === "timeout") {
      catData.failed++
    }
  }

  return categories
}

function getTaskStatusIcon(status: TBTaskStatus): string {
  switch (status) {
    case "passed": return "✓"
    case "failed": return "✗"
    case "error": return "⚠"
    case "timeout": return "⏱"
    case "running": return "▶"
    default: return "○"
  }
}

function renderCategoryTree(): void {
  if (!treeContent) return

  const categories = groupTasksByCategory()
  if (categories.size === 0) {
    treeContent.innerHTML = '<div style="padding: 12px; color: var(--text-muted); font-size: 11px;">No tasks loaded</div>'
    return
  }

  const categoryHtml = Array.from(categories.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([catName, catData]) => {
      const isCollapsed = collapsedCategories.has(catName)
      const tasksHtml = catData.tasks.map(task => {
        const icon = getTaskStatusIcon(task.status)
        const isRunning = task.status === "running"
        return `
          <div class="tb-tree-task ${task.status}${isRunning ? " running" : ""}" data-task-id="${task.id}">
            <span class="tb-task-status-icon ${task.status}">${icon}</span>
            <span class="tb-tree-task-name" title="${task.name}">${task.name}</span>
            ${task.difficulty ? `<span class="tb-tree-task-diff ${task.difficulty}">${task.difficulty.slice(0, 1).toUpperCase()}</span>` : ""}
          </div>
        `
      }).join("")

      const statsHtml = catData.passed > 0 || catData.failed > 0
        ? `<span class="tb-category-pass">✓${catData.passed}</span><span class="tb-category-fail">✗${catData.failed}</span>`
        : ""

      return `
        <div class="tb-category${isCollapsed ? " collapsed" : ""}" data-category="${catName}">
          <div class="tb-category-header">
            <span class="tb-category-chevron">▼</span>
            <span class="tb-category-name">${catName}</span>
            <div class="tb-category-stats">
              ${statsHtml}
              <span class="tb-category-count">${catData.total}</span>
            </div>
          </div>
          <div class="tb-category-tasks">
            ${tasksHtml}
          </div>
        </div>
      `
    }).join("")

  treeContent.innerHTML = categoryHtml

  // Add click handlers for category headers (toggle collapse)
  treeContent.querySelectorAll(".tb-category-header").forEach(header => {
    header.addEventListener("click", () => {
      const category = header.closest(".tb-category") as HTMLElement
      const catName = category?.dataset.category
      if (catName) {
        category.classList.toggle("collapsed")
        if (category.classList.contains("collapsed")) {
          collapsedCategories.add(catName)
        } else {
          collapsedCategories.delete(catName)
        }
      }
    })
  })

  // Add click handlers for tasks
  treeContent.querySelectorAll(".tb-tree-task").forEach(taskEl => {
    taskEl.addEventListener("click", () => {
      const taskId = (taskEl as HTMLElement).dataset.taskId
      if (taskId) {
        console.log("[TB] Task clicked:", taskId)
      }
    })
  })
}

function expandAllCategories(): void {
  collapsedCategories.clear()
  treeContent?.querySelectorAll(".tb-category").forEach(cat => {
    cat.classList.remove("collapsed")
  })
}

function collapseAllCategories(): void {
  const categories = groupTasksByCategory()
  for (const catName of categories.keys()) {
    collapsedCategories.add(catName)
  }
  treeContent?.querySelectorAll(".tb-category").forEach(cat => {
    cat.classList.add("collapsed")
  })
}

// Wire up tree controls
treeExpandBtn?.addEventListener("click", expandAllCategories)
treeCollapseBtn?.addEventListener("click", collapseAllCategories)
treeCloseBtn?.addEventListener("click", hideCategoryTree)

// Expose tree functions for triggering from handleHudMessage
;(window as unknown as Record<string, unknown>).__showCategoryTree = showCategoryTree
;(window as unknown as Record<string, unknown>).__renderCategoryTree = renderCategoryTree

export { renderTBDashboard }
