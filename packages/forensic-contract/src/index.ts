export * from "./artifact-witness.ts";
export * from "./canonical.ts";
export * from "./benchmark.ts";
export * from "./claims.ts";
export * from "./coldcard-generator.ts";
export * from "./evidence-derivation.ts";
export * from "./historical-scan.ts";
export * from "./lifecycle.ts";
export * from "./metrics.ts";
export * from "./primitives.ts";
export * from "./projection.ts";
export * from "./reproduction.ts";
export * from "./run.ts";

import {
  COLDCARD_BENCHMARK_MANIFEST_VERSION,
  COLDCARD_HISTORICAL_IMPORT_VERSION,
  COLDCARD_SUITE_MANIFEST_VERSION,
  ColdcardBenchmarkManifestSchema,
  ColdcardHistoricalImportSchema,
  ColdcardSuiteManifestSchema,
} from "./benchmark.ts";
import {
  FORENSIC_CLAIM_RECORD_VERSION,
  FORENSIC_CLAIM_REVISION_VERSION,
  FORENSIC_EVIDENCE_GRAPH_VERSION,
  FORENSIC_FINDING_VERSION,
  FORENSIC_HYPOTHESIS_VERSION,
  ForensicClaimRecordSchema,
  ForensicClaimRevisionSchema,
  ForensicEvidenceGraphSchema,
  ForensicFindingSchema,
  ForensicHypothesisSchema,
} from "./claims.ts";
import {
  FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  FORENSIC_INFRASTRUCTURE_COST_RECEIPT_VERSION,
  FORENSIC_METRIC_DEFINITION_VERSION,
  FORENSIC_METRIC_REGISTRY_VERSION,
  FORENSIC_PROMPT_PROMOTION_VERSION,
  FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION,
  FORENSIC_SCORECARD_VERSION,
  ForensicEvaluatorAdjudicationSchema,
  ForensicInfrastructureCostReceiptSchema,
  ForensicMetricDefinitionSchema,
  ForensicMetricRegistrySchema,
  ForensicPromptPromotionSchema,
  ForensicProviderUsageReceiptSchema,
  ForensicReviewerBurdenReceiptSchema,
  ForensicScorecardSchema,
} from "./metrics.ts";
import {
  FORENSIC_RUN_PUBLIC_PROJECTION_VERSION,
  FORENSIC_SCORECARD_PUBLIC_PROJECTION_VERSION,
  ForensicRunPublicProjectionSchema,
  ForensicScorecardPublicProjectionSchema,
} from "./projection.ts";
import {
  COLDCARD_REPRODUCTION_MANIFEST_VERSION,
  ENTROPY_STATE_MODEL_VERSION,
  GENERATOR_TRACE_VERSION,
  HISTORICAL_CHAIN_SNAPSHOT_VERSION,
  NODE_SCAN_RECEIPT_VERSION,
  TRANSACTION_FINGERPRINT_VERSION,
  ColdcardReproductionManifestSchema,
  EntropyStateModelSchema,
  GeneratorTraceSchema,
  HistoricalChainSnapshotSchema,
  NodeScanReceiptSchema,
  TransactionFingerprintSchema,
} from "./reproduction.ts";
import {
  FORENSIC_COVERAGE_MANIFEST_VERSION,
  FORENSIC_EVIDENCE_RECEIPT_VERSION,
  FORENSIC_PROMPT_ARTIFACT_VERSION,
  FORENSIC_RUN_EVENT_VERSION,
  FORENSIC_RUN_VERSION,
  FORENSIC_SCAN_PROFILE_VERSION,
  FORENSIC_SOURCE_BUNDLE_VERSION,
  FORENSIC_TARGET_SNAPSHOT_VERSION,
  FORENSIC_WORKER_PLACEMENT_VERSION,
  ForensicCoverageManifestSchema,
  ForensicEvidenceReceiptSchema,
  ForensicPromptArtifactSchema,
  ForensicRunEventSchema,
  ForensicRunSchema,
  ForensicScanProfileSchema,
  ForensicSourceBundleSchema,
  ForensicTargetSnapshotSchema,
  ForensicWorkerPlacementSchema,
} from "./run.ts";

export const FORENSIC_CONTRACT_SCHEMAS = {
  [COLDCARD_BENCHMARK_MANIFEST_VERSION]: ColdcardBenchmarkManifestSchema,
  [COLDCARD_HISTORICAL_IMPORT_VERSION]: ColdcardHistoricalImportSchema,
  [FORENSIC_TARGET_SNAPSHOT_VERSION]: ForensicTargetSnapshotSchema,
  [COLDCARD_REPRODUCTION_MANIFEST_VERSION]: ColdcardReproductionManifestSchema,
  [COLDCARD_SUITE_MANIFEST_VERSION]: ColdcardSuiteManifestSchema,
  [FORENSIC_SOURCE_BUNDLE_VERSION]: ForensicSourceBundleSchema,
  [FORENSIC_COVERAGE_MANIFEST_VERSION]: ForensicCoverageManifestSchema,
  [FORENSIC_SCAN_PROFILE_VERSION]: ForensicScanProfileSchema,
  [FORENSIC_WORKER_PLACEMENT_VERSION]: ForensicWorkerPlacementSchema,
  [FORENSIC_PROMPT_ARTIFACT_VERSION]: ForensicPromptArtifactSchema,
  [FORENSIC_RUN_VERSION]: ForensicRunSchema,
  [FORENSIC_FINDING_VERSION]: ForensicFindingSchema,
  [FORENSIC_HYPOTHESIS_VERSION]: ForensicHypothesisSchema,
  [FORENSIC_CLAIM_RECORD_VERSION]: ForensicClaimRecordSchema,
  [FORENSIC_CLAIM_REVISION_VERSION]: ForensicClaimRevisionSchema,
  [FORENSIC_EVIDENCE_GRAPH_VERSION]: ForensicEvidenceGraphSchema,
  [GENERATOR_TRACE_VERSION]: GeneratorTraceSchema,
  [ENTROPY_STATE_MODEL_VERSION]: EntropyStateModelSchema,
  [HISTORICAL_CHAIN_SNAPSHOT_VERSION]: HistoricalChainSnapshotSchema,
  [TRANSACTION_FINGERPRINT_VERSION]: TransactionFingerprintSchema,
  [NODE_SCAN_RECEIPT_VERSION]: NodeScanReceiptSchema,
  [FORENSIC_EVIDENCE_RECEIPT_VERSION]: ForensicEvidenceReceiptSchema,
  [FORENSIC_RUN_EVENT_VERSION]: ForensicRunEventSchema,
  [FORENSIC_METRIC_DEFINITION_VERSION]: ForensicMetricDefinitionSchema,
  [FORENSIC_METRIC_REGISTRY_VERSION]: ForensicMetricRegistrySchema,
  [FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION]: ForensicProviderUsageReceiptSchema,
  [FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION]: ForensicReviewerBurdenReceiptSchema,
  [FORENSIC_INFRASTRUCTURE_COST_RECEIPT_VERSION]: ForensicInfrastructureCostReceiptSchema,
  [FORENSIC_EVALUATOR_ADJUDICATION_VERSION]: ForensicEvaluatorAdjudicationSchema,
  [FORENSIC_SCORECARD_VERSION]: ForensicScorecardSchema,
  [FORENSIC_PROMPT_PROMOTION_VERSION]: ForensicPromptPromotionSchema,
  [FORENSIC_RUN_PUBLIC_PROJECTION_VERSION]: ForensicRunPublicProjectionSchema,
  [FORENSIC_SCORECARD_PUBLIC_PROJECTION_VERSION]: ForensicScorecardPublicProjectionSchema,
} as const;

export type ForensicContractSchemaId = keyof typeof FORENSIC_CONTRACT_SCHEMAS;

export const FORENSIC_CONTRACT_SCHEMA_IDS = [
  COLDCARD_BENCHMARK_MANIFEST_VERSION,
  COLDCARD_HISTORICAL_IMPORT_VERSION,
  COLDCARD_REPRODUCTION_MANIFEST_VERSION,
  COLDCARD_SUITE_MANIFEST_VERSION,
  ENTROPY_STATE_MODEL_VERSION,
  FORENSIC_CLAIM_RECORD_VERSION,
  FORENSIC_CLAIM_REVISION_VERSION,
  FORENSIC_COVERAGE_MANIFEST_VERSION,
  FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  FORENSIC_EVIDENCE_GRAPH_VERSION,
  FORENSIC_EVIDENCE_RECEIPT_VERSION,
  FORENSIC_FINDING_VERSION,
  FORENSIC_HYPOTHESIS_VERSION,
  FORENSIC_INFRASTRUCTURE_COST_RECEIPT_VERSION,
  FORENSIC_METRIC_DEFINITION_VERSION,
  FORENSIC_METRIC_REGISTRY_VERSION,
  FORENSIC_PROMPT_ARTIFACT_VERSION,
  FORENSIC_PROMPT_PROMOTION_VERSION,
  FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION,
  FORENSIC_RUN_VERSION,
  FORENSIC_RUN_EVENT_VERSION,
  FORENSIC_RUN_PUBLIC_PROJECTION_VERSION,
  FORENSIC_SCAN_PROFILE_VERSION,
  FORENSIC_SCORECARD_VERSION,
  FORENSIC_SCORECARD_PUBLIC_PROJECTION_VERSION,
  FORENSIC_SOURCE_BUNDLE_VERSION,
  FORENSIC_TARGET_SNAPSHOT_VERSION,
  FORENSIC_WORKER_PLACEMENT_VERSION,
  GENERATOR_TRACE_VERSION,
  HISTORICAL_CHAIN_SNAPSHOT_VERSION,
  NODE_SCAN_RECEIPT_VERSION,
  TRANSACTION_FINGERPRINT_VERSION,
] as const satisfies ReadonlyArray<ForensicContractSchemaId>;
