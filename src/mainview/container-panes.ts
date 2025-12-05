/**
 * Container Panes Module
 *
 * Displays container execution output in a grid of panes.
 * Shows stdout/stderr streams with status, duration, and exit codes.
 */

import type { ContainerPane } from "./shared-types.js"
import { ZINC, MAX_LINES_PER_PANE, MAX_VISIBLE_PANES } from "./shared-types.js"

// ============================================================================
// State
// ============================================================================

export const containerPanes = new Map<string, ContainerPane>()

// ============================================================================
// Rendering
// ============================================================================

/**
 * Throttled container pane render (avoid excessive DOM updates)
 */
let containerRenderPending = false

export function throttledContainerRender(): void {
  if (containerRenderPending) return
  containerRenderPending = true
  requestAnimationFrame(() => {
    renderContainerPanes()
    containerRenderPending = false
  })
}

/**
 * Escape HTML special characters
 */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
}

/**
 * Render container panes
 */
export function renderContainerPanes(): void {
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
    const statusColor = pane.status === "running" ? ZINC[300]
      : pane.exitCode === 0 ? ZINC[200] : ZINC[500]

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
// Initialization
// ============================================================================

export function initContainerPanes(): void {
  // No initialization needed - DOM elements are cached on first render
}
