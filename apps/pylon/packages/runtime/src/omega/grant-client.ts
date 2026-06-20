import { Effect, Schema as S } from "effect";
import {
  CHATGPT_CODEX_PROVIDER,
  ChatGptCodexProvider,
  GOOGLE_GEMINI_PROVIDER,
  GoogleGeminiProvider,
  ProbeProvider,
  ProviderAccountRef,
  ProviderAuthGrantRef,
  ProviderSecretRef,
  validateProbePublicProjection,
  type ProbePublicProjectionUnsafe,
} from "../contracts/provider-account.js";
import { requireAssignmentGrantRefs, type ProbeRunAssignment } from "../contracts/assignment.js";

export const ProbeMaterializationTarget = S.Union([
  S.Struct({
    kind: S.Literal("env"),
    name: S.Literal("PROBE_CHATGPT_AUTH_CONTENT"),
  }),
  S.Struct({
    kind: S.Literal("file"),
    relativePath: S.String,
  }),
]);
export type ProbeMaterializationTarget = typeof ProbeMaterializationTarget.Type;

export const ProbeChatGptAuthMaterializationPlan = S.Struct({
  kind: S.Literal("probe_chatgpt_auth"),
  provider: ChatGptCodexProvider,
  providerSecretRef: ProviderSecretRef,
  target: ProbeMaterializationTarget,
  homeIsolation: S.Literal("per_run"),
  scrubAfterCloseout: S.Boolean,
});
export type ProbeChatGptAuthMaterializationPlan = typeof ProbeChatGptAuthMaterializationPlan.Type;

export const ProbeGeminiApiKeyMaterializationTarget = S.Struct({
  kind: S.Literal("env"),
  name: S.Literal("GOOGLE_GENERATIVE_AI_API_KEY"),
});
export type ProbeGeminiApiKeyMaterializationTarget = typeof ProbeGeminiApiKeyMaterializationTarget.Type;

export const ProbeGeminiApiKeyMaterializationPlan = S.Struct({
  kind: S.Literal("probe_gemini_api_key"),
  provider: GoogleGeminiProvider,
  providerSecretRef: ProviderSecretRef,
  target: ProbeGeminiApiKeyMaterializationTarget,
  homeIsolation: S.Literal("per_run"),
  scrubAfterCloseout: S.Boolean,
});
export type ProbeGeminiApiKeyMaterializationPlan = typeof ProbeGeminiApiKeyMaterializationPlan.Type;

export const ProbeAuthMaterializationPlan = S.Union([
  ProbeChatGptAuthMaterializationPlan,
  ProbeGeminiApiKeyMaterializationPlan,
]);
export type ProbeAuthMaterializationPlan = typeof ProbeAuthMaterializationPlan.Type;

export const OmegaResolvedAuthGrantStatus = S.Literals(["issued", "used", "expired", "revoked", "failed"]);
export type OmegaResolvedAuthGrantStatus = typeof OmegaResolvedAuthGrantStatus.Type;

export const OmegaResolvedAuthGrant = S.Struct({
  grantRef: ProviderAuthGrantRef,
  provider: ProbeProvider,
  providerAccountRef: ProviderAccountRef,
  providerSecretRef: ProviderSecretRef,
  requestedAction: S.optional(S.String),
  runnerSessionId: S.optional(S.String),
  expiresAt: S.String,
  status: OmegaResolvedAuthGrantStatus,
  materialization: ProbeAuthMaterializationPlan,
});
export type OmegaResolvedAuthGrant = typeof OmegaResolvedAuthGrant.Type;

export class ProbeAuthGrantResolveError extends S.TaggedErrorClass<ProbeAuthGrantResolveError>()(
  "ProbeAuthGrantResolveError",
  {
    reason: S.String,
    statusCode: S.optional(S.Number),
  },
) {}

export class ProbeAuthGrantMismatch extends S.TaggedErrorClass<ProbeAuthGrantMismatch>()("ProbeAuthGrantMismatch", {
  field: S.String,
  expected: S.String,
  actual: S.String,
}) {}

export class ProbeAuthGrantExpired extends S.TaggedErrorClass<ProbeAuthGrantExpired>()("ProbeAuthGrantExpired", {
  grantRef: S.String,
  expiresAt: S.String,
}) {}

export type ProbeAuthGrantError =
  | ProbeAuthGrantResolveError
  | ProbeAuthGrantMismatch
  | ProbeAuthGrantExpired
  | ProbePublicProjectionUnsafe;

export interface OmegaGrantResolver {
  readonly resolveGrant: (assignment: ProbeRunAssignment) => Effect.Effect<OmegaResolvedAuthGrant, ProbeAuthGrantError>;
}

export interface OmegaGrantClientOptions {
  readonly baseUrl: string;
  readonly bearerToken?: string;
  readonly fetch?: typeof fetch;
}

export function makeOmegaGrantResolver(options: OmegaGrantClientOptions): OmegaGrantResolver {
  return {
    resolveGrant: (assignment) => resolveOmegaAuthGrant(options, assignment),
  };
}

// #4999 — Vortex-independent Codex grant-resolution endpoint contract.
//
// The Vortex *credential / endpoint* is fine to keep using, but the Vortex
// *codebase* is deprecated. Pylon-originated cloud sessions must therefore reach
// grant resolution through a neutral, Vortex-independent endpoint contract.
//
// Canonical neutral env var:  OA_CODEX_GRANT_RESOLVE_URL
// Legacy fallback env var:    PROBE_OMEGA_BASE_URL (kept for backward compat)
//
// Both name the *base URL* of the grant-resolution service. The provider-scoped
// path (`/api/provider-accounts/<provider>/grants/resolve`) is appended by
// `resolveOmegaAuthGrant`. The cloud provider (#4997) consumes this contract via
// `makeOmegaGrantResolverFromEnv` so it never imports anything Vortex-specific.
export const OA_CODEX_GRANT_RESOLVE_URL_ENV = "OA_CODEX_GRANT_RESOLVE_URL" as const;
export const LEGACY_OMEGA_BASE_URL_ENV = "PROBE_OMEGA_BASE_URL" as const;
export const OA_CODEX_GRANT_RESOLVE_BEARER_TOKEN_ENV = "OA_CODEX_GRANT_RESOLVE_TOKEN" as const;
export const LEGACY_OMEGA_BEARER_TOKEN_ENV = "PROBE_OMEGA_BEARER_TOKEN" as const;
export const DEFAULT_GRANT_RESOLVE_BASE_URL = "https://openagents.com" as const;

export type GrantResolveBaseUrlSource =
  | typeof OA_CODEX_GRANT_RESOLVE_URL_ENV
  | typeof LEGACY_OMEGA_BASE_URL_ENV
  | "default";

export interface ResolvedGrantResolveEndpoint {
  readonly baseUrl: string;
  readonly baseUrlSource: GrantResolveBaseUrlSource;
  readonly bearerToken?: string;
}

function isNonEmpty(value: string | undefined): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

// Prefer the neutral OA_CODEX_GRANT_RESOLVE_URL; fall back to the legacy
// Vortex-era PROBE_OMEGA_BASE_URL when the neutral one is unset; finally fall
// back to the public openagents.com default. The bearer token follows the same
// preference order.
export function resolveCodexGrantResolveEndpoint(
  env: Readonly<Record<string, string | undefined>> = {},
): ResolvedGrantResolveEndpoint {
  const neutral = env[OA_CODEX_GRANT_RESOLVE_URL_ENV];
  const legacy = env[LEGACY_OMEGA_BASE_URL_ENV];
  const bearerToken =
    env[OA_CODEX_GRANT_RESOLVE_BEARER_TOKEN_ENV] ?? env[LEGACY_OMEGA_BEARER_TOKEN_ENV];

  if (isNonEmpty(neutral)) {
    return {
      baseUrl: neutral,
      baseUrlSource: OA_CODEX_GRANT_RESOLVE_URL_ENV,
      ...(isNonEmpty(bearerToken) ? { bearerToken } : {}),
    };
  }

  if (isNonEmpty(legacy)) {
    return {
      baseUrl: legacy,
      baseUrlSource: LEGACY_OMEGA_BASE_URL_ENV,
      ...(isNonEmpty(bearerToken) ? { bearerToken } : {}),
    };
  }

  return {
    baseUrl: DEFAULT_GRANT_RESOLVE_BASE_URL,
    baseUrlSource: "default",
    ...(isNonEmpty(bearerToken) ? { bearerToken } : {}),
  };
}

// Build a grant resolver from the environment using the neutral, Vortex-
// independent endpoint contract. This is the entry point the cloud provider
// (#4997) should use; it must not import any Vortex-specific module.
export function makeOmegaGrantResolverFromEnv(
  env: Readonly<Record<string, string | undefined>> = {},
  fetchImpl?: typeof fetch,
): OmegaGrantResolver {
  const endpoint = resolveCodexGrantResolveEndpoint(env);
  return makeOmegaGrantResolver({
    baseUrl: endpoint.baseUrl,
    ...(endpoint.bearerToken === undefined ? {} : { bearerToken: endpoint.bearerToken }),
    ...(fetchImpl === undefined ? {} : { fetch: fetchImpl }),
  });
}

export function makeStaticOmegaGrantResolver(grant: unknown): OmegaGrantResolver {
  return {
    resolveGrant: (assignment) => validateResolvedAuthGrantForAssignment(grant, assignment),
  };
}

export function resolveOmegaAuthGrant(
  options: OmegaGrantClientOptions,
  assignment: ProbeRunAssignment,
): Effect.Effect<OmegaResolvedAuthGrant, ProbeAuthGrantError> {
  return Effect.gen(function* () {
    const assignmentWithGrant = yield* requireAssignmentGrantRefs(assignment).pipe(
      Effect.mapError((error) => new ProbeAuthGrantResolveError({ reason: error.reason })),
    );

    const endpoint = new URL(providerGrantResolvePath(assignmentWithGrant.provider), options.baseUrl);
    const fetchImpl = options.fetch ?? fetch;

    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...(options.bearerToken === undefined ? {} : { Authorization: `Bearer ${options.bearerToken}` }),
          },
          body: JSON.stringify({
            grantRef: assignmentWithGrant.authGrantRef,
            providerAccountRef: assignmentWithGrant.providerAccountRef,
            runnerSessionId: assignmentWithGrant.runnerSessionId,
          }),
        }),
      catch: (error) =>
        new ProbeAuthGrantResolveError({
          reason: `Omega grant resolve request failed: ${String(error)}`,
        }),
    });

    if (!response.ok) {
      return yield* Effect.fail(
        new ProbeAuthGrantResolveError({
          reason: `Omega grant resolve failed with HTTP ${response.status}`,
          statusCode: response.status,
        }),
      );
    }

    const payload = yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) =>
        new ProbeAuthGrantResolveError({
          reason: `Omega grant resolve returned invalid JSON: ${String(error)}`,
        }),
    });

    return yield* validateResolvedAuthGrantForAssignment(payload, assignmentWithGrant);
  });
}

export function validateResolvedAuthGrantForAssignment(
  payload: unknown,
  assignment: ProbeRunAssignment,
  now: Date = new Date(),
): Effect.Effect<OmegaResolvedAuthGrant, ProbeAuthGrantError> {
  return Effect.gen(function* () {
    const assignmentWithGrant = yield* requireAssignmentGrantRefs(assignment).pipe(
      Effect.mapError((error) => new ProbeAuthGrantResolveError({ reason: error.reason })),
    );

    yield* validateProbePublicProjection(payload, "grant");

    const grant = yield* S.decodeUnknownEffect(OmegaResolvedAuthGrant)(payload).pipe(
      Effect.mapError(
        (error) =>
          new ProbeAuthGrantResolveError({
            reason: `Omega grant payload did not match Probe contract: ${String(error)}`,
          }),
      ),
    );

    if (grant.provider !== assignmentWithGrant.provider) {
      return yield* Effect.fail(
        new ProbeAuthGrantMismatch({
          field: "provider",
          expected: assignmentWithGrant.provider,
          actual: grant.provider,
        }),
      );
    }

    if (grant.grantRef !== assignmentWithGrant.authGrantRef) {
      return yield* Effect.fail(
        new ProbeAuthGrantMismatch({
          field: "grantRef",
          expected: assignmentWithGrant.authGrantRef,
          actual: grant.grantRef,
        }),
      );
    }

    if (grant.providerAccountRef !== assignmentWithGrant.providerAccountRef) {
      return yield* Effect.fail(
        new ProbeAuthGrantMismatch({
          field: "providerAccountRef",
          expected: assignmentWithGrant.providerAccountRef,
          actual: grant.providerAccountRef,
        }),
      );
    }

    if (grant.runnerSessionId !== undefined && grant.runnerSessionId !== assignmentWithGrant.runnerSessionId) {
      return yield* Effect.fail(
        new ProbeAuthGrantMismatch({
          field: "runnerSessionId",
          expected: assignmentWithGrant.runnerSessionId,
          actual: grant.runnerSessionId,
        }),
      );
    }

    if (grant.status === "expired" || grant.status === "revoked" || grant.status === "failed") {
      return yield* Effect.fail(
        new ProbeAuthGrantResolveError({
          reason: `Omega grant is not usable: ${grant.status}`,
        }),
      );
    }

    if (Number.isNaN(Date.parse(grant.expiresAt)) || Date.parse(grant.expiresAt) <= now.getTime()) {
      return yield* Effect.fail(new ProbeAuthGrantExpired({ grantRef: grant.grantRef, expiresAt: grant.expiresAt }));
    }

    if (grant.materialization.providerSecretRef !== grant.providerSecretRef) {
      return yield* Effect.fail(
        new ProbeAuthGrantMismatch({
          field: "materialization.providerSecretRef",
          expected: grant.providerSecretRef,
          actual: grant.materialization.providerSecretRef,
        }),
      );
    }

    if (grant.materialization.provider !== grant.provider) {
      return yield* Effect.fail(
        new ProbeAuthGrantMismatch({
          field: "materialization.provider",
          expected: grant.provider,
          actual: grant.materialization.provider,
        }),
      );
    }

    return grant;
  });
}

function providerGrantResolvePath(provider: ProbeProvider): string {
  return provider === GOOGLE_GEMINI_PROVIDER
    ? "/api/provider-accounts/google-gemini/grants/resolve"
    : "/api/provider-accounts/chatgpt-codex/grants/resolve";
}
