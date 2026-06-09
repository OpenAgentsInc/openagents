import { Effect, Schema as S } from "effect";
import {
  BlueprintObjectiveSurface,
  BlueprintProgramFamily,
  BlueprintProgramRiskClass,
  BlueprintProgramStatus,
  BlueprintProgramToolScope,
  type BlueprintModuleVersionRef,
  type BlueprintProgramRegistryEntry,
  type BlueprintProgramRegistryProjection,
  type BlueprintProgramRiskClass as BlueprintProgramRiskClassType,
  type BlueprintProgramSignature,
  type BlueprintProgramToolScope as BlueprintProgramToolScopeType,
  type BlueprintProgramType,
  type BlueprintReleaseGateRef,
} from "./contracts";
import { blueprintRegistryProjectionIsSafe } from "./contracts";
import { BlueprintSignatureLookupRequest } from "./contracts";
import { BlueprintSignatureRegistryView } from "./registry-client";

export const BlueprintSignatureLookupInput = S.Struct({
  backendCapabilityRefs: S.Array(S.String),
  lookupId: S.String,
  maxToolCount: S.optional(S.Number),
  registryView: BlueprintSignatureRegistryView,
  request: BlueprintSignatureLookupRequest,
});
export type BlueprintSignatureLookupInput = typeof BlueprintSignatureLookupInput.Type;

export const BlueprintSignatureLookupSelection = S.Struct({
  actionSubmissionRequiredForDirectEffects: S.Literal(true),
  backendCapabilityRefs: S.Array(S.String),
  backendKind: S.optional(S.String),
  candidateEntryIds: S.Array(S.String),
  contextPackRef: S.optional(S.String),
  directMutationAllowed: S.Boolean,
  evidenceRequirementRefs: S.Array(S.String),
  lookupId: S.String,
  moduleVersionIds: S.Array(S.String),
  policyRef: S.String,
  programSignatureIds: S.Array(S.String),
  programTypeIds: S.Array(S.String),
  receiptRequirementRefs: S.Array(S.String),
  registryVersionRef: S.String,
  releaseGateRefs: S.Array(S.String),
  requiresContextPackRef: S.Boolean,
  safeProjection: S.Boolean,
  sourceKind: S.String,
  toolScopes: S.Array(BlueprintProgramToolScope),
});
export type BlueprintSignatureLookupSelection = typeof BlueprintSignatureLookupSelection.Type;

export class BlueprintSignatureLookupError extends S.TaggedErrorClass<BlueprintSignatureLookupError>()(
  "BlueprintSignatureLookupError",
  {
    lookupId: S.String,
    reason: S.String,
  },
) {}

export interface BlueprintSignatureLookupService {
  readonly lookup: (
    input: BlueprintSignatureLookupInput,
  ) => Effect.Effect<BlueprintSignatureLookupSelection, BlueprintSignatureLookupError>;
}

const RISK_ORDER: Record<BlueprintProgramRiskClassType, number> = {
  low: 1,
  medium: 2,
  high: 3,
  legal_sensitive: 4,
  payment_sensitive: 4,
};

const SELECTABLE_STATUSES: ReadonlySet<BlueprintProgramStatus> = new Set(["draft", "active"]);

export function makeBlueprintSignatureLookupService(): BlueprintSignatureLookupService {
  return {
    lookup: lookupBlueprintSignatures,
  };
}

export function lookupBlueprintSignatures(
  input: BlueprintSignatureLookupInput,
): Effect.Effect<BlueprintSignatureLookupSelection, BlueprintSignatureLookupError> {
  return Effect.gen(function* () {
    const { registry } = input.registryView;

    if (!blueprintRegistryProjectionIsSafe(registry)) {
      return yield* failLookup(input.lookupId, "registry projection is unsafe or allows direct mutation");
    }

    const entries = selectCandidateEntries(input, registry);

    if (entries.length === 0) {
      return yield* failLookup(input.lookupId, "no Blueprint registry entries matched the typed lookup request");
    }

    const programTypes = selectedProgramTypes(registry, entries);
    const signatures = selectedProgramSignatures(registry, entries);
    const moduleVersions = selectedModuleVersions(registry, entries, signatures, programTypes);
    const releaseGates = selectedReleaseGates(registry, entries, signatures, programTypes, moduleVersions);

    if (signatures.length === 0 || programTypes.length === 0) {
      return yield* failLookup(input.lookupId, "selected registry entries do not include complete program refs");
    }

    if (programTypes.some((programType) => !riskWithinCeiling(programType.riskClass, input.request.riskCeiling))) {
      return yield* failLookup(input.lookupId, "selected Program Type exceeds requested risk ceiling");
    }

    if (signatures.some((signature) => !signatureHasAllowedSurface(signature, input.request.allowedSurfaces))) {
      return yield* failLookup(input.lookupId, "selected Program Signature has no tool scope for allowed surfaces");
    }

    if (releaseGates.some((gate) => !releaseGateIsSelectable(gate))) {
      return yield* failLookup(input.lookupId, "selected release gate is blocked, failed, rejected, or self-promoting");
    }

    const requiresContextPackRef = signatures.some((signature) => signature.supportsContext);
    if (requiresContextPackRef && input.request.contextPackRef === undefined) {
      return yield* failLookup(input.lookupId, "selected Program Signature requires a contextPackRef");
    }

    const toolScopes = selectedToolScopes(signatures, input.request.allowedSurfaces, input.maxToolCount);

    if (toolScopes.length === 0) {
      return yield* failLookup(input.lookupId, "selected Program Signatures produced no allowed tool scopes");
    }

    const directMutationAllowed =
      entries.every((entry) => entry.directMutationAllowed) &&
      programTypes.every((programType) => programType.directMutationAllowed);

    return {
      actionSubmissionRequiredForDirectEffects: true,
      backendCapabilityRefs: unique(entries.flatMap((entry) => entry.capabilityRefs)),
      backendKind: input.request.backendKind,
      candidateEntryIds: entries.map((entry) => entry.id),
      contextPackRef: input.request.contextPackRef,
      directMutationAllowed,
      evidenceRequirementRefs: unique([
        ...entries.flatMap((entry) => entry.evidenceRefs),
        ...programTypes.flatMap((programType) =>
          programType.evidenceRequirements.map((requirement) => requirement.descriptionRef),
        ),
        ...signatures.flatMap((signature) =>
          signature.evidenceRequirements.map((requirement) => requirement.descriptionRef),
        ),
      ]),
      lookupId: input.lookupId,
      moduleVersionIds: moduleVersions.map((moduleVersion) => moduleVersion.id),
      policyRef: input.registryView.safeProjectionPolicyRef,
      programSignatureIds: signatures.map((signature) => signature.id),
      programTypeIds: programTypes.map((programType) => programType.id),
      receiptRequirementRefs: unique([
        ...entries.flatMap((entry) => entry.receiptRefs),
        ...programTypes.flatMap((programType) =>
          programType.receiptRequirements.map((requirement) => requirement.receiptRef),
        ),
        ...signatures.flatMap((signature) =>
          signature.receiptRequirements.map((requirement) => requirement.receiptRef),
        ),
      ]),
      registryVersionRef: input.registryView.registryVersionRef,
      releaseGateRefs: releaseGates.map((gate) => gate.id),
      requiresContextPackRef,
      safeProjection: input.registryView.registry.safeProjection,
      sourceKind: input.registryView.sourceKind,
      toolScopes,
    };
  });
}

function selectCandidateEntries(
  input: BlueprintSignatureLookupInput,
  registry: BlueprintProgramRegistryProjection,
): ReadonlyArray<BlueprintProgramRegistryEntry> {
  const exactSignatureIds = input.request.programSignatureIds ?? [];
  const exactTypeIds = input.request.programTypeIds ?? [];
  const hasExactRefs = exactSignatureIds.length > 0 || exactTypeIds.length > 0;

  const entries = hasExactRefs
    ? registry.entries.filter(
        (entry) =>
          exactSignatureIds.some((id) => entry.programSignatureIds.includes(id)) ||
          exactTypeIds.includes(entry.programTypeId),
      )
    : registry.entries.filter((entry) => structuredEntryMatches(input, registry, entry));

  return entries.filter((entry) => entryIsSelectable(input, registry, entry));
}

function structuredEntryMatches(
  input: BlueprintSignatureLookupInput,
  registry: BlueprintProgramRegistryProjection,
  entry: BlueprintProgramRegistryEntry,
): boolean {
  if (input.request.preferredFamily !== undefined && entry.family !== input.request.preferredFamily) {
    return false;
  }

  const programType = registry.programTypes.find((candidate) => candidate.id === entry.programTypeId);
  if (programType === undefined) {
    return false;
  }

  return riskWithinCeiling(programType.riskClass, input.request.riskCeiling);
}

function entryIsSelectable(
  input: BlueprintSignatureLookupInput,
  registry: BlueprintProgramRegistryProjection,
  entry: BlueprintProgramRegistryEntry,
): boolean {
  if (!entry.safeProjection || entry.directMutationAllowed) {
    return false;
  }

  if (!SELECTABLE_STATUSES.has(entry.status)) {
    return false;
  }

  if (input.request.backendKind !== undefined && !entry.backendKinds.includes(input.request.backendKind)) {
    return false;
  }

  if (!entry.capabilityRefs.every((ref) => input.backendCapabilityRefs.includes(ref))) {
    return false;
  }

  const signatures = registry.programSignatures.filter((signature) => entry.programSignatureIds.includes(signature.id));
  if (!signatures.every((signature) => SELECTABLE_STATUSES.has(signature.status))) {
    return false;
  }

  if (!signatures.some((signature) => signatureHasAllowedSurface(signature, input.request.allowedSurfaces))) {
    return false;
  }

  const releaseGates = registry.releaseGates.filter((gate) => entry.releaseGateIds.includes(gate.id));
  return releaseGates.every(releaseGateIsSelectable);
}

function selectedProgramTypes(
  registry: BlueprintProgramRegistryProjection,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
): ReadonlyArray<BlueprintProgramType> {
  const ids = new Set(entries.map((entry) => entry.programTypeId));
  return registry.programTypes.filter((programType) => ids.has(programType.id));
}

function selectedProgramSignatures(
  registry: BlueprintProgramRegistryProjection,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
): ReadonlyArray<BlueprintProgramSignature> {
  const ids = new Set(entries.flatMap((entry) => entry.programSignatureIds));
  return registry.programSignatures.filter((signature) => ids.has(signature.id));
}

function selectedModuleVersions(
  registry: BlueprintProgramRegistryProjection,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
  signatures: ReadonlyArray<BlueprintProgramSignature>,
  programTypes: ReadonlyArray<BlueprintProgramType>,
): ReadonlyArray<BlueprintModuleVersionRef> {
  const moduleIds = new Set(entries.flatMap((entry) => entry.moduleVersionIds));
  const signatureIds = new Set(signatures.map((signature) => signature.id));
  const programTypeIds = new Set(programTypes.map((programType) => programType.id));

  return registry.moduleVersions.filter(
    (moduleVersion) =>
      moduleIds.has(moduleVersion.id) ||
      (moduleVersion.programSignatureId !== null && signatureIds.has(moduleVersion.programSignatureId)) ||
      programTypeIds.has(moduleVersion.programTypeId),
  );
}

function selectedReleaseGates(
  registry: BlueprintProgramRegistryProjection,
  entries: ReadonlyArray<BlueprintProgramRegistryEntry>,
  signatures: ReadonlyArray<BlueprintProgramSignature>,
  programTypes: ReadonlyArray<BlueprintProgramType>,
  moduleVersions: ReadonlyArray<BlueprintModuleVersionRef>,
): ReadonlyArray<BlueprintReleaseGateRef> {
  const entryGateIds = new Set(entries.flatMap((entry) => entry.releaseGateIds));
  const targetRefs = new Set([
    ...signatures.map((signature) => signature.id),
    ...programTypes.map((programType) => programType.id),
    ...moduleVersions.map((moduleVersion) => moduleVersion.id),
  ]);

  return registry.releaseGates.filter((gate) => entryGateIds.has(gate.id) || targetRefs.has(gate.targetRef));
}

function selectedToolScopes(
  signatures: ReadonlyArray<BlueprintProgramSignature>,
  allowedSurfaces: ReadonlyArray<BlueprintObjectiveSurface>,
  maxToolCount: number | undefined,
): ReadonlyArray<BlueprintProgramToolScopeType> {
  const scopes = uniqueBy(
    signatures
      .flatMap((signature) => signature.toolScopes)
      .filter((scope) => toolScopeHasAllowedSurface(scope, allowedSurfaces)),
    (scope) => `${scope.toolRef}:${scope.access}:${scope.requiresApproval}`,
  );

  return maxToolCount === undefined ? scopes : scopes.slice(0, maxToolCount);
}

function signatureHasAllowedSurface(
  signature: BlueprintProgramSignature,
  allowedSurfaces: ReadonlyArray<BlueprintObjectiveSurface>,
): boolean {
  return signature.toolScopes.some((scope) => toolScopeHasAllowedSurface(scope, allowedSurfaces));
}

function toolScopeHasAllowedSurface(
  scope: BlueprintProgramToolScopeType,
  allowedSurfaces: ReadonlyArray<BlueprintObjectiveSurface>,
): boolean {
  return scope.allowedSurfaces.some((surface) => allowedSurfaces.includes(surface));
}

function releaseGateIsSelectable(gate: BlueprintReleaseGateRef): boolean {
  return (
    gate.fixturePassState !== "blocked" &&
    gate.fixturePassState !== "failed" &&
    gate.policyState !== "blocked" &&
    gate.reviewState !== "rejected" &&
    gate.decision !== "blocked" &&
    gate.decision !== "rejected" &&
    !gate.selfPromotionAttempt
  );
}

function riskWithinCeiling(risk: BlueprintProgramRiskClassType, ceiling: BlueprintProgramRiskClassType): boolean {
  return RISK_ORDER[risk] <= RISK_ORDER[ceiling];
}

function unique(values: ReadonlyArray<string>): Array<string> {
  return [...new Set(values)];
}

function uniqueBy<T>(values: ReadonlyArray<T>, key: (value: T) => string): Array<T> {
  const seen = new Set<string>();
  const output: T[] = [];

  for (const value of values) {
    const valueKey = key(value);
    if (seen.has(valueKey)) {
      continue;
    }

    seen.add(valueKey);
    output.push(value);
  }

  return output;
}

function failLookup(
  lookupId: string,
  reason: string,
): Effect.Effect<never, BlueprintSignatureLookupError> {
  return Effect.fail(new BlueprintSignatureLookupError({ lookupId, reason }));
}
