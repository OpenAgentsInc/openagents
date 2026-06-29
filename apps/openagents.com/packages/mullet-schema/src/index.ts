import { Schema as S } from 'effect'

export const MulletScenarioId = S.String.pipe(S.brand('MulletScenarioId'))
export type MulletScenarioId = typeof MulletScenarioId.Type

export const MulletSimulationRunId = S.String.pipe(
  S.brand('MulletSimulationRunId'),
)
export type MulletSimulationRunId = typeof MulletSimulationRunId.Type

export const MulletFacilityId = S.String.pipe(S.brand('MulletFacilityId'))
export type MulletFacilityId = typeof MulletFacilityId.Type

export const MulletSiteId = S.String.pipe(S.brand('MulletSiteId'))
export type MulletSiteId = typeof MulletSiteId.Type

export const MulletNodeId = S.String.pipe(S.brand('MulletNodeId'))
export type MulletNodeId = typeof MulletNodeId.Type

export const MulletWorkClassId = S.String.pipe(S.brand('MulletWorkClassId'))
export type MulletWorkClassId = typeof MulletWorkClassId.Type

export const MulletProofPacketId = S.String.pipe(
  S.brand('MulletProofPacketId'),
)
export type MulletProofPacketId = typeof MulletProofPacketId.Type

export const MulletMarketMemoryId = S.String.pipe(
  S.brand('MulletMarketMemoryId'),
)
export type MulletMarketMemoryId = typeof MulletMarketMemoryId.Type

export const MulletEnergyTelemetryRecordId = S.String.pipe(
  S.brand('MulletEnergyTelemetryRecordId'),
)
export type MulletEnergyTelemetryRecordId =
  typeof MulletEnergyTelemetryRecordId.Type

export const MulletSourceRefId = S.String.pipe(S.brand('MulletSourceRefId'))
export type MulletSourceRefId = typeof MulletSourceRefId.Type

export const UsdCents = S.Number.pipe(S.brand('UsdCents'))
export type UsdCents = typeof UsdCents.Type

export const Usd = S.Number.pipe(S.brand('Usd'))
export type Usd = typeof Usd.Type

export const UsdPerHour = S.Number.pipe(S.brand('UsdPerHour'))
export type UsdPerHour = typeof UsdPerHour.Type

export const UsdPerMWh = S.Number.pipe(S.brand('UsdPerMWh'))
export type UsdPerMWh = typeof UsdPerMWh.Type

export const UsdPerKWh = S.Number.pipe(S.brand('UsdPerKWh'))
export type UsdPerKWh = typeof UsdPerKWh.Type

export const Watts = S.Number.pipe(S.brand('Watts'))
export type Watts = typeof Watts.Type

export const Kilowatts = S.Number.pipe(S.brand('Kilowatts'))
export type Kilowatts = typeof Kilowatts.Type

export const Megawatts = S.Number.pipe(S.brand('Megawatts'))
export type Megawatts = typeof Megawatts.Type

export const WattHours = S.Number.pipe(S.brand('WattHours'))
export type WattHours = typeof WattHours.Type

export const KWh = S.Number.pipe(S.brand('KWh'))
export type KWh = typeof KWh.Type

export const MWh = S.Number.pipe(S.brand('MWh'))
export type MWh = typeof MWh.Type

export const Percent = S.Number.pipe(S.brand('Percent'))
export type Percent = typeof Percent.Type

export const Confidence = S.Number.pipe(S.brand('Confidence'))
export type Confidence = typeof Confidence.Type

export const IsoTimestamp = S.String.pipe(S.brand('IsoTimestamp'))
export type IsoTimestamp = typeof IsoTimestamp.Type

export const MulletUnitLabel = S.Literals([
  'count',
  'usd',
  'usd_cents',
  'usd_per_hour',
  'usd_per_kwh',
  'usd_per_mwh',
  'watts',
  'kilowatts',
  'megawatts',
  'watt_hours',
  'kwh',
  'mwh',
  'percent',
  'seconds',
  'minutes',
  'hours',
  'months',
  'years',
  'tokens',
  'attempts',
  'outcomes',
  'score',
  'text',
])
export type MulletUnitLabel = typeof MulletUnitLabel.Type

export const MulletProvenanceLevel = S.Literals([
  'public_claim',
  'customer_reported',
  'manual_input',
  'estimated',
  'modeled',
  'forecast',
  'observed',
  'measured',
  'verified',
  'accepted',
  'paid',
  'settled',
  'placeholder',
])
export type MulletProvenanceLevel = typeof MulletProvenanceLevel.Type

export const MulletSiteClassification = S.Literals([
  'mining_only',
  'mining_led_ai_pilot_not_mullet',
  'balanced_hybrid',
  'mullet_ai_led_mining_backfill',
  'colo_only_candidate',
  'neither_no_fit',
])
export type MulletSiteClassification = typeof MulletSiteClassification.Type

export const MulletSupplyReadinessState = S.Literals([
  'raw_device',
  'inventory_known',
  'site_power_known',
  'cooling_network_ops_reviewed',
  'benchmark_passed',
  'workload_fit_classified',
  'schedulable',
  'accepted_work_proven',
  'payout_proven',
])
export type MulletSupplyReadinessState =
  typeof MulletSupplyReadinessState.Type

export const MulletCapacityLifecycleState = S.Literals([
  'discovered',
  'enrolled',
  'eligible',
  'admitted',
  'assigned',
  'completed',
  'accepted',
  'rejected',
  'settled',
  'payout_proven',
])
export type MulletCapacityLifecycleState =
  typeof MulletCapacityLifecycleState.Type

export const MulletDispatchMode = S.Literals([
  'mine',
  'raw_gpu_market',
  'token_api_inference',
  'openagents_accepted_work',
  'curtail',
  'idle',
  'reserve',
])
export type MulletDispatchMode = typeof MulletDispatchMode.Type

export const MulletPowerDataState = S.Literals([
  'unknown',
  'modeled',
  'measured',
  'mixed',
])
export type MulletPowerDataState = typeof MulletPowerDataState.Type

export const MulletProviderSettlementState = S.Literals([
  'not_payable',
  'payable_pending_settlement',
  'paid',
  'settled_bitcoin',
  'rejected_no_pay',
  'mixed',
])
export type MulletProviderSettlementState =
  typeof MulletProviderSettlementState.Type

export const MulletScenarioKind = S.Literals([
  'tinybox_shc_power',
  'tinybox_residential_power',
  'tinybox_west_texas_power',
  'facility_100mw_80_20',
  'shc_cpu_vps_colo',
  'miner_gpu_island',
  'custom',
])
export type MulletScenarioKind = typeof MulletScenarioKind.Type

export const MulletSourceRef = S.Struct({
  id: MulletSourceRefId,
  label: S.String,
  uri: S.optionalKey(S.String),
  capturedAt: S.optionalKey(IsoTimestamp),
})
export type MulletSourceRef = typeof MulletSourceRef.Type

const provenanceFields = {
  provenance: MulletProvenanceLevel,
  confidence: Confidence,
  source: MulletSourceRef,
  lastUpdated: IsoTimestamp,
  needsDiligence: S.Boolean,
  notes: S.optionalKey(S.String),
} as const

export const MulletProvenancedNumber = S.Struct({
  value: S.Number,
  unit: MulletUnitLabel,
  ...provenanceFields,
})
export type MulletProvenancedNumber = typeof MulletProvenancedNumber.Type

export const MulletProvenancedString = S.Struct({
  value: S.String,
  unit: MulletUnitLabel,
  ...provenanceFields,
})
export type MulletProvenancedString = typeof MulletProvenancedString.Type

export const MulletDateRange = S.Struct({
  startAt: IsoTimestamp,
  endAt: IsoTimestamp,
})
export type MulletDateRange = typeof MulletDateRange.Type

export const MulletPhysicalReadinessProfile = S.Struct({
  interconnectionStatus: MulletProvenancedString,
  energizedCapacityMw: MulletProvenancedNumber,
  transformerStatus: MulletProvenancedString,
  switchgearStatus: MulletProvenancedString,
  coolingCapacityKw: MulletProvenancedNumber,
  pue: MulletProvenancedNumber,
  fiberOrNetworkStatus: MulletProvenancedString,
  redundancyTier: MulletProvenancedString,
  remoteHandsSla: MulletProvenancedString,
  uptimeCommitment: MulletProvenancedNumber,
  liquidatedDamagesExposureUsd: MulletProvenancedNumber,
  permittingOrCommunityRisk: MulletProvenancedString,
})
export type MulletPhysicalReadinessProfile =
  typeof MulletPhysicalReadinessProfile.Type

export const MulletFacility = S.Struct({
  id: MulletFacilityId,
  siteId: MulletSiteId,
  name: S.String,
  market: S.String,
  zone: S.String,
  capacityMw: MulletProvenancedNumber,
  powerContractType: S.String,
  fixedPriceUsdPerMwh: MulletProvenancedNumber,
  maxAiAllocationMw: MulletProvenancedNumber,
  curtailmentPolicy: S.String,
  gridServiceTerms: S.String,
  siteOpsCostUsdPerMwh: MulletProvenancedNumber,
  coolingMultiplier: MulletProvenancedNumber,
  remoteHandsMonthlyUsd: MulletProvenancedNumber,
  physicalReadiness: MulletPhysicalReadinessProfile,
  customerSlaReserveUsdPerMwh: MulletProvenancedNumber,
  siteClassification: MulletSiteClassification,
  readinessState: MulletSupplyReadinessState,
  capacityLifecycleState: MulletCapacityLifecycleState,
})
export type MulletFacility = typeof MulletFacility.Type

export const MulletMiningFleet = S.Struct({
  asicModel: S.String,
  count: S.Number,
  wattsPerUnit: Watts,
  thPerUnit: S.Number,
  joulesPerTh: S.Number,
  capexPerUnitUsd: Usd,
  depreciationMonths: S.Number,
  poolFeePercent: Percent,
  firmwareOpsCostUsdPerMwh: UsdPerMWh,
})
export type MulletMiningFleet = typeof MulletMiningFleet.Type

export const MulletComputeNode = S.Struct({
  nodeId: MulletNodeId,
  nodeType: S.String,
  ownerParty: S.String,
  operatorParty: S.String,
  siteId: MulletSiteId,
  gpuModel: S.optionalKey(S.String),
  gpuCount: S.Number,
  vramGb: S.Number,
  interconnect: S.String,
  cpu: S.String,
  ramGb: S.Number,
  storageGb: S.Number,
  networkGbps: S.Number,
  capexUsd: Usd,
  depreciationMonths: S.Number,
  idlePowerKw: Kilowatts,
  loadPowerKw: Kilowatts,
  powerLimitKw: Kilowatts,
  supportMonthlyUsd: Usd,
  fallbackMarketEligible: S.Boolean,
  trustTier: S.String,
  readinessState: MulletSupplyReadinessState,
  capacityLifecycleState: MulletCapacityLifecycleState,
  workloadFit: S.Array(MulletWorkClassId),
})
export type MulletComputeNode = typeof MulletComputeNode.Type

export const MulletRuntimeBenchmark = S.Struct({
  nodeId: MulletNodeId,
  workClassId: MulletWorkClassId,
  modelId: S.String,
  framework: S.String,
  precision: S.String,
  batchSize: S.Number,
  attemptsPerInstanceHour: S.Number,
  tokensPerSecond: S.Number,
  joulesPerToken: S.Number,
  kwhPerAttempt: KWh,
  wallSecondsPerAttempt: S.Number,
  observedFailureRate: Percent,
  source: MulletSourceRef,
  confidence: Confidence,
})
export type MulletRuntimeBenchmark = typeof MulletRuntimeBenchmark.Type

export const MulletWorkClassFlexibility = S.Struct({
  canPause: S.Boolean,
  canResume: S.Boolean,
  canMigrate: S.Boolean,
  checkpointIntervalMinutes: S.Number,
  maxDelayMinutes: S.Number,
  deadlineMinutes: S.Number,
  customerImpactIfDelayed: S.String,
  privacyTier: S.String,
  stateLocality: S.String,
  requiredTools: S.Array(S.String),
})
export type MulletWorkClassFlexibility =
  typeof MulletWorkClassFlexibility.Type

export const MulletWorkClass = S.Struct({
  id: MulletWorkClassId,
  label: S.String,
  latencyClass: S.String,
  buyerPriceUsd: Usd,
  acceptanceRate: Percent,
  targetMargin: Percent,
  riskReserveUsd: Usd,
  frontierInputTokens: S.Number,
  frontierOutputTokens: S.Number,
  frontierInputPriceUsdPerMillion: Usd,
  frontierOutputPriceUsdPerMillion: Usd,
  cheapModelCostUsd: Usd,
  workroomRuntimeHours: S.Number,
  workroomHourlyCostUsd: UsdPerHour,
  providerComputeHours: S.Number,
  providerPowerKw: Kilowatts,
  validatorCount: S.Number,
  validatorPayoutEachUsd: Usd,
  graderCostUsd: Usd,
  humanReviewMinutes: S.Number,
  humanReviewHourlyCostUsd: UsdPerHour,
  artifactStorageCostUsd: Usd,
  settlementCostUsd: Usd,
  supportOverheadUsd: Usd,
  retryOrFailureCostUsd: Usd,
  flexibility: MulletWorkClassFlexibility,
  demandBacklog: S.Number,
  eligibleNodeTypes: S.Array(S.String),
  minimumTrustTier: S.String,
})
export type MulletWorkClass = typeof MulletWorkClass.Type

export const MulletProviderBidPolicy = S.Struct({
  nodeType: S.String,
  providerMinimumBidUsdPerHour: UsdPerHour,
  providerMinimumBidUsdPerJob: Usd,
  wearUsdPerKwh: UsdPerKWh,
  bandwidthUsdPerJob: Usd,
  desiredProfitUsdPerJob: Usd,
  operatorMarginUsdPerMwh: UsdPerMWh,
  miningFloorPolicy: S.String,
  rawGpuFloorPolicy: S.String,
  vpsColocationFloorPolicy: S.String,
  curtailmentGridServiceFloorPolicy: S.String,
})
export type MulletProviderBidPolicy = typeof MulletProviderBidPolicy.Type

export const MulletPartySplit = S.Struct({
  buyerPaysUsd: Usd,
  openagentsKeepsUsd: Usd,
  providerReceivesUsd: Usd,
  facilityOperatorReceivesUsd: Usd,
  hardwareOwnerReceivesUsd: Usd,
  validatorsReceiveUsd: Usd,
  reviewersReceiveUsd: Usd,
  settlementCostUsd: Usd,
  riskReserveUsd: Usd,
})
export type MulletPartySplit = typeof MulletPartySplit.Type

export const MulletCapitalAssumptions = S.Struct({
  hardwareCapexUsd: MulletProvenancedNumber,
  depreciationMonths: MulletProvenancedNumber,
  discountRatePercent: MulletProvenancedNumber,
  debtServiceMonthlyUsd: MulletProvenancedNumber,
  residualValueUsd: MulletProvenancedNumber,
})
export type MulletCapitalAssumptions = typeof MulletCapitalAssumptions.Type

export const MulletDemandAssumptions = S.Struct({
  acceptedWorkBacklog: MulletProvenancedNumber,
  demandFillPercent: MulletProvenancedNumber,
  rawGpuMarketFillPercent: MulletProvenancedNumber,
  tokenApiFillPercent: MulletProvenancedNumber,
})
export type MulletDemandAssumptions = typeof MulletDemandAssumptions.Type

export const MulletHourlyCandidateMode = S.Struct({
  timestamp: IsoTimestamp,
  mode: MulletDispatchMode,
  buyerRevenueUsd: Usd,
  providerPayoutUsd: Usd,
  openagentsMarginUsd: Usd,
  providerNetUsdPerMwh: UsdPerMWh,
  acceptedOutcomes: S.Number,
  acceptedOutcomesPerMwh: S.Number,
  energyMwh: MWh,
  riskAdjustedNetUsdPerMwh: UsdPerMWh,
  clearsReadiness: S.Boolean,
  clearsDemand: S.Boolean,
  clearsProviderFloor: S.Boolean,
  reasonCode: S.String,
})
export type MulletHourlyCandidateMode = typeof MulletHourlyCandidateMode.Type

export const MulletHourlyDispatchResult = S.Struct({
  timestamp: IsoTimestamp,
  effectivePriceUsdPerMwh: UsdPerMWh,
  selectedMode: MulletDispatchMode,
  candidates: S.Array(MulletHourlyCandidateMode),
  miningRevenueUsd: Usd,
  miningProfitUsd: Usd,
  rawGpuRevenueUsd: Usd,
  tokenApiRevenueUsd: Usd,
  acceptedWorkBuyerRevenueUsd: Usd,
  acceptedWorkProviderPayoutUsd: Usd,
  acceptedWorkOpenagentsMarginUsd: Usd,
  acceptedOutcomes: S.Number,
  acceptedOutcomesPerMwh: S.Number,
  proofPacketIds: S.Array(MulletProofPacketId),
  marketMemoryUpdateIds: S.Array(MulletMarketMemoryId),
  energyTelemetryRecordIds: S.Array(MulletEnergyTelemetryRecordId),
  energyMwh: MWh,
  curtailedMw: Megawatts,
  idleMw: Megawatts,
  reasonCode: S.String,
  provenance: MulletProvenanceLevel,
  confidence: Confidence,
})
export type MulletHourlyDispatchResult =
  typeof MulletHourlyDispatchResult.Type

export const MulletCapitalReturnSummary = S.Struct({
  party: S.String,
  grossRevenueUsd: Usd,
  cogsUsd: Usd,
  grossMarginUsd: Usd,
  paybackMonths: S.Number,
  irrPercent: Percent,
  npvUsd: Usd,
  downsideProtectionUsd: Usd,
})
export type MulletCapitalReturnSummary =
  typeof MulletCapitalReturnSummary.Type

export const MulletAcceptedWorkProofPacket = S.Struct({
  id: MulletProofPacketId,
  workId: S.String,
  workClassId: MulletWorkClassId,
  nodeId: MulletNodeId,
  nodeCapabilitySnapshotRef: S.String,
  assignmentId: S.String,
  executionArtifactRef: S.String,
  validatorVerdictRef: S.String,
  acceptedCloseoutRef: S.String,
  buyerPriceUsd: Usd,
  providerPayoutUsd: Usd,
  settlementReceiptRef: S.optionalKey(S.String),
  routingConsequence: S.String,
  provenance: MulletProvenanceLevel,
})
export type MulletAcceptedWorkProofPacket =
  typeof MulletAcceptedWorkProofPacket.Type

export const MulletMarketMemory = S.Struct({
  id: MulletMarketMemoryId,
  nodeId: MulletNodeId,
  siteId: MulletSiteId,
  workClassId: MulletWorkClassId,
  acceptedCount: S.Number,
  rejectedCount: S.Number,
  acceptanceProbability: Percent,
  medianRuntimeSeconds: S.Number,
  medianPayoutSeconds: S.Number,
  payoutSuccessRate: Percent,
  repeatProviderScore: S.Number,
  repeatBuyerScore: S.Number,
  validatorReliabilityScore: S.Number,
  commonFailureModes: S.Array(S.String),
  lastUpdated: IsoTimestamp,
})
export type MulletMarketMemory = typeof MulletMarketMemory.Type

export const MulletEnergyTelemetryRecord = S.Struct({
  id: MulletEnergyTelemetryRecordId,
  timestamp: IsoTimestamp,
  siteId: MulletSiteId,
  nodeId: MulletNodeId,
  workId: S.String,
  powerKw: Kilowatts,
  energyKwh: KWh,
  powerDataState: MulletPowerDataState,
  gridSignal: S.String,
  curtailmentOrShiftAction: S.String,
  priceCounterfactual: S.String,
  emissionsCounterfactual: S.String,
  customerImpact: S.String,
  payoutUsd: Usd,
  marginUsd: Usd,
  provenance: MulletProvenanceLevel,
})
export type MulletEnergyTelemetryRecord =
  typeof MulletEnergyTelemetryRecord.Type

export const MulletScenario = S.Struct({
  id: MulletScenarioId,
  name: S.String,
  schemaVersion: S.String,
  kind: MulletScenarioKind,
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
  dateRange: MulletDateRange,
  facility: MulletFacility,
  miningFleet: MulletMiningFleet,
  computeNodes: S.Array(MulletComputeNode),
  runtimeBenchmarks: S.Array(MulletRuntimeBenchmark),
  workClasses: S.Array(MulletWorkClass),
  providerPolicies: S.Array(MulletProviderBidPolicy),
  partySplit: MulletPartySplit,
  capitalAssumptions: MulletCapitalAssumptions,
  demandAssumptions: MulletDemandAssumptions,
  sourceRefs: S.Array(MulletSourceRef),
  notes: S.optionalKey(S.String),
})
export type MulletScenario = typeof MulletScenario.Type

export const MulletSimulationRun = S.Struct({
  id: MulletSimulationRunId,
  scenarioId: MulletScenarioId,
  ownerUserId: S.String,
  ownerEmail: S.String,
  status: S.Literals(['queued', 'running', 'succeeded', 'failed']),
  createdAt: IsoTimestamp,
  updatedAt: IsoTimestamp,
  completedAt: S.optionalKey(IsoTimestamp),
  scenario: MulletScenario,
  dispatchResults: S.Array(MulletHourlyDispatchResult),
  capitalReturns: S.Array(MulletCapitalReturnSummary),
  proofPackets: S.Array(MulletAcceptedWorkProofPacket),
  marketMemory: S.Array(MulletMarketMemory),
  energyTelemetry: S.Array(MulletEnergyTelemetryRecord),
  providerSettlementState: MulletProviderSettlementState,
  powerDataState: MulletPowerDataState,
})
export type MulletSimulationRun = typeof MulletSimulationRun.Type

export const MulletSimulationRunExport = S.Struct({
  runId: MulletSimulationRunId,
  scenarioId: MulletScenarioId,
  generatedAt: IsoTimestamp,
  format: S.Literals(['markdown', 'json']),
  privateVisibility: S.Boolean,
  redactionStatus: S.Literals(['not_checked', 'passed', 'failed']),
  modeledValueCount: S.Number,
  measuredValueCount: S.Number,
  acceptedValueCount: S.Number,
  paidValueCount: S.Number,
  settledValueCount: S.Number,
  contentRef: S.String,
})
export type MulletSimulationRunExport =
  typeof MulletSimulationRunExport.Type

export const MulletScenarioJson = S.fromJsonString(MulletScenario)
export const MulletSimulationRunJson = S.fromJsonString(MulletSimulationRun)

export const decodeMulletScenario = S.decodeUnknownSync(MulletScenario)
export const decodeMulletScenarioJson = S.decodeUnknownSync(MulletScenarioJson)
export const encodeMulletScenarioJson = S.encodeSync(MulletScenarioJson)

export const decodeMulletSimulationRun =
  S.decodeUnknownSync(MulletSimulationRun)
export const decodeMulletSimulationRunJson = S.decodeUnknownSync(
  MulletSimulationRunJson,
)
export const encodeMulletSimulationRunJson = S.encodeSync(
  MulletSimulationRunJson,
)
