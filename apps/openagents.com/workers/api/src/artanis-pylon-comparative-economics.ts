import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { OmniProjectionAudience } from './omni-data-classification'

export const ArtanisPylonEconomicsClaimState = S.Literals([
  'blocked',
  'measured',
  'modeled',
  'payable',
  'settled',
  'stale',
  'unsupported',
])
export type ArtanisPylonEconomicsClaimState =
  typeof ArtanisPylonEconomicsClaimState.Type

export const ArtanisPylonEconomicsMarket = S.Literals([
  'ercot',
  'nyiso',
  'unsupported',
])
export type ArtanisPylonEconomicsMarket =
  typeof ArtanisPylonEconomicsMarket.Type

export const ArtanisPylonEconomicsDenominatorKind = S.Literals([
  'chip_tdp',
  'measured_ipmi',
  'measured_pdu',
  'metered_facility',
  'node_system_power',
  'pue_adjusted',
])
export type ArtanisPylonEconomicsDenominatorKind =
  typeof ArtanisPylonEconomicsDenominatorKind.Type

export const ArtanisPylonEconomicsTokenUnitAuditState = S.Literals([
  'failed',
  'pending',
  'verified',
])
export type ArtanisPylonEconomicsTokenUnitAuditState =
  typeof ArtanisPylonEconomicsTokenUnitAuditState.Type

export const ArtanisPylonEconomicsAuthorityBoundary = S.Literals([
  'read_only_comparative_economics_packet',
])
export type ArtanisPylonEconomicsAuthorityBoundary =
  typeof ArtanisPylonEconomicsAuthorityBoundary.Type

export class ArtanisPylonEconomicsAuthority extends S.Class<ArtanisPylonEconomicsAuthority>(
  'ArtanisPylonEconomicsAuthority',
)({
  authorityBoundary: ArtanisPylonEconomicsAuthorityBoundary,
  noAcceptedWorkMutation: S.Boolean,
  noBuyerChargeMutation: S.Boolean,
  noFinancialAdvice: S.Boolean,
  noGridDispatch: S.Boolean,
  noLiveWalletSpend: S.Boolean,
  noMarketDataMutation: S.Boolean,
  noProviderSettlementMutation: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSettlementMutation: S.Boolean,
}) {}

export class ArtanisPylonMargotProvenanceRecord extends S.Class<ArtanisPylonMargotProvenanceRecord>(
  'ArtanisPylonMargotProvenanceRecord',
)({
  caveatRefs: S.Array(S.String),
  commitRef: S.String,
  dataTimestampRefs: S.Array(S.String),
  exportedAtIso: S.String,
  normalizedExportRefs: S.Array(S.String),
  repoRef: S.String,
  sourceUrlRefs: S.Array(S.String),
}) {}

export class ArtanisPylonGpuRentalEvidenceRecord extends S.Class<ArtanisPylonGpuRentalEvidenceRecord>(
  'ArtanisPylonGpuRentalEvidenceRecord',
)({
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  derivedDollarsPerMwh: S.Number,
  dollarsPerGpuHour: S.Number,
  evidenceRef: S.String,
  gpuModel: S.String,
  listingSampleSize: S.Number,
  sampleTimestampIso: S.String,
  sourceRefs: S.Array(S.String),
  tdpSourceRef: S.String,
  tdpWatts: S.Number,
}) {}

export class ArtanisPylonTokenEconomicsEvidenceRecord extends S.Class<ArtanisPylonTokenEconomicsEvidenceRecord>(
  'ArtanisPylonTokenEconomicsEvidenceRecord',
)({
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  completionUsdPerMtok: S.Number,
  completionUsdPerToken: S.Number,
  derivedDollarsPerMwh: S.Number,
  displayPriceUnit: S.String,
  energyPerTokenJoules: S.Number,
  evidenceRef: S.String,
  gpuModel: S.String,
  isStable: S.Boolean,
  mlEnergyRunRef: S.String,
  mlEnergyTask: S.String,
  openRouterModelId: S.String,
  outputThroughputTokensPerSecond: S.Number,
  priceTimestampIso: S.String,
  rawPriceUnit: S.String,
  sourceRefs: S.Array(S.String),
  systemPowerWatts: S.Number,
  unitAuditState: ArtanisPylonEconomicsTokenUnitAuditState,
}) {}

export class ArtanisPylonThroughputCalculatorEvidenceRecord extends S.Class<ArtanisPylonThroughputCalculatorEvidenceRecord>(
  'ArtanisPylonThroughputCalculatorEvidenceRecord',
)({
  calculatorRef: S.String,
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  contextWindowRef: S.String,
  evidenceRef: S.String,
  hardwareRef: S.String,
  modelRef: S.String,
  modeledAtIso: S.String,
  quantizationRef: S.String,
  queryParamRefs: S.Array(S.String),
  urlRef: S.String,
}) {}

export class ArtanisPylonCapacityEvidenceRecord extends S.Class<ArtanisPylonCapacityEvidenceRecord>(
  'ArtanisPylonCapacityEvidenceRecord',
)({
  availabilityWindowRef: S.String,
  caveatRefs: S.Array(S.String),
  chipTdpWatts: S.Number,
  claimState: ArtanisPylonEconomicsClaimState,
  cohortRef: S.String,
  coolingPueAssumption: S.Number,
  costTermRef: S.String,
  denominatorKind: ArtanisPylonEconomicsDenominatorKind,
  effectiveWattsPerGpu: S.Number,
  evidenceRef: S.String,
  gpuCount: S.Number,
  gpuModel: S.String,
  idleDarkCapacityRefs: S.Array(S.String),
  interconnectRef: S.String,
  measuredIpmiAvailable: S.Boolean,
  measuredPduAvailable: S.Boolean,
  meteredFacilityAvailable: S.Boolean,
  nodePowerAdjustedDollarsPerMwh: S.Number,
  nodeRef: S.String,
  resourceMode: S.String,
  runtimeFrameworkRef: S.String,
  sourceRefs: S.Array(S.String),
  systemPowerWatts: S.Number,
  vramGb: S.Number,
}) {}

export class ArtanisPylonPowerMarketWindowRecord extends S.Class<ArtanisPylonPowerMarketWindowRecord>(
  'ArtanisPylonPowerMarketWindowRecord',
)({
  averageLmpDollarsPerMwh: S.Number,
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  evidenceRef: S.String,
  market: ArtanisPylonEconomicsMarket,
  missingDataFlags: S.Array(S.String),
  refreshedAtIso: S.String,
  sourceRefs: S.Array(S.String),
  windowRef: S.String,
  zoneOrSettlementPoint: S.String,
}) {}

export class ArtanisPylonMiningCounterfactualRecord extends S.Class<ArtanisPylonMiningCounterfactualRecord>(
  'ArtanisPylonMiningCounterfactualRecord',
)({
  asicModel: S.String,
  capacityMw: S.Number,
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  curtailmentPolicyRef: S.String,
  efficiencyJoulesPerTh: S.Number,
  evidenceRef: S.String,
  firmwareAssumptionRef: S.String,
  miningMarginDollarsPerMwh: S.Number,
  miningRevenueDollarsPerMwh: S.Number,
  opsAssumptionRef: S.String,
  poolAssumptionRef: S.String,
  sourceRefs: S.Array(S.String),
}) {}

export class ArtanisPylonAcceptedOutcomeEvidenceRecord extends S.Class<ArtanisPylonAcceptedOutcomeEvidenceRecord>(
  'ArtanisPylonAcceptedOutcomeEvidenceRecord',
)({
  acceptedOutcomeValueDollarsPerMwh: S.Number,
  acceptanceRef: S.String,
  artifactRefs: S.Array(S.String),
  assignmentRef: S.String,
  buyerRevenueCents: S.Number,
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  closeoutRef: S.String,
  economicsRefs: S.Array(S.String),
  evidenceRef: S.String,
  gradingRef: S.String,
  grossProfitCents: S.Number,
  providerPayableCents: S.Number,
  providerSettledCents: S.Number,
  rejectionRefs: S.Array(S.String),
  retryRefs: S.Array(S.String),
  runRef: S.String,
  settlementRefs: S.Array(S.String),
}) {}

export class ArtanisPylonComparativeEconomicsPacketRecord extends S.Class<ArtanisPylonComparativeEconomicsPacketRecord>(
  'ArtanisPylonComparativeEconomicsPacketRecord',
)({
  acceptedOutcome: ArtanisPylonAcceptedOutcomeEvidenceRecord,
  agentRef: S.String,
  authority: ArtanisPylonEconomicsAuthority,
  caveatRefs: S.Array(S.String),
  gpuRental: ArtanisPylonGpuRentalEvidenceRecord,
  miningCounterfactual: ArtanisPylonMiningCounterfactualRecord,
  packetRef: S.String,
  powerMarket: ArtanisPylonPowerMarketWindowRecord,
  privateEvidenceRefs: S.Array(S.String),
  provenance: ArtanisPylonMargotProvenanceRecord,
  pylonCapacity: ArtanisPylonCapacityEvidenceRecord,
  sourceRefs: S.Array(S.String),
  throughputCalculator: ArtanisPylonThroughputCalculatorEvidenceRecord,
  tokenEconomics: ArtanisPylonTokenEconomicsEvidenceRecord,
  updatedAtIso: S.String,
  windowRef: S.String,
}) {}

export class ArtanisPylonComparativeEconomicsValueRow extends S.Class<ArtanisPylonComparativeEconomicsValueRow>(
  'ArtanisPylonComparativeEconomicsValueRow',
)({
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  claimState: ArtanisPylonEconomicsClaimState,
  claimStateLabel: S.String,
  denominatorKind: S.NullOr(ArtanisPylonEconomicsDenominatorKind),
  dollarsPerMwh: S.NullOr(S.Number),
  evidenceRefs: S.Array(S.String),
  label: S.String,
  valueRef: S.String,
}) {}

export class ArtanisPylonComparativeEconomicsProjection extends S.Class<ArtanisPylonComparativeEconomicsProjection>(
  'ArtanisPylonComparativeEconomicsProjection',
)({
  acceptedOutcomeValueDollarsPerMwh: S.Number,
  acceptedWorkMutationAllowed: S.Boolean,
  agentRef: S.String,
  audience: OmniProjectionAudience,
  authority: ArtanisPylonEconomicsAuthority,
  buyerChargeMutationAllowed: S.Boolean,
  caveatRefs: S.Array(S.String),
  claimStateRefs: S.Array(S.String),
  denominatorRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  financialAdviceAllowed: S.Boolean,
  gpuRentalFloorDollarsPerMwh: S.Number,
  gridDispatchAllowed: S.Boolean,
  idleDarkCapacityRefs: S.Array(S.String),
  liveWalletSpendAllowed: S.Boolean,
  market: ArtanisPylonEconomicsMarket,
  marketDataMutationAllowed: S.Boolean,
  miningFloorDollarsPerMwh: S.Number,
  nodePowerAdjustedFloorDollarsPerMwh: S.NullOr(S.Number),
  packetRef: S.String,
  powerCostDollarsPerMwh: S.Number,
  privateEvidenceRefs: S.Array(S.String),
  providerSettlementMutationAllowed: S.Boolean,
  publicBlockerRefs: S.Array(S.String),
  publicClaimUpgradeAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  sourceRefs: S.Array(S.String),
  sourceUrlRefs: S.Array(S.String),
  tokenInferenceFloorDollarsPerMwh: S.NullOr(S.Number),
  tokenInferencePublicBlocked: S.Boolean,
  updatedAtDisplay: S.String,
  valueRows: S.Array(ArtanisPylonComparativeEconomicsValueRow),
  windowRef: S.String,
}) {}

export class ArtanisPylonComparativeEconomicsUnsafe extends S.TaggedErrorClass<ArtanisPylonComparativeEconomicsUnsafe>()(
  'ArtanisPylonComparativeEconomicsUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_PYLON_COMPARATIVE_ECONOMICS_READ_ONLY_AUTHORITY:
  ArtanisPylonEconomicsAuthority = {
    authorityBoundary: 'read_only_comparative_economics_packet',
    noAcceptedWorkMutation: true,
    noBuyerChargeMutation: true,
    noFinancialAdvice: true,
    noGridDispatch: true,
    noLiveWalletSpend: true,
    noMarketDataMutation: true,
    noProviderSettlementMutation: true,
    noPublicClaimUpgrade: true,
    noSettlementMutation: true,
  }

const claimStateLabelByState:
  Readonly<Record<ArtanisPylonEconomicsClaimState, string>> = {
    blocked: 'Blocked',
    measured: 'Measured',
    modeled: 'Modeled',
    payable: 'Payable',
    settled: 'Settled',
    stale: 'Stale',
    unsupported: 'Unsupported',
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/?#=&-]{0,340}$/
const unsafeEconomicsRefPattern =
  /(@|\/Users\/|\/home\/|127\.0\.0\.1|192\.168\.|access[_-]?token|api[_-]?key|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|hardware[_-]?telemetry|hostname|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|localhost|mac[_-]?address|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|hardware|key|repo|source|trace|url|wallet)|provider[_-]?(account|credential|grant|payload|secret|telemetry|token)|raw[_-]?(artifact|auth|command|customer|energy|export|host|invoice|log|market|meter|payment|payload|payout|power|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|trading[_-]?(account|order)|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(accepted_outcome\.private|artifact\.private|assignment\.private|caveat\.private|closeout\.private|cohort\.private|dispatch\.private|economics\.private|evidence\.private|export\.private|grading\.private|meter\.private|node\.private|provider\.private|provenance\.private|run\.private|settlement\.private|source\.private|url\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(accepted_outcome\.private|artifact\.private|assignment\.private|closeout\.private|dispatch\.private|meter\.private|provider\.private|run\.private|settlement\.private|source\.private|url\.private|workroom\.private)/i

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
    unsafeEconomicsRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: `${label} contains private customer, provider, wallet, payment, raw command, raw telemetry, raw meter, private URL, private repo, secret, or raw timestamp material.`,
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
  const safeRefs = uniqueRefs(refs)

  if (pattern === null) {
    return safeRefs
  }

  return safeRefs.filter(ref => !pattern.test(ref))
}

const publicLikeAudience = (
  audience: typeof OmniProjectionAudience.Type,
): boolean => audience === 'public' || audience === 'agent' ||
  audience === 'customer'

const assertNonNegativeFinite = (label: string, value: number): void => {
  if (!Number.isFinite(value) || value < 0) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: `${label} must be a non-negative finite number.`,
    })
  }
}

const assertNonNegativeInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

const assertPositiveInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value <= 0) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: `${label} must be a positive integer.`,
    })
  }
}

const hasRefs = (refs: ReadonlyArray<string>): boolean => refs.length > 0

const allPacketRefs = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
): ReadonlyArray<string> => [
  record.packetRef,
  record.agentRef,
  record.windowRef,
  record.provenance.repoRef,
  record.provenance.commitRef,
  ...record.provenance.normalizedExportRefs,
  ...record.provenance.dataTimestampRefs,
  ...record.provenance.sourceUrlRefs,
  ...record.provenance.caveatRefs,
  record.gpuRental.evidenceRef,
  record.gpuRental.tdpSourceRef,
  ...record.gpuRental.sourceRefs,
  ...record.gpuRental.caveatRefs,
  record.tokenEconomics.evidenceRef,
  record.tokenEconomics.mlEnergyRunRef,
  ...record.tokenEconomics.sourceRefs,
  ...record.tokenEconomics.caveatRefs,
  record.throughputCalculator.evidenceRef,
  record.throughputCalculator.calculatorRef,
  record.throughputCalculator.urlRef,
  record.throughputCalculator.modelRef,
  record.throughputCalculator.hardwareRef,
  record.throughputCalculator.contextWindowRef,
  record.throughputCalculator.quantizationRef,
  ...record.throughputCalculator.queryParamRefs,
  ...record.throughputCalculator.caveatRefs,
  record.pylonCapacity.evidenceRef,
  record.pylonCapacity.nodeRef,
  record.pylonCapacity.cohortRef,
  record.pylonCapacity.interconnectRef,
  record.pylonCapacity.runtimeFrameworkRef,
  record.pylonCapacity.availabilityWindowRef,
  record.pylonCapacity.costTermRef,
  ...record.pylonCapacity.idleDarkCapacityRefs,
  ...record.pylonCapacity.sourceRefs,
  ...record.pylonCapacity.caveatRefs,
  record.powerMarket.evidenceRef,
  record.powerMarket.windowRef,
  ...record.powerMarket.missingDataFlags,
  ...record.powerMarket.sourceRefs,
  ...record.powerMarket.caveatRefs,
  record.miningCounterfactual.evidenceRef,
  record.miningCounterfactual.poolAssumptionRef,
  record.miningCounterfactual.firmwareAssumptionRef,
  record.miningCounterfactual.opsAssumptionRef,
  record.miningCounterfactual.curtailmentPolicyRef,
  ...record.miningCounterfactual.sourceRefs,
  ...record.miningCounterfactual.caveatRefs,
  record.acceptedOutcome.evidenceRef,
  record.acceptedOutcome.assignmentRef,
  record.acceptedOutcome.runRef,
  record.acceptedOutcome.gradingRef,
  record.acceptedOutcome.acceptanceRef,
  record.acceptedOutcome.closeoutRef,
  ...record.acceptedOutcome.artifactRefs,
  ...record.acceptedOutcome.rejectionRefs,
  ...record.acceptedOutcome.retryRefs,
  ...record.acceptedOutcome.economicsRefs,
  ...record.acceptedOutcome.settlementRefs,
  ...record.acceptedOutcome.caveatRefs,
  ...record.sourceRefs,
  ...record.caveatRefs,
  ...record.privateEvidenceRefs,
]

const assertReadOnlyAuthority = (
  authority: ArtanisPylonEconomicsAuthority,
): void => {
  if (
    authority.noAcceptedWorkMutation !== true ||
    authority.noBuyerChargeMutation !== true ||
    authority.noFinancialAdvice !== true ||
    authority.noGridDispatch !== true ||
    authority.noLiveWalletSpend !== true ||
    authority.noMarketDataMutation !== true ||
    authority.noProviderSettlementMutation !== true ||
    authority.noPublicClaimUpgrade !== true ||
    authority.noSettlementMutation !== true
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Artanis/Pylon comparative economics packets must be read-only and cannot mutate accepted work, buyers, grid dispatch, market data, wallets, provider settlement, public claims, or settlement.',
    })
  }
}

const assertRecordSafe = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
): void => {
  assertReadOnlyAuthority(record.authority)
  assertSafeRefs('Artanis/Pylon comparative economics refs', allPacketRefs(record))

  Object.entries({
    acceptedOutcomeValueDollarsPerMwh:
      record.acceptedOutcome.acceptedOutcomeValueDollarsPerMwh,
    averageLmpDollarsPerMwh: record.powerMarket.averageLmpDollarsPerMwh,
    capacityMw: record.miningCounterfactual.capacityMw,
    chipTdpWatts: record.pylonCapacity.chipTdpWatts,
    completionUsdPerMtok: record.tokenEconomics.completionUsdPerMtok,
    completionUsdPerToken: record.tokenEconomics.completionUsdPerToken,
    derivedGpuRentalDollarsPerMwh: record.gpuRental.derivedDollarsPerMwh,
    derivedTokenDollarsPerMwh: record.tokenEconomics.derivedDollarsPerMwh,
    dollarsPerGpuHour: record.gpuRental.dollarsPerGpuHour,
    effectiveWattsPerGpu: record.pylonCapacity.effectiveWattsPerGpu,
    efficiencyJoulesPerTh: record.miningCounterfactual.efficiencyJoulesPerTh,
    energyPerTokenJoules: record.tokenEconomics.energyPerTokenJoules,
    miningMarginDollarsPerMwh:
      record.miningCounterfactual.miningMarginDollarsPerMwh,
    miningRevenueDollarsPerMwh:
      record.miningCounterfactual.miningRevenueDollarsPerMwh,
    nodePowerAdjustedDollarsPerMwh:
      record.pylonCapacity.nodePowerAdjustedDollarsPerMwh,
    outputThroughputTokensPerSecond:
      record.tokenEconomics.outputThroughputTokensPerSecond,
    systemPowerWatts: record.pylonCapacity.systemPowerWatts,
    tokenSystemPowerWatts: record.tokenEconomics.systemPowerWatts,
    tdpWatts: record.gpuRental.tdpWatts,
    vramGb: record.pylonCapacity.vramGb,
  }).forEach(([label, value]) => assertNonNegativeFinite(label, value))

  Object.entries({
    buyerRevenueCents: record.acceptedOutcome.buyerRevenueCents,
    gpuCount: record.pylonCapacity.gpuCount,
    grossProfitCents: record.acceptedOutcome.grossProfitCents,
    listingSampleSize: record.gpuRental.listingSampleSize,
    providerPayableCents: record.acceptedOutcome.providerPayableCents,
    providerSettledCents: record.acceptedOutcome.providerSettledCents,
  }).forEach(([label, value]) => assertNonNegativeInteger(label, value))

  assertPositiveInteger('GPU rental listing sample size', record.gpuRental.listingSampleSize)
  assertPositiveInteger('Pylon capacity GPU count', record.pylonCapacity.gpuCount)

  if (!hasRefs(record.caveatRefs) || !hasRefs(record.sourceRefs)) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Artanis/Pylon comparative economics packets require packet caveat and source refs.',
    })
  }

  if (
    !hasRefs(record.provenance.normalizedExportRefs) ||
    !hasRefs(record.provenance.dataTimestampRefs) ||
    !hasRefs(record.provenance.sourceUrlRefs) ||
    !hasRefs(record.provenance.caveatRefs)
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Margot provenance requires normalized export refs, data timestamp refs, source URL refs, and caveats.',
    })
  }

  if (
    record.powerMarket.market === 'unsupported' &&
    ![
      ...record.powerMarket.caveatRefs,
      ...record.caveatRefs,
    ].some(ref => ref.includes('unsupported_market'))
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Unsupported power markets require an explicit unsupported-market caveat ref.',
    })
  }

  if (
    record.tokenEconomics.unitAuditState === 'verified' &&
    record.tokenEconomics.caveatRefs.length === 0
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: 'Verified token unit audits still require caveat refs.',
    })
  }

  if (
    record.acceptedOutcome.claimState === 'payable' &&
    record.acceptedOutcome.providerPayableCents <= 0
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: 'Payable accepted-outcome evidence requires provider payable value.',
    })
  }

  if (
    record.acceptedOutcome.claimState === 'payable' &&
    record.acceptedOutcome.providerSettledCents > 0
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Payable accepted-outcome evidence cannot already carry settled provider value.',
    })
  }

  if (
    record.acceptedOutcome.claimState === 'settled' &&
    (record.acceptedOutcome.providerSettledCents <= 0 ||
      !hasRefs(record.acceptedOutcome.settlementRefs))
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Settled accepted-outcome evidence requires provider settled value and settlement refs.',
    })
  }

  if (
    record.acceptedOutcome.providerSettledCents >
      record.acceptedOutcome.providerPayableCents
  ) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason: 'Provider settled cents cannot exceed provider payable cents.',
    })
  }
}

const stateRefs = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
): ReadonlyArray<string> =>
  uniqueRefs([
    `claim_state.${record.gpuRental.claimState}.gpu_rental`,
    `claim_state.${record.tokenEconomics.claimState}.token_inference`,
    `claim_state.${record.throughputCalculator.claimState}.throughput_calculator`,
    `claim_state.${record.pylonCapacity.claimState}.pylon_capacity`,
    `claim_state.${record.powerMarket.claimState}.power_market`,
    `claim_state.${record.miningCounterfactual.claimState}.mining_counterfactual`,
    `claim_state.${record.acceptedOutcome.claimState}.accepted_outcome`,
  ])

const staleOrBlockedRefs = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
): ReadonlyArray<string> => {
  const states = [
    record.gpuRental.claimState,
    record.tokenEconomics.claimState,
    record.throughputCalculator.claimState,
    record.pylonCapacity.claimState,
    record.powerMarket.claimState,
    record.miningCounterfactual.claimState,
    record.acceptedOutcome.claimState,
  ]
  const stale = states.includes('stale')
  const blocked = states.includes('blocked')
  const unsupported = states.includes('unsupported')

  return uniqueRefs([
    ...(stale
      ? ['blocker.public.artanis_pylon_economics.stale_source']
      : []),
    ...(blocked
      ? ['blocker.public.artanis_pylon_economics.blocked_evidence']
      : []),
    ...(unsupported
      ? ['blocker.public.artanis_pylon_economics.unsupported_market']
      : []),
  ])
}

const tokenPublicBlocked = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
  audience: typeof OmniProjectionAudience.Type,
): boolean =>
  publicLikeAudience(audience) &&
  record.tokenEconomics.unitAuditState !== 'verified'

const nodePowerValue = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
): number | null =>
  record.pylonCapacity.denominatorKind === 'chip_tdp'
    ? null
    : record.pylonCapacity.nodePowerAdjustedDollarsPerMwh

const projectionText = (
  projection: ArtanisPylonComparativeEconomicsProjection,
): string =>
  [
    projection.packetRef,
    projection.agentRef,
    projection.windowRef,
    ...projection.caveatRefs,
    ...projection.claimStateRefs,
    ...projection.denominatorRefs,
    ...projection.evidenceRefs,
    ...projection.idleDarkCapacityRefs,
    ...projection.privateEvidenceRefs,
    ...projection.publicBlockerRefs,
    ...projection.sourceRefs,
    ...projection.sourceUrlRefs,
    ...projection.valueRows.flatMap(row => [
      row.valueRef,
      ...row.blockerRefs,
      ...row.caveatRefs,
      ...row.evidenceRefs,
    ]),
  ].join(' ')

export const artanisPylonComparativeEconomicsProjectionHasPrivateMaterial = (
  projection: ArtanisPylonComparativeEconomicsProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return containsProviderSecretMaterial(text) ||
    unsafeEconomicsRefPattern.test(text) ||
    rawTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

const valueRow = (input: {
  blockerRefs?: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  claimState: ArtanisPylonEconomicsClaimState
  denominatorKind?: ArtanisPylonEconomicsDenominatorKind | null
  dollarsPerMwh: number | null
  evidenceRefs: ReadonlyArray<string>
  label: string
  valueRef: string
}): ArtanisPylonComparativeEconomicsValueRow =>
  new ArtanisPylonComparativeEconomicsValueRow({
    blockerRefs: uniqueRefs(input.blockerRefs ?? []),
    caveatRefs: uniqueRefs(input.caveatRefs),
    claimState: input.claimState,
    claimStateLabel: claimStateLabelByState[input.claimState],
    denominatorKind: input.denominatorKind ?? null,
    dollarsPerMwh: input.dollarsPerMwh,
    evidenceRefs: uniqueRefs(input.evidenceRefs),
    label: input.label,
    valueRef: input.valueRef,
  })

export const projectArtanisPylonComparativeEconomicsPacket = (
  record: ArtanisPylonComparativeEconomicsPacketRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): ArtanisPylonComparativeEconomicsProjection => {
  assertRecordSafe(record)

  const tokenBlocked = tokenPublicBlocked(record, audience)
  const tokenValue = tokenBlocked
    ? null
    : record.tokenEconomics.derivedDollarsPerMwh
  const nodePowerAdjusted = nodePowerValue(record)
  const denominatorBlockers = nodePowerAdjusted === null
    ? ['blocker.public.artanis_pylon_economics.chip_tdp_not_node_power']
    : []
  const publicBlockerRefs = uniqueRefs([
    ...staleOrBlockedRefs(record),
    ...(tokenBlocked
      ? ['blocker.public.artanis_pylon_economics.token_unit_audit_required']
      : []),
    ...denominatorBlockers,
  ])
  const evidenceRefs = safeRefsForAudience(
    'Artanis/Pylon comparative economics evidence refs',
    [
      record.gpuRental.evidenceRef,
      record.tokenEconomics.evidenceRef,
      record.throughputCalculator.evidenceRef,
      record.pylonCapacity.evidenceRef,
      record.powerMarket.evidenceRef,
      record.miningCounterfactual.evidenceRef,
      record.acceptedOutcome.evidenceRef,
      ...record.acceptedOutcome.artifactRefs,
      ...record.acceptedOutcome.economicsRefs,
      ...record.acceptedOutcome.settlementRefs,
      ...record.privateEvidenceRefs,
    ],
    audience,
  )
  const sourceRefs = safeRefsForAudience(
    'Artanis/Pylon comparative economics source refs',
    [
      ...record.sourceRefs,
      ...record.gpuRental.sourceRefs,
      ...record.tokenEconomics.sourceRefs,
      ...record.pylonCapacity.sourceRefs,
      ...record.powerMarket.sourceRefs,
      ...record.miningCounterfactual.sourceRefs,
    ],
    audience,
  )
  const caveatRefs = safeRefsForAudience(
    'Artanis/Pylon comparative economics caveat refs',
    [
      ...record.caveatRefs,
      ...record.provenance.caveatRefs,
      ...record.gpuRental.caveatRefs,
      ...record.tokenEconomics.caveatRefs,
      ...record.throughputCalculator.caveatRefs,
      ...record.pylonCapacity.caveatRefs,
      ...record.powerMarket.caveatRefs,
      ...record.miningCounterfactual.caveatRefs,
      ...record.acceptedOutcome.caveatRefs,
    ],
    audience,
  )
  const privateEvidenceRefs =
    audience === 'operator' || audience === 'private'
      ? safeRefsForAudience(
        'Artanis/Pylon comparative economics private evidence refs',
        record.privateEvidenceRefs,
        audience,
      )
      : []

  const projection: ArtanisPylonComparativeEconomicsProjection = {
    acceptedOutcomeValueDollarsPerMwh:
      record.acceptedOutcome.acceptedOutcomeValueDollarsPerMwh,
    acceptedWorkMutationAllowed: false,
    agentRef: record.agentRef,
    audience,
    authority: record.authority,
    buyerChargeMutationAllowed: false,
    caveatRefs,
    claimStateRefs: stateRefs(record),
    denominatorRefs: uniqueRefs([
      `denominator.${record.pylonCapacity.denominatorKind}`,
      ...(record.pylonCapacity.measuredPduAvailable
        ? ['denominator.measured_pdu.available']
        : []),
      ...(record.pylonCapacity.measuredIpmiAvailable
        ? ['denominator.measured_ipmi.available']
        : []),
      ...(record.pylonCapacity.meteredFacilityAvailable
        ? ['denominator.metered_facility.available']
        : []),
    ]),
    evidenceRefs,
    financialAdviceAllowed: false,
    gpuRentalFloorDollarsPerMwh: record.gpuRental.derivedDollarsPerMwh,
    gridDispatchAllowed: false,
    idleDarkCapacityRefs: safeRefsForAudience(
      'Artanis/Pylon comparative economics idle capacity refs',
      record.pylonCapacity.idleDarkCapacityRefs,
      audience,
    ),
    liveWalletSpendAllowed: false,
    market: record.powerMarket.market,
    marketDataMutationAllowed: false,
    miningFloorDollarsPerMwh:
      record.miningCounterfactual.miningRevenueDollarsPerMwh,
    nodePowerAdjustedFloorDollarsPerMwh: nodePowerAdjusted,
    packetRef: record.packetRef,
    powerCostDollarsPerMwh: record.powerMarket.averageLmpDollarsPerMwh,
    privateEvidenceRefs,
    providerSettlementMutationAllowed: false,
    publicBlockerRefs,
    publicClaimUpgradeAllowed: false,
    settlementMutationAllowed: false,
    sourceRefs,
    sourceUrlRefs: safeRefsForAudience(
      'Artanis/Pylon comparative economics source URL refs',
      record.provenance.sourceUrlRefs,
      audience,
    ),
    tokenInferenceFloorDollarsPerMwh: tokenValue,
    tokenInferencePublicBlocked: tokenBlocked,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    valueRows: [
      valueRow({
        caveatRefs: record.miningCounterfactual.caveatRefs,
        claimState: record.miningCounterfactual.claimState,
        dollarsPerMwh: record.miningCounterfactual.miningRevenueDollarsPerMwh,
        evidenceRefs: [record.miningCounterfactual.evidenceRef],
        label: 'Mining floor',
        valueRef: 'value.public.artanis_pylon_economics.mining_floor',
      }),
      valueRow({
        caveatRefs: record.gpuRental.caveatRefs,
        claimState: record.gpuRental.claimState,
        denominatorKind: 'chip_tdp',
        dollarsPerMwh: record.gpuRental.derivedDollarsPerMwh,
        evidenceRefs: [record.gpuRental.evidenceRef],
        label: 'GPU rental floor',
        valueRef: 'value.public.artanis_pylon_economics.gpu_rental_floor',
      }),
      valueRow({
        blockerRefs: tokenBlocked
          ? ['blocker.public.artanis_pylon_economics.token_unit_audit_required']
          : [],
        caveatRefs: record.tokenEconomics.caveatRefs,
        claimState: record.tokenEconomics.claimState,
        denominatorKind: 'node_system_power',
        dollarsPerMwh: tokenValue,
        evidenceRefs: [record.tokenEconomics.evidenceRef],
        label: 'Token inference floor',
        valueRef: 'value.public.artanis_pylon_economics.token_inference_floor',
      }),
      valueRow({
        blockerRefs: denominatorBlockers,
        caveatRefs: record.pylonCapacity.caveatRefs,
        claimState: record.pylonCapacity.claimState,
        denominatorKind: record.pylonCapacity.denominatorKind,
        dollarsPerMwh: nodePowerAdjusted,
        evidenceRefs: [record.pylonCapacity.evidenceRef],
        label: 'Node-power-adjusted floor',
        valueRef:
          'value.public.artanis_pylon_economics.node_power_adjusted_floor',
      }),
      valueRow({
        caveatRefs: record.acceptedOutcome.caveatRefs,
        claimState: record.acceptedOutcome.claimState,
        dollarsPerMwh: record.acceptedOutcome.acceptedOutcomeValueDollarsPerMwh,
        evidenceRefs: [
          record.acceptedOutcome.evidenceRef,
          record.acceptedOutcome.assignmentRef,
          record.acceptedOutcome.acceptanceRef,
        ],
        label: 'Accepted-outcome value',
        valueRef: 'value.public.artanis_pylon_economics.accepted_outcome',
      }),
      valueRow({
        caveatRefs: record.powerMarket.caveatRefs,
        claimState: record.powerMarket.claimState,
        dollarsPerMwh: record.powerMarket.averageLmpDollarsPerMwh,
        evidenceRefs: [record.powerMarket.evidenceRef],
        label: 'Power cost',
        valueRef: 'value.public.artanis_pylon_economics.power_cost',
      }),
    ],
    windowRef: record.windowRef,
  }

  if (artanisPylonComparativeEconomicsProjectionHasPrivateMaterial(projection)) {
    throw new ArtanisPylonComparativeEconomicsUnsafe({
      reason:
        'Artanis/Pylon comparative economics projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const exampleArtanisPylonComparativeEconomicsPacket = ():
  ArtanisPylonComparativeEconomicsPacketRecord => ({
    acceptedOutcome: {
      acceptedOutcomeValueDollarsPerMwh: 920,
      acceptanceRef: 'accepted_outcome.public.trace_summary.accepted',
      artifactRefs: ['artifact.public.trace_summary'],
      assignmentRef: 'assignment.public.pylon_trace_summary',
      buyerRevenueCents: 230000,
      caveatRefs: ['caveat.public.accepted_work_not_settlement'],
      claimState: 'payable',
      closeoutRef: 'closeout.public.trace_summary',
      economicsRefs: ['economics.public.trace_summary'],
      evidenceRef: 'evidence.public.accepted_outcome.trace_summary',
      gradingRef: 'grading.public.trace_summary',
      grossProfitCents: 148000,
      providerPayableCents: 26000,
      providerSettledCents: 0,
      rejectionRefs: [],
      retryRefs: ['retry.public.none'],
      runRef: 'run.public.trace_summary',
      settlementRefs: [],
    },
    agentRef: 'agent.public.artanis',
    authority: ARTANIS_PYLON_COMPARATIVE_ECONOMICS_READ_ONLY_AUTHORITY,
    caveatRefs: [
      'caveat.public.modeled_until_metered',
      'caveat.public.token_unit_audit_required',
    ],
    gpuRental: {
      caveatRefs: ['caveat.public.vast_sample_spot_market'],
      claimState: 'modeled',
      derivedDollarsPerMwh: 4315.45,
      dollarsPerGpuHour: 6.7872,
      evidenceRef: 'evidence.public.vast_ai.gpu_pricing_20260601',
      gpuModel: 'B300 SXM6 AC',
      listingSampleSize: 26,
      sampleTimestampIso: '2026-06-01T00:00:00.000Z',
      sourceRefs: ['source.public.vast_ai_sample'],
      tdpSourceRef: 'tdp.public.gpu_pricing_json',
      tdpWatts: 1573,
    },
    miningCounterfactual: {
      asicModel: 'Antminer S21 XP',
      capacityMw: 10,
      caveatRefs: ['caveat.public.mining_counterfactual_same_window'],
      claimState: 'modeled',
      curtailmentPolicyRef: 'curtailment.public.threshold',
      efficiencyJoulesPerTh: 13.5,
      evidenceRef: 'evidence.public.mining_counterfactual.ercot_north',
      firmwareAssumptionRef: 'firmware.public.stock',
      miningMarginDollarsPerMwh: 42,
      miningRevenueDollarsPerMwh: 78,
      opsAssumptionRef: 'ops.public.standard_pool_fee',
      poolAssumptionRef: 'pool.public.default_mining_pool',
      sourceRefs: ['source.public.cbeci_machines'],
    },
    packetRef: 'packet.public.artanis_pylon_economics.demo_1',
    powerMarket: {
      averageLmpDollarsPerMwh: 31.2,
      caveatRefs: ['caveat.public.ercot_cache_refresh_lag'],
      claimState: 'measured',
      evidenceRef: 'evidence.public.ercot.north_lmp_window',
      market: 'ercot',
      missingDataFlags: ['missing_data.public.none'],
      refreshedAtIso: '2026-06-06T12:00:00.000Z',
      sourceRefs: ['source.public.ercot_lmp_cache'],
      windowRef: 'window.public.ercot_north.20260601_20260606',
      zoneOrSettlementPoint: 'ERCOT North',
    },
    privateEvidenceRefs: [
      'evidence.private.operator.margot_export_trace',
      'meter.private.operator.pdu_snapshot',
    ],
    provenance: {
      caveatRefs: ['caveat.public.margot_export_operator_reviewed'],
      commitRef: 'commit.public.oa_aibtc_model.efccd28',
      dataTimestampRefs: [
        'data_timestamp.public.vast_ai.20260601',
        'data_timestamp.public.ercot_cache.current',
      ],
      exportedAtIso: '2026-06-06T13:30:00.000Z',
      normalizedExportRefs: ['export.public.margot.normalized.demo_1'],
      repoRef: 'repo.public.dmrobotix.oa_aibtc_model',
      sourceUrlRefs: [
        'https://github.com/dmrobotix/oa_aibtc_model',
        'https://openrouter.ai/api/v1/models',
      ],
    },
    pylonCapacity: {
      availabilityWindowRef: 'availability.public.overnight_window',
      caveatRefs: ['caveat.public.node_power_not_facility_metered'],
      chipTdpWatts: 1000,
      claimState: 'modeled',
      cohortRef: 'cohort.public.pylon_b300_demo',
      coolingPueAssumption: 1.18,
      costTermRef: 'cost.public.operator_reviewed',
      denominatorKind: 'node_system_power',
      effectiveWattsPerGpu: 1573,
      evidenceRef: 'evidence.public.pylon_capacity.b300_node',
      gpuCount: 8,
      gpuModel: 'B300 SXM6 AC',
      idleDarkCapacityRefs: [
        'capacity.public.dark_capacity.overnight_available',
      ],
      interconnectRef: 'interconnect.public.sxm_nvlink',
      measuredIpmiAvailable: false,
      measuredPduAvailable: false,
      meteredFacilityAvailable: false,
      nodePowerAdjustedDollarsPerMwh: 4315.45,
      nodeRef: 'node.public.pylon_b300_demo',
      resourceMode: 'overnight',
      runtimeFrameworkRef: 'runtime.public.pylon_psionic',
      sourceRefs: ['source.public.nodes_json'],
      systemPowerWatts: 12584,
      vramGb: 1536,
    },
    sourceRefs: [
      'source.public.margot_facility_simulator',
      'source.public.oa_aibtc_model_claude_notes',
    ],
    throughputCalculator: {
      calculatorRef: 'calculator.public.ocolo_throughput',
      caveatRefs: ['caveat.public.throughput_calculator_modeled_only'],
      claimState: 'modeled',
      contextWindowRef: 'context.public.128k',
      evidenceRef: 'evidence.public.ocolo.modeled_b200',
      hardwareRef: 'hardware.public.b200_proxy',
      modelRef: 'model.public.b200_proxy',
      modeledAtIso: '2026-06-06T13:45:00.000Z',
      quantizationRef: 'quantization.public.fp8',
      queryParamRefs: [
        'query.public.ocolo.hardware_b200',
        'query.public.ocolo.context_128k',
      ],
      urlRef: 'url.public.ocolo_calculator',
    },
    tokenEconomics: {
      caveatRefs: ['caveat.public.openrouter_ml_energy_unit_audit_pending'],
      claimState: 'modeled',
      completionUsdPerMtok: 0.32,
      completionUsdPerToken: 0.00000032,
      derivedDollarsPerMwh: 6400,
      displayPriceUnit: '$/Mtok',
      energyPerTokenJoules: 0.00018,
      evidenceRef: 'evidence.public.token_inference.b200_gpqa',
      gpuModel: 'NVIDIA B200',
      isStable: true,
      mlEnergyRunRef: 'ml_energy.public.run.b200_gpqa_stable',
      mlEnergyTask: 'gpqa',
      openRouterModelId: 'openrouter_model.public.b200_proxy',
      outputThroughputTokensPerSecond: 125.3,
      priceTimestampIso: '2026-06-06T12:15:00.000Z',
      rawPriceUnit: '$/token',
      sourceRefs: [
        'source.public.openrouter_models',
        'source.public.ml_energy_leaderboard',
      ],
      systemPowerWatts: 8000,
      unitAuditState: 'pending',
    },
    updatedAtIso: '2026-06-06T13:50:00.000Z',
    windowRef: 'window.public.ercot_north.20260601_20260606',
  })
