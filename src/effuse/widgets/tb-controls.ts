/**
 * TB Controls Widget
 *
 * Handles TB suite loading, task selection, and run control.
 * Provides UI for starting/stopping runs, selecting tasks, and displaying status.
 */

import { Effect, Stream, pipe } from "effect"
import { html, joinTemplates } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import { SocketServiceTag } from "../services/socket.js"
import type { HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/** Task info from suite */
export interface TBTaskInfo {
  id: string
  name: string
  difficulty: string
  category: string
}

/** Suite info */
export interface TBSuiteInfo {
  name: string
  version: string
  tasks: TBTaskInfo[]
}

/** Widget state */
export interface TBControlsState {
  /** Current suite path */
  suitePath: string
  /** Loaded suite info */
  suite: TBSuiteInfo | null
  /** Selected task IDs */
  selectedTaskIds: Set<string>
  /** Loading state */
  loading: boolean
  /** Status message */
  status: string
  /** Status type for styling */
  statusType: "idle" | "loading" | "running" | "error" | "success"
  /** Whether a run is active */
  isRunning: boolean
  /** Current run ID */
  runId: string | null
  /** Collapsed state */
  collapsed: boolean
}

/** Widget events */
export type TBControlsEvent =
  | { type: "setSuitePath"; path: string }
  | { type: "loadSuite" }
  | { type: "startRun" }
  | { type: "startRandomTask" }
  | { type: "stopRun" }
  | { type: "selectAll" }
  | { type: "selectNone" }
  | { type: "toggleTask"; taskId: string }
  | { type: "toggleCollapse" }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get difficulty badge class
 */
const getDifficultyClass = (difficulty: string): string => {
  switch (difficulty.toLowerCase()) {
    case "easy":
      return "bg-emerald-900/40 text-emerald-300 border-emerald-700/50"
    case "medium":
      return "bg-amber-900/40 text-amber-300 border-amber-700/50"
    case "hard":
      return "bg-red-900/40 text-red-300 border-red-700/50"
    default:
      return "bg-zinc-800/40 text-zinc-300 border-zinc-700/50"
  }
}

/**
 * Get status color class
 */
const getStatusColorClass = (statusType: TBControlsState["statusType"]): string => {
  switch (statusType) {
    case "running":
      return "text-blue-400"
    case "error":
      return "text-red-400"
    case "success":
      return "text-emerald-400"
    case "loading":
      return "text-zinc-400"
    default:
      return "text-zinc-500"
  }
}

// ============================================================================
// Type Guards
// ============================================================================

const isTBRunStart = (msg: HudMessage): msg is HudMessage & {
  type: "tb_run_start"
  runId: string
} => msg.type === "tb_run_start"

const isTBRunComplete = (msg: HudMessage): msg is HudMessage & {
  type: "tb_run_complete"
  runId: string
} => msg.type === "tb_run_complete"

const isTBMessage = (msg: HudMessage): boolean =>
  isTBRunStart(msg) || isTBRunComplete(msg)

// ============================================================================
// Widget Definition
// ============================================================================

export const TBControlsWidget: Widget<TBControlsState, TBControlsEvent, SocketServiceTag> = {
  id: "tb-controls",

  initialState: () => ({
    suitePath: "./tasks/terminal-bench-2.json",
    suite: null,
    selectedTaskIds: new Set(),
    loading: false,
    status: "Ready",
    statusType: "idle",
    isRunning: false,
    runId: null,
    collapsed: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      const statusColor = getStatusColorClass(state.statusType)

      // Header with status
      const header = html`
        <div
          class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer"
          data-action="toggleCollapse"
        >
          <div class="flex items-center gap-3">
            <h2 class="text-zinc-100 font-bold font-mono text-lg">Terminal-Bench</h2>
            <span class="text-xs ${statusColor}">${state.status}</span>
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

      // Suite path input and load button
      const pathInput = html`
        <div class="flex items-center gap-2 px-4 py-3 border-b border-zinc-800/40">
          <input
            type="text"
            class="flex-1 bg-zinc-900/60 border border-zinc-700/50 rounded px-3 py-2 text-sm font-mono text-zinc-200 placeholder-zinc-500 focus:border-zinc-600/60 focus:outline-none"
            placeholder="Path to suite..."
            value="${state.suitePath}"
            data-input="suitePath"
            ${state.isRunning ? "disabled" : ""}
          />
          <button
            class="px-3 py-2 text-xs font-mono uppercase rounded border transition-colors
                   ${state.isRunning
              ? "border-zinc-700 text-zinc-500 bg-zinc-800/40 cursor-not-allowed"
              : "border-zinc-700 text-zinc-200 bg-zinc-900/80 hover:bg-zinc-900/95"}"
            data-action="loadSuite"
            ${state.isRunning ? "disabled" : ""}
          >
            ${state.loading ? "Loading..." : "Load"}
          </button>
        </div>
      `

      // Control buttons
      const controls = html`
        <div class="flex items-center gap-2 px-4 py-3 border-b border-zinc-800/40">
          <button
            class="px-4 py-2 text-xs font-mono uppercase rounded border transition-colors
                   ${state.isRunning || !state.suite
              ? "border-zinc-700 text-zinc-500 bg-zinc-800/40 cursor-not-allowed"
              : "border-emerald-700 text-emerald-300 bg-emerald-900/40 hover:bg-emerald-900/60"}"
            data-action="startRun"
            ${state.isRunning || !state.suite ? "disabled" : ""}
          >
            Start
          </button>
          <button
            class="px-4 py-2 text-xs font-mono uppercase rounded border transition-colors
                   ${state.isRunning || !state.suite
              ? "border-zinc-700 text-zinc-500 bg-zinc-800/40 cursor-not-allowed"
              : "border-violet-700 text-violet-300 bg-violet-900/40 hover:bg-violet-900/60"}"
            data-action="startRandomTask"
            ${state.isRunning || !state.suite ? "disabled" : ""}
          >
            Random
          </button>
          <button
            class="px-4 py-2 text-xs font-mono uppercase rounded border transition-colors
                   ${!state.isRunning
              ? "border-zinc-700 text-zinc-500 bg-zinc-800/40 cursor-not-allowed"
              : "border-red-700 text-red-300 bg-red-900/40 hover:bg-red-900/60"}"
            data-action="stopRun"
            ${!state.isRunning ? "disabled" : ""}
          >
            Stop
          </button>
          <span class="flex-1"></span>
          <span class="text-xs text-zinc-500 font-mono">
            ${state.selectedTaskIds.size}/${state.suite?.tasks.length ?? 0} selected
          </span>
        </div>
      `

      // Task list (if suite loaded)
      const taskList = state.suite
        ? html`
            <div class="border-b border-zinc-800/40">
              <div class="flex items-center justify-between px-4 py-2 bg-zinc-900/40">
                <span class="text-xs text-zinc-400 font-mono">
                  ${state.suite.name} v${state.suite.version}
                </span>
                <div class="flex items-center gap-2">
                  <button
                    class="text-xs text-zinc-400 hover:text-zinc-200 transition-colors"
                    data-action="selectAll"
                  >
                    All
                  </button>
                  <span class="text-zinc-600">|</span>
                  <button
                    class="text-xs text-zinc-400 hover:text-zinc-200 transition-colors"
                    data-action="selectNone"
                  >
                    None
                  </button>
                </div>
              </div>
              <div class="max-h-60 overflow-y-auto">
                ${joinTemplates(
                  state.suite.tasks.map((task) => {
                    const isSelected = state.selectedTaskIds.has(task.id)
                    const diffClass = getDifficultyClass(task.difficulty)

                    return html`
                      <label
                        class="flex items-center gap-3 px-4 py-2 hover:bg-zinc-900/40 cursor-pointer transition-colors"
                      >
                        <input
                          type="checkbox"
                          class="form-checkbox rounded border-zinc-600 bg-zinc-800 text-blue-500 focus:ring-blue-500/20"
                          data-action="toggleTask"
                          data-task-id="${task.id}"
                          ${isSelected ? "checked" : ""}
                        />
                        <span class="flex-1 text-sm font-mono text-zinc-200 truncate" title="${task.name}">
                          ${task.name}
                        </span>
                        <span class="text-xs px-1.5 py-0.5 rounded border ${diffClass}">
                          ${task.difficulty}
                        </span>
                      </label>
                    `
                  })
                )}
              </div>
            </div>
          `
        : ""

      return html`
        <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header}
          ${pathInput}
          ${controls}
          ${taskList}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Handle button clicks
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        const taskId = el.dataset.taskId

        if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        } else if (action === "loadSuite") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "loadSuite" }))
        } else if (action === "startRun") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "startRun" }))
        } else if (action === "startRandomTask") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "startRandomTask" }))
        } else if (action === "stopRun") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "stopRun" }))
        } else if (action === "selectAll") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "selectAll" }))
        } else if (action === "selectNone") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "selectNone" }))
        } else if (action === "toggleTask" && taskId) {
          Effect.runFork(ctx.emit({ type: "toggleTask", taskId }))
        }
      })

      // Handle input changes
      yield* ctx.dom.delegate(ctx.container, "[data-input]", "input", (_e, target) => {
        const el = target as HTMLInputElement
        const inputType = el.dataset.input

        if (inputType === "suitePath") {
          Effect.runFork(ctx.emit({ type: "setSuitePath", path: el.value }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "setSuitePath":
          yield* ctx.state.update((s) => ({ ...s, suitePath: event.path }))
          break

        case "loadSuite": {
          const state = yield* ctx.state.get
          if (!state.suitePath.trim()) {
            yield* ctx.state.update((s) => ({ ...s, status: "No path", statusType: "error" as const }))
            return
          }

          yield* ctx.state.update((s) => ({ ...s, loading: true, status: "Loading...", statusType: "loading" as const }))

          const result = yield* socket.loadTBSuite(state.suitePath).pipe(
            Effect.map((suite) => ({ suite: suite as TBSuiteInfo, error: null })),
            Effect.catchAll((e) => Effect.succeed({ suite: null as TBSuiteInfo | null, error: e.message }))
          )

          if (result.error) {
            yield* ctx.state.update((s) => ({
              ...s,
              loading: false,
              suite: null,
              selectedTaskIds: new Set(),
              status: "Load failed",
              statusType: "error" as const,
            }))
          } else if (result.suite) {
            // Select all tasks by default
            const allIds = new Set(result.suite.tasks.map((t) => t.id))
            yield* ctx.state.update((s) => ({
              ...s,
              loading: false,
              suite: result.suite,
              selectedTaskIds: allIds,
              status: "Ready",
              statusType: "idle" as const,
            }))
          }
          break
        }

        case "startRun": {
          const state = yield* ctx.state.get
          if (!state.suite || state.selectedTaskIds.size === 0) return

          yield* ctx.state.update((s) => ({
            ...s,
            isRunning: true,
            status: "Starting...",
            statusType: "running" as const,
          }))

          const result = yield* socket.startTBRun({
            suitePath: state.suitePath,
            taskIds: Array.from(state.selectedTaskIds),
            timeout: 300000,
            maxTurns: 10,
          }).pipe(
            Effect.map((res) => ({ runId: res.runId, error: null })),
            Effect.catchAll((e) => Effect.succeed({ runId: null as string | null, error: e.message }))
          )

          if (result.error) {
            yield* ctx.state.update((s) => ({
              ...s,
              isRunning: false,
              status: "Start failed",
              statusType: "error" as const,
            }))
          } else {
            yield* ctx.state.update((s) => ({
              ...s,
              runId: result.runId,
              status: "Running...",
              statusType: "running" as const,
            }))
          }
          break
        }

        case "startRandomTask": {
          const state = yield* ctx.state.get
          if (!state.suite || state.suite.tasks.length === 0) return

          const randomIndex = Math.floor(Math.random() * state.suite.tasks.length)
          const randomTask = state.suite.tasks[randomIndex]

          yield* ctx.state.update((s) => ({
            ...s,
            isRunning: true,
            status: `Random: ${randomTask.name}`,
            statusType: "running" as const,
          }))

          const result = yield* socket.startTBRun({
            suitePath: state.suitePath,
            taskIds: [randomTask.id],
            timeout: 300000,
            maxTurns: 10,
          }).pipe(
            Effect.map((res) => ({ runId: res.runId, error: null })),
            Effect.catchAll((e) => Effect.succeed({ runId: null as string | null, error: e.message }))
          )

          if (result.error) {
            yield* ctx.state.update((s) => ({
              ...s,
              isRunning: false,
              status: "Start failed",
              statusType: "error" as const,
            }))
          } else {
            yield* ctx.state.update((s) => ({
              ...s,
              runId: result.runId,
              status: "Running...",
              statusType: "running" as const,
            }))
          }
          break
        }

        case "stopRun": {
          yield* ctx.state.update((s) => ({ ...s, status: "Stopping...", statusType: "loading" as const }))

          const result = yield* socket.stopTBRun().pipe(
            Effect.map((res) => ({ stopped: res.stopped, error: null })),
            Effect.catchAll((e) => Effect.succeed({ stopped: false, error: e.message }))
          )

          yield* ctx.state.update((s) => ({
            ...s,
            isRunning: false,
            runId: null,
            status: result.stopped ? "Stopped" : "No active run",
            statusType: result.error ? ("error" as const) : ("idle" as const),
          }))
          break
        }

        case "selectAll": {
          const state = yield* ctx.state.get
          if (!state.suite) return
          const allIds = new Set(state.suite.tasks.map((t) => t.id))
          yield* ctx.state.update((s) => ({ ...s, selectedTaskIds: allIds }))
          break
        }

        case "selectNone":
          yield* ctx.state.update((s) => ({ ...s, selectedTaskIds: new Set() }))
          break

        case "toggleTask": {
          yield* ctx.state.update((s) => {
            const newSelected = new Set(s.selectedTaskIds)
            if (newSelected.has(event.taskId)) {
              newSelected.delete(event.taskId)
            } else {
              newSelected.add(event.taskId)
            }
            return { ...s, selectedTaskIds: newSelected }
          })
          break
        }

        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break
      }
    }),

  subscriptions: (ctx) => {
    const socket = Effect.map(SocketServiceTag, (s) => s)

    return [
      pipe(
        Stream.unwrap(Effect.map(socket, (s) => s.getMessages())),
        Stream.filter((msg): msg is HudMessage => isTBMessage(msg)),
        Stream.map((msg) =>
          Effect.gen(function* () {
            if (isTBRunStart(msg)) {
              yield* ctx.state.update((s) => ({
                ...s,
                isRunning: true,
                runId: msg.runId,
                status: "Running...",
                statusType: "running" as const,
              }))
            }

            if (isTBRunComplete(msg)) {
              yield* ctx.state.update((s) => {
                if (s.runId !== msg.runId) return s
                return {
                  ...s,
                  isRunning: false,
                  runId: null,
                  status: "Complete",
                  statusType: "success" as const,
                }
              })
            }
          })
        )
      ),
    ]
  },
}

// ============================================================================
// Export initial state for testing
// ============================================================================

export const initialTBControlsState: TBControlsState = TBControlsWidget.initialState()
