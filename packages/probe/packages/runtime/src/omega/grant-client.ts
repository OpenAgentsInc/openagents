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
} from "../contracts/provider-account";
import { requireAssignmentGrantRefs, type ProbeRunAssignment } from "../contracts/assignment";

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
