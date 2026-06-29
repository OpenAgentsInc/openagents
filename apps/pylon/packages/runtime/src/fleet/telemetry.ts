import { Effect, Schema as S } from "effect";
import {
  CHATGPT_CODEX_PROVIDER,
  ProviderAccountRef,
  ProviderAuthGrantRef,
  validateProbePublicProjection,
  type JsonValue,
  type ProbePublicProjectionUnsafe,
} from "../contracts/provider-account.js";

export const ProbeAuthFailureClass = S.Literals([
  "access_token_failed",
  "refresh_failed",
  "requires_reauth",
  "low_credit",
  "rate_limited",
  "provider_unavailable",
  "non_auth_failure",
]);
export type ProbeAuthFailureClass = typeof ProbeAuthFailureClass.Type;

export const ProbeAuthHealthSignal = S.Struct({
  kind: S.Literal("probe_auth_health_signal"),
  provider: S.Literal(CHATGPT_CODEX_PROVIDER),
  providerAccountRef: ProviderAccountRef,
  authGrantRef: S.optional(ProviderAuthGrantRef),
  leaseRef: S.optional(S.String),
  assignmentId: S.optional(S.String),
  runnerSessionId: S.optional(S.String),
  outcome: S.Literals(["success", "failure", "scrubbed"]),
  failureClass: S.optional(ProbeAuthFailureClass),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
  metadata: S.optional(S.Record(S.String, S.Unknown)),
});
export type ProbeAuthHealthSignal = typeof ProbeAuthHealthSignal.Type;

export const OmegaAccountHealthPatch = S.Struct({
  providerAccountRef: ProviderAccountRef,
  health: S.Literals(["healthy", "unhealthy", "requires_reauth"]),
  status: S.optional(S.Literals(["connected", "unhealthy", "expired"])),
  lowCredit: S.optional(S.Boolean),
  cooldownReason: S.optional(S.String),
  reauthRequiredReason: S.optional(S.String),
  recentFailureClass: S.optional(ProbeAuthFailureClass),
  lastProbeSignalAt: S.String,
});
export type OmegaAccountHealthPatch = typeof OmegaAccountHealthPatch.Type;

export class ProbeFleetTelemetryError extends S.TaggedErrorClass<ProbeFleetTelemetryError>()(
  "ProbeFleetTelemetryError",
  {
    reason: S.String,
    statusCode: S.optional(S.Number),
  },
) {}

export interface ProbeFleetTelemetryClient {
  readonly reportSignal: (
    signal: ProbeAuthHealthSignal,
    patch: OmegaAccountHealthPatch,
  ) => Effect.Effect<void, ProbeFleetTelemetryError | ProbePublicProjectionUnsafe>;
  readonly requestFailover: (
    signal: ProbeAuthHealthSignal,
  ) => Effect.Effect<OmegaFailoverRequestReceipt, ProbeFleetTelemetryError | ProbePublicProjectionUnsafe>;
}

export interface OmegaFailoverRequestReceipt {
  readonly kind: "probe_auth_failover_requested";
  readonly providerAccountRef: ProviderAccountRef;
  readonly leaseRef?: string;
  readonly failureClass: ProbeAuthFailureClass;
  readonly requestedAt: string;
  readonly contentRedacted: true;
}

export interface OmegaFleetTelemetryClientOptions {
  readonly baseUrl: string;
  readonly bearerToken?: string;
  readonly fetch?: typeof fetch;
}

export function makeProbeAuthHealthSignal(
  input: Omit<ProbeAuthHealthSignal, "kind" | "provider" | "contentRedacted" | "observedAt"> & {
    readonly observedAt?: string;
  },
): ProbeAuthHealthSignal {
  return {
    kind: "probe_auth_health_signal",
    provider: CHATGPT_CODEX_PROVIDER,
    observedAt: input.observedAt ?? new Date().toISOString(),
    contentRedacted: true,
    ...input,
  };
}

export function deriveOmegaAccountHealthPatch(signal: ProbeAuthHealthSignal): OmegaAccountHealthPatch {
  if (signal.outcome === "success" || signal.outcome === "scrubbed") {
    return {
      providerAccountRef: signal.providerAccountRef,
      health: "healthy",
      status: "connected",
      lastProbeSignalAt: signal.observedAt,
      recentFailureClass: signal.failureClass,
    };
  }

  if (
    signal.failureClass === "access_token_failed" ||
    signal.failureClass === "refresh_failed" ||
    signal.failureClass === "requires_reauth"
  ) {
    return {
      providerAccountRef: signal.providerAccountRef,
      health: "requires_reauth",
      status: "expired",
      reauthRequiredReason: signal.failureClass,
      recentFailureClass: signal.failureClass,
      lastProbeSignalAt: signal.observedAt,
    };
  }

  if (signal.failureClass === "low_credit") {
    return {
      providerAccountRef: signal.providerAccountRef,
      health: "unhealthy",
      status: "unhealthy",
      lowCredit: true,
      cooldownReason: "low_credit",
      recentFailureClass: signal.failureClass,
      lastProbeSignalAt: signal.observedAt,
    };
  }

  return {
    providerAccountRef: signal.providerAccountRef,
    health: "unhealthy",
    status: "unhealthy",
    cooldownReason: signal.failureClass ?? "provider_unavailable",
    recentFailureClass: signal.failureClass,
    lastProbeSignalAt: signal.observedAt,
  };
}

export function shouldRequestOmegaFailover(signal: ProbeAuthHealthSignal): boolean {
  return (
    signal.outcome === "failure" &&
    signal.failureClass !== undefined &&
    signal.failureClass !== "non_auth_failure" &&
    signal.leaseRef !== undefined
  );
}

export function recordProbeAuthHealthSignal(
  client: ProbeFleetTelemetryClient,
  signal: ProbeAuthHealthSignal,
): Effect.Effect<OmegaFailoverRequestReceipt | undefined, ProbeFleetTelemetryError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbePublicProjection(signal as unknown as JsonValue, "signal");
    const patch = deriveOmegaAccountHealthPatch(signal);
    yield* validateProbePublicProjection(patch as unknown as JsonValue, "healthPatch");
    yield* client.reportSignal(signal, patch);

    if (!shouldRequestOmegaFailover(signal)) {
      return undefined;
    }

    return yield* client.requestFailover(signal);
  });
}

export function makeOmegaFleetTelemetryClient(options: OmegaFleetTelemetryClientOptions): ProbeFleetTelemetryClient {
  return {
    reportSignal: (signal, patch) =>
      requestOmega(options, `/api/provider-accounts/${encodeURIComponent(signal.providerAccountRef)}/health`, {
        health: patch.health,
        status: patch.status,
        lowCredit: patch.lowCredit,
        cooldownReason: patch.cooldownReason,
        reauthRequiredReason: patch.reauthRequiredReason,
        recentFailureClass: patch.recentFailureClass,
        lastProbeSignalAt: patch.lastProbeSignalAt,
      }),
    requestFailover: (signal) =>
      requestOmega(options, "/api/operator/provider-accounts/chatgpt-codex/leases/failover", {
        providerAccountRef: signal.providerAccountRef,
        leaseRef: signal.leaseRef,
        authGrantRef: signal.authGrantRef,
        assignmentId: signal.assignmentId,
        runnerSessionId: signal.runnerSessionId,
        failureClass: signal.failureClass,
      }).pipe(
        Effect.as({
          kind: "probe_auth_failover_requested" as const,
          providerAccountRef: signal.providerAccountRef,
          leaseRef: signal.leaseRef,
          failureClass: signal.failureClass as ProbeAuthFailureClass,
          requestedAt: new Date().toISOString(),
          contentRedacted: true as const,
        }),
      ),
  };
}

export function makeStaticProbeFleetTelemetryClient() {
  const reported: Array<{ readonly signal: ProbeAuthHealthSignal; readonly patch: OmegaAccountHealthPatch }> = [];
  const failovers: ProbeAuthHealthSignal[] = [];
  const client: ProbeFleetTelemetryClient = {
    reportSignal: (signal, patch) =>
      Effect.sync(() => {
        reported.push({ signal, patch });
      }),
    requestFailover: (signal) =>
      Effect.sync(() => {
        failovers.push(signal);
        return {
          kind: "probe_auth_failover_requested" as const,
          providerAccountRef: signal.providerAccountRef,
          leaseRef: signal.leaseRef,
          failureClass: signal.failureClass as ProbeAuthFailureClass,
          requestedAt: signal.observedAt,
          contentRedacted: true as const,
        };
      }),
  };

  return { client, reported, failovers };
}

function requestOmega(
  options: OmegaFleetTelemetryClientOptions,
  path: string,
  body: unknown,
): Effect.Effect<void, ProbeFleetTelemetryError> {
  return Effect.gen(function* () {
    const endpoint = new URL(path, options.baseUrl);
    const fetchImpl = options.fetch ?? fetch;
    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...(options.bearerToken === undefined ? {} : { Authorization: `Bearer ${options.bearerToken}` }),
          },
          body: JSON.stringify(body),
        }),
      catch: (error) => new ProbeFleetTelemetryError({ reason: `Omega telemetry request failed: ${String(error)}` }),
    });

    if (!response.ok) {
      return yield* Effect.fail(
        new ProbeFleetTelemetryError({
          reason: `Omega telemetry request failed with HTTP ${response.status}`,
          statusCode: response.status,
        }),
      );
    }
  });
}
