import {
  Confidence,
  IsoTimestamp,
  MWh,
  Megawatts,
  type MulletCapitalReturnSummary,
  type MulletComputeNode,
  type MulletDispatchMode,
  type MulletFacility,
  type MulletHourlyCandidateMode,
  type MulletHourlyDispatchResult,
  type MulletMiningFleet,
  type MulletProviderBidPolicy,
  type MulletScenario,
  type MulletWorkClass,
  Percent,
  Usd,
  UsdPerMWh,
} from '@openagentsinc/mullet-schema'

export type ReasonCode =
  | 'accepted_work_clears_all_gates'
  | 'accepted_work_blocked_no_backlog'
  | 'accepted_work_blocked_not_eligible'
  | 'accepted_work_blocked_not_ready'
  | 'accepted_work_blocked_negative_margin'
  | 'accepted_work_blocked_provider_floor'
  | 'mining_best_available'
  | 'raw_gpu_market_best_available'
  | 'token_api_best_available'
  | 'curtailment_best_available'
  | 'idle_best_available'
  | 'reserve_best_available'

export interface FrontierModelCostInput {
  readonly inputTokens: number
  readonly outputTokens: number
  readonly inputUsdPerMillion: number
  readonly outputUsdPerMillion: number
}

export interface AcceptedOutcomeEconomicsInput extends FrontierModelCostInput {
  readonly cheapModelCostUsd?: number
  readonly workroomRuntimeHours: number
  readonly workroomHourlyCostUsd: number
  readonly providerExecutionJobs?: number
  readonly providerPayoutEachUsd?: number
  readonly validatorCount: number
  readonly validatorPayoutEachUsd: number
  readonly graderCostUsd?: number
  readonly humanReviewMinutes: number
  readonly humanReviewHourlyCostUsd: number
  readonly artifactStorageCostUsd: number
  readonly settlementCostUsd: number
  readonly retryOrFailureCostUsd?: number
  readonly supportOverheadUsd: number
  readonly acceptanceRate: number
  readonly targetMargin: number
  readonly riskReserveUsd: number
}

export interface AcceptedOutcomeEconomics {
  readonly frontierModelCostUsd: number
  readonly cheapModelCostUsd: number
  readonly workroomRuntimeCostUsd: number
  readonly providerExecutionCostUsd: number
  readonly validatorPayoutsUsd: number
  readonly graderCostUsd: number
  readonly humanReviewCostUsd: number
  readonly artifactStorageCostUsd: number
  readonly settlementCostUsd: number
  readonly retryOrFailureCostUsd: number
  readonly supportOverheadUsd: number
  readonly costPerAttemptUsd: number
  readonly costPerAcceptedOutcomeUsd: number
  readonly buyerPriceUsd: number
}

export interface ConsumerProviderBidInput {
  readonly averagePowerKw: number
  readonly runtimeHours: number
  readonly electricityUsdPerKwh: number
  readonly hardwareWearUsd: number
  readonly bandwidthUsd: number
  readonly inconvenienceReserveUsd?: number
  readonly desiredProfitUsd: number
}

export interface MinerProviderFloorInput {
  readonly jobKwh: number
  readonly powerCostUsdPerKwh: number
  readonly wearUsdPerKwh: number
  readonly operatorMarginUsdPerKwh: number
  readonly miningFloorUsdPerKwh: number
  readonly vpsOrColoOpportunityUsdPerKwh: number
}

export interface RuntimeCapacityInput {
  readonly loadPowerKw: number
  readonly attemptsPerInstanceHour: number
  readonly acceptanceRate: number
  readonly demandFillPercent: number
}

export interface RuntimeCapacity {
  readonly energyMwhPerHour: number
  readonly attemptsPerHour: number
  readonly acceptedOutcomesPerHour: number
  readonly acceptedOutcomesPerMwh: number
}

export interface MiningHourInput {
  readonly miningMw: number
  readonly revenuePerMwh: number
  readonly electricityUsdPerMwh: number
  readonly opsUsdPerMwh: number
  readonly poolFeePercent?: number
}

export interface MiningHourEconomics {
  readonly energyMwh: number
  readonly grossRevenueUsd: number
  readonly poolFeeUsd: number
  readonly energyCostUsd: number
  readonly opsCostUsd: number
  readonly profitUsd: number
  readonly profitUsdPerMwh: number
}

export interface MargotStyleFacilityInput {
  readonly facilityMw: number
  readonly miningAllocationPercent: number
  readonly aiAllocationPercent: number
  readonly miningRevenueUsdPerMwh: number
  readonly electricityUsdPerMwh: number
  readonly siteOpsUsdPerMwh: number
  readonly poolFeePercent: number
  readonly aiSystemPowerKw: number
  readonly aiGpuCountPerNode: number
  readonly aiGpuRentalUsdPerHour: number
  readonly aiUtilizationPercent: number
}

export interface MargotStyleFacilityBaseline {
  readonly miningMw: number
  readonly aiMw: number
  readonly aiGpuCount: number
  readonly miningRevenueUsdPerHour: number
  readonly miningProfitUsdPerHour: number
  readonly rawAiRevenueUsdPerHour: number
  readonly rawAiRevenueUsdPerMwh: number
  readonly combinedGrossRevenueUsdPerHour: number
  readonly combinedProfitBeforeCapexUsdPerHour: number
}

export interface CandidateModeInput {
  readonly timestamp: string
  readonly mode: MulletDispatchMode
  readonly buyerRevenueUsd: number
  readonly providerPayoutUsd: number
  readonly openagentsMarginUsd: number
  readonly providerNetUsdPerMwh: number
  readonly acceptedOutcomes: number
  readonly acceptedOutcomesPerMwh: number
  readonly energyMwh: number
  readonly riskAdjustedNetUsdPerMwh: number
  readonly clearsReadiness: boolean
  readonly clearsDemand: boolean
  readonly clearsProviderFloor: boolean
  readonly reasonCode: ReasonCode
}

export interface ScenarioHourOptions {
  readonly timestamp?: string
  readonly effectivePriceUsdPerMwh?: number
  readonly miningRevenueUsdPerMwh?: number
  readonly rawGpuRevenueUsdPerHour?: number
  readonly tokenApiRevenueUsdPerHour?: number
  readonly curtailmentValueUsdPerMwh?: number
}

export interface CapitalReturnInput {
  readonly party: string
  readonly grossRevenueUsd: number
  readonly cogsUsd: number
  readonly capexUsd: number
  readonly monthlyNetUsd: number
  readonly months: number
  readonly annualDiscountRatePercent: number
  readonly residualValueUsd: number
  readonly downsideProtectionUsd?: number
}

export const MODEL1_SMALL_CODING_PATCH: AcceptedOutcomeEconomicsInput = {
  inputTokens: 1_000_000,
  outputTokens: 100_000,
  inputUsdPerMillion: 3,
  outputUsdPerMillion: 15,
  workroomRuntimeHours: 2,
  workroomHourlyCostUsd: 0.2,
  validatorCount: 3,
  validatorPayoutEachUsd: 0.25,
  humanReviewMinutes: 10,
  humanReviewHourlyCostUsd: 90,
  artifactStorageCostUsd: 0.5,
  settlementCostUsd: 0.1,
  supportOverheadUsd: 5,
  acceptanceRate: 0.7,
  riskReserveUsd: 10,
  targetMargin: 0.35,
}

export const MODEL1_MEDIUM_CODING_MISSION: AcceptedOutcomeEconomicsInput = {
  inputTokens: 5_000_000,
  outputTokens: 400_000,
  inputUsdPerMillion: 3,
  outputUsdPerMillion: 15,
  workroomRuntimeHours: 6,
  workroomHourlyCostUsd: 0.3,
  providerExecutionJobs: 4,
  providerPayoutEachUsd: 0.75,
  validatorCount: 5,
  validatorPayoutEachUsd: 0.5,
  humanReviewMinutes: 30,
  humanReviewHourlyCostUsd: 100,
  artifactStorageCostUsd: 2,
  settlementCostUsd: 0.25,
  supportOverheadUsd: 15,
  acceptanceRate: 0.6,
  riskReserveUsd: 50,
  targetMargin: 0.4,
}

export const MARGOT_STYLE_100MW_80_20_BASELINE: MargotStyleFacilityInput = {
  facilityMw: 100,
  miningAllocationPercent: 0.8,
  aiAllocationPercent: 0.2,
  miningRevenueUsdPerMwh: 92,
  electricityUsdPerMwh: 45,
  siteOpsUsdPerMwh: 5,
  poolFeePercent: 0.02,
  aiSystemPowerKw: 12.584,
  aiGpuCountPerNode: 8,
  aiGpuRentalUsdPerHour: 6.7872,
  aiUtilizationPercent: 0.8,
}

export const roundTo = (value: number, digits: number): number => {
  const scale = 10 ** digits

  return Math.round((value + Number.EPSILON) * scale) / scale
}

export const roundUsd = (value: number): number => roundTo(value, 2)

export const safeDivide = (numerator: number, denominator: number): number =>
  denominator === 0 ? 0 : numerator / denominator

export const frontierModelCostUsd = (input: FrontierModelCostInput): number =>
  (input.inputTokens / 1_000_000) * input.inputUsdPerMillion +
  (input.outputTokens / 1_000_000) * input.outputUsdPerMillion

export const humanReviewCostUsd = (
  minutes: number,
  hourlyCostUsd: number,
): number => (minutes / 60) * hourlyCostUsd

export const calculateAcceptedOutcomeEconomics = (
  input: AcceptedOutcomeEconomicsInput,
): AcceptedOutcomeEconomics => {
  const frontierCost = frontierModelCostUsd(input)
  const cheapModelCost = input.cheapModelCostUsd ?? 0
  const workroomRuntimeCost =
    input.workroomRuntimeHours * input.workroomHourlyCostUsd
  const providerExecutionCost =
    (input.providerExecutionJobs ?? 0) * (input.providerPayoutEachUsd ?? 0)
  const validatorPayouts = input.validatorCount * input.validatorPayoutEachUsd
  const graderCost = input.graderCostUsd ?? 0
  const reviewCost = humanReviewCostUsd(
    input.humanReviewMinutes,
    input.humanReviewHourlyCostUsd,
  )
  const retryOrFailureCost = input.retryOrFailureCostUsd ?? 0
  const costPerAttempt =
    frontierCost +
    cheapModelCost +
    workroomRuntimeCost +
    providerExecutionCost +
    validatorPayouts +
    graderCost +
    reviewCost +
    input.artifactStorageCostUsd +
    input.settlementCostUsd +
    retryOrFailureCost +
    input.supportOverheadUsd
  const costPerAcceptedOutcome = safeDivide(
    costPerAttempt,
    input.acceptanceRate,
  )
  const buyerPrice =
    costPerAcceptedOutcome * (1 + input.targetMargin) + input.riskReserveUsd

  return {
    frontierModelCostUsd: roundUsd(frontierCost),
    cheapModelCostUsd: roundUsd(cheapModelCost),
    workroomRuntimeCostUsd: roundUsd(workroomRuntimeCost),
    providerExecutionCostUsd: roundUsd(providerExecutionCost),
    validatorPayoutsUsd: roundUsd(validatorPayouts),
    graderCostUsd: roundUsd(graderCost),
    humanReviewCostUsd: roundUsd(reviewCost),
    artifactStorageCostUsd: roundUsd(input.artifactStorageCostUsd),
    settlementCostUsd: roundUsd(input.settlementCostUsd),
    retryOrFailureCostUsd: roundUsd(retryOrFailureCost),
    supportOverheadUsd: roundUsd(input.supportOverheadUsd),
    costPerAttemptUsd: roundUsd(costPerAttempt),
    costPerAcceptedOutcomeUsd: roundUsd(costPerAcceptedOutcome),
    buyerPriceUsd: roundUsd(buyerPrice),
  }
}

export const calculateWorkClassEconomics = (
  workClass: MulletWorkClass,
  providerPolicy?: MulletProviderBidPolicy,
): AcceptedOutcomeEconomics =>
  calculateAcceptedOutcomeEconomics({
    inputTokens: workClass.frontierInputTokens,
    outputTokens: workClass.frontierOutputTokens,
    inputUsdPerMillion: workClass.frontierInputPriceUsdPerMillion,
    outputUsdPerMillion: workClass.frontierOutputPriceUsdPerMillion,
    cheapModelCostUsd: workClass.cheapModelCostUsd,
    workroomRuntimeHours: workClass.workroomRuntimeHours,
    workroomHourlyCostUsd: workClass.workroomHourlyCostUsd,
    providerExecutionJobs:
      workClass.providerComputeHours > 0 && providerPolicy !== undefined
        ? 1
        : 0,
    providerPayoutEachUsd:
      providerPolicy === undefined
        ? 0
        : Math.max(
            providerPolicy.providerMinimumBidUsdPerJob,
            providerPolicy.providerMinimumBidUsdPerHour *
              workClass.providerComputeHours,
          ),
    validatorCount: workClass.validatorCount,
    validatorPayoutEachUsd: workClass.validatorPayoutEachUsd,
    graderCostUsd: workClass.graderCostUsd,
    humanReviewMinutes: workClass.humanReviewMinutes,
    humanReviewHourlyCostUsd: workClass.humanReviewHourlyCostUsd,
    artifactStorageCostUsd: workClass.artifactStorageCostUsd,
    settlementCostUsd: workClass.settlementCostUsd,
    retryOrFailureCostUsd: workClass.retryOrFailureCostUsd,
    supportOverheadUsd: workClass.supportOverheadUsd,
    acceptanceRate: workClass.acceptanceRate,
    targetMargin: workClass.targetMargin,
    riskReserveUsd: workClass.riskReserveUsd,
  })

export const consumerProviderMinimumBidUsd = (
  input: ConsumerProviderBidInput,
): number =>
  input.averagePowerKw * input.runtimeHours * input.electricityUsdPerKwh +
  input.hardwareWearUsd +
  input.bandwidthUsd +
  (input.inconvenienceReserveUsd ?? 0) +
  input.desiredProfitUsd

export const shcReservedHourlyCostUsd = (
  annualCostUsd: number,
  utilizationPercent: number,
): number => safeDivide(annualCostUsd, 8760) / utilizationPercent

export const shcReservedFractionalRuntimeCostUsd = (input: {
  readonly annualCostUsd: number
  readonly utilizationPercent: number
  readonly allocationPercent: number
  readonly runtimeHours: number
  readonly supportMultiplier?: number
}): number =>
  shcReservedHourlyCostUsd(input.annualCostUsd, input.utilizationPercent) *
  input.allocationPercent *
  input.runtimeHours *
  (input.supportMultiplier ?? 1)

export const minerProviderMinimumBidUsd = (
  input: MinerProviderFloorInput,
): number => {
  const powerWearAndMargin =
    input.powerCostUsdPerKwh +
    input.wearUsdPerKwh +
    input.operatorMarginUsdPerKwh
  const floorUsdPerKwh = Math.max(
    powerWearAndMargin,
    input.miningFloorUsdPerKwh,
    input.vpsOrColoOpportunityUsdPerKwh,
  )

  return floorUsdPerKwh * input.jobKwh
}

export const facilityHourEnergyMwh = (
  facility: Pick<MulletFacility, 'capacityMw'>,
  allocationPercent = 1,
): number => facility.capacityMw.value * allocationPercent

export const miningRevenuePerMwh = (input: {
  readonly networkRevenueUsdPerMwh: number
  readonly networkEfficiencyJPerTh: number
  readonly facilityEfficiencyJPerTh: number
}): number =>
  input.networkRevenueUsdPerMwh *
  safeDivide(input.networkEfficiencyJPerTh, input.facilityEfficiencyJPerTh)

export const calculateMiningHour = (
  input: MiningHourInput,
): MiningHourEconomics => {
  const energyMwh = input.miningMw
  const grossRevenue = energyMwh * input.revenuePerMwh
  const poolFee = grossRevenue * (input.poolFeePercent ?? 0)
  const energyCost = energyMwh * input.electricityUsdPerMwh
  const opsCost = energyMwh * input.opsUsdPerMwh
  const profit = grossRevenue - poolFee - energyCost - opsCost

  return {
    energyMwh,
    grossRevenueUsd: roundUsd(grossRevenue),
    poolFeeUsd: roundUsd(poolFee),
    energyCostUsd: roundUsd(energyCost),
    opsCostUsd: roundUsd(opsCost),
    profitUsd: roundUsd(profit),
    profitUsdPerMwh: roundUsd(safeDivide(profit, energyMwh)),
  }
}

export const calculateRuntimeCapacity = (
  input: RuntimeCapacityInput,
): RuntimeCapacity => {
  const energyMwh = input.loadPowerKw / 1000
  const attempts = input.attemptsPerInstanceHour * input.demandFillPercent
  const acceptedOutcomes = attempts * input.acceptanceRate

  return {
    energyMwhPerHour: energyMwh,
    attemptsPerHour: attempts,
    acceptedOutcomesPerHour: acceptedOutcomes,
    acceptedOutcomesPerMwh: safeDivide(acceptedOutcomes, energyMwh),
  }
}

export const calculateMargotStyleFacilityBaseline = (
  input: MargotStyleFacilityInput,
): MargotStyleFacilityBaseline => {
  const miningMw = input.facilityMw * input.miningAllocationPercent
  const aiMw = input.facilityMw * input.aiAllocationPercent
  const mining = calculateMiningHour({
    miningMw,
    revenuePerMwh: input.miningRevenueUsdPerMwh,
    electricityUsdPerMwh: input.electricityUsdPerMwh,
    opsUsdPerMwh: input.siteOpsUsdPerMwh,
    poolFeePercent: input.poolFeePercent,
  })
  const kwPerGpu = input.aiSystemPowerKw / input.aiGpuCountPerNode
  const aiGpuCount = (aiMw * 1000) / kwPerGpu
  const rawAiRevenue =
    aiGpuCount * input.aiGpuRentalUsdPerHour * input.aiUtilizationPercent
  const aiEnergyCost = aiMw * input.electricityUsdPerMwh
  const aiOpsCost = aiMw * input.siteOpsUsdPerMwh

  return {
    miningMw,
    aiMw,
    aiGpuCount: roundTo(aiGpuCount, 2),
    miningRevenueUsdPerHour: mining.grossRevenueUsd,
    miningProfitUsdPerHour: mining.profitUsd,
    rawAiRevenueUsdPerHour: roundUsd(rawAiRevenue),
    rawAiRevenueUsdPerMwh: roundUsd(safeDivide(rawAiRevenue, aiMw)),
    combinedGrossRevenueUsdPerHour: roundUsd(
      mining.grossRevenueUsd + rawAiRevenue,
    ),
    combinedProfitBeforeCapexUsdPerHour: roundUsd(
      mining.profitUsd + rawAiRevenue - aiEnergyCost - aiOpsCost,
    ),
  }
}

export const candidateMode = (
  input: CandidateModeInput,
): MulletHourlyCandidateMode => ({
  timestamp: IsoTimestamp.make(input.timestamp),
  mode: input.mode,
  buyerRevenueUsd: Usd.make(roundUsd(input.buyerRevenueUsd)),
  providerPayoutUsd: Usd.make(roundUsd(input.providerPayoutUsd)),
  openagentsMarginUsd: Usd.make(roundUsd(input.openagentsMarginUsd)),
  providerNetUsdPerMwh: UsdPerMWh.make(roundUsd(input.providerNetUsdPerMwh)),
  acceptedOutcomes: roundTo(input.acceptedOutcomes, 4),
  acceptedOutcomesPerMwh: roundTo(input.acceptedOutcomesPerMwh, 4),
  energyMwh: MWh.make(roundTo(input.energyMwh, 6)),
  riskAdjustedNetUsdPerMwh: UsdPerMWh.make(
    roundUsd(input.riskAdjustedNetUsdPerMwh),
  ),
  clearsReadiness: input.clearsReadiness,
  clearsDemand: input.clearsDemand,
  clearsProviderFloor: input.clearsProviderFloor,
  reasonCode: input.reasonCode,
})

export const selectDispatchCandidate = (
  candidates: readonly MulletHourlyCandidateMode[],
): MulletHourlyCandidateMode => {
  const eligible = candidates.filter(
    candidate =>
      candidate.clearsReadiness &&
      candidate.clearsDemand &&
      candidate.clearsProviderFloor,
  )
  const selectable = eligible.length > 0 ? eligible : candidates
  const selected = selectable.reduce((best, candidate) =>
    candidate.riskAdjustedNetUsdPerMwh > best.riskAdjustedNetUsdPerMwh
      ? candidate
      : best,
  )

  return selected
}

export const nodeClearsAcceptedWorkReadiness = (
  node: MulletComputeNode,
): boolean =>
  [
    'benchmark_passed',
    'workload_fit_classified',
    'schedulable',
    'accepted_work_proven',
    'payout_proven',
  ].includes(node.readinessState)

export const nodeClearsWorkClassEligibility = (
  node: MulletComputeNode,
  workClass: MulletWorkClass,
): boolean =>
  node.workloadFit.includes(workClass.id) &&
  workClass.eligibleNodeTypes.includes(node.nodeType)

export const acceptedWorkReasonCode = (input: {
  readonly backlog: number
  readonly eligible: boolean
  readonly ready: boolean
  readonly marginUsdPerMwh: number
  readonly clearsProviderFloor: boolean
}): ReasonCode => {
  if (input.backlog <= 0) {
    return 'accepted_work_blocked_no_backlog'
  }
  if (!input.eligible) {
    return 'accepted_work_blocked_not_eligible'
  }
  if (!input.ready) {
    return 'accepted_work_blocked_not_ready'
  }
  if (input.marginUsdPerMwh <= 0) {
    return 'accepted_work_blocked_negative_margin'
  }
  if (!input.clearsProviderFloor) {
    return 'accepted_work_blocked_provider_floor'
  }

  return 'accepted_work_clears_all_gates'
}

export const simulateScenarioHour = (
  scenario: MulletScenario,
  options: ScenarioHourOptions = {},
): MulletHourlyDispatchResult => {
  const timestamp = options.timestamp ?? scenario.dateRange.startAt
  const node = requireFirst(scenario.computeNodes, 'compute node')
  const workClass = requireFirst(scenario.workClasses, 'work class')
  const benchmark = requireFirst(
    scenario.runtimeBenchmarks,
    'runtime benchmark',
  )
  const providerPolicy = requireFirst(
    scenario.providerPolicies,
    'provider policy',
  )
  const effectivePriceUsdPerMwh =
    options.effectivePriceUsdPerMwh ??
    scenario.facility.fixedPriceUsdPerMwh.value
  const demandFillPercent = scenario.demandAssumptions.demandFillPercent.value
  const runtime = calculateRuntimeCapacity({
    loadPowerKw: node.loadPowerKw,
    attemptsPerInstanceHour: benchmark.attemptsPerInstanceHour,
    acceptanceRate: workClass.acceptanceRate,
    demandFillPercent,
  })
  const providerFloorUsd = Math.max(
    providerPolicy.providerMinimumBidUsdPerHour,
    providerPolicy.providerMinimumBidUsdPerJob * runtime.attemptsPerHour,
    runtime.energyMwhPerHour *
      (effectivePriceUsdPerMwh +
        providerPolicy.operatorMarginUsdPerMwh +
        providerPolicy.wearUsdPerKwh * 1000),
  )
  const acceptedProviderPayout = Math.max(
    providerFloorUsd,
    scenario.partySplit.providerReceivesUsd * runtime.acceptedOutcomesPerHour,
  )
  const acceptedBuyerRevenue =
    runtime.acceptedOutcomesPerHour * workClass.buyerPriceUsd
  const acceptedOpenagentsMargin =
    acceptedBuyerRevenue -
    acceptedProviderPayout -
    runtime.acceptedOutcomesPerHour *
      (scenario.partySplit.validatorsReceiveUsd +
        scenario.partySplit.reviewersReceiveUsd +
        scenario.partySplit.facilityOperatorReceivesUsd +
        scenario.partySplit.hardwareOwnerReceivesUsd +
        scenario.partySplit.settlementCostUsd +
        scenario.partySplit.riskReserveUsd)
  const acceptedMarginPerMwh = safeDivide(
    acceptedOpenagentsMargin,
    runtime.energyMwhPerHour,
  )
  const acceptedClearsProviderFloor = acceptedProviderPayout >= providerFloorUsd
  const acceptedReady = nodeClearsAcceptedWorkReadiness(node)
  const acceptedEligible = nodeClearsWorkClassEligibility(node, workClass)
  const acceptedReason = acceptedWorkReasonCode({
    backlog: Math.min(
      scenario.demandAssumptions.acceptedWorkBacklog.value,
      workClass.demandBacklog,
    ),
    eligible: acceptedEligible,
    ready: acceptedReady,
    marginUsdPerMwh: acceptedMarginPerMwh,
    clearsProviderFloor: acceptedClearsProviderFloor,
  })
  const mining = calculateMiningCandidate({
    timestamp,
    facility: scenario.facility,
    miningFleet: scenario.miningFleet,
    effectivePriceUsdPerMwh,
    ...(options.miningRevenueUsdPerMwh === undefined
      ? {}
      : { miningRevenueUsdPerMwh: options.miningRevenueUsdPerMwh }),
  })
  const rawGpuRevenueUsd = options.rawGpuRevenueUsdPerHour ?? 0
  const tokenApiRevenueUsd = options.tokenApiRevenueUsdPerHour ?? 0
  const curtailmentValueUsdPerMwh = options.curtailmentValueUsdPerMwh ?? 0
  const rawGpu = fallbackCandidate({
    timestamp,
    mode: 'raw_gpu_market',
    revenueUsd: rawGpuRevenueUsd,
    energyMwh: runtime.energyMwhPerHour,
    reasonCode: 'raw_gpu_market_best_available',
  })
  const tokenApi = fallbackCandidate({
    timestamp,
    mode: 'token_api_inference',
    revenueUsd: tokenApiRevenueUsd,
    energyMwh: runtime.energyMwhPerHour,
    reasonCode: 'token_api_best_available',
  })
  const curtailment = fallbackCandidate({
    timestamp,
    mode: 'curtail',
    revenueUsd: curtailmentValueUsdPerMwh * runtime.energyMwhPerHour,
    energyMwh: runtime.energyMwhPerHour,
    reasonCode: 'curtailment_best_available',
  })
  const idle = candidateMode({
    timestamp,
    mode: 'idle',
    buyerRevenueUsd: 0,
    providerPayoutUsd: 0,
    openagentsMarginUsd: 0,
    providerNetUsdPerMwh: 0,
    acceptedOutcomes: 0,
    acceptedOutcomesPerMwh: 0,
    energyMwh: 0,
    riskAdjustedNetUsdPerMwh: 0,
    clearsReadiness: true,
    clearsDemand: true,
    clearsProviderFloor: true,
    reasonCode: 'idle_best_available',
  })
  const acceptedWork = candidateMode({
    timestamp,
    mode: 'openagents_accepted_work',
    buyerRevenueUsd: acceptedBuyerRevenue,
    providerPayoutUsd: acceptedProviderPayout,
    openagentsMarginUsd: acceptedOpenagentsMargin,
    providerNetUsdPerMwh: safeDivide(
      acceptedProviderPayout,
      runtime.energyMwhPerHour,
    ),
    acceptedOutcomes: runtime.acceptedOutcomesPerHour,
    acceptedOutcomesPerMwh: runtime.acceptedOutcomesPerMwh,
    energyMwh: runtime.energyMwhPerHour,
    riskAdjustedNetUsdPerMwh: acceptedMarginPerMwh,
    clearsReadiness: acceptedReady && acceptedEligible,
    clearsDemand:
      scenario.demandAssumptions.acceptedWorkBacklog.value > 0 &&
      workClass.demandBacklog > 0 &&
      demandFillPercent > 0,
    clearsProviderFloor: acceptedClearsProviderFloor,
    reasonCode: acceptedReason,
  })
  const candidates = [mining, rawGpu, tokenApi, acceptedWork, curtailment, idle]
  const selected = selectDispatchCandidate(candidates)

  return {
    timestamp: IsoTimestamp.make(timestamp),
    effectivePriceUsdPerMwh: UsdPerMWh.make(roundUsd(effectivePriceUsdPerMwh)),
    selectedMode: selected.mode,
    candidates,
    miningRevenueUsd: mining.buyerRevenueUsd,
    miningProfitUsd: mining.openagentsMarginUsd,
    rawGpuRevenueUsd: rawGpu.buyerRevenueUsd,
    tokenApiRevenueUsd: tokenApi.buyerRevenueUsd,
    acceptedWorkBuyerRevenueUsd: acceptedWork.buyerRevenueUsd,
    acceptedWorkProviderPayoutUsd: acceptedWork.providerPayoutUsd,
    acceptedWorkOpenagentsMarginUsd: acceptedWork.openagentsMarginUsd,
    acceptedOutcomes: acceptedWork.acceptedOutcomes,
    acceptedOutcomesPerMwh: acceptedWork.acceptedOutcomesPerMwh,
    proofPacketIds: [],
    marketMemoryUpdateIds: [],
    energyTelemetryRecordIds: [],
    energyMwh: MWh.make(roundTo(Number(selected.energyMwh), 6)),
    curtailedMw: Megawatts.make(
      selected.mode === 'curtail' ? node.loadPowerKw / 1000 : 0,
    ),
    idleMw: Megawatts.make(
      selected.mode === 'idle' ? node.loadPowerKw / 1000 : 0,
    ),
    reasonCode: selected.reasonCode,
    provenance: 'modeled',
    confidence: Confidence.make(0.72),
  }
}

export const calculateCapitalReturnSummary = (
  input: CapitalReturnInput,
): MulletCapitalReturnSummary => {
  const grossMarginUsd = input.grossRevenueUsd - input.cogsUsd
  const monthlyDiscountRate = input.annualDiscountRatePercent / 12 / 100
  const discountedCashFlows = Array.from(
    { length: input.months },
    (_, index) => {
      const month = index + 1

      return input.monthlyNetUsd / (1 + monthlyDiscountRate) ** month
    },
  )
  const discountedResidual =
    input.residualValueUsd / (1 + monthlyDiscountRate) ** input.months
  const npv =
    discountedCashFlows.reduce((sum, value) => sum + value, 0) +
    discountedResidual -
    input.capexUsd
  const paybackMonths =
    input.monthlyNetUsd <= 0
      ? Number.POSITIVE_INFINITY
      : input.capexUsd / input.monthlyNetUsd

  return {
    party: input.party,
    grossRevenueUsd: Usd.make(roundUsd(input.grossRevenueUsd)),
    cogsUsd: Usd.make(roundUsd(input.cogsUsd)),
    grossMarginUsd: Usd.make(roundUsd(grossMarginUsd)),
    paybackMonths: roundTo(paybackMonths, 2),
    irrPercent: Percent.make(
      roundTo(safeDivide(grossMarginUsd * 12, input.capexUsd) * 100, 2),
    ),
    npvUsd: Usd.make(roundUsd(npv)),
    downsideProtectionUsd: Usd.make(roundUsd(input.downsideProtectionUsd ?? 0)),
  }
}

const calculateMiningCandidate = (input: {
  readonly timestamp: string
  readonly facility: MulletFacility
  readonly miningFleet: MulletMiningFleet
  readonly effectivePriceUsdPerMwh: number
  readonly miningRevenueUsdPerMwh?: number
}): MulletHourlyCandidateMode => {
  const miningMw = Math.min(
    input.facility.capacityMw.value,
    (input.miningFleet.count * input.miningFleet.wattsPerUnit) / 1_000_000,
  )
  const revenuePerMwh = input.miningRevenueUsdPerMwh ?? 0
  const mining = calculateMiningHour({
    miningMw,
    revenuePerMwh,
    electricityUsdPerMwh: input.effectivePriceUsdPerMwh,
    opsUsdPerMwh: input.miningFleet.firmwareOpsCostUsdPerMwh,
    poolFeePercent: input.miningFleet.poolFeePercent,
  })

  return candidateMode({
    timestamp: input.timestamp,
    mode: 'mine',
    buyerRevenueUsd: mining.grossRevenueUsd,
    providerPayoutUsd: 0,
    openagentsMarginUsd: mining.profitUsd,
    providerNetUsdPerMwh: 0,
    acceptedOutcomes: 0,
    acceptedOutcomesPerMwh: 0,
    energyMwh: mining.energyMwh,
    riskAdjustedNetUsdPerMwh: mining.profitUsdPerMwh,
    clearsReadiness: miningMw > 0,
    clearsDemand: true,
    clearsProviderFloor: true,
    reasonCode: 'mining_best_available',
  })
}

const fallbackCandidate = (input: {
  readonly timestamp: string
  readonly mode: MulletDispatchMode
  readonly revenueUsd: number
  readonly energyMwh: number
  readonly reasonCode: ReasonCode
}): MulletHourlyCandidateMode =>
  candidateMode({
    timestamp: input.timestamp,
    mode: input.mode,
    buyerRevenueUsd: input.revenueUsd,
    providerPayoutUsd: 0,
    openagentsMarginUsd: input.revenueUsd,
    providerNetUsdPerMwh: 0,
    acceptedOutcomes: 0,
    acceptedOutcomesPerMwh: 0,
    energyMwh: input.energyMwh,
    riskAdjustedNetUsdPerMwh: safeDivide(input.revenueUsd, input.energyMwh),
    clearsReadiness: input.revenueUsd > 0,
    clearsDemand: input.revenueUsd > 0,
    clearsProviderFloor: true,
    reasonCode: input.reasonCode,
  })

const requireFirst = <A>(values: readonly A[], label: string): A => {
  const value = values[0]

  if (value === undefined) {
    throw new Error(`Mullet simulation requires at least one ${label}`)
  }

  return value
}
