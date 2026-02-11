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

const decodeCreateTaskResponse = Schema.decodeUnknown(CreateTaskResponseSchema);
const decodeConvexGetTaskResult = Schema.decodeUnknown(ConvexGetTaskResultSchema);
const decodeConvexListTasksResult = Schema.decodeUnknown(ConvexListTasksResultSchema);

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

export type LightningApi = {
  readonly createTask: (
    input: CreateLightningTaskInput,
  ) => Effect.Effect<CreateLightningTaskResult, LightningApiError, RequestContextService>;
  readonly getTask: (taskId: string) => Effect.Effect<LightningTask, LightningApiError, RequestContextService>;
  readonly listTasks: (input?: {
    readonly status?: LightningTaskStatus;
    readonly limit?: number;
  }) => Effect.Effect<ReadonlyArray<LightningTask>, LightningApiError, RequestContextService>;
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

  return { ok: response.ok, status: response.status, json };
});

const createTask = Effect.fn("LightningApi.createTask")(function* (input: CreateLightningTaskInput) {
  const response = yield* fetchJson({
    operation: "createTask",
    url: "/api/lightning/l402/tasks",
    method: "POST",
    body: input,
  });

  if (!response.ok) {
    return yield* Effect.fail(
      LightningApiError.make({
        operation: "createTask.http",
        status: response.status,
        error: new Error(`HTTP ${response.status}`),
      }),
    );
  }

  const decoded = yield* decodeWithSchema<CreateTaskResponse>("createTask", decodeCreateTaskResponse, response.json);
  return {
    task: decoded.task,
    existed: decoded.existed,
    requestId: decoded.requestId,
  } satisfies CreateLightningTaskResult;
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
      subscribeTask,
    });
  }),
);
