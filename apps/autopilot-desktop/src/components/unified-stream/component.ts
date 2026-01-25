/**
 * Codex Unified Stream Component (Effuse)
 */

import { Effect, Queue, Schema, Scope, Stream, pipe } from "effect"
import { listen } from "@tauri-apps/api/event"
import {
  makeEzRegistry,
  mountEzRuntimeWith,
  DomServiceTag,
  type Component,
  type ComponentContext,
  type EzAction,
} from "../../effuse/index.js"
import {
  getCurrentDirectory,
} from "./api.js"
import { PROMPT_MESSAGE } from "./constants.js"
import { appendEvent } from "./feed.js"
import {
  applyMessageChunk,
  applyReasoningChunk,
  applyToolCallStart,
  applyToolCallUpdate,
  appendFormattedMessage,
  createFormattedState,
  finalizeStreaming,
  renderFormattedConversation,
  resetFormatted,
} from "./formatted.js"
import {
  setButtonEnabled,
  setButtonLabel,
  setPhase,
  setText,
  setUsageCreditsLabel,
  setUsagePercent,
  setUsageResetLabel,
  setUsageWeeklyVisible,
} from "./dom.js"
import { UnifiedEventSchema } from "../../contracts/tauri.js"
import type { RuntimeState, UnifiedEvent } from "./types.js"
import { renderUnifiedStreamView } from "./view.js"
import { AgentRegistry } from "../../agent/registry.js"
import type { AgentId } from "../../agent/types.js"

type UnifiedState = {}

type UsageState = {
  sessionPercent: number | null
  weeklyPercent: number | null
  sessionResetLabel: string | null
  weeklyResetLabel: string | null
  creditsLabel: string | null
  showWeekly: boolean
}

export const UnifiedStreamComponent: Component<
  UnifiedState,
  never,
  Scope.Scope
> = {
  id: "unified-stream",

  initialState: () => ({}),

  render: () => Effect.succeed(renderUnifiedStreamView(renderFormattedConversation([]))),

  setupEvents: (ctx: ComponentContext<UnifiedState, never>) =>
    Effect.gen(function* () {
      const state: RuntimeState = {
        phase: "connecting",
        workspacePath: null,
        workspaceId: null,
        sessionId: null,
        eventCount: 0,
        isProcessing: false,
      }

      const formatted = createFormattedState()
      const usage: UsageState = {
        sessionPercent: null,
        weeklyPercent: null,
        sessionResetLabel: null,
        weeklyResetLabel: null,
        creditsLabel: null,
        showWeekly: false,
      }

      const updateEventCount = (count: number) =>
        setText(ctx.container, "[data-role='events-count']", String(count))

      const formatResetLabel = (resetsAt: number | null | undefined): string | null => {
        if (typeof resetsAt !== "number" || !Number.isFinite(resetsAt)) {
          return null
        }
        const resetMs = resetsAt > 1_000_000_000_000 ? resetsAt : resetsAt * 1000
        const now = Date.now()
        const diffMs = resetMs - now
        if (diffMs < 0) {
          return "Resets soon"
        }
        const minutes = Math.floor(diffMs / 60000)
        const hours = Math.floor(minutes / 60)
        const days = Math.floor(hours / 24)
        if (days > 0) {
          return `Resets in ${days}d`
        }
        if (hours > 0) {
          return `Resets in ${hours}h`
        }
        if (minutes > 0) {
          return `Resets in ${minutes}m`
        }
        return "Resets soon"
      }

      const updateUsage = () =>
        Effect.gen(function* () {
          yield* setUsagePercent(ctx.container, "session", usage.sessionPercent)
          yield* setUsageResetLabel(
            ctx.container,
            "session",
            usage.sessionResetLabel
          )
          yield* setUsagePercent(ctx.container, "weekly", usage.weeklyPercent)
          yield* setUsageResetLabel(
            ctx.container,
            "weekly",
            usage.weeklyResetLabel
          )
          yield* setUsageWeeklyVisible(ctx.container, usage.showWeekly)
          yield* setUsageCreditsLabel(ctx.container, usage.creditsLabel)
        })

      const resetUsage = () => {
        usage.sessionPercent = null
        usage.weeklyPercent = null
        usage.sessionResetLabel = null
        usage.weeklyResetLabel = null
        usage.creditsLabel = null
        usage.showWeekly = false
      }

      const renderFormattedFeed = () =>
        Effect.gen(function* () {
          const container = ctx.container.querySelector(
            "[data-role='formatted-feed']"
          )
          if (!container) {
            return
          }

          const content = renderFormattedConversation(formatted.items)
          yield* ctx.dom.swap(container, content, "inner").pipe(
            Effect.catchAll(() => Effect.void)
          )
        })

      const handleUnifiedEvent = (event: UnifiedEvent) =>
        Effect.gen(function* () {
          state.eventCount += 1
          yield* updateEventCount(state.eventCount)
          yield* appendEvent(ctx.dom, ctx.container, event, state.eventCount)

          let shouldRenderFormatted = false

          switch (event.type) {
            case "SessionStarted":
              if (state.sessionId !== event.session_id) {
                resetFormatted(formatted)
                resetUsage()
                shouldRenderFormatted = true
                yield* updateUsage()
              }
              state.sessionId = event.session_id
              yield* setText(
                ctx.container,
                "[data-role='session-id']",
                event.session_id
              )
              yield* setPhase(ctx.container, state, "ready")
              yield* setButtonEnabled(ctx.container, true)
              break
            case "MessageChunk":
              applyMessageChunk(formatted, event.content, event.is_complete)
              shouldRenderFormatted = true
              break
            case "ThoughtChunk":
              applyReasoningChunk(formatted, event.content, event.is_complete)
              shouldRenderFormatted = true
              break
            case "SessionCompleted":
              state.isProcessing = false
              finalizeStreaming(formatted)
              yield* setPhase(ctx.container, state, "ready")
              yield* setButtonLabel(ctx.container, "Send prompt")
              yield* setButtonEnabled(ctx.container, true)
              shouldRenderFormatted = true
              break
            case "ToolCall": {
              const input =
                event.arguments && Object.keys(event.arguments).length > 0
                  ? JSON.stringify(event.arguments, null, 2)
                  : ""
              applyToolCallStart(formatted, event.tool_id, event.tool_name, input)
              shouldRenderFormatted = true
              break
            }
            case "ToolCallUpdate":
              applyToolCallUpdate(
                formatted,
                event.tool_id,
                event.output,
                event.is_complete
              )
              shouldRenderFormatted = true
              break
            case "RateLimitUpdate": {
              const percent = Math.min(
                Math.max(Math.round(event.used_percent), 0),
                100
              )
              usage.sessionPercent = percent
              usage.sessionResetLabel = formatResetLabel(event.resets_at)
              usage.showWeekly = usage.weeklyPercent !== null
              yield* updateUsage()
              break
            }
            case "TokenUsage": {
              const input = event.input_tokens.toLocaleString()
              const output = event.output_tokens.toLocaleString()
              const total = event.total_tokens.toLocaleString()
              usage.creditsLabel = `Tokens: ${input} in · ${output} out · ${total} total`
              yield* updateUsage()
              break
            }
          }

          if (shouldRenderFormatted) {
            yield* renderFormattedFeed()
          }
        })

      const setupUnifiedListener = Effect.gen(function* () {
        const eventQueue = yield* Effect.acquireRelease(
          Queue.unbounded<UnifiedEvent>(),
          (queue) => Queue.shutdown(queue)
        )

        yield* pipe(
          Stream.fromQueue(eventQueue),
          Stream.tap((event) => handleUnifiedEvent(event)),
          Stream.runDrain,
          Effect.forkScoped
        )

        const unlisten = yield* Effect.tryPromise({
          try: () =>
            listen<unknown>("unified-event", (event) => {
              Effect.runFork(
                Schema.decodeUnknown(UnifiedEventSchema)(event.payload).pipe(
                  Effect.flatMap((decoded) => Queue.offer(eventQueue, decoded)),
                  Effect.catchAll(() => Effect.void)
                )
              )
            }),
          catch: (error) => new Error(String(error)),
        })

        yield* Effect.addFinalizer(() => Effect.sync(() => unlisten()))
      }).pipe(
        Effect.catchAll((error) =>
          setPhase(ctx.container, state, "error", error.message)
        )
      )

      const connectUnified = (agentId: AgentId = "Adjutant", attemptedFallback = false) => Effect.gen(function* () {
        yield* setPhase(ctx.container, state, "connecting")

        const workspacePath = yield* getCurrentDirectory()
        const workspaceId = `workspace-${Date.now()}`
        state.workspacePath = workspacePath
        state.workspaceId = workspaceId
        yield* setText(ctx.container, "[data-role='workspace-path']", workspacePath)

        const agent = AgentRegistry.getAgent(agentId)
        if (!agent) {
          throw new Error(`Unknown agent: ${agentId}`)
        }

        const connectResult = yield* agent.connect(workspacePath, workspaceId)

        if (!connectResult.success) {
          throw new Error(
            `Unified agent connection failed: ${JSON.stringify(connectResult)}`
          )
        }

        state.sessionId = connectResult.sessionId
        yield* agent.startSession(connectResult.sessionId, workspacePath)

        yield* setPhase(ctx.container, state, "connected")
      }).pipe(
        Effect.catchAll((error) => {
          const message = error.message ?? String(error)
          if (
            agentId === "Codex" &&
            !attemptedFallback &&
            message.toLowerCase().includes("codex-acp not found")
          ) {
            return Effect.gen(function* () {
              const selector = ctx.container.querySelector<HTMLSelectElement>(
                "[data-role='agent-selector']"
              )
              if (selector) {
                selector.value = "Adjutant"
              }
              yield* setPhase(
                ctx.container,
                state,
                "connecting",
                "Codex unavailable; switched to Adjutant."
              )
              return yield* connectUnified("Adjutant", true)
            })
          }
          return setPhase(ctx.container, state, "error", message)
        })
      )

      const sendPrompt: EzAction = () =>
        Effect.gen(function* () {
          if (!state.sessionId || state.isProcessing) {
            return
          }

          const selector = ctx.container.querySelector<HTMLSelectElement>("[data-role='agent-selector']")
          const agentId = (selector?.value as AgentId) || "Adjutant"
          const agent = AgentRegistry.getAgent(agentId)
          
          if (!agent) {
            return
          }

          appendFormattedMessage(formatted, "user", PROMPT_MESSAGE, false)
          yield* renderFormattedFeed()

          state.isProcessing = true
          yield* setButtonEnabled(ctx.container, false)
          yield* setText(
            ctx.container,
            "[data-role='status-text']",
            "Awaiting response"
          )

          const sent = yield* agent.sendMessage(state.sessionId, PROMPT_MESSAGE).pipe(
            Effect.as(true),
            Effect.catchAll((error) =>
              Effect.gen(function* () {
                state.isProcessing = false
                yield* setPhase(ctx.container, state, "error", error.message)
                yield* setButtonLabel(ctx.container, "Send prompt")
                yield* setButtonEnabled(ctx.container, true)
                return false
              })
            )
          )

          if (!sent) {
            return
          }

          return undefined
        })

      const registry = makeEzRegistry([["unified.send", sendPrompt]])

      const setupAgentSelector = Effect.sync(() => {
        const selector = ctx.container.querySelector<HTMLSelectElement>("[data-role='agent-selector']")
        if (selector) {
          selector.addEventListener("change", () => {
            const agentId = selector.value as AgentId
            Effect.runFork(connectUnified(agentId))
          })
        }
      })

      yield* mountEzRuntimeWith(ctx.container, registry).pipe(
        Effect.provideService(DomServiceTag, ctx.dom),
        Effect.catchAll(() => Effect.void)
      )
      yield* renderFormattedFeed()
      yield* setupUnifiedListener
      yield* setupAgentSelector
      yield* Effect.forkScoped(connectUnified())
    }),
}
