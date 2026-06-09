import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const PylonForwardPowerScenarioKind = S.Literals([
  'forward_power_window',
  'interconnection_value',
])
export type PylonForwardPowerScenarioKind =
  typeof PylonForwardPowerScenarioKind.Type

export const PylonForwardPowerClaimState = S.Literals([
  'contracted',
  'measured',
  'modeled',
  'settled',
])
export type PylonForwardPowerClaimState =
  typeof PylonForwardPowerClaimState.Type

export const PylonForwardPowerScenarioAuthorityBoundary = S.Literals([
  'read_only_scenario_projection',
])
export type PylonForwardPowerScenarioAuthorityBoundary =
  typeof PylonForwardPowerScenarioAuthorityBoundary.Type

export class PylonForwardPowerScenarioAuthority extends S.Class<PylonForwardPowerScenarioAuthority>(
  'PylonForwardPowerScenarioAuthority',
)({
  authorityBoundary: PylonForwardPowerScenarioAuthorityBoundary,
  noCapacityDispatch: S.Boolean,
  noFinancialAdvice: S.Boolean,
  noGridParticipation: S.Boolean,
  noInterconnectionMutation: S.Boolean,
  noPowerTrading: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class PylonForwardPowerScenarioRecord extends S.Class<PylonForwardPowerScenarioRecord>(
  'PylonForwardPowerScenarioRecord',
)({
  assumptionRefs: S.Array(S.String),
  authority: PylonForwardPowerScenarioAuthority,
  avoidedDelayCostCents: S.Number,
  avoidedDelayDays: S.Number,
  avoidedUpgradeCostCents: S.Number,
  caveatRefs: S.Array(S.String),
  contractRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  facilityRef: S.String,
  id: S.String,
  interconnectionRefs: S.Array(S.String),
  proofOfResponseRefs: S.Array(S.String),
  scenarioKind: PylonForwardPowerScenarioKind,
  scenarioRef: S.String,
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonForwardPowerClaimState,
  updatedAtIso: S.String,
  unusedPowerWattHours: S.Number,
  workloadFitBps: S.Number,
  workloadFitRefs: S.Array(S.String),
}) {}

export class PylonForwardPowerScenarioProjection extends S.Class<PylonForwardPowerScenarioProjection>(
  'PylonForwardPowerScenarioProjection',
)({
  assumptionRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  authority: PylonForwardPowerScenarioAuthority,
  avoidedCostTotalCents: S.Number,
  avoidedDelayCostCents: S.Number,
  avoidedDelayDays: S.Number,
  avoidedDelayValueClaimAllowed: S.Boolean,
  avoidedUpgradeCostCents: S.Number,
  avoidedUpgradeValueClaimAllowed: S.Boolean,
  capacityDispatchAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  contractRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  facilityRef: S.String,
  financialAdviceAllowed: S.Boolean,
  gridParticipationAllowed: S.Boolean,
  id: S.String,
  interconnectionMutationAllowed: S.Boolean,
  interconnectionRefs: S.Array(S.String),
  measuredPowerClaimAllowed: S.Boolean,
  modeledScenario: S.Boolean,
  powerTradingAllowed: S.Boolean,
  proofOfResponseRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  scenarioKind: PylonForwardPowerScenarioKind,
  scenarioKindLabel: S.String,
  scenarioRef: S.String,
  settlementClaimAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: PylonForwardPowerClaimState,
  stateLabel: S.String,
  updatedAtDisplay: S.String,
  unusedPowerMwh: S.Number,
  unusedPowerWattHours: S.Number,
  workloadFitBps: S.Number,
  workloadFitRefs: S.Array(S.String),
  workloadFitPercent: S.Number,
}) {}

export class PylonForwardPowerScenarioUnsafe extends S.TaggedErrorClass<PylonForwardPowerScenarioUnsafe>()(
  'PylonForwardPowerScenarioUnsafe',
  {
    reason: S.String,
  },
) {}

export const PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY:
  PylonForwardPowerScenarioAuthority = {
    authorityBoundary: 'read_only_scenario_projection',
    noCapacityDispatch: true,
    noFinancialAdvice: true,
    noGridParticipation: true,
    noInterconnectionMutation: true,
    noPowerTrading: true,
    noPublicClaimUpgrade: true,
    noSettlementMutation: true,
  }

const scenarioKindLabelByKind:
  Record<PylonForwardPowerScenarioKind, string> = {
    forward_power_window: 'Forward-power window',
    interconnection_value: 'Interconnection value',
  }

const stateLabelByState: Record<PylonForwardPowerClaimState, string> = {
  contracted: 'Contracted',
  measured: 'Measured',
  modeled: 'Modeled',
  settled: 'Settled',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeForwardPowerRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mac[_-]?address|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(hardware|key|wallet)|provider[_-]?(grant|payload|secret|telemetry|token)|raw[_-]?(contract|energy|host|interconnection|invoice|meter|payment|payload|payout|power|prompt|provider|runner|run[_-]?log|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|trading[_-]?(account|order)|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(assumption\.private|caveat\.private|contract\.private|evidence\.private|facility\.private|interconnection\.private|proof\.private|settlement\.private|source\.private|workload\.private)/i
const teamUnsafeRefPattern =
  /(contract\.private|facility\.private|interconnection\.private|settlement\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeForwardPowerRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: `${label} contains private facility, interconnection, contract, provider, wallet, payment, trading, raw telemetry, raw meter, raw power, private repo, secret, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent' || audience === 'customer') {
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
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertRecordSafe = (record: PylonForwardPowerScenarioRecord): void => {
  assertSafeRefs('forward-power identity refs', [
    record.id,
    record.scenarioRef,
    record.facilityRef,
  ])
  assertSafeRefs('forward-power assumption refs', record.assumptionRefs)
  assertSafeRefs('forward-power caveat refs', record.caveatRefs)
  assertSafeRefs('forward-power contract refs', record.contractRefs)
  assertSafeRefs('forward-power evidence refs', record.evidenceRefs)
  assertSafeRefs('forward-power interconnection refs', record.interconnectionRefs)
  assertSafeRefs('forward-power proof response refs', record.proofOfResponseRefs)
  assertSafeRefs('forward-power settlement refs', record.settlementRefs)
  assertSafeRefs('forward-power source refs', record.sourceRefs)
  assertSafeRefs('forward-power workload refs', record.workloadFitRefs)

  Object.entries({
    avoidedDelayCostCents: record.avoidedDelayCostCents,
    avoidedDelayDays: record.avoidedDelayDays,
    avoidedUpgradeCostCents: record.avoidedUpgradeCostCents,
    unusedPowerWattHours: record.unusedPowerWattHours,
    workloadFitBps: record.workloadFitBps,
  }).forEach(([label, value]) => assertNonNegativeInteger(label, value))

  if (record.workloadFitBps > 10000) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: 'workloadFitBps cannot exceed 10000.',
    })
  }

  if (
    record.authority.noCapacityDispatch !== true ||
    record.authority.noFinancialAdvice !== true ||
    record.authority.noGridParticipation !== true ||
    record.authority.noInterconnectionMutation !== true ||
    record.authority.noPowerTrading !== true ||
    record.authority.noPublicClaimUpgrade !== true ||
    record.authority.noSettlementMutation !== true
  ) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason:
        'Forward-power scenarios must remain read-only and cannot dispatch capacity, give financial advice, join grid programs, mutate interconnection state, trade power, upgrade public claims, or mutate settlement.',
    })
  }

  if (!hasRefs(record.assumptionRefs) || !hasRefs(record.caveatRefs)) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: 'Forward-power scenarios require assumption refs and caveat refs.',
    })
  }

  if (
    (record.avoidedDelayCostCents > 0 ||
      record.avoidedDelayDays > 0 ||
      record.avoidedUpgradeCostCents > 0) &&
    !hasRefs(record.interconnectionRefs)
  ) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason:
        'Avoided cost or avoided delay scenarios require interconnection refs.',
    })
  }

  if (
    (record.state === 'measured' || record.state === 'settled') &&
    !hasRefs(record.proofOfResponseRefs)
  ) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: 'Measured or settled scenarios require proof-of-response refs.',
    })
  }

  if (
    (record.state === 'contracted' || record.state === 'settled') &&
    !hasRefs(record.contractRefs)
  ) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: 'Contracted or settled scenarios require contract refs.',
    })
  }

  if (record.state === 'settled' && !hasRefs(record.settlementRefs)) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason: 'Settled scenarios require settlement refs.',
    })
  }
}

const rounded = (value: number): number => Math.round(value * 1000) / 1000

const projectionText = (
  projection: PylonForwardPowerScenarioProjection,
): string =>
  [
    projection.id,
    projection.scenarioRef,
    projection.facilityRef,
    ...projection.assumptionRefs,
    ...projection.caveatRefs,
    ...projection.contractRefs,
    ...projection.evidenceRefs,
    ...projection.interconnectionRefs,
    ...projection.proofOfResponseRefs,
    ...projection.settlementRefs,
    ...projection.sourceRefs,
    ...projection.workloadFitRefs,
  ].join(' ')

export const pylonForwardPowerScenarioProjectionHasPrivateMaterial = (
  projection: PylonForwardPowerScenarioProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeForwardPowerRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectPylonForwardPowerScenario = (
  record: PylonForwardPowerScenarioRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): PylonForwardPowerScenarioProjection => {
  assertRecordSafe(record)

  const proofOfResponseRefs = safeRefsForAudience(
    'forward-power proof response refs',
    record.proofOfResponseRefs,
    audience,
  )
  const settlementRefs = safeRefsForAudience(
    'forward-power settlement refs',
    record.settlementRefs,
    audience,
  )
  const projection: PylonForwardPowerScenarioProjection = {
    assumptionRefs: safeRefsForAudience(
      'forward-power assumption refs',
      record.assumptionRefs,
      audience,
    ),
    audience,
    authority: record.authority,
    avoidedCostTotalCents:
      record.avoidedDelayCostCents + record.avoidedUpgradeCostCents,
    avoidedDelayCostCents: record.avoidedDelayCostCents,
    avoidedDelayDays: record.avoidedDelayDays,
    avoidedDelayValueClaimAllowed:
      record.avoidedDelayDays > 0 &&
      hasRefs(record.interconnectionRefs) &&
      hasRefs(record.caveatRefs),
    avoidedUpgradeCostCents: record.avoidedUpgradeCostCents,
    avoidedUpgradeValueClaimAllowed:
      record.avoidedUpgradeCostCents > 0 &&
      hasRefs(record.interconnectionRefs) &&
      hasRefs(record.caveatRefs),
    capacityDispatchAllowed: false,
    caveatRefs: safeRefsForAudience(
      'forward-power caveat refs',
      record.caveatRefs,
      audience,
    ),
    contractRefs: safeRefsForAudience(
      'forward-power contract refs',
      record.contractRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience(
      'forward-power evidence refs',
      record.evidenceRefs,
      audience,
    ),
    facilityRef: safeRefsForAudience(
      'forward-power facility ref',
      [record.facilityRef],
      audience,
    )[0] ?? 'facility.redacted',
    financialAdviceAllowed: false,
    gridParticipationAllowed: false,
    id: record.id,
    interconnectionMutationAllowed: false,
    interconnectionRefs: safeRefsForAudience(
      'forward-power interconnection refs',
      record.interconnectionRefs,
      audience,
    ),
    measuredPowerClaimAllowed:
      (record.state === 'measured' || record.state === 'settled') &&
      proofOfResponseRefs.length > 0,
    modeledScenario: record.state === 'modeled',
    powerTradingAllowed: false,
    proofOfResponseRefs,
    publicClaimUpgradeAllowed: false,
    scenarioKind: record.scenarioKind,
    scenarioKindLabel: scenarioKindLabelByKind[record.scenarioKind],
    scenarioRef: record.scenarioRef,
    settlementClaimAllowed:
      record.state === 'settled' && settlementRefs.length > 0,
    settlementMutationAllowed: false,
    settlementRefs,
    sourceRefs: safeRefsForAudience(
      'forward-power source refs',
      record.sourceRefs,
      audience,
    ),
    state: record.state,
    stateLabel: stateLabelByState[record.state],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    unusedPowerMwh: rounded(record.unusedPowerWattHours / 1_000_000),
    unusedPowerWattHours: record.unusedPowerWattHours,
    workloadFitBps: record.workloadFitBps,
    workloadFitRefs: safeRefsForAudience(
      'forward-power workload refs',
      record.workloadFitRefs,
      audience,
    ),
    workloadFitPercent: rounded(record.workloadFitBps / 100),
  }

  if (pylonForwardPowerScenarioProjectionHasPrivateMaterial(projection)) {
    throw new PylonForwardPowerScenarioUnsafe({
      reason:
        'Forward-power scenario projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const examplePylonForwardPowerScenario =
  (): PylonForwardPowerScenarioRecord => ({
    assumptionRefs: ['assumption.public.workload_fit_v1'],
    authority: PYLON_FORWARD_POWER_SCENARIO_READ_ONLY_AUTHORITY,
    avoidedDelayCostCents: 2500000,
    avoidedDelayDays: 45,
    avoidedUpgradeCostCents: 15000000,
    caveatRefs: ['caveat.public.modeled_not_financial_advice'],
    contractRefs: [],
    createdAtIso: '2026-06-06T23:20:00.000Z',
    evidenceRefs: ['evidence.public.power_inventory_summary'],
    facilityRef: 'facility.public_demo_1',
    id: 'forward_power_scenario.demo_1',
    interconnectionRefs: ['interconnection.public.queue_position'],
    proofOfResponseRefs: [],
    scenarioKind: 'interconnection_value',
    scenarioRef: 'scenario.forward_power.demo_1',
    settlementRefs: [],
    sourceRefs: ['source.public.utility_tariff_summary'],
    state: 'modeled',
    updatedAtIso: '2026-06-06T23:25:00.000Z',
    unusedPowerWattHours: 5000000,
    workloadFitBps: 7200,
    workloadFitRefs: ['workload.public.flexible_inference'],
  })
