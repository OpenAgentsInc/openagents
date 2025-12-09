/**
 * Trajectory Pane Component
 *
 * Displays unified trajectories (TB runs, ATIF traces) in a side panel.
 * Supports loading, rendering, and selecting trajectories.
 */

import { Effect } from "effect"
import type { Component } from "../component/types.js"
import { html } from "../template/html.js"
import { SocketServiceTag } from "../services/socket.js"
import type { UnifiedTrajectory } from "../../desktop/protocol.js"

// ============================================================================
// Types
// ============================================================================

/**
 * Trajectory Pane State
 */
export interface TrajectoryPaneState {
  /** List of trajectories */
  trajectories: UnifiedTrajectory[]
  /** Currently selected trajectory ID */
  selectedId: string | null
  /** Loading state */
  loading: boolean
  /** Error message if any */
  error: string | null
  /** Collapsed state */
  collapsed: boolean
}

/**
 * Trajectory Pane Events
 */
export type TrajectoryPaneEvent =
  | { type: "load" }
  | { type: "select"; trajectoryId: string }
  | { type: "toggleCollapse" }
  | { type: "clear" }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Format timestamp to local string
 */
const formatTimestamp = (iso: string): string => {
  try {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    })
  } catch {
    return iso
  }
}

/**
 * Get short ID (last 8 chars)
 */
const shortId = (id: string): string => id.slice(-8)

/**
 * Get type badge class
 */
const getTypeClass = (type: UnifiedTrajectory["type"]): string => {
  return type === "tb-run"
    ? "bg-emerald-900/40 text-emerald-300 border-emerald-700/50"
    : "bg-violet-900/40 text-violet-300 border-violet-700/50"
}

/**
 * Get type label
 */
const getTypeLabel = (type: UnifiedTrajectory["type"]): string => {
  return type === "tb-run" ? "TB" : "ATIF"
}

// ============================================================================
// Component Definition
// ============================================================================

export const TrajectoryPaneComponent: Component<TrajectoryPaneState, TrajectoryPaneEvent, SocketServiceTag> = {
  id: "trajectory-pane",

  initialState: () => ({
    trajectories: [],
    selectedId: null,
    loading: false,
    error: null,
    collapsed: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Header with collapse toggle and load button
      const header = html`
        <div class="flex items-center justify-between px-3 py-2 border-b border-zinc-800/60">
          <button
            class="text-sm font-medium text-zinc-300 hover:text-zinc-100 transition-colors"
            data-action="toggleCollapse"
          >
            ${state.collapsed ? "+" : "-"} Trajectories
          </button>
          <div class="flex items-center gap-2">
            <span class="text-xs text-zinc-500">${state.trajectories.length}</span>
            <button
              class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
              data-action="load"
            >
              ${state.loading ? "Loading..." : "Refresh"}
            </button>
          </div>
        </div>
      `

      // Collapsed view
      if (state.collapsed) {
        return html`
          <div class="h-full flex flex-col bg-zinc-950/80 border-r border-zinc-800/60">
            ${header}
          </div>
        `
      }

      // Loading state
      if (state.loading && state.trajectories.length === 0) {
        return html`
          <div class="h-full flex flex-col bg-zinc-950/80 border-r border-zinc-800/60">
            ${header}
            <div class="flex-1 flex items-center justify-center">
              <div class="text-sm text-zinc-500">Loading trajectories...</div>
            </div>
          </div>
        `
      }

      // Error state
      if (state.error && state.trajectories.length === 0) {
        return html`
          <div class="h-full flex flex-col bg-zinc-950/80 border-r border-zinc-800/60">
            ${header}
            <div class="flex-1 flex items-center justify-center p-4">
              <div class="text-sm text-red-400 text-center">${state.error}</div>
            </div>
          </div>
        `
      }

      // Empty state
      if (state.trajectories.length === 0) {
        return html`
          <div class="h-full flex flex-col bg-zinc-950/80 border-r border-zinc-800/60">
            ${header}
            <div class="flex-1 flex items-center justify-center p-4">
              <div class="text-sm text-zinc-500 text-center">
                No trajectories found.<br />
                <button class="text-zinc-400 hover:text-zinc-200 underline mt-2" data-action="load">
                  Load trajectories
                </button>
              </div>
            </div>
          </div>
        `
      }

      // Trajectory list
      const trajectoryCards = state.trajectories.map((traj) => {
        const isSelected = traj.id === state.selectedId
        const typeClass = getTypeClass(traj.type)

        return html`
          <div
            class="px-3 py-2 border-b border-zinc-800/40 cursor-pointer transition-colors
                   ${isSelected ? "bg-zinc-800/60" : "hover:bg-zinc-900/40"}"
            data-action="select"
            data-trajectory-id="${traj.id}"
          >
            <div class="flex items-center justify-between mb-1">
              <code class="text-xs font-mono text-zinc-400">${shortId(traj.id)}</code>
              <span class="text-xs px-1.5 py-0.5 rounded border ${typeClass}">
                ${getTypeLabel(traj.type)}
              </span>
            </div>
            <div class="text-sm text-zinc-300 truncate" title="${traj.label}">
              ${traj.label}
            </div>
            <div class="text-xs text-zinc-500 mt-1">
              ${formatTimestamp(traj.timestamp)}
            </div>
          </div>
        `
      })

      return html`
        <div class="h-full flex flex-col bg-zinc-950/80 border-r border-zinc-800/60">
          ${header}
          <div class="flex-1 overflow-y-auto">
            ${trajectoryCards}
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const action = (target as HTMLElement).dataset.action
        const trajectoryId = (target as HTMLElement).dataset.trajectoryId

        if (action === "load") {
          Effect.runFork(ctx.emit({ type: "load" }))
        } else if (action === "select" && trajectoryId) {
          Effect.runFork(ctx.emit({ type: "select", trajectoryId }))
        } else if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socket = yield* SocketServiceTag

      switch (event.type) {
        case "load":
          yield* ctx.state.update((s) => ({ ...s, loading: true, error: null }))

          const result = yield* socket.loadUnifiedTrajectories(50).pipe(
            Effect.map((trajectories) => ({ trajectories, error: null })),
            Effect.catchAll((e) => Effect.succeed({ trajectories: [] as UnifiedTrajectory[], error: e.message }))
          )

          yield* ctx.state.update((s) => ({
            ...s,
            loading: false,
            trajectories: result.trajectories,
            error: result.error,
          }))
          break

        case "select":
          yield* ctx.state.update((s) => ({ ...s, selectedId: event.trajectoryId }))
          // Future: emit event to parent to show trajectory details
          break

        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "clear":
          yield* ctx.state.update((s) => ({
            ...s,
            trajectories: [],
            selectedId: null,
            error: null,
          }))
          break
      }
    }),
}

// ============================================================================
// Export initial state for testing
// ============================================================================

export const initialTrajectoryPaneState: TrajectoryPaneState = TrajectoryPaneComponent.initialState()
