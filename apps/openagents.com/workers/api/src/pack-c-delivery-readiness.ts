import { assertNoProviderSecretMaterial } from '@openagentsinc/provider-account-schema'

import { isoTimestampAfterIso } from './runtime-primitives'

export const PACK_C_DELIVERY_READINESS_VERSION =
  'pack-c-delivery-readiness:v1' as const

const PACK_C_DELIVERY_READINESS_COLLECTION = 'pack_c_delivery_readiness_public'

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PACK_C_DELIVERY_PRIVATE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ]patch/i,
  /raw[-_ ]file/i,
  /raw[-_ ]source/i,
  /raw[-_ ]shell/i,
  /raw[-_ ]command/i,
  /raw[-_ ]prompt/i,
  /raw[-_ ]log/i,
  /private[-_ ]repo/i,
  /private[-_ ]content/i,
  /provider[-_ ]payload/i,
  /wallet|payment[-_ ](?:material|preimage|hash)/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
]

class PackCDeliveryReadinessError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'PackCDeliveryReadinessError'
  }
}

export type PackCDeliveryReadinessVisibility =
  | 'customer'
  | 'operator'
  | 'public'
  | 'team'

export type PackCDeliveryReadinessIdentityStatus = 'blocked' | 'ready' | 'stale'

export type PackCDeliveryReadinessChangeStatus =
  | 'blocked'
  | 'review_ready'
  | 'stale'

export type PackCDeliveryAuthorityBoundary = Readonly<{
  acceptanceAuthority: 'receipt_present' | 'separate_receipt_required'
  agentDeliveryAuthority: 'evidence_only'
  humanMergeAuthority: 'not_delegated'
  marketDeliveryAuthority: 'evidence_only'
  prDraftWritebackAuthority: 'blocked' | 'ready' | 'scoped_exception'
  settlementAuthority: 'receipt_present' | 'separate_receipt_required'
}>

export type PackCDeliveryReadinessInput = Readonly<{
  acceptanceReceiptRefs?: ReadonlyArray<string> | undefined
  agentDeliveryRefs?: ReadonlyArray<string> | undefined
  caveatRefs?: ReadonlyArray<string> | undefined
  changeCaptureRefs: ReadonlyArray<string>
  changeCaptureStatus: PackCDeliveryReadinessChangeStatus
  deliveryReceiptRefs?: ReadonlyArray<string> | undefined
  deliveryRef: string
  generatedAt: string
  githubWritebackAuthorityRefs?: ReadonlyArray<string> | undefined
  humanMergeCaveatRefs?: ReadonlyArray<string> | undefined
  marketDeliveryRefs?: ReadonlyArray<string> | undefined
  observedAt: string
  publicSafe: boolean
  repositoryIdentityRef: string
  repositoryIdentityStatus: PackCDeliveryReadinessIdentityStatus
  reviewRefs?: ReadonlyArray<string> | undefined
  scopedExceptionRef?: string | null | undefined
  settlementReceiptRefs?: ReadonlyArray<string> | undefined
  staleAfterMs: number
  verificationRefs?: ReadonlyArray<string> | undefined
  visibility: PackCDeliveryReadinessVisibility
  worktreeIdentityRef: string
  worktreeIdentityStatus: PackCDeliveryReadinessIdentityStatus
}>

export type PackCDeliveryReadinessProjection = Readonly<{
  acceptanceReceiptRefs: ReadonlyArray<string>
  ageMs: number
  agentDeliveryRefs: ReadonlyArray<string>
  authorityBoundary: PackCDeliveryAuthorityBoundary
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  changeCaptureRefs: ReadonlyArray<string>
  changeCaptureStatus: PackCDeliveryReadinessChangeStatus
  deliveryReadinessVersion: typeof PACK_C_DELIVERY_READINESS_VERSION
  deliveryReceiptRefs: ReadonlyArray<string>
  deliveryRef: string
  freshness: 'fresh' | 'stale'
  generatedAt: string
  githubWritebackAuthorityRefs: ReadonlyArray<string>
  humanMergeCaveatRefs: ReadonlyArray<string>
  marketDeliveryRefs: ReadonlyArray<string>
  observedAt: string
  publicSafe: boolean
  repositoryIdentityRef: string
  repositoryIdentityStatus: PackCDeliveryReadinessIdentityStatus
  reviewRefs: ReadonlyArray<string>
  scopedExceptionRef: string | null
  settlementReceiptRefs: ReadonlyArray<string>
  staleAt: string
  status: 'blocked' | 'ready' | 'scoped_exception'
  verificationRefs: ReadonlyArray<string>
  visibility: PackCDeliveryReadinessVisibility
  worktreeIdentityRef: string
  worktreeIdentityStatus: PackCDeliveryReadinessIdentityStatus
}>

const assertNoPrivateDeliveryMaterial = (
  value: unknown,
  context: string,
): void => {
  assertNoProviderSecretMaterial(value, context)

  const text = typeof value === 'string' ? value : JSON.stringify(value)

  if (PACK_C_DELIVERY_PRIVATE_MARKERS.some(marker => marker.test(text))) {
    throw new PackCDeliveryReadinessError(
      `${context} contains raw patch, raw shell, private repo, local path, or payment material.`,
    )
  }
}

const safeRef = (field: string, value: string): string => {
  const trimmed = value.trim()
  assertNoPrivateDeliveryMaterial(trimmed, field)

  if (!SAFE_REF_PATTERN.test(trimmed)) {
    throw new PackCDeliveryReadinessError(
      `${field} must be a stable Pack C delivery ref.`,
    )
  }

  return trimmed
}

const safeRefs = (
  field: string,
  values: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => (values ?? []).map(value => safeRef(field, value))

const safeOptionalRef = (
  field: string,
  value: string | null | undefined,
): string | null =>
  value === null || value === undefined ? null : safeRef(field, value)

const ageMs = (generatedAt: string, observedAt: string): number => {
  const age = Date.parse(generatedAt) - Date.parse(observedAt)

  return Number.isFinite(age) ? Math.max(0, age) : Number.POSITIVE_INFINITY
}

const staleAt = (observedAt: string, staleAfterMs: number): string =>
  isoTimestampAfterIso(observedAt, Math.max(0, Math.trunc(staleAfterMs)))

const blockers = (
  refs: Readonly<{
    changeCaptureRefs: ReadonlyArray<string>
    changeCaptureStatus: PackCDeliveryReadinessChangeStatus
    deliveryRef: string
    freshness: 'fresh' | 'stale'
    githubWritebackAuthorityRefs: ReadonlyArray<string>
    humanMergeCaveatRefs: ReadonlyArray<string>
    publicSafe: boolean
    repositoryIdentityStatus: PackCDeliveryReadinessIdentityStatus
    reviewRefs: ReadonlyArray<string>
    verificationRefs: ReadonlyArray<string>
    visibility: PackCDeliveryReadinessVisibility
    worktreeIdentityStatus: PackCDeliveryReadinessIdentityStatus
  }>,
): ReadonlyArray<string> => [
  ...(refs.changeCaptureRefs.length === 0
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:missing-change-capture`,
      ]
    : []),
  ...(refs.githubWritebackAuthorityRefs.length === 0
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:missing-writeback-authority`,
      ]
    : []),
  ...(refs.verificationRefs.length === 0
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:missing-verification`,
      ]
    : []),
  ...(refs.reviewRefs.length === 0
    ? [`pack-c-delivery-readiness-blocker:${refs.deliveryRef}:missing-review`]
    : []),
  ...(refs.humanMergeCaveatRefs.length === 0
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:missing-human-merge-caveat`,
      ]
    : []),
  ...(refs.repositoryIdentityStatus === 'stale'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:stale-repository-identity`,
      ]
    : []),
  ...(refs.repositoryIdentityStatus === 'blocked'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:blocked-repository-identity`,
      ]
    : []),
  ...(refs.worktreeIdentityStatus === 'stale'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:stale-worktree-identity`,
      ]
    : []),
  ...(refs.worktreeIdentityStatus === 'blocked'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:blocked-worktree-identity`,
      ]
    : []),
  ...(refs.changeCaptureStatus === 'stale'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:stale-change-capture`,
      ]
    : []),
  ...(refs.changeCaptureStatus === 'blocked'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:blocked-change-capture`,
      ]
    : []),
  ...(refs.freshness === 'stale'
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:stale-delivery-readiness`,
      ]
    : []),
  ...(refs.visibility === 'public' && !refs.publicSafe
    ? [
        `pack-c-delivery-readiness-blocker:${refs.deliveryRef}:unsafe-public-visibility`,
      ]
    : []),
]

const authorityBoundary = (
  status: PackCDeliveryReadinessProjection['status'],
  acceptanceReceiptRefs: ReadonlyArray<string>,
  settlementReceiptRefs: ReadonlyArray<string>,
): PackCDeliveryAuthorityBoundary => ({
  acceptanceAuthority:
    acceptanceReceiptRefs.length > 0
      ? 'receipt_present'
      : 'separate_receipt_required',
  agentDeliveryAuthority: 'evidence_only',
  humanMergeAuthority: 'not_delegated',
  marketDeliveryAuthority: 'evidence_only',
  prDraftWritebackAuthority: status,
  settlementAuthority:
    settlementReceiptRefs.length > 0
      ? 'receipt_present'
      : 'separate_receipt_required',
})

export const projectPackCDeliveryReadiness = (
  input: PackCDeliveryReadinessInput,
): PackCDeliveryReadinessProjection => {
  assertNoPrivateDeliveryMaterial(input, 'pack-c-delivery-readiness.input')

  const deliveryRef = safeRef(
    'pack-c-delivery-readiness.deliveryRef',
    input.deliveryRef,
  )
  const acceptanceReceiptRefs = safeRefs(
    'pack-c-delivery-readiness.acceptanceReceiptRefs',
    input.acceptanceReceiptRefs,
  )
  const agentDeliveryRefs = safeRefs(
    'pack-c-delivery-readiness.agentDeliveryRefs',
    input.agentDeliveryRefs,
  )
  const caveatRefs = safeRefs(
    'pack-c-delivery-readiness.caveatRefs',
    input.caveatRefs,
  )
  const changeCaptureRefs = safeRefs(
    'pack-c-delivery-readiness.changeCaptureRefs',
    input.changeCaptureRefs,
  )
  const deliveryReceiptRefs = safeRefs(
    'pack-c-delivery-readiness.deliveryReceiptRefs',
    input.deliveryReceiptRefs,
  )
  const githubWritebackAuthorityRefs = safeRefs(
    'pack-c-delivery-readiness.githubWritebackAuthorityRefs',
    input.githubWritebackAuthorityRefs,
  )
  const humanMergeCaveatRefs = safeRefs(
    'pack-c-delivery-readiness.humanMergeCaveatRefs',
    input.humanMergeCaveatRefs,
  )
  const marketDeliveryRefs = safeRefs(
    'pack-c-delivery-readiness.marketDeliveryRefs',
    input.marketDeliveryRefs,
  )
  const reviewRefs = safeRefs(
    'pack-c-delivery-readiness.reviewRefs',
    input.reviewRefs,
  )
  const settlementReceiptRefs = safeRefs(
    'pack-c-delivery-readiness.settlementReceiptRefs',
    input.settlementReceiptRefs,
  )
  const verificationRefs = safeRefs(
    'pack-c-delivery-readiness.verificationRefs',
    input.verificationRefs,
  )
  const repositoryIdentityRef = safeRef(
    'pack-c-delivery-readiness.repositoryIdentityRef',
    input.repositoryIdentityRef,
  )
  const worktreeIdentityRef = safeRef(
    'pack-c-delivery-readiness.worktreeIdentityRef',
    input.worktreeIdentityRef,
  )
  const observedAgeMs = ageMs(input.generatedAt, input.observedAt)
  const freshness =
    Number.isFinite(observedAgeMs) && observedAgeMs <= input.staleAfterMs
      ? 'fresh'
      : 'stale'
  const scopedExceptionRef = safeOptionalRef(
    'pack-c-delivery-readiness.scopedExceptionRef',
    input.scopedExceptionRef,
  )
  const blockerRefs = blockers({
    changeCaptureRefs,
    changeCaptureStatus: input.changeCaptureStatus,
    deliveryRef,
    freshness,
    githubWritebackAuthorityRefs,
    humanMergeCaveatRefs,
    publicSafe: input.publicSafe,
    repositoryIdentityStatus: input.repositoryIdentityStatus,
    reviewRefs,
    verificationRefs,
    visibility: input.visibility,
    worktreeIdentityStatus: input.worktreeIdentityStatus,
  })
  const status =
    blockerRefs.length === 0
      ? 'ready'
      : scopedExceptionRef === null
        ? 'blocked'
        : 'scoped_exception'
  const projection: PackCDeliveryReadinessProjection = {
    acceptanceReceiptRefs,
    ageMs: observedAgeMs,
    agentDeliveryRefs,
    authorityBoundary: authorityBoundary(
      status,
      acceptanceReceiptRefs,
      settlementReceiptRefs,
    ),
    blockerRefs,
    caveatRefs,
    changeCaptureRefs,
    changeCaptureStatus: input.changeCaptureStatus,
    deliveryReadinessVersion: PACK_C_DELIVERY_READINESS_VERSION,
    deliveryReceiptRefs,
    deliveryRef,
    freshness,
    generatedAt: input.generatedAt,
    githubWritebackAuthorityRefs,
    humanMergeCaveatRefs,
    marketDeliveryRefs,
    observedAt: input.observedAt,
    publicSafe: input.publicSafe,
    repositoryIdentityRef,
    repositoryIdentityStatus: input.repositoryIdentityStatus,
    reviewRefs,
    scopedExceptionRef,
    settlementReceiptRefs,
    staleAt: staleAt(input.observedAt, input.staleAfterMs),
    status,
    verificationRefs,
    visibility: input.visibility,
    worktreeIdentityRef,
    worktreeIdentityStatus: input.worktreeIdentityStatus,
  }

  assertNoPrivateDeliveryMaterial(
    projection,
    PACK_C_DELIVERY_READINESS_COLLECTION,
  )

  return projection
}
