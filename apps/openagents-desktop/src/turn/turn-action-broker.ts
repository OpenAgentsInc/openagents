import { Context, Effect, Layer, Ref } from "effect"

import {
  turnIntentTaskClass,
  type CandidateRef,
  type DebugRef,
  type ProposalCandidate,
  type RunRef,
  type SourceControlRef,
  type TurnTaskClass,
} from "@openagentsinc/agent-runtime-schema"
import {
  ActionBroker,
  type ActionBrokerDelivery,
} from "@openagentsinc/agent-turn-runtime"

/**
 * AFS-06 turn-to-action broker.
 *
 * The shared turn kernel folds an advisory provider stream into ONE terminal
 * candidate and hands it to the injected `ActionBroker`. This module is the
 * Desktop broker: it converts that advisory delivery into a typed action
 * REQUEST for the owning host service and records the backlink. It performs no
 * effect itself.
 *
 * The Action invariant (plan section "Action invariant"): inference output is
 * advisory. A file mutation must become an IDE-08 proposal. A task must use
 * IDE-10. Debug work must use IDE-11. Source-control and delivery work must use
 * IDE-12. This broker honors that invariant structurally:
 *
 * 1. Routing is derived from the HOST-owned intent, never from model output. A
 *    model result cannot redirect the action class by declaring a different task
 *    class: a candidate whose declared task class disagrees with the host intent
 *    is refused to `AdvisoryOnly`.
 * 2. The typed action refs (proposal ref, run ref, debug ref, source-control
 *    ref) come from host-owned sources — the IDE-08-minted proposal ref on a
 *    proposal candidate, and the run/debug/source-control refs on the intent.
 *    Model TEXT only ever fills an advisory `instruction`, `explanation`, or
 *    `draftMessage` field. Model text never becomes a command argument.
 * 3. The request union has NO apply, accept, run, stage, commit, push, or
 *    deliver variant. Run, debug control, stage, commit, push, and delivery each
 *    require a separate existing owner command. The broker only produces a
 *    proposal (Pending, owner-reviewed), a read-only run/debug context request,
 *    or a source-control draft field.
 *
 * A real file change already went through IDE-08 before the candidate reached
 * the kernel: the cursor/IDE-08 service minted the durable proposal with exact
 * preimages and hash/generation checks and returned a `ProposalCandidate` that
 * references it by `proposalRef`. The broker records the backlink from that
 * candidate to the IDE-08 proposal; it never writes a file or re-mints a
 * proposal, and it never converts an advisory answer into a file change.
 */

/** Why an advisory delivery produced no typed action. */
export type TurnAdvisoryReason =
  | "no_action_class"
  | "no_proposal_ref"
  | "task_class_mismatch"
  | "empty_advisory_text"

/**
 * The typed action request the broker routes to the owning IDE service. Each
 * non-advisory variant carries only host-owned typed refs plus one advisory
 * text field. There is deliberately no apply/accept/run/commit/push variant.
 */
export type TurnActionRequest =
  | {
      readonly _tag: "ProposalRequest"
      readonly candidateRef: CandidateRef
      readonly proposalRef: ProposalCandidate["proposalRef"]
      readonly instruction: string
      readonly threadRef: ActionBrokerDelivery["threadRef"]
      readonly requestRef: ActionBrokerDelivery["requestRef"]
    }
  | {
      readonly _tag: "RunEvidenceRequest"
      readonly candidateRef: CandidateRef
      readonly runRef: RunRef
      readonly explanation: string
      readonly threadRef: ActionBrokerDelivery["threadRef"]
      readonly requestRef: ActionBrokerDelivery["requestRef"]
    }
  | {
      readonly _tag: "DebugContextRequest"
      readonly candidateRef: CandidateRef
      readonly debugRef: DebugRef
      readonly explanation: string
      readonly threadRef: ActionBrokerDelivery["threadRef"]
      readonly requestRef: ActionBrokerDelivery["requestRef"]
    }
  | {
      readonly _tag: "CommitMessageDraft"
      readonly candidateRef: CandidateRef
      readonly sourceControlRef: SourceControlRef
      readonly draftMessage: string
      readonly threadRef: ActionBrokerDelivery["threadRef"]
      readonly requestRef: ActionBrokerDelivery["requestRef"]
    }
  | {
      readonly _tag: "AdvisoryOnly"
      readonly candidateRef: CandidateRef
      readonly taskClass: TurnTaskClass
      readonly reason: TurnAdvisoryReason
    }

/**
 * Derive the typed action request from an advisory delivery. Pure and total: the
 * routing decision is host-owned and every fail-safe path resolves to
 * `AdvisoryOnly`, so an inference result can never mint an action, evidence, or
 * acceptance.
 */
export const deriveTurnActionRequest = (
  delivery: ActionBrokerDelivery,
): TurnActionRequest => {
  const { candidate, intent, threadRef, requestRef } = delivery
  const candidateRef = candidate.candidateRef
  const intentTaskClass = turnIntentTaskClass[intent._tag]

  // A model result cannot redirect the action class: the candidate's declared
  // task class must agree with the host intent's task class.
  if (candidate.provenance.taskClass !== intentTaskClass) {
    return {
      _tag: "AdvisoryOnly",
      candidateRef,
      taskClass: candidate.provenance.taskClass,
      reason: "task_class_mismatch",
    }
  }

  const answerText = candidate.kind === "answer" ? candidate.text : null

  switch (intent._tag) {
    case "ProposeEdit": {
      // Only a genuine proposal candidate carries the IDE-08-minted proposal
      // ref. An advisory answer that claims an edit cannot mint a proposal.
      if (candidate.kind !== "proposal") {
        return { _tag: "AdvisoryOnly", candidateRef, taskClass: intentTaskClass, reason: "no_proposal_ref" }
      }
      return {
        _tag: "ProposalRequest",
        candidateRef,
        proposalRef: candidate.proposalRef,
        instruction: candidate.instruction,
        threadRef,
        requestRef,
      }
    }
    case "ExplainFailure": {
      if (answerText === null || answerText.length === 0) {
        return { _tag: "AdvisoryOnly", candidateRef, taskClass: intentTaskClass, reason: "empty_advisory_text" }
      }
      return {
        _tag: "RunEvidenceRequest",
        candidateRef,
        runRef: intent.runRef,
        explanation: answerText,
        threadRef,
        requestRef,
      }
    }
    case "ExplainDebug": {
      if (answerText === null || answerText.length === 0) {
        return { _tag: "AdvisoryOnly", candidateRef, taskClass: intentTaskClass, reason: "empty_advisory_text" }
      }
      return {
        _tag: "DebugContextRequest",
        candidateRef,
        debugRef: intent.debugRef,
        explanation: answerText,
        threadRef,
        requestRef,
      }
    }
    case "DraftCommitMessage": {
      if (answerText === null || answerText.length === 0) {
        return { _tag: "AdvisoryOnly", candidateRef, taskClass: intentTaskClass, reason: "empty_advisory_text" }
      }
      return {
        _tag: "CommitMessageDraft",
        candidateRef,
        sourceControlRef: intent.sourceControlRef,
        draftMessage: answerText,
        threadRef,
        requestRef,
      }
    }
    // Ask, Complete, NextEdit, and RecommendRoute stay advisory. They start no
    // action: a chat answer, a completion, a next-edit hint, and a route
    // recommendation carry no action authority.
    default:
      return { _tag: "AdvisoryOnly", candidateRef, taskClass: intentTaskClass, reason: "no_action_class" }
  }
}

/** The host-owned action-result reference recorded back for the turn. */
export type TurnActionResultRef = `ide.turn-action.${string}`

/** A recorded routing result: the request plus the backlink recorded on the turn. */
export interface TurnActionRecord {
  readonly request: TurnActionRequest
  readonly resultRef: TurnActionResultRef | null
}

/** Deterministic host-owned backlink ref for a routed request. */
const resultRefFor = (request: TurnActionRequest): TurnActionResultRef | null => {
  switch (request._tag) {
    case "ProposalRequest":
      return `ide.turn-action.ide08.${request.requestRef}.${request.proposalRef}`
    case "RunEvidenceRequest":
      return `ide.turn-action.ide10.${request.requestRef}.${request.runRef}`
    case "DebugContextRequest":
      return `ide.turn-action.ide11.${request.requestRef}.${request.debugRef}`
    case "CommitMessageDraft":
      return `ide.turn-action.ide12.${request.requestRef}.${request.sourceControlRef}`
    case "AdvisoryOnly":
      return null
  }
}

/**
 * `TurnActionSink` is the narrow port the broker routes typed requests to. The
 * Desktop host provides the concrete sink. The default in-memory sink records
 * the backlink; the real IDE-08/10/11/12 services plug into the SAME port to
 * submit the proposal, attach read-only run/debug context, or set the
 * source-control draft field. The port exposes no apply/accept/run/commit/push
 * method: those stay owner commands on the existing IDE services.
 */
export interface TurnActionSinkShape {
  readonly route: (request: TurnActionRequest) => Effect.Effect<TurnActionRecord>
}

export class TurnActionSink extends Context.Service<TurnActionSink, TurnActionSinkShape>()(
  "@openagentsinc/openagents-desktop/TurnActionSink",
) {}

/**
 * The default record-only sink. It converts every non-advisory request into a
 * host-owned backlink and retains the ordered action ledger. It executes
 * nothing. This is the current-revision-safe wiring: the advisory result becomes
 * the typed request the IDE service consumes, and the backlink is recorded,
 * without run/debug/Git/file execution.
 */
export const makeInMemoryTurnActionSink: Effect.Effect<{
  readonly sink: TurnActionSinkShape
  readonly recorded: Effect.Effect<ReadonlyArray<TurnActionRecord>>
}> = Effect.gen(function* () {
  const ledger = yield* Ref.make<ReadonlyArray<TurnActionRecord>>([])
  const route = Effect.fn("TurnActionSink.route")(function* (request: TurnActionRequest) {
    const record: TurnActionRecord = { request, resultRef: resultRefFor(request) }
    yield* Ref.update(ledger, (all) => [...all, record].slice(-256))
    return record
  })
  return { sink: TurnActionSink.of({ route }), recorded: Ref.get(ledger) } as const
})

/** The default in-memory sink layer for the Desktop composition. */
export const inMemoryTurnActionSinkLayer: Layer.Layer<TurnActionSink> = Layer.effect(
  TurnActionSink,
  Effect.map(makeInMemoryTurnActionSink, (built) => built.sink),
)

/**
 * The Desktop `ActionBroker`: derive the typed request from the advisory
 * delivery and route it to the injected `TurnActionSink`. Nothing here executes
 * an IDE action.
 */
export const makeDesktopActionBrokerLayer: Layer.Layer<ActionBroker, never, TurnActionSink> =
  Layer.effect(
    ActionBroker,
    Effect.gen(function* () {
      const sink = yield* TurnActionSink
      const deliver = Effect.fn("DesktopActionBroker.deliver")(function* (
        delivery: ActionBrokerDelivery,
      ) {
        const request = deriveTurnActionRequest(delivery)
        yield* sink.route(request)
      })
      return ActionBroker.of({ deliver })
    }),
  )

/**
 * The drop-in Desktop broker layer over the default in-memory action sink. This
 * replaces the AFS-01 no-op broker in the first production composition.
 */
export const desktopActionBrokerLayer: Layer.Layer<ActionBroker> =
  makeDesktopActionBrokerLayer.pipe(Layer.provide(inMemoryTurnActionSinkLayer))
