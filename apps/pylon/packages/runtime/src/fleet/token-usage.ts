import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";
import { type AppleFmUsageMeasurement } from "../backends/apple-fm/contract.js";
import { type ResolvedProbeBackendProfile } from "../backends/backend-profile.js";
import { type GeminiCompleteResult } from "../backends/gemini/client.js";
import { type ProbeRunAssignment } from "../contracts/assignment.js";
import { validateProbePublicProjection, type ProbePublicProjectionUnsafe } from "../contracts/provider-account.js";
import { type ProbeLlmUsage } from "../llm/usage.js";

export type ProbeTokenUsageProducerSystem = "probe";
export type ProbeTokenUsageSourceRoute =
  | "omega_hosted_gemini"
  | "probe_direct_provider"
  | "probe_local_model";
export type ProbeTokenUsageTruth = "exact" | "estimated" | "unknown";

export interface ProbeTokenUsageCounts {
  readonly inputTokens: number;
  readonly outputTokens: number;
  readonly reasoningTokens: number;
  readonly cacheReadTokens: number;
  readonly cacheWrite5mTokens: number;
  readonly cacheWrite1hTokens: number;
  readonly totalTokens: number;
}

export interface ProbeTokenUsageEvent {
  readonly schemaVersion: "openagents.token_usage_event.v1";
  readonly actor?: {
    readonly accountRef?: string;
    readonly teamId?: string;
    readonly userId?: string;
  };
  readonly backendProfile?: string;
  readonly cost?: {
    readonly amount: number;
    readonly currency: string;
  };
  readonly eventId: string;
  readonly idempotencyKey: string;
  readonly model?: string;
  readonly observedAt: string;
  readonly privacy?: {
    readonly leaderboardEligible: boolean;
    readonly privacyOptOut: boolean;
  };
  readonly producerSystem: ProbeTokenUsageProducerSystem;
  readonly provider?: string;
  readonly safeMetadata?: Readonly<Record<string, unknown>>;
  readonly sourceRefs?: {
    readonly anonymizedSourceRef?: string;
    readonly repositoryRef?: string;
    readonly runRef?: string;
    readonly sessionRef?: string;
    readonly taskRef?: string;
  };
  readonly sourceRoute: ProbeTokenUsageSourceRoute;
  readonly tokenCounts: ProbeTokenUsageCounts;
  readonly usageTruth: ProbeTokenUsageTruth;
}

export interface ProbeTokenUsageTelemetryClient {
  readonly reportEvent: (
    event: ProbeTokenUsageEvent,
  ) => Effect.Effect<void, ProbePublicProjectionUnsafe | ProbeTokenUsageTelemetryError | ProbeTokenUsageTelemetryUnsafe>;
}

export interface ProbeTokenUsageTelemetryClientOptions {
  readonly baseUrl: string;
  readonly bearerToken?: string;
  readonly fetch?: typeof fetch;
}

export interface ProbeTokenUsageTelemetryEnvOptions {
  readonly env?: Readonly<Record<string, string | undefined>>;
  readonly fetch?: typeof fetch;
  readonly managedAssignment?: boolean;
}

export class ProbeTokenUsageTelemetryError extends S.TaggedErrorClass<ProbeTokenUsageTelemetryError>()(
  "ProbeTokenUsageTelemetryError",
  {
    reason: S.String,
    statusCode: S.optional(S.Number),
  },
) {}

export class ProbeTokenUsageTelemetryUnsafe extends S.TaggedErrorClass<ProbeTokenUsageTelemetryUnsafe>()(
  "ProbeTokenUsageTelemetryUnsafe",
  {
    path: S.String,
    reason: S.String,
  },
) {}

const PROBE_RUNTIME_VERSION = "0.0.0";
const disabledValues = new Set(["0", "disabled", "false", "no", "off"]);

export function makeOmegaTokenUsageTelemetryClient(
  options: ProbeTokenUsageTelemetryClientOptions,
): ProbeTokenUsageTelemetryClient {
  return {
    reportEvent: (event) =>
      Effect.gen(function* () {
        yield* validateProbeTokenUsageEvent(event);
        const fetchImpl = options.fetch ?? fetch;
        const endpoint = new URL("/api/stats/token-usage/events", options.baseUrl);
        const response = yield* Effect.tryPromise({
          try: () =>
            fetchImpl(endpoint, {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
                ...(options.bearerToken === undefined ? {} : { Authorization: `Bearer ${options.bearerToken}` }),
              },
              body: JSON.stringify(event),
            }),
          catch: (error) =>
            new ProbeTokenUsageTelemetryError({
              reason: `Omega token usage telemetry request failed: ${String(error)}`,
            }),
        });

        if (!response.ok) {
          return yield* Effect.fail(
            new ProbeTokenUsageTelemetryError({
              reason: `Omega token usage telemetry failed with HTTP ${response.status}`,
              statusCode: response.status,
            }),
          );
        }
      }),
  };
}

export function makeNoopProbeTokenUsageTelemetryClient(): ProbeTokenUsageTelemetryClient {
  return {
    reportEvent: (event) => validateProbeTokenUsageEvent(event),
  };
}

export function makeStaticProbeTokenUsageTelemetryClient() {
  const events: ProbeTokenUsageEvent[] = [];
  const client: ProbeTokenUsageTelemetryClient = {
    reportEvent: (event) =>
      validateProbeTokenUsageEvent(event).pipe(
        Effect.map(() => {
          events.push(event);
        }),
      ),
  };

  return { client, events };
}

export function makeProbeTokenUsageTelemetryClientFromEnv(
  options: ProbeTokenUsageTelemetryEnvOptions = {},
): ProbeTokenUsageTelemetryClient {
  const env = options.env ?? {};

  if (probeTokenUsageTelemetryDisabled(env)) {
    return makeNoopProbeTokenUsageTelemetryClient();
  }

  const baseUrl = tokenUsageOmegaBaseUrl(env, options.managedAssignment === true);

  if (baseUrl === undefined) {
    return makeNoopProbeTokenUsageTelemetryClient();
  }

  return makeOmegaTokenUsageTelemetryClient({
    baseUrl,
    bearerToken: env.PROBE_TOKEN_USAGE_BEARER_TOKEN ?? env.PROBE_OMEGA_BEARER_TOKEN,
    fetch: options.fetch,
  });
}

export function recordProbeTokenUsageEvent(
  client: ProbeTokenUsageTelemetryClient,
  event: ProbeTokenUsageEvent,
): Effect.Effect<void, ProbePublicProjectionUnsafe | ProbeTokenUsageTelemetryError | ProbeTokenUsageTelemetryUnsafe> {
  return client.reportEvent(event);
}

export function bestEffortRecordProbeTokenUsageEvent(
  client: ProbeTokenUsageTelemetryClient,
  event: ProbeTokenUsageEvent,
): Effect.Effect<void, never> {
  return recordProbeTokenUsageEvent(client, event).pipe(Effect.catch(() => Effect.void));
}

export function makeGeminiProbeTokenUsageEvent(input: {
  readonly actor?: ProbeTokenUsageEvent["actor"];
  readonly agentSurface: string;
  readonly command?: string;
  readonly privacy?: ProbeTokenUsageEvent["privacy"];
  readonly result: GeminiCompleteResult;
  readonly sourceRefs?: ProbeTokenUsageEvent["sourceRefs"];
}): ProbeTokenUsageEvent {
  const sourceRoute = geminiSourceRoute(input.result.profile);
  const safeMetadata = compactRecord({
    agentSurface: input.agentSurface,
    backendKind: input.result.profile.kind,
    backendProfileId: input.result.profile.id,
    command: input.command,
    modelRoundTrips: input.result.roundTrips,
    probeVersion: PROBE_RUNTIME_VERSION,
    route: sourceRoute,
  });

  return makeProbeLlmTokenUsageEvent({
    actor: input.actor,
    backendProfile: input.result.profile.id,
    model: input.result.finalRequest.model.model,
    observedAt: input.result.receipt.observedAt,
    privacy: input.privacy,
    provider: "google_gemini",
    safeMetadata,
    sourceRefs: input.sourceRefs,
    sourceRoute,
    usage: input.result.receipt.usage,
    usageTruth: input.result.receipt.usage === undefined ? "unknown" : "exact",
  });
}

export function makeAppleFmProbeTokenUsageEvent(input: {
  readonly actor?: ProbeTokenUsageEvent["actor"];
  readonly agentSurface: string;
  readonly command?: string;
  readonly privacy?: ProbeTokenUsageEvent["privacy"];
  readonly profile: ResolvedProbeBackendProfile;
  readonly sourceRefs?: ProbeTokenUsageEvent["sourceRefs"];
  readonly usage: AppleFmUsageMeasurement;
  readonly model?: string;
  readonly observedAt: string;
}): ProbeTokenUsageEvent {
  return makeProbeLlmTokenUsageEvent({
    actor: input.actor,
    backendProfile: input.profile.id,
    model: input.model ?? input.profile.model,
    observedAt: input.observedAt,
    privacy: input.privacy,
    provider: "apple_fm",
    safeMetadata: compactRecord({
      agentSurface: input.agentSurface,
      backendKind: input.profile.kind,
      backendProfileId: input.profile.id,
      command: input.command,
      probeVersion: PROBE_RUNTIME_VERSION,
      route: "probe_local_model",
    }),
    sourceRefs: input.sourceRefs,
    sourceRoute: "probe_local_model",
    tokenCounts: tokenCountsFromAppleFmUsage(input.usage),
    usageTruth: input.usage.truth,
  });
}

export function makeProbeAssignmentTokenUsageSourceRefs(
  assignment: ProbeRunAssignment,
): NonNullable<ProbeTokenUsageEvent["sourceRefs"]> {
  return compactRecord({
    repositoryRef: repositoryRefFromAssignment(assignment),
    runRef: `probe.assignment.${safeRefSegment(assignment.assignmentId)}`,
    sessionRef: `probe.runner_session.${safeRefSegment(assignment.runnerSessionId)}`,
    taskRef: `probe.task.${safeRefSegment(assignment.assignmentId)}`,
  });
}

export function probeTokenUsageActorFromEnv(
  env: Readonly<Record<string, string | undefined>> = {},
): ProbeTokenUsageEvent["actor"] | undefined {
  return compactRecord({
    accountRef: optionalEnv(env.PROBE_TOKEN_USAGE_ACCOUNT_REF),
    teamId: optionalEnv(env.PROBE_TOKEN_USAGE_TEAM_ID),
    userId: optionalEnv(env.PROBE_TOKEN_USAGE_USER_ID),
  });
}

export function probeTokenUsagePrivacyFromEnv(
  env: Readonly<Record<string, string | undefined>> = {},
): NonNullable<ProbeTokenUsageEvent["privacy"]> {
  const privacyOptOut = envFlag(env.PROBE_TOKEN_USAGE_PRIVACY_OPT_OUT) || envFlag(env.PROBE_PRIVACY_OPT_OUT);

  return {
    leaderboardEligible: !privacyOptOut,
    privacyOptOut,
  };
}

export function validateProbeTokenUsageEvent(
  event: ProbeTokenUsageEvent,
): Effect.Effect<void, ProbePublicProjectionUnsafe | ProbeTokenUsageTelemetryUnsafe> {
  return validateProbePublicProjection(event, "tokenUsage").pipe(
    Effect.flatMap(() => validateNoPrivateTokenUsageMaterial(event, "tokenUsage")),
  );
}

function makeProbeLlmTokenUsageEvent(input: {
  readonly actor?: ProbeTokenUsageEvent["actor"];
  readonly backendProfile?: string;
  readonly model?: string;
  readonly observedAt: string;
  readonly privacy?: ProbeTokenUsageEvent["privacy"];
  readonly provider?: string;
  readonly safeMetadata?: Readonly<Record<string, unknown>>;
  readonly sourceRefs?: ProbeTokenUsageEvent["sourceRefs"];
  readonly sourceRoute: ProbeTokenUsageSourceRoute;
  readonly tokenCounts?: ProbeTokenUsageCounts;
  readonly usage?: ProbeLlmUsage;
  readonly usageTruth?: ProbeTokenUsageTruth;
}): ProbeTokenUsageEvent {
  const tokenCounts = input.tokenCounts ?? tokenCountsFromProbeLlmUsage(input.usage);
  const idSeed = stableJson({
    backendProfile: input.backendProfile,
    model: input.model,
    observedAt: input.observedAt,
    provider: input.provider,
    sourceRefs: input.sourceRefs,
    sourceRoute: input.sourceRoute,
    tokenCounts,
    usageTruth: input.usageTruth,
  });
  const digest = sha256(idSeed);

  return {
    schemaVersion: "openagents.token_usage_event.v1",
    actor: emptyRecordToUndefined(input.actor),
    backendProfile: input.backendProfile,
    eventId: `probe_token_usage_${digest.slice(0, 32)}`,
    idempotencyKey: `probe:${digest}`,
    model: input.model,
    observedAt: input.observedAt,
    privacy: input.privacy,
    producerSystem: "probe",
    provider: input.provider,
    safeMetadata: emptyRecordToUndefined(input.safeMetadata),
    sourceRefs: emptyRecordToUndefined(input.sourceRefs),
    sourceRoute: input.sourceRoute,
    tokenCounts,
    usageTruth: input.usageTruth ?? (hasReportedCount(tokenCounts) ? "exact" : "unknown"),
  };
}

function tokenCountsFromProbeLlmUsage(usage: ProbeLlmUsage | undefined): ProbeTokenUsageCounts {
  const inputTokens = nonNegativeInteger(usage?.inputTokens);
  const outputTokens = nonNegativeInteger(usage?.outputTokens);
  const reasoningTokens = nonNegativeInteger(usage?.reasoningTokens);
  const cacheReadTokens = nonNegativeInteger(usage?.cacheReadInputTokens);
  const cacheWrite5mTokens = nonNegativeInteger(usage?.cacheWriteInputTokens);
  const totalTokens = nonNegativeInteger(usage?.totalTokens ?? inputTokens + outputTokens);

  return {
    inputTokens,
    outputTokens,
    reasoningTokens,
    cacheReadTokens,
    cacheWrite5mTokens,
    cacheWrite1hTokens: 0,
    totalTokens,
  };
}

function tokenCountsFromAppleFmUsage(usage: AppleFmUsageMeasurement): ProbeTokenUsageCounts {
  const inputTokens = nonNegativeInteger(usage.promptTokens);
  const outputTokens = nonNegativeInteger(usage.completionTokens);
  const totalTokens = nonNegativeInteger(usage.totalTokens ?? inputTokens + outputTokens);

  return {
    inputTokens,
    outputTokens,
    reasoningTokens: 0,
    cacheReadTokens: 0,
    cacheWrite5mTokens: 0,
    cacheWrite1hTokens: 0,
    totalTokens,
  };
}

function validateNoPrivateTokenUsageMaterial(
  value: unknown,
  path: string,
): Effect.Effect<void, ProbeTokenUsageTelemetryUnsafe> {
  if (value === null || value === undefined) {
    return Effect.void;
  }

  if (typeof value === "string") {
    return unsafeTokenUsageValuePattern.test(value)
      ? Effect.fail(new ProbeTokenUsageTelemetryUnsafe({ path, reason: "contains private telemetry material" }))
      : Effect.void;
  }

  if (Array.isArray(value)) {
    return Effect.all(value.map((entry, index) => validateNoPrivateTokenUsageMaterial(entry, `${path}[${index}]`))).pipe(
      Effect.asVoid,
    );
  }

  if (typeof value !== "object") {
    return Effect.void;
  }

  return Effect.gen(function* () {
    for (const [key, entry] of Object.entries(value)) {
      const childPath = `${path}.${key}`;

      if (unsafeTokenUsageKeyPattern.test(key)) {
        return yield* Effect.fail(
          new ProbeTokenUsageTelemetryUnsafe({ path: childPath, reason: "contains unsafe telemetry key" }),
        );
      }

      yield* validateNoPrivateTokenUsageMaterial(entry, childPath);
    }
  });
}

const unsafeTokenUsageKeyPattern =
  /(^|[_-])(access[_-]?token|api[_-]?key|authorization|bearer[_-]?token|callback[_-]?(token|url)|code[_-]?verifier|completion|cookie|credential|device[_-]?auth|private[_-]?(key|path|repo|source|trace)|prompt|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|completion|log|payload|prompt|provider|response|source|text|trace)|refresh[_-]?token|secret|source[_-]?code|tool[_-]?args)$/i;

const unsafeTokenUsageValuePattern =
  /(@|\/Users\/|\/home\/|Bearer\s+[A-Za-z0-9._-]{8,}|authorization:\s*bearer|access[_-]?token=|api[_-]?key=|callback[_-]?token|callback[_-]?url|cookie=|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|mnemonic|private[_-]?(key|repo|source)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(completion|payload|prompt|provider|response|source|text|trace)|refresh[_-]?token|secret|sk-[a-z0-9]|source[_-]?archive|wallet[_-]?(key|mnemonic|secret|seed))/i;

function geminiSourceRoute(profile: ResolvedProbeBackendProfile): ProbeTokenUsageSourceRoute {
  return profile.baseUrlSource === "PROBE_OMEGA_BASE_URL" ? "omega_hosted_gemini" : "probe_direct_provider";
}

function repositoryRefFromAssignment(assignment: ProbeRunAssignment): string | undefined {
  const repo = assignment.repo;

  if (repo === undefined) {
    return undefined;
  }

  if (repo.commit !== undefined && repo.commit.trim().length > 0) {
    return `repo.commit.${safeRefSegment(repo.commit)}`;
  }

  const material = [repo.url, repo.path, repo.branch].filter((value): value is string => optionalEnv(value) !== undefined).join("|");

  return material.length === 0 ? undefined : `repo.sha256.${sha256(material).slice(0, 32)}`;
}

function tokenUsageOmegaBaseUrl(
  env: Readonly<Record<string, string | undefined>>,
  managedAssignment: boolean,
): string | undefined {
  return (
    optionalEnv(env.PROBE_TOKEN_USAGE_OMEGA_BASE_URL) ??
    optionalEnv(env.PROBE_OMEGA_BASE_URL) ??
    (managedAssignment ? "https://openagents.com" : undefined)
  );
}

function probeTokenUsageTelemetryDisabled(env: Readonly<Record<string, string | undefined>>): boolean {
  return (
    disabledValues.has((env.PROBE_TOKEN_USAGE_TELEMETRY ?? "").trim().toLowerCase()) ||
    envFlag(env.PROBE_TOKEN_USAGE_TELEMETRY_DISABLED) ||
    envFlag(env.PROBE_TOKEN_USAGE_OPT_OUT)
  );
}

function hasReportedCount(counts: ProbeTokenUsageCounts): boolean {
  return (
    counts.inputTokens > 0 ||
    counts.outputTokens > 0 ||
    counts.reasoningTokens > 0 ||
    counts.cacheReadTokens > 0 ||
    counts.cacheWrite5mTokens > 0 ||
    counts.cacheWrite1hTokens > 0 ||
    counts.totalTokens > 0
  );
}

function nonNegativeInteger(value: number | undefined): number {
  return value === undefined || !Number.isFinite(value) ? 0 : Math.max(0, Math.trunc(value));
}

function optionalEnv(value: string | undefined): string | undefined {
  const trimmed = value?.trim();

  return trimmed === undefined || trimmed === "" ? undefined : trimmed;
}

function envFlag(value: string | undefined): boolean {
  const normalized = value?.trim().toLowerCase();

  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on";
}

function safeRefSegment(value: string): string {
  return value.replace(/[^A-Za-z0-9._:-]/g, "_").slice(0, 160);
}

function compactRecord<T extends Readonly<Record<string, unknown>>>(record: T): T {
  return Object.fromEntries(Object.entries(record).filter((entry) => entry[1] !== undefined)) as T;
}

function emptyRecordToUndefined<T extends Readonly<Record<string, unknown>>>(record: T | undefined): T | undefined {
  if (record === undefined) {
    return undefined;
  }

  return Object.keys(record).length === 0 ? undefined : record;
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function stableJson(value: unknown): string {
  return JSON.stringify(sortJson(value));
}

function sortJson(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortJson);
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, sortJson(entry)]),
  );
}
