import { Schema as S } from "effect";

import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import {
  ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF,
  FORENSIC_SCORECARD_VERSION,
  ForensicScorecardSchema,
  TOKENS_TO_IDENTIFICATION_METRIC_REF,
  type ForensicScorecard,
} from "./metrics.ts";
import { Exactness, NonNegativeInteger, Sha256Digest } from "./primitives.ts";
import {
  CleanupState,
  CoverageStatus,
  ForensicRunSchema,
  ForensicRunState,
  type ForensicRun,
} from "./run.ts";

export const FORENSIC_RUN_PUBLIC_PROJECTION_VERSION =
  "openagents.forensic_run_public_projection.v1" as const;
export const FORENSIC_SCORECARD_PUBLIC_PROJECTION_VERSION =
  "openagents.forensic_scorecard_public_projection.v1" as const;

const PublicAvailableUsageTruthSchema = S.Struct({
  inputTokens: NonNegativeInteger,
  cachedInputTokens: NonNegativeInteger,
  outputTokens: NonNegativeInteger,
  reasoningTokens: NonNegativeInteger,
  totalTokens: NonNegativeInteger,
  exactness: S.Literals(["exact", "estimated", "upper_bound"]),
});
const PublicUnavailableUsageTruthSchema = S.Struct({
  exactness: S.Literal("unavailable"),
});
export const PublicUsageTruthSchema = S.Union([
  PublicAvailableUsageTruthSchema,
  PublicUnavailableUsageTruthSchema,
]);
export type PublicUsageTruth = typeof PublicUsageTruthSchema.Type;

export const ForensicRunPublicProjectionSchema = S.Struct({
  schema: S.Literal(FORENSIC_RUN_PUBLIC_PROJECTION_VERSION),
  runDigest: Sha256Digest,
  sourceDigest: Sha256Digest,
  promptDigest: Sha256Digest,
  modelDigest: Sha256Digest,
  workerImageDigest: Sha256Digest,
  workerProfileDigest: Sha256Digest,
  state: ForensicRunState,
  coverageStatus: CoverageStatus,
  cleanupState: CleanupState,
  findingCount: NonNegativeInteger,
  hypothesisCount: NonNegativeInteger,
  errorCount: NonNegativeInteger,
  lastEventSequence: NonNegativeInteger,
  totalDurationMilliseconds: S.optionalKey(NonNegativeInteger),
  totalDurationExactness: S.optionalKey(Exactness),
  usage: S.optionalKey(PublicUsageTruthSchema),
})
  .pipe(
    S.check(
      S.makeFilter(
        (projection) =>
          (projection.totalDurationMilliseconds === undefined) ===
          (projection.totalDurationExactness === undefined),
        { message: "public duration value and exactness must appear together" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicRunPublicProjection" });
export interface ForensicRunPublicProjection extends S.Schema.Type<
  typeof ForensicRunPublicProjectionSchema
> {}

export const projectForensicRunPublicSafe = (input: unknown): ForensicRunPublicProjection => {
  const run = strictDecode(ForensicRunSchema, input);
  const projection = {
    schema: FORENSIC_RUN_PUBLIC_PROJECTION_VERSION,
    runDigest: forensicSha256Digest(run),
    sourceDigest: run.sourceDigest,
    promptDigest: run.promptDigest,
    modelDigest: run.modelDigest,
    workerImageDigest: run.workerImageDigest,
    workerProfileDigest: run.workerProfileDigest,
    state: run.state,
    coverageStatus: run.coverageStatus,
    cleanupState: run.cleanupState,
    findingCount: run.findingRefs.length,
    hypothesisCount: run.hypothesisRefs.length,
    errorCount: run.errorRefs.length,
    lastEventSequence: run.lastEventSequence,
    ...(run.totalDuration === undefined
      ? {}
      : {
          totalDurationMilliseconds: run.totalDuration.milliseconds,
          totalDurationExactness: run.totalDuration.exactness,
        }),
    ...(run.usage === undefined
      ? {}
      : run.usage.exactness === "unavailable"
        ? { usage: { exactness: "unavailable" as const } }
        : {
            usage: {
              inputTokens: run.usage.inputTokens,
              cachedInputTokens: run.usage.cachedInputTokens,
              outputTokens: run.usage.outputTokens,
              reasoningTokens: run.usage.reasoningTokens,
              totalTokens: run.usage.totalTokens,
              exactness: run.usage.exactness,
            },
          }),
  };
  return strictDecode(ForensicRunPublicProjectionSchema, projection);
};

export const assertPublicProjectionOrigin = (
  run: ForensicRun,
  projection: ForensicRunPublicProjection,
): boolean =>
  projection.runDigest === forensicSha256Digest(run) &&
  projection.sourceDigest === run.sourceDigest &&
  projection.promptDigest === run.promptDigest;

export const PublicScorecardPopulationGroupSchema = S.Struct({
  datasetSplit: S.Literals(["train", "development", "holdout", "clean_holdout"]),
  population: S.Literals([
    "incomplete",
    "vulnerable",
    "structural_variant",
    "fixed_control",
    "clean_control",
  ]),
  runCount: NonNegativeInteger,
  hitCount: NonNegativeInteger,
  missCount: NonNegativeInteger,
  censorCount: NonNegativeInteger,
  identificationDurationSampleCount: NonNegativeInteger,
  identificationDurationTotalMilliseconds: NonNegativeInteger,
  identificationTokenSampleCount: NonNegativeInteger,
  identificationTokenTotal: NonNegativeInteger,
});

export const ForensicScorecardPublicProjectionSchema = S.Struct({
  schema: S.Literal(FORENSIC_SCORECARD_PUBLIC_PROJECTION_VERSION),
  scorecardDigest: Sha256Digest,
  datasetRevisionDigest: Sha256Digest,
  metricDefinitionRevisionDigest: Sha256Digest,
  evaluatorRevisionDigest: Sha256Digest,
  candidateDigest: Sha256Digest,
  allHardGatesPassed: S.Boolean,
  populations: S.Array(PublicScorecardPopulationGroupSchema).check(S.isMaxLength(20)),
  censorCount: NonNegativeInteger,
  missCount: NonNegativeInteger,
  costMicros: S.optionalKey(NonNegativeInteger),
  costExactness: Exactness,
  sourceEventCount: NonNegativeInteger,
  sourceReceiptCount: NonNegativeInteger,
})
  .pipe(
    S.check(
      S.makeFilter(
        (projection) =>
          projection.costExactness === "unavailable"
            ? projection.costMicros === undefined
            : projection.costMicros !== undefined,
        { message: "public cost cannot turn unavailable evidence into numeric zero" },
      ),
    ),
  )
  .annotate({ identifier: "ForensicScorecardPublicProjection" });
export interface ForensicScorecardPublicProjection extends S.Schema.Type<
  typeof ForensicScorecardPublicProjectionSchema
> {}

const numericMetric = (run: ForensicScorecard["runs"][number], metricRef: string) => {
  const value = run.values.find((candidate) => candidate.metricRef === metricRef);
  return value?.exactness === "unavailable" ? undefined : value?.numericValue;
};

export const projectForensicScorecardPublicSafe = (
  input: unknown,
): ForensicScorecardPublicProjection => {
  const scorecard = strictDecode(ForensicScorecardSchema, input);
  const populations = scorecard.populationGroups.map((group) => {
    const runs = scorecard.runs.filter(
      (run) => run.datasetSplit === group.datasetSplit && run.population === group.population,
    );
    const durations = runs
      .map((run) => numericMetric(run, ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF))
      .filter((value): value is number => value !== undefined);
    const tokens = runs
      .map((run) => numericMetric(run, TOKENS_TO_IDENTIFICATION_METRIC_REF))
      .filter((value): value is number => value !== undefined);
    return {
      ...group,
      identificationDurationSampleCount: durations.length,
      identificationDurationTotalMilliseconds: durations.reduce((total, value) => total + value, 0),
      identificationTokenSampleCount: tokens.length,
      identificationTokenTotal: tokens.reduce((total, value) => total + value, 0),
    };
  });
  const sourceEventCount = new Set(
    scorecard.runs.flatMap((run) => run.values.flatMap((value) => value.sourceEventRefs)),
  ).size;
  const sourceReceiptCount = new Set(
    scorecard.runs.flatMap((run) => run.values.flatMap((value) => value.sourceReceiptRefs)),
  ).size;
  return strictDecode(ForensicScorecardPublicProjectionSchema, {
    schema: FORENSIC_SCORECARD_PUBLIC_PROJECTION_VERSION,
    scorecardDigest: forensicSha256Digest(scorecard),
    datasetRevisionDigest: scorecard.datasetRevisionDigest,
    metricDefinitionRevisionDigest: scorecard.metricDefinitionRevisionDigest,
    evaluatorRevisionDigest: scorecard.evaluatorRevisionDigest,
    candidateDigest: scorecard.candidateDigest,
    allHardGatesPassed: scorecard.hardGates.every((gate) => gate.passed),
    populations,
    censorCount: scorecard.censorCount,
    missCount: scorecard.missCount,
    ...(scorecard.cost.exactness === "unavailable" ? {} : { costMicros: scorecard.cost.micros }),
    costExactness: scorecard.cost.exactness,
    sourceEventCount,
    sourceReceiptCount,
  });
};

export const assertPublicScorecardProjectionOrigin = (
  scorecard: ForensicScorecard,
  projection: ForensicScorecardPublicProjection,
): boolean =>
  scorecard.schema === FORENSIC_SCORECARD_VERSION &&
  projection.scorecardDigest === forensicSha256Digest(scorecard) &&
  projection.metricDefinitionRevisionDigest === scorecard.metricDefinitionRevisionDigest &&
  projection.evaluatorRevisionDigest === scorecard.evaluatorRevisionDigest;
