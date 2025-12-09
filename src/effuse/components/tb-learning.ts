/**
 * TB Learning Metrics Component
 *
 * Displays FM (Foundation Model) learning features during Terminal Bench runs:
 * - Skills used/learned
 * - Memories used
 * - Reflexion status
 * - Learning summary
 */

import { Effect, Stream, pipe } from "effect"
import type { Component } from "../component/types.js"
import { html } from "../template/html.js"
import { SocketServiceTag } from "../services/socket.js"
import type { TBLearningMetricsMessage, TBLearningSummaryMessage, HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

export interface TBLearningState {
  /** Current run ID being tracked */
  runId: string | null
  /** Model being used */
  model: string | null
  /** Learning features enabled */
  learningFlags: {
    skills: boolean
    memory: boolean
    reflexion: boolean
    learn: boolean
  } | null
  /** Skills used in current run */
  skillsUsed: number
  /** Skill IDs used */
  skillIds: string[]
  /** Memories used */
  memoriesUsed: number
  /** Reflexion enabled */
  reflexionEnabled: boolean
  /** Reflections generated */
  reflectionsGenerated: number
  /** New skills learned */
  newSkillsLearned: number
  /** Skill library size */
  skillLibrarySize: number | null
  /** Summary data */
  summary: {
    totalTasks: number
    passed: number
    passRate: number
  } | null
  /** Collapsed state */
  collapsed: boolean
  /** Loading state */
  loading: boolean
}

export type TBLearningEvent = { type: "toggleCollapse" } | { type: "clear" }

// ============================================================================
// Type Guards
// ============================================================================

const isTBLearningMetrics = (msg: unknown): msg is TBLearningMetricsMessage => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return m.type === "tb_learning_metrics"
}

const isTBLearningSummary = (msg: unknown): msg is TBLearningSummaryMessage => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return m.type === "tb_learning_summary"
}

const isTBRunStart = (
  msg: unknown
): msg is {
  type: "tb_run_start"
  runId: string
} => {
  if (typeof msg !== "object" || msg === null) return false
  const m = msg as Record<string, unknown>
  return m.type === "tb_run_start" && typeof m.runId === "string"
}

// ============================================================================
// Component Definition
// ============================================================================

export const TBLearningComponent: Component<TBLearningState, TBLearningEvent, SocketServiceTag> = {
  id: "tb-learning",

  initialState: () => ({
    runId: null,
    model: null,
    learningFlags: null,
    skillsUsed: 0,
    skillIds: [],
    memoriesUsed: 0,
    reflexionEnabled: false,
    reflectionsGenerated: 0,
    newSkillsLearned: 0,
    skillLibrarySize: null,
    summary: null,
    collapsed: false,
    loading: false,
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
            <h3 class="text-sm font-bold font-mono text-zinc-100">Learning Metrics</h3>
            ${state.model ? html`<span class="text-xs px-2 py-0.5 rounded bg-violet-900/40 text-violet-300 border border-violet-700/50 font-mono">${state.model}</span>` : ""}
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
      if (!state.runId && !state.summary) {
        return html`
          <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm">
            ${header}
            <div class="px-4 py-8 text-center text-sm text-zinc-500">
              ${state.loading ? "Waiting for learning data..." : "No learning metrics available"}
            </div>
          </div>
        `
      }

      // Learning flags indicators
      const flagsDisplay = state.learningFlags
        ? html`
            <div class="px-4 py-2 bg-zinc-900/20 border-b border-zinc-800/40">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-xs text-zinc-500 font-mono">Features:</span>
                ${state.learningFlags.skills
                  ? html`<span class="text-xs px-2 py-0.5 rounded bg-emerald-900/40 text-emerald-300 border border-emerald-700/50 font-mono">✓ Skills</span>`
                  : html`<span class="text-xs px-2 py-0.5 rounded bg-zinc-800/40 text-zinc-500 border border-zinc-700/50 font-mono">Skills</span>`}
                ${state.learningFlags.memory
                  ? html`<span class="text-xs px-2 py-0.5 rounded bg-emerald-900/40 text-emerald-300 border border-emerald-700/50 font-mono">✓ Memory</span>`
                  : html`<span class="text-xs px-2 py-0.5 rounded bg-zinc-800/40 text-zinc-500 border border-zinc-700/50 font-mono">Memory</span>`}
                ${state.learningFlags.reflexion
                  ? html`<span class="text-xs px-2 py-0.5 rounded bg-emerald-900/40 text-emerald-300 border border-emerald-700/50 font-mono">✓ Reflexion</span>`
                  : html`<span class="text-xs px-2 py-0.5 rounded bg-zinc-800/40 text-zinc-500 border border-zinc-700/50 font-mono">Reflexion</span>`}
                ${state.learningFlags.learn
                  ? html`<span class="text-xs px-2 py-0.5 rounded bg-emerald-900/40 text-emerald-300 border border-emerald-700/50 font-mono">✓ Learn</span>`
                  : html`<span class="text-xs px-2 py-0.5 rounded bg-zinc-800/40 text-zinc-500 border border-zinc-700/50 font-mono">Learn</span>`}
              </div>
            </div>
          `
        : ""

      // Metrics display
      const metricsDisplay = html`
        <div class="px-4 py-3 space-y-3">
          <div class="grid grid-cols-2 gap-4 text-xs font-mono">
            <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
              <div class="text-zinc-500 mb-1">Skills Used</div>
              <div class="text-2xl font-bold text-violet-400">${state.skillsUsed}</div>
              ${state.skillIds.length > 0
                ? html`<div class="text-[10px] text-zinc-500 mt-1 truncate" title="${state.skillIds.join(", ")}">${state.skillIds.slice(0, 3).join(", ")}${state.skillIds.length > 3 ? "..." : ""}</div>`
                : ""}
            </div>

            <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
              <div class="text-zinc-500 mb-1">Memories Used</div>
              <div class="text-2xl font-bold text-blue-400">${state.memoriesUsed}</div>
            </div>

            <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
              <div class="text-zinc-500 mb-1">Reflections</div>
              <div class="text-2xl font-bold text-amber-400">${state.reflectionsGenerated}</div>
              ${state.reflexionEnabled ? html`<div class="text-[10px] text-emerald-400 mt-1">✓ Enabled</div>` : html`<div class="text-[10px] text-zinc-600 mt-1">Disabled</div>`}
            </div>

            <div class="bg-zinc-900/40 rounded-lg p-3 border border-zinc-800/40">
              <div class="text-zinc-500 mb-1">Skills Learned</div>
              <div class="text-2xl font-bold text-emerald-400">${state.newSkillsLearned}</div>
              ${state.skillLibrarySize !== null ? html`<div class="text-[10px] text-zinc-500 mt-1">Library: ${state.skillLibrarySize}</div>` : ""}
            </div>
          </div>
        </div>
      `

      // Summary display (shown after run complete)
      const summaryDisplay = state.summary
        ? html`
            <div class="px-4 py-3 border-t border-zinc-800/40 bg-zinc-900/20">
              <div class="text-xs font-mono text-zinc-400 mb-2">Run Summary</div>
              <div class="grid grid-cols-3 gap-3 text-xs font-mono">
                <div>
                  <div class="text-zinc-500">Tasks</div>
                  <div class="text-zinc-200">${state.summary.totalTasks}</div>
                </div>
                <div>
                  <div class="text-zinc-500">Passed</div>
                  <div class="text-emerald-400">${state.summary.passed}</div>
                </div>
                <div>
                  <div class="text-zinc-500">Pass Rate</div>
                  <div class="${state.summary.passRate >= 0.8 ? "text-emerald-400" : state.summary.passRate >= 0.5 ? "text-amber-400" : "text-red-400"}">
                    ${Math.round(state.summary.passRate * 100)}%
                  </div>
                </div>
              </div>
            </div>
          `
        : ""

      return html`
        <div class="rounded-xl border border-zinc-800/60 bg-zinc-950/80 shadow-xl backdrop-blur-sm overflow-hidden">
          ${header} ${flagsDisplay} ${metricsDisplay} ${summaryDisplay}
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Handle button clicks
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action

        if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "clear":
          yield* ctx.state.update(() => TBLearningComponent.initialState())
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
                runId: msg.runId,
                loading: true,
              }))
            }

            // Track learning metrics updates
            if (isTBLearningMetrics(msg)) {
              const state = yield* ctx.state.get
              if (state.runId === msg.runId) {
                yield* ctx.state.update((s) => ({
                  ...s,
                  model: msg.model,
                  skillsUsed: s.skillsUsed + msg.skillsUsed,
                  skillIds: Array.from(new Set([...s.skillIds, ...msg.skillIds])),
                  memoriesUsed: s.memoriesUsed + msg.memoriesUsed,
                  reflexionEnabled: msg.reflexionEnabled || s.reflexionEnabled,
                  reflectionsGenerated: s.reflectionsGenerated + msg.reflectionsGenerated,
                  newSkillsLearned: s.newSkillsLearned + msg.newSkillsLearned,
                  loading: false,
                }))
              }
            }

            // Track learning summary
            if (isTBLearningSummary(msg)) {
              const state = yield* ctx.state.get
              if (state.runId === msg.runId) {
                yield* ctx.state.update((s) => ({
                  ...s,
                  model: msg.model,
                  learningFlags: msg.learningFlags,
                  skillsUsed: msg.totalSkillsUsed,
                  memoriesUsed: msg.totalMemoriesUsed,
                  reflectionsGenerated: msg.totalReflectionsGenerated,
                  newSkillsLearned: msg.newSkillsLearned,
                  skillLibrarySize: msg.skillLibrarySize ?? null,
                  summary: {
                    totalTasks: msg.totalTasks,
                    passed: msg.passed,
                    passRate: msg.passRate,
                  },
                  loading: false,
                }))
              }
            }
          })
        )
      ),
    ]
  },
}

export const initialTBLearningState: TBLearningState = TBLearningComponent.initialState()
