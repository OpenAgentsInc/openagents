/**
 * APM Component - Actions Per Minute Monitor
 *
 * Displays real-time APM metrics from agent sessions.
 * First Effuse component implementation.
 */

import { Effect, Stream, pipe } from "effect"
import { html } from "../template/html.js"
import type { Component } from "../component/types.js"
import { SocketServiceTag } from "../services/socket.js"
import type { HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/**
 * APM Component State
 */
export interface APMState {
  /** Current session APM */
  sessionAPM: number
  /** APM over last 5 minutes */
  recentAPM: number
  /** Total actions this session */
  totalActions: number
  /** Session duration in minutes */
  durationMinutes: number
  /** Historical APM values */
  apm1h: number
  apm6h: number
  apm1d: number
  apmLifetime: number
  /** Comparison metrics */
  claudeCodeAPM: number
  mechaCoderAPM: number
  efficiencyRatio: number
  /** UI state */
  expanded: boolean
}

/**
 * APM Component Events
 */
export type APMEvent =
  | { type: "toggleExpand" }
  | { type: "refresh" }

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get color class based on APM value
 */
const getAPMColorClass = (apm: number): string => {
  if (apm >= 30) return "text-emerald-400" // High activity
  if (apm >= 15) return "text-zinc-200" // Good
  if (apm >= 5) return "text-zinc-400" // Active
  return "text-zinc-500" // Baseline
}

/**
 * Get background gradient based on APM
 */
const getAPMBgClass = (apm: number): string => {
  if (apm >= 30) return "bg-emerald-950/20 border-emerald-800/40"
  if (apm >= 15) return "bg-zinc-900/60 border-zinc-700/50"
  if (apm >= 5) return "bg-zinc-900/40 border-zinc-700/40"
  return "bg-zinc-950/40 border-zinc-800/30"
}

/**
 * Format duration nicely
 */
const formatDuration = (minutes: number): string => {
  if (minutes < 60) return `${minutes.toFixed(0)}m`
  const hours = Math.floor(minutes / 60)
  const mins = Math.round(minutes % 60)
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`
}

/**
 * Type guard for APM update message
 */
const isAPMUpdate = (msg: HudMessage): msg is HudMessage & {
  type: "apm_update"
  sessionAPM: number
  recentAPM: number
  totalActions: number
  durationMinutes: number
} => msg.type === "apm_update"

/**
 * Type guard for APM snapshot message
 */
const isAPMSnapshot = (msg: HudMessage): msg is HudMessage & {
  type: "apm_snapshot"
  combined: {
    apm1h: number
    apm6h: number
    apm1d: number
    apmLifetime: number
  }
  comparison: {
    claudeCodeAPM: number
    mechaCoderAPM: number
    efficiencyRatio: number
  }
} => msg.type === "apm_snapshot"

// ============================================================================
// Component Definition
// ============================================================================

export const APMComponent: Component<APMState, APMEvent, SocketServiceTag> = {
  id: "apm-component",

  initialState: () => ({
    sessionAPM: 0,
    recentAPM: 0,
    totalActions: 0,
    durationMinutes: 0,
    apm1h: 0,
    apm6h: 0,
    apm1d: 0,
    apmLifetime: 0,
    claudeCodeAPM: 0,
    mechaCoderAPM: 0,
    efficiencyRatio: 0,
    expanded: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      const colorClass = getAPMColorClass(state.sessionAPM)
      const bgClass = getAPMBgClass(state.sessionAPM)

      // Compact view (default)
      if (!state.expanded) {
        return html`
          <div
            class="fixed bottom-4 right-4 rounded-xl border ${bgClass} px-4 py-3 shadow-lg backdrop-blur-sm cursor-pointer transition-all hover:scale-105"
            data-action="toggleExpand"
          >
            <div class="flex items-center gap-3">
              <div class="text-2xl font-bold ${colorClass} font-mono">
                ${state.sessionAPM.toFixed(1)}
              </div>
              <div class="text-xs text-zinc-500 uppercase tracking-wide">APM</div>
            </div>
            ${state.totalActions > 0
              ? html`
                  <div class="text-xs text-zinc-500 mt-1">
                    ${state.totalActions} actions in ${formatDuration(state.durationMinutes)}
                  </div>
                `
              : ""}
          </div>
        `
      }

      // Expanded view
      return html`
        <div
          class="fixed bottom-4 right-4 w-72 rounded-xl border ${bgClass} shadow-xl backdrop-blur-sm"
        >
          <!-- Header -->
          <div
            class="flex items-center justify-between px-4 py-3 border-b border-zinc-800/50 cursor-pointer"
            data-action="toggleExpand"
          >
            <div class="flex items-center gap-2">
              <div class="text-xl font-bold ${colorClass} font-mono">
                ${state.sessionAPM.toFixed(1)}
              </div>
              <div class="text-xs text-zinc-500 uppercase">APM</div>
            </div>
            <div class="text-zinc-500 text-sm">-</div>
          </div>

          <!-- Current Session -->
          <div class="px-4 py-3 border-b border-zinc-800/30">
            <div class="text-xs text-zinc-500 uppercase tracking-wide mb-2">Session</div>
            <div class="grid grid-cols-2 gap-2 text-sm">
              <div>
                <span class="text-zinc-500">Recent:</span>
                <span class="text-zinc-300 font-mono ml-1">${state.recentAPM.toFixed(1)}</span>
              </div>
              <div>
                <span class="text-zinc-500">Actions:</span>
                <span class="text-zinc-300 font-mono ml-1">${state.totalActions}</span>
              </div>
              <div class="col-span-2">
                <span class="text-zinc-500">Duration:</span>
                <span class="text-zinc-300 ml-1">${formatDuration(state.durationMinutes)}</span>
              </div>
            </div>
          </div>

          <!-- Historical -->
          ${state.apm1h > 0 || state.apmLifetime > 0
            ? html`
                <div class="px-4 py-3 border-b border-zinc-800/30">
                  <div class="text-xs text-zinc-500 uppercase tracking-wide mb-2">Historical</div>
                  <div class="grid grid-cols-2 gap-2 text-sm">
                    <div>
                      <span class="text-zinc-500">1h:</span>
                      <span class="text-zinc-300 font-mono ml-1">${state.apm1h.toFixed(1)}</span>
                    </div>
                    <div>
                      <span class="text-zinc-500">6h:</span>
                      <span class="text-zinc-300 font-mono ml-1">${state.apm6h.toFixed(1)}</span>
                    </div>
                    <div>
                      <span class="text-zinc-500">1d:</span>
                      <span class="text-zinc-300 font-mono ml-1">${state.apm1d.toFixed(1)}</span>
                    </div>
                    <div>
                      <span class="text-zinc-500">All:</span>
                      <span class="text-zinc-300 font-mono ml-1">${state.apmLifetime.toFixed(1)}</span>
                    </div>
                  </div>
                </div>
              `
            : ""}

          <!-- Comparison -->
          ${state.efficiencyRatio > 0
            ? html`
                <div class="px-4 py-3">
                  <div class="text-xs text-zinc-500 uppercase tracking-wide mb-2">Comparison</div>
                  <div class="grid grid-cols-2 gap-2 text-sm">
                    <div>
                      <span class="text-zinc-500">Claude:</span>
                      <span class="text-zinc-300 font-mono ml-1">${state.claudeCodeAPM.toFixed(1)}</span>
                    </div>
                    <div>
                      <span class="text-zinc-500">MC:</span>
                      <span class="text-zinc-300 font-mono ml-1">${state.mechaCoderAPM.toFixed(1)}</span>
                    </div>
                  </div>
                  <div class="mt-2 text-sm">
                    <span class="text-emerald-400 font-medium">
                      ${state.efficiencyRatio.toFixed(1)}x
                    </span>
                    <span class="text-zinc-500 ml-1">efficiency boost</span>
                  </div>
                </div>
              `
            : ""}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Use event delegation for toggle
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const action = (target as HTMLElement).dataset.action
        if (action === "toggleExpand") {
          Effect.runFork(ctx.emit({ type: "toggleExpand" }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "toggleExpand":
          yield* ctx.state.update((s) => ({ ...s, expanded: !s.expanded }))
          break
        case "refresh":
          // Could request fresh APM data if needed
          break
      }
    }),

  subscriptions: (ctx) => {
    const socket = Effect.map(SocketServiceTag, (s) => s)

    // Subscribe to APM messages from socket
    return [
      pipe(
        Stream.unwrap(
          Effect.map(socket, (s) => s.getMessages())
        ),
        Stream.filter((msg): msg is HudMessage => isAPMUpdate(msg) || isAPMSnapshot(msg)),
        Stream.map((msg) =>
          Effect.gen(function* () {
            if (isAPMUpdate(msg)) {
              yield* ctx.state.update((s) => ({
                ...s,
                sessionAPM: msg.sessionAPM,
                recentAPM: msg.recentAPM,
                totalActions: msg.totalActions,
                durationMinutes: msg.durationMinutes,
              }))
            }

            if (isAPMSnapshot(msg)) {
              yield* ctx.state.update((s) => ({
                ...s,
                apm1h: msg.combined.apm1h,
                apm6h: msg.combined.apm6h,
                apm1d: msg.combined.apm1d,
                apmLifetime: msg.combined.apmLifetime,
                claudeCodeAPM: msg.comparison.claudeCodeAPM,
                mechaCoderAPM: msg.comparison.mechaCoderAPM,
                efficiencyRatio: msg.comparison.efficiencyRatio,
              }))
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

export const initialAPMState: APMState = APMComponent.initialState()
