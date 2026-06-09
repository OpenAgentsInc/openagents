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

export const BlueprintContractConsumer = S.Literals([
  "ai_agent",
  "nexus",
  "oa_node",
  "oa_workroomd",
  "probe",
  "psionic",
  "pylon",
  "treasury",
]);
export type BlueprintContractConsumer = typeof BlueprintContractConsumer.Type;

export const BlueprintContractStability = S.Literals(["seed", "stable"]);
export type BlueprintContractStability = typeof BlueprintContractStability.Type;

export const BlueprintContractPrivacyPolicy = S.Literals(["public_refs_only", "operator_refs_only"]);
export type BlueprintContractPrivacyPolicy = typeof BlueprintContractPrivacyPolicy.Type;

export const BlueprintJsonSchemaContract = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  id: S.String,
  jsonSchemaUrl: S.String,
  name: S.String,
  openApiComponentRef: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  schemaRef: S.String,
  stability: BlueprintContractStability,
  versionRef: S.String,
});
export type BlueprintJsonSchemaContract = typeof BlueprintJsonSchemaContract.Type;

export const BlueprintOpenApiContract = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  id: S.String,
  method: S.String,
  operationRef: S.String,
  path: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  requestSchemaRef: S.NullOr(S.String),
  responseSchemaRef: S.String,
  stability: BlueprintContractStability,
});
export type BlueprintOpenApiContract = typeof BlueprintOpenApiContract.Type;

export const BlueprintEventCatalogEntry = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  eventRef: S.String,
  id: S.String,
  payloadSchemaRef: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  receiptRefs: S.Array(S.String),
  stability: BlueprintContractStability,
  topicRef: S.String,
});
export type BlueprintEventCatalogEntry = typeof BlueprintEventCatalogEntry.Type;

export const BlueprintReceiptCatalogEntry = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  evidenceSchemaRef: S.String,
  id: S.String,
  privacyPolicy: BlueprintContractPrivacyPolicy,
  receiptRef: S.String,
  retentionPolicyRef: S.String,
  stability: BlueprintContractStability,
});
export type BlueprintReceiptCatalogEntry = typeof BlueprintReceiptCatalogEntry.Type;

export const BlueprintContractExportSeed = S.Struct({
  consumers: S.Array(BlueprintContractConsumer),
  eventCatalog: S.Array(BlueprintEventCatalogEntry),
  id: S.String,
  jsonSchemas: S.Array(BlueprintJsonSchemaContract),
  openApi: S.Array(BlueprintOpenApiContract),
  receiptCatalog: S.Array(BlueprintReceiptCatalogEntry),
  versionRef: S.String,
});
export type BlueprintContractExportSeed = typeof BlueprintContractExportSeed.Type;

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

export class BlueprintProjectionUnsafe extends S.TaggedErrorClass<BlueprintProjectionUnsafe>()("BlueprintProjectionUnsafe", {
  path: S.String,
  reason: S.String,
}) {}

const PRIVATE_FIELD_PATTERN =
  /(^|[._-])(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|password|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_webhook|refresh_token|runner_log|secret|source_archive|token|wallet|xprv)([._-]|$)/i;

const PRIVATE_CAMEL_FIELD_PATTERN =
  /^(accessToken|authorization|bearer|callbackUrl|callbackToken|clientSecret|customerEmail|customerName|idToken|invoice|mnemonic|oauth|password|paymentHash|paymentId|paymentPreimage|payoutAddress|payoutDestination|payoutTarget|preimage|privateKey|privateRepo|providerGrant|providerPayload|providerToken|rawEmail|rawPayload|rawPrompt|rawRunLog|rawRunner|rawSourceArchive|rawWebhook|refreshToken|runnerLog|secret|sourceArchive|token|wallet|xprv)$/i;

const PRIVATE_VALUE_PATTERN =
  /\b(access_token|authorization|bearer|callback_url|callback_token|client_secret|customer_email|customer_name|id_token|invoice|mnemonic|oauth|payment_hash|payment_id|payment_preimage|payout_address|payout_destination|payout_target|preimage|private_key|private_repo|provider_grant|provider_payload|provider_token|raw_email|raw_payload|raw_prompt|raw_run_log|raw_runner|raw_source_archive|raw_webhook|refresh_token|runner_log|source_archive|wallet|xprv)\b/i;

type BlueprintJsonPrimitive = string | number | boolean | null;
type BlueprintJsonValue =
  | BlueprintJsonPrimitive
  | ReadonlyArray<BlueprintJsonValue>
  | { readonly [key: string]: BlueprintJsonValue };

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

export function blueprintContractExportSeedIsPrivateDataSafe(seed: BlueprintContractExportSeed): boolean {
  return isBlueprintProjectionPrivateDataSafe(seed);
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

export function isBlueprintProjectionPrivateDataSafe(value: unknown): boolean {
  if (value === null || value === undefined) {
    return true;
  }

  if (typeof value === "string") {
    return !PRIVATE_VALUE_PATTERN.test(value);
  }

  if (Array.isArray(value)) {
    return value.every(isBlueprintProjectionPrivateDataSafe);
  }

  if (typeof value !== "object") {
    return true;
  }

  for (const [key, child] of Object.entries(value)) {
    if (blueprintPrivateFieldKey(key) || !isBlueprintProjectionPrivateDataSafe(child)) {
      return false;
    }
  }

  return true;
}

export function sanitizeBlueprintProjection<T extends BlueprintJsonValue>(value: T): T {
  return sanitizeBlueprintJsonValue(value) as T;
}

function sanitizeBlueprintJsonValue(value: BlueprintJsonValue): BlueprintJsonValue {
  if (typeof value === "string") {
    return PRIVATE_VALUE_PATTERN.test(value) ? "[redacted]" : value;
  }

  if (Array.isArray(value)) {
    return value.map((entry) => sanitizeBlueprintJsonValue(entry));
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  const sanitized: Record<string, BlueprintJsonValue> = {};

  for (const [key, child] of Object.entries(value)) {
    if (blueprintPrivateFieldKey(key)) {
      continue;
    }

    sanitized[key] = sanitizeBlueprintJsonValue(child);
  }

  return sanitized;
}

function blueprintPrivateFieldKey(key: string): boolean {
  return PRIVATE_FIELD_PATTERN.test(key) || PRIVATE_CAMEL_FIELD_PATTERN.test(key);
}
