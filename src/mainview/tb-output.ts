/**
 * TB Output Viewer Module
 *
 * Displays Terminal-Bench output (agent, verification, system) in a side panel.
 * Supports clear, copy, and close operations.
 */

import type { TBState } from "./shared-types.js"

// ============================================================================
// State
// ============================================================================

let tbState: TBState | null = null

// ============================================================================
// DOM Elements
// ============================================================================

let outputViewer: HTMLElement | null = null
let outputContent: HTMLElement | null = null
let outputClearBtn: HTMLElement | null = null
let outputCopyBtn: HTMLElement | null = null
let outputCloseBtn: HTMLElement | null = null

// ============================================================================
// Public API
// ============================================================================

/**
 * Show the output viewer
 */
export function showOutputViewer(): void {
  outputViewer?.classList.remove("hidden")
}

/**
 * Hide the output viewer
 */
function hideOutputViewer(): void {
  outputViewer?.classList.add("hidden")
}

/**
 * Update the output viewer with current TB state
 */
export function updateOutputViewer(): void {
  if (!outputContent || !tbState) return

  // Show viewer when there's output during a run
  if (tbState.outputBuffer.length > 0 && tbState.isRunning) {
    showOutputViewer()
  }

  // Render last 100 lines to avoid DOM bloat
  const linesToShow = tbState.outputBuffer.slice(-100)
  const html = linesToShow.map(line => {
    const escaped = escapeHtml(line.text)
    return `<div class="tb-output-line ${line.source}">${escaped}</div>`
  }).join("")

  outputContent.innerHTML = html

  // Auto-scroll to bottom
  outputContent.scrollTop = outputContent.scrollHeight
}

// ============================================================================
// Event Handlers
// ============================================================================

function clearOutput(): void {
  if (!tbState) return
  tbState.outputBuffer = []
  if (outputContent) outputContent.innerHTML = ""
}

function copyOutput(): void {
  if (!tbState) return
  const text = tbState.outputBuffer.map(l => l.text).join("\n")
  navigator.clipboard.writeText(text).then(() => {
    console.log("[TB] Output copied to clipboard")
  })
}

// ============================================================================
// Utilities
// ============================================================================

/**
 * Escape HTML special characters
 */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
}

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize TB output viewer
 * @param state - TB state reference
 */
export function initTBOutput(state: TBState): void {
  tbState = state

  // Cache DOM elements
  outputViewer = document.getElementById("tb-output-viewer")
  outputContent = document.getElementById("tb-output-content")
  outputClearBtn = document.getElementById("tb-output-clear")
  outputCopyBtn = document.getElementById("tb-output-copy")
  outputCloseBtn = document.getElementById("tb-output-close")

  // Wire up event listeners
  outputClearBtn?.addEventListener("click", clearOutput)
  outputCopyBtn?.addEventListener("click", copyOutput)
  outputCloseBtn?.addEventListener("click", hideOutputViewer)
}
