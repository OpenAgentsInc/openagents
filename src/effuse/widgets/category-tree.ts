/**
 * Category Tree Widget
 *
 * Displays TB tasks grouped by category in a collapsible tree view.
 * Supports expand/collapse all, task status icons, and category statistics.
 */

import { Effect, Stream, pipe } from "effect"
import { html, joinTemplates } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import { SocketServiceTag } from "../services/socket.js"
import type { HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/** Task status */
export type TBTaskStatus = "pending" | "running" | "passed" | "failed" | "timeout" | "error"

/** Task data */
export interface TBTaskData {
  id: string
  name: string
  difficulty: string
  category: string
  status: TBTaskStatus
}

/** Category data with aggregated stats */
export interface CategoryData {
  category: string
  tasks: TBTaskData[]
  passed: number
  failed: number
  pending: number
  total: number
}

/** Widget state */
export interface CategoryTreeState {
  /** All tasks indexed by ID */
  tasks: Map<string, TBTaskData>
  /** Collapsed categories */
  collapsedCategories: Set<string>
  /** Whether the tree is visible */
  visible: boolean
  /** Selected task ID */
  selectedTaskId: string | null
}

/** Widget events */
export type CategoryTreeEvent =
  | { type: "show" }
  | { type: "hide" }
  | { type: "toggleCategory"; category: string }
  | { type: "expandAll" }
  | { type: "collapseAll" }
  | { type: "selectTask"; taskId: string }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get status icon for a task
 */
const getStatusIcon = (status: TBTaskStatus): string => {
  switch (status) {
    case "passed":
      return "✓"
    case "failed":
      return "✗"
    case "error":
      return "⚠"
    case "timeout":
      return "⏱"
    case "running":
      return "▶"
    default:
      return "○"
  }
}

/**
 * Get status color class
 */
const getStatusColorClass = (status: TBTaskStatus): string => {
  switch (status) {
    case "passed":
      return "text-emerald-400"
    case "failed":
      return "text-red-400"
    case "error":
      return "text-orange-400"
    case "timeout":
      return "text-amber-400"
    case "running":
      return "text-blue-400"
    default:
      return "text-zinc-500"
  }
}

/**
 * Get difficulty badge class
 */
const getDifficultyClass = (difficulty: string): string => {
  switch (difficulty.toLowerCase()) {
    case "easy":
      return "bg-emerald-900/40 text-emerald-300"
    case "medium":
      return "bg-amber-900/40 text-amber-300"
    case "hard":
      return "bg-red-900/40 text-red-300"
    default:
      return "bg-zinc-800/40 text-zinc-300"
  }
}

/**
 * Group tasks by category
 */
const groupTasksByCategory = (tasks: Map<string, TBTaskData>): Map<string, CategoryData> => {
  const categories = new Map<string, CategoryData>()

  for (const task of tasks.values()) {
    const cat = task.category || "uncategorized"
    if (!categories.has(cat)) {
      categories.set(cat, { category: cat, tasks: [], passed: 0, failed: 0, pending: 0, total: 0 })
    }
    const catData = categories.get(cat)!
    catData.tasks.push(task)
    catData.total++
    if (task.status === "passed") catData.passed++
    if (task.status === "failed" || task.status === "error" || task.status === "timeout") {
      catData.failed++
    }
    if (task.status === "pending") catData.pending++
  }

  return categories
}

// ============================================================================
// Type Guards
// ============================================================================

const isTBTaskStart = (msg: HudMessage): msg is HudMessage & {
  type: "tb_task_start"
  taskId: string
  taskName: string
  difficulty: string
  category: string
} => msg.type === "tb_task_start"

const isTBTaskComplete = (msg: HudMessage): msg is HudMessage & {
  type: "tb_task_complete"
  taskId: string
  outcome: string
} => msg.type === "tb_task_complete"

const isTBSuiteInfo = (msg: HudMessage): msg is HudMessage & {
  type: "tb_suite_info"
  suiteName: string
  suiteVersion: string
  tasks: Array<{ id: string; name: string; difficulty: string; category: string }>
} => msg.type === "tb_suite_info"

const isTBMessage = (msg: HudMessage): boolean =>
  isTBTaskStart(msg) || isTBTaskComplete(msg) || isTBSuiteInfo(msg)

// ============================================================================
// Widget Definition
// ============================================================================

export const CategoryTreeWidget: Widget<CategoryTreeState, CategoryTreeEvent, SocketServiceTag> = {
  id: "category-tree",

  initialState: () => ({
    tasks: new Map(),
    collapsedCategories: new Set(),
    visible: false,
    selectedTaskId: null,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Hidden state
      if (!state.visible) {
        return html`<div class="hidden"></div>`
      }

      const categories = groupTasksByCategory(state.tasks)

      // Header with controls
      const header = html`
        <div class="flex items-center justify-between px-3 py-2 border-b border-zinc-800/60 bg-zinc-900/80">
          <span class="text-sm font-medium text-zinc-300">Categories</span>
          <div class="flex items-center gap-2">
            <button
              class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
              data-action="expandAll"
            >
              Expand
            </button>
            <button
              class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
              data-action="collapseAll"
            >
              Collapse
            </button>
            <button
              class="text-zinc-500 hover:text-zinc-300 transition-colors"
              data-action="hide"
            >
              ×
            </button>
          </div>
        </div>
      `

      // Empty state
      if (categories.size === 0) {
        return html`
          <div class="fixed right-4 top-20 w-72 rounded-lg border border-zinc-800/60 bg-zinc-950/95 shadow-xl backdrop-blur-sm overflow-hidden">
            ${header}
            <div class="p-4 text-center">
              <span class="text-sm text-zinc-500">No tasks loaded</span>
            </div>
          </div>
        `
      }

      // Render categories
      const categoryList = Array.from(categories.entries())
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([catName, catData]) => {
          const isCollapsed = state.collapsedCategories.has(catName)

          // Category header with stats
          const statsHtml =
            catData.passed > 0 || catData.failed > 0
              ? html`
                  <span class="text-xs text-emerald-400">✓${catData.passed}</span>
                  <span class="text-xs text-red-400">✗${catData.failed}</span>
                `
              : ""

          // Task items
          const taskItems = catData.tasks.map((task) => {
            const icon = getStatusIcon(task.status)
            const statusColor = getStatusColorClass(task.status)
            const diffClass = getDifficultyClass(task.difficulty)
            const isSelected = state.selectedTaskId === task.id
            const isRunning = task.status === "running"

            return html`
              <div
                class="flex items-center gap-2 px-3 py-1.5 hover:bg-zinc-900/40 cursor-pointer transition-colors
                       ${isSelected ? "bg-zinc-800/60" : ""} ${isRunning ? "animate-pulse" : ""}"
                data-action="selectTask"
                data-task-id="${task.id}"
              >
                <span class="${statusColor}">${icon}</span>
                <span class="flex-1 text-xs text-zinc-300 truncate" title="${task.name}">
                  ${task.name}
                </span>
                ${task.difficulty
                  ? html`<span class="text-[9px] px-1 py-0.5 rounded ${diffClass}">${task.difficulty.slice(0, 1).toUpperCase()}</span>`
                  : ""}
              </div>
            `
          })

          return html`
            <div class="border-b border-zinc-800/40 last:border-0">
              <div
                class="flex items-center gap-2 px-3 py-2 hover:bg-zinc-900/40 cursor-pointer transition-colors"
                data-action="toggleCategory"
                data-category="${catName}"
              >
                <span class="text-zinc-500 text-xs">${isCollapsed ? "▶" : "▼"}</span>
                <span class="flex-1 text-sm font-medium text-zinc-200">${catName}</span>
                <div class="flex items-center gap-2">
                  ${statsHtml}
                  <span class="text-xs text-zinc-500">${catData.total}</span>
                </div>
              </div>
              ${isCollapsed ? "" : html`<div class="pb-1">${joinTemplates(taskItems)}</div>`}
            </div>
          `
        })

      return html`
        <div class="fixed right-4 top-20 w-72 max-h-[70vh] flex flex-col rounded-lg border border-zinc-800/60 bg-zinc-950/95 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header}
          <div class="flex-1 overflow-y-auto">
            ${joinTemplates(categoryList)}
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        const category = el.dataset.category
        const taskId = el.dataset.taskId

        if (action === "hide") {
          Effect.runFork(ctx.emit({ type: "hide" }))
        } else if (action === "expandAll") {
          Effect.runFork(ctx.emit({ type: "expandAll" }))
        } else if (action === "collapseAll") {
          Effect.runFork(ctx.emit({ type: "collapseAll" }))
        } else if (action === "toggleCategory" && category) {
          Effect.runFork(ctx.emit({ type: "toggleCategory", category }))
        } else if (action === "selectTask" && taskId) {
          Effect.runFork(ctx.emit({ type: "selectTask", taskId }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "show":
          yield* ctx.state.update((s) => ({ ...s, visible: true }))
          break

        case "hide":
          yield* ctx.state.update((s) => ({ ...s, visible: false }))
          break

        case "toggleCategory":
          yield* ctx.state.update((s) => {
            const newCollapsed = new Set(s.collapsedCategories)
            if (newCollapsed.has(event.category)) {
              newCollapsed.delete(event.category)
            } else {
              newCollapsed.add(event.category)
            }
            return { ...s, collapsedCategories: newCollapsed }
          })
          break

        case "expandAll":
          yield* ctx.state.update((s) => ({ ...s, collapsedCategories: new Set() }))
          break

        case "collapseAll": {
          const state = yield* ctx.state.get
          const categories = groupTasksByCategory(state.tasks)
          const allCategories = new Set(categories.keys())
          yield* ctx.state.update((s) => ({ ...s, collapsedCategories: allCategories }))
          break
        }

        case "selectTask":
          yield* ctx.state.update((s) => ({ ...s, selectedTaskId: event.taskId }))
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
            if (isTBSuiteInfo(msg)) {
              const newTasks = new Map<string, TBTaskData>()
              for (const task of msg.tasks) {
                newTasks.set(task.id, {
                  id: task.id,
                  name: task.name,
                  difficulty: task.difficulty,
                  category: task.category,
                  status: "pending",
                })
              }
              yield* ctx.state.update((s) => ({
                ...s,
                tasks: newTasks,
                visible: true,
              }))
            }

            if (isTBTaskStart(msg)) {
              yield* ctx.state.update((s) => {
                const task = s.tasks.get(msg.taskId)
                if (!task) return s
                const newTasks = new Map(s.tasks)
                newTasks.set(msg.taskId, { ...task, status: "running" })
                return { ...s, tasks: newTasks }
              })
            }

            if (isTBTaskComplete(msg)) {
              yield* ctx.state.update((s) => {
                const task = s.tasks.get(msg.taskId)
                if (!task) return s
                const newTasks = new Map(s.tasks)
                const status = msg.outcome as TBTaskStatus
                newTasks.set(msg.taskId, { ...task, status })
                return { ...s, tasks: newTasks }
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

export const initialCategoryTreeState: CategoryTreeState = CategoryTreeWidget.initialState()
