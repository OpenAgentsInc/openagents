/**
 * MC Tasks Module
 *
 * Displays MechaCoder ready tasks in a widget with task assignment functionality.
 * Supports priority-based styling, task filtering, and collapse/expand.
 */

import type { MCTaskState, ViewMode } from "./shared-types.js"
import type { SocketClient } from "./socket-client.js"

// ============================================================================
// State
// ============================================================================

let mcTasks: MCTaskState[] = []
let mcTasksLoading = false
let mcTasksError: string | null = null
let mcTasksCollapsed = false

// ============================================================================
// Dependencies (injected during init)
// ============================================================================

let socketClient: SocketClient | null = null
let renderCallback: (() => void) | null = null
let getViewMode: (() => ViewMode) | null = null

// ============================================================================
// Public API
// ============================================================================

/**
 * Load ready tasks from the server
 */
export async function loadMCTasks(): Promise<void> {
  if (mcTasksLoading) {
    console.log("[MC] Already loading, skipping")
    return
  }
  if (!socketClient || !renderCallback) {
    console.error("[MC] Socket client or render callback not initialized")
    return
  }

  mcTasksLoading = true
  mcTasksError = null

  const t0 = performance.now()
  renderCallback() // Show loading state
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
      if (renderCallback) renderCallback()
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
    if (renderCallback) renderCallback()
    mcTasksLoading = false
  }
}

/**
 * Render the MC tasks widget
 */
export function renderMCTasksWidget(): void {
  const t0 = performance.now()
  const widget = document.getElementById("mc-tasks-widget")
  if (!widget) return

  // Hide in TB mode
  const viewMode = getViewMode?.() || "flow"
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
      <div class="fixed inset-x-4 top-4 rounded-2xl border border-zinc-800/60 bg-zinc-950/80 px-6 py-5 shadow-2xl backdrop-blur-xl">
        <div class="text-zinc-200 text-center font-mono text-sm">Loading ready tasks...</div>
      </div>
    `
    return
  }

  // Error state
  if (mcTasksError) {
    widget.innerHTML = `
      <div class="fixed inset-x-4 top-4 rounded-2xl border border-zinc-700/80 bg-zinc-950/80 px-6 py-5 shadow-2xl backdrop-blur-xl">
        <div class="text-zinc-400 text-center font-mono text-sm">
          Error: ${mcTasksError.slice(0, 50)}
        </div>
      </div>
    `
    return
  }

  // Empty state
  if (mcTasks.length === 0) {
    widget.innerHTML = `
      <div class="fixed inset-x-4 top-4 rounded-2xl border border-zinc-800/60 bg-zinc-950/70 px-6 py-5 shadow-2xl backdrop-blur-xl">
        <div class="text-zinc-400 text-center font-mono text-sm">No ready tasks found</div>
      </div>
    `
    return
  }

  const assignButtonClass = "inline-flex items-center justify-center border border-zinc-700 px-3 py-1 text-[10px] font-mono font-semibold tracking-[0.25em] uppercase rounded text-zinc-50 bg-zinc-900/80 transition-colors hover:bg-zinc-900/95"

  // Build task rows
  const taskRows = mcTasks.slice(0, 20).map((task) => {
    const prioClasses = getPriorityClasses(task.priority)
    const prioLabel = getPriorityLabel(task.priority)
    const labelStr = task.labels.slice(0, 2).join(", ")

    return `
      <tr class="border-b border-zinc-800 last:border-0">
        <td class="py-2">
          <span class="inline-flex items-center px-1.5 py-0.5 text-[10px] font-bold rounded border ${prioClasses}">
            ${prioLabel}
          </span>
        </td>
        <td class="text-zinc-400 font-mono text-[10px]">${task.id}</td>
        <td class="font-medium font-mono text-zinc-100" title="${task.title}">${task.title}</td>
        <td>
          <span class="text-zinc-200 font-mono text-xs">${task.type}</span>
        </td>
        <td class="text-zinc-400 font-mono text-xs">${labelStr}</td>
        <td>
          <button class="${assignButtonClass}" data-task-id="${task.id}" data-action="assign-mc">
            Assign
          </button>
        </td>
      </tr>
    `
  }).join("")

  const containerClasses = [
    "fixed inset-x-4 top-4 overflow-hidden rounded-[26px] border border-zinc-800/60 bg-zinc-950/80 shadow-2xl backdrop-blur-xl text-zinc-200",
    mcTasksCollapsed ? "" : "max-h-[70vh]",
  ].filter(Boolean).join(" ")

  widget.innerHTML = `
    <div class="${containerClasses}">
      <div class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer" data-action="toggle-mc-tasks">
        <h2 class="text-zinc-100 font-bold font-mono text-lg">Ready Tasks (${mcTasks.length})</h2>
        <div class="flex items-center gap-3">
          <span class="text-xs font-mono text-zinc-500">Ctrl+1 to refresh</span>
          <button class="text-xs font-mono text-zinc-400 transition-colors hover:text-zinc-200" title="${mcTasksCollapsed ? "Expand" : "Collapse"}">
            ${mcTasksCollapsed ? "▼" : "▲"}
          </button>
        </div>
      </div>

      <div class="${mcTasksCollapsed ? "hidden" : "overflow-x-auto max-h-[calc(70vh-60px)] overflow-y-auto"}">
        <table class="min-w-full table-auto text-xs font-mono text-zinc-200">
          <thead>
            <tr class="text-zinc-500 uppercase text-[9px] tracking-[0.4em]">
              <th class="w-12 px-3 py-2">Pri</th>
              <th class="w-24 px-3 py-2">ID</th>
              <th class="px-3 py-2 text-left">Title</th>
              <th class="w-20 px-3 py-2">Type</th>
              <th class="w-32 px-3 py-2">Labels</th>
              <th class="w-24 px-3 py-2">Action</th>
            </tr>
          </thead>
          <tbody>
            ${taskRows}
          </tbody>
        </table>
      </div>

      ${!mcTasksCollapsed && mcTasks.length > 20 ? `
      <div class="px-4 py-2 border-t border-zinc-800/60 text-center text-xs font-mono text-zinc-500">
        + ${mcTasks.length - 20} more tasks...
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

// ============================================================================
// Event Handlers
// ============================================================================

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
  if (!taskId || !socketClient) return

  console.log(`[MC] Assigning task ${taskId} to MechaCoder with sandbox`)
  window.bunLog?.(`[MC] Assigning task ${taskId} to MechaCoder with sandbox`)

  try {
    // Disable button
    target.setAttribute("disabled", "true")
    target.classList.add("cursor-not-allowed", "opacity-60")
    target.textContent = "Starting..."

    // Call RPC to assign and start MechaCoder
    await socketClient.assignTaskToMC(taskId, { sandbox: true })

    // Update button
    target.textContent = "Assigned"

    console.log(`[MC] Task ${taskId} assigned successfully`)
    window.bunLog?.(`[MC] Task ${taskId} assigned successfully`)
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err)
    console.error(`[MC] Failed to assign task ${taskId}:`, errMsg)
    window.bunLog?.(`[MC] Failed to assign task ${taskId}: ${errMsg}`)

    // Reset button
    target.removeAttribute("disabled")
    target.classList.remove("cursor-not-allowed", "opacity-60")
    target.textContent = "Assign"
  }
}

// ============================================================================
// Utilities
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
    case 0:
      return "bg-zinc-950/70 text-zinc-50 border-zinc-700/60"
    case 1:
      return "bg-zinc-900/60 text-zinc-200 border-zinc-600/50"
    case 2:
      return "bg-zinc-900/40 text-zinc-200 border-zinc-500/40"
    case 3:
      return "bg-zinc-800/30 text-zinc-300 border-zinc-500/30"
    case 4:
      return "bg-zinc-800/20 text-zinc-300 border-zinc-600/30"
    default:
      return "bg-zinc-900/40 text-zinc-300 border-zinc-500/30"
  }
}

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize MC tasks module
 * @param client - Socket client for loading tasks
 * @param render - Render callback to trigger UI updates
 * @param viewModeFn - Function to get current view mode
 */
export function initMCTasks(
  client: SocketClient,
  render: () => void,
  viewModeFn: () => ViewMode
): void {
  socketClient = client
  renderCallback = render
  getViewMode = viewModeFn
}
