/**
 * ATIF Trajectory Details Widget
 *
 * Displays step-by-step details of an ATIF trajectory including
 * tool calls, observations, and agent state.
 */

import { Effect } from "effect"
import { html, joinTemplates } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import { SocketServiceTag } from "../services/socket.js"
// import type { ATIFStepMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/**
 * ATIF Step with full details
 */
export interface ATIFStep {
  stepId: number
  timestamp: string
  source: "user" | "agent" | "system"
  message: unknown
  toolCalls?: Array<{
    toolCallId: string
    functionName: string
    arguments: unknown
  }>
  observation?: {
    results: Array<{
      sourceCallId?: string
      content?: unknown
    }>
  }
}

/**
 * ATIF Details State
 */
export interface ATIFDetailsState {
  /** Session ID being viewed */
  sessionId: string | null
  /** Agent name */
  agentName: string | null
  /** Steps in the trajectory */
  steps: ATIFStep[]
  /** Loading state */
  loading: boolean
  /** Error message */
  error: string | null
  /** Collapsed state */
  collapsed: boolean
  /** Currently expanded step (for accordion) */
  expandedStepId: number | null
}

/**
 * ATIF Details Events
 */
export type ATIFDetailsEvent =
  | { type: "load"; sessionId: string }
  | { type: "toggleCollapse" }
  | { type: "toggleStep"; stepId: number }
  | { type: "clear" }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Format timestamp for display
 */
const formatTimestamp = (iso: string): string => {
  try {
    return new Date(iso).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    })
  } catch {
    return iso
  }
}

/**
 * Get source badge class
 */
const getSourceClass = (source: "user" | "agent" | "system"): string => {
  switch (source) {
    case "user":
      return "bg-blue-900/40 text-blue-300 border-blue-700/50"
    case "agent":
      return "bg-emerald-900/40 text-emerald-300 border-emerald-700/50"
    case "system":
      return "bg-zinc-800/40 text-zinc-300 border-zinc-700/50"
  }
}

/**
 * Format JSON for display
 */
const formatJSON = (obj: unknown): string => {
  try {
    return JSON.stringify(obj, null, 2)
  } catch {
    return String(obj)
  }
}

// ============================================================================
// Widget Definition
// ============================================================================

export const ATIFDetailsWidget: Widget<ATIFDetailsState, ATIFDetailsEvent, SocketServiceTag> = {
  id: "atif-details",

  initialState: () => ({
    sessionId: null,
    agentName: null,
    steps: [],
    loading: false,
    error: null,
    collapsed: false,
    expandedStepId: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Header
      const header = html`
        <div
          class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer bg-zinc-900/40"
          data-action="toggleCollapse"
        >
          <div class="flex items-center gap-2">
            <h3 class="text-sm font-bold font-mono text-zinc-100">ATIF Trajectory</h3>
            ${state.agentName ? html`<span class="text-xs text-zinc-400">${state.agentName}</span>` : ""}
          </div>
          <span class="text-zinc-500">${state.collapsed ? "▼" : "▲"}</span>
        </div>
      `

      // Collapsed view
      if (state.collapsed) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
          </div>
        `
      }

      // Empty state
      if (!state.sessionId || state.steps.length === 0) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">
              ${state.loading ? "Loading trajectory..." : "No trajectory selected"}
            </div>
          </div>
        `
      }

      // Steps list
      const stepsList = html`
        <div class="max-h-96 overflow-y-auto">
          ${joinTemplates(
            state.steps.map((step) => {
              const isExpanded = state.expandedStepId === step.stepId
              const sourceClass = getSourceClass(step.source)

              return html`
                <div class="border-b border-zinc-800/40">
                  <div
                    class="flex items-center justify-between px-4 py-2 hover:bg-zinc-900/40 cursor-pointer transition-colors"
                    data-action="toggleStep"
                    data-step-id="${step.stepId}"
                  >
                    <div class="flex items-center gap-3 flex-1">
                      <span class="text-xs font-mono text-zinc-500">#${step.stepId}</span>
                      <span class="text-xs px-1.5 py-0.5 rounded border ${sourceClass} uppercase">
                        ${step.source}
                      </span>
                      <span class="text-xs text-zinc-400 font-mono">${formatTimestamp(step.timestamp)}</span>
                      ${step.toolCalls && step.toolCalls.length > 0
                        ? html`<span class="text-xs text-violet-400">🔧 ${step.toolCalls.length} tool${step.toolCalls.length > 1 ? "s" : ""}</span>`
                        : ""}
                    </div>
                    <span class="text-zinc-500">${isExpanded ? "▲" : "▼"}</span>
                  </div>
                  ${isExpanded
                    ? html`
                        <div class="px-4 py-3 bg-zinc-900/20 space-y-3">
                          ${step.message
                            ? html`
                                <div>
                                  <div class="text-xs font-mono text-zinc-400 mb-1">Message:</div>
                                  <pre class="text-xs font-mono text-zinc-200 bg-zinc-950/60 p-2 rounded border border-zinc-800/40 overflow-x-auto">${formatJSON(step.message)}</pre>
                                </div>
                              `
                            : ""}
                          ${step.toolCalls && step.toolCalls.length > 0
                            ? html`
                                <div>
                                  <div class="text-xs font-mono text-zinc-400 mb-1">Tool Calls:</div>
                                  ${joinTemplates(
                                    step.toolCalls.map(
                                      (tc) => html`
                                        <div class="bg-zinc-950/60 p-2 rounded border border-zinc-800/40 mb-2">
                                          <div class="text-xs font-mono text-violet-300 mb-1">${tc.functionName}</div>
                                          <pre class="text-xs font-mono text-zinc-400 overflow-x-auto">${formatJSON(tc.arguments)}</pre>
                                        </div>
                                      `
                                    )
                                  )}
                                </div>
                              `
                            : ""}
                          ${step.observation && step.observation.results.length > 0
                            ? html`
                                <div>
                                  <div class="text-xs font-mono text-zinc-400 mb-1">Observation:</div>
                                  ${joinTemplates(
                                    step.observation.results.map(
                                      (result) => html`
                                        <pre class="text-xs font-mono text-emerald-300 bg-zinc-950/60 p-2 rounded border border-zinc-800/40 overflow-x-auto mb-2">${formatJSON(result.content)}</pre>
                                      `
                                    )
                                  )}
                                </div>
                              `
                            : ""}
                        </div>
                      `
                    : ""}
                </div>
              `
            })
          )}
        </div>
      `

      return html`
        <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header}
          <div class="px-4 py-2 bg-zinc-900/20 border-b border-zinc-800/40 flex items-center justify-between">
            <span class="text-xs font-mono text-zinc-400">${state.steps.length} steps</span>
            ${state.sessionId ? html`<span class="text-xs font-mono text-zinc-500">${state.sessionId.slice(-8)}</span>` : ""}
          </div>
          ${stepsList}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Handle button clicks
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        const stepId = el.dataset.stepId

        if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        } else if (action === "toggleStep" && stepId) {
          Effect.runFork(ctx.emit({ type: "toggleStep", stepId: parseInt(stepId, 10) }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      // const socket = yield* SocketServiceTag

      switch (event.type) {
        case "load": {
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null, sessionId: event.sessionId }))

          // In a real implementation, this would fetch the trajectory from the socket
          // For now, we'll just clear loading state
          yield* ctx.state.update((s) => ({ ...s, loading: false }))
          break
        }

        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "toggleStep": {
          yield* ctx.state.update((s) => ({
            ...s,
            expandedStepId: s.expandedStepId === event.stepId ? null : event.stepId,
          }))
          break
        }

        case "clear":
          yield* ctx.state.update(() => ATIFDetailsWidget.initialState())
          break
      }
    }),

  subscriptions: () => [],
}

// ============================================================================
// Export initial state for testing
// ============================================================================

export const initialATIFDetailsState: ATIFDetailsState = ATIFDetailsWidget.initialState()
