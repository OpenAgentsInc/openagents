import { Context, Effect, Layer, Schema } from "effect";

import { RequestContextService } from "./requestContext";

export class HomeApiTransportError extends Schema.TaggedError<HomeApiTransportError>()("HomeApiTransportError", {
  operation: Schema.String,
  error: Schema.Defect,
}) {}

export class HomeApiHttpError extends Schema.TaggedError<HomeApiHttpError>()("HomeApiHttpError", {
  operation: Schema.String,
  status: Schema.Number,
  error: Schema.Defect,
}) {}

export class HomeApiDecodeError extends Schema.TaggedError<HomeApiDecodeError>()("HomeApiDecodeError", {
  operation: Schema.String,
  error: Schema.Defect,
}) {}

export class HomeApiRejectedError extends Schema.TaggedError<HomeApiRejectedError>()("HomeApiRejectedError", {
  operation: Schema.String,
  reason: Schema.String,
}) {}

export type HomeApiError =
  | HomeApiTransportError
  | HomeApiHttpError
  | HomeApiDecodeError
  | HomeApiRejectedError;

export type HomeApiUser = {
  readonly id: string;
  readonly email: string | null;
  readonly firstName: string | null;
  readonly lastName: string | null;
};

export type HomeVerifySuccess = {
  readonly userId: string | null;
  readonly token: string | null;
  readonly user: HomeApiUser | null;
};

export type HomeAuthSession = {
  readonly userId: string;
  readonly token: string | null;
  readonly user: HomeApiUser | null;
};

export type HomeDseRecapInput = {
  readonly threadId: string;
  readonly strategyId: "direct.v1" | "rlm_lite.v1";
  readonly budgetProfile: "small" | "medium" | "long";
  readonly question: string;
  readonly e2eMode?: "stub" | "off";
};

export type HomeDseRecapResult = {
  readonly threadId: string;
  readonly runId: string;
  readonly assistantMessageId: string;
};

export type HomeApi = {
  readonly startMagicCode: (input: {
    readonly email: string;
  }) => Effect.Effect<void, HomeApiError, RequestContextService>;
  readonly verifyMagicCode: (input: {
    readonly email: string;
    readonly code: string;
  }) => Effect.Effect<HomeVerifySuccess, HomeApiError, RequestContextService>;
  readonly getAuthSession: () => Effect.Effect<HomeAuthSession | null, HomeApiError, RequestContextService>;
  readonly runDseRecap: (input: HomeDseRecapInput) => Effect.Effect<HomeDseRecapResult, HomeApiError, RequestContextService>;
};

export class HomeApiService extends Context.Tag("@openagents/web/HomeApi")<HomeApiService, HomeApi>() {}

const HomeApiUserSchema = Schema.Struct({
  id: Schema.String,
  email: Schema.optional(Schema.NullOr(Schema.String)),
  firstName: Schema.optional(Schema.NullOr(Schema.String)),
  lastName: Schema.optional(Schema.NullOr(Schema.String)),
});

type HomeApiUserSchemaType = typeof HomeApiUserSchema.Type;

const AuthStartResponseSchema = Schema.Union(
  Schema.Struct({
    ok: Schema.Literal(true),
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    error: Schema.optional(Schema.String),
  }),
);

type AuthStartResponse = typeof AuthStartResponseSchema.Type;

const AuthVerifyResponseSchema = Schema.Union(
  Schema.Struct({
    ok: Schema.Literal(true),
    userId: Schema.optional(Schema.NullOr(Schema.String)),
    token: Schema.optional(Schema.NullOr(Schema.String)),
    user: Schema.optional(Schema.NullOr(HomeApiUserSchema)),
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    error: Schema.optional(Schema.String),
  }),
);

type AuthVerifyResponse = typeof AuthVerifyResponseSchema.Type;

const AuthSessionResponseSchema = Schema.Union(
  Schema.Struct({
    ok: Schema.Literal(true),
    userId: Schema.optional(Schema.NullOr(Schema.String)),
    token: Schema.optional(Schema.NullOr(Schema.String)),
    user: Schema.optional(Schema.NullOr(HomeApiUserSchema)),
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    error: Schema.optional(Schema.String),
  }),
);

type AuthSessionResponse = typeof AuthSessionResponseSchema.Type;

const DseRecapResponseSchema = Schema.Union(
  Schema.Struct({
    ok: Schema.Literal(true),
    threadId: Schema.String,
    runId: Schema.String,
    assistantMessageId: Schema.String,
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    error: Schema.optional(Schema.String),
  }),
);

type DseRecapResponse = typeof DseRecapResponseSchema.Type;

const decodeWithSchema = <A>(
  input: {
    readonly operation: string;
    readonly schema: Schema.Schema<A, any>;
    readonly value: unknown;
  },
): Effect.Effect<A, HomeApiDecodeError> =>
  Schema.decodeUnknown(input.schema)(input.value).pipe(
    Effect.mapError((error) =>
      HomeApiDecodeError.make({
        operation: `${input.operation}.decode`,
        error,
      })),
  );

const normalizeUser = (user: HomeApiUserSchemaType | null | undefined): HomeApiUser | null => {
  if (!user) return null;
  return {
    id: user.id,
    email: user.email ?? null,
    firstName: user.firstName ?? null,
    lastName: user.lastName ?? null,
  };
};

const requestJson = Effect.fn("HomeApi.requestJson")(function* (input: {
  readonly operation: string;
  readonly url: string;
  readonly method: "GET" | "POST";
  readonly body?: unknown;
  readonly headers?: Readonly<Record<string, string>>;
}) {
  const ctx = yield* RequestContextService;
  const url = ctx._tag === "Server" ? new URL(input.url, ctx.request.url).toString() : input.url;
  const headers = new Headers({ accept: "application/json" });
  if (input.body !== undefined) headers.set("content-type", "application/json");
  if (ctx._tag === "Server") {
    const cookie = ctx.request.headers.get("cookie");
    if (cookie) headers.set("cookie", cookie);
    const authorization = ctx.request.headers.get("authorization");
    if (authorization) headers.set("authorization", authorization);
  }
  if (input.headers) {
    for (const [key, value] of Object.entries(input.headers)) {
      headers.set(key, value);
    }
  }

  const response = yield* Effect.tryPromise({
    try: () =>
      fetch(url, {
        method: input.method,
        cache: "no-store",
        credentials: "include",
        headers,
        body: input.body === undefined ? undefined : JSON.stringify(input.body),
      }),
    catch: (error) =>
      HomeApiTransportError.make({
        operation: `${input.operation}.fetch`,
        error,
      }),
  });

  const json = yield* Effect.tryPromise({
    try: () => response.json(),
    catch: (error) =>
      HomeApiDecodeError.make({
        operation: `${input.operation}.json`,
        error,
      }),
  });

  return {
    ok: response.ok,
    status: response.status,
    json,
  };
});

const startMagicCode = Effect.fn("HomeApi.startMagicCode")(function* (input: {
      readonly email: string;
    }) {
  const response = yield* requestJson({
    operation: "startMagicCode",
    url: "/api/auth/start",
    method: "POST",
    body: { email: input.email },
  });

  const decoded = yield* decodeWithSchema<AuthStartResponse>({
    operation: "startMagicCode",
    schema: AuthStartResponseSchema,
    value: response.json,
  });

  if (!decoded.ok) {
    return yield* Effect.fail(
      HomeApiRejectedError.make({
        operation: "startMagicCode",
        reason: decoded.error ?? "request_failed",
      }),
    );
  }
  if (!response.ok) {
    return yield* Effect.fail(
      HomeApiHttpError.make({
        operation: "startMagicCode",
        status: response.status,
        error: new Error(`HTTP ${response.status}`),
      }),
    );
  }
});

const verifyMagicCode = Effect.fn("HomeApi.verifyMagicCode")(function* (input: {
      readonly email: string;
      readonly code: string;
    }) {
  const response = yield* requestJson({
    operation: "verifyMagicCode",
    url: "/api/auth/verify",
    method: "POST",
    body: { email: input.email, code: input.code },
  });

  const decoded = yield* decodeWithSchema<AuthVerifyResponse>({
    operation: "verifyMagicCode",
    schema: AuthVerifyResponseSchema,
    value: response.json,
  });

  if (!decoded.ok) {
    return yield* Effect.fail(
      HomeApiRejectedError.make({
        operation: "verifyMagicCode",
        reason: decoded.error ?? "request_failed",
      }),
    );
  }
  if (!response.ok) {
    return yield* Effect.fail(
      HomeApiHttpError.make({
        operation: "verifyMagicCode",
        status: response.status,
        error: new Error(`HTTP ${response.status}`),
      }),
    );
  }

  return {
    userId: decoded.userId ?? null,
    token: decoded.token ?? null,
    user: normalizeUser(decoded.user),
  } satisfies HomeVerifySuccess;
});

const getAuthSession = Effect.fn("HomeApi.getAuthSession")(function* () {
  const response = yield* requestJson({
    operation: "getAuthSession",
    url: "/api/auth/session",
    method: "GET",
  });

  if (response.status >= 400) {
    return yield* Effect.fail(
      HomeApiHttpError.make({
        operation: "getAuthSession",
        status: response.status,
        error: new Error(`HTTP ${response.status}`),
      }),
    );
  }

  const decoded = yield* decodeWithSchema<AuthSessionResponse>({
    operation: "getAuthSession",
    schema: AuthSessionResponseSchema,
    value: response.json,
  });

  if (!decoded.ok) return null;
  const userId = decoded.userId ?? null;
  if (!userId) return null;

  return {
    userId,
    token: decoded.token ?? null,
    user: normalizeUser(decoded.user),
  } satisfies HomeAuthSession;
});

const runDseRecap = Effect.fn("HomeApi.runDseRecap")(function* (input: HomeDseRecapInput) {
  const response = yield* requestJson({
    operation: "runDseRecap",
    url: "/api/autopilot/dse/recap",
    method: "POST",
    headers: input.e2eMode === "stub" ? { "x-oa-e2e-mode": "stub" } : undefined,
    body: {
      threadId: input.threadId,
      strategyId: input.strategyId,
      budgetProfile: input.budgetProfile,
      question: input.question,
    },
  });

  const decoded = yield* decodeWithSchema<DseRecapResponse>({
    operation: "runDseRecap",
    schema: DseRecapResponseSchema,
    value: response.json,
  });

  if (!decoded.ok) {
    return yield* Effect.fail(
      HomeApiRejectedError.make({
        operation: "runDseRecap",
        reason: decoded.error ?? "request_failed",
      }),
    );
  }
  if (!response.ok) {
    return yield* Effect.fail(
      HomeApiHttpError.make({
        operation: "runDseRecap",
        status: response.status,
        error: new Error(`HTTP ${response.status}`),
      }),
    );
  }

  return {
    threadId: decoded.threadId,
    runId: decoded.runId,
    assistantMessageId: decoded.assistantMessageId,
  } satisfies HomeDseRecapResult;
});

export const HomeApiLive = Layer.succeed(
  HomeApiService,
  HomeApiService.of({
    startMagicCode,
    verifyMagicCode,
    getAuthSession,
    runDseRecap,
  }),
);
