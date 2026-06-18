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
import {
  DataContributionCorrectnessVerification,
  projectDataTraceMarketplaceGate,
  type DataContributionCorrectnessVerification as DataContributionCorrectnessVerificationType,
} from './data-trace-marketplace-gate'
import type { ForumWorkRequestLifecycleKind } from './forum-work-requests'
import {
  TassadarAdversarialVerificationVerdict,
  projectTassadarAdversarialVerificationReleaseGate,
  type TassadarAdversarialVerificationVerdict as TassadarAdversarialVerificationVerdictType,
} from './tassadar-adversarial-verification-market'

export const ArtanisWorkDirectionKind = S.Literals([
  'adversarial_verification',
  'program_authorship',
  'dataset_curation',
])
export type ArtanisWorkDirectionKind = typeof ArtanisWorkDirectionKind.Type

export const ArtanisWorkDirectionVerificationClass = S.Literals([
  'e3_adversarial_divergence',
  'v1_construction',
  'v3_data_correctness',
])
export type ArtanisWorkDirectionVerificationClass =
  typeof ArtanisWorkDirectionVerificationClass.Type

export const ArtanisWorkDirectionRequest = S.Struct({
  budgetSats: S.Number,
  corpusRef: S.optional(S.String),
  deadlineRef: S.String,
  directionKind: ArtanisWorkDirectionKind,
  moduleFamilyRef: S.optional(S.String),
  objectiveRef: S.String,
  repositoryRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  title: S.String,
  verificationClass: ArtanisWorkDirectionVerificationClass,
  verificationCommandRef: S.String,
})
export type ArtanisWorkDirectionRequest =
  typeof ArtanisWorkDirectionRequest.Type

export const ArtanisProgramAuthorshipVerificationVerdict = S.Struct({
  blockerRefs: S.Array(S.String),
  constructionVerified: S.Boolean,
  moduleDigest: S.String,
  moduleKind: S.String,
  realBitcoinMoved: S.Boolean,
  replayVerified: S.Boolean,
  settlementSimulationRef: S.optional(S.String),
  verificationClass: S.Literal('v1_construction'),
  verificationRef: S.String,
})
export type ArtanisProgramAuthorshipVerificationVerdict =
  typeof ArtanisProgramAuthorshipVerificationVerdict.Type

export const ArtanisWorkDirectionDelivery = S.Struct({
  acceptanceEventRef: S.String,
  adversarialVerification: S.optional(TassadarAdversarialVerificationVerdict),
  dataCorrectnessVerification: S.optional(
    DataContributionCorrectnessVerification,
  ),
  directionKind: ArtanisWorkDirectionKind,
  programVerification: S.optional(ArtanisProgramAuthorshipVerificationVerdict),
  providerActorRef: S.String,
  resultRef: S.String,
  verificationCommandRef: S.String,
  workRequestId: S.String,
})
export type ArtanisWorkDirectionDelivery =
  typeof ArtanisWorkDirectionDelivery.Type

export const ArtanisWorkRoutingProposalDirection = S.Struct({
  budgetSats: S.Number,
  corpusRef: S.optional(S.String),
  deadlineRef: S.String,
  directionKind: ArtanisWorkDirectionKind,
  moduleFamilyRef: S.optional(S.String),
  objectiveRef: S.String,
  proposalRef: S.String,
  repositoryRefs: S.Array(S.String),
  selectorRef: S.String,
  sourceRefs: S.Array(S.String),
  title: S.String,
})
export type ArtanisWorkRoutingProposalDirection =
  typeof ArtanisWorkRoutingProposalDirection.Type

export type ArtanisWorkRoutingProposalSelection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  enabled: boolean
  proposalRefs: ReadonlyArray<string>
  requestCount: number
  skippedReason: 'operator_disabled' | null
  workRequests: ReadonlyArray<ArtanisWorkDirectionRequest>
}>

export type ArtanisWorkDirectionVerificationGate = Readonly<{
  blockerRefs: ReadonlyArray<string>
  gateRef: string
  releaseAllowed: boolean
  status: 'accepted' | 'rejected' | 'needs_validator_review'
  validatorReviewRefs: ReadonlyArray<string>
  verificationClass: ArtanisWorkDirectionVerificationClass
  verificationReceiptRefs: ReadonlyArray<string>
}>

export type ArtanisWorkDirectionRequesterDeps = Omit<
  ArtanisLaborRequesterDeps,
  'propose'
> &
  Readonly<{
    proposeWorkDirection: () => Promise<ArtanisWorkDirectionRequest>
  }>

export type ArtanisWorkDirectionAcceptanceDeps = Omit<
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

export type ArtanisWorkDirectionAcceptanceOutcome =
  | Readonly<{
      kind: 'settled'
      lifecycleKinds: ReadonlyArray<'delivered' | 'accepted' | 'settled'>
      releaseReceiptRef: string
      verificationGate: ArtanisWorkDirectionVerificationGate
    }>
  | Readonly<{
      kind: 'rejected_refunded'
      lifecycleKinds: ReadonlyArray<'delivered'>
      reasonRef: string
      refundReceiptRef: string
      verificationGate: ArtanisWorkDirectionVerificationGate
    }>

export class ArtanisWorkDirectionUnsafe extends S.TaggedErrorClass<ArtanisWorkDirectionUnsafe>()(
  'ArtanisWorkDirectionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const moduleDigestPattern = /^[a-f0-9]{32,128}$/i
const unsafeWorkDirectionRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|dataset\.(private|raw)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private([._-]|$)|provider[_-]?(account|credential|grant|payload|secret|token)|raw([._-]|$)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[._-]?(archive|private|raw)|token|trace[._-]?(raw|full|private|payload)|wallet)/i

const decodeWorkDirectionRequest = S.decodeUnknownSync(
  ArtanisWorkDirectionRequest,
)
const decodeWorkDirectionDelivery = S.decodeUnknownSync(
  ArtanisWorkDirectionDelivery,
)
const decodeProgramVerification = S.decodeUnknownSync(
  ArtanisProgramAuthorshipVerificationVerdict,
)
const decodeWorkRoutingProposal = S.decodeUnknownSync(
  ArtanisWorkRoutingProposalDirection,
)
const decodeDataCorrectnessVerification = S.decodeUnknownSync(
  DataContributionCorrectnessVerification,
)
const decodeAdversarialVerification = S.decodeUnknownSync(
  TassadarAdversarialVerificationVerdict,
)

const uniqueRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined))]
    .map(ref => ref.trim())
    .filter(ref => ref.length > 0)
    .sort()

const assertSafeRefs = (
  refs: ReadonlyArray<string | undefined>,
  field: string,
): void => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref =>
    !safeRefPattern.test(ref) || unsafeWorkDirectionRefPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: `${field} must be public-safe refs.`,
    })
  }

  assertArtanisLaborPublicSafe(normalized)
}

const verifierCommandRefFor = (
  directionKind: ArtanisWorkDirectionKind,
): string =>
  directionKind === 'program_authorship'
    ? 'command.public.tassadar.v1_construction_verification'
    : directionKind === 'dataset_curation'
      ? 'command.public.openagents.data_contribution.v3_correctness'
      : 'command.public.tassadar.e3_adversarial_divergence'

const verificationClassFor = (
  directionKind: ArtanisWorkDirectionKind,
): ArtanisWorkDirectionVerificationClass =>
  directionKind === 'program_authorship'
    ? 'v1_construction'
    : directionKind === 'dataset_curation'
      ? 'v3_data_correctness'
      : 'e3_adversarial_divergence'

const capabilityRefsFor = (
  request: ArtanisWorkDirectionRequest,
): ReadonlyArray<string> =>
  uniqueRefs([
    'capability.openagents.artanis_work_direction.ref_only_delivery',
    ...(request.directionKind === 'program_authorship'
      ? [
          'capability.openagents.tassadar.program_authorship.calm_wasm_module',
          'capability.openagents.tassadar.compiled_module.construct',
          'capability.openagents.tassadar.v1_construction_verification',
          'capability.openagents.tassadar.corpus.c1',
        ]
      : request.directionKind === 'dataset_curation'
        ? [
            'capability.openagents.tassadar.dataset_curation.trace_corpus',
            'capability.openagents.data_contribution.v3_correctness',
            'capability.openagents.reference_lane.distill_to_trace_corpus',
          ]
        : [
            'capability.openagents.tassadar.adversarial_verification.divergence_input',
            'capability.openagents.tassadar.adversarial_verification.independent_reproduction',
            'capability.openagents.tassadar.e3_adversarial_divergence',
            'capability.openagents.tassadar.v1_found_defect_settlement',
          ]),
  ])

const assertRequest = (request: ArtanisWorkDirectionRequest): void => {
  if (!Number.isInteger(request.budgetSats) || request.budgetSats <= 0) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: 'Artanis work-direction budget must be positive sats.',
    })
  }
  if (request.title.trim().length < 3 || request.title.length > 160) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: 'Artanis work-direction title must be 3-160 characters.',
    })
  }
  if (request.repositoryRefs.length === 0) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: 'Artanis work-direction request requires repository refs.',
    })
  }
  if (request.verificationClass !== verificationClassFor(request.directionKind)) {
    throw new ArtanisWorkDirectionUnsafe({
      reason:
        'Artanis work-direction verification class must match direction kind.',
    })
  }
  if (request.directionKind === 'program_authorship') {
    assertSafeRefs(
      [request.moduleFamilyRef, request.corpusRef],
      'Artanis program-authorship target refs',
    )
    if (request.moduleFamilyRef === undefined || request.corpusRef === undefined) {
      throw new ArtanisWorkDirectionUnsafe({
        reason:
          'Program authorship requests require moduleFamilyRef and corpusRef.',
      })
    }
  }
  if (request.directionKind === 'dataset_curation') {
    if (request.corpusRef === undefined) {
      throw new ArtanisWorkDirectionUnsafe({
        reason: 'Dataset curation requests require corpusRef.',
      })
    }
  }
  if (request.directionKind === 'adversarial_verification') {
    assertSafeRefs(
      [request.moduleFamilyRef, request.corpusRef],
      'Artanis adversarial-verification target refs',
    )
    if (request.moduleFamilyRef === undefined || request.corpusRef === undefined) {
      throw new ArtanisWorkDirectionUnsafe({
        reason:
          'Adversarial verification requests require moduleFamilyRef and corpusRef.',
      })
    }
  }

  assertSafeRefs(
    [
      request.deadlineRef,
      request.objectiveRef,
      request.verificationCommandRef,
      request.corpusRef,
      request.moduleFamilyRef,
      ...request.repositoryRefs,
      ...request.sourceRefs,
    ],
    'Artanis work-direction request',
  )
  assertArtanisLaborPublicSafe(request)
}

export const buildArtanisWorkDirectionLaborProposal = (
  input: ArtanisWorkDirectionRequest,
): ArtanisLaborRequestProposal => {
  const request = decodeWorkDirectionRequest(input)
  assertRequest(request)

  return {
    budgetSats: request.budgetSats,
    deadlineRef: request.deadlineRef,
    objectiveRef: request.objectiveRef,
    repositoryRefs: request.repositoryRefs,
    requiredCapabilityRefs: capabilityRefsFor(request),
    title: request.title,
    verificationCommandRef: request.verificationCommandRef,
  }
}

export const runArtanisWorkDirectionRequestTick = (
  deps: ArtanisWorkDirectionRequesterDeps,
): Promise<ArtanisLaborRequesterOutcome> =>
  runArtanisLaborRequestTick({
    ...deps,
    propose: async () =>
      buildArtanisWorkDirectionLaborProposal(await deps.proposeWorkDirection()),
  })

export const buildArtanisWorkDirectionRequestFromRoutingProposal = (
  proposalInput: ArtanisWorkRoutingProposalDirection,
): ArtanisWorkDirectionRequest => {
  const proposal = decodeWorkRoutingProposal(proposalInput)
  assertSafeRefs(
    [
      proposal.proposalRef,
      proposal.selectorRef,
      proposal.objectiveRef,
      proposal.deadlineRef,
      proposal.corpusRef,
      proposal.moduleFamilyRef,
      ...proposal.repositoryRefs,
      ...proposal.sourceRefs,
    ],
    'Artanis work-routing proposal',
  )

  const request = decodeWorkDirectionRequest({
    budgetSats: proposal.budgetSats,
    corpusRef: proposal.corpusRef,
    deadlineRef: proposal.deadlineRef,
    directionKind: proposal.directionKind,
    moduleFamilyRef: proposal.moduleFamilyRef,
    objectiveRef: proposal.objectiveRef,
    repositoryRefs: proposal.repositoryRefs,
    sourceRefs: uniqueRefs([
      proposal.proposalRef,
      proposal.selectorRef,
      ...proposal.sourceRefs,
    ]),
    title: proposal.title,
    verificationClass: verificationClassFor(proposal.directionKind),
    verificationCommandRef: verifierCommandRefFor(proposal.directionKind),
  })
  assertRequest(request)

  return request
}

export const filterArtanisWorkRoutingProposalsIntoRequests = (
  input: Readonly<{
    operatorEnabled?: boolean | undefined
    proposals: ReadonlyArray<ArtanisWorkRoutingProposalDirection>
  }>,
): ArtanisWorkRoutingProposalSelection => {
  const proposalRefs = uniqueRefs(input.proposals.map(proposal =>
    proposal.proposalRef
  ))
  assertSafeRefs(proposalRefs, 'Artanis work-routing proposal refs')

  if (input.operatorEnabled !== true) {
    return {
      blockerRefs: ['blocker.public.artanis.work_directions.operator_disabled'],
      enabled: false,
      proposalRefs,
      requestCount: 0,
      skippedReason: 'operator_disabled',
      workRequests: [],
    }
  }

  const workRequests = input.proposals.map(proposal =>
    buildArtanisWorkDirectionRequestFromRoutingProposal(proposal)
  )
  workRequests.forEach(assertRequest)

  return {
    blockerRefs: [],
    enabled: true,
    proposalRefs,
    requestCount: workRequests.length,
    skippedReason: null,
    workRequests,
  }
}

const programVerificationGate = (
  verdictInput: ArtanisProgramAuthorshipVerificationVerdict | undefined,
): ArtanisWorkDirectionVerificationGate => {
  if (verdictInput === undefined) {
    return {
      blockerRefs: [
        'blocker.public.artanis.work_direction.v1_verification_missing',
      ],
      gateRef: 'gate.public.artanis.work_direction.v1.missing',
      releaseAllowed: false,
      status: 'rejected',
      validatorReviewRefs: [],
      verificationClass: 'v1_construction',
      verificationReceiptRefs: [],
    }
  }

  const verdict = decodeProgramVerification(verdictInput)
  assertSafeRefs(
    [
      verdict.verificationRef,
      verdict.settlementSimulationRef,
      verdict.moduleKind,
      ...verdict.blockerRefs,
    ],
    'Artanis program-authorship verification',
  )
  const blockerRefs = uniqueRefs([
    ...verdict.blockerRefs,
    ...(!verdict.constructionVerified
      ? ['blocker.public.artanis.work_direction.v1_construction_failed']
      : []),
    ...(!verdict.replayVerified
      ? ['blocker.public.artanis.work_direction.v1_replay_failed']
      : []),
    ...(verdict.realBitcoinMoved
      ? ['blocker.public.artanis.work_direction.real_bitcoin_not_allowed']
      : []),
    ...(!moduleDigestPattern.test(verdict.moduleDigest)
      ? ['blocker.public.artanis.work_direction.module_digest_invalid']
      : []),
  ])
  const accepted = blockerRefs.length === 0

  return {
    blockerRefs,
    gateRef:
      `gate.public.artanis.work_direction.v1.${verdict.moduleDigest.slice(0, 16)}`,
    releaseAllowed: accepted,
    status: accepted ? 'accepted' : 'rejected',
    validatorReviewRefs: [],
    verificationClass: 'v1_construction',
    verificationReceiptRefs: accepted
      ? uniqueRefs([verdict.verificationRef, verdict.settlementSimulationRef])
      : [],
  }
}

const dataCorrectnessGate = (
  verificationInput: DataContributionCorrectnessVerificationType | undefined,
  delivery: ArtanisWorkDirectionDelivery,
): ArtanisWorkDirectionVerificationGate => {
  if (verificationInput === undefined) {
    return {
      blockerRefs: [
        'blocker.public.artanis.work_direction.v3_verification_missing',
      ],
      gateRef: 'gate.public.artanis.work_direction.v3.missing',
      releaseAllowed: false,
      status: 'rejected',
      validatorReviewRefs: [],
      verificationClass: 'v3_data_correctness',
      verificationReceiptRefs: [],
    }
  }

  const verification = decodeDataCorrectnessVerification(verificationInput)
  const dataGate = projectDataTraceMarketplaceGate({
    correctnessVerification: verification,
    plannerMode: 'structured_query_planner',
    redactionReceiptRefs: [
      'redaction.public.artanis.work_direction.ref_only_delivery',
    ],
    semanticPlannerRefs: [
      'planner.public.artanis.work_direction.structured_data_contribution',
    ],
    traceSubmissionRefs: [delivery.resultRef],
  })
  const accepted =
    verification.correctnessGatePassed &&
    dataGate.correctnessGatePassed &&
    verification.status === 'accepted' &&
    !verification.validatorReviewRequired
  const status = accepted
    ? 'accepted'
    : verification.status === 'needs_validator_review'
      ? 'needs_validator_review'
      : 'rejected'

  return {
    blockerRefs: uniqueRefs([
      ...verification.blockerRefs,
      ...dataGate.correctnessBlockerRefs,
      ...(!accepted && status !== 'needs_validator_review'
        ? ['blocker.public.artanis.work_direction.v3_correctness_failed']
        : []),
    ]),
    gateRef: `gate.public.artanis.work_direction.v3.${verification.verificationRef
      .replace(/[^A-Za-z0-9]+/g, '_')
      .slice(0, 80)}`,
    releaseAllowed: accepted,
    status,
    validatorReviewRefs: verification.validatorReviewRefs,
    verificationClass: 'v3_data_correctness',
    verificationReceiptRefs: accepted ? verification.correctnessReceiptRefs : [],
  }
}

const adversarialVerificationGate = (
  verificationInput:
    | TassadarAdversarialVerificationVerdictType
    | undefined,
): ArtanisWorkDirectionVerificationGate => {
  if (verificationInput === undefined) {
    return {
      blockerRefs: [
        'blocker.public.artanis.work_direction.e3_verification_missing',
      ],
      gateRef: 'gate.public.artanis.work_direction.e3.missing',
      releaseAllowed: false,
      status: 'rejected',
      validatorReviewRefs: [],
      verificationClass: 'e3_adversarial_divergence',
      verificationReceiptRefs: [],
    }
  }

  const verification = decodeAdversarialVerification(verificationInput)
  const divergenceGate =
    projectTassadarAdversarialVerificationReleaseGate(verification)

  return {
    blockerRefs: divergenceGate.blockerRefs,
    gateRef: divergenceGate.gateRef,
    releaseAllowed: divergenceGate.releaseAllowed,
    status: divergenceGate.status,
    validatorReviewRefs: [],
    verificationClass: 'e3_adversarial_divergence',
    verificationReceiptRefs: divergenceGate.verificationReceiptRefs,
  }
}

export const projectArtanisWorkDirectionVerificationGate = (
  deliveryInput: ArtanisWorkDirectionDelivery,
): ArtanisWorkDirectionVerificationGate => {
  const delivery = decodeWorkDirectionDelivery(deliveryInput)
  assertSafeRefs(
    [
      delivery.acceptanceEventRef,
      delivery.providerActorRef,
      delivery.resultRef,
      delivery.verificationCommandRef,
      delivery.workRequestId,
    ],
    'Artanis work-direction delivery',
  )
  assertArtanisLaborPublicSafe(delivery)

  return delivery.directionKind === 'program_authorship'
    ? programVerificationGate(delivery.programVerification)
    : delivery.directionKind === 'dataset_curation'
      ? dataCorrectnessGate(delivery.dataCorrectnessVerification, delivery)
      : adversarialVerificationGate(delivery.adversarialVerification)
}

const laborDeliveryFromWorkDirectionDelivery = (
  delivery: ArtanisWorkDirectionDelivery,
): ArtanisLaborResultDelivery => ({
  acceptanceEventRef: delivery.acceptanceEventRef,
  providerActorRef: delivery.providerActorRef,
  resultRef: delivery.resultRef,
  verificationCommandRef: delivery.verificationCommandRef,
  workRequestId: delivery.workRequestId,
})

const projectWorkDirectionOutcome = async (
  laborOutcome: ArtanisLaborAcceptanceOutcome,
  verificationGate: ArtanisWorkDirectionVerificationGate,
  delivery: ArtanisWorkDirectionDelivery,
  deps: ArtanisWorkDirectionAcceptanceDeps,
): Promise<ArtanisWorkDirectionAcceptanceOutcome> => {
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
      kind: 'settled',
      lifecycleKinds: ['delivered', 'accepted', 'settled'],
      releaseReceiptRef: laborOutcome.releaseReceiptRef,
      verificationGate,
    }
  }

  return {
    kind: 'rejected_refunded',
    lifecycleKinds: ['delivered'],
    reasonRef: laborOutcome.reasonRef,
    refundReceiptRef: laborOutcome.refundReceiptRef,
    verificationGate,
  }
}

export const handleArtanisWorkDirectionDelivery = async (
  deliveryInput: ArtanisWorkDirectionDelivery,
  deps: ArtanisWorkDirectionAcceptanceDeps,
): Promise<ArtanisWorkDirectionAcceptanceOutcome> => {
  const delivery = decodeWorkDirectionDelivery(deliveryInput)
  const verificationGate = projectArtanisWorkDirectionVerificationGate(delivery)

  await deps.recordLifecycle({
    lifecycleKind: 'delivered',
    receiptRef: delivery.resultRef,
    workRequestId: delivery.workRequestId,
  })

  const laborOutcome = await handleArtanisLaborResultDelivery(
    laborDeliveryFromWorkDirectionDelivery(delivery),
    {
      ...deps,
      validateResult: async () =>
        verificationGate.releaseAllowed
          ? {
              passed: true,
              verifierRef:
                verificationGate.verificationReceiptRefs[0] ??
                verificationGate.gateRef,
            }
          : {
              passed: false,
              reasonRef:
                verificationGate.blockerRefs[0] ??
                'blocker.public.artanis.work_direction.verification_failed',
            },
    },
  )

  return projectWorkDirectionOutcome(
    laborOutcome,
    verificationGate,
    delivery,
    deps,
  )
}
