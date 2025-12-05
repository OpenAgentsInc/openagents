/**
 * TB Controls Module
 *
 * Handles TB suite loading, task selection, and run control.
 * Provides UI for starting/stopping runs, selecting tasks, and displaying status.
 */

import type { TBSuiteInfo, TBRunOptions } from "./shared-types.js"
import type { SocketClient } from "./socket-client.js"

// ============================================================================
// State
// ============================================================================

let selectedTaskIds: Set<string> = new Set()
let loadedSuite: TBSuiteInfo | null = null

// ============================================================================
// Dependencies (injected during init)
// ============================================================================

let socketClient: SocketClient | null = null

// ============================================================================
// DOM Elements
// ============================================================================

let tbSuitePathInput: HTMLInputElement | null = null
let tbLoadBtn: HTMLElement | null = null
let tbStartBtn: HTMLElement | null = null
let tbRandomBtn: HTMLElement | null = null
let tbStopBtn: HTMLElement | null = null
let tbStatus: HTMLElement | null = null
let tbTaskSelector: HTMLElement | null = null
let tbSuiteName: HTMLElement | null = null
let tbTaskList: HTMLElement | null = null
let tbSelectAll: HTMLElement | null = null
let tbSelectNone: HTMLElement | null = null

// Compact controls (optional, for compact layout)
let tbLoadBtnCompact: HTMLElement | null = null
let tbStartBtnCompact: HTMLElement | null = null
let tbRandomBtnCompact: HTMLElement | null = null
let tbStopBtnCompact: HTMLElement | null = null

// ============================================================================
// RPC Functions
// ============================================================================

async function loadTBSuiteRpc(suitePath: string): Promise<TBSuiteInfo> {
  if (!socketClient) {
    throw new Error("[TB] Socket client not initialized")
  }
  console.log("[TB] Loading suite:", suitePath)
  return await socketClient.loadTBSuite(suitePath)
}

async function startTBRunRpc(options: TBRunOptions): Promise<string> {
  if (!socketClient) {
    throw new Error("[TB] Socket client not initialized")
  }
  console.log("[TB] Starting run:", options)
  const { runId } = await socketClient.startTBRun(options)
  console.log("[TB] Run started:", runId)
  return runId
}

async function stopTBRunRpc(): Promise<boolean> {
  if (!socketClient) {
    throw new Error("[TB] Socket client not initialized")
  }
  console.log("[TB] Stopping run")
  const { stopped } = await socketClient.stopTBRun()
  console.log("[TB] Stopped:", stopped)
  return stopped
}

// ============================================================================
// UI Update Functions
// ============================================================================

function updateTBStatus(status: string, className?: string): void {
  if (!tbStatus) return
  tbStatus.textContent = status
  tbStatus.className = "tb-status" + (className ? ` ${className}` : "")
}

function updateTBButtons(isRunning: boolean): void {
  if (tbStartBtn) (tbStartBtn as HTMLButtonElement).disabled = isRunning
  if (tbRandomBtn) (tbRandomBtn as HTMLButtonElement).disabled = isRunning
  if (tbStopBtn) (tbStopBtn as HTMLButtonElement).disabled = !isRunning
  if (tbLoadBtn) (tbLoadBtn as HTMLButtonElement).disabled = isRunning
  if (tbSuitePathInput) tbSuitePathInput.disabled = isRunning

  // Also update compact buttons if they exist
  if (tbStartBtnCompact) (tbStartBtnCompact as HTMLButtonElement).disabled = isRunning
  if (tbRandomBtnCompact) (tbRandomBtnCompact as HTMLButtonElement).disabled = isRunning
  if (tbStopBtnCompact) (tbStopBtnCompact as HTMLButtonElement).disabled = !isRunning
  if (tbLoadBtnCompact) (tbLoadBtnCompact as HTMLButtonElement).disabled = isRunning
}

function renderTaskList(suite: TBSuiteInfo): void {
  if (!tbTaskList || !tbSuiteName || !tbTaskSelector) return

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

// ============================================================================
// Control Handlers
// ============================================================================

async function handleLoadSuite(): Promise<void> {
  if (!tbSuitePathInput) return

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
    if (tbRandomBtn) (tbRandomBtn as HTMLButtonElement).disabled = false
  } catch (err) {
    console.error("[TB] Load failed:", err)
    updateTBStatus("Load failed", "error")
    loadedSuite = null
    if (tbTaskSelector) tbTaskSelector.classList.add("hidden")
    if (tbRandomBtn) (tbRandomBtn as HTMLButtonElement).disabled = true
  }
}

async function handleStartRun(): Promise<void> {
  if (!tbSuitePathInput) return

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

  if (!tbSuitePathInput) return

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
  if (!tbTaskList) return

  const checkboxes = tbTaskList.querySelectorAll<HTMLInputElement>("input[type=checkbox]")
  checkboxes.forEach(cb => {
    cb.checked = true
    const taskId = cb.dataset.taskId
    if (taskId) selectedTaskIds.add(taskId)
  })
}

function handleSelectNone(): void {
  if (!tbTaskList) return

  const checkboxes = tbTaskList.querySelectorAll<HTMLInputElement>("input[type=checkbox]")
  checkboxes.forEach(cb => {
    cb.checked = false
    const taskId = cb.dataset.taskId
    if (taskId) selectedTaskIds.delete(taskId)
  })
}

// ============================================================================
// Public API
// ============================================================================

export { updateTBStatus, updateTBButtons }

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize the TB controls module
 * @param client - Socket client for RPC calls
 */
export function initTBControls(client: SocketClient): void {
  socketClient = client

  // Cache DOM elements
  tbSuitePathInput = document.getElementById("tb-suite-path") as HTMLInputElement
  tbLoadBtn = document.getElementById("tb-load-btn")
  tbStartBtn = document.getElementById("tb-start-btn")
  tbRandomBtn = document.getElementById("tb-random-btn")
  tbStopBtn = document.getElementById("tb-stop-btn")
  tbStatus = document.getElementById("tb-status")
  tbTaskSelector = document.getElementById("tb-task-selector")
  tbSuiteName = document.getElementById("tb-suite-name")
  tbTaskList = document.getElementById("tb-task-list")
  tbSelectAll = document.getElementById("tb-select-all")
  tbSelectNone = document.getElementById("tb-select-none")

  // Compact controls (optional)
  tbLoadBtnCompact = document.getElementById("tb-load-btn-compact")
  tbStartBtnCompact = document.getElementById("tb-start-btn-compact")
  tbRandomBtnCompact = document.getElementById("tb-random-btn-compact")
  tbStopBtnCompact = document.getElementById("tb-stop-btn-compact")

  // Wire up button event handlers
  tbLoadBtn?.addEventListener("click", handleLoadSuite)
  tbStartBtn?.addEventListener("click", handleStartRun)
  tbRandomBtn?.addEventListener("click", handleStartRandomTask)
  tbStopBtn?.addEventListener("click", handleStopRun)
  tbSelectAll?.addEventListener("click", handleSelectAll)
  tbSelectNone?.addEventListener("click", handleSelectNone)

  // Compact buttons (optional)
  tbLoadBtnCompact?.addEventListener("click", handleLoadSuite)
  tbStartBtnCompact?.addEventListener("click", handleStartRun)
  tbRandomBtnCompact?.addEventListener("click", handleStartRandomTask)
  tbStopBtnCompact?.addEventListener("click", handleStopRun)
}
