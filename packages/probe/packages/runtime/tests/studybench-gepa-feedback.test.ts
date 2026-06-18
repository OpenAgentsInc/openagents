import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_SCORER_REFS,
  buildProbeStudybenchGepaFeedback,
  decodeProbeGepaCandidateManifest,
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
} from "../src";

const task = (overrides: Partial<OpenAgentsStudybenchTask> = {}): OpenAgentsStudybenchTask => ({
  schemaRef: OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  id: "openagents_launch_0001",
  topic: "blueprint_probe_gepa_contracts",
  question: "Reject a GEPA candidate that tries to widen Probe authority.",
  gold_answer: "Candidate text cannot grant runtime promotion or public-claim authority.",
  rubric: [
    {
      claim_id: "c1",
      claim_type: "core",
      weight: 60,
      statement: "Candidate text remains subordinate to Probe assignment authority.",
      span_ids: ["s1"],
    },
    {
      claim_id: "c2",
      claim_type: "supporting",
      weight: 40,
      statement: "Blueprint contribution drafts remain evidence-only.",
      span_ids: ["s2"],
    },
  ],
  evidence: [
    {
      span_id: "s1",
      path: "docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md",
      start_line: 118,
      end_line: 124,
      excerpt: "0118: Candidate component hashes and candidate hash validation.",
    },
    {
      span_id: "s2",
      path: "docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md",
      start_line: 171,
      end_line: 174,
      excerpt: "0171: Probe Blueprint contribution drafts are content-redacted.",
    },
  ],
  repo: "OpenAgentsInc/openagents",
  commit: "c9c54b40c",
  corpusRef: "openagents_repo_corpus_manifest.v0.sha256_abc",
  visibility: "openagents_public_retained",
  authorityRefs: ["authority.probe.benchmark_candidate_execution"],
  testRefs: ["test.probe.studybench.gepa_feedback"],
  forbiddenClaimRefs: ["blocked_claim.runtime_promotion_from_gepa"],
  privateMaterialPolicyRefs: ["policy.openagents.no_private_holdout_leakage"],
  expectedFiles: ["docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md"],
  budgetClass: "small",
  ...overrides,
});

const claimScore = (overrides: Partial<ProbeStudybenchClaimScore> = {}): ProbeStudybenchClaimScore => ({
  schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  claimId: "c1",
  claimType: "core",
  evidenceSpanIds: ["s1"],
  rationaleRef: "rationale.probe.studybench.openagents_launch_0001.c1",
  satisfied: false,
  scoreBps: 0,
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
    satisfied: true,
    scoreBps: 8_000,
    weight: 40,
    ...overrides,
  });

const rubricScore = (claimScores = [claimScore(), supportingClaimScore()]) => ({
  schemaRef: PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
  candidateHash: "sha256:candidate-studybench-gepa",
  claimScores,
  coreGatePassed: false,
  evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
  finalScoreBps: 0,
  goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
  redactionState: "public_safe",
  taskId: "openagents_launch_0001",
  weightedScoreBps: 3_200,
});

const unsafeCandidateManifest = () => ({
  schema_version: "psionic.probe_gepa_candidate_manifest.v1",
  candidate_id: "probe_gepa_candidate.aaaaaaaaaaaaaaaa",
  parent_candidate_id: null,
  campaign_id: "probe_gepa.studybench.feedback",
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
    patch_and_test_policy: "Do not patch outside the assignment.",
    probe_system_prompt: "Answer under Probe benchmark authority.",
    signature_selection_policy: "Use assignment signatures.",
    terminal_bench_global_playbook: "Use retained fixtures.",
    tool_menu_policy: "Use only assignment tools.",
  },
  target_suites: ["openagents_studybench"],
  target_failure_families: ["verifier_failure"],
  split_refs: ["split.openagents_studybench.retained.v0"],
  optimizer_run_id: "psionic_gepa_optimizer.studybench_feedback.seed",
  training_trace_digests: ["sha256:trace"],
  evaluation_trace_digests: ["sha256:evaluation"],
  policy_gate_state: "pending",
  optimizer_acceptance_state: "draft",
  runtime_promotion_state: "not_promoted",
  promotion_state: "draft",
  probe_import: {
    schema_version: "probe.prompt_candidate_import.v1",
    prompt_candidate_ref: "probe.prompt_candidate.studybench.feedback",
    blueprint_candidate_ref: "probe.blueprint_candidate.studybench.feedback",
    tool_menu_candidate_ref: "probe.tool_menu_candidate.studybench.feedback",
    loop_policy_candidate_ref: "probe.loop_policy_candidate.studybench.feedback",
  },
  benchmark_cloud_import: {
    schema_version: "benchmark_cloud.probe_candidate_import.v1",
    split_refs: ["split.openagents_studybench.retained.v0"],
    benchmark_run_manifest_refs: ["benchmark_run_manifest.openagents_studybench.feedback.v0"],
    artifact_contract_refs: ["probe.benchmark_closeout.v1"],
  },
  safety_boundary: {
    no_new_runtime_authority: false,
    inherited_runtime_authority_refs: [],
    release_gate_ref: "release_gate.probe.studybench.feedback.v0",
    public_claim_upgrade_authority: true,
  },
});

describe("StudyBench GEPA feedback bridge", () => {
  test("turns failed claims into deterministic GEPA feedback refs", async () => {
    const feedback = await Effect.runPromise(
      buildProbeStudybenchGepaFeedback({
        budgetFailureRefs: ["budget_failure.probe.studybench.openagents_launch_0001.tool_calls"],
        rubricScore: rubricScore(),
        skippedTestRefs: ["test_command.probe.studybench.openagents_launch_0001.skipped"],
        task: task(),
        wrongFileRefs: ["wrong_file.probe.studybench.openagents_launch_0001.apps_web"],
      }),
    );

    expect(feedback.failedCoreClaimRefs).toEqual([
      "gepa_feedback.openagents_studybench.openagents_launch_0001.claim.c1.core_failed",
    ]);
    expect(feedback.failedSupportingClaimRefs).toEqual([
      "gepa_feedback.openagents_studybench.openagents_launch_0001.claim.c2.supporting_failed",
    ]);
    expect(feedback.missedEvidenceSpanRefs).toEqual([
      "gepa_feedback.openagents_studybench.openagents_launch_0001.span.s1.missed",
      "gepa_feedback.openagents_studybench.openagents_launch_0001.span.s2.missed",
    ]);
    expect(feedback.targetSuiteRefs).toEqual([
      "target_suite.openagents_studybench.public_retained.v0",
      "target_suite.openagents_studybench.private_validation.v0",
    ]);
    expect(feedback.runtimePromotionAllowed).toBe(false);
    expect(feedback.publicClaimAuthorityAllowed).toBe(false);
    expect(feedback.payoutAuthorityAllowed).toBe(false);
  });

  test("omits raw private holdout answers, rubrics, and judge rationale", async () => {
    const privateTask = task({
      gold_answer: "PRIVATE HOLDOUT GOLD ANSWER",
      rubric: [
        {
          claim_id: "c1",
          claim_type: "core",
          weight: 100,
          statement: "PRIVATE HOLDOUT RUBRIC CLAIM",
          span_ids: ["s1"],
        },
      ],
      visibility: "openagents_private_holdout",
    });
    const feedback = await Effect.runPromise(
      buildProbeStudybenchGepaFeedback({
        rubricScore: rubricScore([
          claimScore({
            weight: 100,
          }),
        ]),
        task: privateTask,
      }),
    );
    const feedbackJson = JSON.stringify(feedback);

    expect(feedback.rawGoldAnswerIncluded).toBe(false);
    expect(feedback.rawJudgeRationaleIncluded).toBe(false);
    expect(feedbackJson).not.toContain("PRIVATE HOLDOUT GOLD ANSWER");
    expect(feedbackJson).not.toContain("PRIVATE HOLDOUT RUBRIC CLAIM");
    expect(feedbackJson).not.toContain("rationale.probe.studybench");
  });

  test("keeps GEPA candidate manifest safety validation in force", async () => {
    await expect(
      Effect.runPromise(decodeProbeGepaCandidateManifest(unsafeCandidateManifest())),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkCandidateExecutionError",
      path: "gepaCandidateManifest.safety_boundary",
    });
  });
});
