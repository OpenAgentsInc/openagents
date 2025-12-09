/**
 * Container Panes Component
 *
 * Displays container execution output in a grid of panes.
 * Shows stdout/stderr streams with status, duration, and exit codes.
 */

import { Effect, Stream, pipe } from "effect"
import type { Component } from "../component/types.js"
import { html, joinTemplates } from "../template/html.js"
import { SocketServiceTag } from "../services/socket.js"
import type { HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/** Output stream type */
export type ContainerStreamType = "stdout" | "stderr"

/** Single output line with metadata */
export interface ContainerOutputLine {
  text: string
  stream: ContainerStreamType
  sequence: number
}

/** Container pane data */
export interface ContainerPane {
  executionId: string
  image: string
  command: string[]
  context: string
  sandboxed: boolean
  workdir: string
  status: "running" | "completed" | "error"
  exitCode?: number
  durationMs?: number
  outputLines: ContainerOutputLine[]
  startedAt: string
}

/** Component state */
export interface ContainerPanesState {
  /** All panes indexed by execution ID */
  panes: Map<string, ContainerPane>
  /** Maximum visible panes */
  maxVisible: number
  /** Maximum lines per pane */
  maxLinesPerPane: number
  /** Collapsed state */
  collapsed: boolean
}

/** Component events */
export type ContainerPanesEvent =
  | { type: "clear" }
  | { type: "toggleCollapse" }
  | { type: "dismiss"; executionId: string }

// ============================================================================
// Constants
// ============================================================================

const MAX_VISIBLE_PANES = 10
const MAX_LINES_PER_PANE = 500

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get status icon
 */
const getStatusIcon = (pane: ContainerPane): string => {
  if (pane.status === "running") return "▶"
  if (pane.status === "completed" && pane.exitCode === 0) return "✓"
  return "✗"
}

/**
 * Get status color class
 */
const getStatusColorClass = (pane: ContainerPane): string => {
  if (pane.status === "running") return "text-zinc-300"
  if (pane.exitCode === 0) return "text-emerald-400"
  return "text-red-400"
}

/**
 * Format duration
 */
const formatDuration = (ms?: number): string => {
  if (ms === undefined) return ""
  return `${(ms / 1000).toFixed(1)}s`
}

/**
 * Truncate command for display
 */
const truncateCommand = (command: string[], maxLen = 60): string => {
  const full = command.join(" ")
  if (full.length <= maxLen) return full
  return full.slice(0, maxLen) + "..."
}

// ============================================================================
// Type Guards
// ============================================================================

const isContainerStart = (msg: HudMessage): msg is HudMessage & {
  type: "container_start"
  executionId: string
  image: string
  command: string[]
  context: string
  sandboxed: boolean
  workdir: string
  timestamp: string
} => msg.type === "container_start"

const isContainerOutput = (msg: HudMessage): msg is HudMessage & {
  type: "container_output"
  executionId: string
  text: string
  stream: ContainerStreamType
  sequence: number
} => msg.type === "container_output"

const isContainerComplete = (msg: HudMessage): msg is HudMessage & {
  type: "container_complete"
  executionId: string
  exitCode: number
  durationMs: number
} => msg.type === "container_complete"

const isContainerError = (msg: HudMessage): msg is HudMessage & {
  type: "container_error"
  executionId: string
  reason: string
  error: string
} => msg.type === "container_error"

const isContainerMessage = (msg: HudMessage): boolean =>
  isContainerStart(msg) || isContainerOutput(msg) || isContainerComplete(msg) || isContainerError(msg)

// ============================================================================
// Component Definition
// ============================================================================

export const ContainerPanesComponent: Component<ContainerPanesState, ContainerPanesEvent, SocketServiceTag> = {
  id: "container-panes",

  initialState: () => ({
    panes: new Map(),
    maxVisible: MAX_VISIBLE_PANES,
    maxLinesPerPane: MAX_LINES_PER_PANE,
    collapsed: false,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Get panes sorted by start time (most recent first)
      const panes = Array.from(state.panes.values())
        .sort((a, b) => b.startedAt.localeCompare(a.startedAt))
        .slice(0, state.maxVisible)

      // Header with controls
      const header = html`
        <div class="flex items-center justify-between px-3 py-2 border-b border-zinc-800/60">
          <button
            class="text-sm font-medium text-zinc-300 hover:text-zinc-100 transition-colors"
            data-action="toggleCollapse"
          >
            ${state.collapsed ? "+" : "-"} Containers
          </button>
          <div class="flex items-center gap-2">
            <span class="text-xs text-zinc-500">${panes.length} active</span>
            ${panes.length > 0
              ? html`
                  <button
                    class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
                    data-action="clear"
                  >
                    Clear
                  </button>
                `
              : ""}
          </div>
        </div>
      `

      // Collapsed view
      if (state.collapsed) {
        return html`
          <div class="flex flex-col bg-zinc-950/80 border border-zinc-800/60 rounded-lg">
            ${header}
          </div>
        `
      }

      // Empty state
      if (panes.length === 0) {
        return html`
          <div class="flex flex-col bg-zinc-950/80 border border-zinc-800/60 rounded-lg">
            ${header}
            <div class="flex items-center justify-center py-8">
              <div class="text-sm text-zinc-500">No container executions</div>
            </div>
          </div>
        `
      }

      // Render pane cards
      const paneCards = panes.map((pane) => {
        const statusIcon = getStatusIcon(pane)
        const statusColor = getStatusColorClass(pane)
        const duration = formatDuration(pane.durationMs)
        const cmdDisplay = truncateCommand(pane.command)

        // Badge for sandbox vs host
        const badge = pane.sandboxed
          ? html`<span class="text-xs px-1.5 py-0.5 rounded bg-violet-900/40 text-violet-300 border border-violet-700/50">sandbox</span>`
          : html`<span class="text-xs px-1.5 py-0.5 rounded bg-amber-900/40 text-amber-300 border border-amber-700/50">host</span>`

        // Exit code badge
        const exitCodeBadge =
          pane.exitCode !== undefined
            ? html`
                <span
                  class="text-xs px-1.5 py-0.5 rounded font-mono ${pane.exitCode === 0
                    ? "bg-emerald-900/40 text-emerald-300"
                    : "bg-red-900/40 text-red-300"}"
                >
                  ${pane.exitCode}
                </span>
              `
            : ""

        // Output lines (last 100)
        const outputLines = pane.outputLines.slice(-100).map(
          (line) => html`
            <div class="font-mono text-xs ${line.stream === "stderr" ? "text-red-400" : "text-zinc-300"}">
              ${line.text}
            </div>
          `
        )

        return html`
          <div
            class="border border-zinc-800/50 rounded-lg overflow-hidden ${pane.status === "running"
              ? "border-l-2 border-l-blue-500"
              : ""}"
            data-execution-id="${pane.executionId}"
          >
            <div class="flex items-center justify-between px-3 py-2 bg-zinc-900/60">
              <div class="flex items-center gap-2">
                <span class="${statusColor}">${statusIcon}</span>
                <span class="text-sm text-zinc-300">${pane.image}</span>
                ${badge}
              </div>
              <div class="flex items-center gap-2">
                ${duration ? html`<span class="text-xs text-zinc-500">${duration}</span>` : ""}
                ${exitCodeBadge}
                <button
                  class="text-zinc-500 hover:text-zinc-300 transition-colors"
                  data-action="dismiss"
                  data-execution-id="${pane.executionId}"
                >
                  ×
                </button>
              </div>
            </div>
            <div class="px-3 py-1 border-b border-zinc-800/40 bg-zinc-900/40">
              <code class="text-xs text-zinc-400 font-mono" title="${pane.command.join(" ")}">
                ${cmdDisplay}
              </code>
            </div>
            <div class="px-3 py-2 max-h-40 overflow-y-auto bg-zinc-950/60">
              ${outputLines.length > 0 ? joinTemplates(outputLines) : html`<div class="text-xs text-zinc-600">No output yet</div>`}
            </div>
          </div>
        `
      })

      return html`
        <div class="flex flex-col bg-zinc-950/80 border border-zinc-800/60 rounded-lg">
          ${header}
          <div class="p-3 grid gap-3">
            ${joinTemplates(paneCards)}
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const el = target as HTMLElement
        const action = el.dataset.action
        const executionId = el.dataset.executionId

        if (action === "clear") {
          Effect.runFork(ctx.emit({ type: "clear" }))
        } else if (action === "toggleCollapse") {
          Effect.runFork(ctx.emit({ type: "toggleCollapse" }))
        } else if (action === "dismiss" && executionId) {
          Effect.runFork(ctx.emit({ type: "dismiss", executionId }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "clear":
          yield* ctx.state.update((s) => ({
            ...s,
            panes: new Map(),
          }))
          break

        case "toggleCollapse":
          yield* ctx.state.update((s) => ({ ...s, collapsed: !s.collapsed }))
          break

        case "dismiss":
          yield* ctx.state.update((s) => {
            const newPanes = new Map(s.panes)
            newPanes.delete(event.executionId)
            return { ...s, panes: newPanes }
          })
          break
      }
    }),

  subscriptions: (ctx) => {
    const socket = Effect.map(SocketServiceTag, (s) => s)

    return [
      pipe(
        Stream.unwrap(Effect.map(socket, (s) => s.getMessages())),
        Stream.filter((msg): msg is HudMessage => isContainerMessage(msg)),
        Stream.map((msg) =>
          Effect.gen(function* () {
            if (isContainerStart(msg)) {
              yield* ctx.state.update((s) => {
                const newPanes = new Map(s.panes)
                newPanes.set(msg.executionId, {
                  executionId: msg.executionId,
                  image: msg.image,
                  command: msg.command,
                  context: msg.context,
                  sandboxed: msg.sandboxed,
                  workdir: msg.workdir,
                  status: "running",
                  outputLines: [],
                  startedAt: msg.timestamp,
                })
                return { ...s, panes: newPanes }
              })
            }

            if (isContainerOutput(msg)) {
              yield* ctx.state.update((s) => {
                const pane = s.panes.get(msg.executionId)
                if (!pane) return s

                const newPanes = new Map(s.panes)
                const updatedLines = [
                  ...pane.outputLines,
                  { text: msg.text, stream: msg.stream, sequence: msg.sequence },
                ].slice(-s.maxLinesPerPane)

                newPanes.set(msg.executionId, {
                  ...pane,
                  outputLines: updatedLines,
                })
                return { ...s, panes: newPanes }
              })
            }

            if (isContainerComplete(msg)) {
              yield* ctx.state.update((s) => {
                const pane = s.panes.get(msg.executionId)
                if (!pane) return s

                const newPanes = new Map(s.panes)
                newPanes.set(msg.executionId, {
                  ...pane,
                  status: "completed",
                  exitCode: msg.exitCode,
                  durationMs: msg.durationMs,
                })
                return { ...s, panes: newPanes }
              })
            }

            if (isContainerError(msg)) {
              yield* ctx.state.update((s) => {
                const pane = s.panes.get(msg.executionId)
                if (!pane) return s

                const newPanes = new Map(s.panes)
                newPanes.set(msg.executionId, {
                  ...pane,
                  status: "error",
                  outputLines: [
                    ...pane.outputLines,
                    { text: `Error: ${msg.error}`, stream: "stderr" as const, sequence: Date.now() },
                  ],
                })
                return { ...s, panes: newPanes }
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

export const initialContainerPanesState: ContainerPanesState = ContainerPanesComponent.initialState()
