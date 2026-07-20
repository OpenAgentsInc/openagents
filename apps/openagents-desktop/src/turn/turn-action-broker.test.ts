import { Effect, Layer, Ref, Schema as S } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  AnswerCandidate,
  ProposalCandidate,
  TurnIntent,
  TurnRequestRef,
  TurnThreadRef,
  type TurnCandidate,
  type TurnTaskClass,
} from "@openagentsinc/agent-runtime-schema"
import { ActionBroker, type ActionBrokerDelivery } from "@openagentsinc/agent-turn-runtime"

import {
  deriveTurnActionRequest,
  makeDesktopActionBrokerLayer,
  TurnActionSink,
  type TurnActionRecord,
  type TurnActionRequest,
} from "./turn-action-broker.ts"

const decodeIntent = S.decodeUnknownSync(TurnIntent)
const decodeAnswer = S.decodeUnknownSync(AnswerCandidate)
const decodeProposal = S.decodeUnknownSync(ProposalCandidate)
const threadRef = S.decodeUnknownSync(TurnThreadRef)("thread.1")
const requestRef = S.decodeUnknownSync(TurnRequestRef)("request.1")

const provenance = (taskClass: TurnTaskClass) => ({
  providerRef: "provider.apple_fm.1",
  candidate: "apple_fm" as const,
  model: "apple-foundation-model",
  taskClass,
  usageTruth: "estimated" as const,
  dataDestination: "on_device_local" as const,
  stale: false,
})

const answerCandidate = (taskClass: TurnTaskClass, text: string): TurnCandidate =>
  decodeAnswer({
    schema: "openagents.agent_turn_candidate.v1",
    kind: "answer",
    candidateRef: "candidate.1",
    provenance: provenance(taskClass),
    text,
  })

const proposalCandidate = (): TurnCandidate =>
  decodeProposal({
    schema: "openagents.agent_turn_candidate.v1",
    kind: "proposal",
    candidateRef: "candidate.1",
    provenance: provenance("propose_edit"),
    instruction: "rename the helper and fix its call sites",
    proposalRef: "proposal.ide08.1",
  })

const delivery = (candidate: TurnCandidate, intent: TurnIntent): ActionBrokerDelivery => ({
  candidate,
  intent,
  threadRef,
  requestRef,
})

describe("deriveTurnActionRequest — advisory result to typed action request", () => {
  test("a chat answer is advisory only and starts no action", () => {
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("local_answer", "Paris is the capital of France."), decodeIntent({ _tag: "Ask", text: "capital?" })),
    )
    expect(request._tag).toBe("AdvisoryOnly")
    if (request._tag === "AdvisoryOnly") expect(request.reason).toBe("no_action_class")
  })

  test("a propose-edit proposal candidate routes to an IDE-08 proposal request by its host-minted proposal ref", () => {
    const request = deriveTurnActionRequest(
      delivery(proposalCandidate(), decodeIntent({ _tag: "ProposeEdit", instruction: "rename the helper" })),
    )
    expect(request._tag).toBe("ProposalRequest")
    if (request._tag === "ProposalRequest") {
      expect(request.proposalRef).toBe("proposal.ide08.1")
      expect(request.instruction).toBe("rename the helper and fix its call sites")
    }
  })

  test("an answer that claims a propose-edit cannot mint a proposal (no proposal ref)", () => {
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("propose_edit", "I edited the file for you."), decodeIntent({ _tag: "ProposeEdit", instruction: "edit it" })),
    )
    expect(request._tag).toBe("AdvisoryOnly")
    if (request._tag === "AdvisoryOnly") expect(request.reason).toBe("no_proposal_ref")
  })

  test("an explain-failure answer becomes a read-only IDE-10 run-evidence request bound to the intent run ref", () => {
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("explain_failure", "The test failed because the fixture is stale."), decodeIntent({ _tag: "ExplainFailure", runRef: "run.42" })),
    )
    expect(request._tag).toBe("RunEvidenceRequest")
    if (request._tag === "RunEvidenceRequest") {
      expect(request.runRef).toBe("run.42")
      expect(request.explanation).toContain("stale")
    }
  })

  test("an explain-debug answer becomes a read-only IDE-11 debug-context request bound to the intent debug ref", () => {
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("explain_debug", "The breakpoint stopped in the parser."), decodeIntent({ _tag: "ExplainDebug", debugRef: "debug.7" })),
    )
    expect(request._tag).toBe("DebugContextRequest")
    if (request._tag === "DebugContextRequest") expect(request.debugRef).toBe("debug.7")
  })

  test("a draft-commit-message answer becomes an IDE-12 draft field only (no stage/commit/push)", () => {
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("draft_commit_message", "fix(parser): repair stale fixture"), decodeIntent({ _tag: "DraftCommitMessage", sourceControlRef: "scm.3" })),
    )
    expect(request._tag).toBe("CommitMessageDraft")
    if (request._tag === "CommitMessageDraft") {
      expect(request.sourceControlRef).toBe("scm.3")
      expect(request.draftMessage).toBe("fix(parser): repair stale fixture")
    }
  })

  test("model text never becomes a command argument: a shell-shaped draft stays draft text and mints no action", () => {
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("draft_commit_message", "$(rm -rf /) && git push --force"), decodeIntent({ _tag: "DraftCommitMessage", sourceControlRef: "scm.3" })),
    )
    // The only place the model text lands is the draft field. There is no run,
    // stage, commit, or push variant in the request union at all.
    expect(request._tag).toBe("CommitMessageDraft")
    if (request._tag === "CommitMessageDraft") expect(request.draftMessage).toBe("$(rm -rf /) && git push --force")
  })

  test("an inference result cannot redirect the action class: a candidate task class that disagrees with the host intent is refused", () => {
    // The candidate declares propose_edit, but the host intent is a plain Ask.
    // Routing is host-owned, so the mismatch fails safe to advisory only.
    const request = deriveTurnActionRequest(
      delivery(answerCandidate("propose_edit", "trust me, apply this"), decodeIntent({ _tag: "Ask", text: "hi" })),
    )
    expect(request._tag).toBe("AdvisoryOnly")
    if (request._tag === "AdvisoryOnly") expect(request.reason).toBe("task_class_mismatch")
  })
})

/** Capture the requests routed to the sink so a test can prove what the broker did. */
const capturingSink = Effect.gen(function* () {
  const log = yield* Ref.make<ReadonlyArray<TurnActionRecord>>([])
  const sink = TurnActionSink.of({
    route: (request: TurnActionRequest) =>
      Effect.gen(function* () {
        const record: TurnActionRecord = {
          request,
          resultRef: request._tag === "AdvisoryOnly" ? null : (`ide.turn-action.test.${request.candidateRef}` as const),
        }
        yield* Ref.update(log, (all) => [...all, record])
        return record
      }),
  })
  return { sink, read: Ref.get(log) } as const
})

describe("desktop ActionBroker — the advisory to proposal to owner-accept seam", () => {
  test("the broker routes a proposal candidate to the sink and records a backlink, applying nothing", () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const { sink, read } = yield* capturingSink
        const broker = yield* ActionBroker.pipe(
          Effect.provide(makeDesktopActionBrokerLayer.pipe(Layer.provide(Layer.succeed(TurnActionSink, sink)))),
        )
        yield* broker.deliver(delivery(proposalCandidate(), decodeIntent({ _tag: "ProposeEdit", instruction: "rename it" })))
        const recorded = yield* read
        expect(recorded).toHaveLength(1)
        expect(recorded[0]!.request._tag).toBe("ProposalRequest")
        // The recorded backlink is evidence, never an acceptance or a completion
        // receipt: it references the IDE-08 proposal the owner still must accept.
        expect(recorded[0]!.resultRef).not.toBeNull()
      }),
    ))

  test("the broker routes a chat answer as advisory only: no action is recorded", () =>
    Effect.runPromise(
      Effect.gen(function* () {
        const { sink, read } = yield* capturingSink
        const broker = yield* ActionBroker.pipe(
          Effect.provide(makeDesktopActionBrokerLayer.pipe(Layer.provide(Layer.succeed(TurnActionSink, sink)))),
        )
        yield* broker.deliver(delivery(answerCandidate("local_answer", "Paris."), decodeIntent({ _tag: "Ask", text: "capital?" })))
        const recorded = yield* read
        expect(recorded).toHaveLength(1)
        expect(recorded[0]!.request._tag).toBe("AdvisoryOnly")
        expect(recorded[0]!.resultRef).toBeNull()
      }),
    ))
})
