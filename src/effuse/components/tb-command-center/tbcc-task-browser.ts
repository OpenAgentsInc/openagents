/**
 * TB Command Center Task Browser Component
 *
 * Browses tasks from a TB suite file.
 */

import { Effect, Stream } from "effect"
import type { Component } from "../../component/types.js"
import { html, joinTemplates } from "../../template/html.js"
import type { TBTask, TBDifficulty } from "./types.js"
import { DIFFICULTY_COLORS } from "./types.js"
import { SocketServiceTag } from "../../services/socket.js"

// ============================================================================
// Types
// ============================================================================

export interface TBCCTaskBrowserState {
  tasks: TBTask[]
  selectedTaskId: string | null
  searchQuery: string
  difficultyFilter: TBDifficulty | "all"
  loading: boolean
  error: string | null
}

export type TBCCTaskBrowserEvent =
  | { type: "loadTasks" }
  | { type: "selectTask"; taskId: string }
  | { type: "updateSearch"; query: string }
  | { type: "updateFilter"; difficulty: TBDifficulty | "all" }
  | { type: "runTask"; taskId: string }

// ============================================================================
// Component Definition
// ============================================================================

export const TBCCTaskBrowserComponent: Component<TBCCTaskBrowserState, TBCCTaskBrowserEvent, SocketServiceTag> = {
  id: "tbcc-task-browser",

  initialState: () => ({
    tasks: [],
    selectedTaskId: null,
    searchQuery: "",
    difficultyFilter: "all",
    loading: true,
    error: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Filter tasks
      const filteredTasks = state.tasks.filter((task) => {
        const matchesSearch =
          state.searchQuery === "" ||
          task.name.toLowerCase().includes(state.searchQuery.toLowerCase()) ||
          task.category.toLowerCase().includes(state.searchQuery.toLowerCase())
        const matchesDifficulty =
          state.difficultyFilter === "all" || task.difficulty === state.difficultyFilter
        return matchesSearch && matchesDifficulty
      })

      // Selected task detail
      const selectedTask = state.selectedTaskId
        ? state.tasks.find((t) => t.id === state.selectedTaskId)
        : null

      // Task List
      const taskList = html`
        <div class="flex flex-col h-full border-r border-zinc-800/60 bg-zinc-900/20 w-1/3 min-w-[300px]">
          <!-- Filters -->
          <div class="p-4 border-b border-zinc-800/60 space-y-3">
            <input
              type="text"
              placeholder="Search tasks..."
              class="w-full bg-zinc-900 border border-zinc-700 rounded px-3 py-2 text-sm text-zinc-200 focus:border-emerald-500 focus:outline-none"
              value="${state.searchQuery}"
              data-action="search"
            />
            <div class="flex gap-2 overflow-x-auto pb-1 no-scrollbar">
              ${joinTemplates(
        (["all", "easy", "medium", "hard", "expert"] as const).map((diff) => {
          const isActive = state.difficultyFilter === diff
          const classes = isActive
            ? "bg-zinc-700 text-zinc-100 border-zinc-600"
            : "bg-zinc-900 text-zinc-400 border-zinc-800 hover:border-zinc-700"
          return html`
                    <button
                      class="px-2.5 py-1 text-xs rounded border ${classes} whitespace-nowrap capitalize transition-colors"
                      data-action="filter"
                      data-difficulty="${diff}"
                    >
                      ${diff}
                    </button>
                  `
        })
      )}
            </div>
          </div>

          <!-- List -->
          <div class="flex-1 overflow-y-auto">
            ${state.loading
          ? html`<div class="p-8 text-center text-zinc-500 text-sm">Loading tasks...</div>`
          : filteredTasks.length === 0
            ? html`<div class="p-8 text-center text-zinc-500 text-sm">No tasks found</div>`
            : html`
                    <div class="divide-y divide-zinc-800/40">
                      ${joinTemplates(
              filteredTasks.map((task) => {
                const isSelected = task.id === state.selectedTaskId
                const diffColor = DIFFICULTY_COLORS[task.difficulty]
                const bgClass = isSelected ? "bg-zinc-800/60" : "hover:bg-zinc-800/30"

                return html`
                            <div
                              class="p-4 cursor-pointer transition-colors ${bgClass}"
                              data-action="selectTask"
                              data-task-id="${task.id}"
                            >
                              <div class="flex items-start justify-between gap-2 mb-1">
                                <div class="font-mono text-sm text-zinc-200 truncate" title="${task.name}">
                                  ${task.name}
                                </div>
                                <span
                                  class="px-1.5 py-0.5 text-[10px] uppercase rounded border ${diffColor.bg} ${diffColor.text} ${diffColor.border}"
                                >
                                  ${task.difficulty}
                                </span>
                              </div>
                              <div class="text-xs text-zinc-500 truncate">${task.category}</div>
                            </div>
                          `
              })
            )}
                    </div>
                  `}
          </div>
        </div>
      `

      // Task Detail
      const taskDetail = selectedTask
        ? html`
            <div class="flex-1 h-full overflow-y-auto p-6">
              <div class="max-w-3xl mx-auto">
                <div class="flex items-start justify-between mb-6">
                  <div>
                    <h2 class="text-xl font-bold font-mono text-zinc-100 mb-2">${selectedTask.name}</h2>
                    <div class="flex items-center gap-3 text-sm">
                      <span class="text-zinc-400">${selectedTask.category}</span>
                      <span class="text-zinc-600">•</span>
                      <span class="${DIFFICULTY_COLORS[selectedTask.difficulty].text} capitalize">
                        ${selectedTask.difficulty}
                      </span>
                    </div>
                  </div>
                  <button
                    class="px-4 py-2 bg-emerald-600 hover:bg-emerald-500 text-white text-sm font-medium rounded transition-colors flex items-center gap-2"
                    data-action="runTask"
                    data-task-id="${selectedTask.id}"
                  >
                    <span>▶ Run Task</span>
                  </button>
                </div>

                <div class="bg-zinc-900/40 border border-zinc-800/60 rounded-lg p-6 mb-6">
                  <h3 class="text-sm font-bold text-zinc-300 mb-3">Description</h3>
                  <div class="prose prose-invert prose-sm max-w-none text-zinc-400">
                    ${selectedTask.description}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 mb-6">
                  <div class="bg-zinc-900/40 border border-zinc-800/60 rounded-lg p-4">
                    <div class="text-xs text-zinc-500 mb-1">Timeout</div>
                    <div class="text-lg font-mono text-zinc-200">${selectedTask.timeoutSeconds}s</div>
                  </div>
                  <div class="bg-zinc-900/40 border border-zinc-800/60 rounded-lg p-4">
                    <div class="text-xs text-zinc-500 mb-1">Max Turns</div>
                    <div class="text-lg font-mono text-zinc-200">${selectedTask.maxTurns}</div>
                  </div>
                </div>

                ${selectedTask.tags.length > 0
            ? html`
                      <div class="mb-6">
                        <h3 class="text-xs font-bold text-zinc-500 uppercase mb-2">Tags</h3>
                        <div class="flex flex-wrap gap-2">
                          ${joinTemplates(
              selectedTask.tags.map(
                (tag) => html`
                                <span class="px-2 py-1 bg-zinc-800 text-zinc-300 text-xs rounded border border-zinc-700">
                                  ${tag}
                                </span>
                              `
              )
            )}
                        </div>
                      </div>
                    `
            : ""}
              </div>
            </div>
          `
        : html`
            <div class="flex-1 h-full flex items-center justify-center text-zinc-500 bg-zinc-950/50">
              <div class="text-center">
                <div class="text-4xl mb-4 opacity-20">←</div>
                <div>Select a task to view details</div>
              </div>
            </div>
          `

      return html`
        <div class="flex h-full overflow-hidden">
          ${taskList} ${taskDetail}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Search input
      yield* ctx.dom.delegate(ctx.container, "[data-action='search']", "input", (_e, target) => {
        const query = (target as HTMLInputElement).value
        Effect.runFork(ctx.emit({ type: "updateSearch", query }))
      })

      // Filter buttons
      yield* ctx.dom.delegate(ctx.container, "[data-action='filter']", "click", (_e, target) => {
        const difficulty = (target as HTMLElement).dataset.difficulty as TBDifficulty | "all"
        if (difficulty) {
          Effect.runFork(ctx.emit({ type: "updateFilter", difficulty }))
        }
      })

      // Select task
      yield* ctx.dom.delegate(ctx.container, "[data-action='selectTask']", "click", (_e, target) => {
        const taskId = (target as HTMLElement).dataset.taskId
        if (taskId) {
          Effect.runFork(ctx.emit({ type: "selectTask", taskId }))
        }
      })

      // Run task
      yield* ctx.dom.delegate(ctx.container, "[data-action='runTask']", "click", (_e, target) => {
        const taskId = (target as HTMLElement).dataset.taskId
        if (taskId) {
          Effect.runFork(ctx.emit({ type: "runTask", taskId }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "loadTasks": {
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null }))

          try {
            // Load tasks from suite file
            // Note: In a real app we might want to cache this or load from a specific location
            const suite = yield* socket.loadTBSuite("tasks/terminal-bench-2.json")

            // Map to TBTask type
            const tasks: TBTask[] = suite.tasks.map((t: any) => ({
              id: t.id,
              name: t.name,
              slug: t.name.toLowerCase().replace(/\s+/g, "-"),
              description: t.description,
              difficulty: (t.difficulty as TBDifficulty) || "unknown",
              category: t.category || "General",
              tags: t.tags || [],
              timeoutSeconds: t.timeout || 300,
              maxTurns: t.max_turns || 50,
              status: "unattempted",
              lastRunId: null,
              attemptCount: 0,
              passCount: 0,
            }))

            yield* ctx.state.update((s) => ({
              ...s,
              tasks,
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

        case "selectTask": {
          yield* ctx.state.update((s) => ({ ...s, selectedTaskId: event.taskId }))
          break
        }

        case "updateSearch": {
          yield* ctx.state.update((s) => ({ ...s, searchQuery: event.query }))
          break
        }

        case "updateFilter": {
          yield* ctx.state.update((s) => ({ ...s, difficultyFilter: event.difficulty }))
          break
        }

        case "runTask": {
          // Start run via socket
          yield* socket.startTBRun({
            suitePath: "tasks/terminal-bench-2.json",
            taskIds: [event.taskId],
          }).pipe(
            Effect.catchAll((error) =>
              ctx.state.update((s) => ({
                ...s,
                error: `Failed to start run: ${error instanceof Error ? error.message : String(error)}`,
              }))
            )
          )
          // Note: The shell will handle the runStarted event via subscription
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
    return [Stream.make(ctx.emit({ type: "loadTasks" }))]
  },
}
