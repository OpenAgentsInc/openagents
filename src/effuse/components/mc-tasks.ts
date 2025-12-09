/**
 * MC Tasks Component
 *
 * Displays MechaCoder ready tasks with task assignment functionality.
 * Supports priority-based styling, collapse/expand, and assign actions.
 */

import { Effect } from "effect"
import type { Component } from "../component/types.js"
import { html, joinTemplates } from "../template/html.js"
import { SocketServiceTag } from "../services/socket.js"

// ============================================================================
// Types
// ============================================================================

/** Task state from .openagents/tasks.jsonl */
export interface MCTask {
  id: string
  title: string
  description: string
  status: string
  priority: number
  type: string
  labels: string[]
  createdAt: string
  updatedAt: string
}

/** Component state */
export interface MCTasksState {
  /** List of ready tasks */
  tasks: MCTask[]
  /** Loading state */
  loading: boolean
  /** Error message if any */
  error: string | null
  /** Collapsed state */
  collapsed: boolean
  /** Max tasks to display */
  maxDisplay: number
  /** Task currently being assigned */
  assigningId: string | null
}

/** Component events */
export type MCTasksEvent =
  | { type: "load" }
  | { type: "toggleCollapse" }
  | { type: "assign"; taskId: string }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get priority label (P0-P4)
 */
const getPriorityLabel = (priority: number): string => {
  return `P${priority}`
}

/**
 * Get priority classes for styling
 */
const getPriorityClasses = (priority: number): string => {
  switch (priority) {
    case 0:
      return "bg-red-900/40 text-red-300 border-red-700/50"
    case 1:
      return "bg-orange-900/40 text-orange-300 border-orange-700/50"
    case 2:
      return "bg-zinc-800/50 text-zinc-200 border-zinc-600/50"
    case 3:
      return "bg-zinc-800/30 text-zinc-300 border-zinc-700/40"
    case 4:
      return "bg-zinc-900/30 text-zinc-400 border-zinc-700/30"
    default:
      return "bg-zinc-800/40 text-zinc-300 border-zinc-600/40"
  }
}

/**
 * Get type badge classes
 */
const getTypeBadgeClass = (type: string): string => {
  switch (type) {
    case "bug":
      return "text-red-400"
    case "feature":
      return "text-emerald-400"
    case "task":
      return "text-blue-400"
    case "epic":
      return "text-violet-400"
    case "chore":
      return "text-zinc-400"
    default:
      return "text-zinc-400"
  }
}

// ============================================================================
// Component Definition
// ============================================================================

export const MCTasksComponent: Component<MCTasksState, MCTasksEvent, SocketServiceTag> = {
  id: "mc-tasks",

  initialState: () => ({
    tasks: [],
    loading: false,
    error: null,
    collapsed: false,
    maxDisplay: 20,
    assigningId: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Header with controls
      const header = html`
        <div
          class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer"
          data-action="toggleCollapse"
        >
          <h2 class="text-zinc-100 font-bold font-mono text-lg">
            Ready Tasks (${state.tasks.length})
          </h2>
          <div class="flex items-center gap-3">
            <button
              class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
              data-action="load"
            >
              ${state.loading ? "Loading..." : "Refresh"}
            </button>
            <span class="text-zinc-500">${state.collapsed ? "▼" : "▲"}</span>
          </div>
        </div>
      `

      // Loading state
      if (state.loading && state.tasks.length === 0) {
        return html`
          <div class="rounded-2xl border border-zinc-800/60 bg-zinc-950/80 shadow-2xl backdrop-blur-xl">
            ${header}
            <div class="flex items-center justify-center py-8">
              <div class="text-zinc-400 font-mono text-sm">Loading ready tasks...</div>
            </div>
          </div>
        `
      }

      // Error state
      if (state.error && state.tasks.length === 0) {
        return html`
          <div class="rounded-2xl border border-zinc-800/60 bg-zinc-950/80 shadow-2xl backdrop-blur-xl">
            ${header}
            <div class="flex items-center justify-center py-8">
              <div class="text-red-400 font-mono text-sm">${state.error}</div>
            </div>
          </div>
        `
      }

      // Empty state
      if (state.tasks.length === 0) {
        return html`
          <div class="rounded-2xl border border-zinc-800/60 bg-zinc-950/80 shadow-2xl backdrop-blur-xl">
            ${header}
            <div class="flex items-center justify-center py-8">
              <div class="text-zinc-500 font-mono text-sm">No ready tasks found</div>
            </div>
          </div>
        `
      }

      // Collapsed state
      if (state.collapsed) {
        return html`
          <div class="rounded-2xl border border-zinc-800/60 bg-zinc-950/80 shadow-2xl backdrop-blur-xl">
            ${header}
          </div>
        `
      }

      // Task rows
      const taskRows = state.tasks.slice(0, state.maxDisplay).map((task) => {
        const prioClasses = getPriorityClasses(task.priority)
        const prioLabel = getPriorityLabel(task.priority)
        const typeClass = getTypeBadgeClass(task.type)
        const labelsStr = task.labels.slice(0, 2).join(", ")
        const isAssigning = state.assigningId === task.id

        return html`
          <tr class="border-b border-zinc-800/40 last:border-0 hover:bg-zinc-900/30">
            <td class="py-2 px-3">
              <span class="inline-flex items-center px-1.5 py-0.5 text-[10px] font-bold rounded border ${prioClasses}">
                ${prioLabel}
              </span>
            </td>
            <td class="py-2 px-3">
              <code class="text-zinc-500 font-mono text-[10px]">${task.id}</code>
            </td>
            <td class="py-2 px-3">
              <span class="font-medium font-mono text-zinc-100 text-sm" title="${task.title}">
                ${task.title.length > 50 ? task.title.slice(0, 50) + "..." : task.title}
              </span>
            </td>
            <td class="py-2 px-3">
              <span class="${typeClass} font-mono text-xs">${task.type}</span>
            </td>
            <td class="py-2 px-3">
              <span class="text-zinc-400 font-mono text-xs">${labelsStr}</span>
            </td>
            <td class="py-2 px-3">
              <button
                class="inline-flex items-center justify-center border px-3 py-1 text-[10px] font-mono font-semibold uppercase rounded transition-colors
                       ${isAssigning
                  ? "border-zinc-600 text-zinc-500 bg-zinc-800/40 cursor-not-allowed"
                  : "border-zinc-700 text-zinc-50 bg-zinc-900/80 hover:bg-zinc-900/95"}"
                data-action="assign"
                data-task-id="${task.id}"
                ${isAssigning ? "disabled" : ""}
              >
                ${isAssigning ? "Starting..." : "Assign"}
              </button>
            </td>
          </tr>
        `
      })

      // More tasks indicator
      const moreIndicator =
        state.tasks.length > state.maxDisplay
          ? html`
              <div class="px-4 py-2 border-t border-zinc-800/60 text-center text-xs font-mono text-zinc-500">
                + ${state.tasks.length - state.maxDisplay} more tasks...
              </div>
            `
          : ""

      return html`
        <div class="rounded-2xl border border-zinc-800/60 bg-zinc-950/80 shadow-2xl backdrop-blur-xl max-h-[70vh] overflow-hidden">
          ${header}
          <div class="overflow-x-auto max-h-[calc(70vh-60px)] overflow-y-auto">
            <table class="min-w-full table-auto text-xs font-mono text-zinc-200">
              <thead>
                <tr class="text-zinc-500 uppercase text-[9px] tracking-[0.4em] bg-zinc-900/40">
                  <th class="w-12 px-3 py-2 text-left">Pri</th>
                  <th class="w-24 px-3 py-2 text-left">ID</th>
                  <th class="px-3 py-2 text-left">Title</th>
                  <th class="w-20 px-3 py-2 text-left">Type</th>
                  <th class="w-32 px-3 py-2 text-left">Labels</th>
                  <th class="w-24 px-3 py-2 text-left">Action</th>
                </tr>
              </thead>
              <tbody>
                ${joinTemplates(taskRows)}
              </tbody>
            </table>
          </div>
          ${moreIndicator}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        const taskId = el.dataset.taskId

        // Prevent collapse toggle when clicking load button
        if (action === "load") {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "load" }))
        } else if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        } else if (action === "assign" && taskId) {
          e.stopPropagation()
          Effect.runFork(ctx.emit({ type: "assign", taskId }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "load":
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null }))

          const result = yield* socket.loadReadyTasks(50).pipe(
            Effect.map((tasks) => ({ tasks: tasks as MCTask[], error: null })),
            Effect.catchAll((e) => Effect.succeed({ tasks: [] as MCTask[], error: e.message }))
          )

          yield* ctx.state.update((s) => ({
            ...s,
            loading: false,
            tasks: result.tasks,
            error: result.error,
          }))
          break

        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "assign":
          yield* ctx.state.update((s) => ({ ...s, assigningId: event.taskId }))

          const assignResult = yield* socket.assignTaskToMC(event.taskId, { sandbox: true }).pipe(
            Effect.map(() => ({ success: true, error: null })),
            Effect.catchAll((e) => Effect.succeed({ success: false, error: e.message }))
          )

          if (assignResult.success) {
            // Remove assigned task from list
            yield* ctx.state.update((s) => ({
              ...s,
              assigningId: null,
              tasks: s.tasks.filter((t) => t.id !== event.taskId),
            }))
          } else {
            // Reset assigning state on error
            yield* ctx.state.update((s) => ({
              ...s,
              assigningId: null,
              error: assignResult.error,
            }))
          }
          break
      }
    }),
}

// ============================================================================
// Export initial state for testing
// ============================================================================

export const initialMCTasksState: MCTasksState = MCTasksComponent.initialState()
