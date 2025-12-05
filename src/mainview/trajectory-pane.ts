/**
 * Trajectory Pane Module
 *
 * Displays unified trajectories (TB runs, ATIF traces) in a side panel.
 * Supports loading, rendering, and selecting trajectories.
 */

import type { UnifiedTrajectory } from "../desktop/protocol.js"
import type { SocketClient } from "./socket-client.js"

// ============================================================================
// State
// ============================================================================

let unifiedTrajectories: UnifiedTrajectory[] = []
let selectedTrajectoryId: string | null = null
let trajectoriesLoading = false

// ============================================================================
// Socket Client Reference
// ============================================================================

let socketClient: SocketClient | null = null

// ============================================================================
// Public API
// ============================================================================

/**
 * Load unified trajectories from the server
 */
export async function loadTrajectories(): Promise<void> {
  if (trajectoriesLoading) return
  if (!socketClient) {
    console.error("[Trajectories] Socket client not initialized")
    return
  }

  trajectoriesLoading = true

  try {
    console.log("[Trajectories] Loading unified trajectories...")
    window.bunLog?.("[Trajectories] Loading unified trajectories...")
    unifiedTrajectories = await socketClient.loadUnifiedTrajectories(50)
    console.log(`[Trajectories] Loaded ${unifiedTrajectories.length} trajectories`)
    window.bunLog?.(`[Trajectories] Loaded ${unifiedTrajectories.length} trajectories`)
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err)
    console.error("[Trajectories] Failed to load:", errMsg)
    window.bunLog?.(`[Trajectories] FAILED: ${errMsg}`)
    unifiedTrajectories = []
  } finally {
    trajectoriesLoading = false
    renderTrajectoryPane()
  }
}

/**
 * Render the trajectory pane
 */
export function renderTrajectoryPane(): void {
  const list = document.getElementById("trajectory-list")
  if (!list) return

  if (trajectoriesLoading) {
    list.innerHTML = '<div class="trajectory-loading">Loading trajectories...</div>'
    return
  }

  if (unifiedTrajectories.length === 0) {
    list.innerHTML = '<div class="trajectory-empty">No trajectories found</div>'
    return
  }

  list.innerHTML = unifiedTrajectories.map((traj) => {
    const isSelected = traj.id === selectedTrajectoryId
    const typeClass = traj.type === "tb-run" ? "tb" : "atif"
    const shortId = traj.id.slice(-8)
    const date = new Date(traj.timestamp).toLocaleString()

    return `
      <div class="trajectory-card ${isSelected ? "selected" : ""}"
           data-trajectory-id="${traj.id}">
        <div class="trajectory-card-id">${shortId}</div>
        <div class="trajectory-card-timestamp">${date}</div>
        <div class="trajectory-card-label ${typeClass}">${traj.label}</div>
      </div>
    `
  }).join("")
}

// ============================================================================
// Event Handlers
// ============================================================================

function handleTrajectoryClick(trajectoryId: string): void {
  selectedTrajectoryId = trajectoryId
  console.log(`[Trajectories] Selected: ${trajectoryId}`)
  window.bunLog?.(`[Trajectories] Selected: ${trajectoryId}`)
  renderTrajectoryPane()
  // Future: load and display trajectory details in main area
}

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize the trajectory pane
 * @param client - Socket client for loading trajectories
 */
export function initTrajectoryPane(client: SocketClient): void {
  socketClient = client

  // Set up event delegation for trajectory card clicks
  const trajectoryList = document.getElementById("trajectory-list")
  trajectoryList?.addEventListener("click", (e) => {
    const target = e.target as HTMLElement
    const card = target.closest(".trajectory-card") as HTMLElement | null
    if (card) {
      const trajectoryId = card.dataset.trajectoryId
      if (trajectoryId) {
        handleTrajectoryClick(trajectoryId)
      }
    }
  })
}
