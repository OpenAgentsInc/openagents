import { Effect, Schema as S } from "effect";
import { makeAppleFmClient } from "../backends/apple-fm/client";
import { APPLE_FM_BACKEND_KIND } from "../backends/apple-fm/contract";
import { resolveGeminiApiKey } from "../backends/gemini/auth";
import {
  GEMINI_API_PROFILE_ID,
  GEMINI_BACKEND_KIND,
  GEMINI_DEFAULT_BASE_URL,
  GEMINI_DEFAULT_MODEL_ID,
  PROBE_GEMINI_BACKEND_CAPABILITY,
} from "../backends/gemini/contract";
import { makeGeminiAvailabilityReceipt } from "../backends/gemini/receipts";
import { resolveGeminiBackendProfile } from "../backends/registry";
import {
  BlueprintProgramRegistryProjection,
  validateBlueprintRegistryProjection,
  type BlueprintProgramRegistryProjection as BlueprintProgramRegistryProjectionType,
} from "../blueprint/contracts";
import { STATIC_BLUEPRINT_PROGRAM_REGISTRY, STATIC_BLUEPRINT_REGISTRY_VERSION_REF } from "../blueprint/fixtures";
import { redactReceiptUrl } from "../receipt-redaction";
import { PROBE_APPLE_FM_BACKEND_CAPABILITY, type ProbeRunnerIdentity } from "../runner/identity";

const APPLE_FM_BLUEPRINT_TOOL_PROJECTION_ADAPTER = "adapter.probe.apple_fm.blueprint_tools.v1" as const;
const PROBE_LOCAL_PROGRAM_RUN_EVIDENCE_CAPABILITY = "probe.program_run.evidence.local_offline" as const;
const DEFAULT_MAX_PROJECTED_APPLE_FM_TOOL_COUNT = 8;

export const ProbeBackendAvailabilityProjection = S.Struct({
  api: S.Boolean,
  local: S.Boolean,
  swarm: S.Boolean,
});
export type ProbeBackendAvailabilityProjection = typeof ProbeBackendAvailabilityProjection.Type;

export const ProbeAppleFmSchemaProjectionSupport = S.Struct({
  maxProjectedToolCount: S.Number,
  supported: S.Boolean,
  supportedInputSchemaRefs: S.Array(S.String),
  unsupportedReason: S.optional(S.String),
});
export type ProbeAppleFmSchemaProjectionSupport = typeof ProbeAppleFmSchemaProjectionSupport.Type;

export const ProbeBlueprintBackendCapabilitySupport = S.Struct({
  appleFmSchemaProjection: ProbeAppleFmSchemaProjectionSupport,
  backendAvailability: ProbeBackendAvailabilityProjection,
  backendToolProjectionAdapters: S.Array(S.String),
  localProgramRunEvidenceOffline: S.Boolean,
  moduleVersionRefs: S.Array(S.String),
  programFamilies: S.Array(S.String),
  programSignatureRefs: S.Array(S.String),
  programTypeRefs: S.Array(S.String),
  registryVersionRefs: S.Array(S.String),
  safeProjection: S.Boolean,
  safeProjectionPolicyRefs: S.Array(S.String),
  supportedBlueprintCapabilityRefs: S.Array(S.String),
  toolRefs: S.Array(S.String),
  warnings: S.Array(S.String),
});
export type ProbeBlueprintBackendCapabilitySupport = typeof ProbeBlueprintBackendCapabilitySupport.Type;

export const ProbeBackendCapabilityReport = S.Struct({
  kind: S.Literal("probe_backend_capability_report"),
  runnerId: S.String,
  runnerKind: S.Literals(["local", "shc", "pylon", "sandbox"]),
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  capability: S.Literal(PROBE_APPLE_FM_BACKEND_CAPABILITY),
  advertisedCapabilities: S.Array(S.String),
  available: S.Boolean,
  status: S.Literals(["ready", "unavailable", "unsupported", "malformed", "unreachable"]),
  baseUrl: S.String,
  platform: S.optional(S.String),
  version: S.optional(S.String),
  unavailableReason: S.optional(S.String),
  message: S.optional(S.String),
  requirements: S.Struct({
    appleSilicon: S.Literal("required"),
    appleIntelligence: S.Literal("required"),
    liveHealth: S.Literal("required"),
  }),
  support: S.Struct({
    snapshotStreaming: S.Boolean,
    toolCallbacks: S.Boolean,
  }),
  blueprintSupport: ProbeBlueprintBackendCapabilitySupport,
  receipt: S.Unknown,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type ProbeBackendCapabilityReport = typeof ProbeBackendCapabilityReport.Type;

export const ProbeGeminiBackendCapabilityReport = S.Struct({
  kind: S.Literal("probe_backend_capability_report"),
  runnerId: S.String,
  runnerKind: S.Literals(["local", "shc", "pylon", "sandbox"]),
  backendKind: S.Literal(GEMINI_BACKEND_KIND),
  profileId: S.String,
  model: S.String,
  capability: S.Literal(PROBE_GEMINI_BACKEND_CAPABILITY),
  advertisedCapabilities: S.Array(S.String),
  available: S.Boolean,
  status: S.Literals(["ready", "unavailable", "malformed"]),
  baseUrl: S.String,
  unavailableReason: S.optional(S.String),
  message: S.optional(S.String),
  requirements: S.Struct({
    apiKey: S.Literal("required"),
    liveHealth: S.Literal("not_required"),
  }),
  support: S.Struct({
    sseStreaming: S.Boolean,
    nativeToolCalls: S.Boolean,
    toolCallbacks: S.Boolean,
  }),
  receipt: S.Unknown,
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type ProbeGeminiBackendCapabilityReport = typeof ProbeGeminiBackendCapabilityReport.Type;

export const ProbeAnyBackendCapabilityReport = S.Union([ProbeBackendCapabilityReport, ProbeGeminiBackendCapabilityReport]);
export type ProbeAnyBackendCapabilityReport = typeof ProbeAnyBackendCapabilityReport.Type;

export interface ReportAppleFmBackendCapabilityInput {
  readonly runner: ProbeRunnerIdentity;
  readonly trustedBackendBaseUrl?: string;
  readonly backendAvailability?: Partial<ProbeBackendAvailabilityProjection>;
  readonly blueprintRegistry?: unknown;
  readonly blueprintRegistryVersionRef?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly fetch?: typeof fetch;
  readonly maxProjectedAppleFmToolCount?: number;
  readonly now?: Date;
}

export function reportAppleFmBackendCapability(
  input: ReportAppleFmBackendCapabilityInput,
): Effect.Effect<ProbeBackendCapabilityReport, never> {
  return makeAppleFmClient({
    explicitBaseUrl: input.trustedBackendBaseUrl,
    env: input.env,
    fetch: input.fetch,
    now: input.now,
  }).pipe(
    Effect.flatMap((client) =>
      Effect.gen(function* () {
        const blueprintSupport = yield* buildBlueprintBackendCapabilitySupport(input);
        const readiness = yield* client.health();
        const backendAvailability = {
          api: input.backendAvailability?.api ?? false,
          local: readiness.ready,
          swarm: input.backendAvailability?.swarm ?? false,
        };
        const routedBlueprintSupport: ProbeBlueprintBackendCapabilitySupport = {
          ...blueprintSupport,
          backendAvailability,
        };
        const blueprintRunnable =
          readiness.ready &&
          routedBlueprintSupport.safeProjection &&
          routedBlueprintSupport.appleFmSchemaProjection.supported;
        const advertisedCapabilities = blueprintRunnable
          ? uniqueStrings([
              PROBE_APPLE_FM_BACKEND_CAPABILITY,
              ...routedBlueprintSupport.supportedBlueprintCapabilityRefs,
              ...routedBlueprintSupport.backendToolProjectionAdapters,
              ...(routedBlueprintSupport.localProgramRunEvidenceOffline
                ? [PROBE_LOCAL_PROGRAM_RUN_EVIDENCE_CAPABILITY]
                : []),
            ])
          : [];

        return {
          kind: "probe_backend_capability_report",
          runnerId: input.runner.runnerId,
          runnerKind: input.runner.kind,
          backendKind: APPLE_FM_BACKEND_KIND,
          profileId: client.profile.id,
          model: readiness.health?.modelId ?? readiness.health?.model ?? client.profile.model,
          capability: PROBE_APPLE_FM_BACKEND_CAPABILITY,
          advertisedCapabilities,
          available: blueprintRunnable,
          status: readiness.ready && !blueprintRunnable ? "malformed" : readiness.status,
          baseUrl: redactReceiptUrl(client.profile.baseUrl),
          platform: readiness.health?.platform,
          version: readiness.health?.version,
          unavailableReason:
            readiness.unavailableReason ?? (readiness.ready && !blueprintRunnable ? "malformed_blueprint_support" : undefined),
          message:
            readiness.message ??
            (readiness.ready && !blueprintRunnable ? routedBlueprintSupport.warnings.join("; ") : undefined),
          requirements: {
            appleSilicon: "required",
            appleIntelligence: "required",
            liveHealth: "required",
          },
          support: {
            snapshotStreaming: true,
            toolCallbacks: true,
          },
          blueprintSupport: routedBlueprintSupport,
          receipt: readiness.receipt,
          observedAt: (input.now ?? new Date()).toISOString(),
          contentRedacted: true,
        } satisfies ProbeBackendCapabilityReport;
      }),
    ),
    Effect.catch(() =>
      Effect.succeed({
        kind: "probe_backend_capability_report" as const,
        runnerId: input.runner.runnerId,
        runnerKind: input.runner.kind,
        backendKind: APPLE_FM_BACKEND_KIND,
        profileId: "apple-fm-local",
        model: "apple-foundation-model",
        capability: PROBE_APPLE_FM_BACKEND_CAPABILITY,
        advertisedCapabilities: [],
        available: false,
        status: "malformed" as const,
        baseUrl: "[redacted-invalid-url]",
        unavailableReason: "malformed_response",
        message: "Apple FM backend capability profile could not be resolved",
        requirements: {
          appleSilicon: "required" as const,
          appleIntelligence: "required" as const,
          liveHealth: "required" as const,
        },
        support: {
          snapshotStreaming: true,
          toolCallbacks: true,
        },
        blueprintSupport: {
          appleFmSchemaProjection: {
            maxProjectedToolCount: input.maxProjectedAppleFmToolCount ?? DEFAULT_MAX_PROJECTED_APPLE_FM_TOOL_COUNT,
            supported: false,
            supportedInputSchemaRefs: [],
            unsupportedReason: "malformed_backend_profile",
          },
          backendAvailability: {
            api: input.backendAvailability?.api ?? false,
            local: false,
            swarm: input.backendAvailability?.swarm ?? false,
          },
          backendToolProjectionAdapters: [],
          localProgramRunEvidenceOffline: false,
          moduleVersionRefs: [],
          programFamilies: [],
          programSignatureRefs: [],
          programTypeRefs: [],
          registryVersionRefs: [],
          safeProjection: false,
          safeProjectionPolicyRefs: [],
          supportedBlueprintCapabilityRefs: [],
          toolRefs: [],
          warnings: ["Apple FM backend capability profile could not be resolved"],
        },
        receipt: {
          kind: "probe_backend_availability",
          backendKind: APPLE_FM_BACKEND_KIND,
          profileId: "apple-fm-local",
          model: "apple-foundation-model",
          baseUrl: "[redacted-invalid-url]",
          ready: false,
          unavailableReason: "malformed_response",
          message: "Apple FM backend capability profile could not be resolved",
          observedAt: (input.now ?? new Date()).toISOString(),
          contentRedacted: true,
        },
        observedAt: (input.now ?? new Date()).toISOString(),
        contentRedacted: true as const,
      }),
    ),
  );
}

export interface ReportGeminiBackendCapabilityInput {
  readonly runner: ProbeRunnerIdentity;
  readonly trustedBackendBaseUrl?: string;
  readonly apiKey?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly now?: Date;
}

export function reportGeminiBackendCapability(
  input: ReportGeminiBackendCapabilityInput,
): Effect.Effect<ProbeGeminiBackendCapabilityReport, never> {
  const observedAt = (input.now ?? new Date()).toISOString();

  return resolveGeminiBackendProfile({
    explicitBaseUrl: input.trustedBackendBaseUrl,
    env: input.env,
  }).pipe(
    Effect.flatMap((profile) =>
      resolveGeminiApiKey({ apiKey: input.apiKey, env: input.env, profileId: profile.id }).pipe(
        Effect.map((apiKey) => ({
          kind: "probe_backend_capability_report" as const,
          runnerId: input.runner.runnerId,
          runnerKind: input.runner.kind,
          backendKind: GEMINI_BACKEND_KIND,
          profileId: profile.id,
          model: profile.model,
          capability: PROBE_GEMINI_BACKEND_CAPABILITY,
          advertisedCapabilities: [PROBE_GEMINI_BACKEND_CAPABILITY],
          available: true,
          status: "ready" as const,
          baseUrl: redactReceiptUrl(profile.baseUrl),
          requirements: {
            apiKey: "required" as const,
            liveHealth: "not_required" as const,
          },
          support: {
            sseStreaming: true,
            nativeToolCalls: true,
            toolCallbacks: false,
          },
          receipt: makeGeminiAvailabilityReceipt({
            profileId: profile.id,
            model: profile.model,
            baseUrl: profile.baseUrl,
            ready: true,
            apiKeySource: apiKey.source,
            observedAt,
          }),
          observedAt,
          contentRedacted: true as const,
        })),
        Effect.catch((error) =>
          Effect.succeed({
            kind: "probe_backend_capability_report" as const,
            runnerId: input.runner.runnerId,
            runnerKind: input.runner.kind,
            backendKind: GEMINI_BACKEND_KIND,
            profileId: profile.id,
            model: profile.model,
            capability: PROBE_GEMINI_BACKEND_CAPABILITY,
            advertisedCapabilities: [],
            available: false,
            status: "unavailable" as const,
            baseUrl: redactReceiptUrl(profile.baseUrl),
            unavailableReason: "missing_credential",
            message: error.reason,
            requirements: {
              apiKey: "required" as const,
              liveHealth: "not_required" as const,
            },
            support: {
              sseStreaming: true,
              nativeToolCalls: true,
              toolCallbacks: false,
            },
            receipt: makeGeminiAvailabilityReceipt({
              profileId: profile.id,
              model: profile.model,
              baseUrl: profile.baseUrl,
              ready: false,
              unavailableReason: "missing_credential",
              message: error.reason,
              observedAt,
            }),
            observedAt,
            contentRedacted: true as const,
          }),
        ),
      ),
    ),
    Effect.catch(() =>
      Effect.succeed({
        kind: "probe_backend_capability_report" as const,
        runnerId: input.runner.runnerId,
        runnerKind: input.runner.kind,
        backendKind: GEMINI_BACKEND_KIND,
        profileId: GEMINI_API_PROFILE_ID,
        model: GEMINI_DEFAULT_MODEL_ID,
        capability: PROBE_GEMINI_BACKEND_CAPABILITY,
        advertisedCapabilities: [],
        available: false,
        status: "malformed" as const,
        baseUrl: redactReceiptUrl(GEMINI_DEFAULT_BASE_URL),
        unavailableReason: "malformed_backend_profile",
        message: "Gemini backend capability profile could not be resolved",
        requirements: {
          apiKey: "required" as const,
          liveHealth: "not_required" as const,
        },
        support: {
          sseStreaming: true,
          nativeToolCalls: true,
          toolCallbacks: false,
        },
        receipt: makeGeminiAvailabilityReceipt({
          profileId: GEMINI_API_PROFILE_ID,
          model: GEMINI_DEFAULT_MODEL_ID,
          baseUrl: GEMINI_DEFAULT_BASE_URL,
          ready: false,
          unavailableReason: "malformed_backend_profile",
          message: "Gemini backend capability profile could not be resolved",
          observedAt,
        }),
        observedAt,
        contentRedacted: true as const,
      }),
    ),
  );
}

function buildBlueprintBackendCapabilitySupport(
  input: ReportAppleFmBackendCapabilityInput,
): Effect.Effect<ProbeBlueprintBackendCapabilitySupport, never> {
  const maxProjectedToolCount = input.maxProjectedAppleFmToolCount ?? DEFAULT_MAX_PROJECTED_APPLE_FM_TOOL_COUNT;

  return S.decodeUnknownEffect(BlueprintProgramRegistryProjection)(
    input.blueprintRegistry ?? STATIC_BLUEPRINT_PROGRAM_REGISTRY,
  ).pipe(
    Effect.flatMap(validateBlueprintRegistryProjection),
    Effect.map((registry) =>
      blueprintSupportFromRegistry({
        maxProjectedToolCount,
        registry,
        registryVersionRef: input.blueprintRegistryVersionRef ?? STATIC_BLUEPRINT_REGISTRY_VERSION_REF,
      }),
    ),
    Effect.catch((error) =>
      Effect.succeed({
        appleFmSchemaProjection: {
          maxProjectedToolCount,
          supported: false,
          supportedInputSchemaRefs: [],
          unsupportedReason: "malformed_blueprint_registry_projection",
        },
        backendAvailability: {
          api: input.backendAvailability?.api ?? false,
          local: false,
          swarm: input.backendAvailability?.swarm ?? false,
        },
        backendToolProjectionAdapters: [],
        localProgramRunEvidenceOffline: false,
        moduleVersionRefs: [],
        programFamilies: [],
        programSignatureRefs: [],
        programTypeRefs: [],
        registryVersionRefs: [],
        safeProjection: false,
        safeProjectionPolicyRefs: [],
        supportedBlueprintCapabilityRefs: [],
        toolRefs: [],
        warnings: [`Blueprint capability support is not safe to advertise: ${String(error._tag)}`],
      }),
    ),
  );
}

function blueprintSupportFromRegistry(input: {
  readonly maxProjectedToolCount: number;
  readonly registry: BlueprintProgramRegistryProjectionType;
  readonly registryVersionRef: string;
}): ProbeBlueprintBackendCapabilitySupport {
  const toolRefs = uniqueStrings(input.registry.programTypes.flatMap((programType) => programType.toolScopes.map((tool) => tool.toolRef)));
  const inputSchemaRefs = toolRefs
    .map((toolRef) => supportedInputSchemaRef(toolRef))
    .filter((schemaRef): schemaRef is string => schemaRef !== undefined);
  const appleFmSchemaProjectionSupported = input.maxProjectedToolCount > 0 && inputSchemaRefs.length === toolRefs.length;
  const warnings = appleFmSchemaProjectionSupported
    ? []
    : [
        input.maxProjectedToolCount <= 0
          ? "Apple FM projected tool count must be greater than zero"
          : "One or more Blueprint tool refs cannot be projected into Apple FM schemas",
      ];

  return {
    appleFmSchemaProjection: {
      maxProjectedToolCount: input.maxProjectedToolCount,
      supported: appleFmSchemaProjectionSupported,
      supportedInputSchemaRefs: inputSchemaRefs,
      unsupportedReason: appleFmSchemaProjectionSupported ? undefined : "unsupported_tool_schema_projection",
    },
    backendAvailability: {
      api: false,
      local: false,
      swarm: false,
    },
    backendToolProjectionAdapters: [APPLE_FM_BLUEPRINT_TOOL_PROJECTION_ADAPTER],
    localProgramRunEvidenceOffline: true,
    moduleVersionRefs: uniqueStrings(input.registry.moduleVersions.map((moduleVersion) => moduleVersion.id)),
    programFamilies: uniqueStrings(input.registry.programTypes.map((programType) => programType.family)),
    programSignatureRefs: uniqueStrings(input.registry.programSignatures.map((signature) => signature.id)),
    programTypeRefs: uniqueStrings(input.registry.programTypes.map((programType) => programType.id)),
    registryVersionRefs: [input.registryVersionRef],
    safeProjection: true,
    safeProjectionPolicyRefs: [input.registry.policyRef],
    supportedBlueprintCapabilityRefs: uniqueStrings(input.registry.entries.flatMap((entry) => entry.capabilityRefs)),
    toolRefs,
    warnings,
  };
}

function supportedInputSchemaRef(toolRef: string): string | undefined {
  switch (toolRef) {
    case "tool.probe.code_search":
      return "schema.probe.tool.code_search.input.v1";
    case "tool.probe.propose_action_submission":
      return "schema.probe.tool.propose_action_submission.input.v1";
    case "tool.probe.read_file":
      return "schema.probe.tool.read_file.input.v1";
    case "tool.probe.record_evidence":
      return "schema.probe.tool.record_evidence.input.v1";
    default:
      return undefined;
  }
}

function uniqueStrings(values: ReadonlyArray<string>): Array<string> {
  return [...new Set(values)].sort();
}
