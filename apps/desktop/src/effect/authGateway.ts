import { Context, Effect, Layer, Schema } from "effect";

import { DesktopConfigService } from "./config";

type AuthUser = Readonly<{
  readonly id: string;
  readonly email: string | null;
  readonly firstName: string | null;
  readonly lastName: string | null;
}>;

export type VerifyResult = Readonly<{
  readonly userId: string;
  readonly token: string;
  readonly user: AuthUser | null;
}>;

export type SessionResult = Readonly<{
  readonly userId: string | null;
  readonly token: string | null;
  readonly user: AuthUser | null;
}>;

export class AuthGatewayTransportError extends Schema.TaggedError<AuthGatewayTransportError>()(
  "AuthGatewayTransportError",
  {
    operation: Schema.String,
    error: Schema.Defect,
  },
) {}

export class AuthGatewayHttpError extends Schema.TaggedError<AuthGatewayHttpError>()("AuthGatewayHttpError", {
  operation: Schema.String,
  status: Schema.Number,
  error: Schema.String,
}) {}

export class AuthGatewayApiError extends Schema.TaggedError<AuthGatewayApiError>()("AuthGatewayApiError", {
  operation: Schema.String,
  error: Schema.String,
}) {}

export type AuthGatewayError = AuthGatewayTransportError | AuthGatewayHttpError | AuthGatewayApiError;

export type AuthGatewayApi = Readonly<{
  readonly startMagicCode: (email: string) => Effect.Effect<void, AuthGatewayError>;
  readonly verifyMagicCode: (input: {
    readonly email: string;
    readonly code: string;
  }) => Effect.Effect<VerifyResult, AuthGatewayError>;
  readonly getSession: (token: string | null) => Effect.Effect<SessionResult, AuthGatewayError>;
}>;

export class AuthGatewayService extends Context.Tag("@openagents/desktop/AuthGatewayService")<
  AuthGatewayService,
  AuthGatewayApi
>() {}

const requestJson = Effect.fn("AuthGateway.requestJson")(function* (input: {
  readonly operation: string;
  readonly url: string;
  readonly method: "GET" | "POST";
  readonly body?: unknown;
  readonly token?: string | null;
  readonly headers?: Readonly<Record<string, string>>;
}) {
  const headers = new Headers({ accept: "application/json" });
  if (input.body !== undefined) headers.set("content-type", "application/json");
  if (input.token) headers.set("authorization", `Bearer ${input.token}`);
  for (const [key, value] of Object.entries(input.headers ?? {})) {
    headers.set(key, value);
  }

  const response = yield* Effect.tryPromise({
    try: () => {
      const init: RequestInit = {
        method: input.method,
        headers,
        credentials: "include",
        cache: "no-store",
      };
      if (input.body !== undefined) init.body = JSON.stringify(input.body);
      return fetch(input.url, init);
    },
    catch: (error) => AuthGatewayTransportError.make({ operation: input.operation, error }),
  });

  const json = yield* Effect.tryPromise({
    try: () => response.json(),
    catch: (error) => AuthGatewayTransportError.make({ operation: `${input.operation}.json`, error }),
  });

  return { response, json };
});

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

const normalizeUser = (value: unknown): AuthUser | null => {
  const rec = asRecord(value);
  if (!rec || typeof rec.id !== "string") return null;
  return {
    id: rec.id,
    email: typeof rec.email === "string" ? rec.email : null,
    firstName: typeof rec.firstName === "string" ? rec.firstName : null,
    lastName: typeof rec.lastName === "string" ? rec.lastName : null,
  };
};

const failOnHttp = (operation: string, status: number, value: unknown): Effect.Effect<never, AuthGatewayHttpError> => {
  const rec = asRecord(value);
  const message = typeof rec?.error === "string" ? rec.error : `HTTP ${status}`;
  return Effect.fail(AuthGatewayHttpError.make({ operation, status, error: message }));
};

export const AuthGatewayLive = Layer.effect(
  AuthGatewayService,
  Effect.gen(function* () {
    const cfg = yield* DesktopConfigService;

    const startMagicCode = Effect.fn("AuthGateway.startMagicCode")(function* (email: string) {
      const { response, json } = yield* requestJson({
        operation: "startMagicCode",
        method: "POST",
        url: `${cfg.openAgentsBaseUrl}/api/auth/email`,
        body: { email },
        headers: { "x-client": "openagents-desktop" },
      });
      if (!response.ok) return yield* failOnHttp("startMagicCode", response.status, json);

      const rec = asRecord(json);
      if (rec?.ok === true) return;

      return yield* Effect.fail(
        AuthGatewayApiError.make({
          operation: "startMagicCode",
          error: typeof rec?.error === "string" ? rec.error : "unexpected_response",
        }),
      );
    });

    const verifyMagicCode = Effect.fn("AuthGateway.verifyMagicCode")(function* (input: {
      readonly email: string;
      readonly code: string;
    }) {
      const { response, json } = yield* requestJson({
        operation: "verifyMagicCode",
        method: "POST",
        url: `${cfg.openAgentsBaseUrl}/api/auth/verify`,
        body: input,
        headers: { "x-client": "openagents-desktop" },
      });
      if (!response.ok) return yield* failOnHttp("verifyMagicCode", response.status, json);

      const rec = asRecord(json);
      if (rec?.ok !== true || typeof rec.userId !== "string") {
        return yield* Effect.fail(
          AuthGatewayApiError.make({
            operation: "verifyMagicCode",
            error: typeof rec?.error === "string" ? rec.error : "unexpected_response",
          }),
        );
      }

      const token = typeof rec.token === "string" ? rec.token.trim() : "";
      if (token.length === 0) {
        return yield* Effect.fail(
          AuthGatewayApiError.make({
            operation: "verifyMagicCode",
            error: "token_missing",
          }),
        );
      }

      return {
        userId: rec.userId,
        token,
        user: normalizeUser(rec.user),
      } satisfies VerifyResult;
    });

    const getSession = Effect.fn("AuthGateway.getSession")(function* (token: string | null) {
      const { response, json } = yield* requestJson({
        operation: "getSession",
        method: "GET",
        url: `${cfg.openAgentsBaseUrl}/api/auth/session`,
        token,
      });
      if (!response.ok) return yield* failOnHttp("getSession", response.status, json);

      const rec = asRecord(json);
      if (rec?.ok !== true) {
        return yield* Effect.fail(
          AuthGatewayApiError.make({
            operation: "getSession",
            error: typeof rec?.error === "string" ? rec.error : "unexpected_response",
          }),
        );
      }

      return {
        userId: typeof rec.userId === "string" ? rec.userId : null,
        token: typeof rec.token === "string" ? rec.token : null,
        user: normalizeUser(rec.user),
      } satisfies SessionResult;
    });

    return AuthGatewayService.of({
      startMagicCode,
      verifyMagicCode,
      getSession,
    });
  }),
);
