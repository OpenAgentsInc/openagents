/**
 * Log Panel Renderer for TestGen Graph Component
 *
 * Renders a collapsible output panel that streams HillClimber/MAP orchestrator
 * output as messages arrive, using ATIF-style thread components.
 */

import { html, joinTemplates } from "../../template/html.js"
import type { TemplateResult } from "../../template/types.js"
import type { LogItem } from "./types.js"

// ============================================================================
// Main Log Panel Renderer
// ============================================================================

/**
 * Render the log/output panel with streaming items
 */
export function renderLogPanel(
  items: LogItem[],
  collapsed: boolean
): TemplateResult {
  if (collapsed) {
    return html`
      <div
        data-action="toggle-log"
        style="
          position: absolute;
          bottom: 10px;
          right: 10px;
          padding: 8px 12px;
          background: rgba(0, 0, 0, 0.8);
          border: 1px solid #333;
          border-radius: 4px;
          cursor: pointer;
          color: #888;
          font-size: 12px;
          z-index: 100;
        "
      >
        ▶ Show Output (${items.length} items)
      </div>
    `
  }

  return html`
    <div
      style="
        position: absolute;
        bottom: 10px;
        right: 10px;
        width: 400px;
        max-height: 50%;
        background: rgba(0, 0, 0, 0.9);
        border: 1px solid #333;
        border-radius: 4px;
        z-index: 100;
        display: flex;
        flex-direction: column;
        overflow: hidden;
      "
    >
      <div
        data-action="toggle-log"
        style="
          padding: 8px 12px;
          background: rgba(0, 0, 0, 0.6);
          border-bottom: 1px solid #333;
          cursor: pointer;
          color: #fff;
          font-size: 12px;
          font-weight: 500;
          display: flex;
          justify-content: space-between;
          align-items: center;
        "
      >
        <span>▼ Output (${items.length} items)</span>
        <span style="color: #666; font-size: 10px;">${items.length > 0 ? formatTime(items[items.length - 1].timestamp) : ""}</span>
      </div>
      <div
        id="log-panel-content"
        style="
          overflow-y: auto;
          max-height: calc(100% - 40px);
          padding: 8px;
          display: flex;
          flex-direction: column;
          gap: 4px;
        "
      >
        ${joinTemplates(items.slice(-50).map(renderLogItem))}
      </div>
    </div>
  `
}

// ============================================================================
// Individual Log Item Renderers
// ============================================================================

/**
 * Render a single log item based on its type
 */
function renderLogItem(item: LogItem): TemplateResult {
  const time = formatTime(item.timestamp)

  switch (item.type) {
    case "turn":
      return html`
        <div
          class="log-turn"
          style="
            padding: 6px 8px;
            background: rgba(59, 130, 246, 0.1);
            border-left: 2px solid #3b82f6;
            border-radius: 2px;
            font-size: 11px;
            color: #93c5fd;
          "
        >
          <span style="margin-right: 6px;">⚙️</span>
          <span style="color: #888; font-family: monospace;">${time}</span>
          <span style="margin-left: 6px;">
            Turn ${item.data.turn}/${item.data.maxTurns}: ${item.data.subtask}
          </span>
        </div>
      `

    case "fm_action":
      const actionEmoji = item.data.action === "thinking" ? "🤔" : item.data.action === "tool_call" ? "🔧" : "✓"
      const actionLabel = item.data.action === "thinking" ? "Thinking" : item.data.action === "tool_call" ? "Tool Call" : "Complete"
      return html`
        <div
          class="log-fm"
          style="
            padding: 6px 8px;
            background: rgba(168, 85, 247, 0.1);
            border-left: 2px solid #a855f7;
            border-radius: 2px;
            font-size: 11px;
            color: #c084fc;
          "
        >
          <span style="margin-right: 6px;">${actionEmoji}</span>
          <span style="color: #888; font-family: monospace;">${time}</span>
          <span style="margin-left: 6px;">
            ${actionLabel}${item.data.tool ? `: ${item.data.tool}` : ""}
          </span>
        </div>
      `

    case "verify":
      const verifyEmoji = item.data.status === "running" ? "⏳" : item.data.status === "passed" ? "✓" : "✗"
      const verifyColor = item.data.status === "running" ? "#fbbf24" : item.data.status === "passed" ? "#10b981" : "#ef4444"
      return html`
        <div
          class="log-verify"
          style="
            padding: 6px 8px;
            background: rgba(${item.data.status === "passed" ? "16, 185, 129" : item.data.status === "failed" ? "239, 68, 68" : "251, 191, 36"}, 0.1);
            border-left: 2px solid ${verifyColor};
            border-radius: 2px;
            font-size: 11px;
            color: ${verifyColor};
          "
        >
          <span style="margin-right: 6px;">${verifyEmoji}</span>
          <span style="color: #888; font-family: monospace;">${time}</span>
          <span style="margin-left: 6px;">
            Verify: ${item.data.status} ${item.data.passed ?? 0}/${item.data.total ?? 0}
          </span>
        </div>
      `

    case "progress":
      return html`
        <div
          class="log-progress"
          style="
            padding: 6px 8px;
            background: rgba(34, 197, 94, 0.1);
            border-left: 2px solid #22c55e;
            border-radius: 2px;
            font-size: 11px;
            color: #4ade80;
          "
        >
          <span style="margin-right: 6px;">📊</span>
          <span style="color: #888; font-family: monospace;">${time}</span>
          <span style="margin-left: 6px;">
            ${item.data.phase}: ${item.data.message}
          </span>
        </div>
      `

    case "complete":
      return html`
        <div
          class="log-complete"
          style="
            padding: 6px 8px;
            background: rgba(16, 185, 129, 0.15);
            border-left: 2px solid #10b981;
            border-radius: 2px;
            font-size: 11px;
            color: #34d399;
          "
        >
          <span style="margin-right: 6px;">${item.data.passed ? "✓" : "✗"}</span>
          <span style="color: #888; font-family: monospace;">${time}</span>
          <span style="margin-left: 6px;">
            Complete: ${item.data.passed ? "PASSED" : "FAILED"} (${(item.data.progress * 100).toFixed(1)}% progress, ${(item.data.duration / 1000).toFixed(1)}s)
          </span>
        </div>
      `

    case "error":
      return html`
        <div
          class="log-error"
          style="
            padding: 6px 8px;
            background: rgba(239, 68, 68, 0.15);
            border-left: 2px solid #ef4444;
            border-radius: 2px;
            font-size: 11px;
            color: #f87171;
          "
        >
          <span style="margin-right: 6px;">✗</span>
          <span style="color: #888; font-family: monospace;">${time}</span>
          <span style="margin-left: 6px;">
            Error: ${item.data.message}
          </span>
        </div>
      `
  }
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Format timestamp as HH:MM:SS
 */
function formatTime(timestamp: number): string {
  const date = new Date(timestamp)
  const hours = date.getHours().toString().padStart(2, "0")
  const minutes = date.getMinutes().toString().padStart(2, "0")
  const seconds = date.getSeconds().toString().padStart(2, "0")
  return `${hours}:${minutes}:${seconds}`
}



