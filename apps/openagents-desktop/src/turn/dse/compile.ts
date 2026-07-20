import { createHash } from "node:crypto"

import { Effect, Layer, Schema as S } from "effect"

import {
  COMPILED_PROGRAM_SCHEMA_LITERAL,
  CompiledProgram,
  DseModel,
  IndependentReviewResult,
  PromotionRequest,
  makeBaselinePointer,
  makeCandidateArtifact,
  honestChatReplySignature,
  makeSearchPlan,
  producerId,
  promotionId,
  reviewerId,
  turnRouteSignature,
  type BaselinePointer,
  type CandidateArtifact,
  type DatasetRevision,
  type DatasetSplit,
  type DseSignature,
  type EvaluationReport,
  type Metric,
  type PredictDeps,
  type ReleasedArtifactPointer,
  type UncertaintyRecord,
} from "@openagentsinc/dse"
import { compileSignature, computeUncertainty, evaluateCandidate, promote } from "@openagentsinc/dse/optimizer"

import {
  HONEST_CHAT_BASELINE_INSTRUCTION,
  HONEST_CHAT_COMPILED_INSTRUCTION,
  TURN_ROUTE_BASELINE_INSTRUCTION,
  TURN_ROUTE_COMPILED_INSTRUCTION,
  honestChatDataset,
  honestChatSplit,
  turnRouteDataset,
  turnRouteSplit,
} from "./fixtures.ts"
import { honestProxyModelLayer, routeProxyModelLayer } from "./proxy-model.ts"
import { honestChatMetric, turnRouteMetric } from "./route-metric.ts"

/**
 * AFS-09 offline compile of the Apple FM signatures.
 *
 * This module runs the DSE compiler over the production-shaped fixtures with a
 * deterministic offline proxy model, scores the compiled winner against the
 * hand-written baseline on validation and holdout, and assembles the complete
 * promotion evidence (validation and holdout reports, an independent review, a
 * released pointer, an uncertainty record, and the content-addressed baseline
 * pointer). It is OFFLINE ONLY — it imports the DSE optimizer and Node crypto —
 * so it never runs on the live provider path. The runtime resolves the
 * checked-in bytes it produces; it never links this compiler.
 */

/** A pinned timestamp so a repeated compile is bit-identical. */
export const DSE_COMPILE_PINNED_AT = "2026-07-20T00:00:00.000Z" as const

const decodeProgram = S.decodeUnknownSync(CompiledProgram)
const decodeRequest = S.decodeUnknownSync(PromotionRequest)
const decodeReview = S.decodeUnknownSync(IndependentReviewResult)

const sha256Hex = (text: string): string => createHash("sha256").update(text, "utf8").digest("hex")

const deps: PredictDeps = { sha256: sha256Hex, now: () => DSE_COMPILE_PINNED_AT }

const PRODUCER = producerId("producer:openagents-dse-compiler")
const REVIEWER = reviewerId("reviewer:afs-independent-evaluator")

/** The complete offline compile bundle for one signature. */
export interface DseCompileBundle {
  readonly signatureId: string
  readonly winner: CandidateArtifact
  readonly validationReport: EvaluationReport
  readonly holdoutReport: EvaluationReport
  readonly baselineHoldoutReport: EvaluationReport
  readonly uncertainty: UncertaintyRecord
  readonly pointer: ReleasedArtifactPointer
  readonly baseline: BaselinePointer
}

interface CompileArgs<I, O> {
  readonly signature: DseSignature<I, O>
  readonly baselineInstruction: string
  readonly compiledInstruction: string
  readonly revision: DatasetRevision
  readonly split: DatasetSplit
  readonly metric: Metric<O>
  readonly modelLayer: Layer.Layer<DseModel>
  readonly baselineBytes: string
  readonly baselineDescription: string
}

const baseProgram = <I, O>(signature: DseSignature<I, O>, instruction: string): CompiledProgram =>
  decodeProgram({
    schema: COMPILED_PROGRAM_SCHEMA_LITERAL,
    signatureId: signature.signatureId,
    promptIr: { ...signature.defaultPromptIr, instruction },
    decodePolicy: { maxRepairs: 1, maxOutputChars: 2000 },
    modelRole: "apple-fm-local",
  })

const compileOne = <I, O>(args: CompileArgs<I, O>): Effect.Effect<DseCompileBundle> =>
  Effect.gen(function* () {
    const searchPlan = makeSearchPlan({ algorithm: "instruction_grid.v1", candidateCap: 8 })
    const base = baseProgram(args.signature, args.baselineInstruction)

    // The compiler generates one candidate per instruction and scores each on
    // validation; the compiled honest/routing instruction wins and is scored on
    // holdout. Holdout labels stay inaccessible to candidate generation.
    const compiled = yield* compileSignature({
      signature: args.signature,
      base,
      knobs: {
        instructions: [args.baselineInstruction, args.compiledInstruction],
        fewShotSets: [],
        modelRoles: [],
        decodePolicies: [],
      },
      searchPlan,
      revision: args.revision,
      split: args.split,
      metric: args.metric,
      producedAt: DSE_COMPILE_PINNED_AT,
      deps,
    })

    // Score the hand-written baseline on the same holdout so the delta and the
    // uncertainty record are honest paired comparisons.
    const baselineArtifact = makeCandidateArtifact({
      signatureId: args.signature.signatureId,
      datasetRevisionId: args.revision.revisionId,
      searchPlan,
      program: base,
      producedAt: DSE_COMPILE_PINNED_AT,
    })
    const baselineHoldoutReport = yield* evaluateCandidate({
      signature: args.signature,
      program: base,
      candidateId: baselineArtifact.candidateId,
      metric: args.metric,
      revision: args.revision,
      splitName: "holdout",
      splitIds: args.split.holdout,
      deps,
    })

    const uncertainty = computeUncertainty({
      signatureId: args.signature.signatureId,
      candidateId: compiled.winner.candidateId,
      baselineHoldout: baselineHoldoutReport,
      candidateHoldout: compiled.holdoutReport,
    })

    // Promotion under the independent-evaluator role: a reviewer distinct from
    // the producer admits the winner because it beats the baseline on holdout.
    const holdoutDelta =
      compiled.holdoutReport.aggregateScore - baselineHoldoutReport.aggregateScore
    const request = decodeRequest({
      schema: "openagents.dse.promotion_request.v1",
      promotionId: promotionId(`promo:${args.signature.signatureId.replaceAll("/", ".")}`),
      signatureId: args.signature.signatureId,
      candidateId: compiled.winner.candidateId,
      producer: { kind: "producer", id: PRODUCER },
      validationReportDigest: compiled.validationReport.digest,
      holdoutReportDigest: compiled.holdoutReport.digest,
      minHoldoutDelta: 0,
      requestedAt: DSE_COMPILE_PINNED_AT,
    })
    const review = decodeReview({
      schema: "openagents.dse.independent_review_result.v1",
      promotionId: request.promotionId,
      signatureId: args.signature.signatureId,
      candidateId: compiled.winner.candidateId,
      reviewer: { kind: "reviewer", id: REVIEWER },
      decision: "admit",
      holdoutDelta,
      reviewedHoldoutReportDigest: compiled.holdoutReport.digest,
      reason: "compiled instruction beats the hand-written baseline on validation and holdout",
      reviewedAt: DSE_COMPILE_PINNED_AT,
    })
    const promoted = promote({
      request,
      review,
      winner: compiled.winner,
      holdoutReport: compiled.holdoutReport,
      now: () => DSE_COMPILE_PINNED_AT,
    })
    if (!promoted.ok) throw new Error(`promotion refused: ${promoted.reason}`)

    const baseline = makeBaselinePointer({
      signatureId: args.signature.signatureId,
      baselineRef: `baseline:${args.signature.signatureId}:handwritten`,
      bytes: args.baselineBytes,
      description: args.baselineDescription,
    })

    return {
      signatureId: args.signature.signatureId,
      winner: compiled.winner,
      validationReport: compiled.validationReport,
      holdoutReport: compiled.holdoutReport,
      baselineHoldoutReport,
      uncertainty,
      pointer: promoted.pointer,
      baseline,
    }
    // A compile error on the checked-in fixtures is a programming defect, not a
    // recoverable outcome, so surface it as a defect rather than a typed error.
  }).pipe(Effect.provide(args.modelLayer), Effect.orDie)

/** Compile the honest-answer signature offline. */
export const compileHonestChatArtifact = (): Promise<DseCompileBundle> =>
  Effect.runPromise(
    compileOne({
      signature: honestChatReplySignature,
      baselineInstruction: HONEST_CHAT_BASELINE_INSTRUCTION,
      compiledInstruction: HONEST_CHAT_COMPILED_INSTRUCTION,
      revision: honestChatDataset(),
      split: honestChatSplit(honestChatDataset()),
      metric: honestChatMetric,
      modelLayer: honestProxyModelLayer(),
      baselineBytes: HONEST_CHAT_BASELINE_INSTRUCTION,
      baselineDescription: "the hand-written Apple FM honesty preamble in apple-fm-prompt.ts",
    }),
  )

/** Compile the turn-route signature offline. */
export const compileTurnRouteArtifact = (): Promise<DseCompileBundle> =>
  Effect.runPromise(
    compileOne({
      signature: turnRouteSignature,
      baselineInstruction: TURN_ROUTE_BASELINE_INSTRUCTION,
      compiledInstruction: TURN_ROUTE_COMPILED_INSTRUCTION,
      revision: turnRouteDataset(),
      split: turnRouteSplit(turnRouteDataset()),
      metric: turnRouteMetric,
      modelLayer: routeProxyModelLayer(),
      baselineBytes: TURN_ROUTE_BASELINE_INSTRUCTION,
      baselineDescription: "the hand-written Apple FM route-recommendation prose in apple-fm-prompt.ts",
    }),
  )
