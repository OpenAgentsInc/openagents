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

export const PROBE_STUDYBENCH_PATCH_CANDIDATE_INPUT_SCHEMA_REF =
  "probe.studybench_patch_candidate_input.v0" as const;

export interface ProbeStudybenchPatchBudgetPolicy {
  readonly budgetPolicyRef: string;
  readonly maxToolCalls: number;
  readonly timeoutMs: number;
  readonly timeoutPolicyRef: string;
}

export interface ProbeStudybenchPatchTranscriptSummary {
  readonly observedDurationMs: number;
  readonly toolCallRefs: ReadonlyArray<string>;
  readonly transcriptSummaryRef: string;
  readonly usedToolRefs: ReadonlyArray<string>;
}

export interface ProbeStudybenchPatchCandidateInput {
  readonly allowedToolRefs: ReadonlyArray<string>;
  readonly budgetPolicyRef: string;
  readonly corpusRef: string;
  readonly expectedFiles: ReadonlyArray<string>;
  readonly evidenceExcerptsVisible: false;
  readonly goldAnswerVisible: false;
  readonly maxToolCalls: number;
  readonly pinnedCheckoutRef: string;
  readonly question: string;
  readonly rubricVisible: false;
  readonly schemaRef: typeof PROBE_STUDYBENCH_PATCH_CANDIDATE_INPUT_SCHEMA_REF;
  readonly scorerMaterialWithheld: true;
  readonly taskId: string;
  readonly testCommandRefs: ReadonlyArray<string>;
  readonly timeoutMs: number;
  readonly timeoutPolicyRef: string;
  readonly topic: string;
  readonly visibility: OpenAgentsStudybenchTask["visibility"];
}

export interface ProbeStudybenchPatchRunnerInput {
  readonly allowedToolRefs: ReadonlyArray<string>;
  readonly assignment: ProbeBenchmarkAssignment | unknown;
  readonly budgetPolicy: ProbeStudybenchPatchBudgetPolicy;
  readonly candidateManifest?: ProbeGepaCandidateManifest | unknown;
  readonly claimScores: ReadonlyArray<ProbeStudybenchClaimScore>;
  readonly completedAt?: string;
  readonly costRef?: string;
  readonly evidenceUseRefs?: ReadonlyArray<string>;
  readonly patchArtifactRefs: ReadonlyArray<string>;
  readonly pinnedCheckoutRef: string;
  readonly resourceUnavailableReason?: string;
  readonly resourceUsageRef?: string;
  readonly routeScorecard?: ProbeBenchmarkRouteScorecard;
  readonly runRef?: string;
  readonly runnerIdentityRef: string;
  readonly runnerTranscript: ProbeStudybenchPatchTranscriptSummary;
  readonly scoringMode?: ProbeStudybenchScoringMode;
  readonly startedAt?: string;
  readonly task: OpenAgentsStudybenchTask | unknown;
  readonly testCommandRefs: ReadonlyArray<string>;
  readonly verifierResultRefs?: ReadonlyArray<string>;
}

export interface ProbeStudybenchPatchRunnerResult {
  readonly assignment: ProbeBenchmarkAssignment;
  readonly bundle: ProbeBenchmarkCloseoutBundle;
  readonly candidateComponentRefs: ReadonlyArray<string>;
  readonly candidateHash: string;
  readonly candidateInput: ProbeStudybenchPatchCandidateInput;
  readonly mode: "baseline" | "candidate";
  readonly patchArtifactRefs: ReadonlyArray<string>;
  readonly rubricScore: ProbeStudybenchRubricScore;
  readonly runStatus: ProbeBenchmarkTerminalRunStatus;
  readonly testCommandRefs: ReadonlyArray<string>;
  readonly transcriptSummaryRef: string;
}

export class ProbeStudybenchPatchRunnerError extends S.TaggedErrorClass<ProbeStudybenchPatchRunnerError>()(
  "ProbeStudybenchPatchRunnerError",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export function runProbeStudybenchPatchCandidate(
  input: ProbeStudybenchPatchRunnerInput,
): Effect.Effect<
  ProbeStudybenchPatchRunnerResult,
  | ProbeStudybenchPatchRunnerError
  | ProbeBenchmarkCandidateExecutionError
  | ProbeBenchmarkCloseoutWriterError
  | ProbeBenchmarkContractError
  | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbePublicProjection(input, "studybenchPatchRunnerInput");

    const task = yield* decodeOpenAgentsStudybenchTask(input.task);
    const decodedAssignment = yield* decodeProbeBenchmarkAssignment(input.assignment);
    const candidateManifest = input.candidateManifest === undefined
      ? undefined
      : yield* decodeProbeGepaCandidateManifest(input.candidateManifest);

    yield* validateStudybenchPatchInput(task, decodedAssignment, input);

    const candidateHash = candidateManifest?.candidate_hash ?? decodedAssignment.candidateHash;
    const assignment = assignmentForCandidate(decodedAssignment, candidateManifest);
    const candidateComponentRefs = candidateManifest === undefined ? [] : candidateComponentRefsFromManifest(candidateManifest);
    const candidateInput = yield* candidateInputFor(task, input);
    const runRef = input.runRef ?? `probe_run.studybench_patch.${task.id}.${shortHash(candidateHash)}`;
    const evidenceUseRefs = input.evidenceUseRefs ?? [`evidence_use.probe.studybench.patch.${task.id}.${shortHash(candidateHash)}`];
    const scoringMode = input.scoringMode ?? "manual_or_judge_supplied";
    const rubricScore = yield* buildProbeStudybenchRubricScore({
      candidateHash,
      claimScores: input.claimScores,
      evidenceUseRefs,
      goldAnswerRef: `gold_answer.openagents_studybench.${visibilityRefPart(task.visibility)}.${task.id}`,
      scoringMode,
      task,
    });
    const runStatus = runStatusFor(input, rubricScore);
    const failureClassification = failureClassificationFor(task, runStatus);
    const scoreRef = `rubric_score.probe.studybench_patch.${task.id}.${shortHash(candidateHash)}`;

    const bundle = yield* makeProbeBenchmarkCloseoutBundle({
      assignment,
      artifactManifestRefs: [...input.patchArtifactRefs],
      candidateComponentRefs,
      completedAt: input.completedAt,
      costRef: input.costRef,
      decisionStepRefs: [
        `decision_step.probe.studybench_patch.${task.id}.candidate_input_public_safe`,
        input.runnerTranscript.transcriptSummaryRef,
        ...input.patchArtifactRefs,
        ...input.testCommandRefs,
      ],
      failureClassification,
      partialArtifactRefs: runStatus === "succeeded" ? [] : [...input.patchArtifactRefs],
      resourceUnavailableReason: runStatus === "timed_out"
        ? input.resourceUnavailableReason ?? "timeout_before_resource_meter_flush"
        : input.resourceUnavailableReason,
      resourceUsageRef: runStatus === "timed_out" ? input.resourceUsageRef : input.resourceUsageRef ?? `resource_usage.${runRef}`,
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
        schemaRef: "probe.studybench_patch_tool_menu_snapshot.v0",
        allowedToolRefs: [...input.allowedToolRefs],
        budgetPolicy: input.budgetPolicy,
        observedDurationMs: input.runnerTranscript.observedDurationMs,
        pinnedCheckoutRef: input.pinnedCheckoutRef,
        runnerIdentityRef: input.runnerIdentityRef,
        testCommandRefs: [...input.testCommandRefs],
        toolCallRefs: [...input.runnerTranscript.toolCallRefs],
        transcriptSummaryRef: input.runnerTranscript.transcriptSummaryRef,
        usedToolRefs: [...input.runnerTranscript.usedToolRefs],
      },
      verifierRef: "verifier.probe.studybench.patch_mode.v0",
      verifierResultRefs: input.verifierResultRefs ?? input.testCommandRefs,
    });

    return {
      assignment,
      bundle,
      candidateComponentRefs,
      candidateHash,
      candidateInput,
      mode: candidateManifest === undefined ? "baseline" : "candidate",
      patchArtifactRefs: [...input.patchArtifactRefs],
      rubricScore,
      runStatus,
      testCommandRefs: [...input.testCommandRefs],
      transcriptSummaryRef: input.runnerTranscript.transcriptSummaryRef,
    };
  });
}

function validateStudybenchPatchInput(
  task: OpenAgentsStudybenchTask,
  assignment: ProbeBenchmarkAssignment,
  input: ProbeStudybenchPatchRunnerInput,
): Effect.Effect<void, ProbeStudybenchPatchRunnerError> {
  return Effect.gen(function* () {
    const expectedSplit = splitForStudybenchVisibility(task.visibility);

    if (assignment.split.evidenceSplit !== expectedSplit) {
      return yield* patchRunnerError(
        "studybenchPatchRunnerInput.assignment.split.evidenceSplit",
        `must be ${expectedSplit} for ${task.visibility}`,
      );
    }

    if (assignment.task.taskRef !== undefined && !assignment.task.taskRef.includes(task.id)) {
      return yield* patchRunnerError(
        "studybenchPatchRunnerInput.assignment.task.taskRef",
        "must reference the StudyBench task id when taskRef is present",
      );
    }

    yield* requireNonEmpty(input.pinnedCheckoutRef, "studybenchPatchRunnerInput.pinnedCheckoutRef");
    yield* requireNonEmpty(input.runnerIdentityRef, "studybenchPatchRunnerInput.runnerIdentityRef");
    yield* requireNonEmpty(input.runnerTranscript.transcriptSummaryRef, "studybenchPatchRunnerInput.runnerTranscript.transcriptSummaryRef");
    yield* requireNonEmptyRefs(input.allowedToolRefs, "studybenchPatchRunnerInput.allowedToolRefs");
    yield* requireNonEmptyRefs(input.patchArtifactRefs, "studybenchPatchRunnerInput.patchArtifactRefs");
    yield* requireNonEmptyRefs(input.testCommandRefs, "studybenchPatchRunnerInput.testCommandRefs");
    yield* validateBudget(input.budgetPolicy);

    if (input.runnerTranscript.toolCallRefs.length > input.budgetPolicy.maxToolCalls) {
      return yield* patchRunnerError(
        "studybenchPatchRunnerInput.runnerTranscript.toolCallRefs",
        "exceeds maxToolCalls budget",
      );
    }

    const allowedToolRefs = new Set(input.allowedToolRefs);
    for (const [index, toolRef] of input.runnerTranscript.usedToolRefs.entries()) {
      yield* requireNonEmpty(toolRef, `studybenchPatchRunnerInput.runnerTranscript.usedToolRefs[${index}]`);

      if (!allowedToolRefs.has(toolRef)) {
        return yield* patchRunnerError(
          `studybenchPatchRunnerInput.runnerTranscript.usedToolRefs[${index}]`,
          "must be included in allowedToolRefs",
        );
      }
    }
  });
}

function candidateInputFor(
  task: OpenAgentsStudybenchTask,
  input: ProbeStudybenchPatchRunnerInput,
): Effect.Effect<ProbeStudybenchPatchCandidateInput, ProbePublicProjectionUnsafe> {
  const candidateInput: ProbeStudybenchPatchCandidateInput = {
    schemaRef: PROBE_STUDYBENCH_PATCH_CANDIDATE_INPUT_SCHEMA_REF,
    allowedToolRefs: [...input.allowedToolRefs],
    budgetPolicyRef: input.budgetPolicy.budgetPolicyRef,
    corpusRef: task.corpusRef,
    expectedFiles: [...task.expectedFiles],
    evidenceExcerptsVisible: false,
    goldAnswerVisible: false,
    maxToolCalls: input.budgetPolicy.maxToolCalls,
    pinnedCheckoutRef: input.pinnedCheckoutRef,
    question: task.question,
    rubricVisible: false,
    scorerMaterialWithheld: true,
    taskId: task.id,
    testCommandRefs: [...input.testCommandRefs],
    timeoutMs: input.budgetPolicy.timeoutMs,
    timeoutPolicyRef: input.budgetPolicy.timeoutPolicyRef,
    topic: task.topic,
    visibility: task.visibility,
  };

  return validateProbePublicProjection(candidateInput, "studybenchPatchCandidateInput").pipe(
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

function runStatusFor(
  input: ProbeStudybenchPatchRunnerInput,
  rubricScore: ProbeStudybenchRubricScore,
): ProbeBenchmarkTerminalRunStatus {
  if (input.runnerTranscript.observedDurationMs > input.budgetPolicy.timeoutMs) {
    return "timed_out";
  }

  return rubricScore.finalScoreBps === 10_000 ? "succeeded" : "failed";
}

function failureClassificationFor(
  task: OpenAgentsStudybenchTask,
  runStatus: ProbeBenchmarkTerminalRunStatus,
): ProbeBenchmarkFailureClassification | undefined {
  if (runStatus === "succeeded") {
    return undefined;
  }

  const family = runStatus === "timed_out" ? "timeout" : "verifier_failure";

  return {
    classificationRef: `failure_classification.probe.studybench_patch.${task.id}.${family}`,
    family,
    summaryRef: `summary.failure.probe.studybench_patch.${task.id}`,
  };
}

function validateBudget(
  budgetPolicy: ProbeStudybenchPatchBudgetPolicy,
): Effect.Effect<void, ProbeStudybenchPatchRunnerError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(budgetPolicy.budgetPolicyRef, "studybenchPatchRunnerInput.budgetPolicy.budgetPolicyRef");
    yield* requireNonEmpty(budgetPolicy.timeoutPolicyRef, "studybenchPatchRunnerInput.budgetPolicy.timeoutPolicyRef");

    if (!Number.isInteger(budgetPolicy.maxToolCalls) || budgetPolicy.maxToolCalls < 0) {
      return yield* patchRunnerError(
        "studybenchPatchRunnerInput.budgetPolicy.maxToolCalls",
        "must be a non-negative integer",
      );
    }

    if (!Number.isInteger(budgetPolicy.timeoutMs) || budgetPolicy.timeoutMs <= 0) {
      return yield* patchRunnerError(
        "studybenchPatchRunnerInput.budgetPolicy.timeoutMs",
        "must be a positive integer duration",
      );
    }
  });
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

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeStudybenchPatchRunnerError> {
  return value.trim().length === 0
    ? patchRunnerError(path, "must be a non-empty ref")
    : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeStudybenchPatchRunnerError> {
  if (refs.length === 0) {
    return patchRunnerError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1
    ? Effect.void
    : patchRunnerError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function patchRunnerError(path: string, reason: string): Effect.Effect<never, ProbeStudybenchPatchRunnerError> {
  return Effect.fail(new ProbeStudybenchPatchRunnerError({ path, reason }));
}
