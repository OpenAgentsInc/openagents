import { Context, Effect, Layer, Option, Schema } from "effect";

import { DesktopConfigService } from "./config";
import type { ExecutorTask, ExecutorTaskRequest, ExecutorTaskStatus } from "./model";

export class TaskProviderTransportError extends Schema.TaggedError<TaskProviderTransportError>()(
  "TaskProviderTransportError",
  {
    operation: Schema.String,
    error: Schema.Defect,
  },
) {}

export class TaskProviderHttpError extends Schema.TaggedError<TaskProviderHttpError>()("TaskProviderHttpError", {
  operation: Schema.String,
  status: Schema.Number,
  error: Schema.String,
}) {}

export class TaskProviderDecodeError extends Schema.TaggedError<TaskProviderDecodeError>()(
  "TaskProviderDecodeError",
  {
    operation: Schema.String,
    reason: Schema.String,
  },
) {}

export class TaskProviderAuthError extends Schema.TaggedError<TaskProviderAuthError>()("TaskProviderAuthError", {
  operation: Schema.String,
  reason: Schema.String,
}) {}

export type TaskProviderError =
  | TaskProviderTransportError
  | TaskProviderHttpError
  | TaskProviderDecodeError
  | TaskProviderAuthError;

export type TaskProviderApi = Readonly<{
  readonly enqueueDemoTask: (input: {
    readonly payload: string;
    readonly token: string;
  }) => Effect.Effect<ExecutorTask, TaskProviderError>;
  readonly pollPendingTask: (input: {
    readonly userId: string;
    readonly token: string;
  }) => Effect.Effect<Option.Option<ExecutorTask>, TaskProviderError>;
  readonly transitionTask: (input: {
    readonly taskId: string;
    readonly token: string;
    readonly toStatus: ExecutorTaskStatus;
    readonly reason?: string;
    readonly errorCode?: string;
    readonly errorMessage?: string;
    readonly metadata?: unknown;
  }) => Effect.Effect<ExecutorTask, TaskProviderError>;
  readonly listTasks: (input: {
    readonly token: string;
    readonly status?: ExecutorTaskStatus;
    readonly limit?: number;
  }) => Effect.Effect<ReadonlyArray<ExecutorTask>, TaskProviderError>;
}>;

export class TaskProviderService extends Context.Tag("@openagents/desktop/TaskProviderService")<
  TaskProviderService,
  TaskProviderApi
>() {}

const taskStatuses = [
  "queued",
  "approved",
  "running",
  "paid",
  "cached",
  "blocked",
  "failed",
  "completed",
] as const;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  Boolean(value) && typeof value === "object" && !Array.isArray(value);

const asString = (value: unknown): string | undefined =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;

const asStatus = (value: unknown): ExecutorTaskStatus | undefined =>
  typeof value === "string" && taskStatuses.includes(value as ExecutorTaskStatus)
    ? (value as ExecutorTaskStatus)
    : undefined;

const asNumber = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;

const normalizeHeaders = (value: unknown): Record<string, string> | undefined => {
  if (!isRecord(value)) return undefined;
  const entries = Object.entries(value)
    .filter(([key, headerValue]) => typeof key === "string" && typeof headerValue === "string")
    .map(([key, headerValue]) => [key, headerValue] as const);
  return entries.length > 0 ? (Object.fromEntries(entries) as Record<string, string>) : undefined;
};

const parseTaskRequest = (value: unknown): ExecutorTaskRequest | null => {
  if (!isRecord(value)) return null;
  const url = asString(value.url);
  const maxSpendMsats = asNumber(value.maxSpendMsats);
  if (!url || maxSpendMsats === undefined) return null;

  const method = asString(value.method);
  const normalizedMethod =
    method === "GET" || method === "POST" || method === "PUT" || method === "PATCH" || method === "DELETE"
      ? method
      : undefined;
  const headers = normalizeHeaders(value.headers);
  const body = asString(value.body);
  const challengeHeader = asString(value.challengeHeader);
  const scope = asString(value.scope);
  const cacheTtlMsRaw = asNumber(value.cacheTtlMs);

  return {
    url,
    maxSpendMsats: Math.max(0, Math.floor(maxSpendMsats)),
    ...(normalizedMethod ? { method: normalizedMethod } : {}),
    ...(headers ? { headers } : {}),
    ...(body ? { body } : {}),
    ...(challengeHeader ? { challengeHeader } : {}),
    ...(typeof value.forceRefresh === "boolean" ? { forceRefresh: value.forceRefresh } : {}),
    ...(scope ? { scope } : {}),
    ...(cacheTtlMsRaw !== undefined ? { cacheTtlMs: Math.max(0, Math.floor(cacheTtlMsRaw)) } : {}),
  } satisfies ExecutorTaskRequest;
};

const parseTask = (operation: string, value: unknown): Effect.Effect<ExecutorTask, TaskProviderDecodeError> =>
  Effect.gen(function* () {
    if (!isRecord(value)) {
      return yield* TaskProviderDecodeError.make({
        operation,
        reason: "task_not_object",
      });
    }

    const id = asString(value.taskId);
    const ownerId = asString(value.ownerId);
    const status = asStatus(value.status);
    const request = parseTaskRequest(value.request);
    const createdAtMs = asNumber(value.createdAtMs);
    const updatedAtMs = asNumber(value.updatedAtMs);
    const attemptCount = asNumber(value.attemptCount);

    if (!id || !ownerId || !status || !request || createdAtMs === undefined || updatedAtMs === undefined || attemptCount === undefined) {
      return yield* TaskProviderDecodeError.make({
        operation,
        reason: "invalid_task_shape",
      });
    }

    const source = asString(value.source);
    const idempotencyKey = asString(value.idempotencyKey);
    const requestId = asString(value.requestId);
    const lastErrorCode = asString(value.lastErrorCode);
    const lastErrorMessage = asString(value.lastErrorMessage);

    return {
      id,
      ownerId,
      status,
      request,
      attemptCount: Math.max(0, Math.floor(attemptCount)),
      createdAtMs: Math.max(0, Math.floor(createdAtMs)),
      updatedAtMs: Math.max(0, Math.floor(updatedAtMs)),
      ...(source ? { source } : {}),
      ...(idempotencyKey ? { idempotencyKey } : {}),
      ...(requestId ? { requestId } : {}),
      ...(value.metadata !== undefined ? { metadata: value.metadata } : {}),
      ...(lastErrorCode ? { lastErrorCode } : {}),
      ...(lastErrorMessage ? { lastErrorMessage, failureReason: lastErrorMessage } : {}),
    } satisfies ExecutorTask;
  });

const normalizeDemoRequest = (payload: string): ExecutorTaskRequest => {
  const trimmed = payload.trim();
  const fallbackUrl = `https://api.example.com/demo/${encodeURIComponent(trimmed || "task")}`;

  let url = fallbackUrl;
  try {
    if (trimmed.length > 0) {
      const parsed = new URL(trimmed);
      url = parsed.toString();
    }
  } catch {
    url = fallbackUrl;
  }

  return {
    url,
    method: "GET",
    maxSpendMsats: 2_500,
    scope: "desktop-demo",
  };
};

const parseHttpErrorMessage = (json: unknown, status: number): string => {
  if (isRecord(json) && typeof json.error === "string" && json.error.trim().length > 0) {
    return json.error.trim();
  }
  return `HTTP ${status}`;
};

const requestJson = Effect.fn("TaskProvider.requestJson")(function* (input: {
  readonly operation: string;
  readonly method: "GET" | "POST";
  readonly url: string;
  readonly token: string;
  readonly body?: unknown;
}) {
  const headers = new Headers({ accept: "application/json" });
  headers.set("authorization", `Bearer ${input.token}`);
  headers.set("x-oa-request-id", `desktop-${crypto.randomUUID()}`);
  if (input.body !== undefined) {
    headers.set("content-type", "application/json");
  }

  const response = yield* Effect.tryPromise({
    try: async () => {
      const init: RequestInit = {
        method: input.method,
        headers,
        cache: "no-store",
      };
      if (input.body !== undefined) {
        init.body = JSON.stringify(input.body);
      }
      return fetch(input.url, init);
    },
    catch: (error) =>
      TaskProviderTransportError.make({
        operation: input.operation,
        error,
      }),
  });

  const json = yield* Effect.tryPromise({
    try: async () => await response.json(),
    catch: (error) =>
      TaskProviderTransportError.make({
        operation: `${input.operation}.json`,
        error,
      }),
  });

  if (!response.ok) {
    return yield* TaskProviderHttpError.make({
      operation: input.operation,
      status: response.status,
      error: parseHttpErrorMessage(json, response.status),
    });
  }

  return json;
});

export const TaskProviderLive = Layer.effect(
  TaskProviderService,
  Effect.gen(function* () {
    const cfg = yield* DesktopConfigService;

    const listTasks = Effect.fn("TaskProvider.listTasks")(function* (input: {
      readonly token: string;
      readonly status?: ExecutorTaskStatus;
      readonly limit?: number;
    }) {
      if (!input.token || input.token.trim().length === 0) {
        return yield* TaskProviderAuthError.make({
          operation: "listTasks",
          reason: "missing_token",
        });
      }

      const params = new URLSearchParams();
      if (input.status) params.set("status", input.status);
      if (input.limit !== undefined) params.set("limit", String(Math.max(1, Math.min(200, Math.floor(input.limit)))));
      const suffix = params.size > 0 ? `?${params.toString()}` : "";
      const json = yield* requestJson({
        operation: "listTasks",
        method: "GET",
        url: `${cfg.openAgentsBaseUrl}/api/lightning/l402/tasks${suffix}`,
        token: input.token,
      });

      if (!isRecord(json) || !Array.isArray(json.tasks)) {
        return yield* TaskProviderDecodeError.make({
          operation: "listTasks",
          reason: "invalid_response_shape",
        });
      }

      return yield* Effect.forEach(json.tasks, (row) => parseTask("listTasks.task", row));
    });

    const pollPendingTask = Effect.fn("TaskProvider.pollPendingTask")(function* (input: {
      readonly userId: string;
      readonly token: string;
    }) {
      if (!input.userId || input.userId.trim().length === 0) {
        return yield* TaskProviderAuthError.make({
          operation: "pollPendingTask",
          reason: "missing_user_id",
        });
      }
      const approved = yield* listTasks({
        token: input.token,
        status: "approved",
        limit: 25,
      });
      const queued = yield* listTasks({
        token: input.token,
        status: "queued",
        limit: 25,
      });
      const candidates = [...approved, ...queued]
        .filter((task) => task.ownerId === input.userId)
        .sort((a, b) => a.createdAtMs - b.createdAtMs);

      return candidates.length > 0 ? Option.some(candidates[0]!) : Option.none<ExecutorTask>();
    });

    const transitionTask = Effect.fn("TaskProvider.transitionTask")(function* (input: {
      readonly taskId: string;
      readonly token: string;
      readonly toStatus: ExecutorTaskStatus;
      readonly reason?: string;
      readonly errorCode?: string;
      readonly errorMessage?: string;
      readonly metadata?: unknown;
    }) {
      if (!input.token || input.token.trim().length === 0) {
        return yield* TaskProviderAuthError.make({
          operation: "transitionTask",
          reason: "missing_token",
        });
      }
      const json = yield* requestJson({
        operation: "transitionTask",
        method: "POST",
        url: `${cfg.openAgentsBaseUrl}/api/lightning/l402/tasks/${encodeURIComponent(input.taskId)}/transition`,
        token: input.token,
        body: {
          toStatus: input.toStatus,
          ...(input.reason ? { reason: input.reason } : {}),
          ...(input.errorCode ? { errorCode: input.errorCode } : {}),
          ...(input.errorMessage ? { errorMessage: input.errorMessage } : {}),
          ...(input.metadata !== undefined ? { metadata: input.metadata } : {}),
        },
      });

      if (!isRecord(json) || !isRecord(json.task)) {
        return yield* TaskProviderDecodeError.make({
          operation: "transitionTask",
          reason: "invalid_response_shape",
        });
      }

      return yield* parseTask("transitionTask.task", json.task);
    });

    const enqueueDemoTask = Effect.fn("TaskProvider.enqueueDemoTask")(function* (input: {
      readonly payload: string;
      readonly token: string;
    }) {
      if (!input.token || input.token.trim().length === 0) {
        return yield* TaskProviderAuthError.make({
          operation: "enqueueDemoTask",
          reason: "missing_token",
        });
      }
      const request = normalizeDemoRequest(input.payload);
      const idempotencyKey = `desktop-${crypto.randomUUID()}`;
      const json = yield* requestJson({
        operation: "enqueueDemoTask",
        method: "POST",
        url: `${cfg.openAgentsBaseUrl}/api/lightning/l402/tasks`,
        token: input.token,
        body: {
          request,
          idempotencyKey,
          source: "desktop_executor_demo",
          metadata: {
            payload: input.payload.trim(),
          },
        },
      });

      if (!isRecord(json) || !isRecord(json.task)) {
        return yield* TaskProviderDecodeError.make({
          operation: "enqueueDemoTask",
          reason: "invalid_response_shape",
        });
      }
      return yield* parseTask("enqueueDemoTask.task", json.task);
    });

    return TaskProviderService.of({
      enqueueDemoTask,
      pollPendingTask,
      transitionTask,
      listTasks,
    });
  }),
);
