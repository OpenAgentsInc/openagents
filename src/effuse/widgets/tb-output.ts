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
import type { HudMessage, ATIFStepMessage } from "../../hud/protocol.js"
import { isATIFStep } from "../../hud/protocol.js"

// ============================================================================
// Types
// ============================================================================

/** Output source type */
export type TBOutputSource = "agent" | "verification" | "system" | "tool"

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
  /** Show line numbers */
  showLineNumbers: boolean
  /** Selected line number */
  selectedLine: number | null
  /** Visible sources filter */
  visibleSources: Record<TBOutputSource, boolean>
}

/** Widget events */
export type TBOutputEvent =
  | { type: "clear" }
  | { type: "copy" }
  | { type: "close" }
  | { type: "open" }
  | { type: "toggleAutoScroll" }
  | { type: "toggleLineNumbers" }
  | { type: "selectLine"; lineNumber: number }
  | { type: "toggleSource"; source: TBOutputSource }

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
    case "tool":
      return "text-amber-300"
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
    case "tool":
      return "TL"
  }
}

/**
 * Format ATIF tool calls into displayable text
 */
const formatToolCall = (toolCall: ATIFStepMessage["step"]["tool_calls"][0]): string => {
  const args = toolCall.arguments
  let argsStr = ""
  if (args && typeof args === "object") {
    // Truncate long argument values for display
    const truncated = Object.entries(args as Record<string, unknown>)
      .slice(0, 3)
      .map(([k, v]) => {
        const val = typeof v === "string" && v.length > 50 ? v.slice(0, 47) + "..." : v
        return `${k}=${JSON.stringify(val)}`
      })
      .join(", ")
    argsStr = truncated + (Object.keys(args as object).length > 3 ? ", ..." : "")
  }
  return `→ ${toolCall.function_name}(${argsStr})`
}

/**
 * Format ATIF observation result into displayable text
 */
const formatObservationResult = (result: { source_call_id?: string; content?: unknown }): string => {
  const content = result.content
  if (typeof content === "string") {
    const truncated = content.length > 100 ? content.slice(0, 97) + "..." : content
    return `← ${truncated}`
  }
  if (content && typeof content === "object") {
    const str = JSON.stringify(content)
    return `← ${str.length > 100 ? str.slice(0, 97) + "..." : str}`
  }
  return `← (result)`
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
  isTBTaskOutput(msg) || isTBRunStart(msg) || isTBRunComplete(msg) || isATIFStep(msg)

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
    showLineNumbers: true,
    selectedLine: null,
    visibleSources: {
      agent: true,
      verification: true,
      system: true,
      tool: true,
    },
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
            <div class="flex items-center gap-1">
              ${(["agent", "verification", "system", "tool"] as const).map((source) => {
                const active = state.visibleSources[source]
                const label = getSourceLabel(source)
                const base = "text-xs px-2 py-1 rounded border transition-colors"
                const activeClasses = "bg-zinc-800/80 text-zinc-100 border-zinc-600/60"
                const inactiveClasses = "text-zinc-400 border-zinc-700/50 hover:border-zinc-600/60"
                return html`
                  <button
                    class="${base} ${active ? activeClasses : inactiveClasses}"
                    data-action="toggleSource"
                    data-source="${source}"
                    title="Toggle ${source} output"
                  >
                    ${label}
                  </button>
                `
              })}
            </div>
            <button
              class="text-xs px-2 py-1 rounded border transition-colors ${state.showLineNumbers
                ? "bg-zinc-800/80 text-zinc-100 border-zinc-600/60"
                : "text-zinc-400 border-zinc-700/50 hover:border-zinc-600/60"}"
              data-action="toggleLineNumbers"
              title="Toggle line numbers"
            >
              #
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
      const visibleLines = state.outputLines.filter((line) => state.visibleSources[line.source])
      const lines =
        visibleLines.length > 0
          ? joinTemplates(
              visibleLines.slice(-100).map((line, index, arr) => {
                const lineNumber = state.outputLines.length - arr.length + index + 1
                const lineNumberStyles = state.selectedLine === lineNumber
                  ? "bg-zinc-800/80 text-zinc-100 border-zinc-700/60"
                  : "text-zinc-500 border-transparent"

                return html`
                  <div class="flex gap-2 font-mono text-xs leading-relaxed">
                    ${state.showLineNumbers
                      ? html`
                          <button
                            class="w-10 text-right pr-2 rounded border ${lineNumberStyles}"
                            data-action="selectLine"
                            data-line="${lineNumber}"
                            title="Line ${lineNumber}"
                          >
                            ${lineNumber}
                          </button>
                        `
                      : ""}
                    <span class="w-8 flex-shrink-0 ${getSourceColorClass(line.source)}">
                      ${getSourceLabel(line.source)}
                    </span>
                    <span class="text-zinc-300 whitespace-pre-wrap break-all">${line.text}</span>
                  </div>
                `
              })
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
            ${visibleLines.length} lines
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
        } else if (action === "toggleLineNumbers") {
          Effect.runFork(ctx.emit({ type: "toggleLineNumbers" }))
        } else if (action === "selectLine") {
          const lineValue = (target as HTMLElement).dataset.line
          if (lineValue) {
            Effect.runFork(ctx.emit({ type: "selectLine", lineNumber: Number(lineValue) }))
          }
        } else if (action === "toggleSource") {
          const source = (target as HTMLElement).dataset.source as TBOutputSource | undefined
          if (source) {
            Effect.runFork(ctx.emit({ type: "toggleSource", source }))
          }
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
            selectedLine: null,
          }))
          break

        case "copy": {
          const state = yield* ctx.state.get
          const linesToCopy = state.outputLines.filter((line) => state.visibleSources[line.source])
          const total = state.outputLines.length
          const start = total - linesToCopy.length
          const text = linesToCopy
            .map((l, index) => {
              const lineNumber = start + index + 1
              const prefix = state.showLineNumbers ? `${lineNumber}: ` : ""
              return `${prefix}[${l.source}] ${l.text}`
            })
            .join("\n")
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

        case "toggleLineNumbers":
          yield* ctx.state.update((s) => ({
            ...s,
            showLineNumbers: !s.showLineNumbers,
            selectedLine: s.showLineNumbers ? null : s.selectedLine,
          }))
          break

        case "selectLine":
          yield* ctx.state.update((s) => ({
            ...s,
            selectedLine: s.selectedLine === event.lineNumber ? null : event.lineNumber,
          }))
          break

        case "toggleSource":
          yield* ctx.state.update((s) => ({
            ...s,
            visibleSources: {
              ...s.visibleSources,
              [event.source]: !s.visibleSources[event.source],
            },
            selectedLine: s.showLineNumbers ? s.selectedLine : null,
          }))
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
                selectedLine: null,
                visibleSources: {
                  agent: true,
                  verification: true,
                  system: true,
                  tool: true,
                },
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

            // Handle ATIF step messages with tool calls
            if (isATIFStep(msg)) {
              yield* ctx.state.update((s) => {
                // Only accept ATIF steps for current run
                if (s.runId && s.runId !== msg.runId) return s

                const newLines: TBOutputLine[] = [...s.outputLines]
                const timestamp = Date.now()

                // Add tool call lines
                if (msg.step.tool_calls && msg.step.tool_calls.length > 0) {
                  for (const toolCall of msg.step.tool_calls) {
                    newLines.push({
                      text: formatToolCall(toolCall),
                      source: "tool",
                      timestamp,
                    })
                  }
                }

                // Add observation/result lines
                if (msg.step.observation?.results && msg.step.observation.results.length > 0) {
                  for (const result of msg.step.observation.results) {
                    newLines.push({
                      text: formatObservationResult(result),
                      source: "tool",
                      timestamp,
                    })
                  }
                }

                return {
                  ...s,
                  outputLines: newLines.slice(-s.maxLines),
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
