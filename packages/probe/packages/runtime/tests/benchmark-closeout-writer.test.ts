import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
  decodeProbeBenchmarkAssignment,
  makeProbeBenchmarkCloseoutBundle,
  projectProbeGepaLiveRunnerGate,
  writeProbeBenchmarkCloseoutBundle,
} from "../src";

const fakeAssignment = async () =>
  Effect.runPromise(
    decodeProbeBenchmarkAssignment({
      schemaRef: PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
      assignmentRef: "probe_benchmark_assignment.configure_git_webserver.1",
      benchmarkRunRef: "benchmark_run.terminal_bench_2.gepa_stage_0.1",
      taskRunRef: "task_run.configure_git_webserver.1",
      dataset: {
        slug: "terminal-bench-2-harbor",
        version: "2026-06-08",
      },
      split: {
        evidenceSplit: "retained",
        splitRef: "split.terminal_bench_2.retained.v1",
      },
      task: {
        taskChecksum: "sha256:9d7a6f8f1b7d0f5e0f0d4c8e2a4f7b3e",
      },
      probeCommit: "abc1234",
      runtime: {
        runtimeRef: "runtime.probe.v1",
        backendProfileRef: "backend_profile.apple_fm.local.v1",
      },
      backend: {
        backendRef: "probe.backend.apple_fm_bridge",
        modelBackendRef: "model_backend.apple_fm.local_foundation_model",
      },
      selectedBlueprintSignatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
      toolMenuRef: "tool_menu.probe.terminal_bench.service_readiness.v1",
      candidateHash: "sha256:candidate-1",
      candidateRefs: {
        promptCandidateRef: "candidate.prompt.service_readiness.v1",
        blueprintCandidateRef: "candidate.blueprint.service_readiness.v1",
        toolMenuCandidateRef: "candidate.tool_menu.service_readiness.v1",
        loopPolicyCandidateRef: "candidate.loop_policy.service_readiness.v1",
      },
      timeoutBudgetPolicy: {
        budgetPolicyRef: "budget_policy.probe.retained_smoke.v1",
        timeoutPolicyRef: "timeout_policy.probe.retained_smoke.v1",
      },
      requiredArtifacts: {
        artifactRefs: ["artifact_manifest.required.probe.closeout.v1"],
        proofBundleRefs: ["proof_bundle.required.probe.closeout.v1"],
      },
      sinks: {
        callbackRefs: ["callback.openagents.benchmark_cloud.probe.v1"],
        proofSinkRefs: ["proof_sink.openagents.benchmark_cloud.probe.v1"],
      },
    }),
  );

describe("Probe benchmark closeout writer", () => {
  test("emits and writes a complete successful closeout bundle", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        artifactManifestRefs: ["artifact_manifest.probe.configure_git_webserver.1"],
        decisionStepRefs: ["decision_step.inspect_service_status.1"],
        proofBundleRefs: ["proof_bundle.probe.configure_git_webserver.1"],
        resourceUsageRef: "resource_usage.probe.configure_git_webserver.1",
        runRef: "probe_run.configure_git_webserver.1",
        runStatus: "succeeded",
        scorerRef: "scorer.terminal_bench.binary.v1",
        toolMenuSnapshot: {
          toolRefs: ["tool.probe.read_file", "tool.probe.code_search"],
        },
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );
    const directory = await mkdtemp(join(tmpdir(), "probe-closeout-"));

    try {
      const writeResult = await Effect.runPromise(writeProbeBenchmarkCloseoutBundle(bundle, directory));
      const closeoutFile = JSON.parse(await readFile(join(directory, "probe-closeout.json"), "utf8"));

      expect(Object.keys(bundle.files).sort()).toEqual([...PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES].sort());
      expect(writeResult.files.map((file) => file.fileName).sort()).toEqual(
        [...PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES].sort(),
      );
      expect(closeoutFile.runStatus).toBe("succeeded");
      expect(closeoutFile.artifactManifestRefs).toEqual(["artifact_manifest.probe.configure_git_webserver.1"]);
      expect(closeoutFile.routeScorecardRef).toBe("route_scorecard.probe.benchmark.probe_run.configure_git_webserver.1");
      expect((bundle.files["route-scorecard.json"] as { readonly selectedRouteKind: string }).selectedRouteKind).toBe(
        "apple_fm",
      );
      expect(JSON.stringify(bundle.files)).not.toContain("raw");
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  });

  test("failed retained runs emit retained-failure refs and failure classification", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        failureClassification: {
          classificationRef: "failure_classification.configure_git_webserver.service_readiness",
          family: "service_readiness",
          summaryRef: "summary.failure.configure_git_webserver.1",
        },
        resourceUsageRef: "resource_usage.probe.configure_git_webserver.failed.1",
        runRef: "probe_run.configure_git_webserver.failed.1",
        runStatus: "failed",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );
    const closeout = bundle.files["probe-closeout.json"] as { readonly [key: string]: unknown };
    const failure = bundle.files["failure-classification.json"] as { readonly [key: string]: unknown };

    expect(closeout.runStatus).toBe("failed");
    expect((closeout.retainedFailureRefs as string[])[0]).toContain("service_readiness");
    expect(JSON.stringify(failure)).toContain("service_readiness");
  });

  test("failed retained StudyBench patch runs carry task and rubric score evidence only", async () => {
    const assignment = await fakeAssignment();
    const studybenchTaskRef = "studybench_task.openagents.public_retained.openagents_launch_0009";
    const studybenchScoreRef = "rubric_score.probe.studybench.openagents_launch_0009.failed.1";
    const evidenceUseRef = "evidence_use.probe.studybench.openagents_launch_0009.failed.1";
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        artifactManifestRefs: ["artifact_manifest.probe.studybench_patch.failed.1"],
        failureClassification: {
          classificationRef: "failure_classification.studybench_patch.verifier_failure",
          family: "verifier_failure",
          summaryRef: "summary.failure.studybench_patch.openagents_launch_0009",
        },
        proofBundleRefs: ["proof_bundle.probe.studybench_patch.failed.1"],
        resourceUsageRef: "resource_usage.probe.studybench_patch.failed.1",
        runRef: "probe_run.studybench_patch.openagents_launch_0009.failed.1",
        runStatus: "failed",
        scorerRef: "scorer.probe.studybench.manual_or_judge_supplied.v0",
        studybenchEvidenceUseRefs: [evidenceUseRef],
        studybenchRubricScore: {
          schemaRef: PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
          candidateHash: assignment.candidateHash,
          claimScores: [
            {
              schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
              claimId: "c1",
              claimType: "core",
              evidenceSpanIds: ["s1"],
              rationaleRef: "rationale.probe.studybench.openagents_launch_0009.c1",
              satisfied: false,
              scoreBps: 0,
              scorerRef: "scorer.probe.studybench.manual_or_judge_supplied.v0",
              weight: 100,
            },
          ],
          coreGatePassed: false,
          evidenceUseRefs: [evidenceUseRef],
          finalScoreBps: 0,
          goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0009",
          redactionState: "public_safe",
          taskId: "openagents_launch_0009",
          weightedScoreBps: 0,
        },
        studybenchScoreRef,
        studybenchTaskRef,
        verifierRef: "verifier.probe.studybench.patch_mode.v0",
      }),
    );
    const closeout = bundle.files["probe-closeout.json"] as { readonly [key: string]: unknown };
    const taskRefFile = bundle.files["studybench-task-ref.json"] as { readonly [key: string]: unknown };
    const rubricScoreFile = bundle.files["rubric-score.json"] as {
      readonly [key: string]: unknown;
      readonly rubricScore?: { readonly finalScoreBps?: number };
    };

    expect((closeout.retainedFailureRefs as string[])[0]).toContain("verifier_failure");
    expect(taskRefFile.taskRef).toBe(studybenchTaskRef);
    expect(rubricScoreFile.rubricScoreRef).toBe(studybenchScoreRef);
    expect(rubricScoreFile.evidenceUseRefs).toEqual([evidenceUseRef]);
    expect(rubricScoreFile.rubricScore?.finalScoreBps).toBe(0);

    const gate = await Effect.runPromise(
      projectProbeGepaLiveRunnerGate({
        bundle,
        candidateManifestAuthorityRefs: ["candidate_manifest_authority.psionic.gepa.stage_0.v1"],
        mode: "sandbox",
        runnerExecutionRefs: ["runner_execution.probe.studybench_patch.failed.1"],
      }),
    );

    expect(gate.runnerGateReady).toBe(true);
    expect(gate.evidenceRefs.studybenchRefs).toContain(studybenchTaskRef);
    expect(gate.evidenceRefs.studybenchRefs).toContain(studybenchScoreRef);
    expect(gate.evidenceRefs.studybenchRefs).toContain(evidenceUseRef);
    expect(gate.publicScoreClaimAllowed).toBe(false);
    expect(gate.productPromotionAllowed).toBe(false);
    expect(gate.payoutClaimAllowed).toBe(false);
  });

  test("timed-out runs emit timeout state, partial artifact refs, and resource unavailable reason", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        artifactManifestRefs: ["artifact_manifest.partial.configure_git_webserver.timeout.1"],
        partialArtifactRefs: ["artifact.partial.stdout_summary.configure_git_webserver.timeout.1"],
        resourceUnavailableReason: "timeout_before_resource_meter_flush",
        runRef: "probe_run.configure_git_webserver.timeout.1",
        runStatus: "timed_out",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );
    const closeout = bundle.files["probe-closeout.json"] as { readonly [key: string]: unknown };
    const artifacts = bundle.files["artifact-refs.json"] as { readonly [key: string]: unknown };
    const resource = bundle.files["resource-usage-ref.json"] as { readonly [key: string]: unknown };

    expect(closeout.runStatus).toBe("timed_out");
    expect((closeout.failureClassification as { readonly family: string }).family).toBe("timeout");
    expect(artifacts.partialArtifactRefs).toEqual(["artifact.partial.stdout_summary.configure_git_webserver.timeout.1"]);
    expect(resource.unavailableReason).toBe("timeout_before_resource_meter_flush");
  });

  test("policy-blocked runs emit blocked policy findings", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        resourceUnavailableReason: "policy_blocked_before_resource_meter",
        runRef: "probe_run.configure_git_webserver.policy_blocked.1",
        runStatus: "policy_blocked",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );
    const policy = bundle.files["policy-findings.json"] as { readonly [key: string]: unknown };

    expect(JSON.stringify(policy)).toContain("blocked");
  });

  test("can include explicit route scorecards with rejected route evidence", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        resourceUsageRef: "resource_usage.probe.configure_git_webserver.pylon.1",
        routeScorecard: {
          schemaRef: "probe.benchmark_route_scorecard.v1",
          scorecardRef: "route_scorecard.probe.configure_git_webserver.pylon.1",
          selectedAgentOrModelRef: "model_backend.local_qwen.coder.v1",
          selectedRunnerRef: "runner.probe.pylon_worker.v1",
          selectedProviderRef: "pylon.public.shc_box_1",
          selectedIsolationProfileRef: "isolation.pylon_worker_sandbox",
          selectedVerifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
          selectedRouteKind: "pylon",
          expectedCostRef: "cost.expected.pylon.unpaid_smoke",
          observedCostRef: "cost.observed.pylon.unpaid_smoke",
          expectedLatencyMs: 300000,
          observedLatencyMs: 90000,
          privacyTier: "pylon_worker",
          trustTier: "registered_pylon",
          selectedSignatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
          toolMenuRef: "tool_menu.probe.terminal_bench.service_readiness.v1",
          candidateHash: assignment.candidateHash,
          rejectedRoutes: [
            {
              reasonRef: "route_rejection.codex.remote_api_not_selected",
              routeKind: "codex",
              routeRef: "route.probe.codex",
            },
            {
              reasonRef: "route_rejection.apple_fm.worker_capacity_missing",
              routeKind: "apple_fm",
              routeRef: "route.probe.apple_fm",
            },
          ],
          routeReasonRef: "route_reason.pylon.distributed_metric_call",
          postCloseoutRouteScoreBps: 8500,
        },
        runRef: "probe_run.configure_git_webserver.pylon.1",
        runStatus: "succeeded",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );
    const scorecard = bundle.files["route-scorecard.json"] as {
      readonly rejectedRoutes: ReadonlyArray<{ readonly routeKind: string }>;
      readonly selectedRouteKind: string;
    };

    expect(scorecard.selectedRouteKind).toBe("pylon");
    expect(scorecard.rejectedRoutes.map((route) => route.routeKind)).toEqual(["codex", "apple_fm"]);
  });

  test("projects live GEPA runner evidence for Omega without score, promotion, or payout overclaim", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        artifactManifestRefs: ["artifact_manifest.probe.live.configure_git_webserver.1"],
        candidateComponentRefs: ["candidate_component.probe.closeout_policy.sha256_1"],
        proofBundleRefs: ["proof_bundle.probe.live.configure_git_webserver.1"],
        resourceUsageRef: "resource_usage.probe.live.configure_git_webserver.1",
        runRef: "probe_run.live.configure_git_webserver.1",
        runStatus: "succeeded",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
        verifierResultRefs: ["verifier_result.terminal_bench.configure_git_webserver.1"],
      }),
    );
    const gate = await Effect.runPromise(
      projectProbeGepaLiveRunnerGate({
        bundle,
        candidateManifestAuthorityRefs: ["candidate_manifest_authority.psionic.gepa.stage_0.v1"],
        mode: "live",
        runnerExecutionRefs: ["runner_execution.probe.terminal_bench.live.1"],
      }),
    );

    expect(gate.schemaRef).toBe("probe.gepa_live_runner_gate.v1");
    expect(gate.runnerGateReady).toBe(true);
    expect(gate.omegaImportAllowed).toBe(true);
    expect(gate.evidenceRefs.artifactRefs).toEqual(["artifact_manifest.probe.live.configure_git_webserver.1"]);
    expect(gate.evidenceRefs.proofRefs).toEqual(["proof_bundle.probe.live.configure_git_webserver.1"]);
    expect(gate.evidenceRefs.resourceRefs).toEqual(["resource_usage.probe.live.configure_git_webserver.1"]);
    expect(gate.evidenceRefs.verifierRefs).toEqual([
      "verifier.terminal_bench.configure_git_webserver.v1",
      "verifier_result.terminal_bench.configure_git_webserver.1",
    ]);
    expect(gate.evidenceRefs.routeScorecardRefs).toContain(
      "route_scorecard.probe.benchmark.probe_run.live.configure_git_webserver.1",
    );
    expect(gate.evidenceRefs.selectedSignatureRefs).toEqual([
      "program_signature.probe.benchmark.service_readiness.v1",
    ]);
    expect(gate.evidenceRefs.toolMenuRefs).toEqual(["tool_menu.probe.terminal_bench.service_readiness.v1"]);
    expect(gate.publicScoreClaimAllowed).toBe(false);
    expect(gate.productPromotionAllowed).toBe(false);
    expect(gate.payoutClaimAllowed).toBe(false);
  });

  test("keeps timeout and policy-blocked closeouts importable only as failure evidence", async () => {
    const assignment = await fakeAssignment();

    for (const runStatus of ["timed_out", "policy_blocked"] as const) {
      const bundle = await Effect.runPromise(
        makeProbeBenchmarkCloseoutBundle({
          assignment,
          artifactManifestRefs: [`artifact_manifest.probe.${runStatus}.configure_git_webserver.1`],
          proofBundleRefs: [`proof_bundle.probe.${runStatus}.configure_git_webserver.1`],
          resourceUnavailableReason: `${runStatus}_before_resource_meter_flush`,
          runRef: `probe_run.configure_git_webserver.${runStatus}.gate.1`,
          runStatus,
          scorerRef: "scorer.terminal_bench.binary.v1",
          verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
        }),
      );
      const gate = await Effect.runPromise(
        projectProbeGepaLiveRunnerGate({
          bundle,
          candidateManifestAuthorityRefs: ["candidate_manifest_authority.psionic.gepa.stage_0.v1"],
          mode: "sandbox",
          runnerExecutionRefs: [`runner_execution.probe.terminal_bench.${runStatus}.1`],
        }),
      );

      expect(gate.runnerGateReady).toBe(true);
      expect(gate.omegaImportAllowed).toBe(true);
      expect(gate.evidenceRefs.failureRefs[0]).toContain("failure_classification");
      expect(gate.evidenceRefs.failureRefs[1]).toContain(runStatus === "timed_out" ? "timeout" : "policy_blocked");
      expect(gate.publicScoreClaimAllowed).toBe(false);
      expect(gate.productPromotionAllowed).toBe(false);
      expect(gate.payoutClaimAllowed).toBe(false);
    }
  });

  test("blocks Omega import when candidate-manifest authority or runner execution evidence is missing", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        resourceUsageRef: "resource_usage.probe.configure_git_webserver.1",
        runRef: "probe_run.configure_git_webserver.missing_authority.1",
        runStatus: "succeeded",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );
    const gate = await Effect.runPromise(
      projectProbeGepaLiveRunnerGate({
        bundle,
        candidateManifestAuthorityRefs: [],
        mode: "sandbox",
        runnerExecutionRefs: [],
      }),
    );

    expect(gate.runnerGateReady).toBe(false);
    expect(gate.omegaImportAllowed).toBe(false);
    expect(gate.blockerRefs).toContain("blocker.probe.gepa_live_runner_gate.missing_candidate_manifest_authority_ref");
    expect(gate.blockerRefs).toContain("blocker.probe.gepa_live_runner_gate.missing_runner_execution_ref");
  });

  test("rejects unsafe writer input before public-safe artifacts are emitted", async () => {
    const assignment = await fakeAssignment();

    await expect(
      Effect.runPromise(
        makeProbeBenchmarkCloseoutBundle({
          assignment,
          resourceUsageRef: "resource_usage.probe.configure_git_webserver.1",
          runRef: "probe_run.configure_git_webserver.unsafe.1",
          runStatus: "succeeded",
          scorerRef: "scorer.terminal_bench.binary.v1",
          toolMenuSnapshot: {
            rawLogs: "captured terminal transcript",
          },
          verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
    });
  });

  test("rejects unsafe live runner gate refs before Omega import", async () => {
    const assignment = await fakeAssignment();
    const bundle = await Effect.runPromise(
      makeProbeBenchmarkCloseoutBundle({
        assignment,
        resourceUsageRef: "resource_usage.probe.configure_git_webserver.1",
        runRef: "probe_run.configure_git_webserver.unsafe_gate.1",
        runStatus: "succeeded",
        scorerRef: "scorer.terminal_bench.binary.v1",
        verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
      }),
    );

    await expect(
      Effect.runPromise(
        projectProbeGepaLiveRunnerGate({
          bundle,
          candidateManifestAuthorityRefs: ["candidate_manifest_authority.psionic.gepa.stage_0.v1"],
          mode: "live",
          runnerExecutionRefs: ["runner_execution.private_repo.git@github.com:OpenAgentsInc/hidden.git"],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
    });
  });
});
