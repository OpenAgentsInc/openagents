import { Schema as S } from "effect";

import {
  BoundedRefs,
  BoundedShortTexts,
  Exactness,
  ForensicRef,
  ForensicTimestamp,
  LongText,
  NonEmptyBoundedRefs,
  NonNegativeInteger,
  Sha256Digest,
  ShortText,
} from "./primitives.ts";

export const FORENSIC_METRIC_DEFINITION_VERSION =
  "openagents.forensic_metric_definition.v1" as const;
export const FORENSIC_SCORECARD_VERSION = "openagents.forensic_scorecard.v1" as const;
export const FORENSIC_PROMPT_PROMOTION_VERSION = "openagents.forensic_prompt_promotion.v1" as const;

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

export const ScorecardRunSchema = S.Struct({
  runDigest: Sha256Digest,
  armRef: ForensicRef,
  datasetSplit: S.Literals(["train", "development", "holdout", "clean_holdout"]),
  coverageStatus: S.Literals(["complete", "incomplete", "denied"]),
  censored: S.Boolean,
  miss: S.Boolean,
  values: S.Array(MetricValueSchema).check(S.isMaxLength(512)),
  failureRefs: BoundedRefs,
});
export interface ScorecardRun extends S.Schema.Type<typeof ScorecardRunSchema> {}

export const ForensicScorecardSchema = S.Struct({
  schema: S.Literal(FORENSIC_SCORECARD_VERSION),
  scorecardRef: ForensicRef,
  datasetRevisionDigest: Sha256Digest,
  metricDefinitionRevisionDigest: Sha256Digest,
  evaluatorRevisionDigest: Sha256Digest,
  candidateDigest: Sha256Digest,
  hardGates: S.Array(HardGateResultSchema).check(S.isMinLength(1), S.isMaxLength(256)),
  runs: S.Array(ScorecardRunSchema).check(S.isMinLength(1), S.isMaxLength(100_000)),
  distributionRefs: BoundedRefs,
  censorCount: NonNegativeInteger,
  missCount: NonNegativeInteger,
  confidenceIntervalRefs: BoundedRefs,
  costMicros: NonNegativeInteger,
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
  releaseGateRef: ForensicRef,
  hardGateRefs: NonEmptyBoundedRefs,
  allHardGatesPassed: S.Boolean,
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
            promotion.nextActivePromptDigest === promotion.candidatePromptDigest),
        { message: "admission requires all hard gates and activation of the evaluated candidate" },
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
