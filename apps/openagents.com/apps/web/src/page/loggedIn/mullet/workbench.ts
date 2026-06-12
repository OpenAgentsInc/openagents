import {
  decodeMulletScenario,
  type MulletHourlyCandidateMode,
  type MulletHourlyDispatchResult,
  type MulletProvenanceLevel,
  type MulletScenarioKind,
} from '@openagentsinc/mullet-schema'
import {
  calculateAcceptedOutcomeEconomics,
  calculateCapitalReturnSummary,
  roundTo,
  roundUsd,
  safeDivide,
  simulateScenarioHour,
} from '@openagentsinc/mullet-sim'

export const mulletScenarioTemplateIds = [
  'tinybox_shc_power',
  'tinybox_residential_power',
  'tinybox_west_texas_power',
  'facility_100mw_80_20',
  'shc_cpu_vps_colo',
  'miner_gpu_island',
] as const
export type MulletScenarioTemplateId =
  (typeof mulletScenarioTemplateIds)[number]

export const mulletSensitivityAxisIds = [
  'acceptance_rate',
  'demand_fill',
  'electricity_price',
  'hashprice',
  'raw_gpu_rate',
  'review_cost',
  'sla_reserve',
  'provider_minimum_bid',
] as const
export type MulletSensitivityAxisId = (typeof mulletSensitivityAxisIds)[number]

export type MulletAssumptionGroup =
  | 'facility'
  | 'power'
  | 'mining fleet'
  | 'hardware'
  | 'work class'
  | 'provider floor'
  | 'party split'
  | 'capital'

export type MulletValueState =
  | 'modeled'
  | 'measured'
  | 'accepted'
  | 'paid'
  | 'settled'
  | 'placeholder'

export type MulletAssumption = Readonly<{
  id: string
  group: MulletAssumptionGroup
  label: string
  unit: string
  draftValue: string
  provenance: MulletProvenanceLevel
  sourceLabel: string
  confidence: number
  state: MulletValueState
  requiredEvidence: string
}>

type TemplateDefaults = Readonly<{
  title: string
  basis: string
  focus: string
  kind: MulletScenarioKind
  market: string
  zone: string
  nodeType: string
  nodeLabel: string
  capacityMw: number
  aiAllocationMw: number
  electricityUsdPerMwh: number
  siteOpsUsdPerMwh: number
  slaReserveUsdPerMwh: number
  curtailmentValueUsdPerMwh: number
  asicCount: number
  asicWatts: number
  hashpriceUsdPerMwh: number
  poolFeePercent: number
  gpuCount: number
  loadPowerKw: number
  rawGpuRateUsdPerGpuHour: number
  hardwareCapexUsd: number
  buyerPriceUsd: number
  attemptsPerHour: number
  acceptanceRate: number
  demandBacklog: number
  demandFillPercent: number
  humanReviewMinutes: number
  humanReviewHourlyUsd: number
  providerMinimumBidUsdPerHour: number
  providerMinimumBidUsdPerJob: number
  providerWearUsdPerKwh: number
  providerOperatorMarginUsdPerMwh: number
  providerUsd: number
  facilityUsd: number
  hardwareUsd: number
  validatorsUsd: number
  reviewersUsd: number
  settlementUsd: number
  riskReserveUsd: number
  depreciationMonths: number
  discountRatePercent: number
  debtServiceMonthlyUsd: number
  residualValueUsd: number
  sourceLabel: string
}>

const templateDefaults: Record<MulletScenarioTemplateId, TemplateDefaults> = {
  tinybox_shc_power: {
    title: 'Tinybox SHC power',
    basis: 'SHC-style reserved power and operator support',
    focus: 'Accepted-work dispatch against a reserved-power floor',
    kind: 'tinybox_shc_power',
    market: 'SHC',
    zone: 'reserved',
    nodeType: 'tinybox_red_v2',
    nodeLabel: 'Tinybox Red v2',
    capacityMw: 0.003,
    aiAllocationMw: 0.002,
    electricityUsdPerMwh: 68,
    siteOpsUsdPerMwh: 12,
    slaReserveUsdPerMwh: 30,
    curtailmentValueUsdPerMwh: 5,
    asicCount: 0,
    asicWatts: 3500,
    hashpriceUsdPerMwh: 0,
    poolFeePercent: 0,
    gpuCount: 4,
    loadPowerKw: 1.6,
    rawGpuRateUsdPerGpuHour: 1.25,
    hardwareCapexUsd: 12000,
    buyerPriceUsd: 7.5,
    attemptsPerHour: 12,
    acceptanceRate: 0.7,
    demandBacklog: 120,
    demandFillPercent: 0.65,
    humanReviewMinutes: 6,
    humanReviewHourlyUsd: 90,
    providerMinimumBidUsdPerHour: 1.35,
    providerMinimumBidUsdPerJob: 0.18,
    providerWearUsdPerKwh: 0.06,
    providerOperatorMarginUsdPerMwh: 35,
    providerUsd: 1.35,
    facilityUsd: 0.22,
    hardwareUsd: 0.35,
    validatorsUsd: 0.18,
    reviewersUsd: 0.25,
    settlementUsd: 0.02,
    riskReserveUsd: 0.45,
    depreciationMonths: 36,
    discountRatePercent: 14,
    debtServiceMonthlyUsd: 0,
    residualValueUsd: 3500,
    sourceLabel: 'Chris revenue call plus MODEL1 assumptions',
  },
  tinybox_residential_power: {
    title: 'Tinybox residential',
    basis: 'Consumer power, home-network, and inconvenience reserve',
    focus: 'Provider floor sensitivity under expensive retail energy',
    kind: 'tinybox_residential_power',
    market: 'residential',
    zone: 'consumer',
    nodeType: 'tinybox_red_v2',
    nodeLabel: 'Tinybox residential',
    capacityMw: 0.003,
    aiAllocationMw: 0.002,
    electricityUsdPerMwh: 165,
    siteOpsUsdPerMwh: 4,
    slaReserveUsdPerMwh: 45,
    curtailmentValueUsdPerMwh: 0,
    asicCount: 0,
    asicWatts: 3500,
    hashpriceUsdPerMwh: 0,
    poolFeePercent: 0,
    gpuCount: 4,
    loadPowerKw: 1.55,
    rawGpuRateUsdPerGpuHour: 0.95,
    hardwareCapexUsd: 12000,
    buyerPriceUsd: 8.5,
    attemptsPerHour: 10,
    acceptanceRate: 0.62,
    demandBacklog: 80,
    demandFillPercent: 0.5,
    humanReviewMinutes: 8,
    humanReviewHourlyUsd: 95,
    providerMinimumBidUsdPerHour: 1.95,
    providerMinimumBidUsdPerJob: 0.25,
    providerWearUsdPerKwh: 0.08,
    providerOperatorMarginUsdPerMwh: 50,
    providerUsd: 1.7,
    facilityUsd: 0.12,
    hardwareUsd: 0.42,
    validatorsUsd: 0.2,
    reviewersUsd: 0.32,
    settlementUsd: 0.02,
    riskReserveUsd: 0.65,
    depreciationMonths: 36,
    discountRatePercent: 18,
    debtServiceMonthlyUsd: 0,
    residualValueUsd: 3200,
    sourceLabel: 'Residential Tinybox provider-floor model',
  },
  tinybox_west_texas_power: {
    title: 'Tinybox West Texas miner-site power',
    basis: 'Low-cost miner-site power and mining opportunity cost',
    focus: 'Accepted work versus mining and raw GPU backfill',
    kind: 'tinybox_west_texas_power',
    market: 'ERCOT',
    zone: 'LZ_WEST',
    nodeType: 'tinybox_red_v2',
    nodeLabel: 'Tinybox at miner site',
    capacityMw: 0.01,
    aiAllocationMw: 0.002,
    electricityUsdPerMwh: 38,
    siteOpsUsdPerMwh: 6,
    slaReserveUsdPerMwh: 22,
    curtailmentValueUsdPerMwh: 18,
    asicCount: 2,
    asicWatts: 3500,
    hashpriceUsdPerMwh: 94,
    poolFeePercent: 0.02,
    gpuCount: 4,
    loadPowerKw: 1.6,
    rawGpuRateUsdPerGpuHour: 1.4,
    hardwareCapexUsd: 12000,
    buyerPriceUsd: 6.75,
    attemptsPerHour: 14,
    acceptanceRate: 0.72,
    demandBacklog: 150,
    demandFillPercent: 0.7,
    humanReviewMinutes: 5,
    humanReviewHourlyUsd: 90,
    providerMinimumBidUsdPerHour: 1.1,
    providerMinimumBidUsdPerJob: 0.15,
    providerWearUsdPerKwh: 0.05,
    providerOperatorMarginUsdPerMwh: 30,
    providerUsd: 1.25,
    facilityUsd: 0.2,
    hardwareUsd: 0.35,
    validatorsUsd: 0.18,
    reviewersUsd: 0.2,
    settlementUsd: 0.02,
    riskReserveUsd: 0.4,
    depreciationMonths: 36,
    discountRatePercent: 15,
    debtServiceMonthlyUsd: 0,
    residualValueUsd: 3500,
    sourceLabel: 'Miner-site Tinybox discussion model',
  },
  facility_100mw_80_20: {
    title: '100 MW 80/20 facility',
    basis: 'Margot-style 80 MW mining and 20 MW AI allocation',
    focus: 'Facility-scale accepted work versus mining baseline',
    kind: 'facility_100mw_80_20',
    market: 'ERCOT',
    zone: 'LZ_WEST',
    nodeType: 'facility_gpu_cluster',
    nodeLabel: '20 MW AI island',
    capacityMw: 100,
    aiAllocationMw: 20,
    electricityUsdPerMwh: 45,
    siteOpsUsdPerMwh: 5,
    slaReserveUsdPerMwh: 18,
    curtailmentValueUsdPerMwh: 25,
    asicCount: 22857,
    asicWatts: 3500,
    hashpriceUsdPerMwh: 92,
    poolFeePercent: 0.02,
    gpuCount: 12714,
    loadPowerKw: 20000,
    rawGpuRateUsdPerGpuHour: 6.7872,
    hardwareCapexUsd: 38142000,
    buyerPriceUsd: 4.25,
    attemptsPerHour: 110000,
    acceptanceRate: 0.58,
    demandBacklog: 75000,
    demandFillPercent: 0.55,
    humanReviewMinutes: 2,
    humanReviewHourlyUsd: 85,
    providerMinimumBidUsdPerHour: 27000,
    providerMinimumBidUsdPerJob: 0.18,
    providerWearUsdPerKwh: 0.025,
    providerOperatorMarginUsdPerMwh: 15,
    providerUsd: 1.2,
    facilityUsd: 0.28,
    hardwareUsd: 0.42,
    validatorsUsd: 0.08,
    reviewersUsd: 0.05,
    settlementUsd: 0.015,
    riskReserveUsd: 0.22,
    depreciationMonths: 48,
    discountRatePercent: 16,
    debtServiceMonthlyUsd: 0,
    residualValueUsd: 9000000,
    sourceLabel: 'Margot 100 MW 80/20 model import placeholder',
  },
  shc_cpu_vps_colo: {
    title: 'SHC CPU/VPS/colo',
    basis: 'CPU/VPS/colo floor against lower-trust accepted work',
    focus: 'Non-GPU workload fit and provider reserve pricing',
    kind: 'shc_cpu_vps_colo',
    market: 'SHC',
    zone: 'colo',
    nodeType: 'cpu_vps_colo',
    nodeLabel: 'SHC CPU/VPS/colo node',
    capacityMw: 0.02,
    aiAllocationMw: 0.008,
    electricityUsdPerMwh: 74,
    siteOpsUsdPerMwh: 16,
    slaReserveUsdPerMwh: 20,
    curtailmentValueUsdPerMwh: 4,
    asicCount: 0,
    asicWatts: 3500,
    hashpriceUsdPerMwh: 0,
    poolFeePercent: 0,
    gpuCount: 0,
    loadPowerKw: 6,
    rawGpuRateUsdPerGpuHour: 0,
    hardwareCapexUsd: 18000,
    buyerPriceUsd: 1.8,
    attemptsPerHour: 75,
    acceptanceRate: 0.76,
    demandBacklog: 400,
    demandFillPercent: 0.6,
    humanReviewMinutes: 2,
    humanReviewHourlyUsd: 70,
    providerMinimumBidUsdPerHour: 7,
    providerMinimumBidUsdPerJob: 0.08,
    providerWearUsdPerKwh: 0.02,
    providerOperatorMarginUsdPerMwh: 18,
    providerUsd: 0.55,
    facilityUsd: 0.08,
    hardwareUsd: 0.12,
    validatorsUsd: 0.05,
    reviewersUsd: 0.04,
    settlementUsd: 0.01,
    riskReserveUsd: 0.08,
    depreciationMonths: 36,
    discountRatePercent: 12,
    debtServiceMonthlyUsd: 0,
    residualValueUsd: 5000,
    sourceLabel: 'SHC CPU/VPS/colo comparison model',
  },
  miner_gpu_island: {
    title: 'Miner-site GPU island',
    basis: 'Dedicated GPU island inside a mining floor',
    focus: 'Raw GPU market, accepted work, mining, and curtailment',
    kind: 'miner_gpu_island',
    market: 'ERCOT',
    zone: 'LZ_WEST',
    nodeType: 'miner_gpu_island',
    nodeLabel: 'Miner-site GPU island',
    capacityMw: 1,
    aiAllocationMw: 0.25,
    electricityUsdPerMwh: 42,
    siteOpsUsdPerMwh: 7,
    slaReserveUsdPerMwh: 20,
    curtailmentValueUsdPerMwh: 30,
    asicCount: 214,
    asicWatts: 3500,
    hashpriceUsdPerMwh: 90,
    poolFeePercent: 0.02,
    gpuCount: 160,
    loadPowerKw: 250,
    rawGpuRateUsdPerGpuHour: 4.2,
    hardwareCapexUsd: 480000,
    buyerPriceUsd: 5.2,
    attemptsPerHour: 2400,
    acceptanceRate: 0.64,
    demandBacklog: 2200,
    demandFillPercent: 0.62,
    humanReviewMinutes: 3,
    humanReviewHourlyUsd: 90,
    providerMinimumBidUsdPerHour: 360,
    providerMinimumBidUsdPerJob: 0.16,
    providerWearUsdPerKwh: 0.035,
    providerOperatorMarginUsdPerMwh: 24,
    providerUsd: 1.05,
    facilityUsd: 0.2,
    hardwareUsd: 0.38,
    validatorsUsd: 0.09,
    reviewersUsd: 0.08,
    settlementUsd: 0.015,
    riskReserveUsd: 0.18,
    depreciationMonths: 42,
    discountRatePercent: 16,
    debtServiceMonthlyUsd: 0,
    residualValueUsd: 120000,
    sourceLabel: 'Miner-site GPU island model',
  },
}

export const mulletScenarioTemplates = mulletScenarioTemplateIds.map(id => ({
  id,
  basis: templateDefaults[id].basis,
  focus: templateDefaults[id].focus,
  title: templateDefaults[id].title,
}))
export const defaultMulletScenarioTemplate = mulletScenarioTemplates[0]!

export const defaultMulletScenarioTemplateId: MulletScenarioTemplateId =
  'tinybox_shc_power'
export const defaultMulletSensitivityAxisId: MulletSensitivityAxisId =
  'acceptance_rate'

const now = '2026-06-08T00:00:00.000Z'

const assumption = (
  id: string,
  group: MulletAssumptionGroup,
  label: string,
  value: number,
  unit: string,
  sourceLabel: string,
  requiredEvidence: string,
  state: MulletValueState = 'modeled',
): MulletAssumption => ({
  id,
  group,
  label,
  unit,
  draftValue: String(value),
  provenance: state === 'placeholder' ? 'placeholder' : state,
  sourceLabel,
  confidence: state === 'measured' ? 0.9 : 0.72,
  state,
  requiredEvidence,
})

export const assumptionsForTemplate = (
  templateId: MulletScenarioTemplateId,
): ReadonlyArray<MulletAssumption> => {
  const t = templateDefaults[templateId]
  const source = t.sourceLabel

  return [
    assumption(
      'facility.capacityMw',
      'facility',
      'Facility capacity',
      t.capacityMw,
      'MW',
      source,
      'Interconnection, transformer, and hosting agreement',
    ),
    assumption(
      'facility.aiAllocationMw',
      'facility',
      'AI allocation',
      t.aiAllocationMw,
      'MW',
      source,
      'Operator allocation decision or measured load plan',
    ),
    assumption(
      'facility.siteOpsUsdPerMwh',
      'facility',
      'Site ops',
      t.siteOpsUsdPerMwh,
      '$/MWh',
      source,
      'Facility operator O&M quote',
    ),
    assumption(
      'power.electricityUsdPerMwh',
      'power',
      'Electricity price',
      t.electricityUsdPerMwh,
      '$/MWh',
      source,
      'Power bill, PPA, or market price trace',
    ),
    assumption(
      'power.slaReserveUsdPerMwh',
      'power',
      'SLA reserve',
      t.slaReserveUsdPerMwh,
      '$/MWh',
      source,
      'Customer SLA reserve policy',
    ),
    assumption(
      'power.curtailmentValueUsdPerMwh',
      'power',
      'Curtailment value',
      t.curtailmentValueUsdPerMwh,
      '$/MWh',
      source,
      'Grid service or curtailment signal',
    ),
    assumption(
      'mining.asicCount',
      'mining fleet',
      'ASIC count',
      t.asicCount,
      'units',
      source,
      'Mining fleet inventory',
    ),
    assumption(
      'mining.wattsPerUnit',
      'mining fleet',
      'ASIC watts',
      t.asicWatts,
      'W',
      source,
      'Miner spec or measured draw',
    ),
    assumption(
      'mining.hashpriceUsdPerMwh',
      'mining fleet',
      'Hashprice revenue',
      t.hashpriceUsdPerMwh,
      '$/MWh',
      source,
      'Hashprice and fleet efficiency trace',
    ),
    assumption(
      'mining.poolFeePercent',
      'mining fleet',
      'Pool fee',
      t.poolFeePercent,
      'decimal',
      source,
      'Pool terms',
    ),
    assumption(
      'hardware.gpuCount',
      'hardware',
      'GPU count',
      t.gpuCount,
      'units',
      source,
      'Hardware inventory',
    ),
    assumption(
      'hardware.loadPowerKw',
      'hardware',
      'Load power',
      t.loadPowerKw,
      'kW',
      source,
      'Measured node power',
    ),
    assumption(
      'hardware.rawGpuRateUsdPerGpuHour',
      'hardware',
      'Raw GPU rate',
      t.rawGpuRateUsdPerGpuHour,
      '$/GPU-hour',
      source,
      'Comparable rental market quote',
    ),
    assumption(
      'hardware.capexUsd',
      'hardware',
      'Hardware capex',
      t.hardwareCapexUsd,
      '$',
      source,
      'Invoice, quote, or Margot baseline',
    ),
    assumption(
      'work.buyerPriceUsd',
      'work class',
      'Buyer price',
      t.buyerPriceUsd,
      '$/accepted outcome',
      source,
      'MODEL1 price or customer quote',
    ),
    assumption(
      'work.attemptsPerHour',
      'work class',
      'Attempts',
      t.attemptsPerHour,
      'attempts/hour',
      source,
      'Benchmark result',
    ),
    assumption(
      'work.acceptanceRate',
      'work class',
      'Acceptance rate',
      t.acceptanceRate,
      'decimal',
      source,
      'Accepted-work history or forecast',
    ),
    assumption(
      'work.demandBacklog',
      'work class',
      'Accepted-work backlog',
      t.demandBacklog,
      'outcomes',
      source,
      'Customer demand evidence',
    ),
    assumption(
      'work.demandFillPercent',
      'work class',
      'Demand fill',
      t.demandFillPercent,
      'decimal',
      source,
      'Demand fill forecast',
    ),
    assumption(
      'work.humanReviewMinutes',
      'work class',
      'Review time',
      t.humanReviewMinutes,
      'minutes',
      source,
      'Reviewer process estimate',
    ),
    assumption(
      'work.humanReviewHourlyUsd',
      'work class',
      'Review cost',
      t.humanReviewHourlyUsd,
      '$/hour',
      source,
      'Reviewer labor rate',
    ),
    assumption(
      'provider.minimumBidUsdPerHour',
      'provider floor',
      'Minimum bid',
      t.providerMinimumBidUsdPerHour,
      '$/hour',
      source,
      'Provider bid or operator floor',
    ),
    assumption(
      'provider.minimumBidUsdPerJob',
      'provider floor',
      'Minimum job bid',
      t.providerMinimumBidUsdPerJob,
      '$/job',
      source,
      'Provider bid policy',
    ),
    assumption(
      'provider.wearUsdPerKwh',
      'provider floor',
      'Wear',
      t.providerWearUsdPerKwh,
      '$/kWh',
      source,
      'Hardware wear model',
    ),
    assumption(
      'provider.operatorMarginUsdPerMwh',
      'provider floor',
      'Operator margin floor',
      t.providerOperatorMarginUsdPerMwh,
      '$/MWh',
      source,
      'Provider floor policy',
    ),
    assumption(
      'split.providerUsd',
      'party split',
      'Provider share',
      t.providerUsd,
      '$/accepted outcome',
      source,
      'Draft payout split',
    ),
    assumption(
      'split.facilityUsd',
      'party split',
      'Facility share',
      t.facilityUsd,
      '$/accepted outcome',
      source,
      'Draft payout split',
    ),
    assumption(
      'split.hardwareUsd',
      'party split',
      'Hardware share',
      t.hardwareUsd,
      '$/accepted outcome',
      source,
      'Draft payout split',
    ),
    assumption(
      'split.validatorsUsd',
      'party split',
      'Validator share',
      t.validatorsUsd,
      '$/accepted outcome',
      source,
      'Draft payout split',
    ),
    assumption(
      'split.reviewersUsd',
      'party split',
      'Reviewer share',
      t.reviewersUsd,
      '$/accepted outcome',
      source,
      'Draft payout split',
    ),
    assumption(
      'split.settlementUsd',
      'party split',
      'Settlement cost',
      t.settlementUsd,
      '$/accepted outcome',
      source,
      'Settlement and payment rail estimate',
    ),
    assumption(
      'split.riskReserveUsd',
      'party split',
      'Risk reserve',
      t.riskReserveUsd,
      '$/accepted outcome',
      source,
      'Risk reserve policy',
    ),
    assumption(
      'capital.depreciationMonths',
      'capital',
      'Depreciation',
      t.depreciationMonths,
      'months',
      source,
      'Capital model',
    ),
    assumption(
      'capital.discountRatePercent',
      'capital',
      'Discount rate',
      t.discountRatePercent,
      '%',
      source,
      'Capital model',
    ),
    assumption(
      'capital.debtServiceMonthlyUsd',
      'capital',
      'Debt service',
      t.debtServiceMonthlyUsd,
      '$/month',
      source,
      'Capital model',
    ),
    assumption(
      'capital.residualValueUsd',
      'capital',
      'Residual value',
      t.residualValueUsd,
      '$',
      source,
      'Capital model',
    ),
  ]
}

export const updateMulletAssumption = (
  assumptions: ReadonlyArray<MulletAssumption>,
  input: Readonly<{ assumptionId: string; field: string; value: string }>,
): ReadonlyArray<MulletAssumption> =>
  assumptions.map(assumption =>
    assumption.id !== input.assumptionId
      ? assumption
      : {
          ...assumption,
          ...(input.field === 'value' ? { draftValue: input.value } : {}),
          ...(input.field === 'sourceLabel'
            ? { sourceLabel: input.value }
            : {}),
          ...(input.field === 'provenance'
            ? {
                provenance: provenanceFromInput(input.value),
                state: valueStateFromProvenance(input.value),
              }
            : {}),
        },
  )

const provenanceFromInput = (value: string): MulletProvenanceLevel =>
  [
    'modeled',
    'measured',
    'accepted',
    'paid',
    'settled',
    'placeholder',
    'manual_input',
    'estimated',
    'forecast',
    'observed',
    'verified',
  ].includes(value)
    ? (value as MulletProvenanceLevel)
    : 'manual_input'

const valueStateFromProvenance = (value: string): MulletValueState =>
  ['modeled', 'measured', 'accepted', 'paid', 'settled', 'placeholder'].includes(
    value,
  )
    ? (value as MulletValueState)
    : 'modeled'

const numberFromAssumptions = (
  assumptions: ReadonlyArray<MulletAssumption>,
  id: string,
): number => {
  const raw = assumptions.find(assumption => assumption.id === id)?.draftValue
  const value = Number.parseFloat(raw ?? '')

  return Number.isFinite(value) ? value : 0
}

export type MulletWorkbenchProjection = Readonly<{
  template: (typeof mulletScenarioTemplates)[number]
  dispatch: MulletHourlyDispatchResult
  selectedCandidate: MulletHourlyCandidateMode
  candidateRows: ReadonlyArray<Record<string, string>>
  partyRows: ReadonlyArray<Record<string, string>>
  acceptedMetricRows: ReadonlyArray<Record<string, string>>
  sensitivityRows: ReadonlyArray<Record<string, string>>
  evidenceRows: ReadonlyArray<Record<string, string>>
  effectiveBuyerPriceUsd: number
  modeledStates: ReadonlyArray<MulletValueState>
}>

type SimulationProjection = Readonly<{
  dispatch: MulletHourlyDispatchResult
  selectedCandidate: MulletHourlyCandidateMode
  effectiveBuyerPriceUsd: number
  contributionUsdPerOutcome: number
}>

export const deriveMulletWorkbenchProjection = (input: {
  readonly selectedTemplateId: MulletScenarioTemplateId
  readonly selectedSensitivityAxisId: MulletSensitivityAxisId
  readonly assumptions: ReadonlyArray<MulletAssumption>
}): MulletWorkbenchProjection => {
  const template =
    mulletScenarioTemplates.find(item => item.id === input.selectedTemplateId) ??
    defaultMulletScenarioTemplate
  const projection = simulateAssumptions(
    input.selectedTemplateId,
    input.assumptions,
  )

  return {
    template,
    dispatch: projection.dispatch,
    selectedCandidate: projection.selectedCandidate,
    candidateRows: candidateRows(projection.dispatch),
    partyRows: partyRows(input.assumptions, projection),
    acceptedMetricRows: acceptedMetricRows(input.assumptions, projection),
    sensitivityRows: sensitivityRows(input),
    evidenceRows: evidenceRows(),
    effectiveBuyerPriceUsd: projection.effectiveBuyerPriceUsd,
    modeledStates: ['modeled', 'measured', 'accepted', 'paid', 'settled'],
  }
}

const simulateAssumptions = (
  templateId: MulletScenarioTemplateId,
  assumptions: ReadonlyArray<MulletAssumption>,
): SimulationProjection => {
  const v = (id: string) => numberFromAssumptions(assumptions, id)
  const splitTotal =
    v('split.providerUsd') +
    v('split.facilityUsd') +
    v('split.hardwareUsd') +
    v('split.validatorsUsd') +
    v('split.reviewersUsd') +
    v('split.settlementUsd') +
    v('split.riskReserveUsd')
  const economics = calculateAcceptedOutcomeEconomics({
    inputTokens: 1_000_000,
    outputTokens: 100_000,
    inputUsdPerMillion: 3,
    outputUsdPerMillion: 15,
    workroomRuntimeHours: 0.25,
    workroomHourlyCostUsd: 0.25,
    providerExecutionJobs: 1,
    providerPayoutEachUsd: v('split.providerUsd'),
    validatorCount: 1,
    validatorPayoutEachUsd: v('split.validatorsUsd'),
    humanReviewMinutes: v('work.humanReviewMinutes'),
    humanReviewHourlyCostUsd: v('work.humanReviewHourlyUsd'),
    artifactStorageCostUsd: 0.02,
    settlementCostUsd: v('split.settlementUsd'),
    supportOverheadUsd: 0.05,
    acceptanceRate: Math.max(0.01, v('work.acceptanceRate')),
    riskReserveUsd: v('split.riskReserveUsd'),
    targetMargin: 0.3,
  })
  const effectiveBuyerPriceUsd = Math.max(
    v('work.buyerPriceUsd'),
    economics.buyerPriceUsd,
  )
  const contributionUsdPerOutcome = Math.max(
    0,
    effectiveBuyerPriceUsd - splitTotal,
  )
  const dispatch = simulateScenarioHour(
    decodeMulletScenario(rawScenario(templateId, assumptions, effectiveBuyerPriceUsd)),
    {
      effectivePriceUsdPerMwh: v('power.electricityUsdPerMwh'),
      miningRevenueUsdPerMwh: v('mining.hashpriceUsdPerMwh'),
      rawGpuRevenueUsdPerHour:
        v('hardware.gpuCount') *
        v('hardware.rawGpuRateUsdPerGpuHour') *
        Math.max(0, v('work.demandFillPercent')),
      tokenApiRevenueUsdPerHour:
        effectiveBuyerPriceUsd *
        v('work.attemptsPerHour') *
        v('work.acceptanceRate') *
        0.08,
      curtailmentValueUsdPerMwh: v('power.curtailmentValueUsdPerMwh'),
    },
  )
  const selectedCandidate =
    dispatch.candidates.find(candidate => candidate.mode === dispatch.selectedMode) ??
    dispatch.candidates[0]!

  return { dispatch, selectedCandidate, effectiveBuyerPriceUsd, contributionUsdPerOutcome }
}

const rawScenario = (
  templateId: MulletScenarioTemplateId,
  assumptions: ReadonlyArray<MulletAssumption>,
  effectiveBuyerPriceUsd: number,
): unknown => {
  const t = templateDefaults[templateId]
  const v = (id: string) => numberFromAssumptions(assumptions, id)
  const sourceRef = {
    id: `source_${templateId}`,
    label: t.sourceLabel,
    uri: 'docs/mullet/2026-06-08-omega-unified-mullet-simulation-runner-audit.md',
    capturedAt: now,
  }
  const pn = (id: string, unit: string) => {
    const sourceAssumption = assumptions.find(assumption => assumption.id === id)

    return {
      value: v(id),
      unit,
      provenance: sourceAssumption?.provenance ?? 'modeled',
      confidence: sourceAssumption?.confidence ?? 0.72,
      source: sourceRef,
      lastUpdated: now,
      needsDiligence: sourceAssumption?.state !== 'measured',
    }
  }
  const ps = (value: string) => ({
    value,
    unit: 'text',
    provenance: 'modeled',
    confidence: 0.72,
    source: sourceRef,
    lastUpdated: now,
    needsDiligence: true,
  })

  return {
    id: `mullet_${templateId}`,
    name: t.title,
    schemaVersion: '2026-06-08.v1',
    kind: t.kind,
    createdAt: now,
    updatedAt: now,
    dateRange: {
      startAt: now,
      endAt: '2026-06-09T00:00:00.000Z',
    },
    facility: {
      id: `facility_${templateId}`,
      siteId: `site_${templateId}`,
      name: t.title,
      market: t.market,
      zone: t.zone,
      capacityMw: pn('facility.capacityMw', 'megawatts'),
      powerContractType: 'modeled_private_operator',
      fixedPriceUsdPerMwh: pn('power.electricityUsdPerMwh', 'usd_per_mwh'),
      maxAiAllocationMw: pn('facility.aiAllocationMw', 'megawatts'),
      curtailmentPolicy: 'modeled_grid_optional',
      gridServiceTerms: 'not_settled',
      siteOpsCostUsdPerMwh: pn('facility.siteOpsUsdPerMwh', 'usd_per_mwh'),
      coolingMultiplier: {
        value: 1.12,
        unit: 'score',
        provenance: 'modeled',
        confidence: 0.72,
        source: sourceRef,
        lastUpdated: now,
        needsDiligence: true,
      },
      remoteHandsMonthlyUsd: {
        value: 0,
        unit: 'usd',
        provenance: 'modeled',
        confidence: 0.72,
        source: sourceRef,
        lastUpdated: now,
        needsDiligence: true,
      },
      physicalReadiness: {
        interconnectionStatus: ps('modeled'),
        energizedCapacityMw: pn('facility.capacityMw', 'megawatts'),
        transformerStatus: ps('modeled'),
        switchgearStatus: ps('modeled'),
        coolingCapacityKw: {
          value: Math.max(1, v('facility.aiAllocationMw') * 1000 * 1.15),
          unit: 'kilowatts',
          provenance: 'modeled',
          confidence: 0.72,
          source: sourceRef,
          lastUpdated: now,
          needsDiligence: true,
        },
        pue: {
          value: 1.18,
          unit: 'score',
          provenance: 'modeled',
          confidence: 0.72,
          source: sourceRef,
          lastUpdated: now,
          needsDiligence: true,
        },
        fiberOrNetworkStatus: ps('modeled'),
        redundancyTier: ps('operator_review_required'),
        remoteHandsSla: ps('operator_review_required'),
        uptimeCommitment: {
          value: 0.95,
          unit: 'percent',
          provenance: 'modeled',
          confidence: 0.72,
          source: sourceRef,
          lastUpdated: now,
          needsDiligence: true,
        },
        liquidatedDamagesExposureUsd: {
          value: 0,
          unit: 'usd',
          provenance: 'modeled',
          confidence: 0.72,
          source: sourceRef,
          lastUpdated: now,
          needsDiligence: true,
        },
        permittingOrCommunityRisk: ps('operator_review_required'),
      },
      customerSlaReserveUsdPerMwh: pn(
        'power.slaReserveUsdPerMwh',
        'usd_per_mwh',
      ),
      siteClassification:
        templateId === 'facility_100mw_80_20'
          ? 'balanced_hybrid'
          : 'mullet_ai_led_mining_backfill',
      readinessState: 'benchmark_passed',
      capacityLifecycleState: 'eligible',
    },
    miningFleet: {
      asicModel: 'Modeled S21-equivalent',
      count: Math.max(0, v('mining.asicCount')),
      wattsPerUnit: v('mining.wattsPerUnit'),
      thPerUnit: 200,
      joulesPerTh: 17.5,
      capexPerUnitUsd: 4000,
      depreciationMonths: 36,
      poolFeePercent: v('mining.poolFeePercent'),
      firmwareOpsCostUsdPerMwh: v('facility.siteOpsUsdPerMwh'),
    },
    computeNodes: [
      {
        nodeId: `node_${templateId}`,
        nodeType: t.nodeType,
        ownerParty: 'hardware_owner',
        operatorParty: 'facility_operator',
        siteId: `site_${templateId}`,
        gpuModel: t.nodeType === 'cpu_vps_colo' ? undefined : 'modeled_gpu',
        gpuCount: v('hardware.gpuCount'),
        vramGb: t.nodeType === 'cpu_vps_colo' ? 0 : 64,
        interconnect: t.nodeType === 'cpu_vps_colo' ? 'ethernet' : 'pcie',
        cpu: t.nodeType === 'cpu_vps_colo' ? 'colo_cpu' : 'gpu_host_cpu',
        ramGb: t.nodeType === 'cpu_vps_colo' ? 512 : 128,
        storageGb: 2000,
        networkGbps: t.nodeType === 'cpu_vps_colo' ? 10 : 1,
        capexUsd: v('hardware.capexUsd'),
        depreciationMonths: v('capital.depreciationMonths'),
        idlePowerKw: Math.max(0.1, v('hardware.loadPowerKw') * 0.2),
        loadPowerKw: Math.max(0.1, v('hardware.loadPowerKw')),
        powerLimitKw: Math.max(0.1, v('hardware.loadPowerKw') * 1.05),
        supportMonthlyUsd: 150,
        fallbackMarketEligible: true,
        trustTier: 'pilot',
        readinessState: 'benchmark_passed',
        capacityLifecycleState: 'admitted',
        workloadFit: [`work_${templateId}`],
      },
    ],
    runtimeBenchmarks: [
      {
        nodeId: `node_${templateId}`,
        workClassId: `work_${templateId}`,
        modelId: 'operator-modeled-workload',
        framework: 'openagents-mullet',
        precision: 'mixed',
        batchSize: 1,
        attemptsPerInstanceHour: Math.max(0, v('work.attemptsPerHour')),
        tokensPerSecond: 80,
        joulesPerToken: 10,
        kwhPerAttempt: safeDivide(v('hardware.loadPowerKw'), Math.max(1, v('work.attemptsPerHour'))),
        wallSecondsPerAttempt: safeDivide(3600, Math.max(1, v('work.attemptsPerHour'))),
        observedFailureRate: Math.max(0, 1 - v('work.acceptanceRate')),
        source: sourceRef,
        confidence: 0.72,
      },
    ],
    workClasses: [
      {
        id: `work_${templateId}`,
        label: `${t.title} accepted outcome`,
        latencyClass: 'batch',
        buyerPriceUsd: roundUsd(effectiveBuyerPriceUsd),
        acceptanceRate: Math.max(0.01, v('work.acceptanceRate')),
        targetMargin: 0.3,
        riskReserveUsd: v('split.riskReserveUsd'),
        frontierInputTokens: 1_000_000,
        frontierOutputTokens: 100_000,
        frontierInputPriceUsdPerMillion: 3,
        frontierOutputPriceUsdPerMillion: 15,
        cheapModelCostUsd: 0,
        workroomRuntimeHours: 0.25,
        workroomHourlyCostUsd: 0.25,
        providerComputeHours: 1,
        providerPowerKw: v('hardware.loadPowerKw'),
        validatorCount: 1,
        validatorPayoutEachUsd: v('split.validatorsUsd'),
        graderCostUsd: 0,
        humanReviewMinutes: v('work.humanReviewMinutes'),
        humanReviewHourlyCostUsd: v('work.humanReviewHourlyUsd'),
        artifactStorageCostUsd: 0.02,
        settlementCostUsd: v('split.settlementUsd'),
        supportOverheadUsd: 0.05,
        retryOrFailureCostUsd: 0.02,
        flexibility: {
          canPause: true,
          canResume: true,
          canMigrate: true,
          checkpointIntervalMinutes: 10,
          maxDelayMinutes: 1440,
          deadlineMinutes: 1440,
          customerImpactIfDelayed: 'modeled',
          privacyTier: 'operator_private',
          stateLocality: 'portable',
          requiredTools: ['artifact-store'],
        },
        demandBacklog: Math.max(0, v('work.demandBacklog')),
        eligibleNodeTypes: [t.nodeType],
        minimumTrustTier: 'pilot',
      },
    ],
    providerPolicies: [
      {
        nodeType: t.nodeType,
        providerMinimumBidUsdPerHour: v('provider.minimumBidUsdPerHour'),
        providerMinimumBidUsdPerJob: v('provider.minimumBidUsdPerJob'),
        wearUsdPerKwh: v('provider.wearUsdPerKwh'),
        bandwidthUsdPerJob: 0.01,
        desiredProfitUsdPerJob: 0.1,
        operatorMarginUsdPerMwh: v('provider.operatorMarginUsdPerMwh'),
        miningFloorPolicy: 'modeled',
        rawGpuFloorPolicy: 'modeled',
        vpsColocationFloorPolicy: 'modeled',
        curtailmentGridServiceFloorPolicy: 'modeled',
      },
    ],
    partySplit: {
      buyerPaysUsd: roundUsd(effectiveBuyerPriceUsd),
      openagentsKeepsUsd: roundUsd(
        Math.max(
          0,
          effectiveBuyerPriceUsd -
            v('split.providerUsd') -
            v('split.facilityUsd') -
            v('split.hardwareUsd') -
            v('split.validatorsUsd') -
            v('split.reviewersUsd') -
            v('split.settlementUsd') -
            v('split.riskReserveUsd'),
        ),
      ),
      providerReceivesUsd: v('split.providerUsd'),
      facilityOperatorReceivesUsd: v('split.facilityUsd'),
      hardwareOwnerReceivesUsd: v('split.hardwareUsd'),
      validatorsReceiveUsd: v('split.validatorsUsd'),
      reviewersReceiveUsd: v('split.reviewersUsd'),
      settlementCostUsd: v('split.settlementUsd'),
      riskReserveUsd: v('split.riskReserveUsd'),
    },
    capitalAssumptions: {
      hardwareCapexUsd: pn('hardware.capexUsd', 'usd'),
      depreciationMonths: pn('capital.depreciationMonths', 'months'),
      discountRatePercent: pn('capital.discountRatePercent', 'percent'),
      debtServiceMonthlyUsd: pn('capital.debtServiceMonthlyUsd', 'usd'),
      residualValueUsd: pn('capital.residualValueUsd', 'usd'),
    },
    demandAssumptions: {
      acceptedWorkBacklog: pn('work.demandBacklog', 'outcomes'),
      demandFillPercent: pn('work.demandFillPercent', 'percent'),
      rawGpuMarketFillPercent: pn('work.demandFillPercent', 'percent'),
      tokenApiFillPercent: {
        value: 0.08,
        unit: 'percent',
        provenance: 'modeled',
        confidence: 0.72,
        source: sourceRef,
        lastUpdated: now,
        needsDiligence: true,
      },
    },
    sourceRefs: [sourceRef],
    notes: 'Private modeled scenario. Do not promote without measured, accepted, paid, and settled evidence.',
  }
}

const modeLabel = (mode: string): string =>
  ({
    mine: 'Mine',
    raw_gpu_market: 'Raw GPU',
    token_api_inference: 'Token/API',
    openagents_accepted_work: 'Accepted work',
    curtail: 'Curtail',
    idle: 'Idle',
    reserve: 'Reserve',
  })[mode] ?? mode

const candidateRows = (
  dispatch: MulletHourlyDispatchResult,
): ReadonlyArray<Record<string, string>> =>
  dispatch.candidates.map(candidate => ({
    mode: modeLabel(candidate.mode),
    selected: candidate.mode === dispatch.selectedMode ? 'Selected' : '',
    revenue: dollars(Number(candidate.buyerRevenueUsd)),
    margin: dollars(Number(candidate.openagentsMarginUsd)),
    provider: dollars(Number(candidate.providerPayoutUsd)),
    energy: `${roundTo(Number(candidate.energyMwh), 4)} MWh`,
    outcomes: roundTo(candidate.acceptedOutcomes, 2).toLocaleString(),
    net: `${dollars(Number(candidate.riskAdjustedNetUsdPerMwh))}/MWh`,
    gates: [
      candidate.clearsReadiness ? 'ready' : 'readiness blocked',
      candidate.clearsDemand ? 'demand' : 'no demand',
      candidate.clearsProviderFloor ? 'floor' : 'floor blocked',
    ].join(' / '),
  }))

const partyRows = (
  assumptions: ReadonlyArray<MulletAssumption>,
  projection: SimulationProjection,
): ReadonlyArray<Record<string, string>> => {
  const v = (id: string) => numberFromAssumptions(assumptions, id)
  const accepted = projection.dispatch.acceptedOutcomes
  const energyCost =
    Number(projection.dispatch.effectivePriceUsdPerMwh) *
    Number(projection.dispatch.energyMwh)
  const rows = [
    {
      party: 'OpenAgents',
      buyerRevenueIncluded: 'Yes, once',
      grossRevenueUsd: Number(projection.dispatch.acceptedWorkBuyerRevenueUsd),
      cogsUsd:
        Number(projection.dispatch.acceptedWorkProviderPayoutUsd) +
        accepted *
          (v('split.facilityUsd') +
            v('split.hardwareUsd') +
            v('split.validatorsUsd') +
            v('split.reviewersUsd') +
            v('split.settlementUsd') +
            v('split.riskReserveUsd')),
      capexUsd: 0,
      monthlyNetUsd:
        Number(projection.dispatch.acceptedWorkOpenagentsMarginUsd) * 24 * 30,
      residualValueUsd: 0,
    },
    {
      party: 'Provider',
      buyerRevenueIncluded: 'No, payout only',
      grossRevenueUsd: Number(projection.dispatch.acceptedWorkProviderPayoutUsd),
      cogsUsd: energyCost + Number(projection.dispatch.energyMwh) * 1000 * v('provider.wearUsdPerKwh'),
      capexUsd: 0,
      monthlyNetUsd:
        (Number(projection.dispatch.acceptedWorkProviderPayoutUsd) -
          energyCost) *
        24 *
        30,
      residualValueUsd: 0,
    },
    {
      party: 'Facility operator',
      buyerRevenueIncluded: 'No, payout only',
      grossRevenueUsd: accepted * v('split.facilityUsd'),
      cogsUsd:
        Number(projection.dispatch.energyMwh) *
        v('facility.siteOpsUsdPerMwh'),
      capexUsd: 0,
      monthlyNetUsd: accepted * v('split.facilityUsd') * 24 * 30,
      residualValueUsd: 0,
    },
    {
      party: 'Hardware owner',
      buyerRevenueIncluded: 'No, payout only',
      grossRevenueUsd: accepted * v('split.hardwareUsd'),
      cogsUsd: safeDivide(v('hardware.capexUsd'), Math.max(1, v('capital.depreciationMonths')) * 30 * 24),
      capexUsd: v('hardware.capexUsd'),
      monthlyNetUsd: accepted * v('split.hardwareUsd') * 24 * 30,
      residualValueUsd: v('capital.residualValueUsd'),
    },
    {
      party: 'Validators',
      buyerRevenueIncluded: 'No, payout only',
      grossRevenueUsd: accepted * v('split.validatorsUsd'),
      cogsUsd: 0,
      capexUsd: 0,
      monthlyNetUsd: accepted * v('split.validatorsUsd') * 24 * 30,
      residualValueUsd: 0,
    },
    {
      party: 'Reviewers',
      buyerRevenueIncluded: 'No, payout only',
      grossRevenueUsd: accepted * v('split.reviewersUsd'),
      cogsUsd:
        safeDivide(v('work.humanReviewMinutes'), 60) *
        v('work.humanReviewHourlyUsd') *
        accepted,
      capexUsd: 0,
      monthlyNetUsd: accepted * v('split.reviewersUsd') * 24 * 30,
      residualValueUsd: 0,
    },
  ]

  return rows.map(row => {
    const summary = calculateCapitalReturnSummary({
      party: row.party,
      grossRevenueUsd: row.grossRevenueUsd,
      cogsUsd: row.cogsUsd,
      capexUsd: row.capexUsd,
      monthlyNetUsd: row.monthlyNetUsd,
      months: Math.max(1, v('capital.depreciationMonths')),
      annualDiscountRatePercent: v('capital.discountRatePercent'),
      residualValueUsd: row.residualValueUsd,
    })

    return {
      party: row.party,
      basis: row.buyerRevenueIncluded,
      gross: dollars(Number(summary.grossRevenueUsd)),
      cogs: dollars(Number(summary.cogsUsd)),
      margin: dollars(Number(summary.grossMarginUsd)),
      payback: Number.isFinite(summary.paybackMonths)
        ? `${roundTo(summary.paybackMonths, 1)} mo`
        : 'No payback',
      npv: dollars(Number(summary.npvUsd)),
    }
  })
}

const acceptedMetricRows = (
  assumptions: ReadonlyArray<MulletAssumption>,
  projection: SimulationProjection,
): ReadonlyArray<Record<string, string>> => {
  const energyMwh = Math.max(0.000001, Number(projection.dispatch.energyMwh))
  const acceptedPerMwh = projection.dispatch.acceptedOutcomesPerMwh
  const acceptedPerKwh = acceptedPerMwh / 1000
  const revenuePerMwh = safeDivide(
    Number(projection.dispatch.acceptedWorkBuyerRevenueUsd),
    energyMwh,
  )
  const marginPerMwh = safeDivide(
    Number(projection.dispatch.acceptedWorkOpenagentsMarginUsd),
    energyMwh,
  )
  const providerPerMwh = safeDivide(
    Number(projection.dispatch.acceptedWorkProviderPayoutUsd),
    energyMwh,
  )
  const v = (id: string) => numberFromAssumptions(assumptions, id)
  const dailyEnergyCost =
    (Number(projection.dispatch.effectivePriceUsdPerMwh) +
      v('power.slaReserveUsdPerMwh')) *
    energyMwh *
    24
  const breakevenOutcomesPerDay = safeDivide(
    dailyEnergyCost,
    Math.max(0.01, projection.contributionUsdPerOutcome),
  )

  return [
    {
      metric: 'Accepted outcomes/kWh',
      value: roundTo(acceptedPerKwh, 5).toLocaleString(),
      state: 'Modeled',
    },
    {
      metric: 'Accepted outcomes/MWh',
      value: roundTo(acceptedPerMwh, 2).toLocaleString(),
      state: 'Modeled',
    },
    {
      metric: 'Revenue/MWh',
      value: dollarsPerMwh(revenuePerMwh),
      state: 'Modeled',
    },
    {
      metric: 'Margin/MWh',
      value: dollarsPerMwh(marginPerMwh),
      state: 'Modeled',
    },
    {
      metric: 'Provider payout/MWh',
      value: dollarsPerMwh(providerPerMwh),
      state: 'Modeled',
    },
    {
      metric: 'Breakeven accepted outcomes/day',
      value: roundTo(breakevenOutcomesPerDay, 1).toLocaleString(),
      state: 'Modeled',
    },
  ]
}

const sensitivityAxes: ReadonlyArray<
  Readonly<{
    id: MulletSensitivityAxisId
    label: string
    assumptionId: string
    low: number
    high: number
  }>
> = [
  {
    id: 'acceptance_rate',
    label: 'Acceptance rate',
    assumptionId: 'work.acceptanceRate',
    low: 0.8,
    high: 1.2,
  },
  {
    id: 'demand_fill',
    label: 'Demand fill',
    assumptionId: 'work.demandFillPercent',
    low: 0.75,
    high: 1.25,
  },
  {
    id: 'electricity_price',
    label: 'Electricity price',
    assumptionId: 'power.electricityUsdPerMwh',
    low: 0.75,
    high: 1.35,
  },
  {
    id: 'hashprice',
    label: 'Hashprice',
    assumptionId: 'mining.hashpriceUsdPerMwh',
    low: 0.75,
    high: 1.35,
  },
  {
    id: 'raw_gpu_rate',
    label: 'Raw GPU rate',
    assumptionId: 'hardware.rawGpuRateUsdPerGpuHour',
    low: 0.7,
    high: 1.4,
  },
  {
    id: 'review_cost',
    label: 'Review cost',
    assumptionId: 'work.humanReviewHourlyUsd',
    low: 0.65,
    high: 1.5,
  },
  {
    id: 'sla_reserve',
    label: 'SLA reserve',
    assumptionId: 'power.slaReserveUsdPerMwh',
    low: 0.5,
    high: 1.75,
  },
  {
    id: 'provider_minimum_bid',
    label: 'Provider minimum bid',
    assumptionId: 'provider.minimumBidUsdPerHour',
    low: 0.7,
    high: 1.45,
  },
]

export const sensitivityAxisOptions = sensitivityAxes.map(axis => ({
  id: axis.id,
  label: axis.label,
}))

const sensitivityRows = (input: {
  readonly selectedTemplateId: MulletScenarioTemplateId
  readonly selectedSensitivityAxisId: MulletSensitivityAxisId
  readonly assumptions: ReadonlyArray<MulletAssumption>
}): ReadonlyArray<Record<string, string>> => {
  const base = simulateAssumptions(input.selectedTemplateId, input.assumptions)

  return sensitivityAxes.map(axis => {
    const low = simulateAssumptions(
      input.selectedTemplateId,
      scaleAssumption(input.assumptions, axis.assumptionId, axis.low),
    )
    const high = simulateAssumptions(
      input.selectedTemplateId,
      scaleAssumption(input.assumptions, axis.assumptionId, axis.high),
    )
    const flip =
      low.dispatch.selectedMode !== base.dispatch.selectedMode ||
      high.dispatch.selectedMode !== base.dispatch.selectedMode

    return {
      axis: axis.label,
      focus: axis.id === input.selectedSensitivityAxisId ? 'Focused' : '',
      low: modeLabel(low.dispatch.selectedMode),
      high: modeLabel(high.dispatch.selectedMode),
      netRange: `${dollarsPerMwh(
        Number(low.selectedCandidate.riskAdjustedNetUsdPerMwh),
      )} to ${dollarsPerMwh(
        Number(high.selectedCandidate.riskAdjustedNetUsdPerMwh),
      )}`,
      decision: flip ? 'Decision flip' : 'Same selected mode',
    }
  })
}

const scaleAssumption = (
  assumptions: ReadonlyArray<MulletAssumption>,
  assumptionId: string,
  multiplier: number,
): ReadonlyArray<MulletAssumption> =>
  assumptions.map(assumption =>
    assumption.id !== assumptionId
      ? assumption
      : {
          ...assumption,
          draftValue: String(
            roundTo(
              Math.max(0, numberFromAssumptions(assumptions, assumptionId) * multiplier),
              6,
            ),
          ),
        },
  )

const evidenceRows = (): ReadonlyArray<Record<string, string>> => [
  {
    boundary: 'Measured energy',
    state: 'Missing measured energy',
    action: 'Attach meter or telemetry records before settlement.',
  },
  {
    boundary: 'Accepted-work demand',
    state: 'Missing accepted-work demand',
    action: 'Import customer backlog or signed demand evidence.',
  },
  {
    boundary: 'Settlement evidence',
    state: 'Missing settlement evidence',
    action: 'No payout or Bitcoin settlement receipt attached.',
  },
  {
    boundary: 'Margot baseline import',
    state: 'Missing Margot baseline import',
    action: 'Attach the Margot reference sheet before presenting facility claims.',
  },
  {
    boundary: 'Readiness proof',
    state: 'Missing readiness proof',
    action: 'Require benchmark, workload-fit, and schedulability proof.',
  },
  {
    boundary: 'Payout proof',
    state: 'Missing payout proof',
    action: 'Provider payout proof is not present in this simulation.',
  },
]

export const assumptionGroups = (
  assumptions: ReadonlyArray<MulletAssumption>,
): ReadonlyArray<
  Readonly<{ group: MulletAssumptionGroup; assumptions: ReadonlyArray<MulletAssumption> }>
> => {
  const groups: readonly MulletAssumptionGroup[] = [
    'facility',
    'power',
    'mining fleet',
    'hardware',
    'work class',
    'provider floor',
    'party split',
    'capital',
  ]

  return groups.map(group => ({
    group,
    assumptions: assumptions.filter(assumption => assumption.group === group),
  }))
}

export const dollars = (value: number): string =>
  value.toLocaleString('en-US', {
    currency: 'USD',
    maximumFractionDigits: Math.abs(value) >= 1000 ? 0 : 2,
    minimumFractionDigits: 0,
    style: 'currency',
  })

export const dollarsPerMwh = (value: number): string => `${dollars(value)}/MWh`
