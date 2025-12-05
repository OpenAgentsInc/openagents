console.log("[OpenAgents] Script loading...");

import { calculateLayout } from "../flow/layout.js"
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
import type { TBRunWithPath } from "../tbench-hud/persistence.js"
// NOTE: persistence.js uses Node.js fs APIs which don't work in browser webview.
// Run history loading must happen via RPC from the main process.
// import { loadRecentRuns, loadTBRun } from "../tbench-hud/persistence.js"
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
import { getSocketClient } from "./socket-client.js"
import type {
  HudMessage,
  APMUpdateMessage,
  APMSnapshotMessage,
} from "../hud/protocol.js"
import type { TBRunHistoryItem } from "../desktop/protocol.js"
import {
  isTBRunStart,
  isTBRunComplete,
  isTBTaskStart,
  isTBTaskProgress,
  isTBTaskOutput,
  isTBTaskComplete,
  isTBRunHistory,
  isContainerStart,
  isContainerOutput,
  isContainerComplete,
  isContainerError,
  type ContainerStreamType,
} from "../hud/protocol.js"
import type { TBRunHistoryItem } from "../desktop/protocol.js"

// ============================================================================
// MC (MechaCoder) Tasks State
// ============================================================================

interface MCTaskState {
  id: string
  title: string
  description: string
  status: string
  priority: number
  type: string
  labels: string[]
  createdAt: string
  updatedAt: string
}

let mcTasks: MCTaskState[] = []
let mcTasksLoading = false
let mcTasksError: string | null = null
let mcTasksCollapsed = false
void mcTasks
void mcTasksError

async function loadMCTasks(): Promise<void> {
  if (mcTasksLoading) {
    console.log("[MC] Already loading, skipping")
    return
  }
  mcTasksLoading = true
  mcTasksError = null

  const t0 = performance.now()
  render() // Show loading state
  const t1 = performance.now()
  const loadingRenderTime = (t1 - t0).toFixed(2)
  console.log(`[MC] Loading state render took ${loadingRenderTime}ms`)
  window.bunLog?.(`[MC] Loading state render took ${loadingRenderTime}ms`)

  try {
    const t2 = performance.now()
    console.log("[MC] Loading ready tasks via RPC...")
    window.bunLog?.("[MC] Loading ready tasks via RPC...")
    const tasks = await socketClient.loadReadyTasks(20)
    const t3 = performance.now()
    const rpcTime = (t3 - t2).toFixed(2)
    console.log(`[MC] RPC took ${rpcTime}ms`)
    window.bunLog?.(`[MC] RPC took ${rpcTime}ms`)

    mcTasks = tasks
    console.log(`[MC] Loaded ${tasks.length} ready tasks`)
    window.bunLog?.(`[MC] Loaded ${tasks.length} ready tasks`)

    // Defer render to next animation frame to avoid blocking
    const t4 = performance.now()
    requestAnimationFrame(() => {
      render()
      const t5 = performance.now()
      const dataRenderTime = (t5 - t4).toFixed(2)
      console.log(`[MC] Data render took ${dataRenderTime}ms`)
      window.bunLog?.(`[MC] Data render took ${dataRenderTime}ms`)
      mcTasksLoading = false
    })
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err)
    console.error("[MC] Failed to load tasks:", errMsg)
    window.bunLog?.(`[MC] FAILED to load tasks: ${errMsg}`)
    mcTasksError = errMsg
    mcTasks = []
    render()
    mcTasksLoading = false
  }
}

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

// APM Widget commented out - not currently used
// function getAPMColor(apm: number): string {
//   if (apm >= 30) return "#f59e0b" // Gold - Elite
//   if (apm >= 15) return "#22c55e" // Green - High velocity
//   if (apm >= 5) return "#3b82f6" // Blue - Active
//   return "#6b7280" // Gray - Baseline
// }
//
// function renderAPMWidget(): string {
//   const color = getAPMColor(apmState.sessionAPM)
//   const efficiencyText = apmState.efficiencyRatio > 0
//     ? `${apmState.efficiencyRatio.toFixed(1)}x faster`
//     : ""
//   const deltaPercent = apmState.efficiencyRatio > 0
//     ? `+${((apmState.efficiencyRatio - 1) * 100).toFixed(0)}%`
//     : ""
//
//   return `
//     <g transform="translate(20, 20)" class="apm-widget">
//       ...
//     </g>
//   `
// }

// ============================================================================
// MC Tasks Widget
// ============================================================================

function getPriorityLabel(priority: number): string {
  switch (priority) {
    case 0: return "P0"
    case 1: return "P1"
    case 2: return "P2"
    case 3: return "P3"
    case 4: return "P4"
    default: return `P${priority}`
  }
}

function getPriorityClasses(priority: number): string {
  switch (priority) {
    case 0: return "bg-red-500/20 text-red-400 border-red-500/30" // Critical
    case 1: return "bg-amber-500/20 text-amber-400 border-amber-500/30" // High
    case 2: return "bg-blue-500/20 text-blue-400 border-blue-500/30" // Medium
    case 3: return "bg-zinc-500/20 text-zinc-400 border-zinc-500/30" // Low
    case 4: return "bg-zinc-600/20 text-zinc-500 border-zinc-600/30" // Backlog
    default: return "bg-zinc-500/20 text-zinc-400 border-zinc-500/30"
  }
}

function renderMCTasksWidget(): void {
  const t0 = performance.now()
  const widget = document.getElementById("mc-tasks-widget")
  if (!widget) return

  // Hide in TB mode
  if (viewMode !== "flow") {
    widget.classList.add("hidden")
    return
  }

  widget.classList.remove("hidden")
  const t1 = performance.now()
  const prepTime = (t1 - t0).toFixed(2)
  console.log(`[MC] Widget prep took ${prepTime}ms`)
  window.bunLog?.(`[MC] Widget prep took ${prepTime}ms`)

  // Loading state
  if (mcTasksLoading) {
    widget.innerHTML = `
      <div class="card fixed inset-x-4 top-4 p-6">
        <div class="text-violet-400 text-center font-mono">Loading ready tasks...</div>
      </div>
    `
    return
  }

  // Error state
  if (mcTasksError) {
    widget.innerHTML = `
      <div class="card fixed inset-x-4 top-4 p-6 border-destructive">
        <div class="text-destructive text-center font-mono">Error: ${mcTasksError.slice(0, 50)}</div>
      </div>
    `
    return
  }

  // Empty state
  if (mcTasks.length === 0) {
    widget.innerHTML = `
      <div class="card fixed inset-x-4 top-4 p-6">
        <div class="text-muted-foreground text-center font-mono">No ready tasks found</div>
      </div>
    `
    return
  }

  // Build task rows
  const taskRows = mcTasks.slice(0, 20).map((task) => {
    const prioClasses = getPriorityClasses(task.priority)
    const prioLabel = getPriorityLabel(task.priority)
    const labelStr = task.labels.slice(0, 2).join(", ")

    return `
      <tr>
        <td>
          <span class="inline-flex items-center px-1.5 py-0.5 text-[10px] font-bold rounded border ${prioClasses}">
            ${prioLabel}
          </span>
        </td>
        <td class="text-muted-foreground font-mono text-xs">${task.id}</td>
        <td class="font-medium font-mono" title="${task.title}">${task.title}</td>
        <td>
          <span class="text-violet-400 font-mono text-xs">${task.type}</span>
        </td>
        <td class="text-muted-foreground font-mono text-xs">${labelStr}</td>
        <td>
          <button class="btn btn-sm btn-primary" data-task-id="${task.id}" data-action="assign-mc">
            Assign
          </button>
        </td>
      </tr>
    `
  }).join("")

  widget.innerHTML = `
    <div class="card fixed inset-x-4 top-4 ${mcTasksCollapsed ? '' : 'max-h-[70vh]'} overflow-hidden">
      <!-- Header -->
      <div class="flex items-center justify-between px-4 py-3 border-b border-border cursor-pointer" data-action="toggle-mc-tasks">
        <h2 class="text-violet-400 font-bold font-mono text-lg">Ready Tasks (${mcTasks.length})</h2>
        <div class="flex items-center gap-3">
          <span class="text-muted-foreground text-xs font-mono">Ctrl+1 to refresh</span>
          <button class="text-muted-foreground hover:text-foreground transition-colors" title="${mcTasksCollapsed ? 'Expand' : 'Collapse'}">
            ${mcTasksCollapsed ? '▼' : '▲'}
          </button>
        </div>
      </div>

      <!-- Table -->
      <div class="overflow-x-auto max-h-[calc(70vh-60px)] overflow-y-auto ${mcTasksCollapsed ? 'hidden' : ''}">
        <table class="table">
          <thead>
            <tr>
              <th class="w-12">Pri</th>
              <th class="w-24">ID</th>
              <th>Title</th>
              <th class="w-20">Type</th>
              <th class="w-32">Labels</th>
              <th class="w-24">Action</th>
            </tr>
          </thead>
          <tbody>
            ${taskRows}
          </tbody>
        </table>
      </div>

      ${!mcTasksCollapsed && mcTasks.length > 20 ? `
      <div class="px-4 py-2 border-t border-border text-center">
        <span class="text-muted-foreground text-xs font-mono">+ ${mcTasks.length - 20} more tasks...</span>
      </div>
      ` : ""}
    </div>
  `
  const t2 = performance.now()
  const innerHTMLTime = (t2 - t1).toFixed(2)
  const totalTime = (t2 - t0).toFixed(2)
  console.log(`[MC] Widget innerHTML set took ${innerHTMLTime}ms, total: ${totalTime}ms`)
  window.bunLog?.(`[MC] Widget innerHTML set took ${innerHTMLTime}ms, total: ${totalTime}ms`)

  // Add click handlers for assign buttons (event delegation)
  widget.addEventListener("click", handleMCTaskAction)
}

async function handleMCTaskAction(e: Event): Promise<void> {
  const target = e.target as HTMLElement

  // Handle toggle collapse
  if (target.closest("[data-action='toggle-mc-tasks']")) {
    mcTasksCollapsed = !mcTasksCollapsed
    renderMCTasksWidget()
    return
  }

  // Handle assign button
  if (!target.matches("[data-action='assign-mc']")) return

  const taskId = target.dataset.taskId
  if (!taskId) return

  console.log(`[MC] Assigning task ${taskId} to MechaCoder with sandbox`)
  window.bunLog?.(`[MC] Assigning task ${taskId} to MechaCoder with sandbox`)

  try {
    // Disable button
    target.setAttribute("disabled", "true")
    target.textContent = "Starting..."

    // Call RPC to assign and start MechaCoder
    await socketClient.assignTaskToMC(taskId, { sandbox: true })

    // Update button
    target.textContent = "Assigned"
    target.classList.remove("btn-primary")
    target.classList.add("btn-success")

    console.log(`[MC] Task ${taskId} assigned successfully`)
    window.bunLog?.(`[MC] Task ${taskId} assigned successfully`)
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err)
    console.error(`[MC] Failed to assign task ${taskId}:`, errMsg)
    window.bunLog?.(`[MC] Failed to assign task ${taskId}: ${errMsg}`)

    // Reset button
    target.removeAttribute("disabled")
    target.textContent = "Assign"
    alert(`Failed to assign task: ${errMsg}`)
  }
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

/**
 * Comparison data between current run and a baseline
 */
interface TBComparison {
  baselineRunId: string
  baselineSuiteName: string
  baselineTimestamp: string
  baselinePassRate: number
  baselinePassed: number
  baselineFailed: number
  baselineTotalDurationMs: number
  // Deltas (positive = improvement for pass rate, negative = faster for duration)
  passRateDelta: number
  passedDelta: number
  failedDelta: number
  durationDelta: number
  // Task-level changes
  improved: string[]    // task IDs that went from fail->pass
  regressed: string[]   // task IDs that went from pass->fail
  unchanged: string[]   // task IDs with same outcome
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
  // Comparison with baseline
  comparison: TBComparison | null
  baselineRunId: string | null
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
  comparison: null,
  baselineRunId: null,
}

// ============================================================================
// View Mode State
// ============================================================================

type ViewMode = "flow" | "tbench"
// Default to MC (flow) mode
// Note: localStorage is blocked in about:blank context (webview setHTML)
let viewMode: ViewMode = "flow"
try {
  viewMode = (localStorage.getItem("hud-view-mode") as ViewMode) || "flow"
} catch {
  // localStorage blocked in webview context
}

function setViewMode(mode: ViewMode): void {
  console.log(`[View] setViewMode called: ${mode}`)
  window.bunLog?.(`[View] setViewMode called: ${mode}`)
  viewMode = mode
  try {
    localStorage.setItem("hud-view-mode", mode)
  } catch {
    // localStorage blocked in webview context
  }
  updateViewModeUI()
  render()

  // Load ready tasks when switching to MC view
  if (mode === "flow") {
    console.log("[View] Triggering loadMCTasks...")
    window.bunLog?.("[View] Triggering loadMCTasks...")
    void loadMCTasks()
  }
}

function updateViewModeUI(): void {
  const flowBtn = document.getElementById("view-flow-btn")
  const tbBtn = document.getElementById("view-tb-btn")
  if (flowBtn && tbBtn) {
    flowBtn.classList.toggle("active", viewMode === "flow")
    tbBtn.classList.toggle("active", viewMode === "tbench")
  }

  // TB controls only visible in TB view
  const tbControls = document.getElementById("tb-controls")
  if (tbControls) {
    tbControls.style.display = viewMode === "tbench" ? "block" : "none"
  }

  // Category tree only visible in TB view
  const categoryTree = document.getElementById("tb-category-tree")
  if (categoryTree) {
    categoryTree.style.display = viewMode === "tbench" ? "block" : "none"
  }
}

// Initialize view mode on load
setTimeout(updateViewModeUI, 0)

// ============================================================================
// Container Pane State
// ============================================================================

interface ContainerOutputLine {
  text: string
  stream: ContainerStreamType
  sequence: number
}

interface ContainerPane {
  executionId: string
  image: string
  command: string[]
  context: string
  sandboxed: boolean
  workdir: string
  status: "running" | "completed" | "error"
  exitCode?: number
  durationMs?: number
  outputLines: ContainerOutputLine[]
  startedAt: string
}

const containerPanes = new Map<string, ContainerPane>()
const MAX_LINES_PER_PANE = 500
const MAX_VISIBLE_PANES = 10

/**
 * Throttled container pane render (avoid excessive DOM updates)
 */
let containerRenderPending = false
function throttledContainerRender(): void {
  if (containerRenderPending) return
  containerRenderPending = true
  requestAnimationFrame(() => {
    renderContainerPanes()
    containerRenderPending = false
  })
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

// ============================================================================
// TB Comparison Functions
// ============================================================================

/**
 * Compute comparison between current run and a baseline run.
 * Returns comparison data with deltas and task-level changes.
 */
async function computeComparison(baselineRunId: string): Promise<TBComparison | null> {
  try {
    const details = await socketClient.loadTBRunDetails(baselineRunId)
    if (!details) {
      console.error(`[TB] Baseline run not found: ${baselineRunId}`)
      return null
    }

    const baseline = details.meta
    const baselineTasks = new Map(details.tasks.map(t => [t.id, t.outcome]))

    // Compute task-level changes
    const improved: string[] = []
    const regressed: string[] = []
    const unchanged: string[] = []

    for (const [taskId, task] of tbState.tasks) {
      const baselineOutcome = baselineTasks.get(taskId)
      if (!baselineOutcome) continue // New task, skip

      const currentPassed = task.status === "passed"
      const baselinePassed = baselineOutcome === "success"

      if (currentPassed && !baselinePassed) {
        improved.push(taskId)
      } else if (!currentPassed && baselinePassed) {
        regressed.push(taskId)
      } else {
        unchanged.push(taskId)
      }
    }

    return {
      baselineRunId: baseline.runId,
      baselineSuiteName: baseline.suiteName,
      baselineTimestamp: baseline.timestamp,
      baselinePassRate: baseline.passRate,
      baselinePassed: baseline.passed,
      baselineFailed: baseline.failed,
      baselineTotalDurationMs: baseline.totalDurationMs,
      passRateDelta: tbState.passRate - baseline.passRate,
      passedDelta: tbState.passed - baseline.passed,
      failedDelta: tbState.failed - baseline.failed,
      durationDelta: tbState.totalDurationMs - baseline.totalDurationMs,
      improved,
      regressed,
      unchanged,
    }
  } catch (err) {
    console.error(`[TB] Failed to compute comparison:`, err)
    return null
  }
}

/**
 * Set baseline for comparison and compute comparison data.
 */
async function setBaseline(runId: string): Promise<void> {
  tbState.baselineRunId = runId
  tbState.comparison = await computeComparison(runId)
  render()
}

/**
 * Clear comparison baseline.
 */
function clearBaseline(): void {
  tbState.baselineRunId = null
  tbState.comparison = null
  render()
}

/**
 * Format a delta value with sign and color indicator.
 */
function formatDelta(value: number, invert = false): { text: string; color: string } {
  const improved = invert ? value < 0 : value > 0
  const sign = value > 0 ? "+" : ""
  return {
    text: `${sign}${value.toFixed(1)}`,
    color: improved ? "#22c55e" : value < 0 ? "#ef4444" : "#6b7280",
  }
}

/**
 * Render comparison widget showing delta from baseline.
 */
function renderComparisonWidget(): string {
  if (!tbState.comparison) return ""

  const comp = tbState.comparison
  const passRateDelta = formatDelta(comp.passRateDelta * 100) // Convert to percentage
  const durationDelta = formatDelta(comp.durationDelta / 1000, true) // Convert to seconds, invert (negative is better)

  // Format baseline timestamp
  const baselineDate = new Date(comp.baselineTimestamp)
  const baselineStr = baselineDate.toLocaleDateString("en-US", { month: "short", day: "numeric" })

  return `
    <g transform="translate(20, 245)" class="tb-comparison-widget">
      <!-- Background -->
      <rect x="0" y="0" width="260" height="85" rx="8" ry="8"
            fill="#141017" stroke="rgba(59, 130, 246, 0.25)" stroke-width="1"/>

      <!-- Header -->
      <text x="16" y="20" fill="#3b82f6" font-size="12" font-weight="bold" font-family="Berkeley Mono, monospace">
        vs ${comp.baselineSuiteName} (${baselineStr})
      </text>

      <!-- Pass rate delta -->
      <text x="16" y="42" fill="#9ca3af" font-size="11" font-family="Berkeley Mono, monospace">
        Pass Rate:
      </text>
      <text x="90" y="42" fill="${passRateDelta.color}" font-size="11" font-weight="bold" font-family="Berkeley Mono, monospace">
        ${passRateDelta.text}%
      </text>

      <!-- Duration delta -->
      <text x="140" y="42" fill="#9ca3af" font-size="11" font-family="Berkeley Mono, monospace">
        Time:
      </text>
      <text x="180" y="42" fill="${durationDelta.color}" font-size="11" font-weight="bold" font-family="Berkeley Mono, monospace">
        ${durationDelta.text}s
      </text>

      <!-- Task changes -->
      <text x="16" y="62" fill="#22c55e" font-size="10" font-family="Berkeley Mono, monospace">
        ▲ ${comp.improved.length} improved
      </text>
      <text x="100" y="62" fill="#ef4444" font-size="10" font-family="Berkeley Mono, monospace">
        ▼ ${comp.regressed.length} regressed
      </text>
      <text x="195" y="62" fill="#6b7280" font-size="10" font-family="Berkeley Mono, monospace">
        = ${comp.unchanged.length}
      </text>

      <!-- Click hint -->
      <text x="16" y="78" fill="#4b5563" font-size="9" font-family="Berkeley Mono, monospace">
        Click for details • Ctrl+B to clear
      </text>
    </g>
  `
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

// Note: Types defined locally to avoid importing from desktop/protocol.ts (not browser-compatible)

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

  if (isTBRunHistory(message)) {
    applyTBRunHistory(message.runs)
    return
  }

  // Handle Terminal-Bench messages
  if (isTBRunStart(message)) {
    // Preserve baseline for comparison across runs
    const preservedBaseline = tbState.baselineRunId
    const preservedComparison = tbState.comparison

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
      // Preserve baseline so user can compare current run against it
      baselineRunId: preservedBaseline,
      comparison: preservedComparison,
    }
    upsertTBRunHistoryItem({
      runId: message.runId,
      suiteName: message.suiteName,
      suiteVersion: message.suiteVersion,
      timestamp: message.timestamp,
      passRate: 0,
      passed: 0,
      failed: 0,
      timeout: 0,
      error: 0,
      totalDurationMs: 0,
      totalTokens: 0,
      taskCount: message.totalTasks,
      filepath: "",
    })
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
    const existingRun = tbFlowState.runs.find((run) => run.runId === message.runId)
    upsertTBRunHistoryItem({
      runId: message.runId,
      suiteName: existingRun?.suiteName ?? tbState.suiteName,
      suiteVersion: existingRun?.suiteVersion ?? tbState.suiteVersion,
      timestamp: existingRun?.timestamp ?? new Date().toISOString(),
      passRate: message.passRate,
      passed: message.passed,
      failed: message.failed,
      timeout: message.timeout,
      error: message.error,
      totalDurationMs: message.totalDurationMs,
      totalTokens: existingRun?.totalTokens ?? 0,
      taskCount: existingRun?.taskCount ?? tbState.totalTasks,
      filepath: existingRun?.filepath ?? "",
    })
    // Sync flow state (run no longer active)
    syncTBFlowWithState()
    render()
    // Sync UI controls
    document.getElementById("tb-status")!.textContent = `Done ${(message.passRate * 100).toFixed(0)}%`
    document.getElementById("tb-status")!.className = "tb-status"
    ;(document.getElementById("tb-start-btn") as HTMLButtonElement).disabled = false
    ;(document.getElementById("tb-stop-btn") as HTMLButtonElement).disabled = true
    // Recompute comparison if baseline is set
    if (tbState.baselineRunId) {
      void computeComparison(tbState.baselineRunId).then(comp => {
        tbState.comparison = comp
        render()
      })
    }
    return
  }

  // Handle Container execution messages
  if (isContainerStart(message)) {
    containerPanes.set(message.executionId, {
      executionId: message.executionId,
      image: message.image,
      command: message.command,
      context: message.context,
      sandboxed: message.sandboxed,
      workdir: message.workdir,
      status: "running",
      outputLines: [],
      startedAt: message.timestamp,
    })
    renderContainerPanes()
    return
  }

  if (isContainerOutput(message)) {
    const pane = containerPanes.get(message.executionId)
    if (pane) {
      pane.outputLines.push({
        text: message.text,
        stream: message.stream,
        sequence: message.sequence,
      })
      // Trim if too large
      if (pane.outputLines.length > MAX_LINES_PER_PANE) {
        pane.outputLines = pane.outputLines.slice(-MAX_LINES_PER_PANE)
      }
      throttledContainerRender()
    }
    return
  }

  if (isContainerComplete(message)) {
    const pane = containerPanes.get(message.executionId)
    if (pane) {
      pane.status = "completed"
      pane.exitCode = message.exitCode
      pane.durationMs = message.durationMs
      renderContainerPanes()
    }
    return
  }

  if (isContainerError(message)) {
    const pane = containerPanes.get(message.executionId)
    if (pane) {
      pane.status = "error"
      pane.outputLines.push({
        text: `[ERROR] ${message.reason}: ${message.error}`,
        stream: "stderr",
        sequence: pane.outputLines.length,
      })
      renderContainerPanes()
    }
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
// Polling interval for flow layout fallback refresh (60s - rely on WebSocket events for live updates)
const REFRESH_INTERVAL_MS = 60000

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

const mapHistoryItemToRun = (item: TBRunHistoryItem): TBRunWithPath => ({
  runId: item.runId,
  suiteName: item.suiteName,
  suiteVersion: item.suiteVersion,
  timestamp: item.timestamp,
  passRate: item.passRate,
  passed: item.passed,
  failed: item.failed,
  timeout: item.timeout,
  error: item.error,
  totalDurationMs: item.totalDurationMs,
  totalTokens: item.totalTokens,
  taskCount: item.taskCount,
  filepath: item.filepath ?? "",
})

const applyTBRunList = (runs: TBRunWithPath[], forceRender = false): void => {
  const sortedRuns = [...runs].sort(
    (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
  )
  const nextExpanded = new Set(
    [...tbFlowState.expandedRunIds].filter((id) =>
      sortedRuns.some((run) => run.runId === id)
    )
  )
  tbRunDetails = new Map(
    [...tbRunDetails].filter(([runId]) =>
      sortedRuns.some((run) => run.runId === runId)
    )
  )
  tbFlowState = {
    ...tbFlowState,
    runs: sortedRuns,
    expandedRunIds: nextExpanded,
  }
  syncTBFlowWithState()
  if (viewMode === "tbench" || forceRender) {
    render()
  }
}

const upsertTBRunHistoryItem = (
  item: TBRunHistoryItem,
  forceRender = false
): void => {
  const run = mapHistoryItemToRun(item)
  const existingIndex = tbFlowState.runs.findIndex(
    (r) => r.runId === run.runId
  )
  const nextRuns =
    existingIndex >= 0
      ? [
          ...tbFlowState.runs.slice(0, existingIndex),
          run,
          ...tbFlowState.runs.slice(existingIndex + 1),
        ]
      : [run, ...tbFlowState.runs]
  applyTBRunList(nextRuns, forceRender)
}

/**
 * Refresh TB flow layout from current state and load run history via RPC.
 * Uses RPC to load run history since browser webview cannot use Node.js fs APIs.
 */
async function refreshTBLayout(): Promise<void> {
  try {
    // Load recent runs via WebSocket RPC
    const runs = await socketClient.loadRecentTBRuns(20)
    console.log(`[TB] Loaded ${runs.length} runs via RPC`)

    applyTBRunList(runs.map(mapHistoryItemToRun))
  } catch (err) {
    console.error("[TB] Failed to load runs via RPC:", err)
    syncTBFlowWithState()
    if (viewMode === "tbench") {
      render()
    }
  }
}

/**
 * Toggle expansion of a run node.
 * Loads run details via RPC when expanding.
 */
async function handleRunNodeClick(runId: string): Promise<void> {
  const wasExpanded = tbFlowState.expandedRunIds.has(runId)

  // Toggle expansion
  tbFlowState = toggleRunExpanded(tbFlowState, runId)

  // Load run details via RPC when expanding (if not already loaded)
  if (!wasExpanded && !tbRunDetails.has(runId)) {
    try {
      console.log(`[TB] Loading details for run: ${runId}`)
      const details = await socketClient.loadTBRunDetails(runId)

      if (details) {
        // Convert to TBRunDetails format expected by tb-map.ts
        // TBRunDetails = { meta: TBRunMeta, tasks: TBTaskResult[] }
        tbRunDetails.set(runId, {
          meta: {
            runId: details.meta.runId,
            suiteName: details.meta.suiteName,
            suiteVersion: details.meta.suiteVersion,
            timestamp: details.meta.timestamp,
            passRate: details.meta.passRate,
            passed: details.meta.passed,
            failed: details.meta.failed,
            timeout: details.meta.timeout,
            error: details.meta.error,
            totalDurationMs: details.meta.totalDurationMs,
            totalTokens: details.meta.totalTokens,
            taskCount: details.meta.taskCount,
          },
          tasks: details.tasks.map((t) => ({
            id: t.id,
            name: t.name,
            category: t.category,
            difficulty: t.difficulty as "easy" | "medium" | "hard" | "expert",
            outcome: t.outcome as "success" | "failure" | "timeout" | "error",
            durationMs: t.durationMs,
            turns: t.turns,
            tokens: t.tokens,
            ...(t.outputLines !== undefined ? { outputLines: t.outputLines } : {}),
          })),
        })
        console.log(`[TB] Loaded ${details.tasks.length} tasks for run ${runId}`)
      }
    } catch (err) {
      console.error(`[TB] Failed to load run details for ${runId}:`, err)
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
  // NOTE: MechaCoder state loading disabled in browser context.
  // The browser webview cannot access the filesystem directly.
  // State is now received via WebSocket HUD messages.
  // This function is a no-op; the initial layout uses sample data.
  if (isRefreshing) return
  isRefreshing = true
  try {
    // Use sample data for now - live data comes via WebSocket
    if (!hasLiveLayout) {
      const recentered = getCenteredPan(canvasState.viewportWidth, canvasState.viewportHeight)
      canvasState = { ...canvasState, ...recentered }
      hasLiveLayout = true
      render()
    }
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
const tbRandomBtn = document.getElementById("tb-random-btn")!
const tbStopBtn = document.getElementById("tb-stop-btn")!
const tbStatus = document.getElementById("tb-status")!
const tbTaskSelector = document.getElementById("tb-task-selector")!
const tbSuiteName = document.getElementById("tb-suite-name")!
const tbTaskList = document.getElementById("tb-task-list")!
const tbSelectAll = document.getElementById("tb-select-all")!
const tbSelectNone = document.getElementById("tb-select-none")!

// TB UI State
let selectedTaskIds: Set<string> = new Set()
let loadedSuite: TBSuiteInfo | null = null

// Initialize canvas state with viewport size
let canvasState = initialCanvasState(window.innerWidth, window.innerHeight)
const initialPan = getCenteredPan(window.innerWidth, window.innerHeight)
canvasState = { ...canvasState, ...initialPan }

// Render SVG content
function render(): void {
  const r0 = performance.now()
  if (viewMode === "flow") {
    // Flow view: MechaCoder graph (SVG) + MC Tasks widget (HTML/Tailwind)
    const r1 = performance.now()
    const flowGroup = renderFlowSVG(layout, canvasState, DEFAULT_RENDER_CONFIG)
    const r2 = performance.now()
    console.log(`[Render] renderFlowSVG took ${(r2 - r1).toFixed(2)}ms`)
    window.bunLog?.(`[Render] renderFlowSVG took ${(r2 - r1).toFixed(2)}ms`)

    svg.innerHTML = svgElementToString(flowGroup)
    const r3 = performance.now()
    console.log(`[Render] svgElementToString + innerHTML took ${(r3 - r2).toFixed(2)}ms`)
    window.bunLog?.(`[Render] svgElementToString + innerHTML took ${(r3 - r2).toFixed(2)}ms`)

    // Render MC tasks as HTML (not SVG)
    renderMCTasksWidget()
    const r4 = performance.now()
    console.log(`[Render] Total render took ${(r4 - r0).toFixed(2)}ms`)
    window.bunLog?.(`[Render] Total render took ${(r4 - r0).toFixed(2)}ms`)
  } else {
    // TB view: TB flow tree (run history nodes) on grid canvas + TB widget + comparison
    const tbFlowGroup = renderFlowSVG(tbLayout, canvasState, DEFAULT_RENDER_CONFIG)
    const tbOverlay = renderTBWidget()
    const comparisonOverlay = renderComparisonWidget()
    svg.innerHTML = svgElementToString(tbFlowGroup) + tbOverlay + comparisonOverlay
    // Hide MC tasks widget in TB mode
    renderMCTasksWidget()
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

// Click handler for node interactions (TB run expand/collapse, baseline selection)
svg.addEventListener("click", (e) => {
  // Only handle clicks when not dragging
  if (canvasState.isDragging) return

  // Find clicked node by checking data-node-id on rect elements
  const target = e.target as SVGElement
  const nodeRect = target.closest("[data-node-id]") as SVGElement | null
  if (!nodeRect) return

  const nodeId = nodeRect.getAttribute("data-node-id")
  if (!nodeId) return

  // Handle TB run node clicks
  if (nodeId.startsWith("tb-run-")) {
    const runId = nodeId.replace("tb-run-", "").replace("expanded-", "")

    // Shift+click: Set as baseline for comparison
    if (e.shiftKey) {
      console.log(`[TB] Setting baseline: ${runId}`)
      void setBaseline(runId)
      return
    }

    // Regular click: Expand/collapse
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
console.log("[OpenAgents] About to render...");
render()
console.log("[OpenAgents] Render complete!");

// Load live data; TB history is refreshed from WebSocket events instead of polling
void refreshLayoutFromState()
setInterval(refreshLayoutFromState, REFRESH_INTERVAL_MS)

// ============================================================================
// Socket Client Setup for Real-time HUD Events
// ============================================================================

// Get the shared socket client and connect
// Note: WebSocket to localhost DOES work from setHTML() content,
// only navigate() to localhost HTTP is blocked by WebKit
const socketClient = getSocketClient({ verbose: true })

// Connect and set up HUD message handler
window.bunLog?.("[Socket] Attempting to connect...")
socketClient.connect().then(() => {
  window.bunLog?.("[Socket] Connected to desktop server!")
  console.log("[Socket] Connected to desktop server")

  // Auto-load MC tasks if in flow mode
  if (viewMode === "flow") {
    void loadMCTasks()
  }

  // Fetch initial TB run history (subsequent updates arrive via push)
  void refreshTBLayout()
}).catch((err) => {
  const errMsg = err instanceof Error ? err.message : String(err)
  window.bunLog?.("[Socket] FAILED to connect:", errMsg)
  console.error("[Socket] Failed to connect:", err)
})

// Handle HUD messages from agents
socketClient.onMessage((message: HudMessage) => {
  handleHudMessage(message)
})

// ============================================================================
// TB Run Controls (exposed for UI interaction)
// ============================================================================

/** Load a TB suite via WebSocket RPC */
async function loadTBSuiteRpc(suitePath: string): Promise<TBSuiteInfo> {
  console.log("[TB] Loading suite:", suitePath)
  return await socketClient.loadTBSuite(suitePath)
}

/** Start a TB run with the given options */
async function startTBRunRpc(options: TBRunOptions): Promise<string> {
  console.log("[TB] Starting run:", options)
  const { runId } = await socketClient.startTBRun(options)
  console.log("[TB] Run started:", runId)
  return runId
}

/** Stop the current TB run */
async function stopTBRunRpc(): Promise<boolean> {
  console.log("[TB] Stopping run")
  const { stopped } = await socketClient.stopTBRun()
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
  ;(tbRandomBtn as HTMLButtonElement).disabled = isRunning
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
    loadedSuite = suite  // Store for random task selection
    renderTaskList(suite)
    updateTBStatus("Ready")
    // Enable random button when suite is loaded
    ;(tbRandomBtn as HTMLButtonElement).disabled = false
  } catch (err) {
    console.error("[TB] Load failed:", err)
    updateTBStatus("Load failed", "error")
    loadedSuite = null
    tbTaskSelector.classList.add("hidden")
    ;(tbRandomBtn as HTMLButtonElement).disabled = true
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

async function handleStartRandomTask(): Promise<void> {
  console.log("[TB] Random button clicked!")
  // Log to terminal via bound bunLog function
  if (typeof window.bunLog === "function") {
    window.bunLog("[TB] Random button clicked!")
  }

  const suitePath = tbSuitePathInput.value.trim()
  console.log("[TB] Suite path:", suitePath)
  if (!suitePath) {
    console.log("[TB] No path provided")
    updateTBStatus("No path", "error")
    return
  }

  // Load suite if not already loaded
  if (!loadedSuite) {
    try {
      updateTBStatus("Loading...")
      window.bunLog?.("[TB] Calling loadTBSuiteRpc with path:", suitePath)
      loadedSuite = await loadTBSuiteRpc(suitePath)
      window.bunLog?.("[TB] loadTBSuiteRpc succeeded:", JSON.stringify(loadedSuite).slice(0, 200))
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err)
      window.bunLog?.("[TB] loadTBSuiteRpc FAILED:", errMsg)
      console.error("[TB] Load failed:", err)
      updateTBStatus("Load failed", "error")
      return
    }
  }

  if (loadedSuite.tasks.length === 0) {
    updateTBStatus("No tasks", "error")
    return
  }

  // Pick a random task
  const randomIndex = Math.floor(Math.random() * loadedSuite.tasks.length)
  const randomTask = loadedSuite.tasks[randomIndex]
  console.log(`[TB] Starting random task: ${randomTask.name} (${randomTask.id})`)

  try {
    updateTBStatus(`Random: ${randomTask.name}`, "running")
    updateTBButtons(true)

    await startTBRunRpc({
      suitePath,
      taskIds: [randomTask.id],
    })

    updateTBStatus("Running...", "running")
  } catch (err) {
    console.error("[TB] Start random failed:", err)
    updateTBStatus("Start failed", "error")
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
tbRandomBtn.addEventListener("click", handleStartRandomTask)
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

  // Ctrl+R: Start random task
  if (e.ctrlKey && e.key === "r") {
    e.preventDefault()
    if (!tbState.isRunning) {
      handleStartRandomTask()
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

  // Ctrl+B: Clear baseline comparison
  if (e.ctrlKey && e.key === "b") {
    e.preventDefault()
    clearBaseline()
    return
  }
})

// Expose for console access during development
declare global {
  interface Window {
    bunLog: (...args: unknown[]) => void
    TB: {
      loadSuite: typeof loadTBSuiteRpc
      startRun: typeof startTBRunRpc
      stopRun: typeof stopTBRunRpc
      handleLoad: typeof handleLoadSuite
      handleStart: typeof handleStartRun
      handleRandom: typeof handleStartRandomTask
      handleStop: typeof handleStopRun
      setBaseline: typeof setBaseline
      clearBaseline: typeof clearBaseline
    }
  }
}

window.TB = {
  loadSuite: loadTBSuiteRpc,
  startRun: startTBRunRpc,
  stopRun: stopTBRunRpc,
  handleLoad: handleLoadSuite,
  handleStart: handleStartRun,
  handleRandom: handleStartRandomTask,
  handleStop: handleStopRun,
  setBaseline,
  clearBaseline,
}

console.log("Flow HUD loaded with WebSocket support")
console.log("View modes: Ctrl+1 (Flow), Ctrl+2 (TB) | TB: Ctrl+L (load), Ctrl+T (start), Ctrl+R (random), Ctrl+X (stop)")
console.log("Comparison: Shift+click run to set baseline, Ctrl+B to clear")

// Log to terminal that JS loaded successfully
if (typeof window.bunLog === "function") {
  window.bunLog("[Mainview] JS loaded and initialized!")
}

// ============================================================================
// Container Panes Rendering
// ============================================================================

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
}

function renderContainerPanes(): void {
  const container = document.getElementById("container-panes")
  if (!container) return

  // Get panes sorted by start time (most recent first)
  const panes = Array.from(containerPanes.values())
    .sort((a, b) => b.startedAt.localeCompare(a.startedAt))
    .slice(0, MAX_VISIBLE_PANES)

  if (panes.length === 0) {
    container.classList.add("hidden")
    return
  }

  container.classList.remove("hidden")

  container.innerHTML = panes.map(pane => {
    const statusClass = pane.status
    const statusIcon = pane.status === "running" ? "▶"
      : pane.status === "completed" && pane.exitCode === 0 ? "✓"
      : "✗"
    const statusColor = pane.status === "running" ? "#3b82f6"
      : pane.exitCode === 0 ? "#22c55e" : "#ef4444"

    const badge = pane.sandboxed
      ? '<span class="container-badge sandboxed">sandbox</span>'
      : '<span class="container-badge host">host</span>'

    const duration = pane.durationMs
      ? `<span class="container-duration">${(pane.durationMs / 1000).toFixed(1)}s</span>`
      : ""

    const exitCode = pane.exitCode !== undefined
      ? `<span class="container-exit-code ${pane.exitCode === 0 ? 'success' : 'failure'}">${pane.exitCode}</span>`
      : ""

    // Render output lines (last 100)
    const outputHtml = pane.outputLines.slice(-100).map(line => {
      const escaped = escapeHtml(line.text)
      const streamClass = line.stream === "stderr" ? "stderr" : "stdout"
      return `<div class="container-output-line ${streamClass}">${escaped}</div>`
    }).join("")

    // Truncate command display
    const cmdDisplay = pane.command.join(" ").slice(0, 60) + (pane.command.join(" ").length > 60 ? "..." : "")

    return `
      <div class="container-pane ${statusClass}" data-execution-id="${pane.executionId}">
        <div class="container-pane-header">
          <span class="container-status" style="color: ${statusColor}">${statusIcon}</span>
          <span class="container-image">${pane.image}</span>
          ${badge}
          ${duration}
          ${exitCode}
        </div>
        <div class="container-pane-command" title="${escapeHtml(pane.command.join(" "))}">${escapeHtml(cmdDisplay)}</div>
        <div class="container-pane-output">${outputHtml}</div>
      </div>
    `
  }).join("")

  // Auto-scroll each pane's output
  container.querySelectorAll(".container-pane-output").forEach(el => {
    (el as HTMLElement).scrollTop = (el as HTMLElement).scrollHeight
  })
}

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
