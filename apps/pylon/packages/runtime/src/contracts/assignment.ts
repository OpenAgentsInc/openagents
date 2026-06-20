import { Effect, Schema as S } from "effect";
import { APPLE_FM_BACKEND_KIND, PROBE_APPLE_FM_BACKEND_CAPABILITY } from "../backends/apple-fm/contract.js";
import {
  GEMINI_API_PROFILE_ID,
  GEMINI_BACKEND_KIND,
  PROBE_GEMINI_BACKEND_CAPABILITY,
} from "../backends/gemini/contract.js";
import {
  PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY,
  PSIONIC_QWEN_BACKEND_KIND,
  PSIONIC_QWEN_LOCAL_PROFILE_ID,
} from "../backends/psionic-qwen/contract.js";
import {
  BlueprintContractExportSeed,
  BlueprintProgramRegistryProjection,
  isBlueprintProjectionPrivateDataSafe,
  sanitizeBlueprintProjection,
  validateBlueprintContractExportSeed,
  validateBlueprintRegistryProjection,
  type BlueprintProjectionUnsafe,
} from "../blueprint/contracts.js";
import {
  ProbeProvider,
  ProviderAccountRef,
  ProviderAuthGrantRef,
  ProbePublicProjectionUnsafe,
  sanitizeProbePublicProjection,
  validateProbePublicProjection,
  type JsonValue,
} from "./provider-account.js";

export const ProbeRepositoryRef = S.Struct({
  url: S.optional(S.String),
  path: S.optional(S.String),
  branch: S.optional(S.String),
  commit: S.optional(S.String),
});
export type ProbeRepositoryRef = typeof ProbeRepositoryRef.Type;

export const ProbeAppleFmAssignmentBackend = S.Struct({
  kind: S.Literal(APPLE_FM_BACKEND_KIND),
  profile: S.optional(S.String),
});
export type ProbeAppleFmAssignmentBackend = typeof ProbeAppleFmAssignmentBackend.Type;

export const ProbeGeminiAssignmentBackend = S.Struct({
  kind: S.Literal(GEMINI_BACKEND_KIND),
  profile: S.optional(S.String),
  backendProfileId: S.optional(S.String),
});
export type ProbeGeminiAssignmentBackend = typeof ProbeGeminiAssignmentBackend.Type;

export const ProbePsionicQwenAssignmentBackend = S.Struct({
  kind: S.Literal(PSIONIC_QWEN_BACKEND_KIND),
  profile: S.optional(S.String),
  backendProfileId: S.optional(S.String),
});
export type ProbePsionicQwenAssignmentBackend = typeof ProbePsionicQwenAssignmentBackend.Type;

export const ProbeAssignmentBackend = S.Union([
  ProbeAppleFmAssignmentBackend,
  ProbeGeminiAssignmentBackend,
  ProbePsionicQwenAssignmentBackend,
]);
export type ProbeAssignmentBackend = typeof ProbeAssignmentBackend.Type;

export const ProbeBlueprintAssignmentScope = S.Struct({
  actionSubmissionPolicyRef: S.optional(S.String),
  backendCapabilityRefs: S.optional(S.Array(S.String)),
  contextPackRefs: S.optional(S.Array(S.String)),
  contractExport: S.optional(BlueprintContractExportSeed),
  moduleVersionRefs: S.optional(S.Array(S.String)),
  programRunPurposeRef: S.optional(S.String),
  programSignatureRefs: S.optional(S.Array(S.String)),
  programTypeRefs: S.optional(S.Array(S.String)),
  registry: S.optional(BlueprintProgramRegistryProjection),
  registryVersionRef: S.String,
  releaseGateRefs: S.optional(S.Array(S.String)),
  sourceAuthorityRefs: S.optional(S.Array(S.String)),
  toolScopeRefs: S.optional(S.Array(S.String)),
});
export type ProbeBlueprintAssignmentScope = typeof ProbeBlueprintAssignmentScope.Type;

export const ProbeRunAssignment = S.Struct({
  assignmentId: S.String,
  runnerSessionId: S.String,
  goal: S.String,
  runtime: S.optional(S.String),
  backend: S.optional(ProbeAssignmentBackend),
  blueprint: S.optional(ProbeBlueprintAssignmentScope),
  provider: S.optional(ProbeProvider),
  providerAccountRef: S.optional(ProviderAccountRef),
  authGrantRef: S.optional(ProviderAuthGrantRef),
  leaseRef: S.optional(S.String),
  repo: S.optional(ProbeRepositoryRef),
  callbackUrl: S.optional(S.String),
  sandbox: S.optional(S.Record(S.String, S.Unknown)),
});
export type ProbeRunAssignment = typeof ProbeRunAssignment.Type;

export class ProbeAssignmentParseError extends S.TaggedErrorClass<ProbeAssignmentParseError>()(
  "ProbeAssignmentParseError",
  {
    reason: S.String,
  },
) {}

export function decodeProbeRunAssignment(
  value: unknown,
): Effect.Effect<ProbeRunAssignment, ProbeAssignmentParseError | ProbePublicProjectionUnsafe | BlueprintProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeAssignmentProjection(value);
    const assignment = yield* S.decodeUnknownEffect(ProbeRunAssignment)(value).pipe(
      Effect.mapError(
        (error) =>
          new ProbeAssignmentParseError({
            reason: String(error),
          }),
      ),
    );

    yield* validateProbeAssignmentBlueprintScope(assignment);

    return assignment;
  });
}

export function validateProbeAssignmentProjection(
  value: unknown,
): Effect.Effect<void, ProbePublicProjectionUnsafe> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return validateProbePublicProjection(value, "assignment");
  }

  const { blueprint, ...assignmentWithoutBlueprint } = value as Record<string, unknown>;

  return Effect.gen(function* () {
    yield* validateProbePublicProjection(assignmentWithoutBlueprint, "assignment");

    if (blueprint !== undefined) {
      if (!isBlueprintProjectionPrivateDataSafe(blueprint)) {
        return yield* Effect.fail(
          new ProbePublicProjectionUnsafe({
            path: "assignment.blueprint",
            reason: "contains private-data-shaped material",
          }),
        );
      }
    }
  });
}

export function sanitizeProbeRunAssignmentProjection<T extends JsonValue>(value: T): T {
  if (value === null || typeof value !== "object" || Array.isArray(value) || !("blueprint" in value)) {
    return sanitizeProbePublicProjection(value);
  }

  const { blueprint, ...assignmentWithoutBlueprint } = value as { readonly [key: string]: JsonValue };
  const sanitized = sanitizeProbePublicProjection(assignmentWithoutBlueprint);

  return {
    ...sanitized,
    blueprint: blueprint === undefined ? undefined : sanitizeBlueprintProjection(blueprint),
  } as unknown as T;
}

export function validateProbeAssignmentBlueprintScope(
  assignment: ProbeRunAssignment,
): Effect.Effect<void, ProbeAssignmentParseError | BlueprintProjectionUnsafe> {
  const scope = assignment.blueprint;

  if (scope === undefined) {
    return Effect.void;
  }

  return Effect.gen(function* () {
    yield* requireBlueprintRefPrefix(scope.registryVersionRef, "blueprint.registryVersionRef", [
      "blueprint_registry.",
    ]);
    yield* requireOptionalBlueprintRefPrefixes(scope.programTypeRefs, "blueprint.programTypeRefs", ["program_type."]);
    yield* requireOptionalBlueprintRefPrefixes(scope.programSignatureRefs, "blueprint.programSignatureRefs", [
      "program_signature.",
    ]);
    yield* requireOptionalBlueprintRefPrefixes(scope.moduleVersionRefs, "blueprint.moduleVersionRefs", [
      "module_version.",
    ]);
    yield* requireOptionalBlueprintRefPrefixes(scope.contextPackRefs, "blueprint.contextPackRefs", ["context_pack."]);
    yield* requireOptionalBlueprintRefPrefixes(scope.sourceAuthorityRefs, "blueprint.sourceAuthorityRefs", [
      "source_authority.",
    ]);
    yield* requireOptionalBlueprintRefPrefixes(scope.toolScopeRefs, "blueprint.toolScopeRefs", ["tool."]);
    yield* requireOptionalBlueprintRefPrefixes(scope.releaseGateRefs, "blueprint.releaseGateRefs", ["release_gate."]);
    yield* requireOptionalBlueprintRefPrefixes(scope.backendCapabilityRefs, "blueprint.backendCapabilityRefs", [
      "probe.backend.",
    ]);

    if (scope.actionSubmissionPolicyRef !== undefined) {
      yield* requireBlueprintRefPrefix(scope.actionSubmissionPolicyRef, "blueprint.actionSubmissionPolicyRef", [
        "policy.",
      ]);
    }

    if (scope.programRunPurposeRef !== undefined) {
      yield* requireBlueprintRefPrefix(scope.programRunPurposeRef, "blueprint.programRunPurposeRef", ["purpose."]);
    }

    yield* validateBackendCapabilityRefs(assignment);

    if (scope.registry !== undefined) {
      yield* validateBlueprintRegistryProjection(scope.registry);
      yield* requireOptionalRefsInKnownSet(
        scope.programTypeRefs,
        new Set(scope.registry.programTypes.map((programType) => programType.id)),
        "blueprint.programTypeRefs",
      );
      yield* requireOptionalRefsInKnownSet(
        scope.programSignatureRefs,
        new Set(scope.registry.programSignatures.map((signature) => signature.id)),
        "blueprint.programSignatureRefs",
      );
      yield* requireOptionalRefsInKnownSet(
        scope.moduleVersionRefs,
        new Set(scope.registry.moduleVersions.map((moduleVersion) => moduleVersion.id)),
        "blueprint.moduleVersionRefs",
      );
      yield* requireOptionalRefsInKnownSet(
        scope.releaseGateRefs,
        new Set(scope.registry.releaseGates.map((gate) => gate.id)),
        "blueprint.releaseGateRefs",
      );
    }

    if (scope.contractExport !== undefined) {
      yield* validateBlueprintContractExportSeed(scope.contractExport);
    }
  });
}

export function assignmentRequiresProviderGrant(assignment: ProbeRunAssignment): boolean {
  return assignment.provider === "chatgpt_codex" || assignment.providerAccountRef !== undefined || assignment.authGrantRef !== undefined;
}

export function assignmentSelectsAppleFmBackend(
  assignment: ProbeRunAssignment,
): assignment is ProbeRunAssignment & { readonly backend: ProbeAppleFmAssignmentBackend } {
  return assignment.backend?.kind === APPLE_FM_BACKEND_KIND;
}

export function assignmentSelectsGeminiBackend(
  assignment: ProbeRunAssignment,
): assignment is ProbeRunAssignment & { readonly backend: ProbeGeminiAssignmentBackend } {
  return assignment.backend?.kind === GEMINI_BACKEND_KIND;
}

export function assignmentSelectsPsionicQwenBackend(
  assignment: ProbeRunAssignment,
): assignment is ProbeRunAssignment & { readonly backend: ProbePsionicQwenAssignmentBackend } {
  return assignment.backend?.kind === PSIONIC_QWEN_BACKEND_KIND;
}

export function selectedAssignmentBackendProfileId(backend: ProbeAssignmentBackend): string | undefined {
  return backend.kind === GEMINI_BACKEND_KIND || backend.kind === PSIONIC_QWEN_BACKEND_KIND
    ? backend.backendProfileId ?? backend.profile
    : backend.profile;
}

export function assignmentInlineBlueprintRegistrySource(
  assignment: ProbeRunAssignment,
): Readonly<{
  blueprintContractExport?: unknown;
  blueprintRegistry?: unknown;
  blueprintRegistryVersionRef?: string;
}> {
  return {
    blueprintContractExport: assignment.blueprint?.contractExport,
    blueprintRegistry: assignment.blueprint?.registry,
    blueprintRegistryVersionRef: assignment.blueprint?.registryVersionRef,
  };
}

function validateBackendCapabilityRefs(assignment: ProbeRunAssignment): Effect.Effect<void, ProbeAssignmentParseError> {
  const refs = assignment.blueprint?.backendCapabilityRefs ?? [];

  if (refs.length === 0) {
    return Effect.void;
  }

  const allowedRefs: readonly string[] = assignmentSelectsAppleFmBackend(assignment)
    ? [PROBE_APPLE_FM_BACKEND_CAPABILITY]
    : assignmentSelectsGeminiBackend(assignment)
      ? [PROBE_GEMINI_BACKEND_CAPABILITY]
      : assignmentSelectsPsionicQwenBackend(assignment)
        ? [PROBE_PSIONIC_QWEN_BACKEND_CAPABILITY]
        : [];
  const mismatched = refs.filter((ref) => !allowedRefs.includes(ref));

  return mismatched.length === 0
    ? Effect.void
    : Effect.fail(
        new ProbeAssignmentParseError({
          reason: `assignment Blueprint backend capability refs do not match selected backend: ${mismatched.join(", ")}`,
        }),
      );
}

function requireOptionalBlueprintRefPrefixes(
  refs: ReadonlyArray<string> | undefined,
  path: string,
  prefixes: ReadonlyArray<string>,
): Effect.Effect<void, ProbeAssignmentParseError> {
  return Effect.forEach(refs ?? [], (ref, index) =>
    requireBlueprintRefPrefix(ref, `${path}[${index}]`, prefixes),
  ).pipe(Effect.asVoid);
}

function requireBlueprintRefPrefix(
  ref: string,
  path: string,
  prefixes: ReadonlyArray<string>,
): Effect.Effect<void, ProbeAssignmentParseError> {
  return prefixes.some((prefix) => ref.startsWith(prefix))
    ? Effect.void
    : Effect.fail(
        new ProbeAssignmentParseError({
          reason: `${path} must use one of these public ref prefixes: ${prefixes.join(", ")}`,
        }),
      );
}

function requireOptionalRefsInKnownSet(
  refs: ReadonlyArray<string> | undefined,
  knownRefs: ReadonlySet<string>,
  path: string,
): Effect.Effect<void, ProbeAssignmentParseError> {
  const missing = (refs ?? []).filter((ref) => !knownRefs.has(ref));

  return missing.length === 0
    ? Effect.void
    : Effect.fail(
        new ProbeAssignmentParseError({
          reason: `${path} includes refs outside the inline Blueprint registry slice: ${missing.join(", ")}`,
        }),
      );
}

export function requireAppleFmAssignmentBackend(
  assignment: ProbeRunAssignment,
): Effect.Effect<ProbeAppleFmAssignmentBackend, ProbeAssignmentParseError> {
  return assignmentSelectsAppleFmBackend(assignment)
    ? Effect.succeed(assignment.backend)
    : Effect.fail(new ProbeAssignmentParseError({ reason: "assignment is not selecting apple_fm_bridge" }));
}

export function requireGeminiAssignmentBackend(
  assignment: ProbeRunAssignment,
): Effect.Effect<ProbeGeminiAssignmentBackend, ProbeAssignmentParseError> {
  return assignmentSelectsGeminiBackend(assignment)
    ? Effect.succeed({
        ...assignment.backend,
        backendProfileId: assignment.backend.backendProfileId ?? assignment.backend.profile ?? GEMINI_API_PROFILE_ID,
      })
    : Effect.fail(new ProbeAssignmentParseError({ reason: "assignment is not selecting gemini_api" }));
}

export function requirePsionicQwenAssignmentBackend(
  assignment: ProbeRunAssignment,
): Effect.Effect<ProbePsionicQwenAssignmentBackend, ProbeAssignmentParseError> {
  return assignmentSelectsPsionicQwenBackend(assignment)
    ? Effect.succeed({
        ...assignment.backend,
        backendProfileId: assignment.backend.backendProfileId ?? assignment.backend.profile ?? PSIONIC_QWEN_LOCAL_PROFILE_ID,
      })
    : Effect.fail(new ProbeAssignmentParseError({ reason: "assignment is not selecting psionic_qwen35" }));
}

export function requireAssignmentGrantRefs(
  assignment: ProbeRunAssignment,
): Effect.Effect<
  ProbeRunAssignment & {
    readonly provider: ProbeProvider;
    readonly providerAccountRef: ProviderAccountRef;
    readonly authGrantRef: ProviderAuthGrantRef;
  },
  ProbeAssignmentParseError
> {
  if (assignment.provider === undefined) {
    return Effect.fail(new ProbeAssignmentParseError({ reason: "assignment is missing provider" }));
  }

  if (assignment.providerAccountRef === undefined) {
    return Effect.fail(new ProbeAssignmentParseError({ reason: "assignment is missing providerAccountRef" }));
  }

  if (assignment.authGrantRef === undefined) {
    return Effect.fail(new ProbeAssignmentParseError({ reason: "assignment is missing authGrantRef" }));
  }

  return Effect.succeed(
    assignment as ProbeRunAssignment & {
      readonly provider: ProbeProvider;
      readonly providerAccountRef: ProviderAccountRef;
      readonly authGrantRef: ProviderAuthGrantRef;
    },
  );
}
