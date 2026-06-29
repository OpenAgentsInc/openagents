import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const MargotExportMarket = S.Literals([
  'ercot',
  'nyiso',
  'unsupported',
])
export type MargotExportMarket = typeof MargotExportMarket.Type

export const MargotExportDispatchPolicy = S.Literals([
  'binary_on_off',
  'intelligent_miner',
  'linear_gradient',
  'operator_defined',
  'threshold',
])
export type MargotExportDispatchPolicy =
  typeof MargotExportDispatchPolicy.Type

export const MargotExportClaimState = S.Literals([
  'measured',
  'modeled',
  'settled',
])
export type MargotExportClaimState = typeof MargotExportClaimState.Type

export const MargotExportAuthorityBoundary = S.Literals([
  'read_only_simulator_packet',
])
export type MargotExportAuthorityBoundary =
  typeof MargotExportAuthorityBoundary.Type

export class MargotExportAuthority extends S.Class<MargotExportAuthority>(
  'MargotExportAuthority',
)({
  authorityBoundary: MargotExportAuthorityBoundary,
  noAcceptedWorkMutation: S.Boolean,
  noFinancialAdvice: S.Boolean,
  noGridParticipation: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noMarketDataMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class MargotExportPacketRecord extends S.Class<MargotExportPacketRecord>(
  'MargotExportPacketRecord',
)({
  acceptedOutcomeAssumptionRefs: S.Array(S.String),
  acceptedWorkCentsPerMwh: S.Number,
  authority: MargotExportAuthority,
  caveatRefs: S.Array(S.String),
  claimState: MargotExportClaimState,
  curtailmentValueCentsPerMwh: S.Number,
  dataRightsRefs: S.Array(S.String),
  dispatchPolicy: MargotExportDispatchPolicy,
  exportRef: S.String,
  generatedAtIso: S.String,
  gpuRentalFloorCentsPerMwh: S.Number,
  gridServiceCentsPerMwh: S.Number,
  id: S.String,
  market: MargotExportMarket,
  miningFloorCentsPerMwh: S.Number,
  nextDiligenceRefs: S.Array(S.String),
  powerCostCentsPerMwh: S.Number,
  provenanceRefs: S.Array(S.String),
  scenarioRefs: S.Array(S.String),
  simulatorName: S.String,
  simulatorVersionRef: S.String,
  settlementRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class MargotExportPacketProjection extends S.Class<MargotExportPacketProjection>(
  'MargotExportPacketProjection',
)({
  acceptedOutcomeAssumptionRefs: S.Array(S.String),
  acceptedWorkCentsPerMwh: S.Number,
  acceptedWorkLaneClaimAllowed: S.Boolean,
  acceptedWorkMutationAllowed: S.Boolean,
  audience: OmniProjectionAudience,
  authority: MargotExportAuthority,
  caveatRefs: S.Array(S.String),
  claimState: MargotExportClaimState,
  claimStateLabel: S.String,
  curtailmentValueCentsPerMwh: S.Number,
  dataRightsRefs: S.Array(S.String),
  dispatchPolicy: MargotExportDispatchPolicy,
  dispatchPolicyLabel: S.String,
  exportRef: S.String,
  financialAdviceAllowed: S.Boolean,
  generatedAtDisplay: S.String,
  gpuRentalFloorCentsPerMwh: S.Number,
  gridParticipationAllowed: S.Boolean,
  gridServiceCentsPerMwh: S.Number,
  id: S.String,
  liveWalletSpendAllowed: S.Boolean,
  market: MargotExportMarket,
  marketDataMutationAllowed: S.Boolean,
  marketLabel: S.String,
  miningFloorCentsPerMwh: S.Number,
  nextDiligenceRefs: S.Array(S.String),
  powerCostCentsPerMwh: S.Number,
  provenanceRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  scenarioRefs: S.Array(S.String),
  settlementMutationAllowed: S.Boolean,
  settlementRefs: S.Array(S.String),
  simulatorName: S.String,
  simulatorVersionRef: S.String,
  sourceRefs: S.Array(S.String),
  supportedMarket: S.Boolean,
  updatedAtDisplay: S.String,
}) {}

export class MargotExportPacketUnsafe extends S.TaggedErrorClass<MargotExportPacketUnsafe>()(
  'MargotExportPacketUnsafe',
  {
    reason: S.String,
  },
) {}

export const MARGOT_EXPORT_READ_ONLY_AUTHORITY: MargotExportAuthority = {
  authorityBoundary: 'read_only_simulator_packet',
  noAcceptedWorkMutation: true,
  noFinancialAdvice: true,
  noGridParticipation: true,
  noLiveWalletSpend: true,
  noMarketDataMutation: true,
  noPublicClaimUpgrade: true,
  noSettlementMutation: true,
}

const marketLabelByMarket: Record<MargotExportMarket, string> = {
  ercot: 'ERCOT',
  nyiso: 'NYISO',
  unsupported: 'Unsupported market',
}

const dispatchPolicyLabelByPolicy:
  Record<MargotExportDispatchPolicy, string> = {
    binary_on_off: 'Binary on/off',
    intelligent_miner: 'Intelligent miner',
    linear_gradient: 'Linear gradient',
    operator_defined: 'Operator defined',
    threshold: 'Threshold',
  }

const claimStateLabelByState: Record<MargotExportClaimState, string> = {
  measured: 'Measured',
  modeled: 'Modeled',
  settled: 'Settled',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeMargotRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mac[_-]?address|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(hardware|key|wallet)|provider[_-]?(grant|payload|secret|telemetry|token)|raw[_-]?(customer|energy|export|host|invoice|market|meter|payment|payload|payout|power|prompt|provider|runner|run[_-]?log|state|target|telemetry|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token|trading[_-]?(account|order)|wallet[._-](key|material|mnemonic|payment|preimage|secret|seed))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_outcome\.private|caveat\.private|data_rights\.private|diligence\.private|export\.private|provenance\.private|scenario\.private|settlement\.private|source\.private)/i
const teamUnsafeRefPattern =
  /(data_rights\.private|export\.private|provenance\.private|source\.private)/i

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
      unsafeMargotRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new MargotExportPacketUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, trading, raw export, raw market, raw telemetry, private repo, secret, or raw timestamp material.`,
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
    throw new MargotExportPacketUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const assertRecordSafe = (record: MargotExportPacketRecord): void => {
  assertSafeRefs('Margot export identity refs', [
    record.id,
    record.exportRef,
    record.simulatorVersionRef,
  ])
  assertSafeRefs(
    'Margot export accepted outcome assumption refs',
    record.acceptedOutcomeAssumptionRefs,
  )
  assertSafeRefs('Margot export caveat refs', record.caveatRefs)
  assertSafeRefs('Margot export data rights refs', record.dataRightsRefs)
  assertSafeRefs('Margot export diligence refs', record.nextDiligenceRefs)
  assertSafeRefs('Margot export provenance refs', record.provenanceRefs)
  assertSafeRefs('Margot export scenario refs', record.scenarioRefs)
  assertSafeRefs('Margot export settlement refs', record.settlementRefs)
  assertSafeRefs('Margot export source refs', record.sourceRefs)

  Object.entries({
    acceptedWorkCentsPerMwh: record.acceptedWorkCentsPerMwh,
    curtailmentValueCentsPerMwh: record.curtailmentValueCentsPerMwh,
    gpuRentalFloorCentsPerMwh: record.gpuRentalFloorCentsPerMwh,
    gridServiceCentsPerMwh: record.gridServiceCentsPerMwh,
    miningFloorCentsPerMwh: record.miningFloorCentsPerMwh,
    powerCostCentsPerMwh: record.powerCostCentsPerMwh,
  }).forEach(([label, value]) => assertNonNegativeInteger(label, value))

  if (
    record.authority.noAcceptedWorkMutation !== true ||
    record.authority.noFinancialAdvice !== true ||
    record.authority.noGridParticipation !== true ||
    record.authority.noLiveWalletSpend !== true ||
    record.authority.noMarketDataMutation !== true ||
    record.authority.noPublicClaimUpgrade !== true ||
    record.authority.noSettlementMutation !== true
  ) {
    throw new MargotExportPacketUnsafe({
      reason:
        'Margot export packets must remain read-only and cannot mutate accepted work, provide financial advice, join grid programs, spend wallets, mutate market data, upgrade public claims, or mutate settlement.',
    })
  }

  if (!hasRefs(record.caveatRefs)) {
    throw new MargotExportPacketUnsafe({
      reason: 'Margot export packets require caveat refs.',
    })
  }

  if (!hasRefs(record.provenanceRefs) || !hasRefs(record.sourceRefs)) {
    throw new MargotExportPacketUnsafe({
      reason: 'Margot export packets require provenance and source refs.',
    })
  }

  if (!hasRefs(record.nextDiligenceRefs)) {
    throw new MargotExportPacketUnsafe({
      reason: 'Margot export packets require next diligence refs.',
    })
  }

  if (
    record.market === 'unsupported' &&
    !record.caveatRefs.some(ref => ref.includes('unsupported_market'))
  ) {
    throw new MargotExportPacketUnsafe({
      reason:
        'Unsupported market packets require an unsupported-market caveat ref.',
    })
  }

  if (
    record.acceptedWorkCentsPerMwh > 0 &&
    !hasRefs(record.acceptedOutcomeAssumptionRefs)
  ) {
    throw new MargotExportPacketUnsafe({
      reason:
        'Accepted-work assumptions require accepted outcome assumption refs.',
    })
  }

  if (record.claimState === 'settled' && !hasRefs(record.settlementRefs)) {
    throw new MargotExportPacketUnsafe({
      reason:
        'Settled Margot export packets require settlement receipt refs.',
    })
  }
}

const projectionText = (projection: MargotExportPacketProjection): string =>
  [
    projection.id,
    projection.exportRef,
    projection.simulatorVersionRef,
    ...projection.acceptedOutcomeAssumptionRefs,
    ...projection.caveatRefs,
    ...projection.dataRightsRefs,
    ...projection.nextDiligenceRefs,
    ...projection.provenanceRefs,
    ...projection.scenarioRefs,
    ...projection.settlementRefs,
    ...projection.sourceRefs,
  ].join(' ')

export const margotExportProjectionHasPrivateMaterial = (
  projection: MargotExportPacketProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeMargotRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const projectMargotExportPacket = (
  record: MargotExportPacketRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): MargotExportPacketProjection => {
  assertRecordSafe(record)

  const projection: MargotExportPacketProjection = {
    acceptedOutcomeAssumptionRefs: safeRefsForAudience(
      'Margot accepted outcome assumption refs',
      record.acceptedOutcomeAssumptionRefs,
      audience,
    ),
    acceptedWorkCentsPerMwh: record.acceptedWorkCentsPerMwh,
    acceptedWorkLaneClaimAllowed:
      record.acceptedWorkCentsPerMwh > 0 &&
      record.claimState !== 'modeled' &&
      hasRefs(record.acceptedOutcomeAssumptionRefs),
    acceptedWorkMutationAllowed: false,
    audience,
    authority: record.authority,
    caveatRefs: safeRefsForAudience(
      'Margot caveat refs',
      record.caveatRefs,
      audience,
    ),
    claimState: record.claimState,
    claimStateLabel: claimStateLabelByState[record.claimState],
    curtailmentValueCentsPerMwh: record.curtailmentValueCentsPerMwh,
    dataRightsRefs: safeRefsForAudience(
      'Margot data rights refs',
      record.dataRightsRefs,
      audience,
    ),
    dispatchPolicy: record.dispatchPolicy,
    dispatchPolicyLabel: dispatchPolicyLabelByPolicy[record.dispatchPolicy],
    exportRef: record.exportRef,
    financialAdviceAllowed: false,
    generatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.generatedAtIso,
      nowIso,
    ),
    gpuRentalFloorCentsPerMwh: record.gpuRentalFloorCentsPerMwh,
    gridParticipationAllowed: false,
    gridServiceCentsPerMwh: record.gridServiceCentsPerMwh,
    id: record.id,
    liveWalletSpendAllowed: false,
    market: record.market,
    marketDataMutationAllowed: false,
    marketLabel: marketLabelByMarket[record.market],
    miningFloorCentsPerMwh: record.miningFloorCentsPerMwh,
    nextDiligenceRefs: safeRefsForAudience(
      'Margot diligence refs',
      record.nextDiligenceRefs,
      audience,
    ),
    powerCostCentsPerMwh: record.powerCostCentsPerMwh,
    provenanceRefs: safeRefsForAudience(
      'Margot provenance refs',
      record.provenanceRefs,
      audience,
    ),
    publicClaimUpgradeAllowed: false,
    scenarioRefs: safeRefsForAudience(
      'Margot scenario refs',
      record.scenarioRefs,
      audience,
    ),
    settlementMutationAllowed: false,
    settlementRefs: safeRefsForAudience(
      'Margot settlement refs',
      record.settlementRefs,
      audience,
    ),
    simulatorName: record.simulatorName,
    simulatorVersionRef: record.simulatorVersionRef,
    sourceRefs: safeRefsForAudience(
      'Margot source refs',
      record.sourceRefs,
      audience,
    ),
    supportedMarket: record.market === 'ercot' || record.market === 'nyiso',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
  }

  if (margotExportProjectionHasPrivateMaterial(projection)) {
    throw new MargotExportPacketUnsafe({
      reason:
        'Margot export projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const exampleMargotExportPacket = (): MargotExportPacketRecord => ({
  acceptedOutcomeAssumptionRefs: [
    'accepted_outcome_assumption.public.agentic_work_v1',
  ],
  acceptedWorkCentsPerMwh: 42000,
  authority: MARGOT_EXPORT_READ_ONLY_AUTHORITY,
  caveatRefs: ['caveat.public.tdp_not_facility_power'],
  claimState: 'modeled',
  curtailmentValueCentsPerMwh: 8000,
  dataRightsRefs: ['data_rights.public.operator_review_only'],
  dispatchPolicy: 'threshold',
  exportRef: 'export.margot.public_demo_1',
  generatedAtIso: '2026-06-06T23:40:00.000Z',
  gpuRentalFloorCentsPerMwh: 27000,
  gridServiceCentsPerMwh: 9000,
  id: 'margot_export.public_demo_1',
  market: 'ercot',
  miningFloorCentsPerMwh: 18000,
  nextDiligenceRefs: ['diligence.public.facility_power_metering'],
  powerCostCentsPerMwh: 4500,
  provenanceRefs: ['provenance.public.oa_aibtc_model'],
  scenarioRefs: ['scenario.public.shc_demo'],
  settlementRefs: [],
  simulatorName: 'oa_aibtc_model',
  simulatorVersionRef: 'simulator_version.public.oa_aibtc_model.v1',
  sourceRefs: ['source.public.margot_synthesis'],
  updatedAtIso: '2026-06-06T23:45:00.000Z',
})
