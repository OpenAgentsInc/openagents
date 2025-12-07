/**
 * TB Command Center Run Browser Widget
 *
 * Browses execution history from both local TB runs and HuggingFace trajectories.
 */

import { Effect, Stream } from "effect"
import { html, joinTemplates } from "../../template/html.js"
import type { Widget } from "../../widget/types.js"
import type { TBRunSummary, TBRunDetail, TBRunSource, TBRunOutcome } from "./types.js"
import { OUTCOME_COLORS } from "./types.js"
import { SocketServiceTag } from "../../services/socket.js"
import type { Trajectory } from "../../../atif/schema.js"

// ============================================================================
// Types
// ============================================================================

export interface TBCCRunBrowserState {
  runs: TBRunSummary[]
  selectedRunId: string | null
  selectedRunDetail: TBRunDetail | null
  dataSource: "all" | "local" | "hf"
  loading: boolean
  loadingDetail: boolean
  error: string | null
  page: number
  pageSize: number
  hasMore: boolean
}

export type TBCCRunBrowserEvent =
  | { type: "loadRuns"; page: number }
  | { type: "selectRun"; runId: string; source: TBRunSource }
  | { type: "changeSource"; source: "all" | "local" | "hf" }
  | { type: "refresh" }

// ============================================================================
// Helpers
// ============================================================================

const formatDate = (iso: string): string => {
  try {
    return new Date(iso).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    })
  } catch {
    return iso.slice(0, 10)
  }
}

const formatDuration = (ms: number | null): string => {
  if (ms === null) return "-"
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return `${minutes}m ${remainingSeconds}s`
}

// ============================================================================
// Widget Definition
// ============================================================================

export const TBCCRunBrowserWidget: Widget<TBCCRunBrowserState, TBCCRunBrowserEvent, SocketServiceTag> = {
  id: "tbcc-run-browser",

  initialState: () => ({
    runs: [],
    selectedRunId: null,
    selectedRunDetail: null,
    dataSource: "all",
    loading: true,
    loadingDetail: false,
    error: null,
    page: 0,
    pageSize: 50,
    hasMore: true,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Filter runs based on source
      const filteredRuns = state.runs.filter((run) => {
        if (state.dataSource === "all") return true
        return run.source === state.dataSource
      })

      // Run List Panel
      const runList = html`
        <div class="flex flex-col h-full border-r border-zinc-800/60 bg-zinc-900/20 w-1/3 min-w-[350px]">
          <!-- Controls -->
          <div class="p-4 border-b border-zinc-800/60 space-y-3">
            <div class="flex items-center justify-between">
              <h3 class="text-sm font-bold font-mono text-zinc-200">Run History</h3>
              <button
                class="text-xs text-zinc-500 hover:text-zinc-300 transition-colors"
                data-action="refresh"
              >
                ↻ Refresh
              </button>
            </div>

            <!-- Source Filter -->
            <div class="flex bg-zinc-900 rounded-lg p-1 border border-zinc-800">
              ${joinTemplates(
        (["all", "local", "hf"] as const).map((source) => {
          const isActive = state.dataSource === source
          const classes = isActive
            ? "bg-zinc-700 text-zinc-100 shadow-sm"
            : "text-zinc-500 hover:text-zinc-300"
          const labels = { all: "All", local: "Local", hf: "HuggingFace" }
          return html`
                    <button
                      class="flex-1 py-1 text-xs font-medium rounded transition-all ${classes}"
                      data-action="changeSource"
                      data-source="${source}"
                    >
                      ${labels[source]}
                    </button>
                  `
        })
      )}
            </div>
          </div>

          <!-- List -->
          <div class="flex-1 overflow-y-auto">
            ${state.loading && state.page === 0
          ? html`<div class="p-8 text-center text-zinc-500 text-sm">Loading runs...</div>`
          : filteredRuns.length === 0
            ? html`<div class="p-8 text-center text-zinc-500 text-sm">No runs found</div>`
            : html`
                    <div class="divide-y divide-zinc-800/40">
                      ${joinTemplates(
              filteredRuns.map((run) => {
                const isSelected = run.id === state.selectedRunId
                const outcomeColors = run.outcome
                  ? OUTCOME_COLORS[run.outcome]
                  : OUTCOME_COLORS.aborted
                const bgClass = isSelected ? "bg-zinc-800/60" : "hover:bg-zinc-800/30"

                return html`
                            <div
                              class="p-4 cursor-pointer transition-colors ${bgClass}"
                              data-action="selectRun"
                              data-run-id="${run.id}"
                              data-source="${run.source}"
                            >
                              <div class="flex items-start justify-between gap-2 mb-1">
                                <div class="font-mono text-sm text-zinc-200 truncate" title="${run.taskName}">
                                  ${run.taskName}
                                </div>
                                <span
                                  class="px-1.5 py-0.5 text-[10px] uppercase rounded border ${outcomeColors.bg} ${outcomeColors.text} ${outcomeColors.border}"
                                >
                                  ${run.outcome ?? "running"}
                                </span>
                              </div>
                              <div class="flex items-center justify-between text-xs text-zinc-500">
                                <span>${formatDate(run.startedAt)}</span>
                                <div class="flex gap-3">
                                  <span>${run.stepsCount} steps</span>
                                  <span>${formatDuration(run.durationMs)}</span>
                                </div>
                              </div>
                              ${run.source === "hf"
                    ? html`
                                    <div class="mt-1 text-[10px] text-zinc-600 flex items-center gap-1">
                                      <span class="w-1.5 h-1.5 rounded-full bg-yellow-600/50"></span>
                                      HF: ${run.agentName}
                                    </div>
                                  `
                    : html`
                                    <div class="mt-1 text-[10px] text-zinc-600 flex items-center gap-1">
                                      <span class="w-1.5 h-1.5 rounded-full bg-blue-600/50"></span>
                                      Local Run
                                    </div>
                                  `}
                            </div>
                          `
              })
            )}
                      ${state.hasMore
                ? html`
                            <div class="p-4 text-center">
                              <button
                                class="text-xs text-zinc-400 hover:text-zinc-200"
                                data-action="loadMore"
                              >
                                Load More ↓
                              </button>
                            </div>
                          `
                : ""}
                    </div>
                  `}
          </div>
        </div>
      `

      // Detail Panel
      const detailPanel = state.selectedRunDetail
        ? html`
            <div class="flex-1 h-full overflow-y-auto bg-zinc-950">
              <!-- Header -->
              <div class="px-6 py-4 border-b border-zinc-800/60 bg-zinc-900/20">
                <div class="flex items-center justify-between mb-2">
                  <h2 class="text-lg font-bold font-mono text-zinc-100">${state.selectedRunDetail.taskName}</h2>
                  <div class="flex items-center gap-2">
                    <span class="text-xs text-zinc-500 font-mono">${state.selectedRunDetail.id}</span>
                    ${state.selectedRunDetail.source === "hf"
            ? html`<span class="px-2 py-0.5 rounded text-xs bg-yellow-900/30 text-yellow-500 border border-yellow-800/50">HuggingFace</span>`
            : html`<span class="px-2 py-0.5 rounded text-xs bg-blue-900/30 text-blue-400 border border-blue-800/50">Local</span>`}
                  </div>
                </div>

                <div class="flex items-center gap-6 text-sm text-zinc-400">
                  <div>
                    <span class="text-zinc-600">Status:</span>
                    <span class="${state.selectedRunDetail.outcome === "success" ? "text-emerald-400" : "text-red-400"} capitalize">
                      ${state.selectedRunDetail.outcome ?? "Unknown"}
                    </span>
                  </div>
                  <div>
                    <span class="text-zinc-600">Steps:</span>
                    <span class="text-zinc-200">${state.selectedRunDetail.stepsCount}</span>
                  </div>
                  <div>
                    <span class="text-zinc-600">Duration:</span>
                    <span class="text-zinc-200">${formatDuration(state.selectedRunDetail.durationMs)}</span>
                  </div>
                </div>
              </div>

              <!-- Steps -->
              <div class="p-6">
                <h3 class="text-sm font-bold text-zinc-500 uppercase mb-4">Execution Steps</h3>
                <div class="space-y-4">
                  ${joinTemplates(
              state.selectedRunDetail.steps.map((step) => {
                const statusColor = step.success ? "border-emerald-900/30" : "border-red-900/30"
                return html`
                        <div class="bg-zinc-900/40 border ${statusColor} rounded-lg overflow-hidden">
                          <div class="px-4 py-2 bg-zinc-900/60 border-b border-zinc-800/40 flex items-center justify-between">
                            <div class="flex items-center gap-3">
                              <span class="text-xs font-mono text-zinc-500">#${step.index}</span>
                              <span class="text-sm font-mono text-zinc-200">${step.actionLabel}</span>
                            </div>
                            <span class="text-xs text-zinc-500">${formatDuration(step.durationMs)}</span>
                          </div>

                          <div class="p-4 space-y-3">
                            ${step.shortReason
                    ? html`
                                  <div class="text-sm text-zinc-400 italic">
                                    "${step.shortReason}"
                                  </div>
                                `
                    : ""}

                            ${step.toolCall
                    ? html`
                                  <div class="bg-zinc-950/50 rounded border border-zinc-800/40 p-3">
                                    <div class="text-xs text-violet-400 font-mono mb-1">Tool Call: ${step.toolCall.functionName}</div>
                                    <pre class="text-xs text-zinc-400 overflow-x-auto whitespace-pre-wrap font-mono">${JSON.stringify(step.toolCall.arguments, null, 2)}</pre>
                                  </div>
                                `
                    : ""}

                            ${step.observation
                    ? html`
                                  <div class="bg-zinc-950/50 rounded border border-zinc-800/40 p-3">
                                    <div class="text-xs text-emerald-400 font-mono mb-1">Observation</div>
                                    <pre class="text-xs text-zinc-400 overflow-x-auto whitespace-pre-wrap font-mono">${typeof step.observation.content === 'string'
                        ? step.observation.content
                        : JSON.stringify(step.observation.content, null, 2)
                      }</pre>
                                  </div>
                                `
                    : ""}
                          </div>
                        </div>
                      `
              })
            )}
                </div>
              </div>
            </div>
          `
        : state.loadingDetail
          ? html`
              <div class="flex-1 h-full flex items-center justify-center text-zinc-500">
                Loading details...
              </div>
            `
          : html`
              <div class="flex-1 h-full flex items-center justify-center text-zinc-500 bg-zinc-950/50">
                <div class="text-center">
                  <div class="text-4xl mb-4 opacity-20">←</div>
                  <div>Select a run to view details</div>
                </div>
              </div>
            `

      return html`
        <div class="flex h-full overflow-hidden">
          ${runList} ${detailPanel}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Source filter
      yield* ctx.dom.delegate(ctx.container, "[data-action='changeSource']", "click", (_e, target) => {
        const source = (target as HTMLElement).dataset.source as "all" | "local" | "hf"
        if (source) {
          Effect.runFork(ctx.emit({ type: "changeSource", source }))
        }
      })

      // Select run
      yield* ctx.dom.delegate(ctx.container, "[data-action='selectRun']", "click", (_e, target) => {
        const runId = (target as HTMLElement).dataset.runId
        const source = (target as HTMLElement).dataset.source as TBRunSource
        if (runId && source) {
          Effect.runFork(ctx.emit({ type: "selectRun", runId, source }))
        }
      })

      // Refresh
      yield* ctx.dom.delegate(ctx.container, "[data-action='refresh']", "click", () => {
        Effect.runFork(ctx.emit({ type: "refresh" }))
      })

      // Load more
      yield* ctx.dom.delegate(ctx.container, "[data-action='loadMore']", "click", () => {
        Effect.runFork(ctx.state.update(s => ({ ...s, page: s.page + 1 })).pipe(
          Effect.flatMap(() => ctx.emit({ type: "loadRuns", page: -1 })) // -1 indicates use current page from state
        ))
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "loadRuns": {
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null }))
          const state = yield* ctx.state.get

          try {
            // Load local runs
            const localRunsRaw = yield* socket.loadRecentTBRuns(20)
            const localRuns: TBRunSummary[] = localRunsRaw.map((r: any) => ({
              id: r.runId,
              source: "local",
              taskId: r.taskIds?.[0] ?? "unknown",
              taskName: r.taskNames?.[0] ?? r.runId,
              outcome: r.outcome as TBRunOutcome,
              status: r.status as "completed" | "error",
              startedAt: r.startedAt,
              finishedAt: r.finishedAt ?? null,
              durationMs: r.durationMs ?? null,
              stepsCount: r.stepsCount ?? 0,
              tokensUsed: r.tokensUsed ?? null,
            }))

            // Load HF runs
            const hfRunsRaw = yield* socket.getHFTrajectories(0, 20)
            const hfRuns: TBRunSummary[] = (hfRunsRaw as Trajectory[]).map((t) => {
              const extra = t.extra as any
              return {
                id: t.session_id,
                source: "hf",
                taskId: extra?.task ?? "unknown",
                taskName: extra?.task ?? "unknown",
                outcome: "success", // HF runs are typically successful demos
                status: "completed",
                startedAt: extra?.date ?? t.steps[0]?.timestamp ?? new Date().toISOString(),
                finishedAt: null,
                durationMs: null,
                stepsCount: t.steps.length,
                tokensUsed: null,
                agentName: t.agent?.name,
                modelName: t.agent?.model_name,
                episode: extra?.episode,
              }
            })

            // Merge and sort
            const allRuns = [...localRuns, ...hfRuns].sort((a, b) =>
              new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime()
            )

            yield* ctx.state.update((s) => ({
              ...s,
              runs: allRuns,
              loading: false,
            }))
          } catch (error) {
            yield* ctx.state.update((s) => ({
              ...s,
              loading: false,
              error: error instanceof Error ? error.message : String(error),
            }))
          }
          break
        }

        case "changeSource": {
          yield* ctx.state.update((s) => ({ ...s, dataSource: event.source }))
          break
        }

        case "selectRun": {
          yield* ctx.state.update((s) => ({
            ...s,
            selectedRunId: event.runId,
            loadingDetail: true,
            selectedRunDetail: null
          }))

          try {
            let detail: TBRunDetail | null = null

            if (event.source === "local") {
              const runData = yield* socket.loadTBRunDetails(event.runId)
              if (runData) {
                // Map TBRunDetails (protocol) to TBRunDetail (widget state)
                // Note: TBRunDetails contains aggregate stats + list of task results
                // For the detail view, we want to show the steps of the *first* task if it's a single task run,
                // or a summary if it's a multi-task run.
                // For now, let's assume single-task runs for the detail view or just show the first task's info.

                // We need to fetch the actual steps/logs which might not be in TBRunDetails fully?
                // Actually TBRunDetails in protocol.ts only has task summaries, not the full steps/logs.
                // We might need a different API to get the full logs/steps for a specific run/task.
                // But for now let's map what we have.

                const firstTask = runData.tasks[0]

                detail = {
                  id: runData.meta.runId,
                  source: "local",
                  taskId: firstTask?.id ?? "unknown",
                  taskName: firstTask?.name ?? runData.meta.suiteName,
                  outcome: (firstTask?.outcome as TBRunOutcome) ?? "unknown",
                  status: "completed", // Assumed if we have details
                  startedAt: runData.meta.timestamp,
                  finishedAt: null,
                  durationMs: runData.meta.totalDurationMs,
                  stepsCount: firstTask?.turns ?? 0,
                  tokensUsed: runData.meta.totalTokens,
                  steps: [], // We don't have steps in TBRunDetails yet
                  terminalOutput: { stdout: [], stderr: [] }
                }
              }
            } else {
              // For HF, we need to fetch the trajectory again or cache it
              // For now, let's just fetch a single one if possible, or filter from list if we have full data
              // Since getHFTrajectories returns full objects, we might already have it in memory if we stored it
              // But our state only stores summaries.
              // Let's re-fetch a small batch around it or implement getHFTrajectoryById
              // For MVP, we'll just fetch the list again and find it (inefficient but works for demo)
              const hfRunsRaw = yield* socket.getHFTrajectories(0, 100) // Hacky
              const t = (hfRunsRaw as Trajectory[]).find(t => t.session_id === event.runId)

              if (t) {
                const extra = t.extra as any
                detail = {
                  id: t.session_id,
                  source: "hf",
                  taskId: extra?.task ?? "unknown",
                  taskName: extra?.task ?? "unknown",
                  outcome: "success",
                  status: "completed",
                  startedAt: extra?.date ?? t.steps[0]?.timestamp ?? new Date().toISOString(),
                  finishedAt: null,
                  durationMs: null,
                  stepsCount: t.steps.length,
                  tokensUsed: null,
                  agentName: t.agent?.name,
                  modelName: t.agent?.model_name,
                  episode: extra?.episode,
                  steps: t.steps.map((s, i) => ({
                    id: s.step_id.toString(),
                    index: i + 1,
                    actionType: "CUSTOM",
                    actionLabel: s.tool_calls?.[0]?.function_name ?? "Thought",
                    shortReason: s.reasoning_content ?? (typeof s.message === 'string' ? s.message : '') ?? "",
                    details: null,
                    timestamp: s.timestamp,
                    success: !s.error,
                    durationMs: null,
                    toolCall: s.tool_calls?.[0] ? {
                      functionName: s.tool_calls[0].function_name,
                      arguments: s.tool_calls[0].arguments
                    } : undefined,
                    observation: s.observation ? {
                      content: s.observation.results[0]?.content,
                      truncated: false
                    } : undefined
                  })),
                  terminalOutput: { stdout: [], stderr: [] }
                }
              }
            }

            yield* ctx.state.update((s) => ({
              ...s,
              selectedRunDetail: detail,
              loadingDetail: false
            }))
          } catch (error) {
            yield* ctx.state.update((s) => ({
              ...s,
              loadingDetail: false,
              error: error instanceof Error ? error.message : String(error)
            }))
          }
          break
        }

        case "refresh": {
          yield* ctx.emit({ type: "loadRuns", page: 0 })
          break
        }
      }
    }),

  subscriptions: (ctx) => {
    return [Stream.make(ctx.emit({ type: "loadRuns", page: 0 }))]
  },
}
