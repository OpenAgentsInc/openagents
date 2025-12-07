/**
 * HuggingFace Trajectory Detail Widget
 *
 * Displays a selected ATIF trajectory with formatted step-by-step accordion view.
 * Shows tool calls, observations, metrics with color-coded source badges.
 * Modeled after ATIFDetailsWidget.
 */

import { Effect } from "effect"
import { html, joinTemplates } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import type { Trajectory, Step } from "../../atif/schema.js"
import {
  hasToolCalls,
  hasObservation,
  isAgentStep,
  isUserStep,
  isSystemStep,
  extractStepText,
} from "../../atif/schema.js"

// ============================================================================
// Types
// ============================================================================

/**
 * HF Trajectory Detail State
 */
export interface HFTrajectoryDetailState {
  /** Currently displayed trajectory */
  trajectory: Trajectory | null
  /** Session ID */
  sessionId: string | null
  /** Loading state */
  loading: boolean
  /** Error message */
  error: string | null
  /** Collapsed state */
  collapsed: boolean
  /** Currently expanded step IDs (multiple steps can be expanded) */
  expandedStepIds: Set<number>
  /** View mode (for future: toggle between formatted/JSON) */
  viewMode: "formatted" | "json"
}

/**
 * HF Trajectory Detail Events
 */
export type HFTrajectoryDetailEvent =
  | { type: "load"; sessionId: string; trajectory: Trajectory }
  | { type: "toggleCollapse" }
  | { type: "toggleStep"; stepId: number }
  | { type: "clear" }
  | { type: "toggleViewMode" }

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
    return iso.slice(0, 19).replace("T", " ")
  }
}

/**
 * Format date for display
 */
const formatDate = (iso: string): string => {
  try {
    return new Date(iso).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    })
  } catch {
    return iso.slice(0, 16).replace("T", " ")
  }
}

/**
 * Get source badge class
 */
const getSourceClass = (source: string): string => {
  switch (source) {
    case "user":
      return "bg-blue-900/40 text-blue-300 border-blue-700/50"
    case "agent":
      return "bg-emerald-900/40 text-emerald-300 border-emerald-700/50"
    case "system":
      return "bg-zinc-800/40 text-zinc-300 border-zinc-700/50"
    default:
      return "bg-zinc-800/40 text-zinc-300 border-zinc-700/50"
  }
}

/**
 * Format JSON for display
 */
const formatJSON = (obj: unknown): string => {
  try {
    if (typeof obj === "string") return obj
    return JSON.stringify(obj, null, 2)
  } catch {
    return String(obj)
  }
}

/**
 * Safely get message text from step
 */
const getMessageText = (step: Step): string => {
  const text = extractStepText(step)
  if (text.length > 500) {
    return text.slice(0, 500) + "... (truncated)"
  }
  return text
}

// ============================================================================
// Widget Definition
// ============================================================================

export const HFTrajectoryDetailWidget: Widget<HFTrajectoryDetailState, HFTrajectoryDetailEvent> = {
  id: "hf-trajectory-detail",

  initialState: () => {
    if ((window as any).bunLog) {
      (window as any).bunLog("[HFTrajectoryDetail] Creating initial state")
    }
    return {
      trajectory: null,
      sessionId: null,
      loading: false,
      error: null,
      collapsed: false,
      expandedStepIds: new Set<number>(),
      viewMode: "formatted",
    }
  },

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Header
      const header = html`
        <div
          class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer bg-zinc-900/40"
          data-action="toggleCollapse"
        >
          <h3 class="text-sm font-bold font-mono text-zinc-100">Trajectory Details</h3>
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

      // Loading state
      if (state.loading) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">Loading trajectory...</div>
          </div>
        `
      }

      // Error state
      if (state.error) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
            <div class="px-4 py-8">
              <div class="text-sm text-red-400 mb-2">Error loading trajectory</div>
              <div class="text-xs text-zinc-500">${state.error}</div>
            </div>
          </div>
        `
      }

      // Empty state
      if (!state.trajectory) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">
              No trajectory selected. Click a trajectory in the sidebar to view details.
            </div>
          </div>
        `
      }

      const traj = state.trajectory
      const agent = traj.agent
      const extra = traj.extra as Record<string, unknown> | undefined
      const task = (extra?.task as string) ?? "unknown"
      const episode = (extra?.episode as string) ?? "unknown"
      const date = (extra?.date as string) ?? traj.steps[0]?.timestamp ?? "unknown"

      // Metadata section
      const metadata = html`
        <div class="px-4 py-3 bg-zinc-900/20 border-b border-zinc-800/40 space-y-1.5">
          <div class="flex items-center gap-2 text-xs">
            <span class="text-zinc-500">Session:</span>
            <span class="text-zinc-300 font-mono">${traj.session_id}</span>
          </div>
          <div class="flex items-center gap-2 text-xs">
            <span class="text-zinc-500">Agent:</span>
            <span class="text-zinc-300">${agent?.name ?? "unknown"}</span>
            ${agent?.model_name ? html`<span class="text-zinc-500">(${agent.model_name})</span>` : ""}
          </div>
          <div class="flex items-center gap-2 text-xs">
            <span class="text-zinc-500">Task:</span>
            <span class="text-zinc-300">${task}</span>
            <span class="text-zinc-500">•</span>
            <span class="text-zinc-500">Episode:</span>
            <span class="text-zinc-300">${episode}</span>
          </div>
          <div class="flex items-center gap-2 text-xs">
            <span class="text-zinc-500">Steps:</span>
            <span class="text-zinc-300">${traj.steps.length}</span>
            <span class="text-zinc-500">•</span>
            <span class="text-zinc-500">Date:</span>
            <span class="text-zinc-300">${formatDate(date)}</span>
          </div>
        </div>
      `

      // Steps list with accordion
      const stepsList = html`
        <div class="max-h-[calc(100vh-20rem)] overflow-y-auto">
          ${joinTemplates(
            traj.steps.map((step) => {
              const isExpanded = state.expandedStepIds.has(step.step_id)
              const source = step.source ?? "system"
              const sourceClass = getSourceClass(source)
              const toolCallCount = hasToolCalls(step) ? step.tool_calls!.length : 0
              const hasObs = hasObservation(step)

              // Step header
              const stepHeader = html`
                <div
                  class="flex items-center justify-between px-4 py-2 hover:bg-zinc-900/40 cursor-pointer transition-colors border-b border-zinc-800/40"
                  data-action="toggleStep"
                  data-step-id="${step.step_id}"
                >
                  <div class="flex items-center gap-3 flex-1">
                    <span class="text-xs font-mono text-zinc-500">#${step.step_id}</span>
                    <span class="text-xs px-1.5 py-0.5 rounded border ${sourceClass} uppercase font-mono">
                      ${source}
                    </span>
                    <span class="text-xs text-zinc-400 font-mono">${formatTimestamp(step.timestamp)}</span>
                    ${toolCallCount > 0
                      ? html`<span class="text-xs text-violet-400">🔧 ${toolCallCount} tool${toolCallCount > 1 ? "s" : ""}</span>`
                      : ""}
                    ${hasObs ? html`<span class="text-xs text-emerald-400">✓ obs</span>` : ""}
                  </div>
                  <span class="text-zinc-500">${isExpanded ? "▲" : "▼"}</span>
                </div>
              `

              // Expanded content
              if (!isExpanded) {
                return stepHeader
              }

              const expandedContent = html`
                <div class="px-4 py-3 bg-zinc-900/20 space-y-3">
                  ${step.message
                    ? html`
                        <div>
                          <div class="text-xs font-mono text-zinc-400 mb-1">Message:</div>
                          <pre
                            class="text-xs font-mono text-zinc-200 bg-zinc-950/60 p-2 rounded border border-zinc-800/40 overflow-x-auto whitespace-pre-wrap"
                          >${getMessageText(step)}</pre>
                        </div>
                      `
                    : ""}
                  ${step.reasoning_content
                    ? html`
                        <div>
                          <div class="text-xs font-mono text-zinc-400 mb-1">Reasoning:</div>
                          <pre
                            class="text-xs font-mono text-zinc-300 bg-zinc-950/60 p-2 rounded border border-zinc-800/40 overflow-x-auto whitespace-pre-wrap"
                          >${step.reasoning_content}</pre>
                        </div>
                      `
                    : ""}
                  ${hasToolCalls(step)
                    ? html`
                        <div>
                          <div class="text-xs font-mono text-zinc-400 mb-1">Tool Calls:</div>
                          ${joinTemplates(
                            step.tool_calls!.map(
                              (tc) => html`
                                <div class="bg-zinc-950/60 p-2 rounded border border-zinc-800/40 mb-2">
                                  <div class="text-xs font-mono text-violet-300 mb-1">${tc.function_name}</div>
                                  <pre
                                    class="text-xs font-mono text-zinc-400 overflow-x-auto whitespace-pre-wrap"
                                  >${formatJSON(tc.arguments)}</pre>
                                </div>
                              `
                            )
                          )}
                        </div>
                      `
                    : ""}
                  ${hasObservation(step)
                    ? html`
                        <div>
                          <div class="text-xs font-mono text-zinc-400 mb-1">Observation:</div>
                          ${joinTemplates(
                            step.observation!.results.map((result) => {
                              const content = result.content
                              const contentStr =
                                typeof content === "string"
                                  ? content.length > 500
                                    ? content.slice(0, 500) + "... (truncated)"
                                    : content
                                  : formatJSON(content)
                              return html`
                                <pre
                                  class="text-xs font-mono text-emerald-300 bg-zinc-950/60 p-2 rounded border border-zinc-800/40 overflow-x-auto mb-2 whitespace-pre-wrap"
                                >${contentStr}</pre>
                              `
                            })
                          )}
                        </div>
                      `
                    : ""}
                  ${step.metrics
                    ? html`
                        <div>
                          <div class="text-xs font-mono text-zinc-400 mb-1">Metrics:</div>
                          <div class="text-xs text-zinc-300 space-x-3">
                            ${step.metrics.prompt_tokens
                              ? html`<span>${step.metrics.prompt_tokens} prompt tokens</span>`
                              : ""}
                            ${step.metrics.completion_tokens
                              ? html`<span>• ${step.metrics.completion_tokens} completion tokens</span>`
                              : ""}
                            ${step.metrics.cost_usd
                              ? html`<span>• $${step.metrics.cost_usd.toFixed(4)}</span>`
                              : ""}
                          </div>
                        </div>
                      `
                    : ""}
                  ${step.error
                    ? html`
                        <div>
                          <div class="text-xs font-mono text-red-400 mb-1">Error:</div>
                          <pre
                            class="text-xs font-mono text-red-300 bg-red-950/20 p-2 rounded border border-red-800/40 overflow-x-auto whitespace-pre-wrap"
                          >${step.error}</pre>
                        </div>
                      `
                    : ""}
                </div>
              `

              return html`
                <div class="border-b border-zinc-800/40">
                  ${stepHeader}
                  ${expandedContent}
                </div>
              `
            })
          )}
        </div>
      `

      return html`
        <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header} ${metadata} ${stepsList}
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
      switch (event.type) {
        case "load": {
          yield* ctx.state.update((s) => ({
            ...s,
            trajectory: event.trajectory,
            sessionId: event.sessionId,
            loading: false,
            error: null,
            expandedStepIds: new Set(),  // Reset expanded steps on new load
          }))
          break
        }

        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "toggleStep": {
          yield* ctx.state.update((s) => {
            const newExpanded = new Set(s.expandedStepIds)
            if (newExpanded.has(event.stepId)) {
              newExpanded.delete(event.stepId)
            } else {
              newExpanded.add(event.stepId)
            }
            return { ...s, expandedStepIds: newExpanded }
          })
          break
        }

        case "clear":
          yield* ctx.state.update(() => HFTrajectoryDetailWidget.initialState())
          break

        case "toggleViewMode": {
          yield* ctx.state.update((s) => ({
            ...s,
            viewMode: s.viewMode === "formatted" ? "json" : "formatted",
          }))
          break
        }
      }
    }),

  subscriptions: () => [],
}

// ============================================================================
// Export initial state for testing
// ============================================================================

export const initialHFTrajectoryDetailState: HFTrajectoryDetailState =
  HFTrajectoryDetailWidget.initialState()
