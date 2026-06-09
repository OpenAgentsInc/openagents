import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonPayoutTargetAdmissionState = S.Literals([
  'heartbeat_hint_only',
  'missing',
  'pending_registration',
  'registered',
  'rejected',
  'revoked',
  'stale',
])
export type PylonPayoutTargetAdmissionState =
  typeof PylonPayoutTargetAdmissionState.Type

export const PylonPayoutTargetKind = S.Literals([
  'bip353_name',
  'bolt11_invoice',
  'bolt12_offer',
  'lnurl_pay',
  'none',
  'unknown',
  'unsupported',
])
export type PylonPayoutTargetKind = typeof PylonPayoutTargetKind.Type

export const PylonPayoutTargetOwnership = S.Literals([
  'external_override',
  'heartbeat_hint_only',
  'unknown',
  'wallet_owned',
])
export type PylonPayoutTargetOwnership =
  typeof PylonPayoutTargetOwnership.Type

export const PylonPayoutTargetVisibility = S.Literals(['private', 'public'])
export type PylonPayoutTargetVisibility =
  typeof PylonPayoutTargetVisibility.Type

export const PylonPayoutTargetAdmissionAuthorityBoundary = S.Literals([
  'read_only_projection',
])
export type PylonPayoutTargetAdmissionAuthorityBoundary =
  typeof PylonPayoutTargetAdmissionAuthorityBoundary.Type

export class PylonPayoutTargetAdmissionAuthority extends S.Class<PylonPayoutTargetAdmissionAuthority>(
  'PylonPayoutTargetAdmissionAuthority',
)({
  authorityBoundary: PylonPayoutTargetAdmissionAuthorityBoundary,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetDisclosure: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noProviderEligibilityMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class PylonPayoutTargetAdmissionRecord extends S.Class<PylonPayoutTargetAdmissionRecord>(
  'PylonPayoutTargetAdmissionRecord',
)({
  admissionState: PylonPayoutTargetAdmissionState,
  authority: PylonPayoutTargetAdmissionAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  heartbeatRefs: S.Array(S.String),
  id: S.String,
  ownerRef: S.String,
  ownerVisibility: PylonPayoutTargetVisibility,
  providerRef: S.String,
  providerVisibility: PylonPayoutTargetVisibility,
  registrationRefs: S.Array(S.String),
  rejectionRefs: S.Array(S.String),
  revocationRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleRefs: S.Array(S.String),
  targetFingerprintRef: S.NullOr(S.String),
  targetKind: PylonPayoutTargetKind,
  targetOwnership: PylonPayoutTargetOwnership,
  targetVerificationRefs: S.Array(S.String),
  targetVisibility: PylonPayoutTargetVisibility,
  updatedAtIso: S.String,
}) {}

export class PylonPayoutTargetAdmissionProjection extends S.Class<PylonPayoutTargetAdmissionProjection>(
  'PylonPayoutTargetAdmissionProjection',
)({
  admissionState: PylonPayoutTargetAdmissionState,
  admissionStateLabel: S.String,
  audience: OmniProjectionAudience,
  authority: PylonPayoutTargetAdmissionAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  heartbeatHintOnly: S.Boolean,
  heartbeatRefs: S.Array(S.String),
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  ownerRef: S.String,
  ownerVisibility: PylonPayoutTargetVisibility,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutTargetDisclosureAllowed: S.Boolean,
  payoutTargetMutationAllowed: S.Boolean,
  providerEligibilityMutationAllowed: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonPayoutTargetVisibility,
  registeredPayoutTargetClaimAllowed: S.Boolean,
  registrationRefs: S.Array(S.String),
  rejectionRefs: S.Array(S.String),
  revocationRefs: S.Array(S.String),
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  staleRefs: S.Array(S.String),
  targetFingerprintRef: S.NullOr(S.String),
  targetKind: PylonPayoutTargetKind,
  targetKindLabel: S.String,
  targetOwnership: PylonPayoutTargetOwnership,
  targetOwnershipLabel: S.String,
  targetVerificationRefs: S.Array(S.String),
  targetVisibility: PylonPayoutTargetVisibility,
  updatedAtDisplay: S.String,
}) {}

export class PylonPayoutTargetAdmissionUnsafe extends S.TaggedErrorClass<PylonPayoutTargetAdmissionUnsafe>()(
  'PylonPayoutTargetAdmissionUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_PAYOUT_TARGET_ADMISSION_READ_ONLY_AUTHORITY:
  PylonPayoutTargetAdmissionAuthority = {
    authorityBoundary: 'read_only_projection',
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetDisclosure: true,
    noPayoutTargetMutation: true,
    noProviderEligibilityMutation: true,
    noSettlementMutation: true,
  }

const admissionLabelByState:
  Readonly<Record<PylonPayoutTargetAdmissionState, string>> = {
    heartbeat_hint_only: 'Heartbeat hint only',
    missing: 'Missing',
    pending_registration: 'Pending registration',
    registered: 'Registered',
    rejected: 'Rejected',
    revoked: 'Revoked',
    stale: 'Stale',
  }

const targetKindLabelByKind: Readonly<Record<PylonPayoutTargetKind, string>> = {
  bip353_name: 'BIP353 name',
  bolt11_invoice: 'BOLT11 invoice',
  bolt12_offer: 'BOLT12 offer',
  lnurl_pay: 'LNURL pay',
  none: 'None',
  unknown: 'Unknown',
  unsupported: 'Unsupported',
}

const targetOwnershipLabelByOwnership:
  Readonly<Record<PylonPayoutTargetOwnership, string>> = {
    external_override: 'External override',
    heartbeat_hint_only: 'Heartbeat hint only',
    unknown: 'Unknown',
    wallet_owned: 'Wallet owned',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafePayoutTargetAdmissionRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|lnurlp:|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|payout)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|invoice|payload|payment|payout|prompt|provider|runner|run[_-]?log|state|target|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(blocker\.private|caveat\.private|heartbeat\.private|owner\.private|provider\.private|registration\.private|rejection\.private|revocation\.private|source\.private|stale\.private|target\.private|verification\.private)/i
const customerUnsafeRefPattern =
  /(blocker\.private|caveat\.private|heartbeat\.private|owner\.private|provider\.private|registration\.private|rejection\.private|revocation\.private|source\.private|stale\.private|target\.private|verification\.private)/i
const teamUnsafeRefPattern =
  /(owner\.private|provider\.private|target\.private|verification\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafePayoutTargetAdmissionRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: `${label} contains raw payout target material, private payout identifiers, wallet material, channel monitor state, payment material, provider secrets, bearer/API credentials, customer data, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const visibleRef = (
  label: string,
  ref: string,
  visibility: PylonPayoutTargetVisibility,
  redactedRef: string,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    visibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience(label, [ref], audience)[0] ?? redactedRef
  }

  return redactedRef
}

const visibleOptionalRef = (
  label: string,
  ref: string | null,
  visibility: PylonPayoutTargetVisibility,
  audience: typeof OmniProjectionAudience.Type,
): string | null => {
  if (ref === null) {
    return null
  }

  if (
    visibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience(label, [ref], audience)[0] ?? null
  }

  return null
}

export const pylonPayoutTargetAdmissionHasNoSettlementAuthority = (
  authority: PylonPayoutTargetAdmissionAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_projection' &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetDisclosure &&
  authority.noPayoutTargetMutation &&
  authority.noProviderEligibilityMutation &&
  authority.noSettlementMutation

export const pylonPayoutTargetAdmissionCanMutateProviderEligibility = (
  record: PylonPayoutTargetAdmissionRecord,
): boolean =>
  !pylonPayoutTargetAdmissionHasNoSettlementAuthority(record.authority)

export const pylonPayoutTargetAdmissionClaimAllowed = (
  record: PylonPayoutTargetAdmissionRecord,
): boolean =>
  record.admissionState === 'registered' &&
  record.targetKind !== 'none' &&
  record.targetKind !== 'unknown' &&
  record.targetKind !== 'unsupported' &&
  record.targetOwnership === 'wallet_owned' &&
  record.targetFingerprintRef !== null &&
  record.registrationRefs.length > 0 &&
  record.targetVerificationRefs.length > 0

const assertRecordSafe = (
  record: PylonPayoutTargetAdmissionRecord,
): void => {
  assertSafeRefs('payout target admission identity refs', [
    record.id,
    record.ownerRef,
    record.providerRef,
  ])
  assertSafeRefs('payout target admission heartbeat refs', record.heartbeatRefs)
  assertSafeRefs(
    'payout target admission registration refs',
    record.registrationRefs,
  )
  assertSafeRefs(
    'payout target admission verification refs',
    record.targetVerificationRefs,
  )
  assertSafeRefs('payout target admission rejection refs', record.rejectionRefs)
  assertSafeRefs('payout target admission revocation refs', record.revocationRefs)
  assertSafeRefs('payout target admission stale refs', record.staleRefs)
  assertSafeRefs('payout target admission blocker refs', record.blockerRefs)
  assertSafeRefs('payout target admission caveat refs', record.caveatRefs)
  assertSafeRefs('payout target admission evidence refs', record.evidenceRefs)
  assertSafeRefs('payout target admission source refs', record.sourceRefs)
  assertSafeRefs(
    'payout target admission fingerprint refs',
    record.targetFingerprintRef === null ? [] : [record.targetFingerprintRef],
  )

  if (!pylonPayoutTargetAdmissionHasNoSettlementAuthority(record.authority)) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Pylon payout target admission is read-only and cannot carry live wallet spend, payout dispatch, payout target disclosure/mutation, provider eligibility mutation, or settlement authority.',
    })
  }

  if (
    record.admissionState === 'heartbeat_hint_only' &&
    record.heartbeatRefs.length === 0
  ) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Heartbeat-only payout target hints require heartbeat refs.',
    })
  }

  if (
    record.admissionState === 'pending_registration' &&
    record.registrationRefs.length === 0
  ) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Pending payout target registration requires registration refs.',
    })
  }

  if (
    record.admissionState === 'registered' &&
    !pylonPayoutTargetAdmissionClaimAllowed(record)
  ) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Registered payout target claims require wallet-owned supported target kind, safe target fingerprint, registration refs, and verification refs.',
    })
  }

  if (record.admissionState === 'rejected' && record.rejectionRefs.length === 0) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Rejected payout target admission requires rejection refs.',
    })
  }

  if (record.admissionState === 'revoked' && record.revocationRefs.length === 0) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Revoked payout target admission requires revocation refs.',
    })
  }

  if (record.admissionState === 'stale' && record.staleRefs.length === 0) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Stale payout target admission requires stale refs.',
    })
  }

  if (
    record.admissionState === 'missing' &&
    record.targetFingerprintRef !== null
  ) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Missing payout target admission cannot carry a target fingerprint.',
    })
  }
}

const projectionText = (
  projection: PylonPayoutTargetAdmissionProjection,
): string =>
  [
    projection.id,
    projection.ownerRef,
    projection.providerRef,
    projection.targetFingerprintRef ?? '',
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
    ...projection.heartbeatRefs,
    ...projection.registrationRefs,
    ...projection.rejectionRefs,
    ...projection.revocationRefs,
    ...projection.sourceRefs,
    ...projection.staleRefs,
    ...projection.targetVerificationRefs,
  ].join(' ')

export const pylonPayoutTargetAdmissionProjectionHasPrivateMaterial = (
  projection: PylonPayoutTargetAdmissionProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafePayoutTargetAdmissionRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonPayoutTargetAdmission = (
  record: PylonPayoutTargetAdmissionRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonPayoutTargetAdmissionProjection => {
  assertRecordSafe(record)

  const projection: PylonPayoutTargetAdmissionProjection = {
    admissionState: record.admissionState,
    admissionStateLabel: admissionLabelByState[record.admissionState],
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'payout target admission blocker refs',
      record.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'payout target admission caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'payout target admission evidence refs',
      record.evidenceRefs,
      audience,
    ),
    heartbeatHintOnly: record.admissionState === 'heartbeat_hint_only',
    heartbeatRefs: safeRefsForAudience(
      'payout target admission heartbeat refs',
      record.heartbeatRefs,
      audience,
    ),
    id: safeRefsForAudience(
      'payout target admission id',
      [record.id],
      audience,
    )[0] ?? 'payout_target_admission.redacted',
    liveWalletSpendAllowed: false,
    ownerRef: visibleRef(
      'payout target admission owner ref',
      record.ownerRef,
      record.ownerVisibility,
      'owner.redacted',
      audience,
    ),
    ownerVisibility: record.ownerVisibility,
    payoutDispatchMutationAllowed: false,
    payoutTargetDisclosureAllowed: false,
    payoutTargetMutationAllowed: false,
    providerEligibilityMutationAllowed: false,
    providerRef: visibleRef(
      'payout target admission provider ref',
      record.providerRef,
      record.providerVisibility,
      'provider.redacted',
      audience,
    ),
    providerVisibility: record.providerVisibility,
    registeredPayoutTargetClaimAllowed:
      pylonPayoutTargetAdmissionClaimAllowed(record),
    registrationRefs: safeRefsForAudience(
      'payout target admission registration refs',
      record.registrationRefs,
      audience,
    ),
    rejectionRefs: safeRefsForAudience(
      'payout target admission rejection refs',
      record.rejectionRefs,
      audience,
    ),
    revocationRefs: safeRefsForAudience(
      'payout target admission revocation refs',
      record.revocationRefs,
      audience,
    ),
    settlementMutationAllowed: false,
    sourceRefs: safeRefsForAudience(
      'payout target admission source refs',
      record.sourceRefs,
      audience,
    ),
    staleRefs: safeRefsForAudience(
      'payout target admission stale refs',
      record.staleRefs,
      audience,
    ),
    targetFingerprintRef: visibleOptionalRef(
      'payout target admission fingerprint ref',
      record.targetFingerprintRef,
      record.targetVisibility,
      audience,
    ),
    targetKind: record.targetKind,
    targetKindLabel: targetKindLabelByKind[record.targetKind],
    targetOwnership: record.targetOwnership,
    targetOwnershipLabel:
      targetOwnershipLabelByOwnership[record.targetOwnership],
    targetVerificationRefs: safeRefsForAudience(
      'payout target admission verification refs',
      record.targetVerificationRefs,
      audience,
    ),
    targetVisibility: record.targetVisibility,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (pylonPayoutTargetAdmissionProjectionHasPrivateMaterial(projection)) {
    throw new PylonPayoutTargetAdmissionUnsafe({
      reason: 'Pylon payout target admission projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_PAYOUT_TARGET_ADMISSION_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonPayoutTargetAdmissionRecord> = [
    {
      admissionState: 'registered',
      authority: PYLON_PAYOUT_TARGET_ADMISSION_READ_ONLY_AUTHORITY,
      blockerRefs: [],
      caveatRefs: ['caveat.public.bolt12_preferred'],
      createdAtIso: '2026-06-07T05:00:00.000Z',
      evidenceRefs: ['evidence.public.nexus_target_registration_1'],
      heartbeatRefs: ['heartbeat.public.pylon_provider_1'],
      id: 'pylon_payout_target_admission.provider_1',
      ownerRef: 'owner.private.user_1',
      ownerVisibility: 'private',
      providerRef: 'provider.private.pylon_1',
      providerVisibility: 'private',
      registrationRefs: [
        'registration.public.ldk_target_1',
        'registration.private.operator_trace_1',
      ],
      rejectionRefs: [],
      revocationRefs: [],
      sourceRefs: ['source.public.pylon_v0_2_registration_1'],
      staleRefs: [],
      targetFingerprintRef: 'target_hash.public.bolt12_abc123',
      targetKind: 'bolt12_offer',
      targetOwnership: 'wallet_owned',
      targetVerificationRefs: [
        'verification.public.ownership_signature_1',
        'verification.private.operator_trace_1',
      ],
      targetVisibility: 'public',
      updatedAtIso: '2026-06-07T05:45:00.000Z',
    },
    {
      admissionState: 'heartbeat_hint_only',
      authority: PYLON_PAYOUT_TARGET_ADMISSION_READ_ONLY_AUTHORITY,
      blockerRefs: ['blocker.public.requires_ldk_v0_2_target'],
      caveatRefs: ['caveat.public.heartbeat_is_not_paid_eligibility'],
      createdAtIso: '2026-06-07T05:20:00.000Z',
      evidenceRefs: ['evidence.public.heartbeat_seen_2'],
      heartbeatRefs: ['heartbeat.public.pylon_provider_2'],
      id: 'pylon_payout_target_admission.provider_2',
      ownerRef: 'owner.public.user_2',
      ownerVisibility: 'public',
      providerRef: 'provider.public.pylon_2',
      providerVisibility: 'public',
      registrationRefs: [],
      rejectionRefs: [],
      revocationRefs: [],
      sourceRefs: ['source.public.pylon_heartbeat_2'],
      staleRefs: [],
      targetFingerprintRef: null,
      targetKind: 'unknown',
      targetOwnership: 'heartbeat_hint_only',
      targetVerificationRefs: [],
      targetVisibility: 'private',
      updatedAtIso: '2026-06-07T05:50:00.000Z',
    },
  ]
