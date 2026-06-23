import { Effect, Schema as S } from "effect";

export const BlueprintProgramFamily = S.Literals([
  "action_planning",
  "artifact_review",
  "context",
  "continuation",
  "email_decisioning",
  "proof_projection",
  "research_policy",
  "review",
  "routing",
  "source_selection",
]);
export type BlueprintProgramFamily = typeof BlueprintProgramFamily.Type;

export const BlueprintProgramStatus = S.Literals(["draft", "active", "suspended", "deprecated", "archived"]);
export type BlueprintProgramStatus = typeof BlueprintProgramStatus.Type;

export const BlueprintProgramRiskClass = S.Literals(["low", "medium", "high", "legal_sensitive", "payment_sensitive"]);
export type BlueprintProgramRiskClass = typeof BlueprintProgramRiskClass.Type;

export const BlueprintSchemaRefKind = S.Literals(["input", "output", "context", "receipt"]);
export type BlueprintSchemaRefKind = typeof BlueprintSchemaRefKind.Type;

export const BlueprintUnknownFieldPolicy = S.Literals(["reject", "strip", "preserve"]);
export type BlueprintUnknownFieldPolicy = typeof BlueprintUnknownFieldPolicy.Type;

export const BlueprintValidationMode = S.Literals(["strict", "compatible", "advisory"]);
export type BlueprintValidationMode = typeof BlueprintValidationMode.Type;

export const BlueprintEvidenceRequirementKind = S.Literals([
  "artifact_ref",
  "context_pack_ref",
  "customer_feedback_ref",
  "human_review_ref",
  "refusal_reason_ref",
  "source_ref",
  "test_result_ref",
]);
export type BlueprintEvidenceRequirementKind = typeof BlueprintEvidenceRequirementKind.Type;

export const BlueprintReceiptRequirementKind = S.Literals([
  "action_submission",
  "deployment",
  "email",
  "program_run",
  "proof_bundle",
  "pull_request",
  "review",
]);
export type BlueprintReceiptRequirementKind = typeof BlueprintReceiptRequirementKind.Type;

export const BlueprintToolAccess = S.Literals(["read", "evidence", "propose_action"]);
export type BlueprintToolAccess = typeof BlueprintToolAccess.Type;

export const BlueprintTassadarModuleStepKind = S.Literals(["dense_weight_module", "linked_dense_module"]);
export type BlueprintTassadarModuleStepKind = typeof BlueprintTassadarModuleStepKind.Type;

export const BlueprintTassadarModuleStepExecutionMode = S.Literals(["fixture_bound", "registry_resolved"]);
export type BlueprintTassadarModuleStepExecutionMode = typeof BlueprintTassadarModuleStepExecutionMode.Type;

export const BlueprintTassadarModuleStepBinding = S.Struct({
  executionMode: BlueprintTassadarModuleStepExecutionMode,
  expectedCapabilityRef: S.String,
  expectedClaimClass: S.String,
  expectedModuleDigest: S.String,
  expectedTraceDigest: S.String,
  expectedTrustPosture: S.String,
  kind: S.Literal("tassadar_module_step"),
  moduleKind: BlueprintTassadarModuleStepKind,
  moduleRef: S.String,
  registryRef: S.String,
  stepRef: S.String,
});
export type BlueprintTassadarModuleStepBinding = typeof BlueprintTassadarModuleStepBinding.Type;

export const BlueprintReplayModuleBinding = S.Struct({
  allowedReplaySlugs: S.Array(S.String),
  defaultReplaySlug: S.String,
  expectedRuntimeRef: S.String,
  kind: S.Literal("replay_module"),
  moduleRef: S.String,
  stepRef: S.String,
});
export type BlueprintReplayModuleBinding = typeof BlueprintReplayModuleBinding.Type;

export const BlueprintObjectiveSurface = S.Literals([
  "agent_api",
  "customer_dashboard",
  "email",
  "github_pull_request",
  "omni_workroom",
  "operator_dashboard",
  "public_site",
  "pylon_desktop",
]);
export type BlueprintObjectiveSurface = typeof BlueprintObjectiveSurface.Type;

export const BlueprintObjectiveReleaseGate = S.Struct({
  evidenceRefs: S.Array(S.String),
  gateKind: S.Literals([
    "build_passed",
    "customer_review",
    "deployment_live",
    "email_sent",
    "operator_review",
    "privacy_review",
    "proof_bundle_ready",
    "security_review",
    "source_exported",
    "tests_passed",
  ]),
  gateRef: S.String,
  required: S.Boolean,
});
export type BlueprintObjectiveReleaseGate = typeof BlueprintObjectiveReleaseGate.Type;

export const BlueprintProgramSchemaRef = S.Struct({
  kind: BlueprintSchemaRefKind,
  schemaRef: S.String,
  versionRef: S.String,
});
export type BlueprintProgramSchemaRef = typeof BlueprintProgramSchemaRef.Type;

export const BlueprintProgramDecodePolicy = S.Struct({
  validationMode: BlueprintValidationMode,
  validationPolicyRef: S.String,
  unknownFieldPolicy: BlueprintUnknownFieldPolicy,
});
export type BlueprintProgramDecodePolicy = typeof BlueprintProgramDecodePolicy.Type;

export const BlueprintProgramEvidenceRequirement = S.Struct({
  descriptionRef: S.String,
  kind: BlueprintEvidenceRequirementKind,
  minimumCount: S.Number,
  required: S.Boolean,
});
export type BlueprintProgramEvidenceRequirement = typeof BlueprintProgramEvidenceRequirement.Type;

export const BlueprintProgramReceiptRequirement = S.Struct({
  kind: BlueprintReceiptRequirementKind,
  receiptRef: S.String,
  required: S.Boolean,
});
export type BlueprintProgramReceiptRequirement = typeof BlueprintProgramReceiptRequirement.Type;

export const BlueprintProgramToolScope = S.Struct({
  access: BlueprintToolAccess,
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  requiresApproval: S.Boolean,
  replayModule: S.optional(BlueprintReplayModuleBinding),
  tassadarModuleStep: S.optional(BlueprintTassadarModuleStepBinding),
  toolRef: S.String,
});
export type BlueprintProgramToolScope = typeof BlueprintProgramToolScope.Type;

export const BlueprintProgramType = S.Struct({
  allowedStrategyRefs: S.Array(S.String),
  directMutationAllowed: S.Boolean,
  evidenceRequirements: S.Array(BlueprintProgramEvidenceRequirement),
  family: BlueprintProgramFamily,
  id: S.String,
  instructionRefs: S.Array(S.String),
  instructionsVersionRef: S.String,
  purposeRef: S.String,
  receiptRequirements: S.Array(BlueprintProgramReceiptRequirement),
  releaseGates: S.Array(BlueprintObjectiveReleaseGate),
  riskClass: BlueprintProgramRiskClass,
  status: BlueprintProgramStatus,
  toolScopes: S.Array(BlueprintProgramToolScope),
});
export type BlueprintProgramType = typeof BlueprintProgramType.Type;

export const BlueprintProgramSignature = S.Struct({
  decodePolicy: BlueprintProgramDecodePolicy,
  evidenceRequirements: S.Array(BlueprintProgramEvidenceRequirement),
  id: S.String,
  inputSchema: BlueprintProgramSchemaRef,
  outputSchema: BlueprintProgramSchemaRef,
  programTypeId: S.String,
  receiptRequirements: S.Array(BlueprintProgramReceiptRequirement),
  status: BlueprintProgramStatus,
  supportsContext: S.Boolean,
  supportsContinuation: S.Boolean,
  supportsProofProjection: S.Boolean,
  supportsReview: S.Boolean,
  supportsRouting: S.Boolean,
  toolScopes: S.Array(BlueprintProgramToolScope),
  versionRef: S.String,
});
export type BlueprintProgramSignature = typeof BlueprintProgramSignature.Type;

export const BlueprintModuleKind = S.Literals([
  "deterministic_reducer",
  "effect_agent_module",
  "human_review_module",
  "model_prompt",
  "optimizer_candidate",
  "runtime_adapter",
]);
export type BlueprintModuleKind = typeof BlueprintModuleKind.Type;

export const BlueprintModuleLifecycleStatus = S.Literals([
  "draft",
  "candidate",
  "approved",
  "promoted",
  "rolled_back",
  "deprecated",
  "archived",
]);
export type BlueprintModuleLifecycleStatus = typeof BlueprintModuleLifecycleStatus.Type;

export const BlueprintModuleReleaseState = S.Literals([
  "unpromoted",
  "release_candidate",
  "production",
  "rolled_back",
  "deprecated",
]);
export type BlueprintModuleReleaseState = typeof BlueprintModuleReleaseState.Type;

export const BlueprintModuleVersionRef = S.Struct({
  id: S.String,
  implementationRef: S.String,
  moduleKind: BlueprintModuleKind,
  moduleRef: S.String,
  programSignatureId: S.NullOr(S.String),
  programTypeId: S.String,
  releaseState: BlueprintModuleReleaseState,
  status: BlueprintModuleLifecycleStatus,
  versionRef: S.String,
});
export type BlueprintModuleVersionRef = typeof BlueprintModuleVersionRef.Type;

export const BlueprintReleaseTargetKind = S.Literals([
  "email_policy",
  "module_version",
  "program_signature",
  "proof_projection",
  "route_selector",
]);
export type BlueprintReleaseTargetKind = typeof BlueprintReleaseTargetKind.Type;

export const BlueprintReleaseGateDecision = S.Literals(["approved", "blocked", "rejected"]);
export type BlueprintReleaseGateDecision = typeof BlueprintReleaseGateDecision.Type;

export const BlueprintReleaseGateState = S.Literals(["blocked", "draft", "failed", "passed"]);
export type BlueprintReleaseGateState = typeof BlueprintReleaseGateState.Type;

export const BlueprintReleaseReviewState = S.Literals(["approved", "not_requested", "pending", "rejected"]);
export type BlueprintReleaseReviewState = typeof BlueprintReleaseReviewState.Type;

export const BlueprintReleasePolicyState = S.Literals(["blocked", "compliant", "not_checked"]);
export type BlueprintReleasePolicyState = typeof BlueprintReleasePolicyState.Type;

export const BlueprintRollbackPosture = S.Literals(["missing", "ready", "verified"]);
export type BlueprintRollbackPosture = typeof BlueprintRollbackPosture.Type;

export const BlueprintReleaseGateRef = S.Struct({
  decidedByRef: S.NullOr(S.String),
  decision: S.NullOr(BlueprintReleaseGateDecision),
  decisionReasonRef: S.NullOr(S.String),
  fixturePassState: BlueprintReleaseGateState,
  fixtureRefs: S.Array(S.String),
  id: S.String,
  policyState: BlueprintReleasePolicyState,
  receiptRefs: S.Array(S.String),
  reviewState: BlueprintReleaseReviewState,
  rollbackPosture: BlueprintRollbackPosture,
  scorecardRef: S.NullOr(S.String),
  selfPromotionAttempt: S.Boolean,
  targetKind: BlueprintReleaseTargetKind,
  targetRef: S.String,
});
export type BlueprintReleaseGateRef = typeof BlueprintReleaseGateRef.Type;

export const BlueprintProgramPromotionState = S.Literals([
  "blocked",
  "candidate",
  "deprecated",
  "draft",
  "production",
  "promotable",
  "review_pending",
  "rolled_back",
]);
export type BlueprintProgramPromotionState = typeof BlueprintProgramPromotionState.Type;

export const BlueprintProgramRunAuthorityBoundary = S.Literals(["evidence_only"]);
export type BlueprintProgramRunAuthorityBoundary = typeof BlueprintProgramRunAuthorityBoundary.Type;

export const BlueprintProgramRunEvidenceFlags = S.Struct({
  authorityBoundary: BlueprintProgramRunAuthorityBoundary,
  directMutationDisabled: S.Boolean,
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
});
export type BlueprintProgramRunEvidenceFlags = typeof BlueprintProgramRunEvidenceFlags.Type;

export const BlueprintProgramRunDetailProjection = S.Struct({
  actorRef: S.String,
  authorityBoundary: BlueprintProgramRunAuthorityBoundary,
  confidence: S.Number,
  costRef: S.String,
  createdAt: S.String,
  directMutationDisabled: S.Boolean,
  evidenceRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  id: S.String,
  latencyMs: S.Number,
  moduleVersionId: S.String,
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
  programSignatureId: S.String,
  programTypeId: S.String,
  promotionState: BlueprintProgramPromotionState,
  purposeRef: S.String,
  receiptRefs: S.Array(S.String),
  routeRef: S.String,
  safeProjection: S.Boolean,
  updatedAt: S.String,
});
export type BlueprintProgramRunDetailProjection = typeof BlueprintProgramRunDetailProjection.Type;

export const BlueprintTassadarModuleStepVerdict = S.Literals(["verified", "rejected"]);
export type BlueprintTassadarModuleStepVerdict = typeof BlueprintTassadarModuleStepVerdict.Type;

export const BlueprintTassadarModuleStepEvidence = S.Struct({
  authorityBoundary: S.Literal("evidence_only"),
  blockerRefs: S.Array(S.String),
  capabilityRef: S.String,
  claimClass: S.String,
  contentRedacted: S.Literal(true),
  directMutationDisabled: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  expectedModuleDigest: S.String,
  expectedTraceDigest: S.String,
  kind: S.Literal("blueprint_tassadar_module_step_evidence"),
  moduleDigest: S.String,
  moduleKind: BlueprintTassadarModuleStepKind,
  moduleRef: S.String,
  noDeploy: S.Literal(true),
  noEmail: S.Literal(true),
  noSourceMutation: S.Literal(true),
  noSpend: S.Literal(true),
  observedAt: S.String,
  receiptRefs: S.Array(S.String),
  registryRef: S.String,
  replayedTraceDigest: S.NullOr(S.String),
  result: S.Record(S.String, S.Unknown),
  stepRef: S.String,
  toolRef: S.String,
  trustPosture: S.String,
  verdict: BlueprintTassadarModuleStepVerdict,
});
export type BlueprintTassadarModuleStepEvidence = typeof BlueprintTassadarModuleStepEvidence.Type;

export const BlueprintReplayModuleViewSpec = S.Struct({
  bundleEndpoint: S.String,
  bundleRef: S.String,
  replaySlug: S.String,
  socialPath: S.optional(S.String),
  websitePath: S.String,
});
export type BlueprintReplayModuleViewSpec = typeof BlueprintReplayModuleViewSpec.Type;

export const BlueprintReplayModuleEvidence = S.Struct({
  authorityBoundary: S.Literal("evidence_only"),
  bundle: S.Record(S.String, S.Unknown),
  bundleEndpoint: S.String,
  bundleRef: S.String,
  contentRedacted: S.Literal(true),
  directMutationDisabled: S.Literal(true),
  evidenceRefs: S.Array(S.String),
  intentRef: S.String,
  kind: S.Literal("blueprint_replay_module_evidence"),
  moduleRef: S.String,
  noDeploy: S.Literal(true),
  noEmail: S.Literal(true),
  noSourceMutation: S.Literal(true),
  noSpend: S.Literal(true),
  observedAt: S.String,
  receiptRefs: S.Array(S.String),
  renderPlan: S.Record(S.String, S.Unknown),
  replaySlug: S.String,
  replayViewSpec: BlueprintReplayModuleViewSpec,
  sourceAuthority: S.String,
  sourceRefs: S.Array(S.String),
  stepRef: S.String,
  summary: S.String,
  targetRef: S.String,
  title: S.String,
  toolRef: S.String,
});
export type BlueprintReplayModuleEvidence = typeof BlueprintReplayModuleEvidence.Type;

export const BlueprintTassadarModuleRegistryEntry = S.Struct({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  capabilityRef: S.String,
  caveatRefs: S.Array(S.String),
  claimBoundary: S.String,
  claimClass: S.String,
  compileReceiptRefs: S.Array(S.String),
  fixtureRef: S.String,
  moduleDigest: S.String,
  moduleId: S.String,
  moduleKind: BlueprintTassadarModuleStepKind,
  moduleRef: S.String,
  publicSafe: S.Literal(true),
  registryVersionRef: S.String,
  traceDigest: S.String,
  trustPosture: S.String,
});
export type BlueprintTassadarModuleRegistryEntry = typeof BlueprintTassadarModuleRegistryEntry.Type;

export const BlueprintTassadarModuleRegistryProjection = S.Struct({
  caveatRefs: S.Array(S.String),
  generatedAt: S.String,
  modules: S.Array(BlueprintTassadarModuleRegistryEntry),
  registryVersionRef: S.String,
  safeProjection: S.Literal(true),
  schemaVersion: S.String,
});
export type BlueprintTassadarModuleRegistryProjection = typeof BlueprintTassadarModuleRegistryProjection.Type;

export const BlueprintProgramRegistryEntry = S.Struct({
  approvalRequired: S.Boolean,
  backendKinds: S.Array(S.String),
  capabilityRefs: S.Array(S.String),
  directMutationAllowed: S.Boolean,
  evidenceRefs: S.Array(S.String),
  failureRefs: S.Array(S.String),
  family: BlueprintProgramFamily,
  id: S.String,
  moduleVersionIds: S.Array(S.String),
  programSignatureIds: S.Array(S.String),
  programTypeId: S.String,
  promotionState: BlueprintProgramPromotionState,
  receiptRefs: S.Array(S.String),
  releaseGateIds: S.Array(S.String),
  riskClass: BlueprintProgramRiskClass,
  runIds: S.Array(S.String),
  safeProjection: S.Boolean,
  status: BlueprintProgramStatus,
});
export type BlueprintProgramRegistryEntry = typeof BlueprintProgramRegistryEntry.Type;

export const BlueprintProgramRegistryProjection = S.Struct({
  entries: S.Array(BlueprintProgramRegistryEntry),
  moduleVersions: S.Array(BlueprintModuleVersionRef),
  policyRef: S.String,
  programSignatures: S.Array(BlueprintProgramSignature),
  programTypes: S.Array(BlueprintProgramType),
  releaseGates: S.Array(BlueprintReleaseGateRef),
  runDetails: S.Array(BlueprintProgramRunDetailProjection),
  safeProjection: S.Boolean,
});
export type BlueprintProgramRegistryProjection = typeof BlueprintProgramRegistryProjection.Type;

// The Blueprint contract-export schema types and the security-critical
// `IsPrivateDataSafe` predicate family are owned by the canonical
// `@openagentsinc/blueprint-contracts` package. Import them for internal use and
// re-export them so this runtime's import sites keep their existing paths while
// there is a single drift-free authority for the security contract. Re-exporting
// (not redefining) these is enforced by scripts/check-contract-drift.mjs.
import {
  BlueprintContractConsumer,
  BlueprintContractExportSeed,
  BlueprintContractPrivacyPolicy,
  BlueprintContractStability,
  BlueprintEventCatalogEntry,
  BlueprintJsonSchemaContract,
  BlueprintOpenApiContract,
  BlueprintProjectionUnsafe,
  BlueprintReceiptCatalogEntry,
  blueprintContractExportSeedIsPrivateDataSafe,
  blueprintPrivateFieldKey,
  isBlueprintProjectionPrivateDataSafe,
  sanitizeBlueprintProjection,
} from "@openagentsinc/blueprint-contracts";

export {
  BlueprintContractConsumer,
  BlueprintContractExportSeed,
  BlueprintContractPrivacyPolicy,
  BlueprintContractStability,
  BlueprintEventCatalogEntry,
  BlueprintJsonSchemaContract,
  BlueprintOpenApiContract,
  BlueprintProjectionUnsafe,
  BlueprintReceiptCatalogEntry,
  blueprintContractExportSeedIsPrivateDataSafe,
  blueprintPrivateFieldKey,
  isBlueprintProjectionPrivateDataSafe,
  sanitizeBlueprintProjection,
};

export const BlueprintRegistrySourceKind = S.Literals(["staticFixture", "assignmentInline", "omegaHttp"]);
export type BlueprintRegistrySourceKind = typeof BlueprintRegistrySourceKind.Type;

export const BlueprintSignatureLookupRequest = S.Struct({
  actorRef: S.String,
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  backendKind: S.optional(S.String),
  contextPackRef: S.optional(S.String),
  objectiveRef: S.optional(S.String),
  preferredFamily: S.optional(BlueprintProgramFamily),
  programSignatureIds: S.optional(S.Array(S.String)),
  programTypeIds: S.optional(S.Array(S.String)),
  registrySource: S.optional(BlueprintRegistrySourceKind),
  riskCeiling: BlueprintProgramRiskClass,
});
export type BlueprintSignatureLookupRequest = typeof BlueprintSignatureLookupRequest.Type;

export const BlueprintSignatureLookupResult = S.Struct({
  entries: S.Array(BlueprintProgramRegistryEntry),
  moduleVersions: S.Array(BlueprintModuleVersionRef),
  programSignatures: S.Array(BlueprintProgramSignature),
  programTypes: S.Array(BlueprintProgramType),
  registryPolicyRef: S.String,
  releaseGates: S.Array(BlueprintReleaseGateRef),
  safeProjection: S.Boolean,
  source: BlueprintRegistrySourceKind,
});
export type BlueprintSignatureLookupResult = typeof BlueprintSignatureLookupResult.Type;

export const ProbeToolMenuTool = S.Struct({
  access: BlueprintToolAccess,
  allowedSurfaces: S.Array(BlueprintObjectiveSurface),
  inputSchemaRef: S.String,
  programSignatureId: S.String,
  requiresApproval: S.Boolean,
  tassadarModuleStep: S.optional(BlueprintTassadarModuleStepBinding),
  toolRef: S.String,
});
export type ProbeToolMenuTool = typeof ProbeToolMenuTool.Type;

export const ProbeToolMenuPlan = S.Struct({
  backendKind: S.String,
  evidenceFlags: BlueprintProgramRunEvidenceFlags,
  programSignatureIds: S.Array(S.String),
  registryPolicyRef: S.String,
  releaseGateIds: S.Array(S.String),
  safeProjection: S.Boolean,
  tools: S.Array(ProbeToolMenuTool),
});
export type ProbeToolMenuPlan = typeof ProbeToolMenuPlan.Type;

export function blueprintProgramRunEvidenceFlagsAreEvidenceOnly(
  flags: BlueprintProgramRunEvidenceFlags,
): boolean {
  return (
    flags.authorityBoundary === "evidence_only" &&
    flags.directMutationDisabled &&
    flags.noDeploy &&
    flags.noEmail &&
    flags.noSourceMutation &&
    flags.noSpend
  );
}

export function blueprintProgramRunDetailProjectionIsEvidenceOnly(
  run: BlueprintProgramRunDetailProjection,
): boolean {
  return blueprintProgramRunEvidenceFlagsAreEvidenceOnly(run);
}

export function blueprintRegistryProjectionIsSafe(projection: BlueprintProgramRegistryProjection): boolean {
  return (
    projection.safeProjection &&
    projection.entries.every((entry) => entry.safeProjection && !entry.directMutationAllowed) &&
    projection.programTypes.every((programType) => !programType.directMutationAllowed) &&
    projection.releaseGates.every((gate) => !gate.selfPromotionAttempt) &&
    projection.runDetails.every(
      (run) => run.safeProjection && blueprintProgramRunDetailProjectionIsEvidenceOnly(run),
    ) &&
    blueprintRegistryProjectionIsPrivateDataSafe(projection)
  );
}

export function blueprintRegistryProjectionIsPrivateDataSafe(projection: BlueprintProgramRegistryProjection): boolean {
  return isBlueprintProjectionPrivateDataSafe(projection);
}

export function validateBlueprintRegistryProjection(
  projection: BlueprintProgramRegistryProjection,
): Effect.Effect<BlueprintProgramRegistryProjection, BlueprintProjectionUnsafe> {
  return Effect.gen(function* () {
    if (!blueprintRegistryProjectionIsSafe(projection)) {
      return yield* Effect.fail(
        new BlueprintProjectionUnsafe({
          path: "projection",
          reason: "registry projection is not safe for Probe consumption",
        }),
      );
    }

    return projection;
  });
}

export function validateBlueprintContractExportSeed(
  seed: BlueprintContractExportSeed,
): Effect.Effect<BlueprintContractExportSeed, BlueprintProjectionUnsafe> {
  return blueprintContractExportSeedIsPrivateDataSafe(seed)
    ? Effect.succeed(seed)
    : Effect.fail(
        new BlueprintProjectionUnsafe({
          path: "contractExport",
          reason: "contract export contains private-data-shaped material",
        }),
      );
}

