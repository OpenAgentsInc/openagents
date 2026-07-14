import { Schema as S } from 'effect'

import { assertArtanisLaborPublicSafe } from './artanis-labor-requester'
import { DataContributionCorrectnessVerification } from './data-trace-marketplace-gate'
import type { ForumWorkRequestLifecycleKind } from './forum-work-requests'

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

// Directions are evidence requests in VP1, not paid marketplace orders.
export const ArtanisWorkDirectionRequest = S.Struct({
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
  replayVerified: S.Boolean,
  verificationClass: S.Literal('v1_construction'),
  verificationRef: S.String,
})
export type ArtanisProgramAuthorshipVerificationVerdict =
  typeof ArtanisProgramAuthorshipVerificationVerdict.Type

export const ArtanisAdversarialVerificationVerdict = S.Struct({
  blockerRefs: S.Array(S.String),
  divergenceReproduced: S.Boolean,
  verificationReceiptRefs: S.Array(S.String),
})
export type ArtanisAdversarialVerificationVerdict =
  typeof ArtanisAdversarialVerificationVerdict.Type

export const ArtanisWorkDirectionDelivery = S.Struct({
  acceptanceEventRef: S.String,
  adversarialVerification: S.optional(ArtanisAdversarialVerificationVerdict),
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
  accepted: boolean
  blockerRefs: ReadonlyArray<string>
  gateRef: string
  status: 'accepted' | 'rejected' | 'needs_validator_review'
  validatorReviewRefs: ReadonlyArray<string>
  verificationClass: ArtanisWorkDirectionVerificationClass
  verificationReceiptRefs: ReadonlyArray<string>
}>

export type ArtanisWorkDirectionAcceptanceDeps = Readonly<{
  recordLifecycle: (input: Readonly<{
    lifecycleKind: ForumWorkRequestLifecycleKind
    receiptRef: string
    workRequestId: string
  }>) => Promise<void>
}>

export type ArtanisWorkDirectionAcceptanceOutcome = Readonly<{
  kind: 'verified' | 'rejected'
  lifecycleKinds: ReadonlyArray<'delivered' | 'accepted'>
  paymentMode: 'no-spend'
  verificationGate: ArtanisWorkDirectionVerificationGate
}>

export class ArtanisWorkDirectionUnsafe extends S.TaggedErrorClass<ArtanisWorkDirectionUnsafe>()(
  'ArtanisWorkDirectionUnsafe',
  { reason: S.String },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const moduleDigestPattern = /^[a-f0-9]{32,128}$/i
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|dataset\.(private|raw)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment|payout|preimage|private([._-]|$)|provider[_-]?(account|credential|grant|payload|secret|token)|raw([._-]|$)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const decodeRequest = S.decodeUnknownSync(ArtanisWorkDirectionRequest)
const decodeDelivery = S.decodeUnknownSync(ArtanisWorkDirectionDelivery)
const decodeProposal = S.decodeUnknownSync(ArtanisWorkRoutingProposalDirection)

const uniqueRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined))]
    .map(ref => ref.trim())
    .filter(Boolean)
    .sort()

const assertSafeRefs = (
  refs: ReadonlyArray<string | undefined>,
  field: string,
): void => {
  const normalized = uniqueRefs(refs)
  if (
    normalized.some(ref =>
      !safeRefPattern.test(ref) || unsafeRefPattern.test(ref)
    )
  ) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: `${field} must be public-safe refs.`,
    })
  }
  assertArtanisLaborPublicSafe(normalized)
}

const verificationClassFor = (
  kind: ArtanisWorkDirectionKind,
): ArtanisWorkDirectionVerificationClass =>
  kind === 'program_authorship'
    ? 'v1_construction'
    : kind === 'dataset_curation'
      ? 'v3_data_correctness'
      : 'e3_adversarial_divergence'

const verifierCommandRefFor = (kind: ArtanisWorkDirectionKind): string =>
  `command.public.artanis.${verificationClassFor(kind)}`

const assertRequest = (request: ArtanisWorkDirectionRequest): void => {
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
      reason: 'Artanis work-direction verification class must match direction kind.',
    })
  }
  if (
    request.directionKind !== 'dataset_curation' &&
    (request.moduleFamilyRef === undefined || request.corpusRef === undefined)
  ) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: 'This work direction requires moduleFamilyRef and corpusRef.',
    })
  }
  if (request.directionKind === 'dataset_curation' && request.corpusRef === undefined) {
    throw new ArtanisWorkDirectionUnsafe({
      reason: 'Dataset curation requests require corpusRef.',
    })
  }
  assertSafeRefs([
    request.deadlineRef,
    request.objectiveRef,
    request.verificationCommandRef,
    request.corpusRef,
    request.moduleFamilyRef,
    ...request.repositoryRefs,
    ...request.sourceRefs,
  ], 'Artanis work-direction request')
  assertArtanisLaborPublicSafe(request)
}

export const buildArtanisWorkDirectionRequestFromRoutingProposal = (
  proposalInput: ArtanisWorkRoutingProposalDirection,
): ArtanisWorkDirectionRequest => {
  const proposal = decodeProposal(proposalInput)
  const request = decodeRequest({
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
  const proposalRefs = uniqueRefs(input.proposals.map(item => item.proposalRef))
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
  const workRequests = input.proposals.map(
    buildArtanisWorkDirectionRequestFromRoutingProposal,
  )
  return {
    blockerRefs: [],
    enabled: true,
    proposalRefs,
    requestCount: workRequests.length,
    skippedReason: null,
    workRequests,
  }
}

const missingGate = (
  verificationClass: ArtanisWorkDirectionVerificationClass,
): ArtanisWorkDirectionVerificationGate => ({
  accepted: false,
  blockerRefs: [
    `blocker.public.artanis.work_direction.${verificationClass}.verification_missing`,
  ],
  gateRef: `gate.public.artanis.work_direction.${verificationClass}.missing`,
  status: 'rejected',
  validatorReviewRefs: [],
  verificationClass,
  verificationReceiptRefs: [],
})

export const projectArtanisWorkDirectionVerificationGate = (
  deliveryInput: ArtanisWorkDirectionDelivery,
): ArtanisWorkDirectionVerificationGate => {
  const delivery = decodeDelivery(deliveryInput)
  assertSafeRefs([
    delivery.acceptanceEventRef,
    delivery.providerActorRef,
    delivery.resultRef,
    delivery.verificationCommandRef,
    delivery.workRequestId,
  ], 'Artanis work-direction delivery')

  if (delivery.directionKind === 'program_authorship') {
    const verdict = delivery.programVerification
    if (verdict === undefined) return missingGate('v1_construction')
    const blockerRefs = uniqueRefs([
      ...verdict.blockerRefs,
      ...(!verdict.constructionVerified
        ? ['blocker.public.artanis.work_direction.v1_construction_failed']
        : []),
      ...(!verdict.replayVerified
        ? ['blocker.public.artanis.work_direction.v1_replay_failed']
        : []),
      ...(!moduleDigestPattern.test(verdict.moduleDigest)
        ? ['blocker.public.artanis.work_direction.module_digest_invalid']
        : []),
    ])
    return {
      accepted: blockerRefs.length === 0,
      blockerRefs,
      gateRef: `gate.public.artanis.work_direction.v1.${verdict.moduleDigest.slice(0, 16)}`,
      status: blockerRefs.length === 0 ? 'accepted' : 'rejected',
      validatorReviewRefs: [],
      verificationClass: 'v1_construction',
      verificationReceiptRefs:
        blockerRefs.length === 0 ? [verdict.verificationRef] : [],
    }
  }

  if (delivery.directionKind === 'dataset_curation') {
    const verdict = delivery.dataCorrectnessVerification
    if (verdict === undefined) return missingGate('v3_data_correctness')
    const accepted =
      verdict.correctnessGatePassed &&
      verdict.status === 'accepted' &&
      !verdict.validatorReviewRequired
    return {
      accepted,
      blockerRefs: accepted ? [] : uniqueRefs(verdict.blockerRefs),
      gateRef: `gate.public.artanis.work_direction.v3.${verdict.verificationRef.replace(/[^A-Za-z0-9]+/g, '_')}`,
      status: accepted
        ? 'accepted'
        : verdict.status === 'needs_validator_review'
          ? 'needs_validator_review'
          : 'rejected',
      validatorReviewRefs: verdict.validatorReviewRefs,
      verificationClass: 'v3_data_correctness',
      verificationReceiptRefs: accepted ? verdict.correctnessReceiptRefs : [],
    }
  }

  const verdict = delivery.adversarialVerification
  if (verdict === undefined) return missingGate('e3_adversarial_divergence')
  return {
    accepted: verdict.divergenceReproduced && verdict.blockerRefs.length === 0,
    blockerRefs: verdict.blockerRefs,
    gateRef: `gate.public.artanis.work_direction.e3.${delivery.workRequestId}`,
    status:
      verdict.divergenceReproduced && verdict.blockerRefs.length === 0
        ? 'accepted'
        : 'rejected',
    validatorReviewRefs: [],
    verificationClass: 'e3_adversarial_divergence',
    verificationReceiptRefs: verdict.verificationReceiptRefs,
  }
}

export const handleArtanisWorkDirectionDelivery = async (
  deliveryInput: ArtanisWorkDirectionDelivery,
  deps: ArtanisWorkDirectionAcceptanceDeps,
): Promise<ArtanisWorkDirectionAcceptanceOutcome> => {
  const delivery = decodeDelivery(deliveryInput)
  const verificationGate = projectArtanisWorkDirectionVerificationGate(delivery)
  await deps.recordLifecycle({
    lifecycleKind: 'delivered',
    receiptRef: delivery.resultRef,
    workRequestId: delivery.workRequestId,
  })
  if (verificationGate.accepted) {
    await deps.recordLifecycle({
      lifecycleKind: 'accepted',
      receiptRef:
        verificationGate.verificationReceiptRefs[0] ?? delivery.acceptanceEventRef,
      workRequestId: delivery.workRequestId,
    })
  }
  return {
    kind: verificationGate.accepted ? 'verified' : 'rejected',
    lifecycleKinds: verificationGate.accepted
      ? ['delivered', 'accepted']
      : ['delivered'],
    paymentMode: 'no-spend',
    verificationGate,
  }
}
