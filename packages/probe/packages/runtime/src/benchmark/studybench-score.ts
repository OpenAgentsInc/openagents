import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  type ProbeBenchmarkRedactionState,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
  type ProbeStudybenchRubricScore,
  decodeOpenAgentsStudybenchTask,
  decodeProbeStudybenchClaimScore,
  decodeProbeStudybenchRubricScore,
} from "./studybench";

export const ProbeStudybenchScoringMode = S.Literals(["manual_or_judge_supplied", "deterministic_check"]);
export type ProbeStudybenchScoringMode = typeof ProbeStudybenchScoringMode.Type;

export const PROBE_STUDYBENCH_SCORER_REFS: Record<ProbeStudybenchScoringMode, string> = {
  deterministic_check: "scorer.probe.studybench.deterministic_check.v0",
  manual_or_judge_supplied: "scorer.probe.studybench.manual_or_judge_supplied.v0",
};

export interface BuildProbeStudybenchRubricScoreInput {
  readonly candidateHash: string;
  readonly claimScores: ReadonlyArray<ProbeStudybenchClaimScore>;
  readonly evidenceUseRefs: ReadonlyArray<string>;
  readonly goldAnswerRef: string;
  readonly redactionState?: ProbeBenchmarkRedactionState;
  readonly scoringMode?: ProbeStudybenchScoringMode;
  readonly strictCoreGate?: boolean;
  readonly task: OpenAgentsStudybenchTask;
}

export function buildProbeStudybenchRubricScore(
  input: BuildProbeStudybenchRubricScoreInput,
): Effect.Effect<ProbeStudybenchRubricScore, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const task = yield* decodeOpenAgentsStudybenchTask(input.task);
    const scoringMode = input.scoringMode ?? "manual_or_judge_supplied";
    const claimScores = yield* decodeStudybenchClaimScores(input.claimScores);

    yield* validateProbeStudybenchScoreVector({
      claimScores,
      scoringMode,
      task,
    });

    const weightedScoreBps = computeProbeStudybenchWeightedScoreBps(claimScores);
    const coreGatePassed = isProbeStudybenchCoreGatePassed(task, claimScores, input.strictCoreGate ?? true);
    const rubricScore: ProbeStudybenchRubricScore = {
      schemaRef: PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
      candidateHash: input.candidateHash,
      claimScores: [...claimScores],
      coreGatePassed,
      evidenceUseRefs: [...input.evidenceUseRefs],
      finalScoreBps: coreGatePassed ? weightedScoreBps : 0,
      goldAnswerRef: input.goldAnswerRef,
      redactionState: input.redactionState ?? "public_safe",
      taskId: task.id,
      weightedScoreBps,
    };

    yield* validateProbeStudybenchRubricScoreRefs(rubricScore);
    yield* validateProbeBenchmarkPublicProjection(rubricScore, "studybenchRubricScore");
    return yield* decodeProbeStudybenchRubricScore(rubricScore);
  });
}

export function validateProbeStudybenchScoreVector(input: {
  readonly claimScores: ReadonlyArray<ProbeStudybenchClaimScore>;
  readonly scoringMode?: ProbeStudybenchScoringMode;
  readonly task: OpenAgentsStudybenchTask;
}): Effect.Effect<void, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const task = yield* decodeOpenAgentsStudybenchTask(input.task);
    const scoringMode = input.scoringMode ?? "manual_or_judge_supplied";
    const expectedScorerRef = PROBE_STUDYBENCH_SCORER_REFS[scoringMode];

    if (input.claimScores.length !== task.rubric.length) {
      return yield* studybenchScoreError(
        "studybenchScore.claimScores",
        "must include exactly one score for every task rubric claim",
      );
    }

    const rubricByClaimId = new Map(task.rubric.map((claim) => [claim.claim_id, claim] as const));
    const seenClaimIds = new Set<string>();

    for (const [index, score] of input.claimScores.entries()) {
      yield* validateProbeStudybenchClaimScoreRefs(score, index);

      if (score.scorerRef !== expectedScorerRef) {
        return yield* studybenchScoreError(
          `studybenchScore.claimScores[${index}].scorerRef`,
          `must match ${expectedScorerRef} for ${scoringMode} scoring`,
        );
      }

      const rubricClaim = rubricByClaimId.get(score.claimId);

      if (rubricClaim === undefined) {
        return yield* studybenchScoreError(
          `studybenchScore.claimScores[${index}].claimId`,
          "must resolve to a task rubric claim",
        );
      }

      if (seenClaimIds.has(score.claimId)) {
        return yield* studybenchScoreError(
          `studybenchScore.claimScores[${index}].claimId`,
          "must be unique in the score vector",
        );
      }

      seenClaimIds.add(score.claimId);

      if (score.claimType !== rubricClaim.claim_type) {
        return yield* studybenchScoreError(
          `studybenchScore.claimScores[${index}].claimType`,
          "must match the task rubric claim type",
        );
      }

      if (score.weight !== rubricClaim.weight) {
        return yield* studybenchScoreError(
          `studybenchScore.claimScores[${index}].weight`,
          "must match the task rubric claim weight",
        );
      }

      if (!sameOrderedStrings(score.evidenceSpanIds, rubricClaim.span_ids)) {
        return yield* studybenchScoreError(
          `studybenchScore.claimScores[${index}].evidenceSpanIds`,
          "must match the task rubric evidence span ids",
        );
      }
    }
  });
}

export function computeProbeStudybenchWeightedScoreBps(
  claimScores: ReadonlyArray<ProbeStudybenchClaimScore>,
): number {
  const weightedTotal = claimScores.reduce((total, score) => total + score.weight * score.scoreBps, 0);
  return Math.round(weightedTotal / 100);
}

export function isProbeStudybenchCoreGatePassed(
  task: OpenAgentsStudybenchTask,
  claimScores: ReadonlyArray<ProbeStudybenchClaimScore>,
  strictCoreGate = true,
): boolean {
  if (!strictCoreGate) {
    return true;
  }

  const scoreByClaimId = new Map(claimScores.map((score) => [score.claimId, score] as const));
  return task.rubric.every((claim) => {
    if (claim.claim_type !== "core") {
      return true;
    }

    return scoreByClaimId.get(claim.claim_id)?.satisfied === true;
  });
}

function decodeStudybenchClaimScores(
  values: ReadonlyArray<ProbeStudybenchClaimScore>,
): Effect.Effect<ReadonlyArray<ProbeStudybenchClaimScore>, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.all(values.map((value) => decodeProbeStudybenchClaimScore(value)));
}

function validateProbeStudybenchRubricScoreRefs(
  rubricScore: ProbeStudybenchRubricScore,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireOpaqueRef(rubricScore.candidateHash, "studybenchRubricScore.candidateHash");
    yield* requireOpaqueRef(rubricScore.goldAnswerRef, "studybenchRubricScore.goldAnswerRef");

    for (const [index, ref] of rubricScore.evidenceUseRefs.entries()) {
      yield* requireOpaqueRef(ref, `studybenchRubricScore.evidenceUseRefs[${index}]`);
    }
  });
}

function validateProbeStudybenchClaimScoreRefs(
  claimScore: ProbeStudybenchClaimScore,
  index: number,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireOpaqueRef(claimScore.claimId, `studybenchScore.claimScores[${index}].claimId`);
    yield* requireOpaqueRef(claimScore.rationaleRef, `studybenchScore.claimScores[${index}].rationaleRef`);
    yield* requireOpaqueRef(claimScore.scorerRef, `studybenchScore.claimScores[${index}].scorerRef`);

    for (const [spanIndex, spanId] of claimScore.evidenceSpanIds.entries()) {
      yield* requireOpaqueRef(spanId, `studybenchScore.claimScores[${index}].evidenceSpanIds[${spanIndex}]`);
    }
  });
}

function requireOpaqueRef(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (value.trim().length === 0) {
    return studybenchScoreError(path, "must be a non-empty ref");
  }

  if (/\s/.test(value) || /[.!?]$/.test(value) || /because|critique/i.test(value)) {
    return studybenchScoreError(path, "must be an opaque artifact ref, not raw evaluator text");
  }

  return Effect.void;
}

function sameOrderedStrings(left: ReadonlyArray<string>, right: ReadonlyArray<string>): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function studybenchScoreError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
