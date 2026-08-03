import { Schema as S } from "effect";

import {
  BoundedRefs,
  BoundedShortTexts,
  DurationTruthSchema,
  Exactness,
  ForensicRef,
  ForensicTimestamp,
  LongText,
  NonEmptyBoundedRefs,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
  ShortText,
  UsageTruthSchema,
} from "./primitives.ts";
import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import { evaluateForensicEventSequence } from "./lifecycle.ts";
import { ForensicRunEventSchema, type ForensicRunEvent } from "./run.ts";

export const FORENSIC_METRIC_DEFINITION_VERSION =
  "openagents.forensic_metric_definition.v1" as const;
export const FORENSIC_SCORECARD_VERSION = "openagents.forensic_scorecard.v1" as const;
export const FORENSIC_PROMPT_PROMOTION_VERSION = "openagents.forensic_prompt_promotion.v1" as const;
export const FORENSIC_METRIC_REGISTRY_VERSION = "openagents.forensic_metric_registry.v1" as const;
export const FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION =
  "openagents.forensic_provider_usage_receipt.v1" as const;
export const FORENSIC_EVALUATOR_ADJUDICATION_VERSION =
  "openagents.forensic_evaluator_adjudication.v1" as const;
export const FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION =
  "openagents.forensic_reviewer_burden_receipt.v1" as const;

export const ForensicMetricDefinitionSchema = S.Struct({
  schema: S.Literal(FORENSIC_METRIC_DEFINITION_VERSION),
  metricRef: ForensicRef,
  name: ShortText,
  formula: LongText,
  unit: S.Literals([
    "boolean",
    "count",
    "ratio",
    "milliseconds",
    "tokens",
    "usd_micros",
    "bytes",
    "transactions_per_second",
    "matches_per_million",
  ]),
  eligiblePopulationRef: ForensicRef,
  censorPolicy: LongText,
  missPolicy: LongText,
  exactnessPolicy: LongText,
  dimensionRefs: BoundedRefs,
  aggregation: S.Literals([
    "none",
    "sum",
    "mean",
    "median",
    "percentile",
    "survival_estimator",
    "confidence_interval",
  ]),
  displayMetadataRefs: BoundedRefs,
  revisionDigest: Sha256Digest,
  createdAt: ForensicTimestamp,
}).annotate({ identifier: "ForensicMetricDefinition" });
export interface ForensicMetricDefinition extends S.Schema.Type<
  typeof ForensicMetricDefinitionSchema
> {}

export const ForensicMetricRegistrySchema = S.Struct({
  schema: S.Literal(FORENSIC_METRIC_REGISTRY_VERSION),
  registryRef: ForensicRef,
  definitions: S.Array(ForensicMetricDefinitionSchema).check(S.isMinLength(1), S.isMaxLength(512)),
  revisionDigest: Sha256Digest,
  frozenAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (registry) =>
          new Set(registry.definitions.map((definition) => definition.metricRef)).size ===
          registry.definitions.length,
        { message: "metric registry definitions must have unique refs" },
      ),
      S.makeFilter(
        (registry) => registry.revisionDigest === forensicSha256Digest(registry.definitions),
        { message: "metric registry revision must match its frozen definitions" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicMetricRegistry" });
export interface ForensicMetricRegistry extends S.Schema.Type<
  typeof ForensicMetricRegistrySchema
> {}

export const ForensicProviderUsageReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION),
  receiptRef: ForensicRef,
  runRef: ForensicRef,
  turnRef: ForensicRef,
  role: S.Literals(["discovery", "verification", "judge"]),
  attempt: PositiveInteger,
  startEventRef: ForensicRef,
  startEventSequence: PositiveInteger,
  settledEventSequence: PositiveInteger,
  abandoned: S.Boolean,
  losingParallelArm: S.Boolean,
  usage: UsageTruthSchema,
  costMicros: S.optionalKey(NonNegativeInteger),
  recordedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter((receipt) => receipt.settledEventSequence >= receipt.startEventSequence, {
        message: "usage settlement cannot precede turn start",
      }),
      S.makeFilter(
        (receipt) => receipt.usage.exactness === "unavailable" || receipt.costMicros !== undefined,
        { message: "available provider usage requires a measured provider cost" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicProviderUsageReceipt" });
export interface ForensicProviderUsageReceipt extends S.Schema.Type<
  typeof ForensicProviderUsageReceiptSchema
> {}

export const ForensicEvaluatorAdjudicationSchema = S.Struct({
  schema: S.Literal(FORENSIC_EVALUATOR_ADJUDICATION_VERSION),
  adjudicationRef: ForensicRef,
  runRef: ForensicRef,
  findingRef: ForensicRef,
  findingEventRef: ForensicRef,
  findingEventDigest: Sha256Digest,
  evaluatorRevisionDigest: Sha256Digest,
  outcome: S.Literals(["qualified", "rejected", "duplicate", "not_applicable"]),
  vulnerabilityIdentityDigest: S.optionalKey(Sha256Digest),
  requiredCausalLinks: NonNegativeInteger,
  supportedCausalLinks: NonNegativeInteger,
  submittedSourceRefs: NonNegativeInteger,
  validSourceRefs: NonNegativeInteger,
  reasonRefs: NonEmptyBoundedRefs,
  evaluatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (adjudication) =>
          adjudication.supportedCausalLinks <= adjudication.requiredCausalLinks &&
          adjudication.validSourceRefs <= adjudication.submittedSourceRefs,
        { message: "adjudication satisfied counts cannot exceed frozen requirements" },
      ),
      S.makeFilter(
        (adjudication) =>
          adjudication.outcome !== "qualified" ||
          (adjudication.vulnerabilityIdentityDigest !== undefined &&
            adjudication.requiredCausalLinks > 0 &&
            adjudication.supportedCausalLinks === adjudication.requiredCausalLinks &&
            adjudication.submittedSourceRefs > 0 &&
            adjudication.validSourceRefs === adjudication.submittedSourceRefs),
        {
          message:
            "qualified adjudications require an identity, every causal link, and valid source refs",
        },
      ),
    ),
  )
  .annotate({ identifier: "ForensicEvaluatorAdjudication" });
export interface ForensicEvaluatorAdjudication extends S.Schema.Type<
  typeof ForensicEvaluatorAdjudicationSchema
> {}

const ReviewerDurationSchema = S.Union([
  S.Struct({
    milliseconds: NonNegativeInteger,
    exactness: S.Literals(["exact", "estimated", "upper_bound"]),
  }),
  S.Struct({
    exactness: S.Literal("unavailable"),
    unavailableReasonRef: ForensicRef,
  }),
]);

export const ForensicReviewerBurdenReceiptSchema = S.Struct({
  schema: S.Literal(FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION),
  receiptRef: ForensicRef,
  runRef: ForensicRef,
  reviewerActorRef: ForensicRef,
  reviewEventRef: ForensicRef,
  reviewEventSequence: PositiveInteger,
  duration: ReviewerDurationSchema,
  correctionCount: NonNegativeInteger,
  rejectionCount: NonNegativeInteger,
  reasonRefs: NonEmptyBoundedRefs,
  recordedAt: ForensicTimestamp,
}).annotate({ identifier: "ForensicReviewerBurdenReceipt" });
export interface ForensicReviewerBurdenReceipt extends S.Schema.Type<
  typeof ForensicReviewerBurdenReceiptSchema
> {}

export const MetricValueSchema = S.Struct({
  metricRef: ForensicRef,
  numericValue: S.optionalKey(S.Number.check(S.isFinite())),
  booleanValue: S.optionalKey(S.Boolean),
  exactness: Exactness,
  unavailableReasonRef: S.optionalKey(ForensicRef),
  sourceEventRefs: BoundedRefs,
  sourceReceiptRefs: BoundedRefs,
})
  .pipe(
    S.check(
      S.makeFilter(
        (value) => {
          const availableValues =
            Number(value.numericValue !== undefined) + Number(value.booleanValue !== undefined);
          if (value.exactness === "unavailable") {
            return availableValues === 0 && value.unavailableReasonRef !== undefined;
          }
          return availableValues === 1 && value.unavailableReasonRef === undefined;
        },
        { message: "metric values require exactly one value, or an unavailable reason" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicMetricValue" });
export interface MetricValue extends S.Schema.Type<typeof MetricValueSchema> {}

export const HardGateResultSchema = S.Struct({
  gateRef: ForensicRef,
  passed: S.Boolean,
  evidenceRefs: BoundedRefs,
  blockerRefs: BoundedRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (gate) => (gate.passed ? gate.evidenceRefs.length > 0 : gate.blockerRefs.length > 0),
      { message: "hard gates require evidence when passed and blockers when failed" },
    ),
  ),
);
export interface HardGateResult extends S.Schema.Type<typeof HardGateResultSchema> {}

export const ScorecardPopulation = S.Literals([
  "incomplete",
  "vulnerable",
  "structural_variant",
  "fixed_control",
  "clean_control",
]);
export type ScorecardPopulation = typeof ScorecardPopulation.Type;

export const ScorecardOutcome = S.Literals(["hit", "miss", "not_eligible", "failed", "cancelled"]);
export type ScorecardOutcome = typeof ScorecardOutcome.Type;

export const ScorecardRunSchema = S.Struct({
  runDigest: Sha256Digest,
  armRef: ForensicRef,
  datasetSplit: S.Literals(["train", "development", "holdout", "clean_holdout"]),
  population: ScorecardPopulation,
  coverageStatus: S.Literals(["complete", "incomplete", "denied"]),
  outcome: ScorecardOutcome,
  eligibleForIdentification: S.Boolean,
  censored: S.Boolean,
  miss: S.Boolean,
  censorAt: S.optionalKey(DurationTruthSchema),
  spentUsage: UsageTruthSchema,
  qualifiedFindingEventRef: S.optionalKey(ForensicRef),
  qualifiedFindingObservedAt: S.optionalKey(ForensicTimestamp),
  values: S.Array(MetricValueSchema).check(S.isMaxLength(512)),
  failureRefs: BoundedRefs,
}).pipe(
  S.check(
    S.makeFilter(
      (run) => {
        const expectedEligibility =
          run.coverageStatus === "complete" &&
          ["vulnerable", "structural_variant"].includes(run.population) &&
          ["development", "holdout"].includes(run.datasetSplit);
        return run.eligibleForIdentification === expectedEligibility;
      },
      {
        message:
          "identification eligibility is limited to complete vulnerable or structural development/holdout runs",
      },
    ),
    S.makeFilter(
      (run) =>
        run.miss === (run.outcome === "miss") &&
        (!run.miss ||
          (run.eligibleForIdentification && run.censored && run.censorAt !== undefined)),
      { message: "misses must remain eligible right-censored observations" },
    ),
    S.makeFilter(
      (run) =>
        run.censored === (run.censorAt !== undefined) &&
        (!run.censored || run.censorAt?.milliseconds !== 0),
      { message: "censored observations require a nonzero censor boundary" },
    ),
    S.makeFilter(
      (run) =>
        run.outcome !== "hit" ||
        (run.eligibleForIdentification &&
          !run.censored &&
          run.qualifiedFindingEventRef !== undefined &&
          run.qualifiedFindingObservedAt !== undefined),
      { message: "hits require the immutable qualified finding event and timestamp" },
    ),
    S.makeFilter(
      (run) =>
        (run.qualifiedFindingEventRef === undefined) ===
        (run.qualifiedFindingObservedAt === undefined),
      { message: "qualified finding ref and immutable timestamp must appear together" },
    ),
    S.makeFilter((run) => run.population !== "incomplete" || run.coverageStatus !== "complete", {
      message: "incomplete populations cannot carry complete coverage",
    }),
    S.makeFilter(
      (run) => run.population !== "clean_control" || run.datasetSplit === "clean_holdout",
      { message: "clean controls must remain in the clean-holdout population" },
    ),
  ),
);
export interface ScorecardRun extends S.Schema.Type<typeof ScorecardRunSchema> {}

export const ScorecardPopulationGroupSchema = S.Struct({
  datasetSplit: S.Literals(["train", "development", "holdout", "clean_holdout"]),
  population: ScorecardPopulation,
  runCount: PositiveInteger,
  hitCount: NonNegativeInteger,
  missCount: NonNegativeInteger,
  censorCount: NonNegativeInteger,
}).pipe(
  S.check(
    S.makeFilter(
      (group) =>
        group.hitCount + group.missCount <= group.runCount && group.censorCount <= group.runCount,
      { message: "population aggregate counts cannot exceed their isolated run population" },
    ),
  ),
);
export interface ScorecardPopulationGroup extends S.Schema.Type<
  typeof ScorecardPopulationGroupSchema
> {}

export const ScorecardCostTruthSchema = S.Union([
  S.Struct({
    micros: NonNegativeInteger,
    exactness: S.Literals(["exact", "estimated", "upper_bound"]),
  }),
  S.Struct({
    exactness: S.Literal("unavailable"),
    unavailableReasonRef: ForensicRef,
  }),
]);
export type ScorecardCostTruth = typeof ScorecardCostTruthSchema.Type;

export const ForensicScorecardSchema = S.Struct({
  schema: S.Literal(FORENSIC_SCORECARD_VERSION),
  scorecardRef: ForensicRef,
  datasetRevisionDigest: Sha256Digest,
  metricDefinitionRevisionDigest: Sha256Digest,
  evaluatorRevisionDigest: Sha256Digest,
  candidateDigest: Sha256Digest,
  hardGates: S.Array(HardGateResultSchema).check(S.isMinLength(1), S.isMaxLength(256)),
  runs: S.Array(ScorecardRunSchema).check(S.isMinLength(1), S.isMaxLength(100_000)),
  populationGroups: S.Array(ScorecardPopulationGroupSchema).check(
    S.isMinLength(1),
    S.isMaxLength(20),
  ),
  distributionRefs: BoundedRefs,
  censorCount: NonNegativeInteger,
  missCount: NonNegativeInteger,
  confidenceIntervalRefs: BoundedRefs,
  cost: ScorecardCostTruthSchema,
  eventDigest: Sha256Digest,
  receiptDigest: Sha256Digest,
  generatedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (scorecard) =>
          scorecard.censorCount === scorecard.runs.filter((run) => run.censored).length &&
          scorecard.missCount === scorecard.runs.filter((run) => run.miss).length,
        { message: "scorecard censor and miss counts must match retained runs" },
      ),
      S.makeFilter(
        (scorecard) =>
          new Set(scorecard.runs.map((run) => run.runDigest)).size === scorecard.runs.length &&
          new Set(scorecard.hardGates.map((gate) => gate.gateRef)).size ===
            scorecard.hardGates.length &&
          scorecard.runs.every(
            (run) => new Set(run.values.map((value) => value.metricRef)).size === run.values.length,
          ),
        { message: "scorecards cannot duplicate runs, gates, or per-run metric values" },
      ),
      S.makeFilter(
        (scorecard) => {
          const expected = new Map<string, ScorecardPopulationGroup>();
          for (const run of scorecard.runs) {
            const key = `${run.datasetSplit}:${run.population}`;
            const group = expected.get(key) ?? {
              datasetSplit: run.datasetSplit,
              population: run.population,
              runCount: 0,
              hitCount: 0,
              missCount: 0,
              censorCount: 0,
            };
            expected.set(key, {
              ...group,
              runCount: group.runCount + 1,
              hitCount: group.hitCount + Number(run.outcome === "hit"),
              missCount: group.missCount + Number(run.miss),
              censorCount: group.censorCount + Number(run.censored),
            });
          }
          return (
            scorecard.populationGroups.length === expected.size &&
            scorecard.populationGroups.every((group) => {
              const expectedGroup = expected.get(`${group.datasetSplit}:${group.population}`);
              return (
                expectedGroup !== undefined &&
                group.runCount === expectedGroup.runCount &&
                group.hitCount === expectedGroup.hitCount &&
                group.missCount === expectedGroup.missCount &&
                group.censorCount === expectedGroup.censorCount
              );
            })
          );
        },
        {
          message:
            "scorecard populations must be complete, isolated, and derived from retained runs",
        },
      ),
    ),
  )
  .annotate({ identifier: "ForensicScorecard" });
export interface ForensicScorecard extends S.Schema.Type<typeof ForensicScorecardSchema> {}

export const ForensicPromptPromotionSchema = S.Struct({
  schema: S.Literal(FORENSIC_PROMPT_PROMOTION_VERSION),
  promotionRef: ForensicRef,
  candidatePromptDigest: Sha256Digest,
  candidateProducerRef: ForensicRef,
  evaluatorRef: ForensicRef,
  scorecardRef: ForensicRef,
  baselineScorecardRef: ForensicRef,
  releaseGateRef: ForensicRef,
  hardGateRefs: NonEmptyBoundedRefs,
  allHardGatesPassed: S.Boolean,
  paretoStatus: S.Literals(["dominates", "non_dominated", "dominated", "incomparable"]),
  qualityRegressionMetricRefs: BoundedRefs,
  efficiencyImprovementMetricRefs: BoundedRefs,
  holdoutDatasetDigest: Sha256Digest,
  cleanHoldoutDatasetDigest: Sha256Digest,
  holdoutEvidenceRefs: NonEmptyBoundedRefs,
  operatorDecisionRef: ForensicRef,
  operatorRef: ForensicRef,
  decision: S.Literals(["admitted", "rejected", "rolled_back"]),
  priorActivePromptDigest: Sha256Digest,
  nextActivePromptDigest: Sha256Digest,
  rollbackPromptDigest: Sha256Digest,
  reasonRefs: BoundedRefs,
  decidedAt: ForensicTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (promotion) =>
          promotion.candidateProducerRef !== promotion.evaluatorRef &&
          promotion.candidateProducerRef !== promotion.operatorRef,
        { message: "prompt candidates cannot evaluate or promote themselves" },
      ),
      S.makeFilter(
        (promotion) =>
          promotion.decision !== "admitted" ||
          (promotion.allHardGatesPassed &&
            ["dominates", "non_dominated"].includes(promotion.paretoStatus) &&
            promotion.qualityRegressionMetricRefs.length === 0 &&
            promotion.nextActivePromptDigest === promotion.candidatePromptDigest),
        {
          message:
            "admission requires hard gates, no quality regression, a Pareto-safe result, and candidate activation",
        },
      ),
    ),
  )
  .annotate({ identifier: "ForensicPromptPromotion" });
export interface ForensicPromptPromotion extends S.Schema.Type<
  typeof ForensicPromptPromotionSchema
> {}

export const ScorecardSummarySchema = S.Struct({
  scorecardDigest: Sha256Digest,
  candidateDigest: Sha256Digest,
  allHardGatesPassed: S.Boolean,
  metricRefs: BoundedRefs,
  caveats: BoundedShortTexts,
});
export interface ScorecardSummary extends S.Schema.Type<typeof ScorecardSummarySchema> {}

export const QUALIFIED_HIT_METRIC_REF = "metric.qualified_hit.v1" as const;
export const ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF =
  "metric.analysis_time_to_identification.v1" as const;
export const TOKENS_TO_IDENTIFICATION_METRIC_REF = "metric.tokens_to_identification.v1" as const;
export const TOTAL_RUN_TOKENS_METRIC_REF = "metric.total_run_tokens.v1" as const;
export const CONTROL_FALSE_POSITIVE_METRIC_REF = "metric.control_false_positive.v1" as const;
export const REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF =
  "metric.reviewer_minutes_per_qualified_finding.v1" as const;
export const CORRECTION_REJECTION_BURDEN_METRIC_REF =
  "metric.correction_rejection_burden.v1" as const;

const REQUIRED_PROJECTOR_METRICS = [
  QUALIFIED_HIT_METRIC_REF,
  ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF,
  TOKENS_TO_IDENTIFICATION_METRIC_REF,
  TOTAL_RUN_TOKENS_METRIC_REF,
  CONTROL_FALSE_POSITIVE_METRIC_REF,
  REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF,
  CORRECTION_REJECTION_BURDEN_METRIC_REF,
] as const;

type FrozenMetricSpec = readonly [
  metricRef: string,
  name: string,
  unit: ForensicMetricDefinition["unit"],
  populationRef: string,
  aggregation: ForensicMetricDefinition["aggregation"],
  formula: string,
];

const FROZEN_METRIC_SPECS: ReadonlyArray<FrozenMetricSpec> = [
  [
    "metric.provision_latency.v1",
    "Provision latency",
    "milliseconds",
    "population.all_admitted",
    "percentile",
    "T1 worker_ready minus T0 request_accepted on the control-plane server clock.",
  ],
  [
    "metric.input_readiness_latency.v1",
    "Input readiness latency",
    "milliseconds",
    "population.all_provisioned",
    "percentile",
    "T2 coverage_ready minus T1 worker_ready on the control-plane server clock.",
  ],
  [
    "metric.time_to_first_hypothesis.v1",
    "Time to first hypothesis",
    "milliseconds",
    "population.all_started",
    "percentile",
    "T4 first typed hypothesis minus T3 analysis_started; diagnostic only.",
  ],
  [
    ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF,
    "Analysis time to qualified identification",
    "milliseconds",
    "population.complete_vulnerable",
    "survival_estimator",
    "Immutable finding event accepted by the frozen evaluator (T5) minus T3 analysis_started.",
  ],
  [
    "metric.end_to_end_time_to_identification.v1",
    "End-to-end time to identification",
    "milliseconds",
    "population.complete_vulnerable",
    "survival_estimator",
    "Immutable qualified finding time T5 minus T0 request_accepted.",
  ],
  [
    "metric.time_to_verification.v1",
    "Time to verification",
    "milliseconds",
    "population.qualified_findings",
    "percentile",
    "T6 verification_observed minus immutable T5.",
  ],
  [
    "metric.time_to_reviewed_finding.v1",
    "Time to reviewed finding",
    "milliseconds",
    "population.qualified_findings",
    "percentile",
    "T7 review_recorded minus T0 request_accepted.",
  ],
  [
    "metric.cleanup_latency.v1",
    "Cleanup latency",
    "milliseconds",
    "population.cleanup_requested",
    "percentile",
    "T8 cleanup_observed minus cleanup_requested.",
  ],
  [
    "metric.first_priority_tranche_hit.v1",
    "First-priority-tranche hit",
    "boolean",
    "population.complete_vulnerable",
    "mean",
    "Whether immutable T5 occurs before the first declared priority tranche closes.",
  ],
  [
    "metric.work_fraction_to_identification.v1",
    "Work fraction to identification",
    "ratio",
    "population.complete_vulnerable",
    "percentile",
    "Ranked tranches and focal sessions started through T5 divided by the frozen scan plan.",
  ],
  [
    QUALIFIED_HIT_METRIC_REF,
    "Qualified hit",
    "boolean",
    "population.complete_vulnerable",
    "mean",
    "Whether a run contains a typed finding accepted by the frozen evaluator with complete causal and reference evidence.",
  ],
  [
    "metric.finding_precision.v1",
    "Finding precision",
    "ratio",
    "population.adjudicated_claims",
    "confidence_interval",
    "Unique qualified vulnerability identities divided by all unique adjudicated vulnerability identities.",
  ],
  [
    "metric.recall_at_fixed_time.v1",
    "Recall at fixed time",
    "ratio",
    "population.complete_vulnerable",
    "confidence_interval",
    "Frozen benchmark vulnerabilities qualified before a preregistered elapsed-time budget divided by all benchmark vulnerabilities.",
  ],
  [
    "metric.recall_at_fixed_tokens.v1",
    "Recall at fixed tokens",
    "ratio",
    "population.complete_vulnerable",
    "confidence_interval",
    "Frozen benchmark vulnerabilities qualified before a preregistered aggregate-token budget divided by all benchmark vulnerabilities.",
  ],
  [
    "metric.causal_chain_coverage.v1",
    "Causal-chain coverage",
    "ratio",
    "population.adjudicated_claims",
    "mean",
    "Frozen causal links supported by valid evidence divided by all required links.",
  ],
  [
    "metric.evidence_reference_validity.v1",
    "Evidence-reference validity",
    "ratio",
    "population.submitted_references",
    "mean",
    "Resolvable target-bound source and artifact refs divided by all submitted refs.",
  ],
  [
    "metric.verification_rate.v1",
    "Verification rate",
    "ratio",
    "population.qualified_findings",
    "confidence_interval",
    "Unique qualified identities with admitted independent verification divided by qualified identities.",
  ],
  [
    "metric.clean_control_false_positive_rate.v1",
    "Clean-control false-positive rate",
    "ratio",
    "population.clean_control",
    "confidence_interval",
    "Clean-control runs receiving a qualified vulnerability identity divided by all clean-control runs.",
  ],
  [
    CONTROL_FALSE_POSITIVE_METRIC_REF,
    "Control false positive",
    "boolean",
    "population.fixed_or_clean_control",
    "mean",
    "Whether a fixed or clean control receives any qualified vulnerability identity.",
  ],
  [
    "metric.fixed_control_regression_rate.v1",
    "Fixed-control regression rate",
    "ratio",
    "population.fixed_control",
    "confidence_interval",
    "Fixed-control runs reporting the historical vulnerability divided by all fixed-control runs.",
  ],
  [
    "metric.duplicate_burden.v1",
    "Duplicate burden",
    "ratio",
    "population.submitted_findings",
    "mean",
    "Duplicate submitted findings divided by all submitted findings after deterministic identity grouping.",
  ],
  [
    "metric.calibration.v1",
    "Claim calibration",
    "ratio",
    "population.adjudicated_claims",
    "confidence_interval",
    "Agreement between typed claim state and frozen later adjudication.",
  ],
  [
    "metric.generalization_gap.v1",
    "Generalization gap",
    "ratio",
    "population.matched_development_holdout",
    "confidence_interval",
    "Development qualified-hit rate minus separately owned blinded-holdout hit rate.",
  ],
  [
    "metric.run_stability.v1",
    "Run stability",
    "ratio",
    "population.matched_repetitions",
    "confidence_interval",
    "Agreement of qualified identities and causal links across matched repetitions.",
  ],
  [
    TOKENS_TO_IDENTIFICATION_METRIC_REF,
    "Tokens to identification",
    "tokens",
    "population.complete_vulnerable",
    "survival_estimator",
    "All provider tokens for every turn started through immutable T5, including retries and losing parallel arms.",
  ],
  [
    TOTAL_RUN_TOKENS_METRIC_REF,
    "Total run tokens",
    "tokens",
    "population.all_runs",
    "percentile",
    "All provider tokens through structural settlement, including failed, retried, abandoned, verifier, judge, and parallel turns.",
  ],
  [
    "metric.tokens_per_unique_qualified_finding.v1",
    "Tokens per unique qualified finding",
    "tokens",
    "population.all_runs",
    "percentile",
    "Total run tokens divided by unique qualified identities; zero-yield runs retain spent tokens and zero yield.",
  ],
  [
    "metric.post_identification_tokens.v1",
    "Post-identification tokens",
    "tokens",
    "population.runs_with_t5",
    "percentile",
    "Provider tokens attributable after immutable T5, retaining provider exactness.",
  ],
  [
    "metric.cache_utilization.v1",
    "Cache utilization",
    "ratio",
    "population.cache_eligible_turns",
    "mean",
    "Provider-reported cached input tokens divided by cache-eligible input tokens.",
  ],
  [
    "metric.cost_to_identification.v1",
    "Cost to identification",
    "usd_micros",
    "population.complete_vulnerable",
    "survival_estimator",
    "Measured provider and incremental GCE cost through immutable T5.",
  ],
  [
    "metric.total_run_cost.v1",
    "Total run cost",
    "usd_micros",
    "population.all_runs",
    "percentile",
    "Measured provider, GCE, artifact, and evaluator cost through T8 cleanup_observed.",
  ],
  [
    "metric.tool_work_to_identification.v1",
    "Tool work to identification",
    "count",
    "population.runs_with_t5",
    "percentile",
    "Bounded tool calls, file reads, dependency crossings, builds, and verifier actions through T5.",
  ],
  [
    "metric.concurrency_amplification.v1",
    "Concurrency amplification",
    "ratio",
    "population.analysis_started",
    "percentile",
    "Sum of active-agent milliseconds divided by analysis wall time.",
  ],
  [
    "metric.worker_utilization.v1",
    "Worker utilization",
    "ratio",
    "population.provisioned",
    "percentile",
    "Measured running CPU time divided by worker lease time, paired with memory high-water evidence.",
  ],
  [
    "metric.admission_success.v1",
    "Admission and run success",
    "ratio",
    "population.requested",
    "confidence_interval",
    "Requested runs reaching worker_ready, structural settlement, and T8 with refusal reasons retained.",
  ],
  [
    "metric.cancellation_latency.v1",
    "Cancellation latency",
    "milliseconds",
    "population.cancelled",
    "percentile",
    "Cancel intent to runtime interruption and T8 cleanup_observed.",
  ],
  [
    "metric.cleanup_success.v1",
    "Cleanup success",
    "ratio",
    "population.provisioned",
    "confidence_interval",
    "Provisioned runs with observed zero residue divided by all provisioned runs.",
  ],
  [
    "metric.budget_compliance.v1",
    "Budget compliance",
    "ratio",
    "population.all_runs",
    "confidence_interval",
    "Runs within frozen time, token, cost, network, and artifact caps divided by all runs.",
  ],
  [
    REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF,
    "Reviewer minutes per qualified finding",
    "milliseconds",
    "population.reviewed",
    "percentile",
    "Explicit bounded active-review intervals divided by unique qualified findings.",
  ],
  [
    CORRECTION_REJECTION_BURDEN_METRIC_REF,
    "Correction and rejection burden",
    "ratio",
    "population.reviewed",
    "confidence_interval",
    "Qualified identities materially corrected or rejected divided by reviewed qualified identities.",
  ],
  [
    "metric.actionable_acceptance_rate.v1",
    "Actionable acceptance rate",
    "ratio",
    "population.reviewed",
    "confidence_interval",
    "Qualified identities accepted for retained remediation or regression work divided by reviewed identities.",
  ],
  [
    "metric.coldcard.generator_vector_coverage.v1",
    "Coldcard generator-vector coverage",
    "ratio",
    "population.coldcard_generator_suite",
    "mean",
    "Frozen generator calls reproduced exactly divided by all required golden-vector calls.",
  ],
  [
    "metric.coldcard.entropy_sensitivity.v1",
    "Coldcard entropy sensitivity coverage",
    "ratio",
    "population.coldcard_entropy_models",
    "mean",
    "Frozen entropy assumptions with executed sensitivity variants divided by all required assumptions.",
  ],
  [
    "metric.coldcard.scan_throughput.v1",
    "Coldcard scan throughput",
    "transactions_per_second",
    "population.coldcard_historical_scan",
    "percentile",
    "Transactions structurally settled divided by measured scan runtime.",
  ],
  [
    "metric.coldcard.candidate_funnel_yield.v1",
    "Coldcard candidate-funnel yield",
    "ratio",
    "population.coldcard_historical_scan",
    "mean",
    "Evidence-linked waves divided by transactions scanned, retaining each intermediate funnel count.",
  ],
  [
    "metric.coldcard.false_matches_per_million.v1",
    "Coldcard false matches per million",
    "matches_per_million",
    "population.coldcard_negative_controls",
    "confidence_interval",
    "False exact fingerprint matches divided by scanned negative-control transactions, scaled to one million.",
  ],
  [
    "metric.coldcard.reconciliation_drift.v1",
    "Coldcard reconciliation drift",
    "count",
    "population.coldcard_reconciled_scans",
    "percentile",
    "Mismatched normalized candidates between independent scan implementations after frozen reconciliation.",
  ],
];

const FROZEN_METRIC_CREATED_AT = "2026-08-01T00:00:00.000Z";
const FROZEN_CENSOR_POLICY =
  "Eligible misses retain spent budget and are right-censored; unidentified percentiles are not_estimable.";
const FROZEN_MISS_POLICY =
  "Misses, failures, cancellations, retries, and losing parallel arms remain in their eligible denominators.";
const FROZEN_EXACTNESS_POLICY =
  "Values retain exact, estimated, upper_bound, or unavailable; unavailable never carries a numeric value.";

const frozenMetricDefinitions = FROZEN_METRIC_SPECS.map(
  ([metricRef, name, unit, populationRef, aggregation, formula]) => {
    const definitionWithoutDigest = {
      schema: FORENSIC_METRIC_DEFINITION_VERSION,
      metricRef,
      name,
      formula,
      unit,
      eligiblePopulationRef: populationRef,
      censorPolicy: FROZEN_CENSOR_POLICY,
      missPolicy: FROZEN_MISS_POLICY,
      exactnessPolicy: FROZEN_EXACTNESS_POLICY,
      dimensionRefs: [
        "dimension.dataset_split",
        "dimension.population",
        "dimension.model_revision",
        "dimension.worker_revision",
      ],
      aggregation,
      displayMetadataRefs: ["display.metric_provenance.v1"],
      createdAt: FROZEN_METRIC_CREATED_AT,
    };
    return strictDecode(ForensicMetricDefinitionSchema, {
      ...definitionWithoutDigest,
      revisionDigest: forensicSha256Digest(definitionWithoutDigest),
    });
  },
);

export const FROZEN_FORENSIC_METRIC_REGISTRY = strictDecode(ForensicMetricRegistrySchema, {
  schema: FORENSIC_METRIC_REGISTRY_VERSION,
  registryRef: "registry.openagents.forensic.metrics.2026-08-01.v1",
  definitions: frozenMetricDefinitions,
  revisionDigest: forensicSha256Digest(frozenMetricDefinitions),
  frozenAt: FROZEN_METRIC_CREATED_AT,
});

export interface ForensicScorecardRunInput {
  readonly runRef: string;
  readonly runDigest: string;
  readonly armRef: string;
  readonly datasetSplit: "train" | "development" | "holdout" | "clean_holdout";
  readonly population: ScorecardPopulation;
  readonly coverageStatus: "complete" | "incomplete" | "denied";
  readonly censorAtMilliseconds?: number;
  readonly events: ReadonlyArray<ForensicRunEvent>;
  readonly usageReceipts: ReadonlyArray<ForensicProviderUsageReceipt>;
  readonly adjudications: ReadonlyArray<ForensicEvaluatorAdjudication>;
  readonly reviewerBurdenReceipts?: ReadonlyArray<ForensicReviewerBurdenReceipt>;
  readonly retainedReceiptDigests: ReadonlyArray<string>;
  readonly failureRefs: ReadonlyArray<string>;
}

export interface RebuildForensicScorecardInput {
  readonly scorecardRef: string;
  readonly datasetRevisionDigest: string;
  readonly evaluatorRevisionDigest: string;
  readonly candidateDigest: string;
  readonly registry: ForensicMetricRegistry;
  readonly hardGates: ReadonlyArray<HardGateResult>;
  readonly runs: ReadonlyArray<ForensicScorecardRunInput>;
  readonly generatedAt: string;
}

const aggregateUsage = (
  receipts: ReadonlyArray<ForensicProviderUsageReceipt>,
  forceUpperBound: boolean,
) => {
  const unavailable = receipts.find((receipt) => receipt.usage.exactness === "unavailable");
  if (receipts.length === 0 || unavailable !== undefined) {
    return {
      exactness: "unavailable" as const,
      unavailableReasonRef:
        unavailable?.usage.exactness === "unavailable"
          ? unavailable.usage.unavailableReasonRef
          : "unavailable.provider_usage.missing",
    };
  }

  const available = receipts
    .map((receipt) => receipt.usage)
    .filter((usage) => {
      return usage.exactness !== "unavailable";
    });
  const exactness =
    forceUpperBound || available.some((usage) => usage.exactness === "upper_bound")
      ? "upper_bound"
      : available.some((usage) => usage.exactness === "estimated")
        ? "estimated"
        : "exact";
  return {
    inputTokens: available.reduce((total, usage) => total + usage.inputTokens, 0),
    cachedInputTokens: available.reduce((total, usage) => total + usage.cachedInputTokens, 0),
    outputTokens: available.reduce((total, usage) => total + usage.outputTokens, 0),
    reasoningTokens: available.reduce((total, usage) => total + usage.reasoningTokens, 0),
    totalTokens: available.reduce((total, usage) => total + usage.totalTokens, 0),
    exactness,
  } as const;
};

const metricUnavailable = (
  metricRef: string,
  reasonRef: string,
  sourceEventRefs: ReadonlyArray<string>,
  sourceReceiptRefs: ReadonlyArray<string>,
) => ({
  metricRef,
  exactness: "unavailable" as const,
  unavailableReasonRef: reasonRef,
  sourceEventRefs,
  sourceReceiptRefs,
});

const deriveRun = (
  input: ForensicScorecardRunInput,
  evaluatorRevisionDigest: string,
): ScorecardRun => {
  const events = input.events.map((event) => strictDecode(ForensicRunEventSchema, event));
  if (events.some((event) => event.runRef !== input.runRef)) {
    throw new Error(`metric events must belong to ${input.runRef}`);
  }
  if (new Set(events.map((event) => event.eventRef)).size !== events.length) {
    throw new Error("metric event refs must be unique within a retained run");
  }
  const sequenceDecision = evaluateForensicEventSequence(events);
  if (sequenceDecision._tag === "Invalid") {
    throw new Error(`metric events must be append-only and dense: ${sequenceDecision.blockerRef}`);
  }
  const firstContext = events[0]?.metricContext;
  if (
    firstContext === undefined ||
    firstContext.armRef !== input.armRef ||
    firstContext.datasetSplit !== input.datasetSplit ||
    firstContext.evaluatorRevisionDigest !== evaluatorRevisionDigest ||
    events.some(
      (event) => forensicSha256Digest(event.metricContext) !== forensicSha256Digest(firstContext),
    )
  ) {
    throw new Error("metric events must retain one exact run, arm, split, and evaluator context");
  }
  const usageReceipts = input.usageReceipts.map((receipt) =>
    strictDecode(ForensicProviderUsageReceiptSchema, receipt),
  );
  if (usageReceipts.some((receipt) => receipt.runRef !== input.runRef)) {
    throw new Error(`usage receipts must belong to ${input.runRef}`);
  }
  if (new Set(usageReceipts.map((receipt) => receipt.receiptRef)).size !== usageReceipts.length) {
    throw new Error("provider usage receipt refs must be unique within a retained run");
  }
  for (const receipt of usageReceipts) {
    const started = events.find(
      (event) =>
        event.eventRef === receipt.startEventRef && event.sequence === receipt.startEventSequence,
    );
    if (
      started?.kind !== "turn_started" ||
      events.find((event) => event.sequence === receipt.settledEventSequence) === undefined
    ) {
      throw new Error("provider usage must bind a retained turn start and settlement sequence");
    }
  }
  const adjudications = input.adjudications.map((adjudication) =>
    strictDecode(ForensicEvaluatorAdjudicationSchema, adjudication),
  );
  const reviewerBurdenReceipts = (input.reviewerBurdenReceipts ?? []).map((receipt) =>
    strictDecode(ForensicReviewerBurdenReceiptSchema, receipt),
  );
  if (
    new Set(reviewerBurdenReceipts.map((receipt) => receipt.receiptRef)).size !==
      reviewerBurdenReceipts.length ||
    reviewerBurdenReceipts.some((receipt) => {
      const event = events.find(
        (candidate) =>
          candidate.eventRef === receipt.reviewEventRef &&
          candidate.sequence === receipt.reviewEventSequence,
      );
      return receipt.runRef !== input.runRef || event?.kind !== "review_recorded";
    })
  ) {
    throw new Error("reviewer burden receipts must bind unique retained review events");
  }
  if (
    new Set(adjudications.map((adjudication) => adjudication.adjudicationRef)).size !==
    adjudications.length
  ) {
    throw new Error("evaluator adjudication refs must be unique within a retained run");
  }
  if (
    adjudications.some(
      (adjudication) =>
        adjudication.runRef !== input.runRef ||
        adjudication.evaluatorRevisionDigest !== evaluatorRevisionDigest,
    )
  ) {
    throw new Error(`adjudications must bind ${input.runRef} and the frozen evaluator revision`);
  }

  const qualified = adjudications
    .filter((adjudication) => adjudication.outcome === "qualified")
    .map((adjudication) => {
      const event = events.find((candidate) => candidate.eventRef === adjudication.findingEventRef);
      if (
        event === undefined ||
        event.kind !== "finding_submitted" ||
        !event.relatedRefs.includes(adjudication.findingRef) ||
        Date.parse(adjudication.evaluatedAt) < Date.parse(event.observedAt) ||
        forensicSha256Digest(event) !== adjudication.findingEventDigest
      ) {
        throw new Error("qualified adjudication must bind the immutable finding event bytes");
      }
      return { adjudication, event };
    })
    .sort((left, right) =>
      left.event.observedAt === right.event.observedAt
        ? left.event.sequence - right.event.sequence
        : left.event.observedAt.localeCompare(right.event.observedAt),
    );

  const eligibleForIdentification =
    input.coverageStatus === "complete" &&
    ["vulnerable", "structural_variant"].includes(input.population) &&
    ["development", "holdout"].includes(input.datasetSplit);
  const firstQualified = eligibleForIdentification ? qualified[0] : undefined;
  const miss = eligibleForIdentification && firstQualified === undefined;
  if (miss && (input.censorAtMilliseconds === undefined || input.censorAtMilliseconds <= 0)) {
    throw new Error("eligible misses require their nonzero right-censor boundary");
  }
  const analysisStarted = events.find((event) => event.kind === "analysis_started");
  const spentUsage = aggregateUsage(usageReceipts, false);
  const receiptRefs = usageReceipts.map((receipt) => receipt.receiptRef);
  const values: Array<MetricValue> = [
    strictDecode(MetricValueSchema, {
      metricRef: QUALIFIED_HIT_METRIC_REF,
      booleanValue: firstQualified !== undefined,
      exactness: "exact",
      sourceEventRefs: firstQualified === undefined ? [] : [firstQualified.event.eventRef],
      sourceReceiptRefs: [],
    }),
  ];
  const unavailableReviewerReceipt = reviewerBurdenReceipts.find(
    (receipt) => receipt.duration.exactness === "unavailable",
  );
  const reviewerReceiptRefs = reviewerBurdenReceipts.map((receipt) => receipt.receiptRef);
  const reviewerEventRefs = reviewerBurdenReceipts.map((receipt) => receipt.reviewEventRef);
  values.push(
    strictDecode(
      MetricValueSchema,
      reviewerBurdenReceipts.length === 0 || unavailableReviewerReceipt !== undefined
        ? metricUnavailable(
            REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF,
            unavailableReviewerReceipt?.duration.exactness === "unavailable"
              ? unavailableReviewerReceipt.duration.unavailableReasonRef
              : "unavailable.reviewer_burden.missing",
            reviewerEventRefs,
            reviewerReceiptRefs,
          )
        : qualified.length === 0
          ? metricUnavailable(
              REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF,
              "unavailable.reviewer_burden.no_qualified_finding",
              reviewerEventRefs,
              reviewerReceiptRefs,
            )
          : {
              metricRef: REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF,
              numericValue:
                reviewerBurdenReceipts.reduce(
                  (total, receipt) =>
                    total +
                    (receipt.duration.exactness === "unavailable"
                      ? 0
                      : receipt.duration.milliseconds),
                  0,
                ) / qualified.length,
              exactness: reviewerBurdenReceipts.some(
                (receipt) => receipt.duration.exactness === "upper_bound",
              )
                ? "upper_bound"
                : reviewerBurdenReceipts.some(
                      (receipt) => receipt.duration.exactness === "estimated",
                    )
                  ? "estimated"
                  : "exact",
              sourceEventRefs: reviewerEventRefs,
              sourceReceiptRefs: reviewerReceiptRefs,
            },
    ),
    strictDecode(
      MetricValueSchema,
      reviewerBurdenReceipts.length === 0
        ? metricUnavailable(
            CORRECTION_REJECTION_BURDEN_METRIC_REF,
            "unavailable.reviewer_burden.missing",
            [],
            [],
          )
        : {
            metricRef: CORRECTION_REJECTION_BURDEN_METRIC_REF,
            numericValue: reviewerBurdenReceipts.reduce(
              (total, receipt) => total + receipt.correctionCount + receipt.rejectionCount,
              0,
            ),
            exactness: "exact",
            sourceEventRefs: reviewerEventRefs,
            sourceReceiptRefs: reviewerReceiptRefs,
          },
    ),
  );

  if (firstQualified === undefined || analysisStarted === undefined) {
    values.push(
      strictDecode(
        MetricValueSchema,
        metricUnavailable(
          ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF,
          miss
            ? "unavailable.identification.right_censored"
            : "unavailable.identification.not_eligible",
          analysisStarted === undefined ? [] : [analysisStarted.eventRef],
          [],
        ),
      ),
    );
  } else {
    const milliseconds =
      Date.parse(firstQualified.event.observedAt) - Date.parse(analysisStarted.observedAt);
    if (!Number.isFinite(milliseconds) || milliseconds < 0) {
      throw new Error("qualified finding timestamp cannot precede analysis start");
    }
    values.push(
      strictDecode(MetricValueSchema, {
        metricRef: ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF,
        numericValue: milliseconds,
        exactness: "exact",
        sourceEventRefs: [analysisStarted.eventRef, firstQualified.event.eventRef],
        sourceReceiptRefs: [],
      }),
    );
  }

  const throughIdentification =
    firstQualified === undefined
      ? []
      : usageReceipts.filter(
          (receipt) => receipt.startEventSequence <= firstQualified.event.sequence,
        );
  const identificationUsage = aggregateUsage(
    throughIdentification,
    firstQualified !== undefined &&
      throughIdentification.some(
        (receipt) => receipt.settledEventSequence > firstQualified.event.sequence,
      ),
  );
  values.push(
    strictDecode(
      MetricValueSchema,
      identificationUsage.exactness === "unavailable"
        ? metricUnavailable(
            TOKENS_TO_IDENTIFICATION_METRIC_REF,
            identificationUsage.unavailableReasonRef,
            firstQualified === undefined ? [] : [firstQualified.event.eventRef],
            throughIdentification.map((receipt) => receipt.receiptRef),
          )
        : {
            metricRef: TOKENS_TO_IDENTIFICATION_METRIC_REF,
            numericValue: identificationUsage.totalTokens,
            exactness: identificationUsage.exactness,
            sourceEventRefs: firstQualified === undefined ? [] : [firstQualified.event.eventRef],
            sourceReceiptRefs: throughIdentification.map((receipt) => receipt.receiptRef),
          },
    ),
    strictDecode(
      MetricValueSchema,
      spentUsage.exactness === "unavailable"
        ? metricUnavailable(
            TOTAL_RUN_TOKENS_METRIC_REF,
            spentUsage.unavailableReasonRef,
            [],
            receiptRefs,
          )
        : {
            metricRef: TOTAL_RUN_TOKENS_METRIC_REF,
            numericValue: spentUsage.totalTokens,
            exactness: spentUsage.exactness,
            sourceEventRefs: [],
            sourceReceiptRefs: receiptRefs,
          },
    ),
    strictDecode(MetricValueSchema, {
      metricRef: CONTROL_FALSE_POSITIVE_METRIC_REF,
      booleanValue:
        ["fixed_control", "clean_control"].includes(input.population) && qualified.length > 0,
      exactness: "exact",
      sourceEventRefs: qualified.map(({ event }) => event.eventRef),
      sourceReceiptRefs: [],
    }),
  );

  return strictDecode(ScorecardRunSchema, {
    runDigest: input.runDigest,
    armRef: input.armRef,
    datasetSplit: input.datasetSplit,
    population: input.population,
    coverageStatus: input.coverageStatus,
    outcome: firstQualified !== undefined ? "hit" : miss ? "miss" : "not_eligible",
    eligibleForIdentification,
    censored: miss,
    miss,
    ...(miss
      ? {
          censorAt: { milliseconds: input.censorAtMilliseconds, exactness: "exact" },
        }
      : {}),
    spentUsage,
    ...(firstQualified === undefined
      ? {}
      : {
          qualifiedFindingEventRef: firstQualified.event.eventRef,
          qualifiedFindingObservedAt: firstQualified.event.observedAt,
        }),
    values,
    failureRefs: input.failureRefs,
  });
};

export const rebuildForensicScorecard = (
  input: RebuildForensicScorecardInput,
): ForensicScorecard => {
  const registry = strictDecode(ForensicMetricRegistrySchema, input.registry);
  const metricRefs = new Set(registry.definitions.map((definition) => definition.metricRef));
  if (REQUIRED_PROJECTOR_METRICS.some((metricRef) => !metricRefs.has(metricRef))) {
    throw new Error("frozen metric registry is missing a required scorecard projector metric");
  }
  if (input.runs.length === 0) throw new Error("scorecard requires retained runs");

  const runs = input.runs.map((run) => deriveRun(run, input.evaluatorRevisionDigest));
  const populationGroups = Array.from(
    runs.reduce((groups, run) => {
      const key = `${run.datasetSplit}:${run.population}`;
      const group = groups.get(key) ?? {
        datasetSplit: run.datasetSplit,
        population: run.population,
        runCount: 0,
        hitCount: 0,
        missCount: 0,
        censorCount: 0,
      };
      groups.set(key, {
        ...group,
        runCount: group.runCount + 1,
        hitCount: group.hitCount + Number(run.outcome === "hit"),
        missCount: group.missCount + Number(run.miss),
        censorCount: group.censorCount + Number(run.censored),
      });
      return groups;
    }, new Map<string, { datasetSplit: ScorecardRun["datasetSplit"]; population: ScorecardPopulation; runCount: number; hitCount: number; missCount: number; censorCount: number }>()),
  ).map(([, group]) => group);
  const usageReceipts = input.runs.flatMap((run) => run.usageReceipts);
  const reviewerBurdenReceipts = input.runs.flatMap((run) => run.reviewerBurdenReceipts ?? []);
  const hasUnavailableCost =
    usageReceipts.length === 0 ||
    usageReceipts.some(
      (receipt) => receipt.usage.exactness === "unavailable" || receipt.costMicros === undefined,
    );
  const retainedReceiptDigests = input.runs
    .flatMap((run) => run.retainedReceiptDigests)
    .map((receiptDigest) => strictDecode(Sha256Digest, receiptDigest));
  const cost = hasUnavailableCost
    ? {
        exactness: "unavailable" as const,
        unavailableReasonRef: "unavailable.provider_cost.incomplete",
      }
    : {
        micros: usageReceipts.reduce((total, receipt) => total + (receipt.costMicros ?? 0), 0),
        exactness: usageReceipts.some((receipt) => receipt.usage.exactness !== "exact")
          ? ("estimated" as const)
          : ("exact" as const),
      };

  return strictDecode(ForensicScorecardSchema, {
    schema: FORENSIC_SCORECARD_VERSION,
    scorecardRef: input.scorecardRef,
    datasetRevisionDigest: input.datasetRevisionDigest,
    metricDefinitionRevisionDigest: registry.revisionDigest,
    evaluatorRevisionDigest: input.evaluatorRevisionDigest,
    candidateDigest: input.candidateDigest,
    hardGates: input.hardGates,
    runs,
    populationGroups,
    distributionRefs: [],
    censorCount: runs.filter((run) => run.censored).length,
    missCount: runs.filter((run) => run.miss).length,
    confidenceIntervalRefs: [],
    cost,
    eventDigest: forensicSha256Digest(
      input.runs.flatMap((run) => [...run.events, ...run.adjudications]),
    ),
    receiptDigest: forensicSha256Digest([
      ...usageReceipts,
      ...reviewerBurdenReceipts,
      ...retainedReceiptDigests,
    ]),
    generatedAt: input.generatedAt,
  });
};
