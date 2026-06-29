import { Schema as S } from 'effect'

import {
  assertArtanisLaborPublicSafe,
  handleArtanisLaborResultDelivery,
  runArtanisLaborRequestTick,
  type ArtanisLaborAcceptanceDeps,
  type ArtanisLaborAcceptanceOutcome,
  type ArtanisLaborRequestProposal,
  type ArtanisLaborRequesterDeps,
  type ArtanisLaborRequesterOutcome,
  type ArtanisLaborResultDelivery,
} from './artanis-labor-requester'
import { projectDataTraceMarketplaceGate } from './data-trace-marketplace-gate'
import {
  type DebtReceiptStudiedKnowledgeSource,
  DebtReceiptStudiedKnowledgeVerificationSchemaRef,
} from './debt-receipt-policy'
import type { ForumWorkRequestLifecycleKind } from './forum-work-requests'

export const OpenAgentsRepoStudiedKnowledgeVerificationSchemaRef =
  'openagents.repo_studied_knowledge_verification.v0' as const

export const ArtanisStudyingContributionKind = S.Literals([
  'study_packet',
  'studied_knowledge_graph',
  'study_packet_and_graph',
])
export type ArtanisStudyingContributionKind =
  typeof ArtanisStudyingContributionKind.Type

export const ArtanisStudyingContributionVerificationVerdict = S.Struct({
  correctnessGatePassed: S.Boolean,
  graphHash: S.optional(S.String),
  graphRef: S.optional(S.String),
  packetHash: S.String,
  packetRef: S.String,
  rejectedCount: S.Number,
  schemaRef: S.Literal(OpenAgentsRepoStudiedKnowledgeVerificationSchemaRef),
  sourceBoundary: S.Literal('public_refs_only'),
  validatorReviewRequired: S.Boolean,
  validatorReviewRefs: S.Array(S.String),
  verificationHash: S.String,
  verificationRef: S.String,
})
export type ArtanisStudyingContributionVerificationVerdict =
  typeof ArtanisStudyingContributionVerificationVerdict.Type

export const ArtanisStudyingContributionWorkRequest = S.Struct({
  budgetSats: S.Number,
  contributionKind: ArtanisStudyingContributionKind,
  deadlineRef: S.String,
  graphRef: S.optional(S.String),
  objectiveRef: S.String,
  packetRef: S.optional(S.String),
  repositoryRef: S.String,
  title: S.String,
  verificationCommandRef: S.String,
})
export type ArtanisStudyingContributionWorkRequest =
  typeof ArtanisStudyingContributionWorkRequest.Type

export const ArtanisStudyingContributionDelivery = S.Struct({
  acceptanceEventRef: S.String,
  contributionKind: ArtanisStudyingContributionKind,
  contributionRef: S.String,
  graphRef: S.optional(S.String),
  packetRef: S.String,
  providerActorRef: S.String,
  resultRef: S.String,
  s3Verification: ArtanisStudyingContributionVerificationVerdict,
  verificationCommandRef: S.String,
  workRequestId: S.String,
})
export type ArtanisStudyingContributionDelivery =
  typeof ArtanisStudyingContributionDelivery.Type

export type ArtanisStudyingContributionCorrectnessGate = Readonly<{
  blockerRefs: ReadonlyArray<string>
  correctnessReceiptRefs: ReadonlyArray<string>
  gateRef: string
  releaseAllowed: boolean
  status: 'accepted' | 'rejected' | 'needs_validator_review'
  validatorReviewRefs: ReadonlyArray<string>
}>

export type ArtanisStudyingLaborRequesterDeps = Omit<
  ArtanisLaborRequesterDeps,
  'propose'
> &
  Readonly<{
    proposeStudyingContribution: () => Promise<ArtanisStudyingContributionWorkRequest>
  }>

export type ArtanisStudyingLaborAcceptanceDeps = Omit<
  ArtanisLaborAcceptanceDeps,
  'validateResult'
> &
  Readonly<{
    recordLifecycle: (input: Readonly<{
      lifecycleKind: ForumWorkRequestLifecycleKind
      receiptRef: string
      workRequestId: string
    }>) => Promise<void>
  }>

export type ArtanisStudyingLaborAcceptanceOutcome =
  | Readonly<{
      correctnessGate: ArtanisStudyingContributionCorrectnessGate
      kind: 'settled'
      releaseReceiptRef: string
      lifecycleKinds: ReadonlyArray<'delivered' | 'accepted' | 'settled'>
    }>
  | Readonly<{
      correctnessGate: ArtanisStudyingContributionCorrectnessGate
      kind: 'rejected_refunded'
      refundReceiptRef: string
      reasonRef: string
      lifecycleKinds: ReadonlyArray<'delivered'>
    }>

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const sha256Pattern = /^sha256:[a-f0-9]{64}$/

export class ArtanisStudyingLaborValidationError extends Error {
  readonly field: string

  constructor(field: string, detail: string) {
    super(`${field} ${detail}`)
    this.field = field
  }
}

const uniqueRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined))]
    .map(ref => ref.trim())
    .filter(ref => ref.length > 0)
    .sort()

const assertSafeRef = (ref: string, field: string): void => {
  if (!safeRefPattern.test(ref)) {
    throw new ArtanisStudyingLaborValidationError(
      field,
      'must be a public-safe ref.',
    )
  }
  assertArtanisLaborPublicSafe(ref)
}

const assertSafeRefs = (
  refs: ReadonlyArray<string>,
  field: string,
): void => refs.forEach(ref => assertSafeRef(ref, field))

const assertSha256 = (hash: string, field: string): void => {
  if (!sha256Pattern.test(hash)) {
    throw new ArtanisStudyingLaborValidationError(
      field,
      'must be a sha256 ref.',
    )
  }
}

const decodeStudyingWorkRequest = S.decodeUnknownSync(
  ArtanisStudyingContributionWorkRequest,
)
const decodeStudyingDelivery = S.decodeUnknownSync(
  ArtanisStudyingContributionDelivery,
)
const decodeStudyingVerdict = S.decodeUnknownSync(
  ArtanisStudyingContributionVerificationVerdict,
)

export const buildArtanisStudyingLaborProposal = (
  input: ArtanisStudyingContributionWorkRequest,
): ArtanisLaborRequestProposal => {
  const request = decodeStudyingWorkRequest(input)
  const refs = uniqueRefs([
    request.repositoryRef,
    request.packetRef,
    request.graphRef,
    request.objectiveRef,
    request.deadlineRef,
    request.verificationCommandRef,
  ])
  assertSafeRefs(refs, 'Artanis studying work request')
  assertArtanisLaborPublicSafe(request)

  return {
    budgetSats: request.budgetSats,
    deadlineRef: request.deadlineRef,
    objectiveRef: request.objectiveRef,
    repositoryRefs: [request.repositoryRef],
    requiredCapabilityRefs: requiredCapabilityRefsFor(request.contributionKind),
    title: request.title,
    verificationCommandRef: request.verificationCommandRef,
  }
}

export const runArtanisStudyingLaborRequestTick = (
  deps: ArtanisStudyingLaborRequesterDeps,
): Promise<ArtanisLaborRequesterOutcome> =>
  runArtanisLaborRequestTick({
    ...deps,
    propose: async () =>
      buildArtanisStudyingLaborProposal(
        await deps.proposeStudyingContribution(),
      ),
  })

export const projectArtanisStudyingContributionCorrectnessGate = (
  deliveryInput: ArtanisStudyingContributionDelivery,
): ArtanisStudyingContributionCorrectnessGate => {
  const delivery = decodeStudyingDelivery(deliveryInput)
  const verdict = decodeStudyingVerdict(delivery.s3Verification)
  const contributionRefs = uniqueRefs([
    delivery.contributionRef,
    delivery.packetRef,
    delivery.graphRef,
    verdict.packetRef,
    verdict.graphRef,
  ])
  assertSafeRefs(contributionRefs, 'Artanis studying contribution')
  assertSha256(verdict.packetHash, 'S3 packet hash')
  assertSha256(verdict.verificationHash, 'S3 verification hash')

  if (verdict.graphHash !== undefined) {
    assertSha256(verdict.graphHash, 'S3 graph hash')
  }

  const packetMatches = delivery.packetRef === verdict.packetRef
  const graphMatches =
    delivery.graphRef === undefined ||
    verdict.graphRef === undefined ||
    delivery.graphRef === verdict.graphRef
  const accepted =
    packetMatches &&
    graphMatches &&
    verdict.correctnessGatePassed &&
    verdict.rejectedCount === 0 &&
    !verdict.validatorReviewRequired
  const validatorReviewRefs = uniqueRefs(verdict.validatorReviewRefs)
  const blockerRefs = [
    ...(!packetMatches
      ? ['blocker.public.study_labor.packet_ref_mismatch']
      : []),
    ...(!graphMatches
      ? ['blocker.public.study_labor.graph_ref_mismatch']
      : []),
    ...(!verdict.correctnessGatePassed
      ? ['blocker.public.study_labor.s3_correctness_failed']
      : []),
    ...(verdict.rejectedCount > 0
      ? ['blocker.public.study_labor.s3_rejected_claims']
      : []),
    ...(verdict.validatorReviewRequired
      ? ['blocker.public.study_labor.validator_review_required']
      : []),
  ].sort()

  const dataGate = projectDataTraceMarketplaceGate({
    correctnessReceiptRefs: accepted ? [verdict.verificationRef] : [],
    plannerMode: 'structured_query_planner',
    redactionReceiptRefs: [
      'receipt.public.study_labor.ref_only_redaction_checked',
    ],
    semanticPlannerRefs: [
      'planner.public.study_labor.structured_s3_verification',
    ],
    traceSubmissionRefs: contributionRefs,
    validatorReviewRefs,
  })
  const dataCorrectnessBlockerRefs = dataGate.blockerRefs.filter(
    ref =>
      ref.includes('correctness') ||
      ref === 'blocker.public.data_market.keyword_routing_disallowed',
  )
  const status = accepted
    ? 'accepted'
    : verdict.validatorReviewRequired
      ? 'needs_validator_review'
      : 'rejected'

  return {
    blockerRefs: uniqueRefs([...blockerRefs, ...dataCorrectnessBlockerRefs]),
    correctnessReceiptRefs: dataGate.correctnessReceiptRefs,
    gateRef: `gate.public.study_labor.${verdict.verificationHash.replace('sha256:', '').slice(0, 16)}`,
    releaseAllowed: accepted && dataGate.correctnessGatePassed,
    status,
    validatorReviewRefs,
  }
}

export const handleArtanisStudyingContributionDelivery = async (
  deliveryInput: ArtanisStudyingContributionDelivery,
  deps: ArtanisStudyingLaborAcceptanceDeps,
): Promise<ArtanisStudyingLaborAcceptanceOutcome> => {
  const delivery = decodeStudyingDelivery(deliveryInput)
  assertArtanisLaborPublicSafe(delivery)
  const correctnessGate =
    projectArtanisStudyingContributionCorrectnessGate(delivery)

  await deps.recordLifecycle({
    lifecycleKind: 'delivered',
    receiptRef: delivery.resultRef,
    workRequestId: delivery.workRequestId,
  })

  const laborOutcome = await handleArtanisLaborResultDelivery(
    laborDeliveryFromStudyingDelivery(delivery),
    {
      ...deps,
      validateResult: async () =>
        correctnessGate.releaseAllowed
          ? {
              passed: true,
              verifierRef:
                correctnessGate.correctnessReceiptRefs[0] ??
                delivery.s3Verification.verificationRef,
            }
          : {
              passed: false,
              reasonRef:
                correctnessGate.blockerRefs.find(ref =>
                  ref.startsWith('blocker.public.study_labor.'),
                ) ??
                correctnessGate.blockerRefs[0] ??
                'blocker.public.study_labor.s3_correctness_failed',
            },
    },
  )

  return projectStudyingLaborOutcome(laborOutcome, correctnessGate, delivery, deps)
}

const requiredCapabilityRefsFor = (
  contributionKind: ArtanisStudyingContributionKind,
): ReadonlyArray<string> => [
  'capability.openagents.study_contribution.ref_only_delivery',
  'capability.openagents.study_verification.s3_correctness',
  ...(contributionKind === 'study_packet'
    ? ['capability.openagents.study_packet.contribute']
    : []),
  ...(contributionKind === 'studied_knowledge_graph'
    ? ['capability.openagents.studied_knowledge_graph.contribute']
    : []),
  ...(contributionKind === 'study_packet_and_graph'
    ? [
        'capability.openagents.study_packet.contribute',
        'capability.openagents.studied_knowledge_graph.contribute',
      ]
    : []),
]

const laborDeliveryFromStudyingDelivery = (
  delivery: ArtanisStudyingContributionDelivery,
): ArtanisLaborResultDelivery => ({
  acceptanceEventRef: delivery.acceptanceEventRef,
  providerActorRef: delivery.providerActorRef,
  resultRef: delivery.resultRef,
  verificationCommandRef: delivery.verificationCommandRef,
  workRequestId: delivery.workRequestId,
})

const projectStudyingLaborOutcome = async (
  laborOutcome: ArtanisLaborAcceptanceOutcome,
  correctnessGate: ArtanisStudyingContributionCorrectnessGate,
  delivery: ArtanisStudyingContributionDelivery,
  deps: ArtanisStudyingLaborAcceptanceDeps,
): Promise<ArtanisStudyingLaborAcceptanceOutcome> => {
  if (laborOutcome.kind === 'accepted') {
    await deps.recordLifecycle({
      lifecycleKind: 'accepted',
      receiptRef: delivery.acceptanceEventRef,
      workRequestId: delivery.workRequestId,
    })
    await deps.recordLifecycle({
      lifecycleKind: 'settled',
      receiptRef: laborOutcome.releaseReceiptRef,
      workRequestId: delivery.workRequestId,
    })
    return {
      correctnessGate,
      kind: 'settled',
      lifecycleKinds: ['delivered', 'accepted', 'settled'],
      releaseReceiptRef: laborOutcome.releaseReceiptRef,
    }
  }

  return {
    correctnessGate,
    kind: 'rejected_refunded',
    lifecycleKinds: ['delivered'],
    reasonRef: laborOutcome.reasonRef,
    refundReceiptRef: laborOutcome.refundReceiptRef,
  }
}

// SA-3 (#5340): wire studied knowledge into the hygiene/refactoring debt-receipt
// lane (EPIC #5335). A studying contribution verified through the SA-1 study
// graph + S3 verification (`generateOpenAgentsRepoStudyArtifact` ->
// `verifyOpenAgentsRepoStudiedKnowledgeClaims`) is the understanding source a
// hygiene pass cites: "you can't safely refactor what you don't understand."
//
// This recognizes Artanis studying labor as a hygiene-lane work type by mapping
// its S3 verification verdict into a debt-receipt studied-knowledge source. The
// mapping carries public refs only and grants no mutation, spend, deployment,
// settlement, or self-review authority — the studied-knowledge gate in
// `debt-receipt-policy` remains evidence-only.

export const ArtanisStudyingHygieneWorkType =
  'hygiene.openagents.studied_knowledge_source' as const

export const studyingVerdictToDebtReceiptStudiedKnowledgeSource = (
  verdictInput: ArtanisStudyingContributionVerificationVerdict,
): DebtReceiptStudiedKnowledgeSource => {
  const verdict = decodeStudyingVerdict(verdictInput)
  const graphRef = verdict.graphRef ?? verdict.packetRef
  const refs = uniqueRefs([
    verdict.packetRef,
    graphRef,
    verdict.verificationRef,
    ...verdict.validatorReviewRefs,
  ])
  assertSafeRefs(refs, 'Artanis studying debt-receipt source')

  return {
    correctnessGatePassed: verdict.correctnessGatePassed,
    graphRef,
    packetRef: verdict.packetRef,
    rejectedCount: verdict.rejectedCount,
    schemaRef: DebtReceiptStudiedKnowledgeVerificationSchemaRef,
    sourceBoundary: 'public_refs_only',
    validatorReviewRefs: uniqueRefs(verdict.validatorReviewRefs),
    validatorReviewRequired: verdict.validatorReviewRequired,
    verificationRef: verdict.verificationRef,
  }
}
