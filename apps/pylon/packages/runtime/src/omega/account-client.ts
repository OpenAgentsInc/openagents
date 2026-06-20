import { Effect, Schema as S } from "effect";
import { PublicProviderAccount, validateProbePublicProjection, type ProbePublicProjectionUnsafe } from "../contracts/provider-account.js";

export const OmegaProviderAccountsResponse = S.Struct({
  accounts: S.Array(PublicProviderAccount),
  pendingAttempts: S.optional(S.Array(S.Record(S.String, S.Unknown))),
});
export type OmegaProviderAccountsResponse = typeof OmegaProviderAccountsResponse.Type;

export const OmegaDeviceLoginStart = S.Struct({
  attemptId: S.String,
  providerAccountRef: S.String,
  verificationUrl: S.String,
  userCode: S.String,
  expiresAt: S.String,
  intervalSeconds: S.optional(S.Number),
});
export type OmegaDeviceLoginStart = typeof OmegaDeviceLoginStart.Type;

export const OmegaDeviceLoginAttempt = S.Struct({
  attemptId: S.String,
  status: S.Literals(["pending", "connected", "expired", "denied", "failed"]),
  providerAccountRef: S.String,
  accountLabel: S.optional(S.String),
  expiresAt: S.optional(S.String),
});
export type OmegaDeviceLoginAttempt = typeof OmegaDeviceLoginAttempt.Type;

export class OmegaAccountClientError extends S.TaggedErrorClass<OmegaAccountClientError>()("OmegaAccountClientError", {
  reason: S.String,
  statusCode: S.optional(S.Number),
}) {}

export interface OmegaAccountClient {
  readonly listProviderAccounts: () => Effect.Effect<
    OmegaProviderAccountsResponse,
    OmegaAccountClientError | ProbePublicProjectionUnsafe
  >;
  readonly startChatGptDeviceLogin: (input?: {
    readonly createNew?: boolean;
    readonly providerAccountRef?: string;
  }) => Effect.Effect<OmegaDeviceLoginStart, OmegaAccountClientError | ProbePublicProjectionUnsafe>;
  readonly readChatGptDeviceLogin: (
    attemptId: string,
  ) => Effect.Effect<OmegaDeviceLoginAttempt, OmegaAccountClientError | ProbePublicProjectionUnsafe>;
}

export interface OmegaAccountClientOptions {
  readonly baseUrl: string;
  readonly bearerToken?: string;
  readonly fetch?: typeof fetch;
}

export function makeOmegaAccountClient(options: OmegaAccountClientOptions): OmegaAccountClient {
  return {
    listProviderAccounts: (): Effect.Effect<
      OmegaProviderAccountsResponse,
      OmegaAccountClientError | ProbePublicProjectionUnsafe
    > =>
      requestOmegaJson(options, "/api/provider-accounts", "GET").pipe(
        Effect.flatMap((payload) => decodeOmegaPayload(OmegaProviderAccountsResponse, payload, "provider accounts")),
        Effect.tap((payload) => validateProbePublicProjection(payload, "providerAccounts")),
      ),
    startChatGptDeviceLogin: (input = {}) =>
      requestOmegaJson(options, "/api/provider-accounts/chatgpt-codex/device-login/start", "POST", {
        createNew: input.createNew ?? true,
        providerAccountRef: input.providerAccountRef,
      }).pipe(
        Effect.flatMap((payload) => decodeOmegaPayload(OmegaDeviceLoginStart, payload, "device login start")),
        Effect.tap((payload) => validateProbePublicProjection(payload, "deviceLoginStart")),
      ),
    readChatGptDeviceLogin: (attemptId) =>
      requestOmegaJson(options, `/api/provider-accounts/chatgpt-codex/device-login/${encodeURIComponent(attemptId)}`, "GET").pipe(
        Effect.flatMap((payload) => decodeOmegaPayload(OmegaDeviceLoginAttempt, payload, "device login attempt")),
        Effect.tap((payload) => validateProbePublicProjection(payload, "deviceLoginAttempt")),
      ),
  };
}

export function makeStaticOmegaAccountClient(fixture: {
  readonly accounts?: OmegaProviderAccountsResponse;
  readonly startedLogin?: OmegaDeviceLoginStart;
  readonly attempt?: OmegaDeviceLoginAttempt;
}): OmegaAccountClient {
  return {
    listProviderAccounts: () => Effect.succeed(fixture.accounts ?? { accounts: [] }),
    startChatGptDeviceLogin: () =>
      fixture.startedLogin === undefined
        ? Effect.fail(new OmegaAccountClientError({ reason: "static device login fixture missing" }))
        : Effect.succeed(fixture.startedLogin),
    readChatGptDeviceLogin: () =>
      fixture.attempt === undefined
        ? Effect.fail(new OmegaAccountClientError({ reason: "static device login attempt fixture missing" }))
        : Effect.succeed(fixture.attempt),
  };
}

function requestOmegaJson(
  options: OmegaAccountClientOptions,
  path: string,
  method: "GET" | "POST",
  body?: unknown,
): Effect.Effect<unknown, OmegaAccountClientError> {
  return Effect.gen(function* () {
    const endpoint = new URL(path, options.baseUrl);
    const fetchImpl = options.fetch ?? fetch;

    const response = yield* Effect.tryPromise({
      try: () =>
        fetchImpl(endpoint, {
          method,
          headers: {
            ...(body === undefined ? {} : { "Content-Type": "application/json" }),
            ...(options.bearerToken === undefined ? {} : { Authorization: `Bearer ${options.bearerToken}` }),
          },
          body: body === undefined ? undefined : JSON.stringify(body),
        }),
      catch: (error) => new OmegaAccountClientError({ reason: `Omega request failed: ${String(error)}` }),
    });

    if (!response.ok) {
      return yield* Effect.fail(
        new OmegaAccountClientError({
          reason: `Omega request failed with HTTP ${response.status}`,
          statusCode: response.status,
        }),
      );
    }

    return yield* Effect.tryPromise({
      try: () => response.json(),
      catch: (error) => new OmegaAccountClientError({ reason: `Omega returned invalid JSON: ${String(error)}` }),
    });
  });
}

// These Omega contract schemas are pure (no service-dependent decoders), so the
// decode requires no Effect services. We pin the requirements channel to `never`
// because `decodeUnknownEffect` surfaces the schema's `DecodingServices`, which
// for deeply-branded structs is inferred as the `unknown` upper bound rather
// than the actual `never`.
function decodeOmegaPayload<Sch extends S.Top>(
  schema: Sch,
  payload: unknown,
  label: string,
): Effect.Effect<Sch["Type"], OmegaAccountClientError> {
  return S.decodeUnknownEffect(schema)(payload).pipe(
    Effect.mapError((error) => new OmegaAccountClientError({ reason: `invalid ${label} payload: ${String(error)}` })),
  ) as Effect.Effect<Sch["Type"], OmegaAccountClientError>;
}
