/**
 * TB Results Component
 *
 * Displays Terminal Bench run results including summary stats,
 * per-task results, and metrics breakdowns.
 */

import { Effect, Stream, pipe } from "effect"
import type { Component } from "../component/types.js"
import { html, joinTemplates } from "../template/html.js"
import { SocketServiceTag } from "../services/socket.js"
import type { TBRunCompleteMessage, TBTaskCompleteMessage, TBDifficulty, TBTaskOutcome, HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

export interface TBTaskResult {
  taskId: string
  taskName: string
  category: string
  difficulty: TBDifficulty
  outcome: TBTaskOutcome
  durationMs: number
  turns: number
  tokens: number
}

export interface TBRunResult {
  runId: string
  suiteName: string
  suiteVersion: string
  passRate: number
  passed: number
  failed: number
  timeout: number
  error: number
  totalDurationMs: number
  totalTasks: number
  totalTokens: number
  taskResults: TBTaskResult[]
  timestamp: string
}

export interface TBResultsState {
  /** Current run result (if viewing one) */
  currentResult: TBRunResult | null
  /** Task results being accumulated during active run */
  activeTasks: Map<string, TBTaskResult>
  /** Active run ID being tracked */
  activeRunId: string | null
  /** Collapsed state */
  collapsed: boolean
  /** Sort column */
  sortBy: "taskId" | "outcome" | "duration" | "turns" | "tokens"
  /** Sort direction */
  sortDir: "asc" | "desc"
  /** Filter by outcome */
  outcomeFilter: TBTaskOutcome | null
}

export type TBResultsEvent =
  | { type: "toggleCollapse" }
  | { type: "clear" }
  | { type: "setSort"; column: "taskId" | "outcome" | "duration" | "turns" | "tokens" }
  | { type: "setOutcomeFilter"; outcome: TBTaskOutcome | null }

// ============================================================================
// Helpers
// ============================================================================

const formatDuration = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${minutes}m ${secs}s`
}

const getOutcomeIcon = (outcome: TBTaskOutcome): string => {
  switch (outcome) {
    case "success":
      return "✓"
    case "failure":
      return "✗"
    case "timeout":
      return "⏱"
    case "error":
      return "⚠"
  }
}

const getOutcomeClass = (outcome: TBTaskOutcome): string => {
  switch (outcome) {
    case "success":
      return "text-emerald-400"
    case "failure":
      return "text-red-400"
    case "timeout":
      return "text-amber-400"
    case "error":
      return "text-orange-400"
  }
}

const getDifficultyBadge = (difficulty: TBDifficulty): string => {
  const classes = {
    easy: "bg-emerald-900/40 text-emerald-300 border-emerald-700/50",
    medium: "bg-amber-900/40 text-amber-300 border-amber-700/50",
    hard: "bg-red-900/40 text-red-300 border-red-700/50",
    expert: "bg-purple-900/40 text-purple-300 border-purple-700/50",
  }
  const labels = {
    easy: "E",
    medium: "M",
    hard: "H",
    expert: "X",
  }
  return `<span class="text-xs px-1.5 py-0.5 rounded border font-mono ${classes[difficulty]}">${labels[difficulty]}</span>`
}

const sortTasks = (
  tasks: TBTaskResult[],
  sortBy: TBResultsState["sortBy"],
  sortDir: TBResultsState["sortDir"]
): TBTaskResult[] => {
  const sorted = [...tasks].sort((a, b) => {
    let aVal: number | string
    let bVal: number | string

    switch (sortBy) {
      case "taskId":
        aVal = a.taskId
        bVal = b.taskId
        break
      case "outcome":
        aVal = a.outcome
        bVal = b.outcome
        break
      case "duration":
        aVal = a.durationMs
        bVal = b.durationMs
        break
      case "turns":
        aVal = a.turns
        bVal = b.turns
        break
      case "tokens":
        aVal = a.tokens
        bVal = b.tokens
        break
    }

    if (typeof aVal === "string" && typeof bVal === "string") {
      return sortDir === "asc" ? aVal.localeCompare(bVal) : bVal.localeCompare(aVal)
    } else {
      return sortDir === "asc" ? (aVal as number) - (bVal as number) : (bVal as number) - (aVal as number)
    }
  })

  return sorted
}

// ============================================================================
// Type Guards
// ============================================================================

const isTBRunComplete = (
  msg: unknown
): msg is TBRunCompleteMessage & {
  type: "tb_run_complete"
  runId: string
  suiteName?: string
  suiteVersion?: string
  totalTasks?: number
  totalTokens?: number
} => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return m.type === "tb_run_complete" && typeof m.runId === "string"
}

const isTBTaskComplete = (
  msg: unknown
): msg is TBTaskCompleteMessage & {
  type: "tb_task_complete"
  runId: string
  taskId: string
  taskName?: string
  category?: string
  difficulty?: TBDifficulty
} => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return m.type === "tb_task_complete" && typeof m.runId === "string" && typeof m.taskId === "string"
}

const isTBRunStart = (
  msg: unknown
): msg is {
  type: "tb_run_start"
  runId: string
  suiteName: string
  suiteVersion: string
  totalTasks: number
  timestamp: string
} => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return (
    m.type === "tb_run_start" &&
    typeof m.runId === "string" &&
    typeof m.suiteName === "string" &&
    typeof m.suiteVersion === "string"
  )
}

const isTBTaskStart = (
  msg: unknown
): msg is {
  type: "tb_task_start"
  runId: string
  taskId: string
  taskName: string
  category: string
  difficulty: TBDifficulty
} => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return (
    m.type === "tb_task_start" &&
    typeof m.runId === "string" &&
    typeof m.taskId === "string" &&
    typeof m.taskName === "string"
  )
}

// ============================================================================
// Component Definition
// ============================================================================

export const TBResultsComponent: Component<TBResultsState, TBResultsEvent, SocketServiceTag> = {
  id: "tb-results",

  initialState: () => ({
    currentResult: null,
    activeTasks: new Map(),
    activeRunId: null,
    collapsed: false,
    sortBy: "taskId",
    sortDir: "asc",
    outcomeFilter: null,
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
            <h3 class="text-sm font-bold font-mono text-zinc-100">Run Results</h3>
            ${state.currentResult ? html`<span class="text-xs text-zinc-400">${state.currentResult.runId.slice(-8)}</span>` : ""}
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
      if (!state.currentResult) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">No results to display</div>
          </div>
        `
      }

      const result = state.currentResult

      // Summary stats
      const summary = html`
        <div class="px-4 py-3 bg-zinc-900/20 border-b border-zinc-800/40">
          <div class="grid grid-cols-4 gap-4 text-xs font-mono">
            <div>
              <div class="text-zinc-500 mb-1">Pass Rate</div>
              <div class="text-lg font-bold ${result.passRate >= 0.8 ? "text-emerald-400" : result.passRate >= 0.5 ? "text-amber-400" : "text-red-400"}">
                ${Math.round(result.passRate * 100)}%
              </div>
            </div>
            <div>
              <div class="text-zinc-500 mb-1">Results</div>
              <div class="text-zinc-200">
                <span class="text-emerald-400">✓${result.passed}</span>
                <span class="text-zinc-500 mx-1">|</span>
                <span class="text-red-400">✗${result.failed}</span>
                ${result.timeout > 0 ? html`<span class="text-zinc-500 mx-1">|</span><span class="text-amber-400">⏱${result.timeout}</span>` : ""}
                ${result.error > 0 ? html`<span class="text-zinc-500 mx-1">|</span><span class="text-orange-400">⚠${result.error}</span>` : ""}
              </div>
            </div>
            <div>
              <div class="text-zinc-500 mb-1">Duration</div>
              <div class="text-zinc-200">${formatDuration(result.totalDurationMs)}</div>
            </div>
            <div>
              <div class="text-zinc-500 mb-1">Tokens</div>
              <div class="text-zinc-200">${result.totalTokens.toLocaleString()}</div>
            </div>
          </div>
        </div>
      `

      // Filter controls
      const filterControls = html`
        <div class="px-4 py-2 bg-zinc-900/20 border-b border-zinc-800/40 flex items-center gap-2">
          <span class="text-xs text-zinc-500 font-mono">Filter:</span>
          <select
            class="bg-zinc-900/60 border border-zinc-700/50 rounded px-2 py-1 text-xs font-mono text-zinc-200 focus:border-zinc-600/60 focus:outline-none"
            data-input="outcomeFilter"
          >
            <option value="" ${!state.outcomeFilter ? "selected" : ""}>All Outcomes</option>
            <option value="success" ${state.outcomeFilter === "success" ? "selected" : ""}>Success</option>
            <option value="failure" ${state.outcomeFilter === "failure" ? "selected" : ""}>Failure</option>
            <option value="timeout" ${state.outcomeFilter === "timeout" ? "selected" : ""}>Timeout</option>
            <option value="error" ${state.outcomeFilter === "error" ? "selected" : ""}>Error</option>
          </select>
          <button
            class="ml-auto text-xs text-zinc-400 hover:text-zinc-200 font-mono"
            data-action="clear"
          >
            Clear
          </button>
        </div>
      `

      // Task results table
      let filteredTasks = result.taskResults
      if (state.outcomeFilter) {
        filteredTasks = filteredTasks.filter((t) => t.outcome === state.outcomeFilter)
      }
      const sortedTasks = sortTasks(filteredTasks, state.sortBy, state.sortDir)

      const taskTable = html`
        <div class="max-h-96 overflow-y-auto">
          <table class="w-full text-xs font-mono">
            <thead class="sticky top-0 bg-zinc-900/80 border-b border-zinc-800/40">
              <tr class="text-left text-zinc-400">
                <th class="px-4 py-2 cursor-pointer hover:text-zinc-200" data-action="setSort" data-column="taskId">
                  Task ${state.sortBy === "taskId" ? (state.sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th class="px-4 py-2">Diff</th>
                <th class="px-4 py-2 cursor-pointer hover:text-zinc-200" data-action="setSort" data-column="outcome">
                  Status ${state.sortBy === "outcome" ? (state.sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th class="px-4 py-2 cursor-pointer hover:text-zinc-200 text-right" data-action="setSort" data-column="duration">
                  Duration ${state.sortBy === "duration" ? (state.sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th class="px-4 py-2 cursor-pointer hover:text-zinc-200 text-right" data-action="setSort" data-column="turns">
                  Turns ${state.sortBy === "turns" ? (state.sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
                <th class="px-4 py-2 cursor-pointer hover:text-zinc-200 text-right" data-action="setSort" data-column="tokens">
                  Tokens ${state.sortBy === "tokens" ? (state.sortDir === "asc" ? "▲" : "▼") : ""}
                </th>
              </tr>
            </thead>
            <tbody>
              ${joinTemplates(
                sortedTasks.map(
                  (task) => html`
                    <tr class="border-b border-zinc-800/20 hover:bg-zinc-900/40">
                      <td class="px-4 py-2 text-zinc-200">
                        <div class="truncate max-w-xs" title="${task.taskName}">${task.taskName}</div>
                        <div class="text-zinc-500 text-[10px]">${task.taskId}</div>
                      </td>
                      <td class="px-4 py-2">${getDifficultyBadge(task.difficulty)}</td>
                      <td class="px-4 py-2 ${getOutcomeClass(task.outcome)}">
                        ${getOutcomeIcon(task.outcome)} ${task.outcome}
                      </td>
                      <td class="px-4 py-2 text-zinc-400 text-right">${formatDuration(task.durationMs)}</td>
                      <td class="px-4 py-2 text-zinc-400 text-right">${task.turns}</td>
                      <td class="px-4 py-2 text-zinc-400 text-right">${task.tokens.toLocaleString()}</td>
                    </tr>
                  `
                )
              )}
            </tbody>
          </table>
          ${filteredTasks.length === 0 ? html`<div class="px-4 py-8 text-center text-sm text-zinc-500">No tasks match filter</div>` : ""}
        </div>
      `

      return html`
        <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header} ${summary} ${filterControls} ${taskTable}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Handle button clicks
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        const column = el.dataset.column

        if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        } else if (action === "clear") {
          Effect.runFork(ctx.emit({ type: "clear" }))
        } else if (action === "setSort" && column) {
          Effect.runFork(
            ctx.emit({
              type: "setSort",
              column: column as "taskId" | "outcome" | "duration" | "turns" | "tokens",
            })
          )
        }
      })

      // Handle filter changes
      yield* ctx.dom.delegate(ctx.container, "[data-input='outcomeFilter']", "change", (e, target) => {
        const select = target as HTMLSelectElement
        const value = select.value
        Effect.runFork(
          ctx.emit({
            type: "setOutcomeFilter",
            outcome: value ? (value as TBTaskOutcome) : null,
          })
        )
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "clear":
          yield* ctx.state.update(() => TBResultsComponent.initialState())
          break

        case "setSort": {
          yield* ctx.state.update((s) => ({
            ...s,
            sortBy: event.column,
            sortDir: s.sortBy === event.column && s.sortDir === "asc" ? "desc" : "asc",
          }))
          break
        }

        case "setOutcomeFilter":
          yield* ctx.state.update((s) => ({ ...s, outcomeFilter: event.outcome }))
          break
      }
    }),

  subscriptions: (ctx) => {
    const socket = Effect.map(SocketServiceTag, (s) => s)
    const isTBMessage = (msg: unknown): msg is HudMessage => {
      if (typeof msg !== "object" || msg === null) return false
      const m = msg as Record<string, unknown>
      return typeof m.type === "string" && m.type.startsWith("tb_")
    }

    return [
      pipe(
        Stream.unwrap(Effect.map(socket, (s) => s.getMessages())),
        Stream.filter((msg): msg is HudMessage => isTBMessage(msg)),
        Stream.map((msg) =>
          Effect.gen(function* () {
            // Track run start
            if (isTBRunStart(msg)) {
              yield* ctx.state.update((s) => ({
                ...s,
                activeRunId: msg.runId,
                activeTasks: new Map(),
                currentResult: {
                  runId: msg.runId,
                  suiteName: msg.suiteName,
                  suiteVersion: msg.suiteVersion,
                  passRate: 0,
                  passed: 0,
                  failed: 0,
                  timeout: 0,
                  error: 0,
                  totalDurationMs: 0,
                  totalTasks: msg.totalTasks,
                  totalTokens: 0,
                  taskResults: [],
                  timestamp: msg.timestamp,
                },
              }))
            }

            // Track task metadata
            if (isTBTaskStart(msg)) {
              const state = yield* ctx.state.get
              if (state.activeRunId === msg.runId) {
                const newTask: Partial<TBTaskResult> = {
                  taskId: msg.taskId,
                  taskName: msg.taskName,
                  category: msg.category,
                  difficulty: msg.difficulty,
                }
                const updatedTasks = new Map(state.activeTasks)
                updatedTasks.set(msg.taskId, { ...newTask } as TBTaskResult)
                yield* ctx.state.update((s) => ({ ...s, activeTasks: updatedTasks }))
              }
            }

            // Track task completion
            if (isTBTaskComplete(msg)) {
              const state = yield* ctx.state.get
              if (state.activeRunId === msg.runId) {
                const existing = state.activeTasks.get(msg.taskId)
                const taskResult: TBTaskResult = {
                  taskId: msg.taskId,
                  taskName: existing?.taskName || msg.taskId,
                  category: existing?.category || "unknown",
                  difficulty: existing?.difficulty || "medium",
                  outcome: msg.outcome,
                  durationMs: msg.durationMs,
                  turns: msg.turns,
                  tokens: msg.tokens,
                }

                const updatedTasks = new Map(state.activeTasks)
                updatedTasks.set(msg.taskId, taskResult)

                yield* ctx.state.update((s) => ({ ...s, activeTasks: updatedTasks }))
              }
            }

            // Update final results on run complete
            if (isTBRunComplete(msg)) {
              const state = yield* ctx.state.get
              if (state.activeRunId === msg.runId && state.currentResult) {
                const taskResults = Array.from(state.activeTasks.values())
                const totalTokens = msg.totalTokens || taskResults.reduce((sum, t) => sum + t.tokens, 0)

                yield* ctx.state.update((s) => ({
                  ...s,
                  currentResult: {
                    ...s.currentResult!,
                    passRate: msg.passRate,
                    passed: msg.passed,
                    failed: msg.failed,
                    timeout: msg.timeout,
                    error: msg.error,
                    totalDurationMs: msg.totalDurationMs,
                    totalTokens,
                    taskResults,
                  },
                  activeRunId: null,
                }))
              }
            }
          })
        )
      ),
    ]
  },
}

export const initialTBResultsState: TBResultsState = TBResultsComponent.initialState()
