import { L402FetchRequest } from "@openagentsinc/lightning-effect";
import { Context, Effect, Layer, Schema, Stream } from "effect";

import { api } from "../../convex/_generated/api";
import { ConvexService } from "./convex";
import { RequestContextService } from "./requestContext";
import { TelemetryService } from "./telemetry";

export class LightningApiError extends Schema.TaggedError<LightningApiError>()("LightningApiError", {
  operation: Schema.String,
  status: Schema.optional(Schema.Number),
  error: Schema.Defect,
}) {}

export const LightningTaskStatus = Schema.Literal(
  "queued",
  "approved",
  "running",
  "paid",
  "cached",
  "blocked",
  "failed",
  "completed",
);
export type LightningTaskStatus = typeof LightningTaskStatus.Type;

export const LightningTask = Schema.Struct({
  taskId: Schema.String,
  ownerId: Schema.String,
  status: LightningTaskStatus,
  request: L402FetchRequest,
  idempotencyKey: Schema.optional(Schema.String),
  source: Schema.optional(Schema.String),
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  attemptCount: Schema.Number,
  lastErrorCode: Schema.optional(Schema.String),
  lastErrorMessage: Schema.optional(Schema.String),
  createdAtMs: Schema.Number,
  updatedAtMs: Schema.Number,
  lastTransitionAtMs: Schema.Number,
});
export type LightningTask = typeof LightningTask.Type;

export const LightningPaywallStatus = Schema.Literal("active", "paused", "archived");
export type LightningPaywallStatus = typeof LightningPaywallStatus.Type;

export const LightningPaywallPolicy = Schema.Struct({
  paywallId: Schema.String,
  ownerId: Schema.String,
  pricingMode: Schema.Literal("fixed"),
  fixedAmountMsats: Schema.Number,
  maxPerRequestMsats: Schema.optional(Schema.Number),
  allowedHosts: Schema.optional(Schema.Array(Schema.String)),
  blockedHosts: Schema.optional(Schema.Array(Schema.String)),
  quotaPerMinute: Schema.optional(Schema.Number),
  quotaPerDay: Schema.optional(Schema.Number),
  killSwitch: Schema.Boolean,
  createdAtMs: Schema.Number,
  updatedAtMs: Schema.Number,
});
export type LightningPaywallPolicy = typeof LightningPaywallPolicy.Type;

export const LightningPaywallRoute = Schema.Struct({
  routeId: Schema.String,
  paywallId: Schema.String,
  ownerId: Schema.String,
  hostPattern: Schema.String,
  pathPattern: Schema.String,
  upstreamUrl: Schema.String,
  protocol: Schema.Literal("http", "https"),
  timeoutMs: Schema.Number,
  priority: Schema.Number,
  createdAtMs: Schema.Number,
  updatedAtMs: Schema.Number,
});
export type LightningPaywallRoute = typeof LightningPaywallRoute.Type;

export const LightningPaywall = Schema.Struct({
  paywallId: Schema.String,
  ownerId: Schema.String,
  name: Schema.String,
  description: Schema.optional(Schema.String),
  status: LightningPaywallStatus,
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  createdAtMs: Schema.Number,
  updatedAtMs: Schema.Number,
  policy: LightningPaywallPolicy,
  routes: Schema.Array(LightningPaywallRoute),
});
export type LightningPaywall = typeof LightningPaywall.Type;

export const LightningSettlement = Schema.Struct({
  settlementId: Schema.String,
  paywallId: Schema.String,
  ownerId: Schema.String,
  invoiceId: Schema.optional(Schema.String),
  amountMsats: Schema.Number,
  paymentProofRef: Schema.String,
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  createdAtMs: Schema.Number,
});
export type LightningSettlement = typeof LightningSettlement.Type;

export const LightningDeploymentStatus = Schema.Literal("pending", "applied", "failed", "rolled_back");
export type LightningDeploymentStatus = typeof LightningDeploymentStatus.Type;

export const LightningGatewayDeployment = Schema.Struct({
  deploymentId: Schema.String,
  paywallId: Schema.optional(Schema.String),
  ownerId: Schema.optional(Schema.String),
  configHash: Schema.String,
  imageDigest: Schema.optional(Schema.String),
  status: LightningDeploymentStatus,
  diagnostics: Schema.optional(Schema.Unknown),
  appliedAtMs: Schema.optional(Schema.Number),
  rolledBackFrom: Schema.optional(Schema.String),
  createdAtMs: Schema.Number,
  updatedAtMs: Schema.Number,
});
export type LightningGatewayDeployment = typeof LightningGatewayDeployment.Type;

export const LightningGatewayEventLevel = Schema.Literal("info", "warn", "error");
export type LightningGatewayEventLevel = typeof LightningGatewayEventLevel.Type;

export const LightningGatewayEvent = Schema.Struct({
  eventId: Schema.String,
  paywallId: Schema.String,
  ownerId: Schema.String,
  eventType: Schema.String,
  level: LightningGatewayEventLevel,
  requestId: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
  createdAtMs: Schema.Number,
});
export type LightningGatewayEvent = typeof LightningGatewayEvent.Type;

const CreateTaskResponseSchema = Schema.Struct({
  ok: Schema.Literal(true),
  task: LightningTask,
  existed: Schema.Boolean,
  requestId: Schema.NullOr(Schema.String),
});
type CreateTaskResponse = typeof CreateTaskResponseSchema.Type;

const ConvexGetTaskResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  task: LightningTask,
});
type ConvexGetTaskResult = typeof ConvexGetTaskResultSchema.Type;

const ConvexListTasksResultSchema = Schema.Struct({
  ok: Schema.Literal(true),
  tasks: Schema.Array(LightningTask),
});
type ConvexListTasksResult = typeof ConvexListTasksResultSchema.Type;

const ListPaywallsResponseSchema = Schema.Struct({
  ok: Schema.Literal(true),
  paywalls: Schema.Array(LightningPaywall),
  requestId: Schema.NullOr(Schema.String),
});
type ListPaywallsResponse = typeof ListPaywallsResponseSchema.Type;

const GetPaywallResponseSchema = Schema.Struct({
  ok: Schema.Literal(true),
  paywall: LightningPaywall,
  requestId: Schema.NullOr(Schema.String),
});
type GetPaywallResponse = typeof GetPaywallResponseSchema.Type;

const ListSettlementsResponseSchema = Schema.Struct({
  ok: Schema.Literal(true),
  settlements: Schema.Array(LightningSettlement),
  nextCursor: Schema.NullOr(Schema.Number),
  requestId: Schema.NullOr(Schema.String),
});
type ListSettlementsResponse = typeof ListSettlementsResponseSchema.Type;

const ListDeploymentsResponseSchema = Schema.Struct({
  ok: Schema.Literal(true),
  deployments: Schema.Array(LightningGatewayDeployment),
  nextCursor: Schema.NullOr(Schema.Number),
  requestId: Schema.NullOr(Schema.String),
});
type ListDeploymentsResponse = typeof ListDeploymentsResponseSchema.Type;

const ListDeploymentEventsResponseSchema = Schema.Struct({
  ok: Schema.Literal(true),
  events: Schema.Array(LightningGatewayEvent),
  nextCursor: Schema.NullOr(Schema.Number),
  requestId: Schema.NullOr(Schema.String),
});
type ListDeploymentEventsResponse = typeof ListDeploymentEventsResponseSchema.Type;

const decodeCreateTaskResponse = Schema.decodeUnknown(CreateTaskResponseSchema);
const decodeConvexGetTaskResult = Schema.decodeUnknown(ConvexGetTaskResultSchema);
const decodeConvexListTasksResult = Schema.decodeUnknown(ConvexListTasksResultSchema);
const decodeListPaywallsResponse = Schema.decodeUnknown(ListPaywallsResponseSchema);
const decodeGetPaywallResponse = Schema.decodeUnknown(GetPaywallResponseSchema);
const decodeListSettlementsResponse = Schema.decodeUnknown(ListSettlementsResponseSchema);
const decodeListDeploymentsResponse = Schema.decodeUnknown(ListDeploymentsResponseSchema);
const decodeListDeploymentEventsResponse = Schema.decodeUnknown(ListDeploymentEventsResponseSchema);

export type CreateLightningTaskInput = {
  readonly request: typeof L402FetchRequest.Type;
  readonly idempotencyKey?: string;
  readonly source?: string;
  readonly metadata?: unknown;
};

export type CreateLightningTaskResult = {
  readonly task: LightningTask;
  readonly existed: boolean;
  readonly requestId: string | null;
};

export type ListLightningPaywallsResult = {
  readonly paywalls: ReadonlyArray<LightningPaywall>;
  readonly requestId: string | null;
};

export type ListLightningSettlementsResult = {
  readonly settlements: ReadonlyArray<LightningSettlement>;
  readonly nextCursor: number | null;
  readonly requestId: string | null;
};

export type ListLightningDeploymentsResult = {
  readonly deployments: ReadonlyArray<LightningGatewayDeployment>;
  readonly nextCursor: number | null;
  readonly requestId: string | null;
};

export type ListLightningDeploymentEventsResult = {
  readonly events: ReadonlyArray<LightningGatewayEvent>;
  readonly nextCursor: number | null;
  readonly requestId: string | null;
};

export type LightningApi = {
  readonly createTask: (
    input: CreateLightningTaskInput,
  ) => Effect.Effect<CreateLightningTaskResult, LightningApiError, RequestContextService>;
  readonly getTask: (taskId: string) => Effect.Effect<LightningTask, LightningApiError, RequestContextService>;
  readonly listTasks: (input?: {
    readonly status?: LightningTaskStatus;
    readonly limit?: number;
  }) => Effect.Effect<ReadonlyArray<LightningTask>, LightningApiError, RequestContextService>;
  readonly listPaywalls: (input?: {
    readonly status?: LightningPaywallStatus;
    readonly limit?: number;
  }) => Effect.Effect<ListLightningPaywallsResult, LightningApiError, RequestContextService>;
  readonly getPaywall: (paywallId: string) => Effect.Effect<LightningPaywall, LightningApiError, RequestContextService>;
  readonly listOwnerSettlements: (input?: {
    readonly limit?: number;
    readonly beforeCreatedAtMs?: number;
  }) => Effect.Effect<ListLightningSettlementsResult, LightningApiError, RequestContextService>;
  readonly listPaywallSettlements: (
    paywallId: string,
    input?: {
      readonly limit?: number;
      readonly beforeCreatedAtMs?: number;
    },
  ) => Effect.Effect<ListLightningSettlementsResult, LightningApiError, RequestContextService>;
  readonly listDeployments: (input?: {
    readonly paywallId?: string;
    readonly status?: LightningDeploymentStatus;
    readonly limit?: number;
    readonly beforeUpdatedAtMs?: number;
  }) => Effect.Effect<ListLightningDeploymentsResult, LightningApiError, RequestContextService>;
  readonly listDeploymentEvents: (input?: {
    readonly paywallId?: string;
    readonly level?: LightningGatewayEventLevel;
    readonly limit?: number;
    readonly beforeCreatedAtMs?: number;
  }) => Effect.Effect<ListLightningDeploymentEventsResult, LightningApiError, RequestContextService>;
  readonly subscribeTask: (
    taskId: string,
  ) => Effect.Effect<Stream.Stream<LightningTask, LightningApiError>, never, RequestContextService>;
};

export class LightningApiService extends Context.Tag("@openagents/web/LightningApi")<
  LightningApiService,
  LightningApi
>() {}

const decodeWithSchema = <A>(
  operation: string,
  decode: (value: unknown) => Effect.Effect<A, unknown>,
  value: unknown,
): Effect.Effect<A, LightningApiError> =>
  decode(value).pipe(
    Effect.mapError((error) =>
      LightningApiError.make({
        operation: `${operation}.decode`,
        error,
      }),
    ),
  );

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

const maybeErrorMessage = (value: unknown): string | null => {
  const record = asRecord(value);
  if (!record) return null;
  return typeof record.error === "string" && record.error.length > 0 ? record.error : null;
};

const failHttp = (operation: string, status: number, json: unknown): Effect.Effect<never, LightningApiError> =>
  Effect.fail(
    LightningApiError.make({
      operation: `${operation}.http`,
      status,
      error: new Error(maybeErrorMessage(json) ?? `HTTP ${status}`),
    }),
  );

const withSearchParams = (
  basePath: string,
  params: ReadonlyArray<readonly [string, string | number | undefined]>,
): string => {
  const search = new URLSearchParams();
  for (const [key, value] of params) {
    if (value === undefined) continue;
    search.set(key, String(value));
  }
  const encoded = search.toString();
  return encoded.length > 0 ? `${basePath}?${encoded}` : basePath;
};

const fetchJson = Effect.fn("LightningApi.fetchJson")(function* (input: {
  readonly operation: string;
  readonly url: string;
  readonly method: "POST" | "GET";
  readonly body?: unknown;
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

  const response = yield* Effect.tryPromise({
    try: () =>
      fetch(url, {
        method: input.method,
        cache: "no-store",
        credentials: "include",
        headers,
        body: input.body === undefined ? undefined : JSON.stringify(input.body),
      }),
    catch: (error) => LightningApiError.make({ operation: `${input.operation}.fetch`, error }),
  });

  const json = yield* Effect.tryPromise({
    try: () => response.json(),
    catch: (error) => LightningApiError.make({ operation: `${input.operation}.json`, error }),
  });

  return {
    ok: response.ok,
    status: response.status,
    json,
    responseRequestId: response.headers.get("x-oa-request-id"),
  };
});

const createTask = Effect.fn("LightningApi.createTask")(function* (input: CreateLightningTaskInput) {
  const response = yield* fetchJson({
    operation: "createTask",
    url: "/api/lightning/l402/tasks",
    method: "POST",
    body: input,
  });

  if (!response.ok) {
    return yield* failHttp("createTask", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<CreateTaskResponse>("createTask", decodeCreateTaskResponse, response.json);
  return {
    task: decoded.task,
    existed: decoded.existed,
    requestId: decoded.requestId ?? response.responseRequestId,
  } satisfies CreateLightningTaskResult;
});

const listPaywalls = Effect.fn("LightningApi.listPaywalls")(function* (input?: {
  readonly status?: LightningPaywallStatus;
  readonly limit?: number;
}) {
  const response = yield* fetchJson({
    operation: "listPaywalls",
    url: withSearchParams("/api/lightning/paywalls", [
      ["status", input?.status],
      ["limit", input?.limit],
    ]),
    method: "GET",
  });

  if (!response.ok) {
    return yield* failHttp("listPaywalls", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<ListPaywallsResponse>("listPaywalls", decodeListPaywallsResponse, response.json);
  return {
    paywalls: decoded.paywalls,
    requestId: decoded.requestId ?? response.responseRequestId,
  } satisfies ListLightningPaywallsResult;
});

const getPaywall = Effect.fn("LightningApi.getPaywall")(function* (paywallId: string) {
  const encoded = encodeURIComponent(paywallId.trim());
  const response = yield* fetchJson({
    operation: "getPaywall",
    url: `/api/lightning/paywalls/${encoded}`,
    method: "GET",
  });

  if (!response.ok) {
    return yield* failHttp("getPaywall", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<GetPaywallResponse>("getPaywall", decodeGetPaywallResponse, response.json);
  return decoded.paywall;
});

const listOwnerSettlements = Effect.fn("LightningApi.listOwnerSettlements")(function* (input?: {
  readonly limit?: number;
  readonly beforeCreatedAtMs?: number;
}) {
  const response = yield* fetchJson({
    operation: "listOwnerSettlements",
    url: withSearchParams("/api/lightning/settlements", [
      ["limit", input?.limit],
      ["beforeCreatedAtMs", input?.beforeCreatedAtMs],
    ]),
    method: "GET",
  });

  if (!response.ok) {
    return yield* failHttp("listOwnerSettlements", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<ListSettlementsResponse>(
    "listOwnerSettlements",
    decodeListSettlementsResponse,
    response.json,
  );
  return {
    settlements: decoded.settlements,
    nextCursor: decoded.nextCursor,
    requestId: decoded.requestId ?? response.responseRequestId,
  } satisfies ListLightningSettlementsResult;
});

const listPaywallSettlements = Effect.fn("LightningApi.listPaywallSettlements")(function* (
  paywallId: string,
  input?: {
    readonly limit?: number;
    readonly beforeCreatedAtMs?: number;
  },
) {
  const encoded = encodeURIComponent(paywallId.trim());
  const response = yield* fetchJson({
    operation: "listPaywallSettlements",
    url: withSearchParams(`/api/lightning/paywalls/${encoded}/settlements`, [
      ["limit", input?.limit],
      ["beforeCreatedAtMs", input?.beforeCreatedAtMs],
    ]),
    method: "GET",
  });

  if (!response.ok) {
    return yield* failHttp("listPaywallSettlements", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<ListSettlementsResponse>(
    "listPaywallSettlements",
    decodeListSettlementsResponse,
    response.json,
  );
  return {
    settlements: decoded.settlements,
    nextCursor: decoded.nextCursor,
    requestId: decoded.requestId ?? response.responseRequestId,
  } satisfies ListLightningSettlementsResult;
});

const listDeployments = Effect.fn("LightningApi.listDeployments")(function* (input?: {
  readonly paywallId?: string;
  readonly status?: LightningDeploymentStatus;
  readonly limit?: number;
  readonly beforeUpdatedAtMs?: number;
}) {
  const response = yield* fetchJson({
    operation: "listDeployments",
    url: withSearchParams("/api/lightning/deployments", [
      ["paywallId", input?.paywallId],
      ["status", input?.status],
      ["limit", input?.limit],
      ["beforeUpdatedAtMs", input?.beforeUpdatedAtMs],
    ]),
    method: "GET",
  });

  if (!response.ok) {
    return yield* failHttp("listDeployments", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<ListDeploymentsResponse>(
    "listDeployments",
    decodeListDeploymentsResponse,
    response.json,
  );
  return {
    deployments: decoded.deployments,
    nextCursor: decoded.nextCursor,
    requestId: decoded.requestId ?? response.responseRequestId,
  } satisfies ListLightningDeploymentsResult;
});

const listDeploymentEvents = Effect.fn("LightningApi.listDeploymentEvents")(function* (input?: {
  readonly paywallId?: string;
  readonly level?: LightningGatewayEventLevel;
  readonly limit?: number;
  readonly beforeCreatedAtMs?: number;
}) {
  const response = yield* fetchJson({
    operation: "listDeploymentEvents",
    url: withSearchParams("/api/lightning/deployments/events", [
      ["paywallId", input?.paywallId],
      ["level", input?.level],
      ["limit", input?.limit],
      ["beforeCreatedAtMs", input?.beforeCreatedAtMs],
    ]),
    method: "GET",
  });

  if (!response.ok) {
    return yield* failHttp("listDeploymentEvents", response.status, response.json);
  }

  const decoded = yield* decodeWithSchema<ListDeploymentEventsResponse>(
    "listDeploymentEvents",
    decodeListDeploymentEventsResponse,
    response.json,
  );
  return {
    events: decoded.events,
    nextCursor: decoded.nextCursor,
    requestId: decoded.requestId ?? response.responseRequestId,
  } satisfies ListLightningDeploymentEventsResult;
});

export const LightningApiLive = Layer.effect(
  LightningApiService,
  Effect.gen(function* () {
    const convex = yield* ConvexService;
    const telemetry = yield* TelemetryService;
    const t = telemetry.withNamespace("lightning.api");

    const getTask = Effect.fn("LightningApi.getTask")(function* (taskId: string) {
      const result = yield* convex.query(api.lightning.tasks.getTask, { taskId }).pipe(
        Effect.mapError((error) => LightningApiError.make({ operation: "getTask.convex", error })),
      );
      const decoded = yield* decodeWithSchema<ConvexGetTaskResult>("getTask", decodeConvexGetTaskResult, result);
      yield* t.event("task.get", { taskId, status: decoded.task.status });
      return decoded.task;
    });

    const listTasks = Effect.fn("LightningApi.listTasks")(function* (input?: {
      readonly status?: LightningTaskStatus;
      readonly limit?: number;
    }) {
      const result = yield* convex
        .query(api.lightning.tasks.listTasks, {
          status: input?.status,
          limit: input?.limit,
        })
        .pipe(Effect.mapError((error) => LightningApiError.make({ operation: "listTasks.convex", error })));
      const decoded = yield* decodeWithSchema<ConvexListTasksResult>("listTasks", decodeConvexListTasksResult, result);
      return decoded.tasks;
    });

    const subscribeTask = (taskId: string) =>
      Effect.succeed(
        convex
          .subscribeQuery(api.lightning.tasks.getTask, { taskId })
          .pipe(
            Stream.mapError((error) =>
              LightningApiError.make({
                operation: "subscribeTask.convex",
                error,
              }),
            ),
            Stream.mapEffect((result) =>
              decodeWithSchema<ConvexGetTaskResult>("subscribeTask", decodeConvexGetTaskResult, result).pipe(
                Effect.map((decoded) => decoded.task),
              ),
            ),
          ),
      );

    return LightningApiService.of({
      createTask,
      getTask,
      listTasks,
      listPaywalls,
      getPaywall,
      listOwnerSettlements,
      listPaywallSettlements,
      listDeployments,
      listDeploymentEvents,
      subscribeTask,
    });
  }),
);
