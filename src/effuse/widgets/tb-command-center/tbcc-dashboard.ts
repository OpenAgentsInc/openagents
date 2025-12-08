/**
 * TB Command Center Dashboard Widget
 *
 * Shows KPIs, quick actions, and recent runs table.
 */

import { Effect, Stream } from "effect"
import { html, joinTemplates } from "../../template/html.js"
import type { Widget } from "../../widget/types.js"
import type {
  DashboardStats,
  CurrentRunInfo,
  TBRunSummary,
  TBRunOutcome,
  TBModelOption,
} from "./types.js"
import { OUTCOME_COLORS, DEFAULT_EXECUTION_SETTINGS } from "./types.js"
import { SocketServiceTag } from "../../services/socket.js"

// ============================================================================
// Settings Helpers
// ============================================================================

const STORAGE_KEY = "tbcc_settings"
const DEFAULT_SUITE_PATH = "tasks/terminal-bench-2.json"

/** Read saved settings from localStorage */
const getSettings = (): { model: TBModelOption } => {
  if (typeof localStorage === "undefined") {
    return { model: DEFAULT_EXECUTION_SETTINGS.model }
  }
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) {
      const parsed = JSON.parse(saved)
      return { model: parsed.execution?.model ?? DEFAULT_EXECUTION_SETTINGS.model }
    }
  } catch {
    // Ignore parse errors
  }
  return { model: DEFAULT_EXECUTION_SETTINGS.model }
}

// ============================================================================
// Types
// ============================================================================

export interface TBCCDashboardState {
  /** Aggregated stats from recent runs */
  stats: DashboardStats | null
  /** Recent runs for table */
  recentRuns: TBRunSummary[]
  /** Current active run */
  currentRun: CurrentRunInfo | null
  /** Loading state */
  loading: boolean
  /** Error message */
  error: string | null
}

export type TBCCDashboardEvent =
  | { type: "refresh" }
  | { type: "runFullBenchmark" }
  | { type: "runRandomTask" }
  | { type: "viewRun"; runId: string }
  | { type: "runStarted"; runId: string; taskName: string }
  | { type: "taskCompleted"; outcome: TBRunOutcome }
  | { type: "runCompleted"; runId: string }

// ============================================================================
// Helpers
// ============================================================================

const formatDuration = (ms: number | null): string => {
  if (ms === null) return "-"
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = seconds % 60
  return `${minutes}m ${remainingSeconds}s`
}

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

const formatPercent = (value: number): string => {
  return `${(value * 100).toFixed(1)}%`
}

// ============================================================================
// Widget Definition
// ============================================================================

export const TBCCDashboardWidget: Widget<TBCCDashboardState, TBCCDashboardEvent, SocketServiceTag> = {
  id: "tbcc-dashboard",

  initialState: () => ({
    stats: null,
    recentRuns: [],
    currentRun: null,
    loading: true,
    error: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Current run status card
      const currentRunCard = state.currentRun
        ? html`
            <div class="bg-blue-900/20 border border-blue-800/50 rounded-xl p-4 mb-4">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <span class="w-3 h-3 rounded-full bg-blue-500 animate-pulse"></span>
                  <div>
                    <div class="text-sm font-mono text-blue-200">${state.currentRun.taskName}</div>
                    <div class="text-xs text-blue-400">
                      Step ${state.currentRun.currentStep}${state.currentRun.totalSteps ? ` / ${state.currentRun.totalSteps}` : ""} · Attempt
                      ${state.currentRun.attempt}/${state.currentRun.maxAttempts}
                    </div>
                  </div>
                </div>
                <button
                  class="px-3 py-1.5 text-xs font-mono rounded border border-red-700 text-red-300 bg-red-900/40 hover:bg-red-900/60 transition-colors"
                  data-action="stopRun"
                >
                  Stop
                </button>
              </div>
            </div>
          `
        : ""

      // KPI cards
      const kpiCards = state.stats
        ? html`
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-6">
              <!-- Overall Success Rate -->
              <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-4">
                <div class="text-xs text-zinc-500 mb-1">Success Rate</div>
                <div class="text-2xl font-mono font-bold text-emerald-400">${formatPercent(state.stats.overallSuccessRate)}</div>
                <div class="text-xs text-zinc-500 mt-1">Last 50: ${formatPercent(state.stats.last50SuccessRate)}</div>
              </div>

              <!-- Average Steps -->
              <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-4">
                <div class="text-xs text-zinc-500 mb-1">Avg Steps</div>
                <div class="text-2xl font-mono font-bold text-zinc-200">${state.stats.avgStepsPerRun.toFixed(1)}</div>
                <div class="text-xs text-zinc-500 mt-1">per run</div>
              </div>

              <!-- Average Duration -->
              <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-4">
                <div class="text-xs text-zinc-500 mb-1">Avg Duration</div>
                <div class="text-2xl font-mono font-bold text-zinc-200">${formatDuration(state.stats.avgDurationSeconds * 1000)}</div>
                <div class="text-xs text-zinc-500 mt-1">per run</div>
              </div>

              <!-- Total Runs -->
              <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-4">
                <div class="text-xs text-zinc-500 mb-1">Total Runs</div>
                <div class="text-2xl font-mono font-bold text-zinc-200">${state.stats.totalRuns}</div>
                <div class="text-xs text-zinc-500 mt-1">all time</div>
              </div>
            </div>
          `
        : html`
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-6">
              ${joinTemplates(
          [1, 2, 3, 4].map(
            () => html`
                    <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-4 animate-pulse">
                      <div class="h-3 bg-zinc-800 rounded w-16 mb-2"></div>
                      <div class="h-6 bg-zinc-800 rounded w-12"></div>
                    </div>
                  `
          )
        )}
            </div>
          `

      // Quick actions
      const quickActions = html`
        <div class="flex gap-3 mb-6">
          <button
            type="button"
            class="px-4 py-2 text-xs font-mono uppercase rounded border border-emerald-700 text-emerald-300 bg-emerald-900/40 hover:bg-emerald-900/60 transition-colors"
            data-action="runFullBenchmark"
            ${state.currentRun ? "disabled" : ""}
          >
            ▶ Run Full Benchmark
          </button>
          <button
            class="px-4 py-2 text-xs font-mono uppercase rounded border border-zinc-700 text-zinc-200 bg-zinc-900/80 hover:bg-zinc-900/95 transition-colors"
            data-action="runRandomTask"
            ${state.currentRun ? "disabled" : ""}
          >
            🎲 Random Task
          </button>
          <button
            class="px-4 py-2 text-xs font-mono uppercase rounded border border-zinc-700 text-zinc-200 bg-zinc-900/80 hover:bg-zinc-900/95 transition-colors"
            data-action="refresh"
          >
            ↻ Refresh
          </button>
        </div>
      `

      // Recent runs table
      const recentRunsTable =
        state.recentRuns.length > 0
          ? html`
              <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg overflow-hidden">
                <div class="px-4 py-3 border-b border-zinc-800/40">
                  <h3 class="text-sm font-bold font-mono text-zinc-200">Recent Runs</h3>
                </div>
                <div class="max-h-80 overflow-y-auto">
                  <table class="w-full text-sm">
                    <thead class="sticky top-0 bg-zinc-900/80 border-b border-zinc-800/40 text-left text-zinc-400 text-xs font-mono">
                      <tr>
                        <th class="px-4 py-2">Task</th>
                        <th class="px-4 py-2">Outcome</th>
                        <th class="px-4 py-2">Steps</th>
                        <th class="px-4 py-2">Duration</th>
                        <th class="px-4 py-2">Date</th>
                        <th class="px-4 py-2"></th>
                      </tr>
                    </thead>
                    <tbody>
                      ${joinTemplates(
            state.recentRuns.slice(0, 10).map((run) => {
              const outcomeColors = run.outcome ? OUTCOME_COLORS[run.outcome] : OUTCOME_COLORS.error
              return html`
                            <tr class="border-b border-zinc-800/20 hover:bg-zinc-800/30 transition-colors">
                              <td class="px-4 py-2 font-mono text-zinc-200">${run.taskName}</td>
                              <td class="px-4 py-2">
                                <span
                                  class="px-2 py-0.5 rounded text-xs font-mono ${outcomeColors.bg} ${outcomeColors.text} border ${outcomeColors.border}"
                                >
                                  ${run.outcome ?? "running"}
                                </span>
                              </td>
                              <td class="px-4 py-2 text-zinc-400">${run.stepsCount}</td>
                              <td class="px-4 py-2 text-zinc-400">${formatDuration(run.durationMs)}</td>
                              <td class="px-4 py-2 text-zinc-500 text-xs">${formatDate(run.startedAt)}</td>
                              <td class="px-4 py-2">
                                <button
                                  class="text-zinc-400 hover:text-zinc-200 text-xs transition-colors"
                                  data-action="viewRun"
                                  data-run-id="${run.id}"
                                >
                                  View →
                                </button>
                              </td>
                            </tr>
                          `
            })
          )}
                    </tbody>
                  </table>
                </div>
              </div>
            `
          : state.loading
            ? html`
                <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-8 text-center">
                  <div class="text-zinc-500">Loading recent runs...</div>
                </div>
              `
            : html`
                <div class="bg-zinc-900/60 border border-zinc-800/40 rounded-lg p-8 text-center">
                  <div class="text-zinc-500 mb-2">No runs yet</div>
                  <div class="text-xs text-zinc-600">Run a benchmark to see results here</div>
                </div>
              `

      // Error state
      const errorBanner = state.error
        ? html`
            <div class="bg-red-900/20 border border-red-800/50 rounded-lg p-4 mb-4">
              <div class="text-sm text-red-300">${state.error}</div>
              <button
                class="mt-2 text-xs text-red-400 hover:text-red-300 transition-colors"
                data-action="refresh"
              >
                Retry
              </button>
            </div>
          `
        : ""

      return html`
        <div class="h-full overflow-y-auto p-6">
          <h2 class="text-xl font-bold font-mono text-zinc-100 mb-6">Dashboard</h2>
          ${errorBanner} ${currentRunCard} ${kpiCards} ${quickActions} ${recentRunsTable}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      if (typeof window !== "undefined" && (window as any).bunLog) {
        (window as any).bunLog("[TBCCDashboard] Setting up event handlers")
      }
      
      // Quick action buttons
      yield* ctx.dom.delegate(ctx.container, "button[data-action]", "click", (e, target) => {
        const button = target as HTMLButtonElement
        const action = button.dataset.action
        
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog(`[TBCCDashboard] Click detected on button with action="${action}", disabled=${button.disabled}`)
        }
        
        // Ignore clicks on disabled buttons
        if (button.disabled) {
          if (typeof window !== "undefined" && (window as any).bunLog) {
            (window as any).bunLog(`[TBCCDashboard] Button is disabled, ignoring click`)
          }
          return
        }
        
        const runId = button.dataset.runId

        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog(`[TBCCDashboard] Processing action="${action}"`)
        }

        if (action === "refresh") {
          Effect.runFork(ctx.emit({ type: "refresh" }))
        } else if (action === "runFullBenchmark") {
          if (typeof window !== "undefined" && (window as any).bunLog) {
            (window as any).bunLog(`[TBCCDashboard] Emitting runFullBenchmark event`)
          }
          Effect.runFork(ctx.emit({ type: "runFullBenchmark" }))
        } else if (action === "runRandomTask") {
          Effect.runFork(ctx.emit({ type: "runRandomTask" }))
        } else if (action === "viewRun" && runId) {
          Effect.runFork(ctx.emit({ type: "viewRun", runId }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "refresh": {
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null }))

          try {
            // Load recent runs
            const runs = yield* socket.loadRecentTBRuns(20)
            const recentRuns: TBRunSummary[] = runs.map((r: any) => ({
              id: r.runId,
              source: "local" as const,
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

            // Compute stats from runs
            const stats = computeStats(recentRuns)

            yield* ctx.state.update((s) => ({
              ...s,
              recentRuns,
              stats,
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

        case "runFullBenchmark": {
          if (typeof window !== "undefined" && (window as any).bunLog) {
            (window as any).bunLog(`[TBCCDashboard] handleEvent: runFullBenchmark received`)
          }
          
          // Read model preference from settings
          const settings = getSettings()
          if (typeof window !== "undefined" && (window as any).bunLog) {
            (window as any).bunLog(`[TBCCDashboard] Settings:`, JSON.stringify(settings))
          }
          
          const runOptions = {
            suitePath: DEFAULT_SUITE_PATH,
            model: settings.model,
          }
          
          if (typeof window !== "undefined" && (window as any).bunLog) {
            (window as any).bunLog(`[TBCCDashboard] Starting run with options:`, JSON.stringify(runOptions))
          }
          yield* Effect.tryPromise({
            try: async () => {
              const result = await Effect.runPromise(socket.startTBRun(runOptions))
              return result
            },
            catch: (e) => e,
          }).pipe(
            Effect.flatMap((result: any) =>
              ctx.state.update((s) => ({
                ...s,
                currentRun: {
                  runId: result.runId,
                  taskId: "all",
                  taskName: `Full Benchmark (${settings.model})`,
                  attempt: 1,
                  maxAttempts: 1,
                  status: "running",
                  startedAt: Date.now(),
                  currentStep: 0,
                  totalSteps: null,
                },
              }))
            ),
            Effect.catchAll((error) =>
              ctx.state.update((s) => ({
                ...s,
                error: `Failed to start benchmark: ${error instanceof Error ? error.message : String(error)}`,
              }))
            )
          )
          break
        }

        case "runRandomTask": {
          // Read model preference from settings
          const settings = getSettings()
          yield* Effect.tryPromise({
            try: async () => {
              const result = await Effect.runPromise(socket.startTBRun({
                suitePath: DEFAULT_SUITE_PATH,
                model: settings.model,
                random: true,
              }))
              return result
            },
            catch: (e) => e,
          }).pipe(
            Effect.flatMap((result: any) =>
              ctx.state.update((s) => ({
                ...s,
                currentRun: {
                  runId: result.runId,
                  taskId: "random",
                  taskName: `Random Task (${settings.model})`,
                  attempt: 1,
                  maxAttempts: 5,
                  status: "running",
                  startedAt: Date.now(),
                  currentStep: 0,
                  totalSteps: null,
                },
              }))
            ),
            Effect.catchAll((error) =>
              ctx.state.update((s) => ({
                ...s,
                error: `Failed to start task: ${error instanceof Error ? error.message : String(error)}`,
              }))
            )
          )
          break
        }

        case "runStarted": {
          yield* ctx.state.update((s) => ({
            ...s,
            currentRun: {
              runId: event.runId,
              taskId: "unknown",
              taskName: event.taskName,
              attempt: 1,
              maxAttempts: 5,
              status: "running",
              startedAt: Date.now(),
              currentStep: 0,
              totalSteps: null,
            },
          }))
          break
        }

        case "runCompleted": {
          yield* ctx.state.update((s) => ({ ...s, currentRun: null }))
          // Refresh data
          yield* ctx.emit({ type: "refresh" })
          break
        }

        case "viewRun": {
          // This will be handled by parent shell to navigate to Runs tab
          console.log("[Dashboard] View run:", event.runId)
          break
        }
      }
    }).pipe(
      Effect.catchAll((error) =>
        ctx.state.update((s) => ({
          ...s,
          error: `Unexpected error: ${error instanceof Error ? error.message : String(error)}`
        }))
      )
    ),

  subscriptions: (ctx) => {
    // Initial load
    const initialLoad = Stream.make(ctx.emit({ type: "refresh" }))

    // Subscribe to run events
    const runEvents = Stream.unwrap(
      Effect.gen(function* () {
        const socket = yield* SocketServiceTag
        return socket.getMessages().pipe(
          Stream.map((msg) =>
            Effect.gen(function* () {
              if (msg.type === "tb_run_start") {
                const data = msg as { runId: string; taskNames?: string[] }
                yield* ctx.emit({
                  type: "runStarted",
                  runId: data.runId,
                  taskName: data.taskNames?.[0] ?? "Task",
                })
              } else if (msg.type === "tb_run_complete") {
                const data = msg as { runId: string }
                yield* ctx.emit({ type: "runCompleted", runId: data.runId })
              } else if (msg.type === "tb_task_complete") {
                const data = msg as { outcome: string }
                yield* ctx.emit({ type: "taskCompleted", outcome: data.outcome as TBRunOutcome })
              }
            })
          )
        )
      })
    )

    return [initialLoad, runEvents]
  },
}

// ============================================================================
// Helper Functions
// ============================================================================

function computeStats(runs: TBRunSummary[]): DashboardStats {
  if (runs.length === 0) {
    return {
      overallSuccessRate: 0,
      last50SuccessRate: 0,
      avgStepsPerRun: 0,
      avgDurationSeconds: 0,
      totalRuns: 0,
      byDifficulty: {
        easy: { passed: 0, total: 0 },
        medium: { passed: 0, total: 0 },
        hard: { passed: 0, total: 0 },
        expert: { passed: 0, total: 0 },
        unknown: { passed: 0, total: 0 },
      },
    }
  }

  const completedRuns = runs.filter((r) => r.outcome !== null)
  const passedRuns = completedRuns.filter((r) => r.outcome === "success")

  const last50 = completedRuns.slice(0, 50)
  const last50Passed = last50.filter((r) => r.outcome === "success")

  const totalSteps = completedRuns.reduce((sum, r) => sum + r.stepsCount, 0)
  const totalDuration = completedRuns.reduce((sum, r) => sum + (r.durationMs ?? 0), 0)

  return {
    overallSuccessRate: completedRuns.length > 0 ? passedRuns.length / completedRuns.length : 0,
    last50SuccessRate: last50.length > 0 ? last50Passed.length / last50.length : 0,
    avgStepsPerRun: completedRuns.length > 0 ? totalSteps / completedRuns.length : 0,
    avgDurationSeconds: completedRuns.length > 0 ? totalDuration / completedRuns.length / 1000 : 0,
    totalRuns: runs.length,
    byDifficulty: {
      easy: { passed: 0, total: 0 },
      medium: { passed: 0, total: 0 },
      hard: { passed: 0, total: 0 },
      expert: { passed: 0, total: 0 },
      unknown: { passed: 0, total: 0 },
    },
  }
}
