import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Effect, Schema as S } from "effect";
import {
  PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF,
  PROBE_BENCHMARK_DECISION_TRACE_SCHEMA_REF,
  PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF,
  PROBE_BENCHMARK_RUN_SCHEMA_REF,
  ProbeBenchmarkCloseout,
  ProbeBenchmarkContractError,
  ProbeBenchmarkDecisionTrace,
  ProbeBenchmarkFailureClassification,
  ProbeBenchmarkPolicyFinding,
  ProbeBenchmarkRun,
  decodeProbeBenchmarkCloseout,
  decodeProbeBenchmarkDecisionTrace,
  decodeProbeBenchmarkRouteScorecard,
  decodeProbeBenchmarkRun,
  sanitizeProbeBenchmarkProjection,
  validateProbeBenchmarkPublicProjection,
  type ProbeBenchmarkAssignment,
  type ProbeBenchmarkEvidenceSplit,
  type ProbeBenchmarkFailureFamily,
  type ProbeBenchmarkPromotionStatus,
  type ProbeBenchmarkRedactionState,
  type ProbeBenchmarkResourceCostRefs,
  type ProbeBenchmarkRejectedRoute,
  type ProbeBenchmarkRouteKind,
  type ProbeBenchmarkRouteScorecard,
  type ProbeBenchmarkRunStatus,
} from "../contracts/benchmark";
import { type JsonValue, type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { type ProbeStudybenchRubricScore, decodeProbeStudybenchRubricScore } from "./studybench";

export const PROBE_BENCHMARK_CLOSEOUT_BUNDLE_SCHEMA_REF = "probe.benchmark_closeout_bundle.v1" as const;
export const PROBE_GEPA_LIVE_RUNNER_GATE_SCHEMA_REF = "probe.gepa_live_runner_gate.v1" as const;

export const PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES = [
  "probe-run-record.json",
  "probe-closeout.json",
  "decision-trace-summary.json",
  "selected-signatures.json",
  "tool-menu.json",
  "candidate-ref.json",
  "artifact-refs.json",
  "resource-usage-ref.json",
  "policy-findings.json",
  "failure-classification.json",
  "route-scorecard.json",
  "studybench-task-ref.json",
  "rubric-score.json",
] as const;
export type ProbeBenchmarkCloseoutBundleFileName = (typeof PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES)[number];

export type ProbeBenchmarkTerminalRunStatus = Extract<
  ProbeBenchmarkRunStatus,
  "succeeded" | "failed" | "timed_out" | "policy_blocked" | "errored"
>;

export interface ProbeBenchmarkCloseoutWriterInput {
  readonly assignment: ProbeBenchmarkAssignment;
  readonly artifactManifestRefs?: ReadonlyArray<string>;
  readonly backendRouteRef?: string;
  readonly candidateComponentRefs?: ReadonlyArray<string>;
  readonly completedAt?: string;
  readonly costRef?: string;
  readonly decisionStepRefs?: ReadonlyArray<string>;
  readonly failureClassification?: ProbeBenchmarkFailureClassification;
  readonly observedAt?: string;
  readonly partialArtifactRefs?: ReadonlyArray<string>;
  readonly policyFindings?: ReadonlyArray<ProbeBenchmarkPolicyFinding>;
  readonly proofBundleRefs?: ReadonlyArray<string>;
  readonly redactionState?: ProbeBenchmarkRedactionState;
  readonly resourceUnavailableReason?: string;
  readonly resourceUsageRef?: string;
  readonly routeScorecard?: ProbeBenchmarkRouteScorecard;
  readonly retainedFailureRefs?: ReadonlyArray<string>;
  readonly runRef: string;
  readonly runStatus: ProbeBenchmarkTerminalRunStatus;
  readonly scorerRef: string;
  readonly startedAt?: string;
  readonly summaryArtifactRef?: string;
  readonly studybenchEvidenceUseRefs?: ReadonlyArray<string>;
  readonly studybenchRubricScore?: ProbeStudybenchRubricScore;
  readonly studybenchScoreRef?: string;
  readonly studybenchTaskRef?: string;
  readonly toolMenuSnapshot?: JsonValue;
  readonly verifierRef: string;
  readonly verifierResultRefs?: ReadonlyArray<string>;
}

export interface ProbeBenchmarkCloseoutBundle {
  readonly assignmentRef: string;
  readonly bundleRef: string;
  readonly candidateHash: string;
  readonly evidenceSplit: ProbeBenchmarkEvidenceSplit;
  readonly files: Readonly<Record<ProbeBenchmarkCloseoutBundleFileName, JsonValue>>;
  readonly runRef: string;
  readonly schemaRef: typeof PROBE_BENCHMARK_CLOSEOUT_BUNDLE_SCHEMA_REF;
}

export interface ProbeBenchmarkCloseoutBundleWriteResult {
  readonly bundleRef: string;
  readonly directory: string;
  readonly files: ReadonlyArray<{
    readonly fileName: ProbeBenchmarkCloseoutBundleFileName;
    readonly path: string;
  }>;
}

export interface ProbeGepaLiveRunnerGateInput {
  readonly bundle: ProbeBenchmarkCloseoutBundle;
  readonly candidateManifestAuthorityRefs: ReadonlyArray<string>;
  readonly mode: "sandbox" | "live";
  readonly productPromotionGateRefs?: ReadonlyArray<string>;
  readonly publicScoreGateRefs?: ReadonlyArray<string>;
  readonly payoutGateRefs?: ReadonlyArray<string>;
  readonly runnerExecutionRefs: ReadonlyArray<string>;
}

export interface ProbeGepaLiveRunnerGateProjection {
  readonly assignmentRef: string;
  readonly blockerRefs: ReadonlyArray<string>;
  readonly bundleRef: string;
  readonly candidateHash: string;
  readonly candidateManifestAuthorityRefs: ReadonlyArray<string>;
  readonly closeoutRef: string;
  readonly evidenceRefs: {
    readonly artifactRefs: ReadonlyArray<string>;
    readonly candidateRefs: ReadonlyArray<string>;
    readonly failureRefs: ReadonlyArray<string>;
    readonly proofRefs: ReadonlyArray<string>;
    readonly resourceRefs: ReadonlyArray<string>;
    readonly routeScorecardRefs: ReadonlyArray<string>;
    readonly runRefs: ReadonlyArray<string>;
    readonly selectedSignatureRefs: ReadonlyArray<string>;
    readonly studybenchRefs: ReadonlyArray<string>;
    readonly toolMenuRefs: ReadonlyArray<string>;
    readonly verifierRefs: ReadonlyArray<string>;
  };
  readonly evidenceSplit: ProbeBenchmarkEvidenceSplit;
  readonly mode: "sandbox" | "live";
  readonly omegaImportAllowed: boolean;
  readonly productPromotionAllowed: boolean;
  readonly productPromotionGateRefs: ReadonlyArray<string>;
  readonly publicScoreClaimAllowed: boolean;
  readonly publicScoreGateRefs: ReadonlyArray<string>;
  readonly payoutClaimAllowed: boolean;
  readonly payoutGateRefs: ReadonlyArray<string>;
  readonly runnerExecutionRefs: ReadonlyArray<string>;
  readonly runnerGateReady: boolean;
  readonly runRef: string;
  readonly runStatus: ProbeBenchmarkTerminalRunStatus;
  readonly schemaRef: typeof PROBE_GEPA_LIVE_RUNNER_GATE_SCHEMA_REF;
}

export class ProbeBenchmarkCloseoutWriterError extends S.TaggedErrorClass<ProbeBenchmarkCloseoutWriterError>()(
  "ProbeBenchmarkCloseoutWriterError",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export function makeProbeBenchmarkCloseoutBundle(
  input: ProbeBenchmarkCloseoutWriterInput,
): Effect.Effect<
  ProbeBenchmarkCloseoutBundle,
  ProbeBenchmarkCloseoutWriterError | ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(input, "benchmarkCloseoutWriterInput");

    const assignment = input.assignment;
    const observedAt = input.observedAt ?? new Date().toISOString();
    const artifactManifestRefs = materialRefs(
      input.artifactManifestRefs,
      assignment.requiredArtifacts.artifactRefs,
    );
    const proofBundleRefs = materialRefs(input.proofBundleRefs, assignment.requiredArtifacts.proofBundleRefs);
    const partialArtifactRefs = [...(input.partialArtifactRefs ?? [])];
    const resourceCostRefs = makeResourceCostRefs(input);
    const failureClassification = input.failureClassification ?? defaultFailureClassification(input.runStatus);
    const retainedFailureRefs = retainedRefsFor(input, failureClassification.family);
    const policyFindings = policyFindingsFor(input);
    const promotionStatus = input.runStatus === "succeeded"
      ? promotionStatusForSplit(assignment.split.evidenceSplit)
      : "blocked";
    const summaryArtifactRef = input.summaryArtifactRef ?? `artifact.probe.benchmark.${input.runRef}.decision_trace_summary`;
    const routeScorecard = yield* routeScorecardFor(input, resourceCostRefs);
    const studybenchCloseoutRefs = yield* studybenchCloseoutRefsFor(input);

    if (resourceCostRefs.resourceUsageRef === undefined && resourceCostRefs.unavailableReason === undefined) {
      return yield* Effect.fail(
        new ProbeBenchmarkCloseoutWriterError({
          path: "benchmarkCloseoutWriterInput.resourceUsageRef",
          reason: "must include resourceUsageRef or resourceUnavailableReason",
        }),
      );
    }

    const runRecord = yield* decodeProbeBenchmarkRun({
      schemaRef: PROBE_BENCHMARK_RUN_SCHEMA_REF,
      assignmentRef: assignment.assignmentRef,
      candidateHash: assignment.candidateHash,
      closeoutRef: closeoutRefFor(input.runRef),
      completedAt: input.completedAt ?? observedAt,
      evidenceSplit: assignment.split.evidenceSplit,
      resultSummaryRef: summaryArtifactRef,
      runRef: input.runRef,
      startedAt: input.startedAt ?? observedAt,
      status: input.runStatus,
    });

    const decisionTrace = yield* decodeProbeBenchmarkDecisionTrace({
      schemaRef: PROBE_BENCHMARK_DECISION_TRACE_SCHEMA_REF,
      assignmentRef: assignment.assignmentRef,
      candidateHash: assignment.candidateHash,
      decisionStepRefs: input.decisionStepRefs ?? [],
      redactionState: input.redactionState ?? "public_safe",
      runRef: input.runRef,
      selectedSignatureRefs: assignment.selectedBlueprintSignatureRefs,
      summaryArtifactRef,
      toolMenuRef: assignment.toolMenuRef,
      traceRef: `decision_trace.probe.benchmark.${input.runRef}`,
    });

    const closeout = yield* decodeProbeBenchmarkCloseout({
      schemaRef: PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF,
      artifactManifestRefs,
      assignmentRef: assignment.assignmentRef,
      backendRoute: {
        backendRef: assignment.backend.backendRef,
        backendRouteRef: input.backendRouteRef ?? `backend_route.${assignment.backend.backendRef}.${assignment.runtime.backendProfileRef}`,
        modelBackendRef: assignment.backend.modelBackendRef,
        runtimeProfileRef: assignment.runtime.backendProfileRef,
      },
      candidateHash: assignment.candidateHash,
      closeoutRef: closeoutRefFor(input.runRef),
      evidenceSplit: assignment.split.evidenceSplit,
      failureClassification,
      policyFindings,
      promotionStatus,
      proofBundleRefs,
      redactionState: input.redactionState ?? "public_safe",
      resourceCostRefs,
      retainedFailureRefs,
      routeScorecardRef: routeScorecard.scorecardRef,
      runRef: input.runRef,
      runStatus: input.runStatus,
      selectedSignatureRefs: assignment.selectedBlueprintSignatureRefs,
      toolMenuRef: assignment.toolMenuRef,
      verifierScorerRefs: {
        scorerRef: input.scorerRef,
        verifierRef: input.verifierRef,
      },
    });

    const files: Record<ProbeBenchmarkCloseoutBundleFileName, JsonValue> = {
      "probe-run-record.json": toJsonValue(runRecord),
      "probe-closeout.json": toJsonValue(closeout),
      "decision-trace-summary.json": toJsonValue(decisionTraceSummary(decisionTrace)),
      "selected-signatures.json": toJsonValue({
        schemaRef: "probe.selected_signatures_summary.v1",
        assignmentRef: assignment.assignmentRef,
        registrySplitRef: assignment.split.splitRef,
        selectedSignatureRefs: assignment.selectedBlueprintSignatureRefs,
      }),
      "tool-menu.json": toJsonValue({
        schemaRef: "probe.tool_menu_summary.v1",
        assignmentRef: assignment.assignmentRef,
        redactionState: input.redactionState ?? "public_safe",
        selectedSignatureRefs: assignment.selectedBlueprintSignatureRefs,
        snapshot: input.toolMenuSnapshot === undefined ? undefined : sanitizeProbeBenchmarkProjection(input.toolMenuSnapshot),
        toolMenuRef: assignment.toolMenuRef,
      }),
      "candidate-ref.json": toJsonValue({
        schemaRef: "probe.candidate_ref_summary.v1",
        assignmentRef: assignment.assignmentRef,
        candidateHash: assignment.candidateHash,
        candidateComponentRefs: [...(input.candidateComponentRefs ?? [])],
        candidateRefs: assignment.candidateRefs ?? {},
        evidenceSplit: assignment.split.evidenceSplit,
      }),
      "artifact-refs.json": toJsonValue({
        schemaRef: "probe.artifact_refs_summary.v1",
        assignmentRef: assignment.assignmentRef,
        artifactManifestRefs,
        partialArtifactRefs,
        proofBundleRefs,
        runStatus: input.runStatus,
        studybenchEvidenceUseRefs: studybenchCloseoutRefs.evidenceUseRefs,
        studybenchScoreRef: studybenchCloseoutRefs.scoreRef,
        studybenchTaskRef: studybenchCloseoutRefs.taskRef,
        verifierResultRefs: [...(input.verifierResultRefs ?? [])],
      }),
      "resource-usage-ref.json": toJsonValue({
        schemaRef: "probe.resource_usage_ref_summary.v1",
        assignmentRef: assignment.assignmentRef,
        ...resourceCostRefs,
      }),
      "policy-findings.json": toJsonValue({
        schemaRef: "probe.policy_findings_summary.v1",
        assignmentRef: assignment.assignmentRef,
        policyFindings,
      }),
      "failure-classification.json": toJsonValue({
        schemaRef: "probe.failure_classification_summary.v1",
        assignmentRef: assignment.assignmentRef,
        failureClassification,
        retainedFailureRefs,
      }),
      "route-scorecard.json": toJsonValue(routeScorecard),
      "studybench-task-ref.json": toJsonValue({
        schemaRef: "probe.studybench_task_ref_summary.v1",
        assignmentRef: assignment.assignmentRef,
        evidenceSplit: assignment.split.evidenceSplit,
        present: studybenchCloseoutRefs.present,
        taskChecksum: assignment.task.taskChecksum,
        taskRef: studybenchCloseoutRefs.taskRef,
      }),
      "rubric-score.json": toJsonValue({
        schemaRef: "probe.studybench_rubric_score_summary.v1",
        assignmentRef: assignment.assignmentRef,
        evidenceUseRefs: studybenchCloseoutRefs.evidenceUseRefs,
        present: studybenchCloseoutRefs.rubricScore !== undefined || studybenchCloseoutRefs.scoreRef !== undefined,
        rubricScore: studybenchCloseoutRefs.rubricScore,
        rubricScoreRef: studybenchCloseoutRefs.scoreRef,
        taskRef: studybenchCloseoutRefs.taskRef,
      }),
    };

    return {
      assignmentRef: assignment.assignmentRef,
      bundleRef: `probe_benchmark_closeout_bundle.${input.runRef}`,
      candidateHash: assignment.candidateHash,
      evidenceSplit: assignment.split.evidenceSplit,
      files,
      runRef: input.runRef,
      schemaRef: PROBE_BENCHMARK_CLOSEOUT_BUNDLE_SCHEMA_REF,
    };
  });
}

export function projectProbeGepaLiveRunnerGate(
  input: ProbeGepaLiveRunnerGateInput,
): Effect.Effect<
  ProbeGepaLiveRunnerGateProjection,
  ProbeBenchmarkCloseoutWriterError | ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(input, "probeGepaLiveRunnerGateInput");

    const files = input.bundle.files;
    const run = yield* expectRecord(files["probe-run-record.json"], "probe-run-record.json");
    const closeout = yield* expectRecord(files["probe-closeout.json"], "probe-closeout.json");
    const artifacts = yield* expectRecord(files["artifact-refs.json"], "artifact-refs.json");
    const resource = yield* expectRecord(files["resource-usage-ref.json"], "resource-usage-ref.json");
    const failure = yield* expectRecord(files["failure-classification.json"], "failure-classification.json");
    const scorecard = yield* expectRecord(files["route-scorecard.json"], "route-scorecard.json");
    const signatures = yield* expectRecord(files["selected-signatures.json"], "selected-signatures.json");
    const toolMenu = yield* expectRecord(files["tool-menu.json"], "tool-menu.json");
    const candidate = yield* expectRecord(files["candidate-ref.json"], "candidate-ref.json");
    const studybenchTask = expectOptionalRecord(files["studybench-task-ref.json"]);
    const rubricScore = expectOptionalRecord(files["rubric-score.json"]);

    const evidenceRefs = {
      artifactRefs: refsFrom(artifacts.artifactManifestRefs, artifacts.partialArtifactRefs),
      candidateRefs: refsFrom(
        candidate.candidateComponentRefs,
        Object.values(expectOptionalRecord(candidate.candidateRefs)),
      ),
      failureRefs: refsFrom(
        getNestedString(failure, ["failureClassification", "classificationRef"]),
        failure.retainedFailureRefs,
      ),
      proofRefs: refsFrom(artifacts.proofBundleRefs),
      resourceRefs: refsFrom(resource.resourceUsageRef, resource.unavailableReason),
      routeScorecardRefs: refsFrom(scorecard.scorecardRef, closeout.routeScorecardRef),
      runRefs: refsFrom(input.bundle.runRef, run.runRef),
      selectedSignatureRefs: refsFrom(signatures.selectedSignatureRefs, closeout.selectedSignatureRefs),
      studybenchRefs: refsFrom(
        studybenchTask.taskRef,
        studybenchTask.taskChecksum,
        rubricScore.rubricScoreRef,
        rubricScore.evidenceUseRefs,
        artifacts.studybenchTaskRef,
        artifacts.studybenchScoreRef,
        artifacts.studybenchEvidenceUseRefs,
      ),
      toolMenuRefs: refsFrom(toolMenu.toolMenuRef, closeout.toolMenuRef),
      verifierRefs: refsFrom(getNestedString(closeout, ["verifierScorerRefs", "verifierRef"]), artifacts.verifierResultRefs),
    };

    const blockerRefs = [
      ...missingRefBlockers(evidenceRefs.artifactRefs, "artifact"),
      ...missingRefBlockers(evidenceRefs.candidateRefs, "candidate"),
      ...missingRefBlockers(evidenceRefs.proofRefs, "proof"),
      ...missingRefBlockers(evidenceRefs.resourceRefs, "resource"),
      ...missingRefBlockers(evidenceRefs.routeScorecardRefs, "route_scorecard"),
      ...missingRefBlockers(evidenceRefs.runRefs, "run"),
      ...missingRefBlockers(evidenceRefs.selectedSignatureRefs, "selected_signature"),
      ...missingRefBlockers(evidenceRefs.toolMenuRefs, "tool_menu"),
      ...missingRefBlockers(evidenceRefs.verifierRefs, "verifier"),
      ...missingRefBlockers(input.candidateManifestAuthorityRefs, "candidate_manifest_authority"),
      ...missingRefBlockers(input.runnerExecutionRefs, "runner_execution"),
    ];

    const runStatus = yield* expectRunStatus(closeout.runStatus);
    if (runStatus !== "succeeded") {
      blockerRefs.push(...missingRefBlockers(evidenceRefs.failureRefs, "failure"));
    }

    const runnerGateReady = blockerRefs.length === 0;
    const publicScoreGateRefs = [...(input.publicScoreGateRefs ?? [])];
    const productPromotionGateRefs = [...(input.productPromotionGateRefs ?? [])];
    const payoutGateRefs = [...(input.payoutGateRefs ?? [])];

    return {
      assignmentRef: yield* expectString(closeout.assignmentRef, "probe-closeout.json.assignmentRef"),
      blockerRefs,
      bundleRef: input.bundle.bundleRef,
      candidateHash: input.bundle.candidateHash,
      candidateManifestAuthorityRefs: [...input.candidateManifestAuthorityRefs],
      closeoutRef: yield* expectString(closeout.closeoutRef, "probe-closeout.json.closeoutRef"),
      evidenceRefs,
      evidenceSplit: input.bundle.evidenceSplit,
      mode: input.mode,
      omegaImportAllowed: runnerGateReady,
      productPromotionAllowed: runnerGateReady && productPromotionGateRefs.length > 0,
      productPromotionGateRefs,
      publicScoreClaimAllowed: runnerGateReady && publicScoreGateRefs.length > 0,
      publicScoreGateRefs,
      payoutClaimAllowed: runnerGateReady && payoutGateRefs.length > 0,
      payoutGateRefs,
      runnerExecutionRefs: [...input.runnerExecutionRefs],
      runnerGateReady,
      runRef: input.bundle.runRef,
      runStatus,
      schemaRef: PROBE_GEPA_LIVE_RUNNER_GATE_SCHEMA_REF,
    };
  });
}

function routeScorecardFor(
  input: ProbeBenchmarkCloseoutWriterInput,
  resourceCostRefs: ProbeBenchmarkResourceCostRefs,
): Effect.Effect<ProbeBenchmarkRouteScorecard, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  if (input.routeScorecard !== undefined) {
    return decodeProbeBenchmarkRouteScorecard(input.routeScorecard);
  }

  const assignment = input.assignment;
  const selectedRouteKind = routeKindForAssignment(assignment);
  const expectedLatencyMs = assignment.timeoutBudgetPolicy.maxDurationMs ?? 300_000;
  const observedLatencyMs = input.runStatus === "succeeded" ? Math.min(expectedLatencyMs, 60_000) : expectedLatencyMs;
  const observedCostRef = resourceCostRefs.costRef ?? `cost.observed.${input.runRef}.unavailable`;

  return decodeProbeBenchmarkRouteScorecard({
    schemaRef: PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF,
    scorecardRef: `route_scorecard.probe.benchmark.${input.runRef}`,
    selectedAgentOrModelRef: assignment.backend.modelBackendRef,
    selectedRunnerRef: assignment.runtime.runtimeRef,
    selectedProviderRef: assignment.backend.backendRef,
    selectedIsolationProfileRef: isolationProfileForRoute(selectedRouteKind),
    selectedVerifierRef: input.verifierRef,
    selectedRouteKind,
    expectedCostRef: input.costRef ?? `cost.expected.${assignment.runtime.backendProfileRef}`,
    observedCostRef,
    expectedLatencyMs,
    observedLatencyMs,
    privacyTier: privacyTierForRoute(selectedRouteKind),
    trustTier: trustTierForRoute(selectedRouteKind),
    selectedSignatureRefs: assignment.selectedBlueprintSignatureRefs,
    toolMenuRef: assignment.toolMenuRef,
    candidateHash: assignment.candidateHash,
    rejectedRoutes: rejectedRoutesForSelectedRoute(selectedRouteKind),
    routeReasonRef: `route_reason.probe.${selectedRouteKind}.${assignment.split.evidenceSplit}`,
    postCloseoutRouteScoreBps: routeScoreForStatus(input.runStatus),
  });
}

function routeKindForAssignment(assignment: ProbeBenchmarkAssignment): ProbeBenchmarkRouteKind {
  const combined = [
    assignment.backend.backendRef,
    assignment.backend.modelBackendRef,
    assignment.runtime.backendProfileRef,
    assignment.runtime.runtimeRef,
  ]
    .join(" ")
    .toLowerCase();

  if (combined.includes("apple_fm") || combined.includes("foundation_model")) {
    return "apple_fm";
  }

  if (combined.includes("qwen")) {
    return "local_qwen";
  }

  if (combined.includes("pylon")) {
    return "pylon";
  }

  if (combined.includes("shc")) {
    return "shc";
  }

  if (combined.includes("probe") && combined.includes("codex")) {
    return "probe_codex";
  }

  if (combined.includes("codex")) {
    return "codex";
  }

  return "probe_codex";
}

function isolationProfileForRoute(routeKind: ProbeBenchmarkRouteKind): string {
  switch (routeKind) {
    case "apple_fm":
    case "local_qwen":
      return "isolation.local_sandbox";
    case "codex":
    case "probe_codex":
      return "isolation.workspace_shell";
    case "pylon":
      return "isolation.pylon_worker_sandbox";
    case "shc":
      return "isolation.shc_box";
  }
}

function privacyTierForRoute(routeKind: ProbeBenchmarkRouteKind): ProbeBenchmarkRouteScorecard["privacyTier"] {
  switch (routeKind) {
    case "apple_fm":
    case "local_qwen":
      return "local_only";
    case "shc":
      return "shc_box";
    case "pylon":
      return "pylon_worker";
    case "codex":
    case "probe_codex":
      return "remote_api";
  }
}

function trustTierForRoute(routeKind: ProbeBenchmarkRouteKind): ProbeBenchmarkRouteScorecard["trustTier"] {
  switch (routeKind) {
    case "apple_fm":
    case "local_qwen":
      return "self_hosted";
    case "shc":
      return "owned_worker";
    case "pylon":
      return "registered_pylon";
    case "codex":
    case "probe_codex":
      return "external_provider";
  }
}

function rejectedRoutesForSelectedRoute(routeKind: ProbeBenchmarkRouteKind): ReadonlyArray<ProbeBenchmarkRejectedRoute> {
  const allRoutes: ReadonlyArray<ProbeBenchmarkRouteKind> = [
    "codex",
    "probe_codex",
    "apple_fm",
    "local_qwen",
    "shc",
    "pylon",
  ];

  return allRoutes
    .filter((candidate) => candidate !== routeKind)
    .map((candidate) => ({
      reasonRef: `route_rejection.probe.${candidate}.not_selected_for_current_assignment`,
      routeKind: candidate,
      routeRef: `route.probe.${candidate}`,
    }));
}

function routeScoreForStatus(runStatus: ProbeBenchmarkTerminalRunStatus): number {
  switch (runStatus) {
    case "succeeded":
      return 10_000;
    case "failed":
      return 2_500;
    case "timed_out":
      return 1_000;
    case "policy_blocked":
      return 0;
    case "errored":
      return 500;
  }
}

interface StudybenchCloseoutRefs {
  readonly evidenceUseRefs: ReadonlyArray<string>;
  readonly present: boolean;
  readonly rubricScore?: ProbeStudybenchRubricScore;
  readonly scoreRef?: string;
  readonly taskRef?: string;
}

function studybenchCloseoutRefsFor(
  input: ProbeBenchmarkCloseoutWriterInput,
): Effect.Effect<StudybenchCloseoutRefs, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const hasStudybenchInput =
      input.studybenchTaskRef !== undefined ||
      input.studybenchScoreRef !== undefined ||
      input.studybenchRubricScore !== undefined ||
      (input.studybenchEvidenceUseRefs?.length ?? 0) > 0;

    if (!hasStudybenchInput) {
      return {
        evidenceUseRefs: [],
        present: false,
      };
    }

    if (input.studybenchTaskRef === undefined) {
      return yield* studybenchCloseoutError(
        "benchmarkCloseoutWriterInput.studybenchTaskRef",
        "is required when StudyBench score evidence is attached",
      );
    }

    yield* requireOpaqueStudybenchRef(input.studybenchTaskRef, "benchmarkCloseoutWriterInput.studybenchTaskRef");

    let rubricScore: ProbeStudybenchRubricScore | undefined;
    if (input.studybenchRubricScore !== undefined) {
      rubricScore = yield* decodeProbeStudybenchRubricScore(input.studybenchRubricScore);
      yield* validateRubricScorePublicRefs(rubricScore);
    }

    const scoreRef =
      input.studybenchScoreRef ??
      (rubricScore === undefined
        ? undefined
        : `rubric_score.probe.studybench.${input.runRef}.${rubricScore.taskId}`);

    if (scoreRef !== undefined) {
      yield* requireOpaqueStudybenchRef(scoreRef, "benchmarkCloseoutWriterInput.studybenchScoreRef");
    }

    const evidenceUseRefs = input.studybenchEvidenceUseRefs ?? rubricScore?.evidenceUseRefs ?? [];
    for (const [index, ref] of evidenceUseRefs.entries()) {
      yield* requireOpaqueStudybenchRef(ref, `benchmarkCloseoutWriterInput.studybenchEvidenceUseRefs[${index}]`);
    }

    return {
      evidenceUseRefs: [...evidenceUseRefs],
      present: true,
      rubricScore,
      scoreRef,
      taskRef: input.studybenchTaskRef,
    };
  });
}

function validateRubricScorePublicRefs(
  rubricScore: ProbeStudybenchRubricScore,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireOpaqueStudybenchRef(rubricScore.candidateHash, "studybenchRubricScore.candidateHash");
    yield* requireOpaqueStudybenchRef(rubricScore.goldAnswerRef, "studybenchRubricScore.goldAnswerRef");

    for (const [index, ref] of rubricScore.evidenceUseRefs.entries()) {
      yield* requireOpaqueStudybenchRef(ref, `studybenchRubricScore.evidenceUseRefs[${index}]`);
    }

    for (const [index, claimScore] of rubricScore.claimScores.entries()) {
      yield* requireOpaqueStudybenchRef(claimScore.claimId, `studybenchRubricScore.claimScores[${index}].claimId`);
      yield* requireOpaqueStudybenchRef(
        claimScore.rationaleRef,
        `studybenchRubricScore.claimScores[${index}].rationaleRef`,
      );
      yield* requireOpaqueStudybenchRef(claimScore.scorerRef, `studybenchRubricScore.claimScores[${index}].scorerRef`);

      for (const [spanIndex, spanId] of claimScore.evidenceSpanIds.entries()) {
        yield* requireOpaqueStudybenchRef(
          spanId,
          `studybenchRubricScore.claimScores[${index}].evidenceSpanIds[${spanIndex}]`,
        );
      }
    }
  });
}

function requireOpaqueStudybenchRef(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (value.trim().length === 0) {
    return studybenchCloseoutError(path, "must be a non-empty ref");
  }

  if (/\s/.test(value) || /[!?]$/.test(value) || /because|critique/i.test(value)) {
    return studybenchCloseoutError(path, "must be an opaque artifact ref, not raw evaluator text");
  }

  return Effect.void;
}

function studybenchCloseoutError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

export function writeProbeBenchmarkCloseoutBundle(
  bundle: ProbeBenchmarkCloseoutBundle,
  directory: string,
): Effect.Effect<ProbeBenchmarkCloseoutBundleWriteResult, ProbeBenchmarkCloseoutWriterError> {
  return Effect.tryPromise({
    try: async () => {
      await mkdir(directory, { recursive: true });

      const files = await Promise.all(
        PROBE_BENCHMARK_CLOSEOUT_BUNDLE_FILE_NAMES.map(async (fileName) => {
          const path = join(directory, fileName);
          await writeFile(path, `${JSON.stringify(bundle.files[fileName], null, 2)}\n`, "utf8");
          return { fileName, path };
        }),
      );

      return {
        bundleRef: bundle.bundleRef,
        directory,
        files,
      };
    },
    catch: (error) =>
      new ProbeBenchmarkCloseoutWriterError({
        path: directory,
        reason: error instanceof Error ? error.message : String(error),
      }),
  });
}

function materialRefs(
  explicitRefs: ReadonlyArray<string> | undefined,
  requiredRefs: ReadonlyArray<string>,
): ReadonlyArray<string> {
  return explicitRefs === undefined ? [...requiredRefs] : [...explicitRefs];
}

function makeResourceCostRefs(input: ProbeBenchmarkCloseoutWriterInput): ProbeBenchmarkResourceCostRefs {
  if (input.resourceUsageRef !== undefined) {
    return {
      costRef: input.costRef,
      resourceUsageRef: input.resourceUsageRef,
    };
  }

  return {
    costRef: input.costRef,
    unavailableReason: input.resourceUnavailableReason ?? `${input.runStatus}_resource_usage_unavailable`,
  };
}

function defaultFailureClassification(runStatus: ProbeBenchmarkTerminalRunStatus): ProbeBenchmarkFailureClassification {
  return {
    classificationRef: `failure_classification.probe.${runStatus}`,
    family: failureFamilyForStatus(runStatus),
  };
}

function failureFamilyForStatus(runStatus: ProbeBenchmarkTerminalRunStatus): ProbeBenchmarkFailureFamily {
  switch (runStatus) {
    case "succeeded":
      return "none";
    case "timed_out":
      return "timeout";
    case "policy_blocked":
      return "policy_blocked";
    case "errored":
      return "runtime_error";
    case "failed":
      return "unknown";
  }
}

function retainedRefsFor(
  input: ProbeBenchmarkCloseoutWriterInput,
  failureFamily: ProbeBenchmarkFailureFamily,
): ReadonlyArray<string> {
  if (input.retainedFailureRefs !== undefined) {
    return [...input.retainedFailureRefs];
  }

  if (input.assignment.split.evidenceSplit !== "retained" || input.runStatus === "succeeded") {
    return [];
  }

  return [`retained_failure.${input.assignment.dataset.slug}.${input.assignment.taskRunRef}.${failureFamily}`];
}

function policyFindingsFor(input: ProbeBenchmarkCloseoutWriterInput): ReadonlyArray<ProbeBenchmarkPolicyFinding> {
  if (input.policyFindings !== undefined) {
    return [...input.policyFindings];
  }

  return input.runStatus === "policy_blocked"
    ? [
        {
          findingRef: `policy_finding.probe.benchmark.${input.runRef}.blocked`,
          severity: "blocked",
        },
      ]
    : [];
}

function promotionStatusForSplit(evidenceSplit: ProbeBenchmarkEvidenceSplit): ProbeBenchmarkPromotionStatus {
  switch (evidenceSplit) {
    case "retained":
      return "retained_evidence";
    case "validation":
      return "validation_candidate";
    case "holdout":
      return "holdout_candidate";
    case "live":
      return "live_evidence";
  }
}

function closeoutRefFor(runRef: string): string {
  return `probe_closeout.${runRef}`;
}

function decisionTraceSummary(trace: ProbeBenchmarkDecisionTrace): JsonValue {
  return {
    schemaRef: "probe.decision_trace_summary.v1",
    assignmentRef: trace.assignmentRef,
    candidateHash: trace.candidateHash,
    decisionStepRefs: trace.decisionStepRefs,
    redactionState: trace.redactionState,
    runRef: trace.runRef,
    selectedSignatureRefs: trace.selectedSignatureRefs,
    summaryArtifactRef: trace.summaryArtifactRef,
    toolMenuRef: trace.toolMenuRef,
    traceRef: trace.traceRef,
  };
}

function expectRecord(
  value: JsonValue,
  path: string,
): Effect.Effect<{ readonly [key: string]: JsonValue }, ProbeBenchmarkCloseoutWriterError> {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    return Effect.succeed(value);
  }

  return Effect.fail(
    new ProbeBenchmarkCloseoutWriterError({
      path,
      reason: "expected closeout bundle file to contain a JSON object",
    }),
  );
}

function expectOptionalRecord(value: JsonValue | undefined): { readonly [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? value : {};
}

function expectString(
  value: JsonValue | undefined,
  path: string,
): Effect.Effect<string, ProbeBenchmarkCloseoutWriterError> {
  if (typeof value === "string" && value.length > 0) {
    return Effect.succeed(value);
  }

  return Effect.fail(
    new ProbeBenchmarkCloseoutWriterError({
      path,
      reason: "expected non-empty string ref",
    }),
  );
}

function expectRunStatus(
  value: JsonValue | undefined,
): Effect.Effect<ProbeBenchmarkTerminalRunStatus, ProbeBenchmarkCloseoutWriterError> {
  if (
    value === "succeeded" ||
    value === "failed" ||
    value === "timed_out" ||
    value === "policy_blocked" ||
    value === "errored"
  ) {
    return Effect.succeed(value);
  }

  return Effect.fail(
    new ProbeBenchmarkCloseoutWriterError({
      path: "probe-closeout.json.runStatus",
      reason: "expected terminal run status",
    }),
  );
}

function getNestedString(
  record: { readonly [key: string]: JsonValue },
  path: ReadonlyArray<string>,
): string | undefined {
  let current: JsonValue | undefined = record;

  for (const part of path) {
    if (typeof current !== "object" || current === null || Array.isArray(current)) {
      return undefined;
    }

    current = current[part];
  }

  return typeof current === "string" && current.length > 0 ? current : undefined;
}

function refsFrom(...values: ReadonlyArray<JsonValue | undefined>): ReadonlyArray<string> {
  const refs: string[] = [];

  for (const value of values) {
    if (typeof value === "string" && value.length > 0) {
      refs.push(value);
      continue;
    }

    if (Array.isArray(value)) {
      for (const entry of value) {
        if (typeof entry === "string" && entry.length > 0) {
          refs.push(entry);
        }
      }
    }
  }

  return [...new Set(refs)];
}

function missingRefBlockers(refs: ReadonlyArray<string>, refKind: string): ReadonlyArray<string> {
  return refs.length === 0 ? [`blocker.probe.gepa_live_runner_gate.missing_${refKind}_ref`] : [];
}

function toJsonValue(value: unknown): JsonValue {
  return JSON.parse(JSON.stringify(value)) as JsonValue;
}
