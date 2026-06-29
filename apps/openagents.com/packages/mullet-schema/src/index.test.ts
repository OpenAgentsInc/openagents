import { describe, expect, test } from 'vitest'

import {
  KWh,
  MWh,
  type MulletUnitLabel,
  decodeMulletScenario,
  decodeMulletScenarioJson,
  encodeMulletScenarioJson,
} from './index'

const timestamp = '2026-06-08T00:00:00.000Z'

const sourceRef = {
  id: 'source_combined_model_plan',
  label: 'Combined mining AI model plan',
  uri: 'docs/mullet/2026-06-08-omega-unified-mullet-simulation-runner-audit.md',
  capturedAt: timestamp,
}

const provenancedNumber = (
  value: number,
  unit: MulletUnitLabel,
  notes?: string,
) => ({
  value,
  unit,
  provenance: 'modeled',
  confidence: 0.72,
  source: sourceRef,
  lastUpdated: timestamp,
  needsDiligence: true,
  ...(notes === undefined ? {} : { notes }),
})

const provenancedString = (
  value: string,
  notes?: string,
) => ({
  value,
  unit: 'text',
  provenance: 'modeled',
  confidence: 0.72,
  source: sourceRef,
  lastUpdated: timestamp,
  needsDiligence: true,
  ...(notes === undefined ? {} : { notes }),
})

const physicalReadiness = () => ({
  interconnectionStatus: provenancedString('operator reported'),
  energizedCapacityMw: provenancedNumber(1, 'megawatts'),
  transformerStatus: provenancedString('unknown'),
  switchgearStatus: provenancedString('unknown'),
  coolingCapacityKw: provenancedNumber(20, 'kilowatts'),
  pue: provenancedNumber(1.35, 'score'),
  fiberOrNetworkStatus: provenancedString('commodity broadband'),
  redundancyTier: provenancedString('pilot'),
  remoteHandsSla: provenancedString('best effort'),
  uptimeCommitment: provenancedNumber(0.95, 'percent'),
  liquidatedDamagesExposureUsd: provenancedNumber(0, 'usd'),
  permittingOrCommunityRisk: provenancedString('none for office pilot'),
})

const tinyboxScenario = () => ({
  id: 'mullet_scenario_tinybox_shc',
  name: 'Tinybox at SHC-style power',
  schemaVersion: '2026-06-08.v1',
  kind: 'tinybox_shc_power',
  createdAt: timestamp,
  updatedAt: timestamp,
  dateRange: {
    startAt: timestamp,
    endAt: '2026-06-09T00:00:00.000Z',
  },
  facility: {
    id: 'facility_shc_pilot',
    siteId: 'site_shc',
    name: 'SHC pilot site',
    market: 'ERCOT',
    zone: 'LZ_WEST',
    capacityMw: provenancedNumber(1, 'megawatts'),
    powerContractType: 'fixed_operator_reported',
    fixedPriceUsdPerMwh: provenancedNumber(45, 'usd_per_mwh'),
    maxAiAllocationMw: provenancedNumber(0.002, 'megawatts'),
    curtailmentPolicy: 'operator manual curtailment',
    gridServiceTerms: 'not enrolled',
    siteOpsCostUsdPerMwh: provenancedNumber(5, 'usd_per_mwh'),
    coolingMultiplier: provenancedNumber(1.15, 'score'),
    remoteHandsMonthlyUsd: provenancedNumber(250, 'usd'),
    physicalReadiness: physicalReadiness(),
    customerSlaReserveUsdPerMwh: provenancedNumber(25, 'usd_per_mwh'),
    siteClassification: 'mining_led_ai_pilot_not_mullet',
    readinessState: 'inventory_known',
    capacityLifecycleState: 'eligible',
  },
  miningFleet: {
    asicModel: 'S21',
    count: 0,
    wattsPerUnit: 3500,
    thPerUnit: 200,
    joulesPerTh: 17.5,
    capexPerUnitUsd: 4000,
    depreciationMonths: 36,
    poolFeePercent: 0.02,
    firmwareOpsCostUsdPerMwh: 2,
  },
  computeNodes: [
    {
      nodeId: 'node_tinybox_red_v2',
      nodeType: 'tinybox_red_v2',
      ownerParty: 'openagents',
      operatorParty: 'shc',
      siteId: 'site_shc',
      gpuModel: 'AMD 9070 XT',
      gpuCount: 4,
      vramGb: 64,
      interconnect: 'pcie',
      cpu: 'AMD EPYC',
      ramGb: 128,
      storageGb: 2000,
      networkGbps: 1,
      capexUsd: 12000,
      depreciationMonths: 36,
      idlePowerKw: 0.35,
      loadPowerKw: 1.6,
      powerLimitKw: 1.6,
      supportMonthlyUsd: 150,
      fallbackMarketEligible: true,
      trustTier: 'pilot',
      readinessState: 'benchmark_passed',
      capacityLifecycleState: 'admitted',
      workloadFit: ['work_class_artifact_validation'],
    },
  ],
  runtimeBenchmarks: [
    {
      nodeId: 'node_tinybox_red_v2',
      workClassId: 'work_class_artifact_validation',
      modelId: 'local-small-model',
      framework: 'tinygrad',
      precision: 'fp16',
      batchSize: 1,
      attemptsPerInstanceHour: 12,
      tokensPerSecond: 80,
      joulesPerToken: 10,
      kwhPerAttempt: 0.08,
      wallSecondsPerAttempt: 300,
      observedFailureRate: 0.08,
      source: sourceRef,
      confidence: 0.5,
    },
  ],
  workClasses: [
    {
      id: 'work_class_artifact_validation',
      label: 'Artifact validation',
      latencyClass: 'batch',
      buyerPriceUsd: 2.5,
      acceptanceRate: 0.9,
      targetMargin: 0.25,
      riskReserveUsd: 0.15,
      frontierInputTokens: 1000,
      frontierOutputTokens: 200,
      frontierInputPriceUsdPerMillion: 1.25,
      frontierOutputPriceUsdPerMillion: 10,
      cheapModelCostUsd: 0.05,
      workroomRuntimeHours: 0.1,
      workroomHourlyCostUsd: 0.25,
      providerComputeHours: 0.1,
      providerPowerKw: 1.6,
      validatorCount: 2,
      validatorPayoutEachUsd: 0.05,
      graderCostUsd: 0.05,
      humanReviewMinutes: 0,
      humanReviewHourlyCostUsd: 60,
      artifactStorageCostUsd: 0.01,
      settlementCostUsd: 0.01,
      supportOverheadUsd: 0.03,
      retryOrFailureCostUsd: 0.04,
      flexibility: {
        canPause: true,
        canResume: true,
        canMigrate: true,
        checkpointIntervalMinutes: 10,
        maxDelayMinutes: 1440,
        deadlineMinutes: 1440,
        customerImpactIfDelayed: 'low',
        privacyTier: 'public_safe',
        stateLocality: 'portable',
        requiredTools: ['artifact-store'],
      },
      demandBacklog: 100,
      eligibleNodeTypes: ['tinybox_red_v2'],
      minimumTrustTier: 'pilot',
    },
  ],
  providerPolicies: [
    {
      nodeType: 'tinybox_red_v2',
      providerMinimumBidUsdPerHour: 1,
      providerMinimumBidUsdPerJob: 0.1,
      wearUsdPerKwh: 0.05,
      bandwidthUsdPerJob: 0.01,
      desiredProfitUsdPerJob: 0.1,
      operatorMarginUsdPerMwh: 25,
      miningFloorPolicy: 'none',
      rawGpuFloorPolicy: 'vast_floor',
      vpsColocationFloorPolicy: 'not_applicable',
      curtailmentGridServiceFloorPolicy: 'none',
    },
  ],
  partySplit: {
    buyerPaysUsd: 2.5,
    openagentsKeepsUsd: 0.7,
    providerReceivesUsd: 1.2,
    facilityOperatorReceivesUsd: 0.2,
    hardwareOwnerReceivesUsd: 0.3,
    validatorsReceiveUsd: 0.1,
    reviewersReceiveUsd: 0,
    settlementCostUsd: 0.01,
    riskReserveUsd: 0.15,
  },
  capitalAssumptions: {
    hardwareCapexUsd: provenancedNumber(12000, 'usd'),
    depreciationMonths: provenancedNumber(36, 'months'),
    discountRatePercent: provenancedNumber(12, 'percent'),
    debtServiceMonthlyUsd: provenancedNumber(0, 'usd'),
    residualValueUsd: provenancedNumber(4000, 'usd'),
  },
  demandAssumptions: {
    acceptedWorkBacklog: provenancedNumber(100, 'outcomes'),
    demandFillPercent: provenancedNumber(0.55, 'percent'),
    rawGpuMarketFillPercent: provenancedNumber(0.35, 'percent'),
    tokenApiFillPercent: provenancedNumber(0.1, 'percent'),
  },
  sourceRefs: [sourceRef],
  notes: 'First pilot shell fixture.',
})

const hundredMwScenario = () => {
  const base = tinyboxScenario()
  const node = base.computeNodes[0]

  if (node === undefined) {
    throw new Error('Tinybox fixture missing node')
  }

  return {
    ...base,
    id: 'mullet_scenario_100mw_80_20',
    name: '100 MW 80 percent mining 20 percent AI',
    kind: 'facility_100mw_80_20',
    facility: {
      ...base.facility,
      id: 'facility_100mw',
      siteId: 'site_100mw',
      name: '100 MW facility',
      capacityMw: provenancedNumber(100, 'megawatts'),
      maxAiAllocationMw: provenancedNumber(20, 'megawatts'),
      siteClassification: 'balanced_hybrid',
    },
    miningFleet: {
      asicModel: 'S21',
      count: 22857,
      wattsPerUnit: 3500,
      thPerUnit: 200,
      joulesPerTh: 17.5,
      capexPerUnitUsd: 4000,
      depreciationMonths: 36,
      poolFeePercent: 0.02,
      firmwareOpsCostUsdPerMwh: 2,
    },
    computeNodes: [
      {
        ...node,
        nodeId: 'node_gpu_island_20mw',
        nodeType: 'miner_gpu_island',
        siteId: 'site_100mw',
        gpuModel: 'B300 proxy',
        gpuCount: 12712,
        vramGb: 3_052_800,
        loadPowerKw: 20000,
        powerLimitKw: 20000,
        workloadFit: ['work_class_batch_inference'],
      },
    ],
  }
}

describe('mullet schema package', () => {
  test('decodes a Tinybox scenario shell', () => {
    const scenario = decodeMulletScenario(tinyboxScenario())

    expect(scenario.name).toBe('Tinybox at SHC-style power')
    expect(scenario.facility.capacityMw.provenance).toBe('modeled')
    expect(scenario.computeNodes[0]?.nodeType).toBe('tinybox_red_v2')
    expect(scenario.workClasses[0]?.flexibility.canPause).toBe(true)
  })

  test('decodes a 100 MW 80/20 scenario shell', () => {
    const scenario = decodeMulletScenario(hundredMwScenario())

    expect(scenario.kind).toBe('facility_100mw_80_20')
    expect(scenario.facility.capacityMw.value).toBe(100)
    expect(scenario.facility.maxAiAllocationMw.value).toBe(20)
    expect(scenario.miningFleet.count).toBe(22857)
  })

  test('round-trips scenario JSON through Effect Schema', () => {
    const scenario = decodeMulletScenario(tinyboxScenario())
    const json = encodeMulletScenarioJson(scenario)
    const decoded = decodeMulletScenarioJson(json)

    expect(decoded).toEqual(scenario)
  })

  test('rejects scenario inputs missing provenance metadata', () => {
    const scenario = tinyboxScenario()
    const invalid = {
      ...scenario,
      facility: {
        ...scenario.facility,
        capacityMw: {
          value: 1,
          unit: 'megawatts',
        },
      },
    }

    expect(() => decodeMulletScenario(invalid)).toThrow()
  })

  test('exposes distinct energy unit constructors', () => {
    const kwh = KWh.make(1)
    const mwh = MWh.make(2)

    expect(kwh).toBe(1)
    expect(mwh).toBe(2)
  })
})
