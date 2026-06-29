import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const PylonCapacityFunnelStage = S.Literals([
  'accepted',
  'artifact_producing',
  'assigned',
  'benchmarked',
  'dark',
  'eligible',
  'paid',
  'registered',
  'running',
  'settled',
])
export type PylonCapacityFunnelStage = typeof PylonCapacityFunnelStage.Type

export const PylonCapacityVisibility = S.Literals(['private', 'public'])
export type PylonCapacityVisibility = typeof PylonCapacityVisibility.Type

export class PylonCapacityFunnelRecord extends S.Class<PylonCapacityFunnelRecord>(
  'PylonCapacityFunnelRecord',
)({
  acceptanceRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  benchmarkRefs: S.Array(S.String),
  capacityRef: S.String,
  caveatRefs: S.Array(S.String),
  darkCapacityReasonRefs: S.Array(S.String),
  eligibilityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  nodeRef: S.String,
  nodeVisibility: PylonCapacityVisibility,
  providerRef: S.String,
  providerVisibility: PylonCapacityVisibility,
  rewardRefs: S.Array(S.String),
  runRefs: S.Array(S.String),
  settlementRefs: S.Array(S.String),
  stage: PylonCapacityFunnelStage,
  updatedAtIso: S.String,
  workClassRefs: S.Array(S.String),
}) {}

export class PylonCapacityFunnelProjection extends S.Class<PylonCapacityFunnelProjection>(
  'PylonCapacityFunnelProjection',
)({
  acceptanceRefs: S.Array(S.String),
  acceptedCapacityClaimAllowed: S.Boolean,
  artifactRefs: S.Array(S.String),
  assignmentRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  benchmarkRefs: S.Array(S.String),
  capacityRef: S.String,
  caveatRefs: S.Array(S.String),
  darkCapacityReasonRefs: S.Array(S.String),
  eligibilityRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  nodeRef: S.String,
  nodeVisibility: PylonCapacityVisibility,
  paidCapacityClaimAllowed: S.Boolean,
  providerRef: S.String,
  providerVisibility: PylonCapacityVisibility,
  rewardRefs: S.Array(S.String),
  runRefs: S.Array(S.String),
  settledCapacityClaimAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  stage: PylonCapacityFunnelStage,
  stageLabel: S.String,
  updatedAtDisplay: S.String,
  workClassRefs: S.Array(S.String),
}) {}

export class PylonCapacityFunnelCount extends S.Class<PylonCapacityFunnelCount>(
  'PylonCapacityFunnelCount',
)({
  count: S.Number,
  key: S.String,
}) {}

export const PylonCapacityFunnelAccountingAuthorityBoundary = S.Literals([
  'read_only_capacity_accounting',
])
export type PylonCapacityFunnelAccountingAuthorityBoundary =
  typeof PylonCapacityFunnelAccountingAuthorityBoundary.Type

export class PylonCapacityFunnelAccountingAuthority extends S.Class<PylonCapacityFunnelAccountingAuthority>(
  'PylonCapacityFunnelAccountingAuthority',
)({
  authorityBoundary: PylonCapacityFunnelAccountingAuthorityBoundary,
  noCapacityAssignmentMutation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noPayoutDispatch: S.Boolean,
  noPayoutTargetMutation: S.Boolean,
  noProviderEligibilityMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class PylonCapacityFunnelAggregate extends S.Class<PylonCapacityFunnelAggregate>(
  'PylonCapacityFunnelAggregate',
)({
  acceptedCount: S.Number,
  artifactProducingCount: S.Number,
  assignedCount: S.Number,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  benchmarkedCount: S.Number,
  byDarkCapacityReason: S.Array(PylonCapacityFunnelCount),
  byStage: S.Array(PylonCapacityFunnelCount),
  darkCount: S.Number,
  eligibleCount: S.Number,
  paidCount: S.Number,
  registeredCount: S.Number,
  runningCount: S.Number,
  settledClaimAllowedCount: S.Number,
  settledCount: S.Number,
  totalCount: S.Number,
}) {}

export class PylonCapacityDarkReasonAccounting extends S.Class<PylonCapacityDarkReasonAccounting>(
  'PylonCapacityDarkReasonAccounting',
)({
  capacityRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  count: S.Number,
  evidenceRefs: S.Array(S.String),
  reasonRef: S.String,
  workClassRefs: S.Array(S.String),
}) {}

export class PylonCapacityFunnelAccountingProjection extends S.Class<PylonCapacityFunnelAccountingProjection>(
  'PylonCapacityFunnelAccountingProjection',
)({
  acceptedCount: S.Number,
  artifactProducingCount: S.Number,
  assignedCount: S.Number,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  authority: PylonCapacityFunnelAccountingAuthority,
  benchmarkedCount: S.Number,
  byDarkCapacityReason: S.Array(PylonCapacityDarkReasonAccounting),
  byStage: S.Array(PylonCapacityFunnelCount),
  capacityAssignmentMutationAllowed: S.Boolean,
  claimBoundaryCaveatRefs: S.Array(S.String),
  darkCount: S.Number,
  darkReasonCount: S.Number,
  eligibleCount: S.Number,
  freshCount: S.Number,
  liveWalletSpendAllowed: S.Boolean,
  paidButNotSettledCount: S.Number,
  paidCount: S.Number,
  payoutDispatchMutationAllowed: S.Boolean,
  payoutTargetMutationAllowed: S.Boolean,
  providerEligibilityMutationAllowed: S.Boolean,
  publicClaimUpgradeAllowed: S.Boolean,
  registeredCount: S.Number,
  runningCount: S.Number,
  settledCount: S.Number,
  settlementMutationAllowed: S.Boolean,
  settledWithoutVisibleReceiptCount: S.Number,
  staleCapacityRefs: S.Array(S.String),
  staleCount: S.Number,
  totalCount: S.Number,
  unknownFreshnessCount: S.Number,
  updatedAtDisplay: S.NullOr(S.String),
  visibleSettlementClaimAllowedCount: S.Number,
}) {}

export class PylonCapacityFunnelUnsafe extends S.TaggedErrorClass<PylonCapacityFunnelUnsafe>()(
  'PylonCapacityFunnelUnsafe',
  {
    reason: S.String,
  },
) {}

const stageLabelByStage: Record<PylonCapacityFunnelStage, string> = {
  accepted: 'Accepted',
  artifact_producing: 'Artifact producing',
  assigned: 'Assigned',
  benchmarked: 'Benchmarked',
  dark: 'Dark',
  eligible: 'Eligible',
  paid: 'Paid',
  registered: 'Registered',
  running: 'Running',
  settled: 'Settled',
}

const stageRank: Record<PylonCapacityFunnelStage, number> = {
  accepted: 6,
  artifact_producing: 5,
  assigned: 3,
  benchmarked: 1,
  dark: -1,
  eligible: 2,
  paid: 7,
  registered: 0,
  running: 4,
  settled: 8,
}

export const PYLON_CAPACITY_FUNNEL_ACCOUNTING_READ_ONLY_AUTHORITY:
  PylonCapacityFunnelAccountingAuthority = {
    authorityBoundary: 'read_only_capacity_accounting',
    noCapacityAssignmentMutation: true,
    noLiveWalletSpend: true,
    noPayoutDispatch: true,
    noPayoutTargetMutation: true,
    noProviderEligibilityMutation: true,
    noPublicClaimUpgrade: true,
    noSettlementMutation: true,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hostname|invoice|lnbc|lntb|lnbcrt|lno1|mac[_-]?address|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination)|preimage|private[_-]?hardware|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(host|invoice|payment|payload|prompt|payout[_-]?target|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|serial[_-]?number|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(node\.private|provider\.private|reward\.private|settlement\.private)/i
const customerUnsafeRefPattern =
  /(node\.private|provider\.private|reward\.private|settlement\.private)/i
const teamUnsafeRefPattern =
  /(node\.private|provider\.private|settlement\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const stageAtLeast = (
  stage: PylonCapacityFunnelStage,
  threshold: PylonCapacityFunnelStage,
): boolean => stageRank[stage] >= stageRank[threshold]

// Platform-issued dark-capacity and device-admission taxonomy
// constants are closed enums; the substring heuristic must not reject
// its own taxonomy (the live funnel 500 of 2026-06-11:
// dark_capacity.public.wallet_not_ready contains 'wallet' and fired
// for the first time when the first capability-eligible, wallet-unready
// Pylon came online). device_admission.public.* reason codes are the
// reasoned hardware admission gates of issue #4852 (Pluralis roadmap
// P1.4) surfacing through the same funnel reason-ref channel.
// device_capability.public.* reason codes are receipt-backed capability dataset
// signals such as thermal-throttle status; they are taxonomy refs, not device
// identifiers or raw hardware payloads.
const platformIssuedDarkReasonPattern =
  /^(dark_capacity|device_admission|device_capability)\.public\.[a-z0-9_]+$/

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    (!platformIssuedDarkReasonPattern.test(ref) &&
      universallyUnsafeRefPattern.test(ref)) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new PylonCapacityFunnelUnsafe({
      reason: `${label} contains private host, hardware, provider, runner, wallet, payment, payout target, customer, private repo, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
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
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const visibleIdentityRef = (
  label: string,
  ref: string,
  visibility: PylonCapacityVisibility,
  redactedRef: string,
  audience: BlueprintMissionBriefingAudience,
): string => {
  if (visibility === 'public' || audience === 'operator') {
    return safeRefsForAudience(label, [ref], audience)[0] ?? redactedRef
  }

  return redactedRef
}

const assertRecordSafe = (record: PylonCapacityFunnelRecord): void => {
  assertSafeRefs('capacity identity refs', [
    record.id,
    record.capacityRef,
    record.nodeRef,
    record.providerRef,
  ])
  assertSafeRefs('capacity benchmark refs', record.benchmarkRefs)
  assertSafeRefs('capacity eligibility refs', record.eligibilityRefs)
  assertSafeRefs('capacity assignment refs', record.assignmentRefs)
  assertSafeRefs('capacity run refs', record.runRefs)
  assertSafeRefs('capacity artifact refs', record.artifactRefs)
  assertSafeRefs('capacity acceptance refs', record.acceptanceRefs)
  assertSafeRefs('capacity reward refs', record.rewardRefs)
  assertSafeRefs('capacity settlement refs', record.settlementRefs)
  assertSafeRefs('capacity dark reason refs', record.darkCapacityReasonRefs)
  assertSafeRefs('capacity caveat refs', record.caveatRefs)
  assertSafeRefs('capacity evidence refs', record.evidenceRefs)
  assertSafeRefs('capacity work class refs', record.workClassRefs)

  if (record.stage === 'dark' && record.darkCapacityReasonRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Dark capacity records require dark-capacity reason refs.',
    })
  }

  if (stageAtLeast(record.stage, 'benchmarked') && record.benchmarkRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Benchmarked capacity requires benchmark refs.',
    })
  }

  if (stageAtLeast(record.stage, 'eligible') && record.eligibilityRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Eligible capacity requires eligibility refs.',
    })
  }

  if (stageAtLeast(record.stage, 'assigned') && record.assignmentRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Assigned capacity requires assignment refs.',
    })
  }

  if (stageAtLeast(record.stage, 'running') && record.runRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Running capacity requires run refs.',
    })
  }

  if (
    stageAtLeast(record.stage, 'artifact_producing') &&
    record.artifactRefs.length === 0
  ) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Artifact-producing capacity requires artifact refs.',
    })
  }

  if (stageAtLeast(record.stage, 'accepted') && record.acceptanceRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Accepted capacity requires acceptance refs.',
    })
  }

  if (stageAtLeast(record.stage, 'paid') && record.rewardRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Paid capacity requires reward refs.',
    })
  }

  if (record.stage === 'settled' && record.settlementRefs.length === 0) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Settled capacity requires settlement refs.',
    })
  }
}

const projectionText = (
  projection: PylonCapacityFunnelProjection,
): string =>
  [
    projection.id,
    projection.capacityRef,
    projection.nodeRef,
    projection.providerRef,
    ...projection.benchmarkRefs,
    ...projection.eligibilityRefs,
    ...projection.assignmentRefs,
    ...projection.runRefs,
    ...projection.artifactRefs,
    ...projection.acceptanceRefs,
    ...projection.rewardRefs,
    ...projection.settlementRefs,
    ...projection.darkCapacityReasonRefs,
    ...projection.caveatRefs,
    ...projection.evidenceRefs,
    ...projection.workClassRefs,
  ].join(' ')

export const pylonCapacityProjectionHasPrivateMaterial = (
  projection: PylonCapacityFunnelProjection,
): boolean => {
  // Strip platform-issued dark-reason and device-admission taxonomy
  // constants before the substring scan - the closed enums must not
  // trip their own scanner (dark_capacity.public.wallet_not_ready
  // contains 'wallet').
  const text = projectionText(projection)
    .replaceAll(
      /dark_capacity\.public\.[a-z0-9_]+/g,
      'dark_capacity.public.reason',
    )
    .replaceAll(
      /device_admission\.public\.[a-z0-9_]+/g,
      'device_admission.public.reason',
    )
    .replaceAll(
      /device_capability\.public\.[a-z0-9_]+/g,
      'device_capability.public.reason',
    )
  const pattern = audienceUnsafePattern(projection.audience)

  return universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonCapacityFunnelRecord = (
  record: PylonCapacityFunnelRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonCapacityFunnelProjection => {
  assertRecordSafe(record)

  const projection: PylonCapacityFunnelProjection = {
    acceptanceRefs: safeRefsForAudience(
      'capacity acceptance refs',
      record.acceptanceRefs,
      audience,
    ),
    acceptedCapacityClaimAllowed:
      stageAtLeast(record.stage, 'accepted') && record.acceptanceRefs.length > 0,
    artifactRefs: safeRefsForAudience(
      'capacity artifact refs',
      record.artifactRefs,
      audience,
    ),
    assignmentRefs: safeRefsForAudience(
      'capacity assignment refs',
      record.assignmentRefs,
      audience,
    ),
    audience,
    benchmarkRefs: safeRefsForAudience(
      'capacity benchmark refs',
      record.benchmarkRefs,
      audience,
    ),
    capacityRef: record.capacityRef,
    caveatRefs: safeRefsForAudience('capacity caveat refs', record.caveatRefs, audience),
    darkCapacityReasonRefs: safeRefsForAudience(
      'capacity dark reason refs',
      record.darkCapacityReasonRefs,
      audience,
    ),
    eligibilityRefs: safeRefsForAudience(
      'capacity eligibility refs',
      record.eligibilityRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'capacity evidence refs',
      record.evidenceRefs,
      audience,
    ),
    id: record.id,
    nodeRef: visibleIdentityRef(
      'capacity node ref',
      record.nodeRef,
      record.nodeVisibility,
      'node.redacted',
      audience,
    ),
    nodeVisibility: record.nodeVisibility,
    paidCapacityClaimAllowed:
      stageAtLeast(record.stage, 'paid') && record.rewardRefs.length > 0,
    providerRef: visibleIdentityRef(
      'capacity provider ref',
      record.providerRef,
      record.providerVisibility,
      'provider.redacted',
      audience,
    ),
    providerVisibility: record.providerVisibility,
    rewardRefs: audience === 'operator'
      ? safeRefsForAudience('capacity reward refs', record.rewardRefs, audience)
      : [],
    runRefs: safeRefsForAudience('capacity run refs', record.runRefs, audience),
    settledCapacityClaimAllowed:
      record.stage === 'settled' && record.settlementRefs.length > 0,
    settlementRefs: audience === 'operator'
      ? safeRefsForAudience(
        'capacity settlement refs',
        record.settlementRefs,
        audience,
      )
      : [],
    stage: record.stage,
    stageLabel: stageLabelByStage[record.stage],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workClassRefs: safeRefsForAudience(
      'capacity work class refs',
      record.workClassRefs,
      audience,
    ),
  }

  if (pylonCapacityProjectionHasPrivateMaterial(projection)) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Capacity funnel projection contains private material.',
    })
  }

  return projection
}

const countBy = (
  values: ReadonlyArray<string>,
): ReadonlyArray<PylonCapacityFunnelCount> =>
  [...values.reduce((counts, value) => {
    counts.set(value, (counts.get(value) ?? 0) + 1)

    return counts
  }, new Map<string, number>())]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, count]) => ({ count, key }))

const countStage = (
  projections: ReadonlyArray<PylonCapacityFunnelProjection>,
  stage: PylonCapacityFunnelStage,
): number =>
  projections.filter(projection => projection.stage === stage).length

const latestUpdatedAtIso = (
  records: ReadonlyArray<PylonCapacityFunnelRecord>,
): string | null =>
  [...records]
    .map(record => record.updatedAtIso)
    .filter(iso => Number.isFinite(Date.parse(iso)))
    .sort()
    .at(-1) ?? null

const freshnessClass = (
  updatedAtIso: string,
  nowIso: string,
): 'fresh' | 'stale' | 'unknown' => {
  const updatedAt = Date.parse(updatedAtIso)
  const now = Date.parse(nowIso)

  if (!Number.isFinite(updatedAt) || !Number.isFinite(now)) {
    return 'unknown'
  }

  return now - updatedAt > 24 * 60 * 60 * 1000 ? 'stale' : 'fresh'
}

const darkReasonSummaries = (
  projections: ReadonlyArray<PylonCapacityFunnelProjection>,
): ReadonlyArray<PylonCapacityDarkReasonAccounting> => {
  const summaries = new Map<string, {
    capacityRefs: Array<string>
    caveatRefs: Array<string>
    evidenceRefs: Array<string>
    workClassRefs: Array<string>
  }>()

  for (const projection of projections.filter(item => item.stage === 'dark')) {
    for (const reasonRef of projection.darkCapacityReasonRefs) {
      const existing = summaries.get(reasonRef) ?? {
        capacityRefs: [],
        caveatRefs: [],
        evidenceRefs: [],
        workClassRefs: [],
      }
      existing.capacityRefs.push(projection.capacityRef)
      existing.caveatRefs.push(...projection.caveatRefs)
      existing.evidenceRefs.push(...projection.evidenceRefs)
      existing.workClassRefs.push(...projection.workClassRefs)
      summaries.set(reasonRef, existing)
    }
  }

  return [...summaries.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([reasonRef, summary]) => ({
      capacityRefs: uniqueRefs(summary.capacityRefs),
      caveatRefs: uniqueRefs(summary.caveatRefs),
      count: uniqueRefs(summary.capacityRefs).length,
      evidenceRefs: uniqueRefs(summary.evidenceRefs),
      reasonRef,
      workClassRefs: uniqueRefs(summary.workClassRefs),
    }))
}

export const aggregatePylonCapacityFunnel = (
  records: ReadonlyArray<PylonCapacityFunnelRecord>,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonCapacityFunnelAggregate => {
  const projections = records.map(record =>
    projectPylonCapacityFunnelRecord(record, audience, nowIso),
  )

  return {
    acceptedCount: countStage(projections, 'accepted'),
    artifactProducingCount: countStage(projections, 'artifact_producing'),
    assignedCount: countStage(projections, 'assigned'),
    audience,
    benchmarkedCount: countStage(projections, 'benchmarked'),
    byDarkCapacityReason: countBy(
      projections.flatMap(projection => projection.darkCapacityReasonRefs),
    ),
    byStage: countBy(projections.map(projection => projection.stage)),
    darkCount: countStage(projections, 'dark'),
    eligibleCount: countStage(projections, 'eligible'),
    paidCount: countStage(projections, 'paid'),
    registeredCount: countStage(projections, 'registered'),
    runningCount: countStage(projections, 'running'),
    settledClaimAllowedCount: projections.filter(
      projection => projection.settledCapacityClaimAllowed,
    ).length,
    settledCount: countStage(projections, 'settled'),
    totalCount: projections.length,
  }
}

const accountingProjectionText = (
  projection: PylonCapacityFunnelAccountingProjection,
): string =>
  [
    ...projection.claimBoundaryCaveatRefs,
    ...projection.staleCapacityRefs,
    ...projection.byStage.flatMap(count => [count.key]),
    ...projection.byDarkCapacityReason.flatMap(summary => [
      summary.reasonRef,
      ...summary.capacityRefs,
      ...summary.caveatRefs,
      ...summary.evidenceRefs,
      ...summary.workClassRefs,
    ]),
  ].join(' ')

export const pylonCapacityAccountingProjectionHasPrivateMaterial = (
  projection: PylonCapacityFunnelAccountingProjection,
): boolean => {
  const text = accountingProjectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return universallyUnsafeRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const accountPylonCapacityFunnel = (
  records: ReadonlyArray<PylonCapacityFunnelRecord>,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): PylonCapacityFunnelAccountingProjection => {
  const projections = records.map(record =>
    projectPylonCapacityFunnelRecord(record, audience, nowIso),
  )
  const aggregate = aggregatePylonCapacityFunnel(records, audience, nowIso)
  const staleCapacityRefs = uniqueRefs(
    projections
      .filter((projection, index) =>
        freshnessClass(records[index]!.updatedAtIso, nowIso) === 'stale',
      )
      .map(projection => projection.capacityRef),
  )
  const latestIso = latestUpdatedAtIso(records)
  const visibleSettlementClaimAllowedCount = projections.filter(
    projection =>
      projection.stage === 'settled' &&
      projection.settlementRefs.length > 0,
  ).length
  const settledWithoutVisibleReceiptCount = projections.filter(
    projection =>
      projection.stage === 'settled' &&
      projection.settlementRefs.length === 0,
  ).length
  const projection: PylonCapacityFunnelAccountingProjection = {
    acceptedCount: aggregate.acceptedCount,
    artifactProducingCount: aggregate.artifactProducingCount,
    assignedCount: aggregate.assignedCount,
    audience,
    authority: PYLON_CAPACITY_FUNNEL_ACCOUNTING_READ_ONLY_AUTHORITY,
    benchmarkedCount: aggregate.benchmarkedCount,
    byDarkCapacityReason: darkReasonSummaries(projections),
    byStage: aggregate.byStage,
    capacityAssignmentMutationAllowed: false,
    claimBoundaryCaveatRefs: uniqueRefs(
      projections.flatMap(item => item.caveatRefs),
    ),
    darkCount: aggregate.darkCount,
    darkReasonCount: aggregate.byDarkCapacityReason.length,
    eligibleCount: aggregate.eligibleCount,
    freshCount: records.filter(
      record => freshnessClass(record.updatedAtIso, nowIso) === 'fresh',
    ).length,
    liveWalletSpendAllowed: false,
    paidButNotSettledCount: projections.filter(
      projection => projection.stage === 'paid',
    ).length,
    paidCount: aggregate.paidCount,
    payoutDispatchMutationAllowed: false,
    payoutTargetMutationAllowed: false,
    providerEligibilityMutationAllowed: false,
    publicClaimUpgradeAllowed: false,
    registeredCount: aggregate.registeredCount,
    runningCount: aggregate.runningCount,
    settledCount: aggregate.settledCount,
    settlementMutationAllowed: false,
    settledWithoutVisibleReceiptCount,
    staleCapacityRefs,
    staleCount: staleCapacityRefs.length,
    totalCount: aggregate.totalCount,
    unknownFreshnessCount: records.filter(
      record => freshnessClass(record.updatedAtIso, nowIso) === 'unknown',
    ).length,
    updatedAtDisplay:
      latestIso === null
        ? null
        : friendlyBlueprintMissionBriefingTime(latestIso, nowIso),
    visibleSettlementClaimAllowedCount,
  }

  if (pylonCapacityAccountingProjectionHasPrivateMaterial(projection)) {
    throw new PylonCapacityFunnelUnsafe({
      reason: 'Capacity funnel accounting projection contains private material.',
    })
  }

  return projection
}

export const examplePylonCapacityFunnelRecords =
  (): ReadonlyArray<PylonCapacityFunnelRecord> => [
    {
      acceptanceRefs: ['acceptance.capacity.demo_1'],
      artifactRefs: ['artifact.capacity.demo_1'],
      assignmentRefs: ['assignment.capacity.demo_1'],
      benchmarkRefs: ['benchmark.capacity.demo_1'],
      capacityRef: 'capacity.pylon_demo_1',
      caveatRefs: ['caveat.capacity.not_settlement_claim'],
      darkCapacityReasonRefs: [],
      eligibilityRefs: ['eligibility.capacity.demo_1'],
      evidenceRefs: ['evidence.capacity.demo_1'],
      id: 'capacity_funnel_demo_1',
      nodeRef: 'node.public_demo_1',
      nodeVisibility: 'public',
      providerRef: 'provider.public_demo_1',
      providerVisibility: 'public',
      rewardRefs: ['reward.capacity.demo_1'],
      runRefs: ['run.capacity.demo_1'],
      settlementRefs: ['settlement.capacity.demo_1'],
      stage: 'settled',
      updatedAtIso: '2026-06-06T21:35:00.000Z',
      workClassRefs: ['work_class.flexible_inference'],
    },
    {
      acceptanceRefs: [],
      artifactRefs: [],
      assignmentRefs: [],
      benchmarkRefs: ['benchmark.capacity.demo_2'],
      capacityRef: 'capacity.pylon_demo_2',
      caveatRefs: ['caveat.capacity.dark_until_routed'],
      darkCapacityReasonRefs: [
        'dark_reason.no_work_assigned',
        'dark_reason.missing_payout_target',
      ],
      eligibilityRefs: ['eligibility.capacity.demo_2'],
      evidenceRefs: ['evidence.capacity.demo_2'],
      id: 'capacity_funnel_demo_2',
      nodeRef: 'node.private_demo_2',
      nodeVisibility: 'private',
      providerRef: 'provider.private_demo_2',
      providerVisibility: 'private',
      rewardRefs: [],
      runRefs: [],
      settlementRefs: [],
      stage: 'dark',
      updatedAtIso: '2026-06-06T21:35:00.000Z',
      workClassRefs: ['work_class.flexible_inference'],
    },
  ]
