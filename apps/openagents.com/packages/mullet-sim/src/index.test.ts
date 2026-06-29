import { decodeMulletScenario } from '@openagentsinc/mullet-schema'
import { describe, expect, test } from 'vitest'

import {
  MARGOT_STYLE_100MW_80_20_BASELINE,
  MODEL1_MEDIUM_CODING_MISSION,
  MODEL1_SMALL_CODING_PATCH,
  calculateAcceptedOutcomeEconomics,
  calculateCapitalReturnSummary,
  calculateMargotStyleFacilityBaseline,
  candidateMode,
  consumerProviderMinimumBidUsd,
  minerProviderMinimumBidUsd,
  selectDispatchCandidate,
  shcReservedFractionalRuntimeCostUsd,
  shcReservedHourlyCostUsd,
  simulateScenarioHour,
} from './index'

const timestamp = '2026-06-08T00:00:00.000Z'

const sourceRef = {
  id: 'source_mullet_sim_fixture',
  label: 'Mullet simulation fixture',
  uri: 'packages/mullet-sim/src/index.test.ts',
  capturedAt: timestamp,
}

const provenancedNumber = (value: number, unit: string) => ({
  value,
  unit,
  provenance: 'modeled',
  confidence: 0.72,
  source: sourceRef,
  lastUpdated: timestamp,
  needsDiligence: true,
})

const provenancedString = (value: string) => ({
  value,
  unit: 'text',
  provenance: 'modeled',
  confidence: 0.72,
  source: sourceRef,
  lastUpdated: timestamp,
  needsDiligence: true,
})

const physicalReadiness = () => ({
  interconnectionStatus: provenancedString('fixture'),
  energizedCapacityMw: provenancedNumber(1, 'megawatts'),
  transformerStatus: provenancedString('fixture'),
  switchgearStatus: provenancedString('fixture'),
  coolingCapacityKw: provenancedNumber(20, 'kilowatts'),
  pue: provenancedNumber(1.2, 'score'),
  fiberOrNetworkStatus: provenancedString('fixture'),
  redundancyTier: provenancedString('fixture'),
  remoteHandsSla: provenancedString('fixture'),
  uptimeCommitment: provenancedNumber(0.95, 'percent'),
  liquidatedDamagesExposureUsd: provenancedNumber(0, 'usd'),
  permittingOrCommunityRisk: provenancedString('fixture'),
})

const scenario = (
  overrides: {
    readonly acceptedBacklog?: number
    readonly workBacklog?: number
    readonly providerMinimumBidUsdPerHour?: number
    readonly readinessState?: string
    readonly eligibleNodeTypes?: readonly string[]
  } = {},
) => ({
  id: 'mullet_scenario_dispatch_fixture',
  name: 'Dispatch fixture',
  schemaVersion: '2026-06-08.v1',
  kind: 'tinybox_west_texas_power',
  createdAt: timestamp,
  updatedAt: timestamp,
  dateRange: {
    startAt: timestamp,
    endAt: '2026-06-09T00:00:00.000Z',
  },
  facility: {
    id: 'facility_dispatch_fixture',
    siteId: 'site_dispatch_fixture',
    name: 'Dispatch fixture site',
    market: 'ERCOT',
    zone: 'LZ_WEST',
    capacityMw: provenancedNumber(1, 'megawatts'),
    powerContractType: 'fixed_fixture',
    fixedPriceUsdPerMwh: provenancedNumber(45, 'usd_per_mwh'),
    maxAiAllocationMw: provenancedNumber(0.002, 'megawatts'),
    curtailmentPolicy: 'fixture',
    gridServiceTerms: 'fixture',
    siteOpsCostUsdPerMwh: provenancedNumber(5, 'usd_per_mwh'),
    coolingMultiplier: provenancedNumber(1.1, 'score'),
    remoteHandsMonthlyUsd: provenancedNumber(100, 'usd'),
    physicalReadiness: physicalReadiness(),
    customerSlaReserveUsdPerMwh: provenancedNumber(25, 'usd_per_mwh'),
    siteClassification: 'mullet_ai_led_mining_backfill',
    readinessState: 'benchmark_passed',
    capacityLifecycleState: 'eligible',
  },
  miningFleet: {
    asicModel: 'S21 fixture',
    count: 286,
    wattsPerUnit: 3500,
    thPerUnit: 200,
    joulesPerTh: 17.5,
    capexPerUnitUsd: 4000,
    depreciationMonths: 36,
    poolFeePercent: 0,
    firmwareOpsCostUsdPerMwh: 2,
  },
  computeNodes: [
    {
      nodeId: 'node_dispatch_fixture',
      nodeType: 'tinybox_red_v2',
      ownerParty: 'openagents',
      operatorParty: 'fixture_provider',
      siteId: 'site_dispatch_fixture',
      gpuModel: 'AMD 9070 XT',
      gpuCount: 4,
      vramGb: 64,
      interconnect: 'pcie',
      cpu: 'fixture',
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
      readinessState: overrides.readinessState ?? 'benchmark_passed',
      capacityLifecycleState: 'admitted',
      workloadFit: ['work_class_dispatch_fixture'],
    },
  ],
  runtimeBenchmarks: [
    {
      nodeId: 'node_dispatch_fixture',
      workClassId: 'work_class_dispatch_fixture',
      modelId: 'fixture-local-model',
      framework: 'fixture',
      precision: 'fp16',
      batchSize: 1,
      attemptsPerInstanceHour: 12,
      tokensPerSecond: 80,
      joulesPerToken: 10,
      kwhPerAttempt: 0.08,
      wallSecondsPerAttempt: 300,
      observedFailureRate: 0.08,
      source: sourceRef,
      confidence: 0.72,
    },
  ],
  workClasses: [
    {
      id: 'work_class_dispatch_fixture',
      label: 'Dispatch fixture work',
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
      demandBacklog: overrides.workBacklog ?? 100,
      eligibleNodeTypes: [
        ...(overrides.eligibleNodeTypes ?? ['tinybox_red_v2']),
      ],
      minimumTrustTier: 'pilot',
    },
  ],
  providerPolicies: [
    {
      nodeType: 'tinybox_red_v2',
      providerMinimumBidUsdPerHour: overrides.providerMinimumBidUsdPerHour ?? 1,
      providerMinimumBidUsdPerJob: 0.1,
      wearUsdPerKwh: 0.05,
      bandwidthUsdPerJob: 0.01,
      desiredProfitUsdPerJob: 0.1,
      operatorMarginUsdPerMwh: 25,
      miningFloorPolicy: 'modeled',
      rawGpuFloorPolicy: 'modeled',
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
    acceptedWorkBacklog: provenancedNumber(
      overrides.acceptedBacklog ?? 100,
      'outcomes',
    ),
    demandFillPercent: provenancedNumber(0.55, 'percent'),
    rawGpuMarketFillPercent: provenancedNumber(0.35, 'percent'),
    tokenApiFillPercent: provenancedNumber(0.1, 'percent'),
  },
  sourceRefs: [sourceRef],
})

describe('MODEL1 accepted-outcome economics', () => {
  test('reproduces the small coding patch fixture', () => {
    const result = calculateAcceptedOutcomeEconomics(MODEL1_SMALL_CODING_PATCH)

    expect(result.frontierModelCostUsd).toBe(4.5)
    expect(result.workroomRuntimeCostUsd).toBe(0.4)
    expect(result.validatorPayoutsUsd).toBe(0.75)
    expect(result.humanReviewCostUsd).toBe(15)
    expect(result.costPerAttemptUsd).toBe(26.25)
    expect(result.costPerAcceptedOutcomeUsd).toBe(37.5)
    expect(result.buyerPriceUsd).toBe(60.63)
  })

  test('reproduces the medium coding mission fixture', () => {
    const result = calculateAcceptedOutcomeEconomics(
      MODEL1_MEDIUM_CODING_MISSION,
    )

    expect(result.frontierModelCostUsd).toBe(21)
    expect(result.workroomRuntimeCostUsd).toBe(1.8)
    expect(result.providerExecutionCostUsd).toBe(3)
    expect(result.validatorPayoutsUsd).toBe(2.5)
    expect(result.humanReviewCostUsd).toBe(50)
    expect(result.costPerAttemptUsd).toBe(95.55)
    expect(result.costPerAcceptedOutcomeUsd).toBe(159.25)
    expect(result.buyerPriceUsd).toBe(272.95)
  })

  test('models decentralized validation and SHC reserved capacity examples', () => {
    const providerBid = consumerProviderMinimumBidUsd({
      averagePowerKw: 0.15,
      runtimeHours: 1,
      electricityUsdPerKwh: 0.15,
      hardwareWearUsd: 0.02,
      bandwidthUsd: 0.01,
      desiredProfitUsd: 0.2,
    })

    expect(providerBid).toBeCloseTo(0.2525, 4)
    expect(providerBid + 0.05).toBeCloseTo(0.3025, 4)
    expect(shcReservedHourlyCostUsd(1031.79, 1)).toBeCloseTo(0.1178, 4)
    expect(shcReservedHourlyCostUsd(1031.79, 0.5)).toBeCloseTo(0.2356, 4)
    expect(
      shcReservedFractionalRuntimeCostUsd({
        annualCostUsd: 1031.79,
        utilizationPercent: 0.5,
        allocationPercent: 0.25,
        runtimeHours: 4,
        supportMultiplier: 2,
      }),
    ).toBeCloseTo(0.4711, 4)
  })

  test('models miner opportunity floor per job', () => {
    expect(
      minerProviderMinimumBidUsd({
        jobKwh: 1,
        powerCostUsdPerKwh: 0.045,
        wearUsdPerKwh: 0.005,
        operatorMarginUsdPerKwh: 0.01,
        miningFloorUsdPerKwh: 0.08,
        vpsOrColoOpportunityUsdPerKwh: 0.05,
      }),
    ).toBe(0.08)
  })
})

describe('Margot-style frozen baselines', () => {
  test('reproduces the 100 MW 80/20 mining plus raw-AI rental fixture', () => {
    const baseline = calculateMargotStyleFacilityBaseline(
      MARGOT_STYLE_100MW_80_20_BASELINE,
    )

    expect(baseline.miningMw).toBe(80)
    expect(baseline.aiMw).toBe(20)
    expect(baseline.aiGpuCount).toBeCloseTo(12714.56, 2)
    expect(baseline.miningRevenueUsdPerHour).toBe(7360)
    expect(baseline.miningProfitUsdPerHour).toBe(3212.8)
    expect(baseline.rawAiRevenueUsdPerHour).toBeCloseTo(69037, 2)
    expect(baseline.rawAiRevenueUsdPerMwh).toBeCloseTo(3451.85, 2)
    expect(baseline.combinedGrossRevenueUsdPerHour).toBeCloseTo(76397, 2)
    expect(baseline.combinedProfitBeforeCapexUsdPerHour).toBeCloseTo(71249.8, 2)
  })
})

describe('dispatch selection', () => {
  test('selects the highest eligible candidate and keeps reason codes explicit', () => {
    const selected = selectDispatchCandidate([
      candidateMode({
        timestamp,
        mode: 'mine',
        buyerRevenueUsd: 100,
        providerPayoutUsd: 0,
        openagentsMarginUsd: 45,
        providerNetUsdPerMwh: 0,
        acceptedOutcomes: 0,
        acceptedOutcomesPerMwh: 0,
        energyMwh: 1,
        riskAdjustedNetUsdPerMwh: 45,
        clearsReadiness: true,
        clearsDemand: true,
        clearsProviderFloor: true,
        reasonCode: 'mining_best_available',
      }),
      candidateMode({
        timestamp,
        mode: 'openagents_accepted_work',
        buyerRevenueUsd: 400,
        providerPayoutUsd: 100,
        openagentsMarginUsd: 300,
        providerNetUsdPerMwh: 100,
        acceptedOutcomes: 10,
        acceptedOutcomesPerMwh: 10,
        energyMwh: 1,
        riskAdjustedNetUsdPerMwh: 300,
        clearsReadiness: true,
        clearsDemand: false,
        clearsProviderFloor: true,
        reasonCode: 'accepted_work_blocked_no_backlog',
      }),
    ])

    expect(selected.mode).toBe('mine')
    expect(selected.reasonCode).toBe('mining_best_available')
  })

  test('chooses mining when accepted-work demand is zero', () => {
    const result = simulateScenarioHour(
      decodeMulletScenario(scenario({ acceptedBacklog: 0 })),
      {
        miningRevenueUsdPerMwh: 100,
      },
    )

    expect(result.selectedMode).toBe('mine')
    expect(result.reasonCode).toBe('mining_best_available')
    expect(
      result.candidates.find(candidate => {
        return candidate.mode === 'openagents_accepted_work'
      })?.reasonCode,
    ).toBe('accepted_work_blocked_no_backlog')
  })

  test('chooses accepted work when backlog, eligibility, margin, readiness, and provider floor clear', () => {
    const result = simulateScenarioHour(decodeMulletScenario(scenario()), {
      miningRevenueUsdPerMwh: 50,
    })

    expect(result.selectedMode).toBe('openagents_accepted_work')
    expect(result.reasonCode).toBe('accepted_work_clears_all_gates')
  })

  test('blocks accepted work when eligibility, readiness, or provider floors fail', () => {
    expect(
      simulateScenarioHour(
        decodeMulletScenario(
          scenario({ eligibleNodeTypes: ['different_node_type'] }),
        ),
        { miningRevenueUsdPerMwh: 90 },
      ).reasonCode,
    ).toBe('mining_best_available')
    expect(
      simulateScenarioHour(
        decodeMulletScenario(scenario({ readinessState: 'inventory_known' })),
        {
          miningRevenueUsdPerMwh: 90,
        },
      ).reasonCode,
    ).toBe('mining_best_available')
    expect(
      simulateScenarioHour(
        decodeMulletScenario(
          scenario({ providerMinimumBidUsdPerHour: 10_000 }),
        ),
        { miningRevenueUsdPerMwh: 90 },
      ).reasonCode,
    ).toBe('mining_best_available')
  })
})

describe('capital returns', () => {
  test('summarizes party-specific capital returns without side effects', () => {
    const summary = calculateCapitalReturnSummary({
      party: 'hardware_owner',
      grossRevenueUsd: 6000,
      cogsUsd: 2200,
      capexUsd: 12000,
      monthlyNetUsd: 1000,
      months: 36,
      annualDiscountRatePercent: 12,
      residualValueUsd: 4000,
      downsideProtectionUsd: 1000,
    })

    expect(summary.party).toBe('hardware_owner')
    expect(summary.grossMarginUsd).toBe(3800)
    expect(summary.paybackMonths).toBe(12)
    expect(summary.downsideProtectionUsd).toBe(1000)
  })
})
