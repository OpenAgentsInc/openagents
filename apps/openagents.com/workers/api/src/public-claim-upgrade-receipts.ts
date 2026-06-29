import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  PublicClaimKind,
  PublicClaimState,
} from './public-claim-state'
import {
  PublicClaimProjectionAudience,
} from './public-claim-projections'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const PublicClaimUpgradeEvidenceKind = S.Literals([
  'accepted_work_settlement',
  'buyer_payment',
  'measurement',
  'operator_approval',
  'site_checkout',
  'source_authority',
  'verification',
])
export type PublicClaimUpgradeEvidenceKind =
  typeof PublicClaimUpgradeEvidenceKind.Type

export const PublicClaimUpgradeReceiptStatus = S.Literals([
  'accepted',
  'blocked',
])
export type PublicClaimUpgradeReceiptStatus =
  typeof PublicClaimUpgradeReceiptStatus.Type

export const PublicClaimUpgradeEvidenceRef = S.Struct({
  evidenceKind: PublicClaimUpgradeEvidenceKind,
  evidenceRef: S.String,
})
export type PublicClaimUpgradeEvidenceRef =
  typeof PublicClaimUpgradeEvidenceRef.Type

export const PublicClaimUpgradeRequest = S.Struct({
  approverRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  createdAt: S.String,
  evidenceRefs: S.Array(PublicClaimUpgradeEvidenceRef),
  idempotencyKey: S.String,
  previousState: PublicClaimState,
  requestedState: PublicClaimState,
  sourceAuthorityRefs: S.Array(S.String),
})
export type PublicClaimUpgradeRequest =
  typeof PublicClaimUpgradeRequest.Type

export const PublicClaimUpgradeReceipt = S.Struct({
  approverRefs: S.Array(S.String),
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  createdAt: S.String,
  denialRefs: S.Array(S.String),
  evidenceRefs: S.Array(PublicClaimUpgradeEvidenceRef),
  idempotencyKey: S.String,
  missingEvidenceRefs: S.Array(S.String),
  nextState: PublicClaimState,
  previousState: PublicClaimState,
  receiptId: S.String,
  requestedState: PublicClaimState,
  requiredEvidenceRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: PublicClaimUpgradeReceiptStatus,
})
export type PublicClaimUpgradeReceipt =
  typeof PublicClaimUpgradeReceipt.Type

export const PublicClaimUpgradeReceiptProjection = S.Struct({
  approverRefs: S.Array(S.String),
  audience: PublicClaimProjectionAudience,
  claimId: S.String,
  claimKind: PublicClaimKind,
  claimRef: S.String,
  createdAt: S.String,
  denialRefs: S.Array(S.String),
  evidenceRefs: S.Array(PublicClaimUpgradeEvidenceRef),
  idempotencyKeyRef: S.String,
  missingEvidenceRefs: S.Array(S.String),
  nextState: PublicClaimState,
  previousState: PublicClaimState,
  receiptId: S.String,
  requestedState: PublicClaimState,
  requiredEvidenceRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: PublicClaimUpgradeReceiptStatus,
})
export type PublicClaimUpgradeReceiptProjection =
  typeof PublicClaimUpgradeReceiptProjection.Type

export class PublicClaimUpgradeReceiptUnsafe extends S.TaggedErrorClass<PublicClaimUpgradeReceiptUnsafe>()(
  'PublicClaimUpgradeReceiptUnsafe',
  {
    reason: S.String,
  },
) {}

const stateRank: Record<PublicClaimState, number> = {
  blocked: 0,
  planned: 0,
  modeled: 1,
  measured: 2,
  prohibited: 0,
  verified: 3,
  settled: 4,
}

const requiredEvidenceKindsByState: Record<
  PublicClaimState,
  ReadonlyArray<PublicClaimUpgradeEvidenceKind>
> = {
  blocked: [],
  planned: [],
  modeled: ['source_authority'],
  measured: ['measurement'],
  prohibited: [],
  verified: ['verification', 'operator_approval'],
  settled: ['accepted_work_settlement', 'operator_approval'],
}

const requiredEvidenceRefByKind: Record<PublicClaimUpgradeEvidenceKind, string> =
  {
    accepted_work_settlement: 'required.accepted_work_settlement_receipt',
    buyer_payment: 'required.buyer_payment_receipt',
    measurement: 'required.measurement_evidence',
    operator_approval: 'required.operator_approval_ref',
    site_checkout: 'required.site_checkout_receipt',
    source_authority: 'required.source_authority_ref',
    verification: 'required.verification_evidence',
  }

const missingEvidenceDenialRefByKind:
  Record<PublicClaimUpgradeEvidenceKind, string> = {
    accepted_work_settlement:
      'denial.missing.accepted_work_settlement_receipt',
    buyer_payment: 'denial.missing.buyer_payment_receipt',
    measurement: 'denial.missing.measurement_evidence',
    operator_approval: 'denial.missing.operator_approval_ref',
    site_checkout: 'denial.missing.site_checkout_receipt',
    source_authority: 'denial.missing.source_authority_ref',
    verification: 'denial.missing.verification_evidence',
  }

const nonSettlementPaymentKinds: ReadonlyArray<PublicClaimUpgradeEvidenceKind> =
  ['buyer_payment', 'site_checkout']

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeValuePattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|token|wallet|workroom[_-]?private)/i

const valueHasPrivateMaterial = (value: unknown): boolean => {
  if (typeof value === 'string') {
    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
  }

  if (Array.isArray(value)) {
    return value.some(valueHasPrivateMaterial)
  }

  if (value !== null && typeof value === 'object') {
    return openAgentsRunnerGatewayPayloadHasPrivateMaterial(value) ||
      Object.values(value).some(valueHasPrivateMaterial)
  }

  return false
}

const safeRef = (ref: string): string | undefined => {
  const trimmed = ref.trim()

  return trimmed !== '' &&
    safeRefPattern.test(trimmed) &&
    !valueHasPrivateMaterial(trimmed)
    ? trimmed
    : undefined
}

const safeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const projected = [...new Set(refs)].map(safeRef)

  if (projected.some(ref => ref === undefined)) {
    throw new PublicClaimUpgradeReceiptUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, operator-only, or workroom material.`,
    })
  }

  return projected.filter((ref): ref is string => ref !== undefined).sort()
}

const safeEvidenceRefs = (
  refs: ReadonlyArray<PublicClaimUpgradeEvidenceRef>,
): ReadonlyArray<PublicClaimUpgradeEvidenceRef> =>
  refs.map(ref => ({
    evidenceKind: ref.evidenceKind,
    evidenceRef: safeRefs('claim upgrade evidence refs', [ref.evidenceRef])[0]!,
  })).sort((left, right) =>
    `${left.evidenceKind}:${left.evidenceRef}`.localeCompare(
      `${right.evidenceKind}:${right.evidenceRef}`,
    ),
  )

const hasEvidenceKind = (
  input: PublicClaimUpgradeRequest,
  evidenceKind: PublicClaimUpgradeEvidenceKind,
): boolean => {
  if (evidenceKind === 'operator_approval') {
    return input.approverRefs.length > 0 ||
      input.evidenceRefs.some(ref => ref.evidenceKind === evidenceKind)
  }

  if (evidenceKind === 'source_authority') {
    return input.sourceAuthorityRefs.length > 0 ||
      input.evidenceRefs.some(ref => ref.evidenceKind === evidenceKind)
  }

  return input.evidenceRefs.some(ref => ref.evidenceKind === evidenceKind)
}

const missingEvidenceKinds = (
  input: PublicClaimUpgradeRequest,
): ReadonlyArray<PublicClaimUpgradeEvidenceKind> =>
  requiredEvidenceKindsByState[input.requestedState]
    .filter(evidenceKind => !hasEvidenceKind(input, evidenceKind))

const hasOnlyNonSettlementPaymentEvidence = (
  input: PublicClaimUpgradeRequest,
): boolean =>
  input.requestedState === 'settled' &&
  input.evidenceRefs.some(ref =>
    nonSettlementPaymentKinds.includes(ref.evidenceKind),
  ) &&
  !hasEvidenceKind(input, 'accepted_work_settlement')

const requestIsUpgrade = (input: PublicClaimUpgradeRequest): boolean =>
  stateRank[input.requestedState] > stateRank[input.previousState]

const receiptIdForRequest = (
  input: PublicClaimUpgradeRequest,
): string =>
  `receipt:public_claim_upgrade:${input.claimId}:${input.requestedState}:${input.createdAt}`

const assertRequestSafe = (input: PublicClaimUpgradeRequest): void => {
  safeRefs('claim upgrade refs', [
    input.claimId,
    input.claimRef,
    input.idempotencyKey,
  ])
  safeRefs('approver refs', input.approverRefs)
  safeRefs('source authority refs', input.sourceAuthorityRefs)
  safeEvidenceRefs(input.evidenceRefs)

  if (valueHasPrivateMaterial(input.createdAt)) {
    throw new PublicClaimUpgradeReceiptUnsafe({
      reason: 'createdAt contains private material.',
    })
  }
}

export const createPublicClaimUpgradeReceipt = (
  input: PublicClaimUpgradeRequest,
): PublicClaimUpgradeReceipt => {
  assertRequestSafe(input)

  const requiredEvidenceRefs =
    requiredEvidenceKindsByState[input.requestedState]
      .map(evidenceKind => requiredEvidenceRefByKind[evidenceKind])
  const missingKinds = missingEvidenceKinds(input)
  const missingEvidenceRefs = missingKinds
    .map(evidenceKind => requiredEvidenceRefByKind[evidenceKind])
  const baseDenialRefs = missingKinds
    .map(evidenceKind => missingEvidenceDenialRefByKind[evidenceKind])
  const settlementDenialRefs = hasOnlyNonSettlementPaymentEvidence(input)
    ? ['denial.settlement_requires_accepted_work_receipt']
    : []
  const denialRefs = [...baseDenialRefs, ...settlementDenialRefs].sort()
  const status = requestIsUpgrade(input) && denialRefs.length > 0
    ? 'blocked'
    : 'accepted'
  const nextState = status === 'accepted'
    ? input.requestedState
    : input.previousState
  const receipt: PublicClaimUpgradeReceipt = {
    approverRefs: safeRefs('approver refs', input.approverRefs),
    claimId: safeRef(input.claimId) ?? input.claimId,
    claimKind: input.claimKind,
    claimRef: safeRef(input.claimRef) ?? input.claimRef,
    createdAt: input.createdAt,
    denialRefs,
    evidenceRefs: safeEvidenceRefs(input.evidenceRefs),
    idempotencyKey: safeRef(input.idempotencyKey) ?? input.idempotencyKey,
    missingEvidenceRefs,
    nextState,
    previousState: input.previousState,
    receiptId: receiptIdForRequest(input),
    requestedState: input.requestedState,
    requiredEvidenceRefs,
    sourceAuthorityRefs: safeRefs(
      'source authority refs',
      input.sourceAuthorityRefs,
    ),
    status,
  }

  if (valueHasPrivateMaterial(receipt)) {
    throw new PublicClaimUpgradeReceiptUnsafe({
      reason: 'Public claim upgrade receipt contains private material.',
    })
  }

  return receipt
}

export const resolvePublicClaimUpgradeReceipt = (
  input: PublicClaimUpgradeRequest,
  existingReceipts: ReadonlyArray<PublicClaimUpgradeReceipt>,
): PublicClaimUpgradeReceipt =>
  existingReceipts.find(receipt =>
    receipt.idempotencyKey === input.idempotencyKey,
  ) ?? createPublicClaimUpgradeReceipt(input)

const publicSourceAuthorityRefs = (
  refs: ReadonlyArray<string>,
  audience: PublicClaimProjectionAudience,
): ReadonlyArray<string> => {
  if (audience === 'operator') {
    return safeRefs('source authority refs', refs)
  }

  if (audience === 'team') {
    return safeRefs(
      'source authority refs',
      refs.filter(ref => !ref.startsWith('operator_ref.')),
    )
  }

  return safeRefs(
    'source authority refs',
    refs.filter(ref =>
      !ref.startsWith('operator_ref.') &&
      !ref.startsWith('team_ref.'),
    ),
  )
}

export const projectPublicClaimUpgradeReceipt = (
  receipt: PublicClaimUpgradeReceipt,
  audience: PublicClaimProjectionAudience,
): PublicClaimUpgradeReceiptProjection => {
  const projection: PublicClaimUpgradeReceiptProjection = {
    approverRefs: audience === 'operator'
      ? safeRefs('approver refs', receipt.approverRefs)
      : [],
    audience,
    claimId: safeRef(receipt.claimId) ?? receipt.claimId,
    claimKind: receipt.claimKind,
    claimRef: safeRef(receipt.claimRef) ?? receipt.claimRef,
    createdAt: receipt.createdAt,
    denialRefs: safeRefs('denial refs', receipt.denialRefs),
    evidenceRefs: safeEvidenceRefs(receipt.evidenceRefs),
    idempotencyKeyRef: `idempotency:${receipt.receiptId}`,
    missingEvidenceRefs: safeRefs(
      'missing evidence refs',
      receipt.missingEvidenceRefs,
    ),
    nextState: receipt.nextState,
    previousState: receipt.previousState,
    receiptId: safeRef(receipt.receiptId) ?? receipt.receiptId,
    requestedState: receipt.requestedState,
    requiredEvidenceRefs: safeRefs(
      'required evidence refs',
      receipt.requiredEvidenceRefs,
    ),
    sourceAuthorityRefs: publicSourceAuthorityRefs(
      receipt.sourceAuthorityRefs,
      audience,
    ),
    status: receipt.status,
  }

  if (valueHasPrivateMaterial(projection)) {
    throw new PublicClaimUpgradeReceiptUnsafe({
      reason: 'Public claim upgrade receipt projection contains private material.',
    })
  }

  return projection
}

export const publicClaimUpgradeReceiptHasPrivateMaterial =
  valueHasPrivateMaterial
