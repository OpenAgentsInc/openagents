import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_SCORER_REFS,
  runProbeStudybenchPatchCandidate,
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
} from "../src";

const assignment = (
  taskId = "openagents_launch_0001",
  split: "retained" | "validation" | "holdout" = "retained",
) => ({
  schemaRef: PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  assignmentRef: `probe_benchmark_assignment.studybench_patch.${taskId}.${split}`,
  benchmarkRunRef: `benchmark_run.studybench_patch.${taskId}.${split}`,
  taskRunRef: `task_run.studybench_patch.${taskId}.${split}`,
  dataset: {
    slug: "openagents-studybench",
    version: "2026-06-17",
  },
  split: {
    evidenceSplit: split,
    splitRef: `split.openagents_studybench.${split}.v0`,
  },
  task: {
    taskRef: `studybench_task.openagents_public_retained.${taskId}`,
  },
  probeCommit: "87d1a9464",
  runtime: {
    runtimeRef: "runtime.probe.studybench.patch.v0",
    backendProfileRef: "backend_profile.probe.studybench.patch.v0",
  },
  backend: {
    backendRef: "probe.backend.studybench.patch",
    modelBackendRef: "model_backend.probe.studybench.patch",
  },
  selectedBlueprintSignatureRefs: ["program_signature.probe.studybench.patch.v0"],
  toolMenuRef: "tool_menu.probe.studybench.patch_mode.v0",
  candidateHash: "sha256:baseline-studybench-patch",
  candidateRefs: {
    promptCandidateRef: "probe.prompt_candidate.studybench_patch.baseline",
    blueprintCandidateRef: "probe.blueprint_candidate.studybench_patch.baseline",
    toolMenuCandidateRef: "probe.tool_menu_candidate.studybench_patch.baseline",
    loopPolicyCandidateRef: "probe.loop_policy_candidate.studybench_patch.baseline",
  },
  timeoutBudgetPolicy: {
    budgetPolicyRef: "budget_policy.probe.studybench.patch.v0",
    maxToolCalls: 4,
    maxDurationMs: 60_000,
    timeoutPolicyRef: "timeout_policy.probe.studybench.patch.v0",
  },
  requiredArtifacts: {
    artifactRefs: ["artifact_manifest.required.probe.studybench.patch.v0"],
    proofBundleRefs: ["proof_bundle.required.probe.studybench.patch.v0"],
  },
  sinks: {
    callbackRefs: ["callback.openagents.benchmark_cloud.probe.v1"],
    proofSinkRefs: ["proof_sink.openagents.benchmark_cloud.probe.v1"],
  },
});

const task = (id = "openagents_launch_0001", overrides: Partial<OpenAgentsStudybenchTask> = {}): OpenAgentsStudybenchTask => ({
  schemaRef: OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  id,
  topic: "studybench_answer_and_patch_modes",
  question: `Patch the repo surface for ${id} without overclaiming repo studying.`,
  gold_answer: "Use patch-mode evidence, run tests, and keep product claims gated.",
  rubric: [
    {
      claim_id: "c1",
      claim_type: "core",
      weight: 60,
      statement: "The patch preserves StudyBench patch-mode evidence boundaries.",
      span_ids: ["s1"],
    },
    {
      claim_id: "c2",
      claim_type: "supporting",
      weight: 40,
      statement: "The patch emits test refs and closeout evidence.",
      span_ids: ["s2"],
    },
  ],
  evidence: [
    {
      span_id: "s1",
      path: "docs/research/machine-studying/README.md",
      start_line: 51,
      end_line: 53,
      excerpt: "0051: Answer-mode and agentic patch-mode evaluation are both required.",
    },
    {
      span_id: "s2",
      path: "docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md",
      start_line: 156,
      end_line: 158,
      excerpt: "0156: a pinned repo checkout, a StudyBench row, an allowed tool menu...",
    },
  ],
  repo: "OpenAgentsInc/openagents",
  commit: "87d1a9464",
  corpusRef: "openagents_repo_corpus_manifest.v0.sha256_abc",
  visibility: "openagents_public_retained",
  authorityRefs: ["authority.openagents.studybench_patch_runner"],
  testRefs: ["test.probe.studybench.patch_runner"],
  forbiddenClaimRefs: ["blocked_claim.private_gold_in_candidate_context"],
  privateMaterialPolicyRefs: ["policy.openagents.no_private_holdout_leakage"],
  expectedFiles: ["docs/research/machine-studying/README.md"],
  budgetClass: "small",
  ...overrides,
});

const budgetPolicy = {
  budgetPolicyRef: "budget_policy.probe.studybench.patch.v0",
  maxToolCalls: 4,
  timeoutMs: 60_000,
  timeoutPolicyRef: "timeout_policy.probe.studybench.patch.v0",
};

const transcript = (overrides: Record<string, unknown> = {}) => ({
  observedDurationMs: 20_000,
  toolCallRefs: ["tool_call.read.1", "tool_call.edit.1", "tool_call.test.1"],
  transcriptSummaryRef: "transcript_summary.probe.studybench_patch.openagents_launch_0001",
  usedToolRefs: ["tool.probe.read_file", "tool.probe.edit_file", "tool.probe.run_tests"],
  ...overrides,
});

const claimScore = (overrides: Partial<ProbeStudybenchClaimScore> = {}): ProbeStudybenchClaimScore => ({
  schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  claimId: "c1",
  claimType: "core",
  evidenceSpanIds: ["s1"],
  rationaleRef: "rationale.probe.studybench_patch.c1",
  satisfied: true,
  scoreBps: 10_000,
  scorerRef: PROBE_STUDYBENCH_SCORER_REFS.manual_or_judge_supplied,
  weight: 60,
  ...overrides,
});

const supportingClaimScore = (overrides: Partial<ProbeStudybenchClaimScore> = {}): ProbeStudybenchClaimScore =>
  claimScore({
    claimId: "c2",
    claimType: "supporting",
    evidenceSpanIds: ["s2"],
    rationaleRef: "rationale.probe.studybench_patch.c2",
    weight: 40,
    ...overrides,
  });

const baseRunInput = (taskId = "openagents_launch_0001") => ({
  allowedToolRefs: ["tool.probe.read_file", "tool.probe.edit_file", "tool.probe.run_tests"],
  assignment: assignment(taskId),
  budgetPolicy,
  claimScores: [claimScore(), supportingClaimScore()],
  patchArtifactRefs: [`patch_artifact.probe.studybench_patch.${taskId}.diff`],
  pinnedCheckoutRef: "checkout.openagents.87d1a9464.fixture",
  resourceUsageRef: `resource_usage.probe.studybench_patch.${taskId}`,
  runnerIdentityRef: "runner.probe.studybench.patch.pre_recorded.v0",
  runnerTranscript: transcript({
    transcriptSummaryRef: `transcript_summary.probe.studybench_patch.${taskId}`,
  }),
  task: task(taskId),
  testCommandRefs: [`test_command.probe.studybench_patch.${taskId}.bun_test`],
});

describe("StudyBench patch-mode runner", () => {
  test("runs two public-retained rows in patch mode against a fixture checkout", async () => {
    const first = await Effect.runPromise(runProbeStudybenchPatchCandidate(baseRunInput("openagents_launch_0001")));
    const second = await Effect.runPromise(runProbeStudybenchPatchCandidate(baseRunInput("openagents_launch_0002")));
    const firstArtifacts = first.bundle.files["artifact-refs.json"] as { readonly [key: string]: unknown };
    const secondArtifacts = second.bundle.files["artifact-refs.json"] as { readonly [key: string]: unknown };

    expect(first.runStatus).toBe("succeeded");
    expect(second.runStatus).toBe("succeeded");
    expect(first.patchArtifactRefs[0]).toContain("openagents_launch_0001");
    expect(second.patchArtifactRefs[0]).toContain("openagents_launch_0002");
    expect(firstArtifacts.artifactManifestRefs).toEqual(["patch_artifact.probe.studybench_patch.openagents_launch_0001.diff"]);
    expect(secondArtifacts.verifierResultRefs).toEqual(["test_command.probe.studybench_patch.openagents_launch_0002.bun_test"]);
  });

  test("enforces max tool calls and timeout budget", async () => {
    await expect(
      Effect.runPromise(
        runProbeStudybenchPatchCandidate({
          ...baseRunInput(),
          runnerTranscript: transcript({
            toolCallRefs: ["tool_call.1", "tool_call.2", "tool_call.3", "tool_call.4", "tool_call.5"],
          }),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeStudybenchPatchRunnerError",
      path: "studybenchPatchRunnerInput.runnerTranscript.toolCallRefs",
    });

    const timedOut = await Effect.runPromise(
      runProbeStudybenchPatchCandidate({
        ...baseRunInput(),
        resourceUsageRef: undefined,
        runnerTranscript: transcript({
          observedDurationMs: 120_000,
        }),
      }),
    );
    const closeout = timedOut.bundle.files["probe-closeout.json"] as { readonly [key: string]: unknown };

    expect(timedOut.runStatus).toBe("timed_out");
    expect((closeout.failureClassification as { readonly family: string }).family).toBe("timeout");
  });

  test("rejects tools outside the assignment tool menu", async () => {
    await expect(
      Effect.runPromise(
        runProbeStudybenchPatchCandidate({
          ...baseRunInput(),
          runnerTranscript: transcript({
            usedToolRefs: ["tool.probe.read_file", "tool.probe.raw_network_exfiltration"],
          }),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeStudybenchPatchRunnerError",
      path: "studybenchPatchRunnerInput.runnerTranscript.usedToolRefs[1]",
    });
  });

  test("failed patch runs emit claim feedback and retained failure refs", async () => {
    const failed = await Effect.runPromise(
      runProbeStudybenchPatchCandidate({
        ...baseRunInput(),
        claimScores: [
          claimScore({
            satisfied: false,
            scoreBps: 0,
          }),
          supportingClaimScore({ scoreBps: 5_000 }),
        ],
      }),
    );
    const closeout = failed.bundle.files["probe-closeout.json"] as { readonly [key: string]: unknown };
    const rubricScore = failed.bundle.files["rubric-score.json"] as {
      readonly rubricScore?: { readonly finalScoreBps?: number };
    };

    expect(failed.runStatus).toBe("failed");
    expect((closeout.retainedFailureRefs as string[])[0]).toContain("verifier_failure");
    expect(rubricScore.rubricScore?.finalScoreBps).toBe(0);
  });

  test("private holdout candidate input withholds gold, rubric, and evidence material", async () => {
    const privateTask = task("openagents_launch_0001", {
      gold_answer: "Private holdout gold answer must not be mounted.",
      rubric: [
        {
          claim_id: "c1",
          claim_type: "core",
          weight: 100,
          statement: "Private holdout rubric claim.",
          span_ids: ["s1"],
        },
      ],
      visibility: "openagents_private_holdout",
    });
    const holdoutAssignment = {
      ...assignment("openagents_launch_0001", "holdout"),
      task: {
        taskRef: "studybench_task.openagents_private_holdout.openagents_launch_0001",
      },
    };
    const result = await Effect.runPromise(
      runProbeStudybenchPatchCandidate({
        ...baseRunInput(),
        assignment: holdoutAssignment,
        claimScores: [
          claimScore({
            weight: 100,
          }),
        ],
        task: privateTask,
      }),
    );
    const candidateInput = result.candidateInput as unknown as Record<string, unknown>;
    const candidateInputJson = JSON.stringify(result.candidateInput);

    expect(candidateInput.gold_answer).toBeUndefined();
    expect(candidateInput.rubric).toBeUndefined();
    expect(candidateInput.evidence).toBeUndefined();
    expect(candidateInputJson).not.toContain("Private holdout gold answer");
    expect(candidateInputJson).not.toContain("Private holdout rubric claim");
    expect(result.candidateInput.goldAnswerVisible).toBe(false);
    expect(result.candidateInput.rubricVisible).toBe(false);
    expect(result.candidateInput.evidenceExcerptsVisible).toBe(false);
  });
});
