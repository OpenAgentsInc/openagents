/**
 * Deterministic dispatch milestone signals for the Khala tool dispatcher
 * (openagents issue #8782).
 *
 * These are pipeline signals — test/orchestration synchronization events on a
 * typed Effect PubSub — NOT the user-facing evidence "receipts" vocabulary
 * used by Blueprint/Cloud. Never surface one as user-facing evidence.
 */
import { Effect, Schema as S } from "effect"
import type { Scope } from "effect"
import {
  makeDrainableWorker,
  makePipelineSignalBus,
  type PipelineSignalBus,
} from "@openagentsinc/pipeline-signals"
import { makeKhalaToolDispatcher, type KhalaToolDispatcherOptions, type KhalaToolDispatchInput } from "./dispatcher.js"

const MilestoneBase = {
  invocationId: S.String,
  toolName: S.String,
} as const

/** The dispatcher recorded one tool call against the turn's call budget. */
export const KhalaDispatchTurnCallRecordedSchema = S.Struct({
  ...MilestoneBase,
  kind: S.Literal("khala.dispatch.turn_call_recorded"),
  turnId: S.String,
  toolCallCount: S.Number,
  maxToolCalls: S.optional(S.Number),
})

/** The turn's tool-call budget is exhausted; the dispatch was refused. */
export const KhalaDispatchTurnBudgetExhaustedSchema = S.Struct({
  ...MilestoneBase,
  kind: S.Literal("khala.dispatch.turn_budget_exhausted"),
  turnId: S.String,
  toolCallCount: S.Number,
  maxToolCalls: S.Number,
})

/**
 * Bounded-output finalization: the model output exceeded the byte budget and
 * was truncated, with the full output preserved in a private artifact.
 */
export const KhalaDispatchOutputBoundedSchema = S.Struct({
  ...MilestoneBase,
  kind: S.Literal("khala.dispatch.output_bounded"),
  artifactRef: S.String,
  modelOutputBytes: S.Number,
  maxModelOutputBytes: S.Number,
})

/** One dispatch settled (hooks ran, result finalized), in any terminal phase. */
export const KhalaDispatchSettledSchema = S.Struct({
  ...MilestoneBase,
  kind: S.Literal("khala.dispatch.settled"),
  phase: S.Literals(["resolve", "validate", "permission", "execute", "bound_output", "completed", "failed"]),
  status: S.Literals(["ok", "failed", "denied", "needs_input", "unavailable"]),
})

export const KhalaDispatchMilestoneSchema = S.Union([
  KhalaDispatchTurnCallRecordedSchema,
  KhalaDispatchTurnBudgetExhaustedSchema,
  KhalaDispatchOutputBoundedSchema,
  KhalaDispatchSettledSchema,
])
export type KhalaDispatchMilestone = typeof KhalaDispatchMilestoneSchema.Type

export type KhalaDispatchSignalBus = PipelineSignalBus<KhalaDispatchMilestone>

/** Create a typed milestone bus for dispatcher pipelines. */
export const makeKhalaDispatchSignalBus: Effect.Effect<KhalaDispatchSignalBus> =
  makePipelineSignalBus<KhalaDispatchMilestone>()

export const isKhalaDispatchSettled = (
  signal: KhalaDispatchMilestone,
): signal is typeof KhalaDispatchSettledSchema.Type => signal.kind === "khala.dispatch.settled"

export const isKhalaDispatchOutputBounded = (
  signal: KhalaDispatchMilestone,
): signal is typeof KhalaDispatchOutputBoundedSchema.Type => signal.kind === "khala.dispatch.output_bounded"

export const isKhalaDispatchTurnBudgetExhausted = (
  signal: KhalaDispatchMilestone,
): signal is typeof KhalaDispatchTurnBudgetExhaustedSchema.Type =>
  signal.kind === "khala.dispatch.turn_budget_exhausted"

/**
 * A queue-backed dispatcher built on the shared DrainableWorker primitive.
 *
 * `enqueue` accepts dispatch inputs; `drain` settles when every accepted
 * dispatch has fully finalized. Outcomes are observable as
 * `khala.dispatch.settled` milestones on the dispatcher's signal bus, so
 * orchestration and tests await typed signals instead of polling state.
 */
export interface QueuedKhalaToolDispatcher {
  readonly enqueue: (input: KhalaToolDispatchInput) => Effect.Effect<boolean>
  readonly drain: Effect.Effect<void>
}

export const makeQueuedKhalaToolDispatcher = (
  options: KhalaToolDispatcherOptions = {},
): Effect.Effect<QueuedKhalaToolDispatcher, never, Scope.Scope> =>
  Effect.gen(function* () {
    const dispatcher = makeKhalaToolDispatcher(options)
    const worker = yield* makeDrainableWorker((input: KhalaToolDispatchInput) =>
      dispatcher.dispatch(input).pipe(Effect.asVoid),
    )
    return { drain: worker.drain, enqueue: worker.enqueue } satisfies QueuedKhalaToolDispatcher
  })
