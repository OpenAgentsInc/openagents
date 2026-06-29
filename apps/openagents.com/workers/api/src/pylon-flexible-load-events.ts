import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const PylonFlexibleLoadEventState = S.Literals([
  'acknowledged',
  'blocked',
  'compensated',
  'executed',
  'failed',
  'measured',
  'requested',
  'settled',
  'verified',
])
export type PylonFlexibleLoadEventState =
  typeof PylonFlexibleLoadEventState.Type

export const PylonFlexibleLoadEventAuthorityBoundary = S.Literals([
  'read_only_event_telemetry',
])
export type PylonFlexibleLoadEventAuthorityBoundary =
  typeof PylonFlexibleLoadEventAuthorityBoundary.Type

export class PylonFlexibleLoadEventAuthority extends S.Class<PylonFlexibleLoadEventAuthority>(
  'PylonFlexibleLoadEventAuthority',
)({
  authorityBoundary: PylonFlexibleLoadEventAuthorityBoundary,
  noAcceptedWorkMutation: S.Boolean,
  noCapacityDispatch: S.Boolean,
  noGridServiceClaimUpgrade: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class PylonFlexibleLoadEventRecord extends S.Class<PylonFlexibleLoadEventRecord>(
  'PylonFlexibleLoadEventRecord',
)({
  acceptedWorkImpactRefs: S.Array(S.String),
  acknowledgementRefs: S.Array(S.String),
  actualResponseWatts: S.NullOr(S.Number),
  authority: PylonFlexibleLoadEventAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  checkpointRefs: S.Array(S.String),
  compensationRefs: S.Array(S.String),
  createdAtIso: S.String,
  eventRef: S.String,
  evidenceRefs: S.Array(S.String),
  executionRefs: S.Array(S.String),
  id: S.String,
  interruptedWorkRefs: S.Array(S.String),
  lostWorkCostCents: S.Number,
  measurementRefs: S.Array(S.String),
  profileRefs: S.Array(S.String),
  providerRef: S.String,
  requestedResponseWatts: S.Number,
  requestRefs: S.Array(S.String),
  resumeRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonFlexibleLoadEventState,
  updatedAtIso: S.String,
  workClassRefs: S.Array(S.String),
}) {}

export class PylonFlexibleLoadEventProjection extends S.Class<PylonFlexibleLoadEventProjection>(
  'PylonFlexibleLoadEventProjection',
)({
  acceptedWorkImpactClaimAllowed: S.Boolean,
  acceptedWorkImpactRefs: S.Array(S.String),
  acknowledgementClaimAllowed: S.Boolean,
  acknowledgementRefs: S.Array(S.String),
  actualResponseWatts: S.NullOr(S.Number),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  authority: PylonFlexibleLoadEventAuthority,
  blockerRefs: S.Array(S.String),
  capacityDispatchAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  checkpointRefs: S.Array(S.String),
  compensationClaimAllowed: S.Boolean,
  compensationRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  eventRef: S.String,
  evidenceRefs: S.Array(S.String),
  executedClaimAllowed: S.Boolean,
  executionRefs: S.Array(S.String),
  gridServiceClaimUpgradeAllowed: S.Boolean,
  id: S.String,
  interruptedWorkRefs: S.Array(S.String),
  liveWalletSpendAllowed: S.Boolean,
  lostWorkCostCents: S.Number,
  measuredClaimAllowed: S.Boolean,
  measurementRefs: S.Array(S.String),
  payoutDispatchAllowed: S.Boolean,
  profileRefs: S.Array(S.String),
  providerRef: S.String,
  requestedClaimAllowed: S.Boolean,
  requestedResponseWatts: S.Number,
  requestRefs: S.Array(S.String),
  responseRatioBps: S.NullOr(S.Number),
  resumeRefs: S.Array(S.String),
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonFlexibleLoadEventState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  verifiedClaimAllowed: S.Boolean,
  workClassRefs: S.Array(S.String),
}) {}

export class PylonFlexibleLoadEventUnsafe extends S.TaggedErrorClass<PylonFlexibleLoadEventUnsafe>()(
  'PylonFlexibleLoadEventUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY:
  PylonFlexibleLoadEventAuthority = {
    authorityBoundary: 'read_only_event_telemetry',
    noAcceptedWorkMutation: true,
    noCapacityDispatch: true,
    noGridServiceClaimUpgrade: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noSettlementMutation: true,
  }

const stateLabelByState: Record<PylonFlexibleLoadEventState, string> = {
  acknowledged: 'Acknowledged',
  blocked: 'Blocked',
  compensated: 'Compensated',
  executed: 'Executed',
  failed: 'Failed',
  measured: 'Measured',
  requested: 'Requested',
  settled: 'Settled',
  verified: 'Verified',
}

const stateRank: Record<PylonFlexibleLoadEventState, number> = {
  acknowledged: 1,
  blocked: -1,
  compensated: 5,
  executed: 2,
  failed: -1,
  measured: 3,
  requested: 0,
  settled: 6,
  verified: 4,
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?(hardware|key)|provider[_-]?(account|grant|payload|telemetry|token)|raw[_-]?(host|invoice|measurement|meter|payment|payload|power|prompt|payout[_-]?target|runner|run[_-]?log|source[_-]?archive|telemetry|webhook)|runner[_-]?log|secret|serial[_-]?number|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_work\.private|ack\.private|blocker\.private|checkpoint\.private|compensation\.private|event\.private|evidence\.private|execution\.private|interrupt\.private|measurement\.private|profile\.private|provider\.private|request\.private|resume\.private|settlement\.private|source\.private|work_class\.private)/i
const teamUnsafeRefPattern =
  /(compensation\.private|measurement\.private|provider\.private|settlement\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stageAtLeast = (
  state: PylonFlexibleLoadEventState,
  threshold: PylonFlexibleLoadEventState,
): boolean => stateRank[state] >= stateRank[threshold]

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: `${label} contains private provider, runner, wallet, payment, payout target, customer, private hardware, raw telemetry, raw measurement, private repo, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public' || audience === 'customer') {
    return publicUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertRecordSafe = (record: PylonFlexibleLoadEventRecord): void => {
  assertSafeRefs('flexible-load event identity refs', [
    record.id,
    record.eventRef,
    record.providerRef,
  ])
  assertSafeRefs('flexible-load profile refs', record.profileRefs)
  assertSafeRefs('flexible-load request refs', record.requestRefs)
  assertSafeRefs('flexible-load acknowledgement refs', record.acknowledgementRefs)
  assertSafeRefs('flexible-load execution refs', record.executionRefs)
  assertSafeRefs('flexible-load measurement refs', record.measurementRefs)
  assertSafeRefs('flexible-load verification/evidence refs', record.evidenceRefs)
  assertSafeRefs('flexible-load compensation refs', record.compensationRefs)
  assertSafeRefs('flexible-load settlement refs', record.settlementRefs)
  assertSafeRefs('flexible-load interrupted work refs', record.interruptedWorkRefs)
  assertSafeRefs('flexible-load checkpoint refs', record.checkpointRefs)
  assertSafeRefs('flexible-load resume refs', record.resumeRefs)
  assertSafeRefs('flexible-load accepted work impact refs', record.acceptedWorkImpactRefs)
  assertSafeRefs('flexible-load blocker refs', record.blockerRefs)
  assertSafeRefs('flexible-load caveat refs', record.caveatRefs)
  assertSafeRefs('flexible-load source refs', record.sourceRefs)
  assertSafeRefs('flexible-load work class refs', record.workClassRefs)

  assertNonNegativeInteger('requestedResponseWatts', record.requestedResponseWatts)
  assertNonNegativeInteger('lostWorkCostCents', record.lostWorkCostCents)

  if (record.actualResponseWatts !== null) {
    assertNonNegativeInteger('actualResponseWatts', record.actualResponseWatts)
  }

  if (
    record.authority.noAcceptedWorkMutation !== true ||
    record.authority.noCapacityDispatch !== true ||
    record.authority.noGridServiceClaimUpgrade !== true ||
    record.authority.noLiveWalletSpend !== true ||
    record.authority.noPayoutDispatch !== true ||
    record.authority.noSettlementMutation !== true
  ) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason:
        'Flexible-load event telemetry must remain read-only and cannot mutate accepted work, dispatch capacity, upgrade grid-service claims, spend wallets, dispatch payouts, or mutate settlement.',
    })
  }

  if (!hasRefs(record.requestRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Flexible-load events require request refs.',
    })
  }

  if (stageAtLeast(record.state, 'acknowledged') && !hasRefs(record.acknowledgementRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Acknowledged flexible-load events require acknowledgement refs.',
    })
  }

  if (stageAtLeast(record.state, 'executed') && !hasRefs(record.executionRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Executed flexible-load events require execution refs.',
    })
  }

  if (
    stageAtLeast(record.state, 'measured') &&
    (record.actualResponseWatts === null || !hasRefs(record.measurementRefs))
  ) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason:
        'Measured flexible-load events require actual response watts and measurement refs.',
    })
  }

  if (stageAtLeast(record.state, 'verified') && !hasRefs(record.evidenceRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Verified flexible-load events require evidence refs.',
    })
  }

  if (
    stageAtLeast(record.state, 'compensated') &&
    !hasRefs(record.compensationRefs)
  ) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Compensated flexible-load events require compensation refs.',
    })
  }

  if (record.state === 'settled' && !hasRefs(record.settlementRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Settled flexible-load events require settlement refs.',
    })
  }

  if (record.lostWorkCostCents > 0 && !hasRefs(record.interruptedWorkRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Lost-work cost requires interrupted work refs.',
    })
  }

  if (hasRefs(record.resumeRefs) && !hasRefs(record.checkpointRefs)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason: 'Resume refs require checkpoint refs.',
    })
  }
}

const responseRatioBps = (
  requestedResponseWatts: number,
  actualResponseWatts: number | null,
): number | null =>
  requestedResponseWatts <= 0 || actualResponseWatts === null
    ? null
    : Math.round((actualResponseWatts * 10000) / requestedResponseWatts)

const projectionText = (
  projection: PylonFlexibleLoadEventProjection,
): string =>
  [
    projection.id,
    projection.eventRef,
    projection.providerRef,
    ...projection.profileRefs,
    ...projection.requestRefs,
    ...projection.acknowledgementRefs,
    ...projection.executionRefs,
    ...projection.measurementRefs,
    ...projection.evidenceRefs,
    ...projection.compensationRefs,
    ...projection.settlementRefs,
    ...projection.interruptedWorkRefs,
    ...projection.checkpointRefs,
    ...projection.resumeRefs,
    ...projection.acceptedWorkImpactRefs,
    ...projection.blockerRefs,
    ...projection.caveatRefs,
    ...projection.sourceRefs,
    ...projection.workClassRefs,
  ].join(' ')

export const pylonFlexibleLoadEventProjectionHasPrivateMaterial = (
  projection: PylonFlexibleLoadEventProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonFlexibleLoadEvent = (
  record: PylonFlexibleLoadEventRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonFlexibleLoadEventProjection => {
  assertRecordSafe(record)

  const projection: PylonFlexibleLoadEventProjection = {
    acceptedWorkImpactClaimAllowed: hasRefs(record.acceptedWorkImpactRefs),
    acceptedWorkImpactRefs: safeRefsForAudience(
      'flexible-load accepted work impact refs',
      record.acceptedWorkImpactRefs,
      audience,
    ),
    acknowledgementClaimAllowed:
      stageAtLeast(record.state, 'acknowledged') &&
      hasRefs(record.acknowledgementRefs),
    acknowledgementRefs: safeRefsForAudience(
      'flexible-load acknowledgement refs',
      record.acknowledgementRefs,
      audience,
    ),
    actualResponseWatts: record.actualResponseWatts,
    audience,
    authority: record.authority,
    blockerRefs: safeRefsForAudience(
      'flexible-load blocker refs',
      record.blockerRefs,
      audience,
    ),
    capacityDispatchAllowed: false,
    caveatRefs: safeRefsForAudience(
      'flexible-load caveat refs',
      record.caveatRefs,
      audience,
    ),
    checkpointRefs: safeRefsForAudience(
      'flexible-load checkpoint refs',
      record.checkpointRefs,
      audience,
    ),
    compensationClaimAllowed:
      stageAtLeast(record.state, 'compensated') &&
      hasRefs(record.compensationRefs),
    compensationRefs: safeRefsForAudience(
      'flexible-load compensation refs',
      record.compensationRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    eventRef: record.eventRef,
    evidenceRefs: safeRefsForAudience(
      'flexible-load evidence refs',
      record.evidenceRefs,
      audience,
    ),
    executedClaimAllowed:
      stageAtLeast(record.state, 'executed') && hasRefs(record.executionRefs),
    executionRefs: safeRefsForAudience(
      'flexible-load execution refs',
      record.executionRefs,
      audience,
    ),
    gridServiceClaimUpgradeAllowed: false,
    id: record.id,
    interruptedWorkRefs: safeRefsForAudience(
      'flexible-load interrupted work refs',
      record.interruptedWorkRefs,
      audience,
    ),
    liveWalletSpendAllowed: false,
    lostWorkCostCents: record.lostWorkCostCents,
    measuredClaimAllowed:
      stageAtLeast(record.state, 'measured') &&
      record.actualResponseWatts !== null &&
      hasRefs(record.measurementRefs),
    measurementRefs: safeRefsForAudience(
      'flexible-load measurement refs',
      record.measurementRefs,
      audience,
    ),
    payoutDispatchAllowed: false,
    profileRefs: safeRefsForAudience(
      'flexible-load profile refs',
      record.profileRefs,
      audience,
    ),
    providerRef: safeRefsForAudience(
      'flexible-load provider ref',
      [record.providerRef],
      audience,
    )[0] ?? 'provider.redacted',
    requestedClaimAllowed: hasRefs(record.requestRefs),
    requestedResponseWatts: record.requestedResponseWatts,
    requestRefs: safeRefsForAudience(
      'flexible-load request refs',
      record.requestRefs,
      audience,
    ),
    responseRatioBps: responseRatioBps(
      record.requestedResponseWatts,
      record.actualResponseWatts,
    ),
    resumeRefs: safeRefsForAudience(
      'flexible-load resume refs',
      record.resumeRefs,
      audience,
    ),
    settlementClaimAllowed:
      record.state === 'settled' && hasRefs(record.settlementRefs),
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'flexible-load settlement refs',
      record.settlementRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience(
      'flexible-load source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    verifiedClaimAllowed:
      stageAtLeast(record.state, 'verified') && hasRefs(record.evidenceRefs),
    workClassRefs: safeRefsForAudience(
      'flexible-load work class refs',
      record.workClassRefs,
      audience,
    ),
  }

  if (pylonFlexibleLoadEventProjectionHasPrivateMaterial(projection)) {
    throw new PylonFlexibleLoadEventUnsafe({
      reason:
        'Flexible-load event projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const examplePylonFlexibleLoadEvent =
  (): PylonFlexibleLoadEventRecord => ({
    acceptedWorkImpactRefs: ['accepted_work.public.site_revision_5'],
    acknowledgementRefs: ['ack.public.flex_event_1'],
    actualResponseWatts: 180000,
    authority: PYLON_FLEXIBLE_LOAD_EVENT_READ_ONLY_AUTHORITY,
    blockerRefs: [],
    caveatRefs: ['caveat.public.flex_event_not_grid_settlement'],
    checkpointRefs: ['checkpoint.public.flex_event_1'],
    compensationRefs: ['compensation.public.flex_event_1'],
    createdAtIso: '2026-06-06T23:00:00.000Z',
    eventRef: 'event.flex.public_1',
    evidenceRefs: ['evidence.public.flex_event_1'],
    executionRefs: ['execution.public.flex_event_1'],
    id: 'flex_event.public_1',
    interruptedWorkRefs: ['interrupted_work.public.site_revision_5'],
    lostWorkCostCents: 250,
    measurementRefs: ['measurement.public.flex_event_1'],
    profileRefs: ['profile.flex.autopilot_sites_review_build'],
    providerRef: 'provider.public_demo_1',
    requestedResponseWatts: 200000,
    requestRefs: ['request.public.flex_event_1'],
    resumeRefs: ['resume.public.flex_event_1'],
    settlementRefs: ['settlement.public.flex_event_1'],
    sourceRefs: ['source.public.flex_event_1'],
    state: 'settled',
    updatedAtIso: '2026-06-06T23:05:00.000Z',
    workClassRefs: ['work_class.autopilot_sites_review_build'],
  })
