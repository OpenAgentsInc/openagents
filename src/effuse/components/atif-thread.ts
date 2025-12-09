/**
 * ATIF-Style Thread Components
 *
 * Reusable thread-based UI components for chronological displays.
 * Used by TestGen, ATIF viewer, agent logs, etc.
 */

import { html, joinTemplates } from "../template/html.js"
import type { TemplateResult } from "../template/types.js"

// ============================================================================
// Types
// ============================================================================

export interface ProgressData {
  phase: string
  category: string | null
  round: number
  status: string
}

export interface ReflectionData {
  category: string | null
  text: string
  action: "refining" | "assessing" | "complete"
}

export interface TestData {
  id: string
  category: string
  input: string
  expectedOutput: string | null
  reasoning: string
  confidence: number
}

export interface CompleteData {
  totalTests: number
  totalRounds: number
  comprehensivenessScore: number | null
  totalTokensUsed: number
  durationMs: number
  uncertainties: string[]
}

export interface ErrorData {
  error: string
}

export type ThreadItem =
  | { type: "progress"; timestamp: number; data: ProgressData }
  | { type: "reflection"; timestamp: number; data: ReflectionData }
  | { type: "test"; timestamp: number; data: TestData }
  | { type: "complete"; timestamp: number; data: CompleteData }
  | { type: "error"; timestamp: number; data: ErrorData }

export interface ThreadItemState {
  isExpanded: boolean
  onToggle?: (id: string) => void
}

export interface ThreadOptions {
  expandedItemId: string | null
  onToggle?: (itemId: string) => void
}

// ============================================================================
// Main Thread Container
// ============================================================================

/**
 * Render a full thread container with all items in chronological order.
 */
export function renderThreadContainer(
  items: ThreadItem[],
  options: ThreadOptions
): TemplateResult {
  const itemElements = items.map((item) => {
    const itemId = getItemId(item)
    const isExpanded = itemId === options.expandedItemId
    const state: ThreadItemState = {
      isExpanded,
      ...(options.onToggle !== undefined ? { onToggle: options.onToggle } : {}),
    }
    return renderThreadItem(item, state)
  })

  return html`
    <div class="flex flex-col gap-2">
      ${joinTemplates(itemElements)}
    </div>
  `
}

// ============================================================================
// Individual Item Renderers
// ============================================================================

/**
 * Render a single thread item based on its type.
 */
export function renderThreadItem(
  item: ThreadItem,
  state: ThreadItemState
): TemplateResult {
  const timestamp = formatTimestamp(item.timestamp)

  switch (item.type) {
    case "progress":
      return renderProgressItem(timestamp, item.data, state)
    case "reflection":
      return renderReflectionItem(timestamp, item.data, state)
    case "test":
      return renderTestItem(timestamp, item.data, state)
    case "complete":
      return renderCompleteItem(timestamp, item.data, state)
    case "error":
      return renderErrorItem(timestamp, item.data, state)
  }
}

/**
 * Render a progress item.
 */
function renderProgressItem(
  timestamp: string,
  progress: ProgressData,
  state: ThreadItemState
): TemplateResult {
  return html`
    <div class="p-3 bg-zinc-900/40 border border-zinc-800/60 rounded-lg">
      <div class="flex items-center gap-3">
        <span class="text-xs text-zinc-500 font-mono">${timestamp}</span>
        <span class="text-zinc-400">⚙️</span>
        <span class="text-xs font-mono text-zinc-300 uppercase">PROGRESS</span>
        <span class="text-sm text-zinc-400">${progress.status}</span>
        ${progress.category
          ? html`<span class="text-xs text-zinc-500">(${progress.category} - round ${progress.round})</span>`
          : ""}
      </div>
    </div>
  `
}

/**
 * Render a reflection item.
 */
function renderReflectionItem(
  timestamp: string,
  reflection: ReflectionData,
  state: ThreadItemState
): TemplateResult {
  const actionLabels = {
    refining: "Refining",
    assessing: "Assessing",
    complete: "Complete",
  }

  return html`
    <div class="p-3 bg-blue-900/20 border border-blue-800/50 rounded-lg">
      <div class="flex items-start gap-3">
        <span class="text-xs text-zinc-500 font-mono">${timestamp}</span>
        <span class="text-blue-300">💭</span>
        <div class="flex-1">
          <div class="flex items-center gap-2 mb-1">
            <span class="text-xs font-mono text-blue-300 uppercase">REFLECTION</span>
            <span class="text-xs text-blue-400">${actionLabels[reflection.action]}</span>
            ${reflection.category
              ? html`<span class="px-2 py-0.5 bg-blue-900/40 border border-blue-700/50 rounded text-xs text-blue-300 font-mono">${reflection.category}</span>`
              : ""}
          </div>
          <p class="text-sm text-blue-200 font-mono leading-relaxed">${reflection.text}</p>
        </div>
      </div>
    </div>
  `
}

/**
 * Render a test item with accordion expansion.
 */
function renderTestItem(
  timestamp: string,
  test: TestData,
  state: ThreadItemState
): TemplateResult {
  const categoryBadge = getCategoryBadge(test.category)
  const confidencePercent = Math.round(test.confidence * 100)
  const itemId = test.id

  const header = html`
    <div
      class="flex items-center justify-between p-3 bg-zinc-900/60 border border-zinc-800/60 rounded-lg cursor-pointer hover:bg-zinc-900/80 transition-colors"
      data-action="toggleItem"
      data-item-id="${itemId}"
    >
      <div class="flex items-center gap-3 flex-1 min-w-0">
        <span class="text-xs text-zinc-500 font-mono flex-shrink-0">${timestamp}</span>
        ${categoryBadge}
        <span class="text-sm text-zinc-200 font-mono truncate">${test.id}</span>
        <span class="text-xs text-zinc-400 flex-shrink-0">(${confidencePercent}%)</span>
      </div>
      <span class="text-zinc-500 flex-shrink-0 ml-2">${state.isExpanded ? "▲" : "▼"}</span>
    </div>
  `

  if (!state.isExpanded) {
    return header
  }

  const confidenceBar = renderConfidenceBar(test.confidence)

  const details = html`
    <div class="mt-2 p-4 bg-zinc-950/60 border border-zinc-800/40 rounded-lg space-y-3">
      <div>
        <label class="text-xs font-mono text-zinc-500 uppercase">Input</label>
        <pre class="mt-1 p-2 bg-zinc-900/60 rounded text-sm font-mono text-emerald-300 overflow-x-auto whitespace-pre-wrap">${test.input}</pre>
      </div>

      ${test.expectedOutput
        ? html`
            <div>
              <label class="text-xs font-mono text-zinc-500 uppercase">Expected Output</label>
              <pre class="mt-1 p-2 bg-zinc-900/60 rounded text-sm font-mono text-blue-300 overflow-x-auto whitespace-pre-wrap">${test.expectedOutput}</pre>
            </div>
          `
        : ""}

      <div>
        <label class="text-xs font-mono text-zinc-500 uppercase">Reasoning</label>
        <p class="mt-1 text-sm text-zinc-300 leading-relaxed">${test.reasoning}</p>
      </div>

      <div>
        <label class="text-xs font-mono text-zinc-500 uppercase">Confidence</label>
        <div class="mt-1">${confidenceBar}</div>
      </div>
    </div>
  `

  return html`${header} ${details}`
}

/**
 * Render a completion item.
 */
function renderCompleteItem(
  timestamp: string,
  complete: CompleteData,
  state: ThreadItemState
): TemplateResult {
  return html`
    <div class="p-4 bg-emerald-900/20 border border-emerald-700/50 rounded-lg">
      <div class="flex items-center gap-3 mb-3">
        <span class="text-xs text-zinc-500 font-mono">${timestamp}</span>
        <span class="text-emerald-400">✓</span>
        <span class="text-xs font-mono text-emerald-300 uppercase">COMPLETE</span>
      </div>
      <div class="space-y-2 text-sm">
        <div>
          <span class="text-zinc-400">Total Tests: </span>
          <span class="text-emerald-300 font-mono">${complete.totalTests}</span>
        </div>
        <div>
          <span class="text-zinc-400">Total Rounds: </span>
          <span class="text-emerald-300 font-mono">${complete.totalRounds}</span>
        </div>
        ${complete.comprehensivenessScore !== null
          ? html`
              <div>
                <span class="text-zinc-400">Comprehensiveness Score: </span>
                <span class="text-emerald-300 font-mono">${complete.comprehensivenessScore}/10</span>
              </div>
            `
          : ""}
        <div>
          <span class="text-zinc-400">Tokens Used: </span>
          <span class="text-emerald-300 font-mono">${complete.totalTokensUsed.toLocaleString()}</span>
        </div>
        <div>
          <span class="text-zinc-400">Duration: </span>
          <span class="text-emerald-300 font-mono">${(complete.durationMs / 1000).toFixed(1)}s</span>
        </div>
        ${complete.uncertainties.length > 0
          ? html`
              <div>
                <span class="text-zinc-400">Uncertainties: </span>
                <ul class="mt-1 space-y-1">
                  ${joinTemplates(
                    complete.uncertainties.map(
                      (u) => html`
                        <li class="text-yellow-300 text-xs">• ${u}</li>
                      `
                    )
                  )}
                </ul>
              </div>
            `
          : ""}
      </div>
    </div>
  `
}

/**
 * Render an error item.
 */
function renderErrorItem(
  timestamp: string,
  error: ErrorData,
  state: ThreadItemState
): TemplateResult {
  return html`
    <div class="p-3 bg-red-900/20 border border-red-700/50 rounded-lg">
      <div class="flex items-start gap-3">
        <span class="text-xs text-zinc-500 font-mono">${timestamp}</span>
        <span class="text-red-400">✗</span>
        <div class="flex-1">
          <div class="flex items-center gap-2 mb-1">
            <span class="text-xs font-mono text-red-300 uppercase">ERROR</span>
          </div>
          <p class="text-sm text-red-200 font-mono">${error.error}</p>
        </div>
      </div>
    </div>
  `
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get a unique ID for a thread item.
 */
function getItemId(item: ThreadItem): string {
  switch (item.type) {
    case "test":
      return item.data.id
    case "progress":
      return `progress-${item.timestamp}`
    case "reflection":
      return `reflection-${item.timestamp}`
    case "complete":
      return `complete-${item.timestamp}`
    case "error":
      return `error-${item.timestamp}`
  }
}

/**
 * Format timestamp as HH:MM:SS.
 */
function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp)
  const hours = date.getHours().toString().padStart(2, "0")
  const minutes = date.getMinutes().toString().padStart(2, "0")
  const seconds = date.getSeconds().toString().padStart(2, "0")
  return `${hours}:${minutes}:${seconds}`
}

/**
 * Get category badge with emoji and colors.
 */
function getCategoryBadge(category: string): TemplateResult {
  const badges: Record<
    string,
    { emoji: string; bg: string; text: string; border: string }
  > = {
    anti_cheat: {
      emoji: "🔴",
      bg: "bg-red-900/40",
      text: "text-red-300",
      border: "border-red-700/50",
    },
    existence: {
      emoji: "🔵",
      bg: "bg-blue-900/40",
      text: "text-blue-300",
      border: "border-blue-700/50",
    },
    correctness: {
      emoji: "🟢",
      bg: "bg-emerald-900/40",
      text: "text-emerald-300",
      border: "border-emerald-700/50",
    },
    boundary: {
      emoji: "🟡",
      bg: "bg-yellow-900/40",
      text: "text-yellow-300",
      border: "border-yellow-700/50",
    },
    integration: {
      emoji: "🟣",
      bg: "bg-purple-900/40",
      text: "text-purple-300",
      border: "border-purple-700/50",
    },
  }

  const badge =
    badges[category] ||
    ({
      emoji: "⚪",
      bg: "bg-zinc-800/40",
      text: "text-zinc-300",
      border: "border-zinc-700/50",
    } as const)

  return html`
    <span class="px-2 py-1 text-xs font-mono border rounded ${badge.bg} ${badge.text} ${badge.border} flex-shrink-0">
      ${badge.emoji} ${category}
    </span>
  `
}

/**
 * Render confidence bar.
 */
function renderConfidenceBar(confidence: number): TemplateResult {
  const percent = Math.round(confidence * 100)
  const width = `${percent}%`

  // Use a neutral color for the bar
  const barColor = "bg-emerald-500"

  return html`
    <div class="flex items-center gap-2">
      <div class="flex-1 h-2 bg-zinc-800 rounded overflow-hidden">
        <div class="h-full ${barColor} transition-all" style="width: ${width}"></div>
      </div>
      <span class="text-xs text-zinc-400 font-mono">${percent}%</span>
    </div>
  `
}
