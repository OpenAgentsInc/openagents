/**
 * HuggingFace Trajectory List Widget
 *
 * Displays a paginated, searchable list of OpenThoughts SFT trajectories.
 * User can browse through 15,209 trajectories, search by agent/task/episode,
 * and select a trajectory to view in the detail widget.
 */

import { Effect, Stream } from "effect"
import { html, joinTemplates } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import { SocketServiceTag } from "../services/socket.js"
import type { Trajectory } from "../../atif/schema.js"

// ============================================================================
// Types
// ============================================================================

/**
 * Trajectory metadata for list display
 */
export interface TrajectoryMetadata {
  sessionId: string
  agentName: string
  modelName: string
  task: string
  episode: string
  date: string
  stepCount: number
  index: number  // For fetching full trajectory
}

/**
 * HF Trajectory List State
 */
export interface HFTrajectoryListState {
  /** All trajectories for current page */
  trajectories: TrajectoryMetadata[]
  /** Filtered trajectories based on search */
  filteredTrajectories: TrajectoryMetadata[]
  /** Selected trajectory session ID */
  selectedSessionId: string | null
  /** Search query */
  searchQuery: string
  /** Current page (0-indexed) */
  currentPage: number
  /** Items per page */
  pageSize: number
  /** Total trajectory count */
  totalCount: number
  /** Loading state */
  loading: boolean
  /** Error message */
  error: string | null
  /** Collapsed state */
  collapsed: boolean
}

/**
 * HF Trajectory List Events
 */
export type HFTrajectoryListEvent =
  | { type: "loadPage"; page: number }
  | { type: "search"; query: string }
  | { type: "select"; sessionId: string; index: number }
  | { type: "toggleCollapse" }
  | { type: "refresh" }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Extract metadata from full trajectory
 */
const extractMetadata = (trajectory: Trajectory, index: number): TrajectoryMetadata => {
  const agent = trajectory.agent
  const extra = trajectory.extra as Record<string, unknown> | undefined

  return {
    sessionId: trajectory.session_id,
    agentName: agent?.name ?? "unknown",
    modelName: agent?.model_name ?? "unknown",
    task: (extra?.task as string) ?? "unknown",
    episode: (extra?.episode as string) ?? "unknown",
    date: (extra?.date as string) ?? trajectory.steps[0]?.timestamp ?? new Date().toISOString(),
    stepCount: trajectory.steps.length,
    index,
  }
}

/**
 * Filter trajectories by search query
 */
const filterTrajectories = (
  trajectories: TrajectoryMetadata[],
  query: string
): TrajectoryMetadata[] => {
  if (!query.trim()) return trajectories

  const lowercaseQuery = query.toLowerCase()
  return trajectories.filter(
    (t) =>
      t.agentName.toLowerCase().includes(lowercaseQuery) ||
      t.task.toLowerCase().includes(lowercaseQuery) ||
      t.episode.toLowerCase().includes(lowercaseQuery) ||
      t.sessionId.toLowerCase().includes(lowercaseQuery)
  )
}

/**
 * Format date for display
 */
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

// ============================================================================
// Widget Definition
// ============================================================================

export const HFTrajectoryListWidget: Widget<
  HFTrajectoryListState,
  HFTrajectoryListEvent,
  SocketServiceTag
> = {
  id: "hf-trajectory-list",

  initialState: () => {
    if (typeof window !== "undefined" && (window as any).bunLog) {
      (window as any).bunLog("[HFTrajectoryList] Creating initial state")
    }
    return {
      trajectories: [],
      filteredTrajectories: [],
      selectedSessionId: null,
      searchQuery: "",
      currentPage: 0,
      pageSize: 100,
      totalCount: 0,
      loading: true,
      error: null,
      collapsed: false,
    }
  },

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      if (typeof window !== "undefined" && (window as any).bunLog) {
        (window as any).bunLog(`[HFTrajectoryList] Rendering, loading=${state.loading}, totalCount=${state.totalCount}, trajectories=${state.trajectories.length}, error=${state.error}`)
      }

      // Header
      const header = html`
        <div
          class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/60 cursor-pointer bg-zinc-900/40"
          data-action="toggleCollapse"
        >
          <div class="flex items-center gap-2">
            <h3 class="text-sm font-bold font-mono text-zinc-100">Trajectories</h3>
            ${state.totalCount > 0 ? html`<span class="text-xs text-zinc-400">(${state.totalCount})</span>` : ""}
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

      // Search input
      const searchInput = html`
        <div class="px-4 pt-3 pb-2">
          <input
            type="text"
            placeholder="Search by agent, task, episode..."
            class="w-full px-3 py-2 bg-zinc-900 border border-zinc-800 rounded text-zinc-100 text-sm placeholder:text-zinc-500 focus:border-zinc-700 focus:outline-none"
            data-action="search"
            value="${state.searchQuery}"
          />
        </div>
      `

      // Page info
      const startIndex = state.currentPage * state.pageSize + 1
      const endIndex = Math.min((state.currentPage + 1) * state.pageSize, state.totalCount)
      const pageInfo = html`
        <div class="px-4 py-2 border-b border-zinc-800/40 text-xs text-zinc-400">
          Showing ${startIndex}-${endIndex} of ${state.totalCount}
        </div>
      `

      // Loading state
      if (state.loading) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header} ${searchInput} ${pageInfo}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">Loading trajectories...</div>
          </div>
        `
      }

      // Error state
      if (state.error) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header} ${searchInput} ${pageInfo}
            <div class="px-4 py-8">
              <div class="text-sm text-red-400 mb-2">Error loading trajectories</div>
              <div class="text-xs text-zinc-500">${state.error}</div>
              <button
                class="mt-3 px-3 py-1.5 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs transition-colors"
                data-action="refresh"
              >
                Retry
              </button>
            </div>
          </div>
        `
      }

      // Empty state
      if (state.filteredTrajectories.length === 0) {
        const message = state.searchQuery
          ? `No trajectories match "${state.searchQuery}"`
          : "No trajectories found"
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header} ${searchInput} ${pageInfo}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">${message}</div>
          </div>
        `
      }

      // Trajectory list
      const trajectoryItems = joinTemplates(
        state.filteredTrajectories.map((traj) => {
          const isSelected = traj.sessionId === state.selectedSessionId
          const baseClasses =
            "bg-zinc-900/40 border border-zinc-800/40 rounded-lg p-3 mb-2 cursor-pointer transition-colors"
          const selectedClasses = isSelected
            ? "bg-zinc-800/60 border-zinc-700/50"
            : "hover:bg-zinc-900/60"

          return html`
            <div
              class="${baseClasses} ${selectedClasses}"
              data-action="select"
              data-session-id="${traj.sessionId}"
              data-index="${traj.index}"
            >
              <div class="flex items-center justify-between mb-1">
                <span class="text-sm font-mono text-zinc-200">${traj.agentName}</span>
                <span class="text-xs text-zinc-500">${formatDate(traj.date)}</span>
              </div>
              <div class="text-xs text-zinc-400 mb-0.5">task: ${traj.task}</div>
              <div class="flex items-center gap-2 text-xs text-zinc-500">
                <span>${traj.episode}</span>
                <span>•</span>
                <span>${traj.stepCount} steps</span>
              </div>
            </div>
          `
        })
      )

      // Pagination controls
      const hasPrev = state.currentPage > 0
      const hasNext = (state.currentPage + 1) * state.pageSize < state.totalCount
      const pagination = html`
        <div class="px-4 py-3 border-t border-zinc-800/40 flex items-center justify-between">
          <button
            class="px-3 py-1.5 rounded bg-zinc-800 text-zinc-200 text-xs transition-colors ${hasPrev
          ? "hover:bg-zinc-700"
          : "opacity-50 cursor-not-allowed"}"
            data-action="prevPage"
            ${hasPrev ? "" : "disabled"}
          >
            ← Prev
          </button>
          <span class="text-xs text-zinc-500">Page ${state.currentPage + 1}</span>
          <button
            class="px-3 py-1.5 rounded bg-zinc-800 text-zinc-200 text-xs transition-colors ${hasNext
          ? "hover:bg-zinc-700"
          : "opacity-50 cursor-not-allowed"}"
            data-action="nextPage"
            ${hasNext ? "" : "disabled"}
          >
            Next →
          </button>
        </div>
      `

      return html`
        <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header} ${searchInput} ${pageInfo}
          <div class="max-h-[calc(100vh-16rem)] overflow-y-auto px-4 py-2">${trajectoryItems}</div>
          ${pagination}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Handle clicks
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action

        if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        } else if (action === "select") {
          const sessionId = el.dataset.sessionId
          const index = el.dataset.index
          if (sessionId && index) {
            Effect.runFork(ctx.emit({ type: "select", sessionId, index: parseInt(index, 10) }))
          }
        } else if (action === "prevPage") {
          Effect.runFork(ctx.emit({ type: "loadPage", page: -1 }))  // -1 = prev
        } else if (action === "nextPage") {
          Effect.runFork(ctx.emit({ type: "loadPage", page: -2 }))  // -2 = next
        } else if (action === "refresh") {
          Effect.runFork(ctx.emit({ type: "refresh" }))
        }
      })

      // Handle search input
      yield* ctx.dom.delegate(ctx.container, "[data-action='search']", "input", (e) => {
        const input = e.target as HTMLInputElement
        Effect.runFork(ctx.emit({ type: "search", query: input.value }))
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "loadPage": {
          const state = yield* ctx.state.get
          let newPage = event.page

          // Handle prev/next shortcuts
          if (newPage === -1) {
            newPage = Math.max(0, state.currentPage - 1)
          } else if (newPage === -2) {
            newPage = state.currentPage + 1
          }

          // Validate page bounds
          const maxPage = Math.ceil(state.totalCount / state.pageSize) - 1
          if (newPage < 0 || newPage > maxPage) break

          // Set loading
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null }))

          try {
            // Load page via RPC
            const offset = newPage * state.pageSize
            const trajectories = yield* socket.getHFTrajectories(offset, state.pageSize)

            const metadata = (trajectories as Trajectory[]).map((t, i) => extractMetadata(t, offset + i))

            // Update state
            yield* ctx.state.update((s) => ({
              ...s,
              trajectories: metadata,
              filteredTrajectories: filterTrajectories(metadata, s.searchQuery),
              currentPage: newPage,
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

        case "search": {
          yield* ctx.state.update((s) => ({
            ...s,
            searchQuery: event.query,
            filteredTrajectories: filterTrajectories(s.trajectories, event.query),
          }))
          break
        }

        case "select": {
          yield* ctx.state.update((s) => ({
            ...s,
            selectedSessionId: event.sessionId,
          }))
          // Note: Parent component (effuse-main.ts) will listen and load full trajectory
          break
        }

        case "toggleCollapse": {
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break
        }

        case "refresh": {
          yield* ctx.emit({ type: "loadPage", page: 0 })
          break
        }
      }
    }),

  subscriptions: (ctx) => {
    // Initial load on mount
    const initialLoad = Effect.gen(function* () {
      if (typeof window !== "undefined" && (window as any).bunLog) {
        (window as any).bunLog("[HFTrajectoryList] Starting initial load...")
      }
      const socket = yield* SocketServiceTag

      try {
        // Get total count via RPC
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog("[HFTrajectoryList] Getting trajectory count...")
        }
        const totalCount = yield* socket.getHFTrajectoryCount()
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog(`[HFTrajectoryList] Total count: ${totalCount}`)
        }

        // Load first page via RPC
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog("[HFTrajectoryList] Loading first page...")
        }
        const trajectories = yield* socket.getHFTrajectories(0, 100)
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog(`[HFTrajectoryList] Loaded trajectories: ${trajectories.length}`)
        }

        const metadata = (trajectories as Trajectory[]).map((t, i) => extractMetadata(t, i))

        yield* ctx.state.update((s) => ({
          ...s,
          trajectories: metadata,
          filteredTrajectories: metadata,
          totalCount,
          loading: false,
        }))
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog("[HFTrajectoryList] Initial load complete")
        }
      } catch (error) {
        if (typeof window !== "undefined" && (window as any).bunLog) {
          (window as any).bunLog(`[HFTrajectoryList] Initial load failed: ${error}`)
        }
        yield* ctx.state.update((s) => ({
          ...s,
          loading: false,
          error: error instanceof Error ? error.message : String(error),
        }))
      }
    })

    // Return as a stream that runs once
    return [Stream.make(initialLoad)]
  },
}

// ============================================================================
// Export initial state for testing
// ============================================================================

export const initialHFTrajectoryListState: HFTrajectoryListState = HFTrajectoryListWidget.initialState()
