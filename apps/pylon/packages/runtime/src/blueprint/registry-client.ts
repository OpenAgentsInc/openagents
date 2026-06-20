import { Effect, Schema as S } from "effect";
import {
  BlueprintContractExportSeed,
  BlueprintProgramRegistryProjection,
  BlueprintRegistrySourceKind,
  validateBlueprintContractExportSeed,
  validateBlueprintRegistryProjection,
  type BlueprintContractExportSeed as BlueprintContractExportSeedType,
  type BlueprintProgramRegistryProjection as BlueprintProgramRegistryProjectionType,
  type BlueprintProjectionUnsafe,
  type BlueprintRegistrySourceKind as BlueprintRegistrySourceKindType,
} from "./contracts.js";
import {
  STATIC_BLUEPRINT_CONTRACT_EXPORT,
  STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
} from "./fixtures.js";

export const BlueprintSignatureRegistryView = S.Struct({
  contractExport: S.optional(BlueprintContractExportSeed),
  contractExportVersionRef: S.optional(S.String),
  registry: BlueprintProgramRegistryProjection,
  registryVersionRef: S.String,
  safeProjectionPolicyRef: S.String,
  sourceKind: BlueprintRegistrySourceKind,
});
export type BlueprintSignatureRegistryView = typeof BlueprintSignatureRegistryView.Type;

export class BlueprintRegistryClientError extends S.TaggedErrorClass<BlueprintRegistryClientError>()(
  "BlueprintRegistryClientError",
  {
    reason: S.String,
    sourceKind: BlueprintRegistrySourceKind,
    statusCode: S.optional(S.Number),
  },
) {}

export type BlueprintRegistryClientFailure = BlueprintRegistryClientError | BlueprintProjectionUnsafe;

export type AssignmentInlineBlueprintRegistry = Readonly<{
  blueprintContractExport?: unknown;
  blueprintRegistry?: unknown;
  blueprintRegistryVersionRef?: string;
}>;

export type BlueprintSignatureRegistrySource =
  | Readonly<{
      sourceKind: "staticFixture";
    }>
  | Readonly<{
      assignment: AssignmentInlineBlueprintRegistry;
      sourceKind: "assignmentInline";
    }>
  | Readonly<{
      baseUrl: string;
      bearerToken?: string;
      fetch?: typeof fetch;
      sourceKind: "omegaHttp";
    }>;

export interface BlueprintSignatureRegistryClientOptions {
  readonly fetch?: typeof fetch;
  readonly staticContractExport?: unknown;
  readonly staticRegistry?: unknown;
  readonly staticRegistryVersionRef?: string;
}

export interface BlueprintSignatureRegistryClient {
  readonly loadRegistry: (
    source: BlueprintSignatureRegistrySource,
  ) => Effect.Effect<BlueprintSignatureRegistryView, BlueprintRegistryClientFailure>;
}

type HttpJsonResponse = Readonly<{
  payload: unknown;
  registryVersionRef?: string;
}>;

export function makeBlueprintSignatureRegistryClient(
  options: BlueprintSignatureRegistryClientOptions = {},
): BlueprintSignatureRegistryClient {
  return {
    loadRegistry: (source) => loadBlueprintSignatureRegistry(source, options),
  };
}

export function loadBlueprintSignatureRegistry(
  source: BlueprintSignatureRegistrySource,
  options: BlueprintSignatureRegistryClientOptions = {},
): Effect.Effect<BlueprintSignatureRegistryView, BlueprintRegistryClientFailure> {
  switch (source.sourceKind) {
    case "staticFixture":
      return loadStaticFixtureRegistry(options);
    case "assignmentInline":
      return loadAssignmentInlineRegistry(source.assignment);
    case "omegaHttp":
      return loadOmegaHttpRegistry({
        ...source,
        fetch: source.fetch ?? options.fetch,
      });
  }
}

function loadStaticFixtureRegistry(
  options: BlueprintSignatureRegistryClientOptions,
): Effect.Effect<BlueprintSignatureRegistryView, BlueprintRegistryClientFailure> {
  return Effect.gen(function* () {
    const registry = yield* decodeSafeRegistryProjection(
      options.staticRegistry ?? STATIC_BLUEPRINT_PROGRAM_REGISTRY,
      "staticFixture",
    );
    const contractExport = yield* decodeSafeContractExport(
      options.staticContractExport ?? STATIC_BLUEPRINT_CONTRACT_EXPORT,
      "staticFixture",
    );

    return normalizeRegistryView({
      contractExport,
      registry,
      registryVersionRef: options.staticRegistryVersionRef ?? STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
      sourceKind: "staticFixture",
    });
  });
}

function loadAssignmentInlineRegistry(
  assignment: AssignmentInlineBlueprintRegistry,
): Effect.Effect<BlueprintSignatureRegistryView, BlueprintRegistryClientFailure> {
  return Effect.gen(function* () {
    if (assignment.blueprintRegistry === undefined) {
      return yield* Effect.fail(
        new BlueprintRegistryClientError({
          reason: "assignment did not include a Blueprint registry projection",
          sourceKind: "assignmentInline",
        }),
      );
    }

    const registry = yield* decodeSafeRegistryProjection(assignment.blueprintRegistry, "assignmentInline");
    const contractExport =
      assignment.blueprintContractExport === undefined
        ? undefined
        : yield* decodeSafeContractExport(assignment.blueprintContractExport, "assignmentInline");

    return normalizeRegistryView({
      contractExport,
      registry,
      registryVersionRef: assignment.blueprintRegistryVersionRef,
      sourceKind: "assignmentInline",
    });
  });
}

function loadOmegaHttpRegistry(
  source: Extract<BlueprintSignatureRegistrySource, { sourceKind: "omegaHttp" }>,
): Effect.Effect<BlueprintSignatureRegistryView, BlueprintRegistryClientFailure> {
  return Effect.gen(function* () {
    const registryResponse = yield* requestBlueprintJson(
      {
        baseUrl: source.baseUrl,
        bearerToken: source.bearerToken,
        fetch: source.fetch,
        sourceKind: source.sourceKind,
      },
      "/api/blueprint/program-registry",
    );
    const contractResponse = yield* requestBlueprintJson(
      {
        baseUrl: source.baseUrl,
        bearerToken: source.bearerToken,
        fetch: source.fetch,
        sourceKind: source.sourceKind,
      },
      "/api/blueprint/contracts",
    );

    const registry = yield* decodeSafeRegistryProjection(registryResponse.payload, "omegaHttp");
    const contractExport = yield* decodeSafeContractExport(contractResponse.payload, "omegaHttp");

    return normalizeRegistryView({
      contractExport,
      registry,
      registryVersionRef: registryResponse.registryVersionRef,
      sourceKind: "omegaHttp",
    });
  });
}

function decodeSafeRegistryProjection(
  payload: unknown,
  sourceKind: BlueprintRegistrySourceKindType,
): Effect.Effect<BlueprintProgramRegistryProjectionType, BlueprintRegistryClientFailure> {
  return S.decodeUnknownEffect(BlueprintProgramRegistryProjection)(payload).pipe(
    Effect.mapError(
      (error) =>
        new BlueprintRegistryClientError({
          reason: `Blueprint registry payload did not match Probe contract: ${String(error)}`,
          sourceKind,
        }),
    ),
    Effect.flatMap(validateBlueprintRegistryProjection),
  );
}

function decodeSafeContractExport(
  payload: unknown,
  sourceKind: BlueprintRegistrySourceKindType,
): Effect.Effect<BlueprintContractExportSeedType, BlueprintRegistryClientFailure> {
  return S.decodeUnknownEffect(BlueprintContractExportSeed)(payload).pipe(
    Effect.mapError(
      (error) =>
        new BlueprintRegistryClientError({
          reason: `Blueprint contract export payload did not match Probe contract: ${String(error)}`,
          sourceKind,
        }),
    ),
    Effect.flatMap(validateBlueprintContractExportSeed),
  );
}

function normalizeRegistryView(input: {
  readonly contractExport?: BlueprintContractExportSeedType;
  readonly registry: BlueprintProgramRegistryProjectionType;
  readonly registryVersionRef?: string;
  readonly sourceKind: BlueprintRegistrySourceKindType;
}): BlueprintSignatureRegistryView {
  return {
    contractExport: input.contractExport,
    contractExportVersionRef: input.contractExport?.versionRef,
    registry: input.registry,
    registryVersionRef: input.registryVersionRef ?? deriveRegistryVersionRef(input.sourceKind, input.registry.policyRef),
    safeProjectionPolicyRef: input.registry.policyRef,
    sourceKind: input.sourceKind,
  };
}

function deriveRegistryVersionRef(sourceKind: BlueprintRegistrySourceKindType, policyRef: string): string {
  return `blueprint_registry.${sourceKind}.${policyRef}`;
}

function requestBlueprintJson(
  options: {
    readonly baseUrl: string;
    readonly bearerToken?: string;
    readonly fetch?: typeof fetch;
    readonly sourceKind: BlueprintRegistrySourceKindType;
  },
  path: string,
): Effect.Effect<HttpJsonResponse, BlueprintRegistryClientError> {
  return Effect.gen(function* () {
    const endpoint = new URL(path, options.baseUrl);
    const fetchImpl = options.fetch ?? fetch;

    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "GET",
          headers: options.bearerToken === undefined ? {} : { Authorization: `Bearer ${options.bearerToken}` },
        }),
      catch: (error) =>
        new BlueprintRegistryClientError({
          reason: `Blueprint registry HTTP request failed: ${String(error)}`,
          sourceKind: options.sourceKind,
        }),
    });

    if (!response.ok) {
      return yield* Effect.fail(
        new BlueprintRegistryClientError({
          reason: `Blueprint registry HTTP request failed with HTTP ${response.status}`,
          sourceKind: options.sourceKind,
          statusCode: response.status,
        }),
      );
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) =>
        new BlueprintRegistryClientError({
          reason: `Blueprint registry HTTP route returned invalid JSON: ${String(error)}`,
          sourceKind: options.sourceKind,
        }),
    });

    return {
      payload,
      registryVersionRef: response.headers.get("x-blueprint-registry-version-ref") ?? undefined,
    };
  });
}
