import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_SCORER_REFS,
  runProbeStudybenchAnswerCandidate,
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
} from "../src";

const assignment = (split: "retained" | "validation" | "holdout" = "retained") => ({
  schemaRef: PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  assignmentRef: `probe_benchmark_assignment.studybench_answer.openagents_launch_0001.${split}`,
  benchmarkRunRef: `benchmark_run.studybench_answer.openagents_launch_0001.${split}`,
  taskRunRef: `task_run.studybench_answer.openagents_launch_0001.${split}`,
  dataset: {
    slug: "openagents-studybench",
    version: "2026-06-17",
  },
  split: {
    evidenceSplit: split,
    splitRef: `split.openagents_studybench.${split}.v0`,
  },
  task: {
    taskRef: "studybench_task.openagents_public_retained.openagents_launch_0001",
  },
  probeCommit: "9d450f46d",
  runtime: {
    runtimeRef: "runtime.probe.studybench.answer.v0",
    backendProfileRef: "backend_profile.probe.studybench.answer.v0",
  },
  backend: {
    backendRef: "probe.backend.studybench.answer",
    modelBackendRef: "model_backend.probe.studybench.answer",
  },
  selectedBlueprintSignatureRefs: ["program_signature.probe.studybench.answer.v0"],
  toolMenuRef: "tool_menu.probe.studybench.answer_mode.v0",
  candidateHash: "sha256:baseline-studybench-answer",
  candidateRefs: {
    promptCandidateRef: "probe.prompt_candidate.studybench_answer.baseline",
    blueprintCandidateRef: "probe.blueprint_candidate.studybench_answer.baseline",
    toolMenuCandidateRef: "probe.tool_menu_candidate.studybench_answer.baseline",
    loopPolicyCandidateRef: "probe.loop_policy_candidate.studybench_answer.baseline",
  },
  timeoutBudgetPolicy: {
    budgetPolicyRef: "budget_policy.probe.studybench.answer.v0",
    maxToolCalls: 0,
    timeoutPolicyRef: "timeout_policy.probe.studybench.answer.v0",
  },
  requiredArtifacts: {
    artifactRefs: ["artifact_manifest.required.probe.studybench.answer.v0"],
    proofBundleRefs: ["proof_bundle.required.probe.studybench.answer.v0"],
  },
  sinks: {
    callbackRefs: ["callback.openagents.benchmark_cloud.probe.v1"],
    proofSinkRefs: ["proof_sink.openagents.benchmark_cloud.probe.v1"],
  },
});

const task = (overrides: Partial<OpenAgentsStudybenchTask> = {}): OpenAgentsStudybenchTask => ({
  schemaRef: OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  id: "openagents_launch_0001",
  topic: "launch_claims_and_promises",
  question: "Update launch copy without overclaiming repo studying.",
  gold_answer: "Keep repo studying internal, evidence-linked, and product-promise gated.",
  rubric: [
    {
      claim_id: "c1",
      claim_type: "core",
      weight: 60,
      statement: "The answer keeps upstream StudyBench as public calibration.",
      span_ids: ["s1"],
    },
    {
      claim_id: "c2",
      claim_type: "supporting",
      weight: 40,
      statement: "The answer preserves the private holdout boundary.",
      span_ids: ["s2"],
    },
  ],
  evidence: [
    {
      span_id: "s1",
      path: "docs/research/machine-studying/README.md",
      start_line: 38,
      end_line: 40,
      excerpt: "0038: Upstream rows are external public calibration only.",
    },
    {
      span_id: "s2",
      path: "docs/research/machine-studying/README.md",
      start_line: 43,
      end_line: 46,
      excerpt: "0043: Private holdout rows are not committed.",
    },
  ],
  repo: "OpenAgentsInc/openagents",
  commit: "9d450f46d",
  corpusRef: "openagents_repo_corpus_manifest.v0.sha256_abc",
  visibility: "openagents_public_retained",
  authorityRefs: ["authority.openagents.product_promises"],
  testRefs: ["test.probe.studybench.answer_runner"],
  forbiddenClaimRefs: ["blocked_claim.repo_studying_public_product"],
  privateMaterialPolicyRefs: ["policy.openagents.no_private_holdout_leakage"],
  expectedFiles: ["docs/research/machine-studying/README.md"],
  budgetClass: "small",
  ...overrides,
});

const claimScore = (overrides: Partial<ProbeStudybenchClaimScore> = {}): ProbeStudybenchClaimScore => ({
  schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  claimId: "c1",
  claimType: "core",
  evidenceSpanIds: ["s1"],
  rationaleRef: "rationale.probe.studybench.openagents_launch_0001.c1",
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
    rationaleRef: "rationale.probe.studybench.openagents_launch_0001.c2",
    weight: 40,
    ...overrides,
  });

const unsafeCandidateManifest = () => ({
  schema_version: "psionic.probe_gepa_candidate_manifest.v1",
  candidate_id: "probe_gepa_candidate.aaaaaaaaaaaaaaaa",
  parent_candidate_id: null,
  campaign_id: "probe_gepa.studybench.answer",
  candidate_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  manifest_hash: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  component_hashes: {
    closeout_policy: "sha256:1",
    failure_family_playbooks: {
      verifier_failure: "sha256:2",
    },
    patch_and_test_policy: "sha256:3",
    probe_system_prompt: "sha256:4",
    signature_selection_policy: "sha256:5",
    terminal_bench_global_playbook: "sha256:6",
    tool_menu_policy: "sha256:7",
  },
  components: {
    closeout_policy: "Emit closeouts.",
    failure_family_playbooks: {
      verifier_failure: "Retain failed StudyBench feedback.",
    },
    patch_and_test_policy: "Do not patch in answer mode.",
    probe_system_prompt: "Answer under Probe benchmark authority.",
    signature_selection_policy: "Use assignment signatures.",
    terminal_bench_global_playbook: "Not used for answer mode.",
    tool_menu_policy: "Use no tools in answer mode.",
  },
  target_suites: ["openagents_studybench"],
  target_failure_families: ["verifier_failure"],
  split_refs: ["split.openagents_studybench.retained.v0"],
  optimizer_run_id: "psionic_gepa_optimizer.studybench_answer.seed",
  training_trace_digests: ["sha256:trace"],
  evaluation_trace_digests: ["sha256:evaluation"],
  policy_gate_state: "pending",
  optimizer_acceptance_state: "draft",
  runtime_promotion_state: "not_promoted",
  promotion_state: "draft",
  probe_import: {
    schema_version: "probe.prompt_candidate_import.v1",
    prompt_candidate_ref: "probe.prompt_candidate.studybench.answer",
    blueprint_candidate_ref: "probe.blueprint_candidate.studybench.answer",
    tool_menu_candidate_ref: "probe.tool_menu_candidate.studybench.answer",
    loop_policy_candidate_ref: "probe.loop_policy_candidate.studybench.answer",
  },
  benchmark_cloud_import: {
    schema_version: "benchmark_cloud.probe_candidate_import.v1",
    split_refs: ["split.openagents_studybench.retained.v0"],
    benchmark_run_manifest_refs: ["benchmark_run_manifest.openagents_studybench.answer.v0"],
    artifact_contract_refs: ["probe.benchmark_closeout.v1"],
  },
  safety_boundary: {
    no_new_runtime_authority: true,
    inherited_runtime_authority_refs: ["runtime_authority.inherited_from_probe_assignment_refs"],
    release_gate_ref: "release_gate.probe.studybench.answer.v0",
    public_claim_upgrade_authority: true,
  },
});

describe("StudyBench answer-mode runner", () => {
  test("scores a public-retained row from a supplied candidate answer ref", async () => {
    const result = await Effect.runPromise(
      runProbeStudybenchAnswerCandidate({
        assignment: assignment(),
        candidateAnswerRef: "candidate_answer.probe.studybench.openagents_launch_0001.baseline",
        claimScores: [claimScore(), supportingClaimScore()],
        resourceUsageRef: "resource_usage.probe.studybench_answer.openagents_launch_0001",
        task: task(),
      }),
    );

    const rubricScore = result.bundle.files["rubric-score.json"] as {
      readonly rubricScore?: { readonly finalScoreBps?: number };
      readonly rubricScoreRef?: string;
    };

    expect(result.runStatus).toBe("succeeded");
    expect(result.rubricScore.finalScoreBps).toBe(10_000);
    expect(rubricScore.rubricScore?.finalScoreBps).toBe(10_000);
    expect(rubricScore.rubricScoreRef).toContain("rubric_score.probe.studybench_answer.openagents_launch_0001");
  });

  test("failed answers produce claim feedback and retained failure refs", async () => {
    const result = await Effect.runPromise(
      runProbeStudybenchAnswerCandidate({
        assignment: assignment(),
        candidateAnswerRef: "candidate_answer.probe.studybench.openagents_launch_0001.failed",
        claimScores: [
          claimScore({
            satisfied: false,
            scoreBps: 0,
          }),
          supportingClaimScore({ scoreBps: 5_000 }),
        ],
        resourceUsageRef: "resource_usage.probe.studybench_answer.openagents_launch_0001.failed",
        task: task(),
      }),
    );
    const closeout = result.bundle.files["probe-closeout.json"] as { readonly [key: string]: unknown };
    const rubricScore = result.bundle.files["rubric-score.json"] as {
      readonly rubricScore?: { readonly claimScores?: ReadonlyArray<{ readonly satisfied?: boolean }> };
    };

    expect(result.runStatus).toBe("failed");
    expect((closeout.retainedFailureRefs as string[])[0]).toContain("verifier_failure");
    expect(rubricScore.rubricScore?.claimScores?.[0]?.satisfied).toBe(false);
  });

  test("private validation candidate input withholds gold, rubric, and evidence material", async () => {
    const privateTask = task({
      gold_answer: "Private scorer-only answer must not reach candidate input.",
      rubric: [
        {
          claim_id: "c1",
          claim_type: "core",
          weight: 100,
          statement: "Private scorer-only rubric claim.",
          span_ids: ["s1"],
        },
      ],
      visibility: "openagents_private_validation",
    });
    const privateAssignment = {
      ...assignment("validation"),
      split: {
        evidenceSplit: "validation",
        splitRef: "split.openagents_studybench.private_validation.v0",
      },
      task: {
        taskRef: "studybench_task.openagents_private_validation.openagents_launch_0001",
      },
    };
    const result = await Effect.runPromise(
      runProbeStudybenchAnswerCandidate({
        assignment: privateAssignment,
        candidateAnswerRef: "candidate_answer.probe.studybench.openagents_launch_0001.private_validation",
        claimScores: [
          claimScore({
            weight: 100,
          }),
        ],
        resourceUsageRef: "resource_usage.probe.studybench_answer.openagents_launch_0001.private_validation",
        task: privateTask,
      }),
    );
    const candidateInput = result.candidateInput as unknown as Record<string, unknown>;
    const candidateInputJson = JSON.stringify(result.candidateInput);

    expect(candidateInput.gold_answer).toBeUndefined();
    expect(candidateInput.rubric).toBeUndefined();
    expect(candidateInput.evidence).toBeUndefined();
    expect(candidateInputJson).not.toContain("Private scorer-only answer");
    expect(candidateInputJson).not.toContain("Private scorer-only rubric claim");
    expect(result.candidateInput.goldAnswerVisible).toBe(false);
    expect(result.candidateInput.rubricVisible).toBe(false);
  });

  test("keeps GEPA candidate manifest safety validation in force", async () => {
    await expect(
      Effect.runPromise(
        runProbeStudybenchAnswerCandidate({
          assignment: assignment(),
          candidateAnswerRef: "candidate_answer.probe.studybench.openagents_launch_0001.unsafe_candidate",
          candidateManifest: unsafeCandidateManifest(),
          claimScores: [claimScore(), supportingClaimScore()],
          resourceUsageRef: "resource_usage.probe.studybench_answer.openagents_launch_0001.unsafe_candidate",
          task: task(),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkCandidateExecutionError",
      path: "gepaCandidateManifest.safety_boundary.public_claim_upgrade_authority",
    });
  });
});
