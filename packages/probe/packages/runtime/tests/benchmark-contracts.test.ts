import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF,
  PROBE_BENCHMARK_DECISION_TRACE_SCHEMA_REF,
  PROBE_BENCHMARK_PROMOTION_DECISION_SCHEMA_REF,
  PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF,
  PROBE_BENCHMARK_RUN_SCHEMA_REF,
  PROBE_BLUEPRINT_CANDIDATE_SCHEMA_REF,
  PROBE_LOOP_POLICY_CANDIDATE_SCHEMA_REF,
  PROBE_PROMPT_CANDIDATE_SCHEMA_REF,
  PROBE_TOOL_MENU_CANDIDATE_SCHEMA_REF,
  decodeProbeBlueprintCandidate,
  decodeProbeBenchmarkAssignment,
  decodeProbeBenchmarkCloseout,
  decodeProbeBenchmarkDecisionTrace,
  decodeProbeBenchmarkPromotionDecision,
  decodeProbeBenchmarkRouteScorecard,
  decodeProbeBenchmarkRun,
  decodeProbeLoopPolicyCandidate,
  decodeProbePromptCandidate,
  decodeProbeToolMenuCandidate,
  sanitizeProbeBenchmarkProjection,
} from "../src";

const assignment = (split: "retained" | "validation" | "holdout" | "live" = "retained") => ({
  schemaRef: PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF,
  assignmentRef: "probe_benchmark_assignment.configure_git_webserver.1",
  benchmarkRunRef: "benchmark_run.terminal_bench_2.gepa_stage_0.1",
  taskRunRef: "task_run.configure_git_webserver.1",
  dataset: {
    slug: "terminal-bench-2-harbor",
    version: "2026-06-08",
  },
  split: {
    evidenceSplit: split,
    splitRef: `split.terminal_bench_2.${split}.v1`,
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
  accountGrantRefs: {
    providerAccountRef: "provider_account.chatgpt_codex.primary",
    authGrantRef: "provider_grant.omega.probe.1",
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
    maxDurationMs: 300000,
    maxToolCalls: 48,
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
});

const closeout = (runStatus: "succeeded" | "failed" | "timed_out" = "succeeded") => ({
  schemaRef: PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF,
  closeoutRef: "probe_closeout.configure_git_webserver.1",
  assignmentRef: "probe_benchmark_assignment.configure_git_webserver.1",
  runRef: "probe_run.configure_git_webserver.1",
  candidateHash: "sha256:candidate-1",
  evidenceSplit: "retained",
  runStatus,
  selectedSignatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
  toolMenuRef: "tool_menu.probe.terminal_bench.service_readiness.v1",
  backendRoute: {
    backendRef: "probe.backend.apple_fm_bridge",
    backendRouteRef: "backend_route.probe.apple_fm.local.v1",
    modelBackendRef: "model_backend.apple_fm.local_foundation_model",
    runtimeProfileRef: "backend_profile.apple_fm.local.v1",
  },
  verifierScorerRefs: {
    verifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
    scorerRef: "scorer.terminal_bench.binary.v1",
  },
  artifactManifestRefs: ["artifact_manifest.probe.configure_git_webserver.1"],
  proofBundleRefs: ["proof_bundle.probe.configure_git_webserver.1"],
  resourceCostRefs: {
    resourceUsageRef: "resource_usage.probe.configure_git_webserver.1",
    costRef: "cost.probe.configure_git_webserver.1",
  },
  policyFindings: [
    {
      findingRef: "policy_finding.probe.public_safe_projection.1",
      severity: "info",
    },
  ],
  failureClassification:
    runStatus === "succeeded"
      ? {
          family: "none",
          classificationRef: "failure_classification.none",
        }
      : {
          family: runStatus === "timed_out" ? "timeout" : "service_readiness",
          classificationRef: "failure_classification.configure_git_webserver.service_readiness",
          summaryRef: "summary.failure.configure_git_webserver.1",
        },
  retainedFailureRefs:
    runStatus === "succeeded" ? [] : ["retained_failure.terminal_bench.configure_git_webserver.service_readiness"],
  redactionState: "public_safe",
  promotionStatus: runStatus === "succeeded" ? "retained_evidence" : "blocked",
});

describe("Probe benchmark contracts", () => {
  test("decodes benchmark assignment, run, decision trace, candidates, and promotion decision", async () => {
    const parsedAssignment = await Effect.runPromise(decodeProbeBenchmarkAssignment(assignment()));
    const parsedRun = await Effect.runPromise(
      decodeProbeBenchmarkRun({
        schemaRef: PROBE_BENCHMARK_RUN_SCHEMA_REF,
        assignmentRef: parsedAssignment.assignmentRef,
        candidateHash: parsedAssignment.candidateHash,
        completedAt: "2026-06-08T00:00:00.000Z",
        evidenceSplit: "retained",
        resultSummaryRef: "summary.probe.run.configure_git_webserver.1",
        runRef: "probe_run.configure_git_webserver.1",
        startedAt: "2026-06-08T00:00:00.000Z",
        status: "succeeded",
      }),
    );
    const parsedTrace = await Effect.runPromise(
      decodeProbeBenchmarkDecisionTrace({
        schemaRef: PROBE_BENCHMARK_DECISION_TRACE_SCHEMA_REF,
        assignmentRef: parsedAssignment.assignmentRef,
        candidateHash: parsedAssignment.candidateHash,
        decisionStepRefs: ["decision_step.inspect_service_status.1"],
        redactionState: "public_safe",
        runRef: parsedRun.runRef,
        selectedSignatureRefs: parsedAssignment.selectedBlueprintSignatureRefs,
        summaryArtifactRef: "artifact.decision_trace_summary.configure_git_webserver.1",
        toolMenuRef: parsedAssignment.toolMenuRef,
        traceRef: "decision_trace.configure_git_webserver.1",
      }),
    );
    const promptCandidate = await Effect.runPromise(
      decodeProbePromptCandidate({
        schemaRef: PROBE_PROMPT_CANDIDATE_SCHEMA_REF,
        artifactRef: "artifact.prompt_candidate.service_readiness.1",
        candidateHash: parsedAssignment.candidateHash,
        promptRef: "prompt.probe.service_readiness.v1",
        redactionState: "public_safe",
      }),
    );
    const blueprintCandidate = await Effect.runPromise(
      decodeProbeBlueprintCandidate({
        schemaRef: PROBE_BLUEPRINT_CANDIDATE_SCHEMA_REF,
        candidateHash: parsedAssignment.candidateHash,
        moduleVersionRefs: ["module_version.probe.service_readiness.v1"],
        registryVersionRef: "blueprint_registry.probe.benchmark.v1",
        releaseGateRefs: ["release_gate.probe.benchmark.evidence_only.v1"],
        selectedSignatureRefs: parsedAssignment.selectedBlueprintSignatureRefs,
      }),
    );
    const toolMenuCandidate = await Effect.runPromise(
      decodeProbeToolMenuCandidate({
        schemaRef: PROBE_TOOL_MENU_CANDIDATE_SCHEMA_REF,
        candidateHash: parsedAssignment.candidateHash,
        deniedToolRefs: [],
        toolMenuRef: parsedAssignment.toolMenuRef,
        toolRefs: ["tool.probe.read_file", "tool.probe.code_search"],
      }),
    );
    const loopPolicyCandidate = await Effect.runPromise(
      decodeProbeLoopPolicyCandidate({
        schemaRef: PROBE_LOOP_POLICY_CANDIDATE_SCHEMA_REF,
        budgetPolicyRef: "budget_policy.probe.retained_smoke.v1",
        candidateHash: parsedAssignment.candidateHash,
        loopPolicyRef: "loop_policy.probe.service_readiness.v1",
        maxTurns: 12,
        stopConditionRefs: ["stop_condition.tests_passed_or_timeout.v1"],
        timeoutPolicyRef: "timeout_policy.probe.retained_smoke.v1",
      }),
    );
    const promotionDecision = await Effect.runPromise(
      decodeProbeBenchmarkPromotionDecision({
        schemaRef: PROBE_BENCHMARK_PROMOTION_DECISION_SCHEMA_REF,
        authorityBoundary: "evidence_only",
        closeoutRef: "probe_closeout.configure_git_webserver.1",
        decisionRef: "promotion_decision.configure_git_webserver.1",
        evidenceSplit: "retained",
        publicClaimLevel: "retained_summary",
        promotionStatus: "retained_evidence",
        reasonRef: "reason.configure_git_webserver.retained_pass.1",
        requiresExternalGateRefs: ["release_gate.openagents.public_claim_review.v1"],
        runtimePromotionAllowed: false,
      }),
    );
    const routeScorecard = await Effect.runPromise(
      decodeProbeBenchmarkRouteScorecard({
        schemaRef: PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF,
        scorecardRef: "route_scorecard.configure_git_webserver.1",
        selectedAgentOrModelRef: "model_backend.apple_fm.local_foundation_model",
        selectedRunnerRef: "runtime.probe.v1",
        selectedProviderRef: "probe.backend.apple_fm_bridge",
        selectedIsolationProfileRef: "isolation.local_sandbox",
        selectedVerifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
        selectedRouteKind: "apple_fm",
        expectedCostRef: "cost.expected.apple_fm.local",
        observedCostRef: "cost.observed.apple_fm.local",
        expectedLatencyMs: 300000,
        observedLatencyMs: 42000,
        privacyTier: "local_only",
        trustTier: "self_hosted",
        selectedSignatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
        toolMenuRef: "tool_menu.probe.terminal_bench.service_readiness.v1",
        candidateHash: "sha256:candidate-1",
        rejectedRoutes: [
          {
            routeKind: "codex",
            routeRef: "route.probe.codex",
            reasonRef: "route_rejection.codex.remote_api_not_needed",
          },
        ],
        routeReasonRef: "route_reason.apple_fm.local_private",
        postCloseoutRouteScoreBps: 9000,
      }),
    );

    expect(parsedAssignment.schemaRef).toBe(PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF);
    expect(parsedRun.status).toBe("succeeded");
    expect(parsedTrace.traceRef).toBe("decision_trace.configure_git_webserver.1");
    expect(promptCandidate.schemaRef).toBe(PROBE_PROMPT_CANDIDATE_SCHEMA_REF);
    expect(blueprintCandidate.selectedSignatureRefs).toEqual(parsedAssignment.selectedBlueprintSignatureRefs);
    expect(toolMenuCandidate.toolRefs).toContain("tool.probe.read_file");
    expect(loopPolicyCandidate.maxTurns).toBe(12);
    expect(promotionDecision.authorityBoundary).toBe("evidence_only");
    expect(promotionDecision.runtimePromotionAllowed).toBe(false);
    expect(routeScorecard.selectedRouteKind).toBe("apple_fm");
    expect(routeScorecard.rejectedRoutes[0]?.routeKind).toBe("codex");
  });

  test("rejects closeouts missing artifact or proof refs", async () => {
    await expect(
      Effect.runPromise(
        decodeProbeBenchmarkCloseout({
          ...closeout(),
          artifactManifestRefs: [],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "benchmarkCloseout.artifactManifestRefs",
    });

    await expect(
      Effect.runPromise(
        decodeProbeBenchmarkCloseout({
          ...closeout(),
          proofBundleRefs: [],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "benchmarkCloseout.proofBundleRefs",
    });
  });

  test("rejects unsafe public benchmark fields and can scrub them", async () => {
    const unsafe = {
      ...assignment(),
      rawLogs: "cat ~/.auth.json && expose raw shell output",
      nested: {
        hiddenVerifierContent: "hidden verifier answer",
        walletMnemonic: "wallet mnemonic words",
      },
      provider: {
        access_token: "raw-token",
      },
    };

    await expect(Effect.runPromise(decodeProbeBenchmarkAssignment(unsafe))).rejects.toMatchObject({
      _tag: "ProbePublicProjectionUnsafe",
    });

    const scrubbed = sanitizeProbeBenchmarkProjection(unsafe);

    expect("rawLogs" in scrubbed).toBe(false);
    expect(scrubbed.nested).toEqual({});
    expect(scrubbed.provider.access_token).toBe("[redacted]");
  });

  test("failed and timed-out retained runs still emit valid closeouts", async () => {
    const failed = await Effect.runPromise(decodeProbeBenchmarkCloseout(closeout("failed")));
    const timedOut = await Effect.runPromise(decodeProbeBenchmarkCloseout(closeout("timed_out")));

    expect(failed.failureClassification.family).toBe("service_readiness");
    expect(failed.retainedFailureRefs).toContain("retained_failure.terminal_bench.configure_git_webserver.service_readiness");
    expect(timedOut.failureClassification.family).toBe("timeout");
    expect(timedOut.promotionStatus).toBe("blocked");
  });

  test("represents retained, validation, holdout, and live evidence separately", async () => {
    const splits = ["retained", "validation", "holdout", "live"] as const;

    for (const split of splits) {
      const parsedAssignment = await Effect.runPromise(decodeProbeBenchmarkAssignment(assignment(split)));
      const parsedCloseout = await Effect.runPromise(
        decodeProbeBenchmarkCloseout({
          ...closeout(),
          evidenceSplit: split,
          promotionStatus:
            split === "retained"
              ? "retained_evidence"
              : split === "validation"
                ? "validation_candidate"
                : split === "holdout"
                  ? "holdout_candidate"
                  : "live_evidence",
        }),
      );

      expect(parsedAssignment.split.evidenceSplit).toBe(split);
      expect(parsedCloseout.evidenceSplit).toBe(split);
    }
  });

  test("rejects unsafe route scorecards and out-of-range route scores", async () => {
    await expect(
      Effect.runPromise(
        decodeProbeBenchmarkRouteScorecard({
          schemaRef: PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF,
          scorecardRef: "route_scorecard.unsafe",
          selectedAgentOrModelRef: "model_backend.apple_fm.local_foundation_model",
          selectedRunnerRef: "runtime.probe.v1",
          selectedProviderRef: "probe.backend.apple_fm_bridge",
          selectedIsolationProfileRef: "isolation.local_sandbox",
          selectedVerifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
          selectedRouteKind: "apple_fm",
          expectedCostRef: "cost.expected.apple_fm.local",
          observedCostRef: "cost.observed.apple_fm.local",
          expectedLatencyMs: 300000,
          observedLatencyMs: 42000,
          privacyTier: "local_only",
          trustTier: "self_hosted",
          selectedSignatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
          toolMenuRef: "tool_menu.probe.terminal_bench.service_readiness.v1",
          candidateHash: "sha256:candidate-1",
          rejectedRoutes: [],
          routeReasonRef: "route_reason.apple_fm.local_private",
          postCloseoutRouteScoreBps: 10001,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "benchmarkRouteScorecard.postCloseoutRouteScoreBps",
    });

    await expect(
      Effect.runPromise(
        decodeProbeBenchmarkRouteScorecard({
          schemaRef: PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF,
          scorecardRef: "route_scorecard.unsafe",
          selectedAgentOrModelRef: "model_backend.apple_fm.local_foundation_model",
          selectedRunnerRef: "runtime.probe.v1",
          selectedProviderRef: "provider.secret.raw_access_token",
          selectedIsolationProfileRef: "isolation.local_sandbox",
          selectedVerifierRef: "verifier.terminal_bench.configure_git_webserver.v1",
          selectedRouteKind: "apple_fm",
          expectedCostRef: "cost.expected.apple_fm.local",
          observedCostRef: "cost.observed.apple_fm.local",
          expectedLatencyMs: 300000,
          observedLatencyMs: 42000,
          privacyTier: "local_only",
          trustTier: "self_hosted",
          selectedSignatureRefs: ["program_signature.probe.benchmark.service_readiness.v1"],
          toolMenuRef: "tool_menu.probe.terminal_bench.service_readiness.v1",
          candidateHash: "sha256:candidate-1",
          rejectedRoutes: [],
          routeReasonRef: "route_reason.apple_fm.local_private",
          postCloseoutRouteScoreBps: 9000,
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
    });
  });
});
