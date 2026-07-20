import { Effect, Queue, Schema as S, Stream } from "effect"

import {
  CANDIDATE_SCHEMA_LITERAL,
  CandidateRef,
  InferenceProviderDescriptor,
  ProviderTurnRef,
  TurnCandidate,
  MAX_TURN_OUTPUT_CHARS,
  type TurnProviderRef,
} from "@openagentsinc/agent-runtime-schema"
import {
  ProviderRegistry,
  ProviderStartError,
  ProviderStreamEvent,
  type ProviderRegistryInterface,
  type ProviderRun,
  type ProviderStartInput,
} from "@openagentsinc/agent-turn-runtime"

import type { ClaudeLocalEvent, ClaudeLocalStartRequest } from "../claude-local-contract.ts"
import type {
  ProviderLane,
  ProviderLaneHistoryMessage,
  ProviderLaneTurnResult,
} from "../provider-lane.ts"

/**
 * AFS-01 Desktop transition adapter: `provider-lane.ts` -> kernel provider.
 *
 * The kernel owns policy, state, and projection. It does not own provider
 * execution, account selection, or the frozen Desktop event envelope. This
 * adapter wraps one existing `ProviderLane` value as one kernel inference
 * provider:
 *
 * - `admit` failure becomes a typed `ProviderStartError`; it never silently
 *   changes the effective lane.
 * - the lane's streamed `emit` events become bounded advisory `Progress` frames.
 * - the lane's terminal result becomes one advisory `Completed` (answer) or
 *   `Failed` event.
 *
 * It copies none of the dispatcher's mixed concerns (journal lifecycle, thread
 * history authority, usage ledger, checkpoints) into the core: those stay in the
 * host composition. It is Node/Electron-free and unit-testable with a fake lane.
 */

const decodeCandidate = S.decodeUnknownSync(TurnCandidate)
const decodeCandidateRef = S.decodeUnknownSync(CandidateRef)
const decodeProviderTurnRef = S.decodeUnknownSync(ProviderTurnRef)

/** A streamed lane event that represents forward progress, not a terminal. */
const isProgressEvent = (event: ClaudeLocalEvent): boolean =>
  event.kind !== "turn_completed" && event.kind !== "turn_failed"

/** The advisory prompt text for a turn intent. */
const intentMessage = (input: ProviderStartInput): string => {
  const intent = input.intent
  if (intent._tag === "Ask") return intent.text
  if (intent._tag === "ProposeEdit") return intent.instruction
  if (intent._tag === "RecommendRoute") return intent.objective
  // Anchor/ref intents carry no free text; the lane receives a neutral request.
  return "Continue the current turn."
}

const boundedAnswer = (text: string): string => {
  const trimmed = text.trim()
  if (trimmed === "") return "(empty answer)"
  return trimmed.slice(0, MAX_TURN_OUTPUT_CHARS)
}

export interface DesktopProviderLaneAdapterConfig<Ctx> {
  /** The wrapped Desktop provider lane value. */
  readonly lane: ProviderLane<Ctx>
  /** The frozen descriptor this lane publishes to the registry. */
  readonly descriptor: InferenceProviderDescriptor
  /** Build the exact Desktop start request from the kernel start input. */
  readonly buildRequest: (input: ProviderStartInput) => ClaudeLocalStartRequest
  /** Prior host-owned history; the renderer can never inject synthetic history. */
  readonly history?: ReadonlyArray<ProviderLaneHistoryMessage>
}

const answerCandidate = (
  descriptor: InferenceProviderDescriptor,
  requestRef: string,
  text: string,
): TurnCandidate =>
  decodeCandidate({
    schema: CANDIDATE_SCHEMA_LITERAL,
    kind: "answer",
    candidateRef: decodeCandidateRef(`candidate.${requestRef}`),
    provenance: {
      providerRef: descriptor.providerRef,
      candidate: descriptor.candidate,
      model: descriptor.model,
      taskClass: "local_answer",
      usageTruth: descriptor.usageTruth,
      dataDestination: descriptor.dataDestination,
      stale: false,
    },
    text: boundedAnswer(text),
  })

/**
 * Build a single-lane kernel provider registry from one Desktop lane. Compose
 * several with `makeDesktopProviderRegistry` when the host publishes more than
 * one lane.
 */
export const makeDesktopLaneProvider = <Ctx>(
  config: DesktopProviderLaneAdapterConfig<Ctx>,
): ProviderRegistryInterface => ({
  describe: Effect.succeed([config.descriptor]),
  start: (input: ProviderStartInput) =>
    Effect.gen(function* () {
      const request = config.buildRequest(input)
      const admission = config.lane.admit(request)
      if (!admission.ok) return yield* Effect.fail(new ProviderStartError({ reason: "unadmitted" }))

      const providerTurnRef = decodeProviderTurnRef(`providerturn.${input.requestRef}`)

      // The lane runs inside the stream's scoped callback fiber. Closing the turn
      // scope (cancellation) interrupts it; its terminal result ends the stream.
      const events = Stream.callback<ProviderStreamEvent>((queue) =>
        Effect.gen(function* () {
          const emit = (event: ClaudeLocalEvent): void => {
            if (!isProgressEvent(event)) return
            const progress: ProviderStreamEvent = ProviderStreamEvent.Progress()
            Queue.offerUnsafe(queue, progress)
          }
          const result: ProviderLaneTurnResult = yield* Effect.tryPromise(() =>
            config.lane.runTurn({
              request,
              model: admission.model,
              context: admission.context,
              history: config.history ?? [],
              message: intentMessage(input),
              background: true,
              emit,
            }),
          ).pipe(
            Effect.catch(() =>
              Effect.succeed<ProviderLaneTurnResult>({
                ok: false,
                reason: "session_failed",
                detail: "provider lane stopped",
              }),
            ),
          )
          const terminal: ProviderStreamEvent = result.ok
            ? ProviderStreamEvent.Completed({
                candidate: answerCandidate(config.descriptor, input.requestRef, result.text),
              })
            : ProviderStreamEvent.Failed({ detail: result.detail })
          Queue.offerUnsafe(queue, terminal)
          yield* Queue.end(queue)
        }),
      )

      const run: ProviderRun = { providerTurnRef, events }
      return run
    }),
})

/** One registry entry: the provider ref its adapter owns. */
export interface DesktopProviderRegistryEntry {
  readonly providerRef: TurnProviderRef
  readonly provider: ProviderRegistryInterface
}

/** Compose a registry that resolves the effective lane by its provider ref. */
export const makeDesktopProviderRegistry = (
  entries: ReadonlyArray<DesktopProviderRegistryEntry>,
): ProviderRegistryInterface => ({
  describe: Effect.forEach(entries, (entry) => entry.provider.describe).pipe(
    Effect.map((lists) => lists.flat()),
  ),
  start: (input) => {
    const entry = entries.find((candidate) => candidate.providerRef === input.providerRef)
    return entry === undefined
      ? Effect.fail(new ProviderStartError({ reason: "unavailable" }))
      : entry.provider.start(input)
  },
})

export { ProviderRegistry }
