import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonAcceptedWorkPayoutSloState = S.Literals([
  'attention_required',
  'blocked',
  'confirmation_observed',
  'dispatch_recorded',
  'dispatch_requested',
  'failed',
  'settled',
  'skipped',
  'stale',
  'verification_complete',
])
export type PylonAcceptedWorkPayoutSloState =
  typeof PylonAcceptedWorkPayoutSloState.Type

export const PylonAcceptedWorkPayoutSloFreshness = S.Literals([
  'expired',
  'fresh',
  'stale',
  'unknown',
])
export type PylonAcceptedWorkPayoutSloFreshness =
  typeof PylonAcceptedWorkPayoutSloFreshness.Type

export const PylonAcceptedWorkPayoutSloVisibility = S.Literals([
  'private',
  'public',
])
export type PylonAcceptedWorkPayoutSloVisibility =
  typeof PylonAcceptedWorkPayoutSloVisibility.Type

export const PylonAcceptedWorkPayoutSloAuthorityBoundary = S.Literals([
  'read_only_projection',
])
export type PylonAcceptedWorkPayoutSloAuthorityBoundary =
  typeof PylonAcceptedWorkPayoutSloAuthorityBoundary.Type

export class PylonAcceptedWorkPayoutSloAuthority extends S.Class<PylonAcceptedWorkPayoutSloAuthority>(
  'PylonAcceptedWorkPayoutSloAuthority',
)({
  authorityBoundary: PylonAcceptedWorkPayoutSloAuthorityBoundary,
  noBuyerChargeMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noProviderEligibilityMutation: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class PylonAcceptedWorkPayoutSloRecord extends S.Class<PylonAcceptedWorkPayoutSloRecord>(
  'PylonAcceptedWorkPayoutSloRecord',
)({
  acceptedWorkRefs: S.Array(S.String),
  authority: PylonAcceptedWorkPayoutSloAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  confirmationObservedAtIso: S.NullOr(S.String),
  confirmationRefs: S.Array(S.String),
  createdAtIso: S.String,
  dispatchRecordedAtIso: S.NullOr(S.String),
  dispatchRecordRefs: S.Array(S.String),
  dispatchRequestedAtIso: S.NullOr(S.String),
  dispatchRequestRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  failedAttemptCount: S.Number,
  failureRefs: S.Array(S.String),
  freshness: PylonAcceptedWorkPayoutSloFreshness,
  freshnessRefs: S.Array(S.String),
  id: S.String,
  jobRef: S.String,
  providerRef: S.String,
  providerVisibility: PylonAcceptedWorkPayoutSloVisibility,
  settlementRefs: S.Array(S.String),
  settledAtIso: S.NullOr(S.String),
  skippedAttemptCount: S.Number,
  skippedRefs: S.Array(S.String),
  sloBreachRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonAcceptedWorkPayoutSloState,
  updatedAtIso: S.String,
  verificationCompletedAtIso: S.NullOr(S.String),
  verificationRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export class PylonAcceptedWorkPayoutSloProjection extends S.Class<PylonAcceptedWorkPayoutSloProjection>(
  'PylonAcceptedWorkPayoutSloProjection',
)({
  acceptedWorkClaimAllowed: S.Boolean,
  acceptedWorkRefs: S.Array(S.String),
  attentionRequired: S.Boolean,
  audience: OmniProjectionAudience,
  authority: PylonAcceptedWorkPayoutSloAuthority,
  blockerRefs: S.Array(S.String),
  buyerChargeMutationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  confirmationLatencyDisplay: S.NullOr(S.String),
  confirmationLatencyMs: S.NullOr(S.Number),
  confirmationObservedClaimAllowed: S.Boolean,
  confirmationRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  dispatchLatencyDisplay: S.NullOr(S.String),
  dispatchLatencyMs: S.NullOr(S.Number),
  dispatchRecordRefs: S.Array(S.String),
  dispatchRecordedClaimAllowed: S.Boolean,
  dispatchRequestRefs: S.Array(S.String),
  dispatchRequestedClaimAllowed: S.Boolean,
  evidenceRefs: S.Array(S.String),
  failedAttemptCount: S.Number,
  failureRefs: S.Array(S.String),
  freshness: PylonAcceptedWorkPayoutSloFreshness,
  freshnessLabel: S.String,
  freshnessRefs: S.Array(S.String),
  id: S.String,
  jobRef: S.String,
  liveWalletSpendAllowed: S.Boolean,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutTargetMutationAllowed: S.Boolean,
  providerEligibilityMutationAllowed: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonAcceptedWorkPayoutSloVisibility,
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  skippedAttemptCount: S.Number,
  skippedRefs: S.Array(S.String),
  sloBreachRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonAcceptedWorkPayoutSloState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  verificationCompleteClaimAllowed: S.Boolean,
  verificationRefs: S.Array(S.String),
  workroomRefs: S.Array(S.String),
}) {}

export class PylonAcceptedWorkPayoutSloUnsafe extends S.TaggedErrorClass<PylonAcceptedWorkPayoutSloUnsafe>()(
  'PylonAcceptedWorkPayoutSloUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_ACCEPTED_WORK_PAYOUT_SLO_READ_ONLY_AUTHORITY:
  PylonAcceptedWorkPayoutSloAuthority = {
    authorityBoundary: 'read_only_projection',
    noBuyerChargeMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetMutation: true,
    noProviderEligibilityMutation: true,
    noSettlementMutation: true,
  }

const positiveStateRank:
  Partial<Record<PylonAcceptedWorkPayoutSloState, number>> = {
    confirmation_observed: 3,
    dispatch_recorded: 2,
    dispatch_requested: 1,
    settled: 5,
    verification_complete: 4,
  }

const stateLabelByState:
  Readonly<Record<PylonAcceptedWorkPayoutSloState, string>> = {
    attention_required: 'Needs attention',
    blocked: 'Blocked',
    confirmation_observed: 'Confirmation observed',
    dispatch_recorded: 'Dispatch recorded',
    dispatch_requested: 'Dispatch requested',
    failed: 'Failed',
    settled: 'Settled',
    skipped: 'Skipped',
    stale: 'Stale',
    verification_complete: 'Verification complete',
  }

const freshnessLabelByFreshness:
  Readonly<Record<PylonAcceptedWorkPayoutSloFreshness, string>> = {
    expired: 'Expired',
    fresh: 'Fresh',
    stale: 'Stale',
    unknown: 'Unknown',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeAcceptedWorkPayoutSloRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|channel[_-]?monitor|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|private|raw|target)|payout[_-]?target[.:_-](address|bc1|destination|ln|private|raw|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|channel|invoice|payment|payload|payout|prompt|provider|runner|run[_-]?log|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(blocker\.private|caveat\.private|confirmation\.private|dispatch\.private|evidence\.private|failure\.private|freshness\.private|provider\.private|settlement\.private|skip\.private|slo\.private|source\.private|verification\.private|workroom\.)/i
const customerUnsafeRefPattern =
  /(blocker\.private|caveat\.private|confirmation\.private|dispatch\.private|evidence\.private|failure\.private|freshness\.private|provider\.private|settlement\.private|skip\.private|slo\.private|source\.private|verification\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(confirmation\.private|dispatch\.private|provider\.private|settlement\.private|verification\.private|workroom\.private)/i

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
    unsafeAcceptedWorkPayoutSloRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: `${label} contains private customer data, wallet material, raw bitcoin payment material, invoices, preimages, raw payout targets, private channel state, provider secrets, raw logs, private repo refs, or raw timestamps.`,
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

const visibleProviderRef = (
  record: PylonAcceptedWorkPayoutSloRecord,
  audience: typeof OmniProjectionAudience.Type,
): string => {
  if (
    record.providerVisibility === 'public' ||
    audience === 'operator' ||
    audience === 'private'
  ) {
    return safeRefsForAudience(
      'accepted-work payout SLO provider ref',
      [record.providerRef],
      audience,
    )[0] ?? 'provider.redacted'
  }

  return 'provider.redacted'
}

const stateAtLeast = (
  state: PylonAcceptedWorkPayoutSloState,
  threshold: PylonAcceptedWorkPayoutSloState,
): boolean =>
  (positiveStateRank[state] ?? -1) >= (positiveStateRank[threshold] ?? 999)

const isAttentionRequired = (
  record: PylonAcceptedWorkPayoutSloRecord,
): boolean =>
  record.state === 'attention_required' ||
  record.state === 'blocked' ||
  record.state === 'failed' ||
  record.state === 'skipped' ||
  record.state === 'stale' ||
  record.freshness === 'expired' ||
  record.freshness === 'stale'

const epochMillis = (label: string, iso: string): number => {
  const millis = Date.parse(iso)

  if (!Number.isFinite(millis)) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }

  return millis
}

const positiveLatencyMs = (
  label: string,
  startIso: string | null,
  endIso: string | null,
): number | null => {
  if (startIso === null || endIso === null) {
    return null
  }

  const latency = epochMillis(`${label} end`, endIso) -
    epochMillis(`${label} start`, startIso)

  if (latency < 0) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: `${label} latency cannot be negative.`,
    })
  }

  return latency
}

const durationDisplay = (millis: number | null): string | null => {
  if (millis === null) {
    return null
  }

  if (millis < 1_000) {
    return `${millis} ms`
  }

  if (millis < 60_000) {
    return `${Math.round(millis / 1_000)} sec`
  }

  if (millis < 3_600_000) {
    return `${Math.round(millis / 60_000)} min`
  }

  return `${Math.round(millis / 3_600_000)} hr`
}

export const pylonAcceptedWorkPayoutSloHasNoMutationAuthority = (
  authority: PylonAcceptedWorkPayoutSloAuthority,
): boolean =>
  authority.authorityBoundary === 'read_only_projection' &&
  authority.noBuyerChargeMutation &&
  authority.noLiveWalletSpend &&
  authority.noPayoutDispatch &&
  authority.noPayoutTargetMutation &&
  authority.noProviderEligibilityMutation &&
  authority.noSettlementMutation

export const pylonAcceptedWorkPayoutSloCanDispatchPayout = (
  record: PylonAcceptedWorkPayoutSloRecord,
): boolean => !pylonAcceptedWorkPayoutSloHasNoMutationAuthority(record.authority)

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertRecordSafe = (
  record: PylonAcceptedWorkPayoutSloRecord,
): void => {
  assertSafeRefs('accepted-work payout SLO identity refs', [
    record.id,
    record.jobRef,
    record.providerRef,
  ])
  assertSafeRefs(
    'accepted-work payout SLO accepted-work refs',
    record.acceptedWorkRefs,
  )
  assertSafeRefs(
    'accepted-work payout SLO dispatch request refs',
    record.dispatchRequestRefs,
  )
  assertSafeRefs(
    'accepted-work payout SLO dispatch record refs',
    record.dispatchRecordRefs,
  )
  assertSafeRefs(
    'accepted-work payout SLO confirmation refs',
    record.confirmationRefs,
  )
  assertSafeRefs(
    'accepted-work payout SLO verification refs',
    record.verificationRefs,
  )
  assertSafeRefs(
    'accepted-work payout SLO settlement refs',
    record.settlementRefs,
  )
  assertSafeRefs('accepted-work payout SLO failure refs', record.failureRefs)
  assertSafeRefs('accepted-work payout SLO skipped refs', record.skippedRefs)
  assertSafeRefs('accepted-work payout SLO blocker refs', record.blockerRefs)
  assertSafeRefs('accepted-work payout SLO caveat refs', record.caveatRefs)
  assertSafeRefs(
    'accepted-work payout SLO freshness refs',
    record.freshnessRefs,
  )
  assertSafeRefs('accepted-work payout SLO breach refs', record.sloBreachRefs)
  assertSafeRefs('accepted-work payout SLO evidence refs', record.evidenceRefs)
  assertSafeRefs('accepted-work payout SLO source refs', record.sourceRefs)
  assertSafeRefs('accepted-work payout SLO workroom refs', record.workroomRefs)

  if (!pylonAcceptedWorkPayoutSloHasNoMutationAuthority(record.authority)) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: 'Accepted-work payout SLO projections are read-only and cannot carry buyer charge, wallet spend, payout dispatch, payout target, provider eligibility, or settlement mutation authority.',
    })
  }

  assertNonNegativeInteger('Failed attempt count', record.failedAttemptCount)
  assertNonNegativeInteger('Skipped attempt count', record.skippedAttemptCount)

  epochMillis('accepted-work payout SLO createdAtIso', record.createdAtIso)
  epochMillis('accepted-work payout SLO updatedAtIso', record.updatedAtIso)

  const requiredStateRefs:
    Readonly<Record<PylonAcceptedWorkPayoutSloState, ReadonlyArray<string>>> = {
      attention_required: [
        ...record.blockerRefs,
        ...record.caveatRefs,
        ...record.sloBreachRefs,
      ],
      blocked: record.blockerRefs,
      confirmation_observed: record.confirmationRefs,
      dispatch_recorded: record.dispatchRecordRefs,
      dispatch_requested: record.dispatchRequestRefs,
      failed: record.failureRefs,
      settled: record.settlementRefs,
      skipped: record.skippedRefs,
      stale: [...record.freshnessRefs, ...record.caveatRefs],
      verification_complete: record.verificationRefs,
    }

  if (requiredStateRefs[record.state].length === 0) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: `${stateLabelByState[record.state]} accepted-work payout SLO state requires matching evidence refs.`,
    })
  }

  if (
    (record.freshness === 'expired' || record.freshness === 'stale') &&
    record.freshnessRefs.length === 0 &&
    record.caveatRefs.length === 0
  ) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: 'Stale or expired payout SLO freshness requires freshness or caveat refs.',
    })
  }
}

const projectionText = (
  projection: PylonAcceptedWorkPayoutSloProjection,
): string =>
  [
    projection.id,
    projection.jobRef,
    projection.providerRef,
    ...projection.acceptedWorkRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.confirmationRefs,
    ...projection.dispatchRecordRefs,
    ...projection.dispatchRequestRefs,
    ...projection.evidenceRefs,
    ...projection.failureRefs,
    ...projection.freshnessRefs,
    ...projection.settlementRefs,
    ...projection.skippedRefs,
    ...projection.sloBreachRefs,
    ...projection.sourceRefs,
    ...projection.verificationRefs,
    ...projection.workroomRefs,
  ].join(' ')

export const pylonAcceptedWorkPayoutSloProjectionHasPrivateMaterial = (
  projection: PylonAcceptedWorkPayoutSloProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeAcceptedWorkPayoutSloRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonAcceptedWorkPayoutSlo = (
  record: PylonAcceptedWorkPayoutSloRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonAcceptedWorkPayoutSloProjection => {
  assertRecordSafe(record)

  const dispatchLatencyMs = positiveLatencyMs(
    'dispatch',
    record.dispatchRequestedAtIso,
    record.dispatchRecordedAtIso,
  )
  const confirmationLatencyMs = positiveLatencyMs(
    'confirmation',
    record.dispatchRecordedAtIso,
    record.confirmationObservedAtIso,
  )
  const projection: PylonAcceptedWorkPayoutSloProjection = {
    acceptedWorkClaimAllowed: record.acceptedWorkRefs.length > 0,
    acceptedWorkRefs: safeRefsForAudience(
      'accepted-work payout SLO accepted-work refs',
      record.acceptedWorkRefs,
      audience,
    ),
    attentionRequired: isAttentionRequired(record),
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'accepted-work payout SLO blocker refs',
      record.blockerRefs,
      audience,
    ),
    buyerChargeMutationAllowed: false,
    caveatRefs: safeRefsForAudience(
      'accepted-work payout SLO caveat refs',
      record.caveatRefs,
      audience,
    ),
    confirmationLatencyDisplay: durationDisplay(confirmationLatencyMs),
    confirmationLatencyMs,
    confirmationObservedClaimAllowed:
      stateAtLeast(record.state, 'confirmation_observed') &&
      record.confirmationRefs.length > 0,
    confirmationRefs: safeRefsForAudience(
      'accepted-work payout SLO confirmation refs',
      record.confirmationRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    dispatchLatencyDisplay: durationDisplay(dispatchLatencyMs),
    dispatchLatencyMs,
    dispatchRecordRefs: safeRefsForAudience(
      'accepted-work payout SLO dispatch record refs',
      record.dispatchRecordRefs,
      audience,
    ),
    dispatchRecordedClaimAllowed:
      stateAtLeast(record.state, 'dispatch_recorded') &&
      record.dispatchRecordRefs.length > 0,
    dispatchRequestRefs: safeRefsForAudience(
      'accepted-work payout SLO dispatch request refs',
      record.dispatchRequestRefs,
      audience,
    ),
    dispatchRequestedClaimAllowed:
      stateAtLeast(record.state, 'dispatch_requested') &&
      record.dispatchRequestRefs.length > 0,
    evidenceRefs: safeRefsForAudience(
      'accepted-work payout SLO evidence refs',
      record.evidenceRefs,
      audience,
    ),
    failedAttemptCount: record.failedAttemptCount,
    failureRefs: safeRefsForAudience(
      'accepted-work payout SLO failure refs',
      record.failureRefs,
      audience,
    ),
    freshness: record.freshness,
    freshnessLabel: freshnessLabelByFreshness[record.freshness],
    freshnessRefs: safeRefsForAudience(
      'accepted-work payout SLO freshness refs',
      record.freshnessRefs,
      audience,
    ),
    id: safeRefsForAudience('accepted-work payout SLO id', [record.id], audience)[0] ??
      'payout_slo.redacted',
    jobRef: record.jobRef,
    liveWalletSpendAllowed: false,
    payoutDispatchMutationAllowed: false,
    payoutTargetMutationAllowed: false,
    providerEligibilityMutationAllowed: false,
    providerRef: visibleProviderRef(record, audience),
    providerVisibility: record.providerVisibility,
    settlementClaimAllowed: record.state === 'settled' &&
      record.settlementRefs.length > 0 &&
      record.verificationRefs.length > 0,
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'accepted-work payout SLO settlement refs',
      record.settlementRefs,
      audience,
    ),
    skippedAttemptCount: record.skippedAttemptCount,
    skippedRefs: safeRefsForAudience(
      'accepted-work payout SLO skipped refs',
      record.skippedRefs,
      audience,
    ),
    sloBreachRefs: safeRefsForAudience(
      'accepted-work payout SLO breach refs',
      record.sloBreachRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience(
      'accepted-work payout SLO source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verificationCompleteClaimAllowed:
      stateAtLeast(record.state, 'verification_complete') &&
      record.verificationRefs.length > 0,
    verificationRefs: safeRefsForAudience(
      'accepted-work payout SLO verification refs',
      record.verificationRefs,
      audience,
    ),
    workroomRefs: safeRefsForAudience(
      'accepted-work payout SLO workroom refs',
      record.workroomRefs,
      audience,
    ),
  }

  if (pylonAcceptedWorkPayoutSloProjectionHasPrivateMaterial(projection)) {
    throw new PylonAcceptedWorkPayoutSloUnsafe({
      reason: 'Accepted-work payout SLO projection still contains private or unsafe material after redaction.',
    })
  }

  return projection
}

export const PYLON_ACCEPTED_WORK_PAYOUT_SLO_CONFORMANCE_FIXTURES:
  ReadonlyArray<PylonAcceptedWorkPayoutSloRecord> = [
    {
      acceptedWorkRefs: ['accepted_work.public.trace_summary'],
      authority: PYLON_ACCEPTED_WORK_PAYOUT_SLO_READ_ONLY_AUTHORITY,
      blockerRefs: [],
      caveatRefs: ['caveat.public.slo_observed_from_nexus'],
      confirmationObservedAtIso: '2026-06-07T08:04:30.000Z',
      confirmationRefs: [
        'confirmation.public.trace_summary',
        'confirmation.private.operator_trace',
      ],
      createdAtIso: '2026-06-07T08:00:00.000Z',
      dispatchRecordedAtIso: '2026-06-07T08:02:00.000Z',
      dispatchRecordRefs: [
        'dispatch.public.trace_summary',
        'dispatch.private.operator_trace',
      ],
      dispatchRequestedAtIso: '2026-06-07T08:00:00.000Z',
      dispatchRequestRefs: ['dispatch_request.public.trace_summary'],
      evidenceRefs: ['evidence.public.accepted_work_payout_slo'],
      failedAttemptCount: 0,
      failureRefs: [],
      freshness: 'fresh',
      freshnessRefs: ['freshness.public.recent_projection'],
      id: 'payout_slo.trace_summary_1',
      jobRef: 'pylon_job.trace_summary_1',
      providerRef: 'provider.private.pylon_1',
      providerVisibility: 'private',
      settlementRefs: ['settlement.public.trace_summary'],
      settledAtIso: '2026-06-07T08:06:00.000Z',
      skippedAttemptCount: 0,
      skippedRefs: [],
      sloBreachRefs: [],
      sourceRefs: ['source.public.nexus_treasury_projection'],
      state: 'settled',
      updatedAtIso: '2026-06-07T08:05:00.000Z',
      verificationCompletedAtIso: '2026-06-07T08:05:30.000Z',
      verificationRefs: [
        'verification.public.trace_summary',
        'verification.private.operator_trace',
      ],
      workroomRefs: ['workroom.private.trace_summary_operator'],
    },
  ]
