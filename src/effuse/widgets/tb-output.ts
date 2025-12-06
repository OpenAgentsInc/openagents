/**
 * TB Output Widget
 *
 * Displays Terminal-Bench output (agent, verification, system) in a side panel.
 * Supports clear, copy, and close operations. Subscribes to tb_task_output messages.
 */

import { Effect, Stream, pipe } from "effect"
import { html, joinTemplates } from "../template/html.js"
import type { Widget } from "../widget/types.js"
import { SocketServiceTag } from "../services/socket.js"
import type { HudMessage } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/** Output source type */
export type TBOutputSource = "agent" | "verification" | "system"

/** Single output line with metadata */
export interface TBOutputLine {
  text: string
  source: TBOutputSource
  timestamp: number
}

/** Widget state */
export interface TBOutputState {
  /** Output lines buffer */
  outputLines: TBOutputLine[]
  /** Maximum lines to display */
  maxLines: number
  /** Whether the viewer is visible */
  visible: boolean
  /** Current run ID (null if not running) */
  runId: string | null
  /** Current task ID */
  taskId: string | null
  /** Auto-scroll enabled */
  autoScroll: boolean
}

/** Widget events */
export type TBOutputEvent =
  | { type: "clear" }
  | { type: "copy" }
  | { type: "close" }
  | { type: "open" }
  | { type: "toggleAutoScroll" }

// ============================================================================
// Constants
// ============================================================================

const MAX_OUTPUT_LINES = 500

// ============================================================================
// Helpers
// ============================================================================

/**
 * Get source color class
 */
const getSourceColorClass = (source: TBOutputSource): string => {
  switch (source) {
    case "agent":
      return "text-blue-300"
    case "verification":
      return "text-emerald-300"
    case "system":
      return "text-zinc-400"
  }
}

/**
 * Get source label
 */
const getSourceLabel = (source: TBOutputSource): string => {
  switch (source) {
    case "agent":
      return "AGT"
    case "verification":
      return "VRF"
    case "system":
      return "SYS"
  }
}

// ============================================================================
// Type Guards
// ============================================================================

const isTBTaskOutput = (msg: HudMessage): msg is HudMessage & {
  type: "tb_task_output"
  runId: string
  taskId: string
  text: string
  source: TBOutputSource
} => msg.type === "tb_task_output"

const isTBRunStart = (msg: HudMessage): msg is HudMessage & {
  type: "tb_run_start"
  runId: string
} => msg.type === "tb_run_start"

const isTBRunComplete = (msg: HudMessage): msg is HudMessage & {
  type: "tb_run_complete"
  runId: string
} => msg.type === "tb_run_complete"

const isTBMessage = (msg: HudMessage): boolean =>
  isTBTaskOutput(msg) || isTBRunStart(msg) || isTBRunComplete(msg)

// ============================================================================
// Widget Definition
// ============================================================================

export const TBOutputWidget: Widget<TBOutputState, TBOutputEvent, SocketServiceTag> = {
  id: "tb-output",

  initialState: () => ({
    outputLines: [],
    maxLines: MAX_OUTPUT_LINES,
    visible: false,
    runId: null,
    taskId: null,
    autoScroll: true,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Hidden state
      if (!state.visible) {
        return html`<div class="hidden"></div>`
      }

      // Header with controls
      const header = html`
        <div class="flex items-center justify-between px-3 py-2 border-b border-zinc-800/60 bg-zinc-900/80">
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium text-zinc-300">TB Output</span>
            ${state.runId
              ? html`<code class="text-xs text-zinc-500 font-mono">${state.runId.slice(-8)}</code>`
              : ""}
            ${state.taskId
              ? html`<span class="text-xs text-zinc-400">→ ${state.taskId}</span>`
              : ""}
          </div>
          <div class="flex items-center gap-2">
            <button
              class="text-xs px-2 py-1 rounded border transition-colors ${state.autoScroll
                ? "bg-blue-900/40 text-blue-300 border-blue-700/50"
                : "text-zinc-400 border-zinc-700/50 hover:border-zinc-600/60"}"
              data-action="toggleAutoScroll"
              title="Auto-scroll"
            >
              ↓
            </button>
            <button
              class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
              data-action="copy"
            >
              Copy
            </button>
            <button
              class="text-xs text-zinc-400 hover:text-zinc-200 px-2 py-1 rounded border border-zinc-700/50 hover:border-zinc-600/60 transition-colors"
              data-action="clear"
            >
              Clear
            </button>
            <button
              class="text-zinc-500 hover:text-zinc-300 transition-colors"
              data-action="close"
            >
              ×
            </button>
          </div>
        </div>
      `

      // Output lines
      const lines =
        state.outputLines.length > 0
          ? joinTemplates(
              state.outputLines.slice(-100).map(
                (line) => html`
                  <div class="flex gap-2 font-mono text-xs leading-relaxed">
                    <span class="w-8 flex-shrink-0 ${getSourceColorClass(line.source)}">
                      ${getSourceLabel(line.source)}
                    </span>
                    <span class="text-zinc-300 whitespace-pre-wrap break-all">${line.text}</span>
                  </div>
                `
              )
            )
          : html`<div class="text-xs text-zinc-600 italic">No output yet</div>`

      return html`
        <div class="fixed right-4 bottom-20 w-[600px] max-h-[400px] flex flex-col bg-zinc-950/95 border border-zinc-800/60 rounded-lg shadow-xl backdrop-blur-sm overflow-hidden">
          ${header}
          <div
            class="flex-1 p-3 overflow-y-auto space-y-1"
            id="tb-output-scroll"
            data-autoscroll="${state.autoScroll ? "true" : "false"}"
          >
            ${lines}
          </div>
          <div class="px-3 py-1 border-t border-zinc-800/40 bg-zinc-900/60 text-xs text-zinc-500">
            ${state.outputLines.length} lines
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      yield* ctx.dom.delegate(ctx.container, "[data-action]", "click", (_e, target) => {
        const action = (target as HTMLElement).dataset.action

        if (action === "clear") {
          Effect.runFork(ctx.emit({ type: "clear" }))
        } else if (action === "copy") {
          Effect.runFork(ctx.emit({ type: "copy" }))
        } else if (action === "close") {
          Effect.runFork(ctx.emit({ type: "close" }))
        } else if (action === "toggleAutoScroll") {
          Effect.runFork(ctx.emit({ type: "toggleAutoScroll" }))
        }
      })
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "clear":
          yield* ctx.state.update((s) => ({
            ...s,
            outputLines: [],
          }))
          break

        case "copy": {
          const state = yield* ctx.state.get
          const text = state.outputLines.map((l) => `[${l.source}] ${l.text}`).join("\n")
          if (typeof navigator !== "undefined" && navigator.clipboard) {
            try {
              yield* Effect.promise(() => navigator.clipboard.writeText(text))
            } catch {
              // Clipboard access may fail in some contexts
            }
          }
          break
        }

        case "close":
          yield* ctx.state.update((s) => ({ ...s, visible: false }))
          break

        case "open":
          yield* ctx.state.update((s) => ({ ...s, visible: true }))
          break

        case "toggleAutoScroll":
          yield* ctx.state.update((s) => ({ ...s, autoScroll: !s.autoScroll }))
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
                runId: msg.runId,
                taskId: null,
                outputLines: [],
                visible: true,
              }))
            }

            if (isTBTaskOutput(msg)) {
              yield* ctx.state.update((s) => {
                // Only accept output for current run
                if (s.runId && s.runId !== msg.runId) return s

                const newLines = [
                  ...s.outputLines,
                  {
                    text: msg.text,
                    source: msg.source,
                    timestamp: Date.now(),
                  },
                ].slice(-s.maxLines)

                return {
                  ...s,
                  taskId: msg.taskId,
                  outputLines: newLines,
                }
              })
            }

            if (isTBRunComplete(msg)) {
              yield* ctx.state.update((s) => {
                if (s.runId !== msg.runId) return s
                return {
                  ...s,
                  // Keep visible but mark run as complete
                  runId: null,
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

export const initialTBOutputState: TBOutputState = TBOutputWidget.initialState()
