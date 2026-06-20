import { Effect, Schema as S } from "effect";
import {
  JsonValue,
  ProbePublicProjectionUnsafe,
  sanitizeProbePublicProjection,
  validateProbePublicProjection,
} from "./provider-account.js";

export const PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF = "probe.benchmark_assignment.v1" as const;
export const PROBE_BENCHMARK_RUN_SCHEMA_REF = "probe.benchmark_run.v1" as const;
export const PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF = "probe.benchmark_closeout.v1" as const;
export const PROBE_BENCHMARK_DECISION_TRACE_SCHEMA_REF = "probe.benchmark_decision_trace.v1" as const;
export const PROBE_PROMPT_CANDIDATE_SCHEMA_REF = "probe.prompt_candidate.v1" as const;
export const PROBE_BLUEPRINT_CANDIDATE_SCHEMA_REF = "probe.blueprint_candidate.v1" as const;
export const PROBE_TOOL_MENU_CANDIDATE_SCHEMA_REF = "probe.tool_menu_candidate.v1" as const;
export const PROBE_LOOP_POLICY_CANDIDATE_SCHEMA_REF = "probe.loop_policy_candidate.v1" as const;
export const PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF = "probe.benchmark_route_scorecard.v1" as const;
export const PROBE_BENCHMARK_PROMOTION_DECISION_SCHEMA_REF =
  "probe.benchmark_promotion_decision.v1" as const;

export const ProbeBenchmarkEvidenceSplit = S.Literals(["retained", "validation", "holdout", "live"]);
export type ProbeBenchmarkEvidenceSplit = typeof ProbeBenchmarkEvidenceSplit.Type;

export const ProbeBenchmarkRunStatus = S.Literals([
  "queued",
  "running",
  "succeeded",
  "failed",
  "timed_out",
  "policy_blocked",
  "errored",
]);
export type ProbeBenchmarkRunStatus = typeof ProbeBenchmarkRunStatus.Type;

export const ProbeBenchmarkRedactionState = S.Literals(["public_safe", "redacted", "withheld", "unsafe_blocked"]);
export type ProbeBenchmarkRedactionState = typeof ProbeBenchmarkRedactionState.Type;

export const ProbeBenchmarkPromotionStatus = S.Literals([
  "not_evaluated",
  "blocked",
  "retained_evidence",
  "validation_candidate",
  "holdout_candidate",
  "live_evidence",
  "rejected",
]);
export type ProbeBenchmarkPromotionStatus = typeof ProbeBenchmarkPromotionStatus.Type;

export const ProbeBenchmarkRouteKind = S.Literals([
  "apple_fm",
  "codex",
  "local_qwen",
  "probe_codex",
  "pylon",
  "shc",
]);
export type ProbeBenchmarkRouteKind = typeof ProbeBenchmarkRouteKind.Type;

export const ProbeBenchmarkPrivacyTier = S.Literals(["local_only", "shc_box", "pylon_worker", "remote_api"]);
export type ProbeBenchmarkPrivacyTier = typeof ProbeBenchmarkPrivacyTier.Type;

export const ProbeBenchmarkTrustTier = S.Literals([
  "self_hosted",
  "owned_worker",
  "registered_pylon",
  "external_provider",
]);
export type ProbeBenchmarkTrustTier = typeof ProbeBenchmarkTrustTier.Type;

export const ProbeBenchmarkFailureFamily = S.Literals([
  "none",
  "service_readiness",
  "database_recovery",
  "sqlite_wal_recovery",
  "parser_correctness",
  "xss_sanitizer_policy",
  "gcode_parser_guard",
  "package_indexing",
  "python_package_index",
  "query_optimization",
  "runner_supervision",
  "timeout",
  "policy_blocked",
  "verifier_failure",
  "runtime_error",
  "unknown",
]);
export type ProbeBenchmarkFailureFamily = typeof ProbeBenchmarkFailureFamily.Type;

export const ProbeBenchmarkDatasetRef = S.Struct({
  slug: S.String,
  version: S.String,
});
export type ProbeBenchmarkDatasetRef = typeof ProbeBenchmarkDatasetRef.Type;

export const ProbeBenchmarkSplitRef = S.Struct({
  evidenceSplit: ProbeBenchmarkEvidenceSplit,
  splitRef: S.String,
});
export type ProbeBenchmarkSplitRef = typeof ProbeBenchmarkSplitRef.Type;

export const ProbeBenchmarkTaskRef = S.Struct({
  taskChecksum: S.optional(S.String),
  taskRef: S.optional(S.String),
});
export type ProbeBenchmarkTaskRef = typeof ProbeBenchmarkTaskRef.Type;

export const ProbeBenchmarkRuntimeProfile = S.Struct({
  backendProfileRef: S.String,
  runtimeRef: S.String,
});
export type ProbeBenchmarkRuntimeProfile = typeof ProbeBenchmarkRuntimeProfile.Type;

export const ProbeBenchmarkBackendRef = S.Struct({
  backendRef: S.String,
  modelBackendRef: S.String,
});
export type ProbeBenchmarkBackendRef = typeof ProbeBenchmarkBackendRef.Type;

export const ProbeBenchmarkAccountGrantRefs = S.Struct({
  authGrantRef: S.optional(S.String),
  providerAccountRef: S.optional(S.String),
});
export type ProbeBenchmarkAccountGrantRefs = typeof ProbeBenchmarkAccountGrantRefs.Type;

export const ProbeBenchmarkTimeoutBudgetPolicy = S.Struct({
  budgetPolicyRef: S.String,
  maxDurationMs: S.optional(S.Number),
  maxToolCalls: S.optional(S.Number),
  timeoutPolicyRef: S.String,
});
export type ProbeBenchmarkTimeoutBudgetPolicy = typeof ProbeBenchmarkTimeoutBudgetPolicy.Type;

export const ProbeBenchmarkRequiredArtifacts = S.Struct({
  artifactRefs: S.Array(S.String),
  proofBundleRefs: S.Array(S.String),
});
export type ProbeBenchmarkRequiredArtifacts = typeof ProbeBenchmarkRequiredArtifacts.Type;

export const ProbeBenchmarkProofSinks = S.Struct({
  callbackRefs: S.Array(S.String),
  proofSinkRefs: S.Array(S.String),
});
export type ProbeBenchmarkProofSinks = typeof ProbeBenchmarkProofSinks.Type;

export const ProbePromptCandidate = S.Struct({
  artifactRef: S.String,
  candidateHash: S.String,
  promptRef: S.String,
  redactionState: ProbeBenchmarkRedactionState,
  schemaRef: S.Literal(PROBE_PROMPT_CANDIDATE_SCHEMA_REF),
});
export type ProbePromptCandidate = typeof ProbePromptCandidate.Type;

export const ProbeBlueprintCandidate = S.Struct({
  candidateHash: S.String,
  moduleVersionRefs: S.Array(S.String),
  registryVersionRef: S.String,
  releaseGateRefs: S.Array(S.String),
  schemaRef: S.Literal(PROBE_BLUEPRINT_CANDIDATE_SCHEMA_REF),
  selectedSignatureRefs: S.Array(S.String),
});
export type ProbeBlueprintCandidate = typeof ProbeBlueprintCandidate.Type;

export const ProbeToolMenuCandidate = S.Struct({
  candidateHash: S.String,
  deniedToolRefs: S.Array(S.String),
  schemaRef: S.Literal(PROBE_TOOL_MENU_CANDIDATE_SCHEMA_REF),
  toolMenuRef: S.String,
  toolRefs: S.Array(S.String),
});
export type ProbeToolMenuCandidate = typeof ProbeToolMenuCandidate.Type;

export const ProbeLoopPolicyCandidate = S.Struct({
  budgetPolicyRef: S.String,
  candidateHash: S.String,
  loopPolicyRef: S.String,
  maxTurns: S.Number,
  schemaRef: S.Literal(PROBE_LOOP_POLICY_CANDIDATE_SCHEMA_REF),
  stopConditionRefs: S.Array(S.String),
  timeoutPolicyRef: S.String,
});
export type ProbeLoopPolicyCandidate = typeof ProbeLoopPolicyCandidate.Type;

export const ProbeBenchmarkCandidateRefs = S.Struct({
  blueprintCandidateRef: S.optional(S.String),
  loopPolicyCandidateRef: S.optional(S.String),
  promptCandidateRef: S.optional(S.String),
  toolMenuCandidateRef: S.optional(S.String),
});
export type ProbeBenchmarkCandidateRefs = typeof ProbeBenchmarkCandidateRefs.Type;

export const ProbeBenchmarkAssignment = S.Struct({
  accountGrantRefs: S.optional(ProbeBenchmarkAccountGrantRefs),
  assignmentRef: S.String,
  backend: ProbeBenchmarkBackendRef,
  benchmarkRunRef: S.String,
  candidateHash: S.String,
  candidateRefs: S.optional(ProbeBenchmarkCandidateRefs),
  dataset: ProbeBenchmarkDatasetRef,
  probeCommit: S.String,
  requiredArtifacts: ProbeBenchmarkRequiredArtifacts,
  runtime: ProbeBenchmarkRuntimeProfile,
  schemaRef: S.Literal(PROBE_BENCHMARK_ASSIGNMENT_SCHEMA_REF),
  selectedBlueprintSignatureRefs: S.Array(S.String),
  split: ProbeBenchmarkSplitRef,
  task: ProbeBenchmarkTaskRef,
  taskRunRef: S.String,
  timeoutBudgetPolicy: ProbeBenchmarkTimeoutBudgetPolicy,
  toolMenuRef: S.String,
  sinks: ProbeBenchmarkProofSinks,
});
export type ProbeBenchmarkAssignment = typeof ProbeBenchmarkAssignment.Type;

export const ProbeBenchmarkRun = S.Struct({
  assignmentRef: S.String,
  candidateHash: S.String,
  closeoutRef: S.optional(S.String),
  completedAt: S.optional(S.String),
  evidenceSplit: ProbeBenchmarkEvidenceSplit,
  resultSummaryRef: S.optional(S.String),
  runRef: S.String,
  schemaRef: S.Literal(PROBE_BENCHMARK_RUN_SCHEMA_REF),
  startedAt: S.optional(S.String),
  status: ProbeBenchmarkRunStatus,
});
export type ProbeBenchmarkRun = typeof ProbeBenchmarkRun.Type;

export const ProbeBenchmarkDecisionTrace = S.Struct({
  assignmentRef: S.String,
  candidateHash: S.String,
  decisionStepRefs: S.Array(S.String),
  redactionState: ProbeBenchmarkRedactionState,
  runRef: S.String,
  schemaRef: S.Literal(PROBE_BENCHMARK_DECISION_TRACE_SCHEMA_REF),
  selectedSignatureRefs: S.Array(S.String),
  summaryArtifactRef: S.String,
  toolMenuRef: S.String,
  traceRef: S.String,
});
export type ProbeBenchmarkDecisionTrace = typeof ProbeBenchmarkDecisionTrace.Type;

export const ProbeBenchmarkBackendRoute = S.Struct({
  backendRef: S.String,
  backendRouteRef: S.String,
  modelBackendRef: S.String,
  runtimeProfileRef: S.String,
});
export type ProbeBenchmarkBackendRoute = typeof ProbeBenchmarkBackendRoute.Type;

export const ProbeBenchmarkVerifierScorerRefs = S.Struct({
  scorerRef: S.String,
  verifierRef: S.String,
});
export type ProbeBenchmarkVerifierScorerRefs = typeof ProbeBenchmarkVerifierScorerRefs.Type;

export const ProbeBenchmarkRejectedRoute = S.Struct({
  reasonRef: S.String,
  routeKind: ProbeBenchmarkRouteKind,
  routeRef: S.String,
});
export type ProbeBenchmarkRejectedRoute = typeof ProbeBenchmarkRejectedRoute.Type;

export const ProbeBenchmarkRouteScorecard = S.Struct({
  candidateHash: S.String,
  expectedCostRef: S.String,
  expectedLatencyMs: S.Number,
  observedCostRef: S.String,
  observedLatencyMs: S.Number,
  postCloseoutRouteScoreBps: S.Number,
  privacyTier: ProbeBenchmarkPrivacyTier,
  rejectedRoutes: S.Array(ProbeBenchmarkRejectedRoute),
  routeReasonRef: S.String,
  schemaRef: S.Literal(PROBE_BENCHMARK_ROUTE_SCORECARD_SCHEMA_REF),
  scorecardRef: S.String,
  selectedAgentOrModelRef: S.String,
  selectedIsolationProfileRef: S.String,
  selectedProviderRef: S.String,
  selectedRouteKind: ProbeBenchmarkRouteKind,
  selectedRunnerRef: S.String,
  selectedSignatureRefs: S.Array(S.String),
  selectedVerifierRef: S.String,
  toolMenuRef: S.String,
  trustTier: ProbeBenchmarkTrustTier,
});
export type ProbeBenchmarkRouteScorecard = typeof ProbeBenchmarkRouteScorecard.Type;

export const ProbeBenchmarkResourceCostRefs = S.Struct({
  costRef: S.optional(S.String),
  resourceUsageRef: S.optional(S.String),
  unavailableReason: S.optional(S.String),
});
export type ProbeBenchmarkResourceCostRefs = typeof ProbeBenchmarkResourceCostRefs.Type;

export const ProbeBenchmarkPolicyFinding = S.Struct({
  findingRef: S.String,
  severity: S.Literals(["info", "warning", "blocked"]),
});
export type ProbeBenchmarkPolicyFinding = typeof ProbeBenchmarkPolicyFinding.Type;

export const ProbeBenchmarkFailureClassification = S.Struct({
  family: ProbeBenchmarkFailureFamily,
  classificationRef: S.String,
  summaryRef: S.optional(S.String),
});
export type ProbeBenchmarkFailureClassification = typeof ProbeBenchmarkFailureClassification.Type;

export const ProbeBenchmarkCloseout = S.Struct({
  artifactManifestRefs: S.Array(S.String),
  assignmentRef: S.String,
  backendRoute: ProbeBenchmarkBackendRoute,
  candidateHash: S.String,
  closeoutRef: S.String,
  evidenceSplit: ProbeBenchmarkEvidenceSplit,
  failureClassification: ProbeBenchmarkFailureClassification,
  policyFindings: S.Array(ProbeBenchmarkPolicyFinding),
  promotionStatus: ProbeBenchmarkPromotionStatus,
  proofBundleRefs: S.Array(S.String),
  redactionState: ProbeBenchmarkRedactionState,
  resourceCostRefs: ProbeBenchmarkResourceCostRefs,
  retainedFailureRefs: S.Array(S.String),
  routeScorecardRef: S.optional(S.String),
  runRef: S.String,
  runStatus: ProbeBenchmarkRunStatus,
  schemaRef: S.Literal(PROBE_BENCHMARK_CLOSEOUT_SCHEMA_REF),
  selectedSignatureRefs: S.Array(S.String),
  toolMenuRef: S.String,
  verifierScorerRefs: ProbeBenchmarkVerifierScorerRefs,
});
export type ProbeBenchmarkCloseout = typeof ProbeBenchmarkCloseout.Type;

export const ProbeBenchmarkPromotionDecision = S.Struct({
  authorityBoundary: S.Literal("evidence_only"),
  closeoutRef: S.String,
  decisionRef: S.String,
  evidenceSplit: ProbeBenchmarkEvidenceSplit,
  publicClaimLevel: S.Literals(["none", "retained_summary", "validation_summary", "holdout_summary", "live_summary"]),
  promotionStatus: ProbeBenchmarkPromotionStatus,
  reasonRef: S.String,
  requiresExternalGateRefs: S.Array(S.String),
  runtimePromotionAllowed: S.Literal(false),
  schemaRef: S.Literal(PROBE_BENCHMARK_PROMOTION_DECISION_SCHEMA_REF),
});
export type ProbeBenchmarkPromotionDecision = typeof ProbeBenchmarkPromotionDecision.Type;

export class ProbeBenchmarkContractError extends S.TaggedErrorClass<ProbeBenchmarkContractError>()(
  "ProbeBenchmarkContractError",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export function decodeProbeBenchmarkAssignment(
  value: unknown,
): Effect.Effect<ProbeBenchmarkAssignment, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "benchmarkAssignment");
    const assignment = yield* decodeBenchmarkSchema(ProbeBenchmarkAssignment, value, "benchmarkAssignment");
    yield* validateProbeBenchmarkAssignment(assignment);
    return assignment;
  });
}

export function decodeProbeBenchmarkRun(
  value: unknown,
): Effect.Effect<ProbeBenchmarkRun, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "benchmarkRun");
    return yield* decodeBenchmarkSchema(ProbeBenchmarkRun, value, "benchmarkRun");
  });
}

export function decodeProbeBenchmarkCloseout(
  value: unknown,
): Effect.Effect<ProbeBenchmarkCloseout, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "benchmarkCloseout");
    const closeout = yield* decodeBenchmarkSchema(ProbeBenchmarkCloseout, value, "benchmarkCloseout");
    yield* validateProbeBenchmarkCloseout(closeout);
    return closeout;
  });
}

export function decodeProbeBenchmarkDecisionTrace(
  value: unknown,
): Effect.Effect<ProbeBenchmarkDecisionTrace, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "benchmarkDecisionTrace");
    return yield* decodeBenchmarkSchema(ProbeBenchmarkDecisionTrace, value, "benchmarkDecisionTrace");
  });
}

export function decodeProbePromptCandidate(
  value: unknown,
): Effect.Effect<ProbePromptCandidate, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "promptCandidate");
    return yield* decodeBenchmarkSchema(ProbePromptCandidate, value, "promptCandidate");
  });
}

export function decodeProbeBlueprintCandidate(
  value: unknown,
): Effect.Effect<ProbeBlueprintCandidate, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "blueprintCandidate");
    return yield* decodeBenchmarkSchema(ProbeBlueprintCandidate, value, "blueprintCandidate");
  });
}

export function decodeProbeToolMenuCandidate(
  value: unknown,
): Effect.Effect<ProbeToolMenuCandidate, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "toolMenuCandidate");
    return yield* decodeBenchmarkSchema(ProbeToolMenuCandidate, value, "toolMenuCandidate");
  });
}

export function decodeProbeLoopPolicyCandidate(
  value: unknown,
): Effect.Effect<ProbeLoopPolicyCandidate, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "loopPolicyCandidate");
    return yield* decodeBenchmarkSchema(ProbeLoopPolicyCandidate, value, "loopPolicyCandidate");
  });
}

export function decodeProbeBenchmarkPromotionDecision(
  value: unknown,
): Effect.Effect<ProbeBenchmarkPromotionDecision, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "benchmarkPromotionDecision");
    return yield* decodeBenchmarkSchema(ProbeBenchmarkPromotionDecision, value, "benchmarkPromotionDecision");
  });
}

export function decodeProbeBenchmarkRouteScorecard(
  value: unknown,
): Effect.Effect<ProbeBenchmarkRouteScorecard, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "benchmarkRouteScorecard");
    const scorecard = yield* decodeBenchmarkSchema(ProbeBenchmarkRouteScorecard, value, "benchmarkRouteScorecard");
    yield* validateProbeBenchmarkRouteScorecard(scorecard);
    return scorecard;
  });
}

export function validateProbeBenchmarkPublicProjection(
  value: unknown,
  path = "benchmark",
): Effect.Effect<void, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbePublicProjection(value, path);
    yield* validateBenchmarkProjection(value, path);
  });
}

export function sanitizeProbeBenchmarkProjection<T extends JsonValue>(value: T): T {
  return sanitizeBenchmarkJsonValue(sanitizeProbePublicProjection(value)) as T;
}

export function validateProbeBenchmarkAssignment(
  assignment: ProbeBenchmarkAssignment,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireTaskRef(assignment.task, "benchmarkAssignment.task");
    yield* requireNonEmptyRefs(assignment.requiredArtifacts.artifactRefs, "benchmarkAssignment.requiredArtifacts.artifactRefs");
    yield* requireNonEmptyRefs(
      assignment.requiredArtifacts.proofBundleRefs,
      "benchmarkAssignment.requiredArtifacts.proofBundleRefs",
    );
    yield* requireNonEmptyRefs(assignment.sinks.proofSinkRefs, "benchmarkAssignment.sinks.proofSinkRefs");
    yield* requireNonEmptyRefs(
      assignment.selectedBlueprintSignatureRefs,
      "benchmarkAssignment.selectedBlueprintSignatureRefs",
    );
  });
}

export function validateProbeBenchmarkCloseout(
  closeout: ProbeBenchmarkCloseout,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmptyRefs(closeout.artifactManifestRefs, "benchmarkCloseout.artifactManifestRefs");
    yield* requireNonEmptyRefs(closeout.proofBundleRefs, "benchmarkCloseout.proofBundleRefs");
    yield* requireNonEmptyRefs(closeout.selectedSignatureRefs, "benchmarkCloseout.selectedSignatureRefs");

    if (closeout.runStatus !== "succeeded" && closeout.failureClassification.family === "none") {
      return yield* Effect.fail(
        new ProbeBenchmarkContractError({
          path: "benchmarkCloseout.failureClassification.family",
          reason: "non-successful benchmark closeouts require a failure family",
        }),
      );
    }

    if (closeout.evidenceSplit === "retained" && closeout.runStatus !== "succeeded") {
      yield* requireNonEmptyRefs(closeout.retainedFailureRefs, "benchmarkCloseout.retainedFailureRefs");
    }
  });
}

export function validateProbeBenchmarkRouteScorecard(
  scorecard: ProbeBenchmarkRouteScorecard,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmptyRefs(scorecard.selectedSignatureRefs, "benchmarkRouteScorecard.selectedSignatureRefs");
    yield* requireNonEmptyRefs([scorecard.selectedAgentOrModelRef], "benchmarkRouteScorecard.selectedAgentOrModelRef");
    yield* requireNonEmptyRefs([scorecard.selectedRunnerRef], "benchmarkRouteScorecard.selectedRunnerRef");
    yield* requireNonEmptyRefs([scorecard.selectedProviderRef], "benchmarkRouteScorecard.selectedProviderRef");
    yield* requireNonEmptyRefs(
      [scorecard.selectedIsolationProfileRef],
      "benchmarkRouteScorecard.selectedIsolationProfileRef",
    );
    yield* requireNonEmptyRefs([scorecard.selectedVerifierRef], "benchmarkRouteScorecard.selectedVerifierRef");
    yield* requireNonEmptyRefs([scorecard.toolMenuRef], "benchmarkRouteScorecard.toolMenuRef");
    yield* requireNonEmptyRefs([scorecard.candidateHash], "benchmarkRouteScorecard.candidateHash");
    yield* requireNonEmptyRefs([scorecard.routeReasonRef], "benchmarkRouteScorecard.routeReasonRef");

    if (
      scorecard.expectedLatencyMs < 0 ||
      scorecard.observedLatencyMs < 0 ||
      scorecard.postCloseoutRouteScoreBps < 0 ||
      scorecard.postCloseoutRouteScoreBps > 10_000
    ) {
      return yield* Effect.fail(
        new ProbeBenchmarkContractError({
          path: "benchmarkRouteScorecard.postCloseoutRouteScoreBps",
          reason: "route scorecard latency and score values must be bounded non-negative public measurements",
        }),
      );
    }

    for (const [index, rejectedRoute] of scorecard.rejectedRoutes.entries()) {
      yield* requireNonEmptyRefs([rejectedRoute.routeRef], `benchmarkRouteScorecard.rejectedRoutes[${index}].routeRef`);
      yield* requireNonEmptyRefs(
        [rejectedRoute.reasonRef],
        `benchmarkRouteScorecard.rejectedRoutes[${index}].reasonRef`,
      );
    }
  });
}

function decodeBenchmarkSchema<A, I>(
  schema: S.Codec<A, I>,
  value: unknown,
  path: string,
): Effect.Effect<A, ProbeBenchmarkContractError> {
  return S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(
      (error) =>
        new ProbeBenchmarkContractError({
          path,
          reason: String(error),
        }),
    ),
  );
}

function validateBenchmarkProjection(value: unknown, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (value === null || value === undefined) {
    return Effect.void;
  }

  if (typeof value === "string") {
    return unsafeBenchmarkStringReason(value) === undefined
      ? Effect.void
      : Effect.fail(new ProbeBenchmarkContractError({ path, reason: unsafeBenchmarkStringReason(value) ?? "" }));
  }

  if (Array.isArray(value)) {
    return Effect.all(value.map((entry, index) => validateBenchmarkProjection(entry, `${path}[${index}]`))).pipe(
      Effect.asVoid,
    );
  }

  if (typeof value !== "object") {
    return Effect.void;
  }

  return Effect.gen(function* () {
    for (const [key, entry] of Object.entries(value)) {
      const childPath = `${path}.${key}`;
      const unsafeKeyReason = unsafeBenchmarkKeyReason(key);

      if (unsafeKeyReason !== undefined) {
        return yield* Effect.fail(new ProbeBenchmarkContractError({ path: childPath, reason: unsafeKeyReason }));
      }

      yield* validateBenchmarkProjection(entry, childPath);
    }
  });
}

function requireTaskRef(task: ProbeBenchmarkTaskRef, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return task.taskChecksum !== undefined || task.taskRef !== undefined
    ? Effect.void
    : Effect.fail(
        new ProbeBenchmarkContractError({
          path,
          reason: "must include a public-safe task checksum or task ref",
        }),
      );
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  const blankRef = refs.find((ref) => ref.trim().length === 0);

  if (refs.length > 0 && blankRef === undefined) {
    return Effect.void;
  }

  return Effect.fail(
    new ProbeBenchmarkContractError({
      path,
      reason: "must include at least one non-empty public ref",
    }),
  );
}

function sanitizeBenchmarkJsonValue(value: JsonValue): JsonValue {
  if (typeof value === "string") {
    return unsafeBenchmarkStringReason(value) === undefined ? value : "[redacted]";
  }

  if (Array.isArray(value)) {
    return value.map((entry) => sanitizeBenchmarkJsonValue(entry));
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  const sanitized: Record<string, JsonValue> = {};

  for (const [key, entry] of Object.entries(value)) {
    if (unsafeBenchmarkKeyReason(key) !== undefined) {
      continue;
    }

    sanitized[key] = sanitizeBenchmarkJsonValue(entry);
  }

  return sanitized;
}

function unsafeBenchmarkKeyReason(key: string): string | undefined {
  const normalized = key.replace(/[_-]/g, "").toLowerCase();

  if (normalized.includes("rawprompt")) {
    return "raw prompt material is not allowed in public benchmark records";
  }

  if (normalized.includes("rawlog") || normalized.includes("shelllog") || normalized === "stdout" || normalized === "stderr") {
    return "unbounded raw logs are not allowed in public benchmark records";
  }

  if (normalized.includes("hiddenverifier") || normalized.includes("verifiercontent")) {
    return "hidden verifier content is not allowed in public benchmark records";
  }

  if (normalized.includes("benchmarksecret")) {
    return "raw benchmark secrets are not allowed in public benchmark records";
  }

  if (normalized.includes("wallet") || normalized.includes("paymentmaterial") || normalized.includes("mnemonic")) {
    return "wallet or payment material is not allowed in public benchmark records";
  }

  if (normalized.includes("privaterepo") || normalized.includes("privaterepository")) {
    return "private repository refs are not allowed in public benchmark records";
  }

  if (normalized.includes("publicclaimupgrade") || normalized.includes("runtimepromotionauthority")) {
    return "benchmark records cannot carry public claim or runtime promotion authority";
  }

  return undefined;
}

function unsafeBenchmarkStringReason(value: string): string | undefined {
  if (/private-repo:\/\//i.test(value) || /\bgit@github\.com:/i.test(value)) {
    return "private repository refs are not allowed in public benchmark records";
  }

  if (/\b(hidden[_ -]?verifier|benchmark[_ -]?secret|wallet[_ -]?mnemonic|payment[_ -]?preimage)\b/i.test(value)) {
    return "unsafe benchmark control material is not allowed in public benchmark records";
  }

  if (/\b(raw[_ -]?access[_ -]?token|access[_ -]?token|provider[_ -]?secret|bearer|sk-[a-z0-9])\b/i.test(value)) {
    return "provider credential material is not allowed in public benchmark records";
  }

  return undefined;
}
