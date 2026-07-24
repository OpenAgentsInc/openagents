/**
 * Seven-agents Part 2 (#9183): drive ONE real turn through an SDK
 * {@link AgentHarness} and lower its neutral stream onto the frozen
 * `ClaudeLocalEvent` renderer envelope — the exact host-run harness-lane path
 * HARN-09 (#9167) established for codex/claude, reused verbatim for the new
 * host-run harness lanes (Goose, OpenCode).
 *
 * Division of labor mirrors `codex-harness-attempt.ts`: the DESKTOP owns the
 * transport/binary selection, the interrupt seam, and the workspace; the
 * ADAPTER owns the neutral projection and session/turn lifecycle;
 * `harness-lowering` maps the neutral stream back onto the renderer envelope.
 * Exact usage is teed from the raw `turn.finished` event, never reconstructed
 * from the lowered subset.
 *
 * The runner is generic: a lane supplies a `prepareTurn` Effect that yields the
 * concrete adapter (with its live transport already bound) plus a `shutdown`
 * finalizer. The runner never spawns a process itself, so it stays trivially
 * testable with a scripted adapter.
 */

import type {
  AgentHarness,
  HarnessStartOptions,
} from "@openagentsinc/agent-harness-contract"
import { Cause, Effect, Fiber, Stream } from "effect"
import type { ClaudeChildUsage, ClaudeLocalEvent } from "./claude-local-contract.ts"
import { lowerHarnessEvent } from "./harness-lowering.ts"
import type {
  ProviderLaneHistoryMessage,
  ProviderLaneTurnResult,
} from "./provider-lane.ts"

export type HarnessSdkTurnInput = Readonly<{
  threadRef: string
  turnRef: string
  model: string
  history: ReadonlyArray<ProviderLaneHistoryMessage>
  message: string
  background: boolean
  emit: (event: ClaudeLocalEvent) => void
}>

/** The adapter plus its teardown, produced fresh per turn by the lane. */
export type HarnessTurnAdapter = Readonly<{
  adapter: AgentHarness
  /** Tear down the live transport/process the adapter was built over. */
  shutdown: () => Effect.Effect<void>
}>

export type HarnessSdkTurnDriverConfig = Readonly<{
  /** Event `source` labelling for the session's stream (lane / adapterKind). */
  source: HarnessStartOptions["source"]
  /**
   * Build the concrete adapter for this turn. It runs inside the turn's
   * interruptible scope, so a spawned transport is torn down on interrupt.
   */
  prepareTurn: (input: HarnessSdkTurnInput) => Effect.Effect<HarnessTurnAdapter, unknown>
  /** Bounded per-turn history → a single prompt preamble (host-owned). */
  composePrompt?: (input: HarnessSdkTurnInput) => string
}>

/** Map the neutral `turn.finished` usage onto the frozen five-field split. */
const toChildUsage = (usage: unknown): ClaudeChildUsage | null => {
  if (typeof usage !== "object" || usage === null) return null
  const u = usage as {
    inputTokens?: number
    outputTokens?: number
    reasoningTokens?: number
    cacheReadInputTokens?: number
    totalTokens?: number
  }
  if (typeof u.totalTokens !== "number") return null
  return {
    inputTokens: u.inputTokens ?? 0,
    cachedInputTokens: u.cacheReadInputTokens ?? 0,
    outputTokens: u.outputTokens ?? 0,
    reasoningTokens: u.reasoningTokens ?? 0,
    totalTokens: u.totalTokens,
  }
}

/** Default prompt composer: prepend a bounded plain-text history preamble. */
const defaultComposePrompt = (input: HarnessSdkTurnInput): string => {
  if (input.history.length === 0) return input.message
  const preamble = input.history
    .slice(-16)
    .map((message) => `${message.role}: ${message.text}`)
    .join("\n")
    .slice(0, 24_000)
  return `${preamble}\n\nuser: ${input.message}`
}

/**
 * A turn driver for one host-run SDK-harness lane. `runTurn` executes exactly
 * one turn through the adapter; `interrupt` aborts an in-flight turn by exact
 * `turnRef` (Effect fiber interruption cancels the stream and tears down the
 * transport through the prepared finalizer).
 */
export const makeHarnessSdkTurnDriver = (
  config: HarnessSdkTurnDriverConfig,
): Readonly<{
  runTurn: (input: HarnessSdkTurnInput) => Promise<ProviderLaneTurnResult>
  interrupt: (turnRef: string) => boolean
}> => {
  const active = new Map<string, Fiber.Fiber<ProviderLaneTurnResult, never>>()
  const composePrompt = config.composePrompt ?? defaultComposePrompt

  const runTurn = async (input: HarnessSdkTurnInput): Promise<ProviderLaneTurnResult> => {
    const prompt = composePrompt(input)

    // The turn body: acquire the adapter (spawns the live transport), run the
    // single turn, always tear down the transport. A per-turn mutable capture
    // for text + exact usage teed from the raw stream.
    const body: Effect.Effect<ProviderLaneTurnResult> = Effect.suspend(() => {
      let text = ""
      let usage: ClaudeChildUsage | null = null
      const succeed: Effect.Effect<ProviderLaneTurnResult, unknown> = Effect.acquireUseRelease(
        config.prepareTurn(input),
        (prepared) =>
          Effect.gen(function* () {
            const session = yield* prepared.adapter.start({
              sessionId: input.threadRef,
              source: config.source,
            })
            const control = yield* session.promptTurn({ turnId: input.turnRef, prompt })
            yield* Stream.runForEach(control.events, (event) =>
              Effect.sync(() => {
                if (event.kind === "turn.finished") {
                  usage = toChildUsage((event as { usage?: unknown }).usage)
                }
                for (const lowered of lowerHarnessEvent(event)) {
                  if (lowered.kind === "text_delta") text += lowered.text
                  input.emit(lowered)
                }
              }),
            )
            yield* control.done
            yield* session.stop()
            return {
              ok: true as const,
              text,
              totalTokens: usage?.totalTokens ?? null,
              ...(usage === null ? {} : { usage }),
            } satisfies ProviderLaneTurnResult
          }),
        (prepared) => prepared.shutdown(),
      )
      // Recover EVERY cause to a typed lane result (E = never): an interrupt
      // emits the interrupted envelope exactly once here; a start/transport/
      // timeout failure becomes a typed session failure. The dispatcher never
      // sees a raw provider error.
      return Effect.catchCause(succeed, (cause): Effect.Effect<ProviderLaneTurnResult> => {
        if (Cause.hasInterrupts(cause)) {
          input.emit({ kind: "turn_failed", reason: "interrupted", detail: "turn interrupted" })
          const interrupted: ProviderLaneTurnResult = {
            ok: false,
            reason: "interrupted",
            detail: "turn interrupted",
          }
          return Effect.succeed(interrupted)
        }
        const detail = Cause.pretty(cause).slice(0, 400)
        const failure: ProviderLaneTurnResult = {
          ok: false,
          reason: detail.toLowerCase().includes("timeout") ? "timeout" : "session_failed",
          detail,
        }
        return Effect.succeed(failure)
      })
    })

    const fiber = Effect.runFork(body)
    active.set(input.turnRef, fiber)
    try {
      return await Effect.runPromise(Fiber.join(fiber))
    } finally {
      active.delete(input.turnRef)
    }
  }

  const interrupt = (turnRef: string): boolean => {
    const fiber = active.get(turnRef)
    if (fiber === undefined) return false
    Effect.runFork(Fiber.interrupt(fiber))
    return true
  }

  return { runTurn, interrupt }
}
