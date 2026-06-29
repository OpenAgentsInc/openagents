import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  decodeProbeBenchmarkAssignment,
  type ProbeBenchmarkAssignment,
  type ProbeBenchmarkFailureClassification,
  type ProbeBenchmarkRouteScorecard,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe, validateProbePublicProjection } from "../contracts/provider-account";
import {
  ProbeBenchmarkCandidateExecutionError,
  decodeProbeGepaCandidateManifest,
  type ProbeGepaCandidateManifest,
} from "./candidate-execution";
import {
  makeProbeBenchmarkCloseoutBundle,
  type ProbeBenchmarkCloseoutBundle,
  type ProbeBenchmarkCloseoutWriterError,
  type ProbeBenchmarkTerminalRunStatus,
} from "./closeout-writer";
import {
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
  type ProbeStudybenchRubricScore,
  decodeOpenAgentsStudybenchTask,
} from "./studybench";
import {
  PROBE_STUDYBENCH_SCORER_REFS,
  buildProbeStudybenchRubricScore,
  type ProbeStudybenchScoringMode,
} from "./studybench-score";
import { shortHash } from "./stable-hash";

export const PROBE_STUDYBENCH_ANSWER_CANDIDATE_INPUT_SCHEMA_REF =
  "probe.studybench_answer_candidate_input.v0" as const;

export interface ProbeStudybenchAnswerCandidateInput {
  readonly budgetClass: OpenAgentsStudybenchTask["budgetClass"];
  readonly candidateAnswerRef: string;
  readonly corpusRef: string;
  readonly expectedFiles: ReadonlyArray<string>;
  readonly goldAnswerVisible: false;
  readonly question: string;
  readonly rubricVisible: false;
  readonly schemaRef: typeof PROBE_STUDYBENCH_ANSWER_CANDIDATE_INPUT_SCHEMA_REF;
  readonly scorerMaterialWithheld: true;
  readonly taskId: string;
  readonly topic: string;
  readonly visibility: OpenAgentsStudybenchTask["visibility"];
}

export interface ProbeStudybenchAnswerRunnerInput {
  readonly assignment: ProbeBenchmarkAssignment | unknown;
  readonly candidateAnswerArtifactRef?: string;
  readonly candidateAnswerRef: string;
  readonly candidateManifest?: ProbeGepaCandidateManifest | unknown;
  readonly claimScores: ReadonlyArray<ProbeStudybenchClaimScore>;
  readonly completedAt?: string;
  readonly costRef?: string;
  readonly evidenceUseRefs?: ReadonlyArray<string>;
  readonly resourceUnavailableReason?: string;
  readonly resourceUsageRef?: string;
  readonly routeScorecard?: ProbeBenchmarkRouteScorecard;
  readonly runRef?: string;
  readonly scoringMode?: ProbeStudybenchScoringMode;
  readonly startedAt?: string;
  readonly task: OpenAgentsStudybenchTask | unknown;
  readonly verifierResultRefs?: ReadonlyArray<string>;
}

export interface ProbeStudybenchAnswerRunnerResult {
  readonly assignment: ProbeBenchmarkAssignment;
  readonly bundle: ProbeBenchmarkCloseoutBundle;
  readonly candidateComponentRefs: ReadonlyArray<string>;
  readonly candidateHash: string;
  readonly candidateInput: ProbeStudybenchAnswerCandidateInput;
  readonly mode: "baseline" | "candidate";
  readonly rubricScore: ProbeStudybenchRubricScore;
  readonly runStatus: ProbeBenchmarkTerminalRunStatus;
}

export class ProbeStudybenchAnswerRunnerError extends S.TaggedErrorClass<ProbeStudybenchAnswerRunnerError>()(
  "ProbeStudybenchAnswerRunnerError",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export function runProbeStudybenchAnswerCandidate(
  input: ProbeStudybenchAnswerRunnerInput,
): Effect.Effect<
  ProbeStudybenchAnswerRunnerResult,
  | ProbeStudybenchAnswerRunnerError
  | ProbeBenchmarkCandidateExecutionError
  | ProbeBenchmarkCloseoutWriterError
  | ProbeBenchmarkContractError
  | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbePublicProjection(input, "studybenchAnswerRunnerInput");

    const task = yield* decodeOpenAgentsStudybenchTask(input.task);
    const decodedAssignment = yield* decodeProbeBenchmarkAssignment(input.assignment);
    const candidateManifest = input.candidateManifest === undefined
      ? undefined
      : yield* decodeProbeGepaCandidateManifest(input.candidateManifest);

    yield* validateStudybenchAssignmentCompatibility(task, decodedAssignment);

    const candidateHash = candidateManifest?.candidate_hash ?? decodedAssignment.candidateHash;
    const assignment = assignmentForCandidate(decodedAssignment, candidateManifest);
    const candidateComponentRefs = candidateManifest === undefined ? [] : candidateComponentRefsFromManifest(candidateManifest);
    const candidateInput = yield* candidateInputFor(task, input.candidateAnswerRef);
    const runRef = input.runRef ?? `probe_run.studybench_answer.${task.id}.${shortHash(candidateHash)}`;
    const evidenceUseRefs = input.evidenceUseRefs ?? [`evidence_use.probe.studybench.answer.${task.id}.${shortHash(candidateHash)}`];
    const scoringMode = input.scoringMode ?? "manual_or_judge_supplied";
    const rubricScore = yield* buildProbeStudybenchRubricScore({
      candidateHash,
      claimScores: input.claimScores,
      evidenceUseRefs,
      goldAnswerRef: `gold_answer.openagents_studybench.${visibilityRefPart(task.visibility)}.${task.id}`,
      scoringMode,
      task,
    });
    const runStatus = runStatusForRubricScore(rubricScore);
    const failureClassification = failureClassificationFor(task, runStatus);
    const candidateAnswerArtifactRef = input.candidateAnswerArtifactRef ?? `artifact.probe.studybench_answer.${task.id}.${shortHash(candidateHash)}`;
    const scoreRef = `rubric_score.probe.studybench_answer.${task.id}.${shortHash(candidateHash)}`;

    const bundle = yield* makeProbeBenchmarkCloseoutBundle({
      assignment,
      artifactManifestRefs: [candidateAnswerArtifactRef],
      candidateComponentRefs,
      completedAt: input.completedAt,
      costRef: input.costRef,
      decisionStepRefs: [
        `decision_step.probe.studybench_answer.${task.id}.candidate_input_public_safe`,
        `decision_step.probe.studybench_answer.${task.id}.rubric_score`,
      ],
      failureClassification,
      partialArtifactRefs: runStatus === "succeeded" ? [] : [candidateAnswerArtifactRef],
      resourceUnavailableReason: input.resourceUnavailableReason,
      resourceUsageRef: input.resourceUsageRef ?? `resource_usage.${runRef}`,
      routeScorecard: input.routeScorecard,
      runRef,
      runStatus,
      scorerRef: PROBE_STUDYBENCH_SCORER_REFS[scoringMode],
      startedAt: input.startedAt,
      studybenchEvidenceUseRefs: evidenceUseRefs,
      studybenchRubricScore: rubricScore,
      studybenchScoreRef: scoreRef,
      studybenchTaskRef: `studybench_task.${task.visibility}.${task.id}`,
      toolMenuSnapshot: {
        schemaRef: "probe.studybench_answer_tool_menu_snapshot.v0",
        candidateAnswerRef: input.candidateAnswerRef,
        mode: "answer_only_no_repo_patch",
      },
      verifierRef: "verifier.probe.studybench.answer_mode.v0",
      verifierResultRefs: input.verifierResultRefs ?? [`verifier_result.probe.studybench_answer.${task.id}.${runStatus}`],
    });

    return {
      assignment,
      bundle,
      candidateComponentRefs,
      candidateHash,
      candidateInput,
      mode: candidateManifest === undefined ? "baseline" : "candidate",
      rubricScore,
      runStatus,
    };
  });
}

function validateStudybenchAssignmentCompatibility(
  task: OpenAgentsStudybenchTask,
  assignment: ProbeBenchmarkAssignment,
): Effect.Effect<void, ProbeStudybenchAnswerRunnerError> {
  return Effect.gen(function* () {
    const expectedSplit = splitForStudybenchVisibility(task.visibility);

    if (assignment.split.evidenceSplit !== expectedSplit) {
      return yield* answerRunnerError(
        "studybenchAnswerRunnerInput.assignment.split.evidenceSplit",
        `must be ${expectedSplit} for ${task.visibility}`,
      );
    }

    if (assignment.task.taskRef !== undefined && !assignment.task.taskRef.includes(task.id)) {
      return yield* answerRunnerError(
        "studybenchAnswerRunnerInput.assignment.task.taskRef",
        "must reference the StudyBench task id when taskRef is present",
      );
    }
  });
}

function candidateInputFor(
  task: OpenAgentsStudybenchTask,
  candidateAnswerRef: string,
): Effect.Effect<ProbeStudybenchAnswerCandidateInput, ProbePublicProjectionUnsafe> {
  const candidateInput: ProbeStudybenchAnswerCandidateInput = {
    schemaRef: PROBE_STUDYBENCH_ANSWER_CANDIDATE_INPUT_SCHEMA_REF,
    budgetClass: task.budgetClass,
    candidateAnswerRef,
    corpusRef: task.corpusRef,
    expectedFiles: [...task.expectedFiles],
    goldAnswerVisible: false,
    question: task.question,
    rubricVisible: false,
    scorerMaterialWithheld: true,
    taskId: task.id,
    topic: task.topic,
    visibility: task.visibility,
  };

  return validateProbePublicProjection(candidateInput, "studybenchAnswerCandidateInput").pipe(
    Effect.as(candidateInput),
  );
}

function assignmentForCandidate(
  assignment: ProbeBenchmarkAssignment,
  candidateManifest: ProbeGepaCandidateManifest | undefined,
): ProbeBenchmarkAssignment {
  if (candidateManifest === undefined) {
    return assignment;
  }

  return {
    ...assignment,
    candidateHash: candidateManifest.candidate_hash,
    candidateRefs: {
      blueprintCandidateRef: candidateManifest.probe_import.blueprint_candidate_ref,
      loopPolicyCandidateRef: candidateManifest.probe_import.loop_policy_candidate_ref,
      promptCandidateRef: candidateManifest.probe_import.prompt_candidate_ref,
      toolMenuCandidateRef: candidateManifest.probe_import.tool_menu_candidate_ref,
    },
  };
}

function candidateComponentRefsFromManifest(manifest: ProbeGepaCandidateManifest): ReadonlyArray<string> {
  const refs: string[] = [];

  for (const [component, hash] of Object.entries(manifest.component_hashes)) {
    if (typeof hash === "string") {
      refs.push(`candidate_component.${manifest.candidate_id}.${component}.${shortHash(hash)}`);
      continue;
    }

    for (const [family, familyHash] of Object.entries(hash)) {
      refs.push(`candidate_component.${manifest.candidate_id}.failure_family_playbooks.${family}.${shortHash(familyHash)}`);
    }
  }

  return refs;
}

function splitForStudybenchVisibility(visibility: OpenAgentsStudybenchTask["visibility"]): ProbeBenchmarkAssignment["split"]["evidenceSplit"] {
  switch (visibility) {
    case "external_public_calibration":
    case "openagents_public_retained":
      return "retained";
    case "openagents_private_validation":
      return "validation";
    case "openagents_private_holdout":
      return "holdout";
  }
}

function runStatusForRubricScore(rubricScore: ProbeStudybenchRubricScore): ProbeBenchmarkTerminalRunStatus {
  return rubricScore.finalScoreBps === 10_000 ? "succeeded" : "failed";
}

function failureClassificationFor(
  task: OpenAgentsStudybenchTask,
  runStatus: ProbeBenchmarkTerminalRunStatus,
): ProbeBenchmarkFailureClassification | undefined {
  if (runStatus === "succeeded") {
    return undefined;
  }

  return {
    classificationRef: `failure_classification.probe.studybench_answer.${task.id}.verifier_failure`,
    family: "verifier_failure",
    summaryRef: `summary.failure.probe.studybench_answer.${task.id}`,
  };
}

function visibilityRefPart(visibility: OpenAgentsStudybenchTask["visibility"]): string {
  switch (visibility) {
    case "external_public_calibration":
      return "external_public_calibration";
    case "openagents_public_retained":
      return "public_retained";
    case "openagents_private_validation":
      return "private_validation";
    case "openagents_private_holdout":
      return "private_holdout";
  }
}

function answerRunnerError(path: string, reason: string): Effect.Effect<never, ProbeStudybenchAnswerRunnerError> {
  return Effect.fail(new ProbeStudybenchAnswerRunnerError({ path, reason }));
}
