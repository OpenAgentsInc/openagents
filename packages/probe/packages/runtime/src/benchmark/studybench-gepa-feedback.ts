import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
  type ProbeBenchmarkRedactionState,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchRubricScore,
  decodeOpenAgentsStudybenchTask,
  decodeProbeStudybenchRubricScore,
} from "./studybench";
import {
  validateProbeStudybenchScoreVector,
  type ProbeStudybenchScoringMode,
} from "./studybench-score";
import { shortHash } from "./stable-hash";

export const PROBE_STUDYBENCH_GEPA_FEEDBACK_SCHEMA_REF =
  "probe.studybench_gepa_feedback.v0" as const;
export const PROBE_STUDYBENCH_GEPA_FEEDBACK_TARGET_SUITE_REFS = [
  "target_suite.openagents_studybench.public_retained.v0",
  "target_suite.openagents_studybench.private_validation.v0",
] as const;

export interface BuildProbeStudybenchGepaFeedbackInput {
  readonly budgetFailureRefs?: ReadonlyArray<string>;
  readonly feedbackRef?: string;
  readonly rubricScore: ProbeStudybenchRubricScore | unknown;
  readonly scoringMode?: ProbeStudybenchScoringMode;
  readonly skippedTestRefs?: ReadonlyArray<string>;
  readonly task: OpenAgentsStudybenchTask | unknown;
  readonly wrongFileRefs?: ReadonlyArray<string>;
}

export interface ProbeStudybenchGepaFeedback {
  readonly budgetFailureRefs: ReadonlyArray<string>;
  readonly candidateHash: string;
  readonly coreGatePassed: boolean;
  readonly failedClaimRefs: ReadonlyArray<string>;
  readonly failedCoreClaimRefs: ReadonlyArray<string>;
  readonly failedSupportingClaimRefs: ReadonlyArray<string>;
  readonly feedbackRef: string;
  readonly finalScoreBps: number;
  readonly forbiddenClaimRefs: ReadonlyArray<string>;
  readonly missedEvidenceSpanRefs: ReadonlyArray<string>;
  readonly optimizerAcceptanceBoundaryRef: string;
  readonly payoutAuthorityAllowed: false;
  readonly publicClaimAuthorityAllowed: false;
  readonly rawGoldAnswerIncluded: false;
  readonly rawJudgeRationaleIncluded: false;
  readonly redactionState: ProbeBenchmarkRedactionState;
  readonly runtimePromotionAllowed: false;
  readonly schemaRef: typeof PROBE_STUDYBENCH_GEPA_FEEDBACK_SCHEMA_REF;
  readonly skippedTestRefs: ReadonlyArray<string>;
  readonly splitVisibility: OpenAgentsStudybenchTask["visibility"];
  readonly targetSuiteRefs: ReadonlyArray<string>;
  readonly taskId: string;
  readonly weightedScoreBps: number;
  readonly wrongFileRefs: ReadonlyArray<string>;
}

export function buildProbeStudybenchGepaFeedback(
  input: BuildProbeStudybenchGepaFeedbackInput,
): Effect.Effect<
  ProbeStudybenchGepaFeedback,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const task = yield* decodeOpenAgentsStudybenchTask(input.task);
    const rubricScore = yield* decodeProbeStudybenchRubricScore(input.rubricScore);

    yield* validateProbeStudybenchScoreVector({
      claimScores: rubricScore.claimScores,
      scoringMode: input.scoringMode ?? "manual_or_judge_supplied",
      task,
    });

    if (rubricScore.taskId !== task.id) {
      return yield* gepaFeedbackError(
        "studybenchGepaFeedback.rubricScore.taskId",
        "must match the StudyBench task id",
      );
    }

    const failedClaims = rubricScore.claimScores.filter((claim) => !claim.satisfied || claim.scoreBps < 10_000);
    const failedCoreClaimRefs = failedClaims
      .filter((claim) => claim.claimType === "core")
      .map((claim) => feedbackClaimRef(task.id, claim.claimId, "core"));
    const failedSupportingClaimRefs = failedClaims
      .filter((claim) => claim.claimType === "supporting")
      .map((claim) => feedbackClaimRef(task.id, claim.claimId, "supporting"));
    const missedEvidenceSpanRefs = uniqueStrings(
      failedClaims.flatMap((claim) =>
        claim.evidenceSpanIds.map((spanId) => `gepa_feedback.openagents_studybench.${task.id}.span.${spanId}.missed`),
      ),
    );
    const feedback: ProbeStudybenchGepaFeedback = {
      schemaRef: PROBE_STUDYBENCH_GEPA_FEEDBACK_SCHEMA_REF,
      budgetFailureRefs: [...(input.budgetFailureRefs ?? [])],
      candidateHash: rubricScore.candidateHash,
      coreGatePassed: rubricScore.coreGatePassed,
      failedClaimRefs: [...failedCoreClaimRefs, ...failedSupportingClaimRefs],
      failedCoreClaimRefs,
      failedSupportingClaimRefs,
      feedbackRef: input.feedbackRef ?? `gepa_feedback.openagents_studybench.${task.id}.${shortHash(rubricScore.candidateHash)}`,
      finalScoreBps: rubricScore.finalScoreBps,
      forbiddenClaimRefs: [...task.forbiddenClaimRefs],
      missedEvidenceSpanRefs,
      optimizerAcceptanceBoundaryRef: "boundary.psionic.gepa.optimizer_acceptance_not_runtime_promotion.v0",
      payoutAuthorityAllowed: false,
      publicClaimAuthorityAllowed: false,
      rawGoldAnswerIncluded: false,
      rawJudgeRationaleIncluded: false,
      redactionState: rubricScore.redactionState,
      runtimePromotionAllowed: false,
      skippedTestRefs: [...(input.skippedTestRefs ?? [])],
      splitVisibility: task.visibility,
      targetSuiteRefs: [...PROBE_STUDYBENCH_GEPA_FEEDBACK_TARGET_SUITE_REFS],
      taskId: task.id,
      weightedScoreBps: rubricScore.weightedScoreBps,
      wrongFileRefs: [...(input.wrongFileRefs ?? [])],
    };

    yield* validateProbeBenchmarkPublicProjection(feedback, "studybenchGepaFeedback");
    yield* validateFeedbackRefs(feedback);
    return feedback;
  });
}

function feedbackClaimRef(taskId: string, claimId: string, claimType: string): string {
  return `gepa_feedback.openagents_studybench.${taskId}.claim.${claimId}.${claimType}_failed`;
}

function validateFeedbackRefs(
  feedback: ProbeStudybenchGepaFeedback,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    for (const [index, ref] of [
      feedback.feedbackRef,
      feedback.candidateHash,
      feedback.optimizerAcceptanceBoundaryRef,
      ...feedback.failedClaimRefs,
      ...feedback.missedEvidenceSpanRefs,
      ...feedback.forbiddenClaimRefs,
      ...feedback.skippedTestRefs,
      ...feedback.wrongFileRefs,
      ...feedback.budgetFailureRefs,
      ...feedback.targetSuiteRefs,
    ].entries()) {
      if (/\s/.test(ref) || /because|critique|rationale/i.test(ref)) {
        return yield* gepaFeedbackError(
          `studybenchGepaFeedback.refs[${index}]`,
          "must contain opaque refs only",
        );
      }
    }
  });
}

function uniqueStrings(values: ReadonlyArray<string>): ReadonlyArray<string> {
  return [...new Set(values)];
}

function gepaFeedbackError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
