/**
 * Seven-agents Part 2 (#9183): the `ProviderLane` shell for a HOST-RUN
 * SDK-harness lane (Goose, OpenCode) — the built-in-harness-lane family
 * (#9167), distinct from the ACP trusted-peer-profile family (Grok, Cursor)
 * and from the two native transports (codex-local, claude-local).
 *
 * It is the harness-contract sibling of `makeAcpProviderLane`: a plain
 * `ProviderLane<null>` value whose `runTurn` folds a driver that ALREADY emits
 * the frozen `ClaudeLocalEvent` envelope (the driver lowers the SDK harness
 * stream through `harness-lowering`). The dispatcher owns everything else —
 * journal, graph, checkpoints, renderer forward — exactly as it does for the
 * ACP lanes; this lane invents no private dispatch or renderer projection.
 */

import { type ClaudeLocalEvent } from "./claude-local-contract.ts"
import {
  type ProviderLane,
  type ProviderLaneCapabilityReport,
  type ProviderLaneHistoryMessage,
  type ProviderLaneTurnResult,
} from "./provider-lane.ts"

const summary = (value: string): string => value.slice(0, 400)

/** The turn seam a host-run harness lane provides (emits the frozen envelope). */
export type HarnessProviderLaneDriver = Readonly<{
  runTurn: (input: Readonly<{
    threadRef: string
    turnRef: string
    model: string
    history: ReadonlyArray<ProviderLaneHistoryMessage>
    message: string
    background: boolean
    emit: (event: ClaudeLocalEvent) => void
  }>) => Promise<ProviderLaneTurnResult>
  interrupt: (turnRef: string) => boolean
}>

/**
 * Build a host-run harness `ProviderLane<null>`. Goose and OpenCode instantiate
 * this with different truthful capability reports and drivers; neither gets a
 * private dispatcher or renderer projection.
 */
export const makeHarnessProviderLane = (input: Readonly<{
  laneRef: string
  graphLaneRef: string
  eventChannel: string
  capabilities: ProviderLaneCapabilityReport
  driver: HarnessProviderLaneDriver
}>): ProviderLane<null> => ({
  laneRef: input.laneRef,
  graphLaneRef: input.graphLaneRef,
  eventChannel: input.eventChannel,
  usageProvider: input.capabilities.provider,
  capabilities: () => input.capabilities,
  admit: request => {
    if (request.target !== undefined) {
      return { ok: false, error: "That built-in provider target is not available on this harness lane." }
    }
    if (request.skill !== undefined) {
      return { ok: false, error: "Local Claude skills are not available on this harness lane." }
    }
    if (request.permissionMode === "plan_only" && !input.capabilities.features.planOnly) {
      return { ok: false, error: "Plan-only permission mode is not available on this harness lane." }
    }
    const model = request.model !== undefined && input.capabilities.models.includes(request.model)
      ? request.model
      : input.capabilities.models[0]
    return model === undefined
      ? { ok: false, error: "This harness lane has no available model." }
      : { ok: true, model, context: null }
  },
  streamMeta: ctx => ({
    lane: input.laneRef,
    turnRef: ctx.request.turnRef,
    model: ctx.effectiveModel() ?? ctx.requestedModel,
  }),
  modelNoteText: model => `${input.capabilities.composer.displayName} · ${model}`,
  runTurn: ({ request, model, history, message, background, emit }) =>
    input.driver.runTurn({
      threadRef: request.threadRef,
      turnRef: request.turnRef,
      model,
      history,
      message,
      background,
      emit,
    }),
  interrupt: turnRef => input.driver.interrupt(turnRef),
  finalMeta: ctx => ({
    lane: input.laneRef,
    turnRef: ctx.request.turnRef,
    model: ctx.effectiveModel() ?? ctx.requestedModel,
    ...(ctx.result.accountRef === undefined ? {} : { accountRef: ctx.result.accountRef }),
    ...(ctx.result.providerSessionRef === undefined || ctx.result.providerSessionRef === null
      ? {}
      : { requestId: ctx.result.providerSessionRef }),
    totalTokens: ctx.result.totalTokens,
    durationMs: ctx.durationMs,
  }),
  failureMessage: (reason, detail) =>
    `The ${input.capabilities.composer.displayName} turn failed (${reason}${detail === "" ? "" : ` · ${summary(detail)}`}).`,
})
