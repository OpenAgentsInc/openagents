import { Schema } from "effect";

export const DESKTOP_GRAPH_MEMORY_EVALUATION_SCHEMA_VERSION =
  "openagents.desktop.graph-memory-evaluation.v1" as const;
export const DESKTOP_GRAPH_MEMORY_FIXTURE_SCHEMA_VERSION =
  "openagents.desktop.graph-memory-evaluation-fixture.v1" as const;
export const DESKTOP_GRAPH_MEMORY_MANIFEST_SCHEMA_VERSION =
  "openagents.desktop.graph-memory-evaluation-manifest.v1" as const;
export const DESKTOP_GRAPH_MEMORY_SDK_TRAIN = "0.2.1-rc.2" as const;

const boundedRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(240));
const boundedText = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000));
const digest = Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/u));
const commitSha = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/u));
const nonNegativeInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0));
const positiveInteger = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1));
const finiteNonNegative = Schema.Number.check(Schema.isFinite(), Schema.isGreaterThanOrEqualTo(0));

export const GraphMemoryEvaluationChallengeClassSchema = Schema.Literals([
  "same_name_entities",
  "contradictory_mentions",
  "changed_fact",
  "revoked_source",
  "prompt_injection",
  "partial_extraction",
  "stale_graph",
]);
export type GraphMemoryEvaluationChallengeClass =
  typeof GraphMemoryEvaluationChallengeClassSchema.Type;

export const GraphMemoryEvaluationSourceFixtureSchema = Schema.Struct({
  sourceRef: boundedRef,
  text: boundedText,
  revoked: Schema.Boolean,
});
export interface GraphMemoryEvaluationSourceFixture extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationSourceFixtureSchema
> {}

export const GraphMemoryEvaluationFixtureRowSchema = Schema.Struct({
  rowId: boundedRef,
  challengeClasses: Schema.Array(GraphMemoryEvaluationChallengeClassSchema).check(
    Schema.isMinLength(1),
  ),
  query: boundedText,
  sources: Schema.Array(GraphMemoryEvaluationSourceFixtureSchema).check(Schema.isMinLength(1)),
  expectedAnswerFactRefs: Schema.Array(boundedRef).check(Schema.isMinLength(1)),
  expectedFactSupport: Schema.Array(
    Schema.Struct({
      factRef: boundedRef,
      supportingSourceRefs: Schema.Array(boundedRef).check(Schema.isMinLength(1)),
    }),
  ).check(Schema.isMinLength(1)),
  goldEntityRefs: Schema.Array(boundedRef).check(Schema.isMinLength(1)),
  entityAliases: Schema.Array(
    Schema.Struct({
      entityRef: boundedRef,
      aliases: Schema.Array(boundedText).check(Schema.isMinLength(1)),
    }),
  ).check(Schema.isMinLength(1)),
  goldDistinctEntityPairs: Schema.Array(Schema.Tuple([boundedRef, boundedRef])),
  relevantElementAliases: Schema.Array(boundedRef).check(Schema.isMinLength(1)),
  scenario: Schema.Struct({
    steps: Schema.Array(
      Schema.Struct({
        operation: Schema.Literals([
          "ingest",
          "revoke",
          "replace",
          "extract_partial",
          "snapshot_graph",
          "advance_graph",
        ]),
        sourceRef: Schema.NullOr(boundedRef),
      }),
    ).check(Schema.isMinLength(1)),
    expectedCaps: Schema.Array(boundedRef),
  }),
});
export interface GraphMemoryEvaluationFixtureRow extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationFixtureRowSchema
> {}

export const GraphMemoryEvaluationFixtureFileSchema = Schema.Struct({
  schemaVersion: Schema.Literal(DESKTOP_GRAPH_MEMORY_FIXTURE_SCHEMA_VERSION),
  split: Schema.Literals(["development", "holdout"]),
  rows: Schema.Array(GraphMemoryEvaluationFixtureRowSchema).check(Schema.isMinLength(1)),
});
export interface GraphMemoryEvaluationFixtureFile extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationFixtureFileSchema
> {}

export const GraphMemoryEvaluationManifestSchema = Schema.Struct({
  schemaVersion: Schema.Literal(DESKTOP_GRAPH_MEMORY_MANIFEST_SCHEMA_VERSION),
  datasetRef: boundedRef,
  reviewState: Schema.Literal("public_safe_synthetic"),
  developmentRef: Schema.Literal("development.json"),
  holdoutRef: Schema.Literal("holdout.json"),
  developmentDigest: digest,
  holdoutDigest: digest,
  datasetRevisionDigest: digest,
  requiredChallengeClasses: Schema.Array(GraphMemoryEvaluationChallengeClassSchema).check(
    Schema.isMinLength(7),
  ),
  qualityPolicy: Schema.Struct({
    minimumAnswerSupportImprovement: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    minimumRetrievalRecallImprovement: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    maximumCitationValidityRegression: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    maximumAnswerSupportRegression: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    maximumRetrievalPrecisionRegression: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    maximumRetrievalRecallRegression: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    maximumFalseMergeRate: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
    tieTolerance: Schema.Number.check(
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(0.1),
    ),
  }),
  qualityPolicyDigest: digest,
});
export interface GraphMemoryEvaluationManifest extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationManifestSchema
> {}

export const GraphMemoryEvaluationModelPinSchema = Schema.TaggedUnion({
  Available: {
    provider: boundedRef,
    model: boundedRef,
    modelArtifactDigest: digest,
  },
  Unavailable: {
    reason: Schema.Literals(["not_observed", "provider_refused", "usage_unavailable"]),
  },
  NotUsed: {
    reason: Schema.Literal("deterministic_evaluation"),
    requiredModelCalls: Schema.Literal(0),
    requiredInputTokens: Schema.Literal(0),
    requiredOutputTokens: Schema.Literal(0),
  },
});
export type GraphMemoryEvaluationModelPin = typeof GraphMemoryEvaluationModelPinSchema.Type;

export const GraphMemoryEvaluationSdkPackagePinSchema = Schema.Struct({
  package: Schema.Literals([
    "@openagentsinc/ai",
    "@openagentsinc/rlm",
    "@openagentsinc/history-corpus",
    "@openagentsinc/agent-harness-contract",
    "@openagentsinc/agent-runtime-schema",
    "@openagentsinc/dse",
    "@openagentsinc/graph-corpus",
    "@openagentsinc/conformance-kit",
  ]),
  version: Schema.Literal(DESKTOP_GRAPH_MEMORY_SDK_TRAIN),
  integrity: Schema.String.check(Schema.isPattern(/^sha512-[A-Za-z0-9+/]+={0,2}$/u)),
});

export const GraphMemoryEvaluationPinsSchema = Schema.Struct({
  sdkTrain: Schema.Literal(DESKTOP_GRAPH_MEMORY_SDK_TRAIN),
  sdkPackages: Schema.Array(GraphMemoryEvaluationSdkPackagePinSchema).check(Schema.isMinLength(8)),
  lockDigest: digest,
  openAgentsCommit: commitSha,
  sourceState: Schema.Literal("clean"),
  desktopBuildRef: boundedRef,
  desktopBuildDigest: digest,
  desktopBuildArtifacts: Schema.Array(
    Schema.Struct({
      ref: boundedRef,
      digest,
    }),
  ).check(Schema.isMinLength(1)),
  productWiringSmokeDigest: digest,
  runnerRef: boundedRef,
  runnerDigest: digest,
  oracleRef: boundedRef,
  oracleDigest: digest,
  runtime: Schema.Struct({
    node: boundedRef,
    platform: boundedRef,
    architecture: boundedRef,
  }),
  timingRef: boundedRef,
  model: GraphMemoryEvaluationModelPinSchema,
  parserRef: boundedRef,
  parserVersion: boundedRef,
  parserArtifactDigest: digest,
  datasetRevisionDigest: digest,
  developmentSplitDigest: digest,
  holdoutSplitDigest: digest,
  corpusDigest: digest,
  policyDigest: digest,
  budgetDigest: digest,
  qualityPolicyDigest: digest,
});
export interface GraphMemoryEvaluationPins extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationPinsSchema
> {}

export const GraphMemoryEvaluationOutcomeSchema = Schema.Literals([
  "complete",
  "partial",
  "refused",
  "failed",
  "inconclusive",
]);
export type GraphMemoryEvaluationOutcome = typeof GraphMemoryEvaluationOutcomeSchema.Type;

export const GraphMemoryEvaluationTokenUsageSchema = Schema.TaggedUnion({
  Exact: {
    inputTokens: nonNegativeInteger,
    outputTokens: nonNegativeInteger,
    totalTokens: nonNegativeInteger,
  },
  Unavailable: {
    reason: Schema.Literals(["not_reported", "mixed_truth", "not_run"]),
  },
});
export type GraphMemoryEvaluationTokenUsage = typeof GraphMemoryEvaluationTokenUsageSchema.Type;

export const GraphMemoryEvaluationArmRowSchema = Schema.Struct({
  rowId: boundedRef,
  arm: Schema.Literals(["history_only", "graph_assisted"]),
  outcome: GraphMemoryEvaluationOutcomeSchema,
  inputDigest: digest,
  corpusDigest: digest,
  queryDigest: digest,
  policyDigest: digest,
  budgetDigest: digest,
  modelCalls: nonNegativeInteger,
  extractionEvidence: Schema.Struct({
    status: Schema.Literals(["complete", "partial", "refused", "failed", "not_run"]),
    receiptDigest: Schema.NullOr(digest),
    usageTruth: Schema.Literals(["exact", "unavailable", "not_run"]),
    inputCorpusDigest: Schema.NullOr(digest),
    budgetDigest: Schema.NullOr(digest),
    graphStateDigest: Schema.NullOr(digest),
    entityCount: nonNegativeInteger,
    mergeCount: nonNegativeInteger,
  }),
  citationEvidence: Schema.Struct({
    validationDigest: Schema.NullOr(digest),
    invalidCount: nonNegativeInteger,
  }),
  emittedAnswerFactRefs: Schema.Array(boundedRef),
  emittedCitationRefs: Schema.Array(boundedRef),
  validCitationRefs: Schema.Array(boundedRef),
  mergedEntityPairs: Schema.Array(Schema.Tuple([boundedRef, boundedRef])),
  observedEntityRefs: Schema.Array(boundedRef),
  retrievedSourceRefs: Schema.Array(boundedRef),
  retrievedElementAliases: Schema.Array(boundedRef),
  retrievalEvidence: Schema.Struct({
    mappingDigest: digest,
    mappings: Schema.Array(
      Schema.Struct({
        observedElementDigest: digest,
        oracleElementAlias: boundedRef,
      }),
    ),
  }),
  recallLatencySamplesMs: Schema.Array(finiteNonNegative).check(Schema.isMinLength(3)),
  setupLatencyMs: Schema.NullOr(finiteNonNegative),
  tokens: GraphMemoryEvaluationTokenUsageSchema,
  truncated: Schema.Boolean,
  hitCaps: Schema.Array(boundedRef),
});
export interface GraphMemoryEvaluationArmRow extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationArmRowSchema
> {}

export const GraphMemoryEvaluationFractionMetricSchema = Schema.Struct({
  status: Schema.Literals(["supported", "unsupported"]),
  numerator: nonNegativeInteger,
  denominator: nonNegativeInteger,
  value: Schema.NullOr(
    Schema.Number.check(
      Schema.isFinite(),
      Schema.isGreaterThanOrEqualTo(0),
      Schema.isLessThanOrEqualTo(1),
    ),
  ),
  reason: Schema.NullOr(boundedRef),
});
export interface GraphMemoryEvaluationFractionMetric extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationFractionMetricSchema
> {}

export const GraphMemoryEvaluationLatencyMetricSchema = Schema.Struct({
  samples: nonNegativeInteger,
  p50Ms: Schema.NullOr(finiteNonNegative),
  p95Ms: Schema.NullOr(finiteNonNegative),
});
export interface GraphMemoryEvaluationLatencyMetric extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationLatencyMetricSchema
> {}

export const GraphMemoryEvaluationUsageSummarySchema = Schema.Struct({
  truth: Schema.Literals(["exact", "unavailable"]),
  exactRows: nonNegativeInteger,
  unavailableRows: nonNegativeInteger,
  inputTokens: Schema.NullOr(nonNegativeInteger),
  outputTokens: Schema.NullOr(nonNegativeInteger),
  totalTokens: Schema.NullOr(nonNegativeInteger),
});
export interface GraphMemoryEvaluationUsageSummary extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationUsageSummarySchema
> {}

export const GraphMemoryEvaluationArmSummarySchema = Schema.Struct({
  arm: Schema.Literals(["history_only", "graph_assisted"]),
  rows: positiveInteger,
  outcomes: Schema.Struct({
    complete: nonNegativeInteger,
    partial: nonNegativeInteger,
    refused: nonNegativeInteger,
    failed: nonNegativeInteger,
    inconclusive: nonNegativeInteger,
  }),
  citationValidity: GraphMemoryEvaluationFractionMetricSchema,
  answerSupport: GraphMemoryEvaluationFractionMetricSchema,
  falseMergeRate: GraphMemoryEvaluationFractionMetricSchema,
  missedEntityRate: GraphMemoryEvaluationFractionMetricSchema,
  retrievalPrecision: GraphMemoryEvaluationFractionMetricSchema,
  retrievalRecall: GraphMemoryEvaluationFractionMetricSchema,
  latency: GraphMemoryEvaluationLatencyMetricSchema,
  usage: GraphMemoryEvaluationUsageSummarySchema,
  truncation: Schema.Struct({
    rows: nonNegativeInteger,
    hitCaps: Schema.Array(boundedRef),
  }),
});
export interface GraphMemoryEvaluationArmSummary extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationArmSummarySchema
> {}

export const GraphMemoryEvaluationQualityResultSchema = Schema.Literals([
  "improved",
  "neutral",
  "regressed",
  "inconclusive",
]);
export type GraphMemoryEvaluationQualityResult =
  typeof GraphMemoryEvaluationQualityResultSchema.Type;

export const GraphMemoryEvaluationDispositionSchema = Schema.Struct({
  implementation: Schema.Literals(["implemented", "not_implemented"]),
  evidence: Schema.Literals(["present", "partial", "absent"]),
  quality: GraphMemoryEvaluationQualityResultSchema,
  ownerReview: Schema.Literals(["unreviewed", "reviewed_accepted", "reviewed_rejected"]),
  release: Schema.Literals(["not_released", "release_candidate", "released"]),
  publicClaim: Schema.Literals(["not_authorized", "authorized"]),
});
export interface GraphMemoryEvaluationDisposition extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationDispositionSchema
> {}

export const DesktopGraphMemoryEvaluationReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal(DESKTOP_GRAPH_MEMORY_EVALUATION_SCHEMA_VERSION),
  issue: Schema.Literal("OA-GMEM-04"),
  evaluatedAt: boundedRef,
  pins: GraphMemoryEvaluationPinsSchema,
  dataset: Schema.Struct({
    datasetRef: boundedRef,
    reviewState: Schema.Literal("public_safe_synthetic"),
    developmentRows: positiveInteger,
    holdoutRows: positiveInteger,
    physicalHoldoutIsolation: Schema.Literal(true),
    splitIdentityOverlap: Schema.Literal(0),
    challengeClasses: Schema.Array(GraphMemoryEvaluationChallengeClassSchema).check(
      Schema.isMinLength(7),
    ),
  }),
  rowEvidence: Schema.Array(
    Schema.Struct({
      rowId: boundedRef,
      inputDigest: digest,
      corpusDigest: digest,
      queryDigest: digest,
      policyDigest: digest,
      budgetDigest: digest,
      historyOnly: Schema.Struct({
        outcome: GraphMemoryEvaluationOutcomeSchema,
        modelCalls: nonNegativeInteger,
        extractionEvidence: GraphMemoryEvaluationArmRowSchema.fields.extractionEvidence,
        citationEvidence: GraphMemoryEvaluationArmRowSchema.fields.citationEvidence,
      }),
      graphAssisted: Schema.Struct({
        outcome: GraphMemoryEvaluationOutcomeSchema,
        modelCalls: nonNegativeInteger,
        extractionEvidence: GraphMemoryEvaluationArmRowSchema.fields.extractionEvidence,
        citationEvidence: GraphMemoryEvaluationArmRowSchema.fields.citationEvidence,
      }),
    }),
  ).check(Schema.isMinLength(1)),
  privateDetailReceiptDigest: digest,
  historyOnly: GraphMemoryEvaluationArmSummarySchema,
  graphAssisted: GraphMemoryEvaluationArmSummarySchema,
  comparison: Schema.Struct({
    quality: GraphMemoryEvaluationQualityResultSchema,
    reasons: Schema.Array(boundedRef).check(Schema.isMinLength(1)),
  }),
  disposition: GraphMemoryEvaluationDispositionSchema,
});
export interface DesktopGraphMemoryEvaluationReceipt extends Schema.Schema.Type<
  typeof DesktopGraphMemoryEvaluationReceiptSchema
> {}

export const GraphMemoryEvaluationInputSchema = Schema.Struct({
  evaluatedAt: boundedRef,
  expectedPins: GraphMemoryEvaluationPinsSchema,
  observedPins: GraphMemoryEvaluationPinsSchema,
  development: GraphMemoryEvaluationFixtureFileSchema,
  holdout: GraphMemoryEvaluationFixtureFileSchema,
  manifest: GraphMemoryEvaluationManifestSchema,
  historyOnlyRows: Schema.Array(GraphMemoryEvaluationArmRowSchema).check(Schema.isMinLength(1)),
  graphAssistedRows: Schema.Array(GraphMemoryEvaluationArmRowSchema).check(Schema.isMinLength(1)),
});
export interface GraphMemoryEvaluationInput extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationInputSchema
> {}

export const GraphMemoryEvaluationRefusalSchema = Schema.Struct({
  ok: Schema.Literal(false),
  reason: Schema.Literals([
    "invalid_input",
    "pin_mismatch",
    "split_mismatch",
    "split_digest_mismatch",
    "split_identity_overlap",
    "challenge_coverage_missing",
    "fixture_semantics_invalid",
    "row_result_mismatch",
  ]),
  detailSafe: boundedText,
});
export interface GraphMemoryEvaluationRefusal extends Schema.Schema.Type<
  typeof GraphMemoryEvaluationRefusalSchema
> {}

export type GraphMemoryEvaluationResult =
  | Readonly<{ ok: true; receipt: DesktopGraphMemoryEvaluationReceipt }>
  | GraphMemoryEvaluationRefusal;
