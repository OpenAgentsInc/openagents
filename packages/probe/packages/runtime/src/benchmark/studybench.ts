import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  ProbeBenchmarkRedactionState,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";

export const OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF = "openagents.studybench_task.v0" as const;
export const OPENAGENTS_STUDYBENCH_RUBRIC_CLAIM_SCHEMA_REF =
  "openagents.studybench_rubric_claim.v0" as const;
export const OPENAGENTS_STUDYBENCH_EVIDENCE_SPAN_SCHEMA_REF =
  "openagents.studybench_evidence_span.v0" as const;
export const OPENAGENTS_STUDYBENCH_DATASET_PACKAGE_SCHEMA_REF =
  "openagents.studybench_dataset_package.v0" as const;
export const PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF = "probe.studybench_claim_score.v0" as const;
export const PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF = "probe.studybench_rubric_score.v0" as const;

export const OpenAgentsStudybenchClaimType = S.Literals(["core", "supporting"]);
export type OpenAgentsStudybenchClaimType = typeof OpenAgentsStudybenchClaimType.Type;

export const OpenAgentsStudybenchVisibility = S.Literals([
  "external_public_calibration",
  "openagents_public_retained",
  "openagents_private_validation",
  "openagents_private_holdout",
]);
export type OpenAgentsStudybenchVisibility = typeof OpenAgentsStudybenchVisibility.Type;

export const OpenAgentsStudybenchBudgetClass = S.Literals(["tiny", "small", "medium", "large"]);
export type OpenAgentsStudybenchBudgetClass = typeof OpenAgentsStudybenchBudgetClass.Type;

export const OpenAgentsStudybenchSourceBoundary = S.Literals([
  "public_refs_only",
  "private_refs_withheld",
]);
export type OpenAgentsStudybenchSourceBoundary = typeof OpenAgentsStudybenchSourceBoundary.Type;

export const OpenAgentsStudybenchRubricClaim = S.Struct({
  claim_id: S.String,
  claim_type: OpenAgentsStudybenchClaimType,
  schemaRef: S.optional(S.Literal(OPENAGENTS_STUDYBENCH_RUBRIC_CLAIM_SCHEMA_REF)),
  span_ids: S.Array(S.String),
  statement: S.String,
  weight: S.Number,
});
export type OpenAgentsStudybenchRubricClaim = typeof OpenAgentsStudybenchRubricClaim.Type;

export const OpenAgentsStudybenchEvidenceSpan = S.Struct({
  end_line: S.Number,
  excerpt: S.String,
  path: S.String,
  schemaRef: S.optional(S.Literal(OPENAGENTS_STUDYBENCH_EVIDENCE_SPAN_SCHEMA_REF)),
  span_id: S.String,
  start_line: S.Number,
});
export type OpenAgentsStudybenchEvidenceSpan = typeof OpenAgentsStudybenchEvidenceSpan.Type;

export const OpenAgentsStudybenchTask = S.Struct({
  authorityRefs: S.Array(S.String),
  budgetClass: OpenAgentsStudybenchBudgetClass,
  commit: S.String,
  corpusRef: S.String,
  evidence: S.Array(OpenAgentsStudybenchEvidenceSpan),
  expectedFiles: S.Array(S.String),
  forbiddenClaimRefs: S.Array(S.String),
  gold_answer: S.String,
  id: S.String,
  privateMaterialPolicyRefs: S.Array(S.String),
  question: S.String,
  repo: S.String,
  rubric: S.Array(OpenAgentsStudybenchRubricClaim),
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF),
  testRefs: S.Array(S.String),
  topic: S.String,
  visibility: OpenAgentsStudybenchVisibility,
});
export type OpenAgentsStudybenchTask = typeof OpenAgentsStudybenchTask.Type;

export const OpenAgentsStudybenchDatasetPackage = S.Struct({
  datasetRef: S.String,
  packageRef: S.String,
  packageVisibility: OpenAgentsStudybenchVisibility,
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_DATASET_PACKAGE_SCHEMA_REF),
  sourceBoundary: OpenAgentsStudybenchSourceBoundary,
  tasks: S.Array(OpenAgentsStudybenchTask),
});
export type OpenAgentsStudybenchDatasetPackage = typeof OpenAgentsStudybenchDatasetPackage.Type;

export const ProbeStudybenchClaimScore = S.Struct({
  claimId: S.String,
  claimType: OpenAgentsStudybenchClaimType,
  evidenceSpanIds: S.Array(S.String),
  rationaleRef: S.String,
  satisfied: S.Boolean,
  schemaRef: S.Literal(PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF),
  scoreBps: S.Number,
  scorerRef: S.String,
  weight: S.Number,
});
export type ProbeStudybenchClaimScore = typeof ProbeStudybenchClaimScore.Type;

export const ProbeStudybenchRubricScore = S.Struct({
  candidateHash: S.String,
  claimScores: S.Array(ProbeStudybenchClaimScore),
  coreGatePassed: S.Boolean,
  evidenceUseRefs: S.Array(S.String),
  finalScoreBps: S.Number,
  goldAnswerRef: S.String,
  redactionState: ProbeBenchmarkRedactionState,
  schemaRef: S.Literal(PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF),
  taskId: S.String,
  weightedScoreBps: S.Number,
});
export type ProbeStudybenchRubricScore = typeof ProbeStudybenchRubricScore.Type;

export function decodeOpenAgentsStudybenchTask(
  value: unknown,
): Effect.Effect<OpenAgentsStudybenchTask, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchTask");
    const task = yield* decodeStudybenchSchema(OpenAgentsStudybenchTask, value, "studybenchTask");
    yield* validateOpenAgentsStudybenchTask(task);
    return task;
  });
}

export function decodeOpenAgentsStudybenchDatasetPackage(
  value: unknown,
): Effect.Effect<OpenAgentsStudybenchDatasetPackage, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchDatasetPackage");
    const packageRecord = yield* decodeStudybenchSchema(
      OpenAgentsStudybenchDatasetPackage,
      value,
      "studybenchDatasetPackage",
    );
    yield* validateOpenAgentsStudybenchDatasetPackage(packageRecord);
    return packageRecord;
  });
}

export function decodeProbeStudybenchClaimScore(
  value: unknown,
): Effect.Effect<ProbeStudybenchClaimScore, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchClaimScore");
    const claimScore = yield* decodeStudybenchSchema(ProbeStudybenchClaimScore, value, "studybenchClaimScore");
    yield* validateProbeStudybenchClaimScore(claimScore);
    return claimScore;
  });
}

export function decodeProbeStudybenchRubricScore(
  value: unknown,
): Effect.Effect<ProbeStudybenchRubricScore, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchRubricScore");
    const rubricScore = yield* decodeStudybenchSchema(ProbeStudybenchRubricScore, value, "studybenchRubricScore");
    yield* validateProbeStudybenchRubricScore(rubricScore);
    return rubricScore;
  });
}

export function validateOpenAgentsStudybenchTask(
  task: OpenAgentsStudybenchTask,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(task.id, "studybenchTask.id");
    yield* requireNonEmpty(task.topic, "studybenchTask.topic");
    yield* requireNonEmpty(task.question, "studybenchTask.question");
    yield* requireNonEmpty(task.gold_answer, "studybenchTask.gold_answer");
    yield* requireNonEmpty(task.repo, "studybenchTask.repo");
    yield* requireNonEmpty(task.commit, "studybenchTask.commit");
    yield* requireNonEmpty(task.corpusRef, "studybenchTask.corpusRef");
    yield* requireNonEmptyRefs(task.authorityRefs, "studybenchTask.authorityRefs");
    yield* requireNonEmptyRefs(task.privateMaterialPolicyRefs, "studybenchTask.privateMaterialPolicyRefs");

    if (task.rubric.length === 0) {
      return yield* studybenchError("studybenchTask.rubric", "must include at least one weighted claim");
    }

    if (task.evidence.length === 0) {
      return yield* studybenchError("studybenchTask.evidence", "must include at least one source evidence span");
    }

    yield* validateEvidenceSpans(task.evidence);
    yield* validateRubricClaims(task.rubric, new Set(task.evidence.map((span) => span.span_id)));
  });
}

export function validateOpenAgentsStudybenchDatasetPackage(
  packageRecord: OpenAgentsStudybenchDatasetPackage,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(packageRecord.datasetRef, "studybenchDatasetPackage.datasetRef");
    yield* requireNonEmpty(packageRecord.packageRef, "studybenchDatasetPackage.packageRef");

    if (packageRecord.tasks.length === 0) {
      return yield* studybenchError("studybenchDatasetPackage.tasks", "must include at least one task");
    }

    const ids = new Set<string>();

    for (const [index, task] of packageRecord.tasks.entries()) {
      yield* validateOpenAgentsStudybenchTask(task);

      if (ids.has(task.id)) {
        return yield* studybenchError(`studybenchDatasetPackage.tasks[${index}].id`, "must be unique in the package");
      }

      ids.add(task.id);

      if (packageRecord.sourceBoundary === "public_refs_only" && isPrivateStudybenchVisibility(task.visibility)) {
        return yield* studybenchError(
          `studybenchDatasetPackage.tasks[${index}].visibility`,
          "public StudyBench packages cannot include private validation or holdout rows",
        );
      }
    }
  });
}

function validateEvidenceSpans(
  evidence: ReadonlyArray<OpenAgentsStudybenchEvidenceSpan>,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    const spanIds = new Set<string>();

    for (const [index, span] of evidence.entries()) {
      const path = `studybenchTask.evidence[${index}]`;
      yield* requireNonEmpty(span.span_id, `${path}.span_id`);
      yield* requireNonEmpty(span.path, `${path}.path`);
      yield* requireNonEmpty(span.excerpt, `${path}.excerpt`);

      if (spanIds.has(span.span_id)) {
        return yield* studybenchError(`${path}.span_id`, "must be unique in the task evidence");
      }

      spanIds.add(span.span_id);

      if (!Number.isInteger(span.start_line) || !Number.isInteger(span.end_line) || span.start_line < 1) {
        return yield* studybenchError(`${path}.start_line`, "source line ranges must use positive integer lines");
      }

      if (span.end_line < span.start_line) {
        return yield* studybenchError(`${path}.end_line`, "must be greater than or equal to start_line");
      }
    }
  });
}

function validateRubricClaims(
  rubric: ReadonlyArray<OpenAgentsStudybenchRubricClaim>,
  evidenceSpanIds: ReadonlySet<string>,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    const claimIds = new Set<string>();
    let totalWeight = 0;
    let coreClaims = 0;

    for (const [index, claim] of rubric.entries()) {
      const path = `studybenchTask.rubric[${index}]`;
      yield* requireNonEmpty(claim.claim_id, `${path}.claim_id`);
      yield* requireNonEmpty(claim.statement, `${path}.statement`);

      if (claimIds.has(claim.claim_id)) {
        return yield* studybenchError(`${path}.claim_id`, "must be unique in the task rubric");
      }

      claimIds.add(claim.claim_id);

      if (!Number.isInteger(claim.weight) || claim.weight <= 0) {
        return yield* studybenchError(`${path}.weight`, "must be a positive integer");
      }

      totalWeight += claim.weight;
      coreClaims += claim.claim_type === "core" ? 1 : 0;

      if (claim.span_ids.length === 0) {
        return yield* studybenchError(`${path}.span_ids`, "must include at least one evidence span id");
      }

      for (const [spanIndex, spanId] of claim.span_ids.entries()) {
        yield* requireNonEmpty(spanId, `${path}.span_ids[${spanIndex}]`);

        if (!evidenceSpanIds.has(spanId)) {
          return yield* studybenchError(`${path}.span_ids[${spanIndex}]`, "must resolve to task evidence");
        }
      }
    }

    if (coreClaims === 0) {
      return yield* studybenchError("studybenchTask.rubric", "must include at least one core claim");
    }

    if (totalWeight !== 100) {
      return yield* studybenchError("studybenchTask.rubric", "claim weights must sum to 100");
    }
  });
}

function validateProbeStudybenchClaimScore(
  claimScore: ProbeStudybenchClaimScore,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(claimScore.claimId, "studybenchClaimScore.claimId");
    yield* requireNonEmpty(claimScore.rationaleRef, "studybenchClaimScore.rationaleRef");
    yield* requireNonEmpty(claimScore.scorerRef, "studybenchClaimScore.scorerRef");

    if (!Number.isInteger(claimScore.weight) || claimScore.weight <= 0) {
      return yield* studybenchError("studybenchClaimScore.weight", "must be a positive integer");
    }

    yield* requireScoreBps(claimScore.scoreBps, "studybenchClaimScore.scoreBps");
  });
}

function validateProbeStudybenchRubricScore(
  rubricScore: ProbeStudybenchRubricScore,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(rubricScore.taskId, "studybenchRubricScore.taskId");
    yield* requireNonEmpty(rubricScore.candidateHash, "studybenchRubricScore.candidateHash");
    yield* requireNonEmpty(rubricScore.goldAnswerRef, "studybenchRubricScore.goldAnswerRef");
    yield* requireNonEmptyRefs(rubricScore.evidenceUseRefs, "studybenchRubricScore.evidenceUseRefs");

    if (rubricScore.claimScores.length === 0) {
      return yield* studybenchError("studybenchRubricScore.claimScores", "must include at least one claim score");
    }

    yield* requireScoreBps(rubricScore.weightedScoreBps, "studybenchRubricScore.weightedScoreBps");
    yield* requireScoreBps(rubricScore.finalScoreBps, "studybenchRubricScore.finalScoreBps");

    for (const claimScore of rubricScore.claimScores) {
      yield* validateProbeStudybenchClaimScore(claimScore);
    }
  });
}

function isPrivateStudybenchVisibility(visibility: OpenAgentsStudybenchVisibility): boolean {
  return visibility === "openagents_private_validation" || visibility === "openagents_private_holdout";
}

function decodeStudybenchSchema<A, I>(
  schema: S.Schema<A, I>,
  value: unknown,
  path: string,
): Effect.Effect<A, ProbeBenchmarkContractError> {
  return S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(
      (error) =>
        new ProbeBenchmarkContractError({
          path,
          reason: String(error),
        }),
    ),
  );
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? studybenchError(path, "must be a non-empty string")
    : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return studybenchError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1
    ? Effect.void
    : studybenchError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function requireScoreBps(value: number, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Number.isInteger(value) && value >= 0 && value <= 10_000
    ? Effect.void
    : studybenchError(path, "must be an integer basis-point score from 0 to 10000");
}

function studybenchError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
