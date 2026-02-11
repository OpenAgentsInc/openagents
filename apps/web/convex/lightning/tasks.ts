import { v } from "convex/values";
import { Effect } from "effect";

import type { EffectMutationCtx, EffectQueryCtx } from "../effect/ctx";
import { effectMutation, effectQuery } from "../effect/functions";
import { tryPromise } from "../effect/tryPromise";

import { getSubject } from "../autopilot/access";

const nowMs = () => Date.now();
const newId = () => crypto.randomUUID();

export type LightningTaskStatus =
  | "queued"
  | "approved"
  | "running"
  | "paid"
  | "cached"
  | "blocked"
  | "failed"
  | "completed";

export type LightningTaskActor = "web_worker" | "desktop_executor" | "system";

const taskStatusValidator = v.union(
  v.literal("queued"),
  v.literal("approved"),
  v.literal("running"),
  v.literal("paid"),
  v.literal("cached"),
  v.literal("blocked"),
  v.literal("failed"),
  v.literal("completed"),
);

const taskActorValidator = v.union(v.literal("web_worker"), v.literal("desktop_executor"), v.literal("system"));

const httpMethodValidator = v.union(
  v.literal("GET"),
  v.literal("POST"),
  v.literal("PUT"),
  v.literal("PATCH"),
  v.literal("DELETE"),
);

const l402RequestValidator = v.object({
  url: v.string(),
  method: v.optional(httpMethodValidator),
  headers: v.optional(v.record(v.string(), v.string())),
  body: v.optional(v.string()),
  maxSpendMsats: v.number(),
  challengeHeader: v.optional(v.string()),
  forceRefresh: v.optional(v.boolean()),
  scope: v.optional(v.string()),
  cacheTtlMs: v.optional(v.number()),
});

const taskValidator = v.object({
  taskId: v.string(),
  ownerId: v.string(),
  status: taskStatusValidator,
  request: l402RequestValidator,
  idempotencyKey: v.optional(v.string()),
  source: v.optional(v.string()),
  requestId: v.optional(v.string()),
  metadata: v.optional(v.any()),
  attemptCount: v.number(),
  lastErrorCode: v.optional(v.string()),
  lastErrorMessage: v.optional(v.string()),
  createdAtMs: v.number(),
  updatedAtMs: v.number(),
  lastTransitionAtMs: v.number(),
});

const taskEventValidator = v.object({
  taskId: v.string(),
  ownerId: v.string(),
  fromStatus: v.optional(taskStatusValidator),
  toStatus: taskStatusValidator,
  actor: taskActorValidator,
  reason: v.optional(v.string()),
  requestId: v.optional(v.string()),
  errorCode: v.optional(v.string()),
  errorMessage: v.optional(v.string()),
  metadata: v.optional(v.any()),
  createdAtMs: v.number(),
});

type L402Request = {
  readonly url: string;
  readonly method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  readonly headers?: Readonly<Record<string, string>>;
  readonly body?: string;
  readonly maxSpendMsats: number;
  readonly challengeHeader?: string;
  readonly forceRefresh?: boolean;
  readonly scope?: string;
  readonly cacheTtlMs?: number;
};

type LightningTaskDoc = {
  readonly _id: any;
  readonly taskId: string;
  readonly ownerId: string;
  readonly status: LightningTaskStatus;
  readonly request: L402Request;
  readonly idempotencyKey?: string;
  readonly source?: string;
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly attemptCount: number;
  readonly lastErrorCode?: string;
  readonly lastErrorMessage?: string;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
  readonly lastTransitionAtMs: number;
};

const transitionGraph: Record<LightningTaskStatus, ReadonlyArray<LightningTaskStatus>> = {
  queued: ["approved", "running", "blocked", "failed", "completed"],
  approved: ["running", "blocked", "failed", "completed"],
  running: ["paid", "cached", "blocked", "failed", "completed"],
  paid: ["completed", "failed", "cached"],
  cached: ["completed", "running", "failed"],
  blocked: ["queued", "failed"],
  failed: ["queued"],
  completed: [],
};

const toTask = (task: LightningTaskDoc) => ({
  taskId: task.taskId,
  ownerId: task.ownerId,
  status: task.status,
  request: task.request,
  idempotencyKey: task.idempotencyKey,
  source: task.source,
  requestId: task.requestId,
  metadata: task.metadata,
  attemptCount: task.attemptCount,
  lastErrorCode: task.lastErrorCode,
  lastErrorMessage: task.lastErrorMessage,
  createdAtMs: task.createdAtMs,
  updatedAtMs: task.updatedAtMs,
  lastTransitionAtMs: task.lastTransitionAtMs,
});

const normalizeOptionalString = (value: string | undefined, maxLen: number): string | undefined => {
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  if (!trimmed) return undefined;
  return trimmed.slice(0, maxLen);
};

const normalizeHeaders = (input: Readonly<Record<string, string>> | undefined): Record<string, string> | undefined => {
  if (!input) return undefined;
  const entries = Object.entries(input)
    .filter(([k, v]) => typeof k === "string" && k.length > 0 && typeof v === "string")
    .map(([k, v]) => [k.trim().slice(0, 128), v.trim().slice(0, 4_096)] as const)
    .filter(([k]) => k.length > 0)
    .sort(([a], [b]) => a.localeCompare(b));
  if (entries.length === 0) return undefined;
  return Object.fromEntries(entries);
};

const normalizeRequest = (request: L402Request): Effect.Effect<L402Request, Error> =>
  Effect.gen(function* () {
    const url = normalizeOptionalString(request.url, 4_096);
    if (!url) return yield* Effect.fail(new Error("invalid_input"));

    const maxSpendMsats = Math.floor(request.maxSpendMsats);
    if (!Number.isFinite(maxSpendMsats) || maxSpendMsats < 0) {
      return yield* Effect.fail(new Error("invalid_input"));
    }

    const cacheTtlMs =
      typeof request.cacheTtlMs === "number" && Number.isFinite(request.cacheTtlMs)
        ? Math.max(0, Math.floor(request.cacheTtlMs))
        : undefined;

    return {
      url,
      method: request.method,
      headers: normalizeHeaders(request.headers),
      body: typeof request.body === "string" ? request.body.slice(0, 64_000) : undefined,
      maxSpendMsats,
      challengeHeader: normalizeOptionalString(request.challengeHeader, 8_000),
      forceRefresh: request.forceRefresh === true ? true : undefined,
      scope: normalizeOptionalString(request.scope, 512),
      cacheTtlMs,
    };
  });

const requireSubject = (ctx: EffectQueryCtx | EffectMutationCtx): Effect.Effect<string, Error> =>
  getSubject(ctx).pipe(
    Effect.flatMap((subject) => (subject ? Effect.succeed(subject) : Effect.fail(new Error("unauthorized")))),
  );

const loadTaskByTaskId = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  taskId: string,
): Effect.Effect<LightningTaskDoc | null, Error> =>
  tryPromise(() =>
    ctx.db
      .query("lightningTasks")
      .withIndex("by_taskId", (q) => q.eq("taskId", taskId))
      .unique(),
  ).pipe(
    Effect.mapError((error) => (error instanceof Error ? error : new Error(String(error)))),
  ) as Effect.Effect<LightningTaskDoc | null, Error>;

const assertTaskAccess = (
  ctx: EffectQueryCtx | EffectMutationCtx,
  args: { readonly taskId: string; readonly ownerId: string },
): Effect.Effect<LightningTaskDoc, Error> =>
  Effect.gen(function* () {
    const task = yield* loadTaskByTaskId(ctx, args.taskId);
    if (!task) return yield* Effect.fail(new Error("task_not_found"));
    if (task.ownerId !== args.ownerId) return yield* Effect.fail(new Error("forbidden"));
    return task;
  });

const canTransition = (fromStatus: LightningTaskStatus, toStatus: LightningTaskStatus): boolean =>
  fromStatus === toStatus || transitionGraph[fromStatus].includes(toStatus);

const shouldPersistError = (status: LightningTaskStatus): boolean => status === "blocked" || status === "failed";

const insertTaskEvent = (
  ctx: EffectMutationCtx,
  input: {
    readonly taskId: string;
    readonly ownerId: string;
    readonly fromStatus?: LightningTaskStatus;
    readonly toStatus: LightningTaskStatus;
    readonly actor: LightningTaskActor;
    readonly reason?: string;
    readonly requestId?: string;
    readonly errorCode?: string;
    readonly errorMessage?: string;
    readonly metadata?: unknown;
    readonly createdAtMs: number;
  },
): Effect.Effect<{
  readonly taskId: string;
  readonly ownerId: string;
  readonly fromStatus?: LightningTaskStatus;
  readonly toStatus: LightningTaskStatus;
  readonly actor: LightningTaskActor;
  readonly reason?: string;
  readonly requestId?: string;
  readonly errorCode?: string;
  readonly errorMessage?: string;
  readonly metadata?: unknown;
  readonly createdAtMs: number;
}, Error> =>
  Effect.gen(function* () {
    yield* tryPromise(() =>
      ctx.db.insert("lightningTaskEvents", {
        taskId: input.taskId,
        ownerId: input.ownerId,
        fromStatus: input.fromStatus,
        toStatus: input.toStatus,
        actor: input.actor,
        reason: input.reason,
        requestId: input.requestId,
        errorCode: input.errorCode,
        errorMessage: input.errorMessage,
        metadata: input.metadata,
        createdAtMs: input.createdAtMs,
      }),
    ).pipe(Effect.mapError((error) => (error instanceof Error ? error : new Error(String(error)))));
    return input;
  });

export const createTaskImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly request: L402Request;
    readonly idempotencyKey?: string;
    readonly source?: string;
    readonly requestId?: string;
    readonly metadata?: unknown;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const now = nowMs();
    const request = yield* normalizeRequest(args.request);
    const idempotencyKey = normalizeOptionalString(args.idempotencyKey, 200);
    const source = normalizeOptionalString(args.source, 120);
    const requestId = normalizeOptionalString(args.requestId, 160);

    if (idempotencyKey) {
      const existing = (yield* tryPromise(() =>
        ctx.db
          .query("lightningTasks")
          .withIndex("by_ownerId_idempotencyKey", (q) => q.eq("ownerId", ownerId).eq("idempotencyKey", idempotencyKey))
          .unique(),
      )) as LightningTaskDoc | null;
      if (existing) {
        return { ok: true, existed: true, task: toTask(existing) };
      }
    }

    const taskId = newId();
    yield* tryPromise(() =>
      ctx.db.insert("lightningTasks", {
        taskId,
        ownerId,
        status: "queued",
        request,
        idempotencyKey,
        source,
        requestId,
        metadata: args.metadata,
        attemptCount: 0,
        createdAtMs: now,
        updatedAtMs: now,
        lastTransitionAtMs: now,
      }),
    );

    const task = yield* assertTaskAccess(ctx, { taskId, ownerId });

    yield* insertTaskEvent(ctx, {
      taskId,
      ownerId,
      toStatus: "queued",
      actor: "web_worker",
      reason: "task_created",
      requestId,
      metadata: args.metadata,
      createdAtMs: now,
    });

    return { ok: true, existed: false, task: toTask(task) };
  });

export const createTask = effectMutation({
  args: {
    request: l402RequestValidator,
    idempotencyKey: v.optional(v.string()),
    source: v.optional(v.string()),
    requestId: v.optional(v.string()),
    metadata: v.optional(v.any()),
  },
  returns: v.object({
    ok: v.boolean(),
    existed: v.boolean(),
    task: taskValidator,
  }),
  handler: createTaskImpl,
});

export const getTaskImpl = (ctx: EffectQueryCtx, args: { readonly taskId: string }) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const task = yield* assertTaskAccess(ctx, { taskId: args.taskId, ownerId });
    return { ok: true, task: toTask(task) };
  });

export const getTask = effectQuery({
  args: {
    taskId: v.string(),
  },
  returns: v.object({
    ok: v.boolean(),
    task: taskValidator,
  }),
  handler: getTaskImpl,
});

export const listTasksImpl = (
  ctx: EffectQueryCtx,
  args: { readonly status?: LightningTaskStatus | undefined; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit) ? Math.max(1, Math.min(200, Math.floor(args.limit))) : 100;

    const rows: ReadonlyArray<LightningTaskDoc> = yield* (args.status
      ? (tryPromise(() =>
          ctx.db
            .query("lightningTasks")
            .withIndex("by_ownerId_status_updatedAtMs", (q) => q.eq("ownerId", ownerId).eq("status", args.status!))
            .order("desc")
            .take(limit),
        ) as Effect.Effect<ReadonlyArray<LightningTaskDoc>, unknown>)
      : (tryPromise(() =>
          ctx.db
            .query("lightningTasks")
            .withIndex("by_ownerId_updatedAtMs", (q) => q.eq("ownerId", ownerId))
            .order("desc")
            .take(limit),
        ) as Effect.Effect<ReadonlyArray<LightningTaskDoc>, unknown>));

    return {
      ok: true,
      tasks: rows.map((task) => toTask(task)),
    };
  });

export const listTasks = effectQuery({
  args: {
    status: v.optional(taskStatusValidator),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    tasks: v.array(taskValidator),
  }),
  handler: listTasksImpl,
});

export const transitionTaskImpl = (
  ctx: EffectMutationCtx,
  args: {
    readonly taskId: string;
    readonly toStatus: LightningTaskStatus;
    readonly actor?: LightningTaskActor | undefined;
    readonly reason?: string | undefined;
    readonly requestId?: string | undefined;
    readonly errorCode?: string | undefined;
    readonly errorMessage?: string | undefined;
    readonly metadata?: unknown;
  },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    const task = yield* assertTaskAccess(ctx, { taskId: args.taskId, ownerId });
    const fromStatus = task.status;

    if (!canTransition(fromStatus, args.toStatus)) {
      return yield* Effect.fail(new Error("invalid_transition"));
    }

    if (fromStatus === args.toStatus) {
      return {
        ok: true,
        changed: false,
        task: toTask(task),
      };
    }

    const actor = args.actor ?? "desktop_executor";
    const reason = normalizeOptionalString(args.reason, 500);
    const requestId = normalizeOptionalString(args.requestId, 160);
    const errorCode = shouldPersistError(args.toStatus) ? normalizeOptionalString(args.errorCode, 120) : undefined;
    const errorMessage = shouldPersistError(args.toStatus) ? normalizeOptionalString(args.errorMessage, 2_000) : undefined;
    const nextAttemptCount = args.toStatus === "running" ? Math.max(0, Math.floor(task.attemptCount)) + 1 : task.attemptCount;
    const now = nowMs();

    yield* tryPromise(() =>
      ctx.db.patch(task._id, {
        status: args.toStatus,
        attemptCount: nextAttemptCount,
        lastErrorCode: errorCode,
        lastErrorMessage: errorMessage,
        updatedAtMs: now,
        lastTransitionAtMs: now,
      }),
    );

    const updated = yield* assertTaskAccess(ctx, { taskId: args.taskId, ownerId });
    const event = yield* insertTaskEvent(ctx, {
      taskId: args.taskId,
      ownerId,
      fromStatus,
      toStatus: args.toStatus,
      actor,
      reason,
      requestId,
      errorCode,
      errorMessage,
      metadata: args.metadata,
      createdAtMs: now,
    });

    return {
      ok: true,
      changed: true,
      task: toTask(updated),
      event,
    };
  });

export const transitionTask = effectMutation({
  args: {
    taskId: v.string(),
    toStatus: taskStatusValidator,
    actor: v.optional(taskActorValidator),
    reason: v.optional(v.string()),
    requestId: v.optional(v.string()),
    errorCode: v.optional(v.string()),
    errorMessage: v.optional(v.string()),
    metadata: v.optional(v.any()),
  },
  returns: v.object({
    ok: v.boolean(),
    changed: v.boolean(),
    task: taskValidator,
    event: v.optional(taskEventValidator),
  }),
  handler: transitionTaskImpl,
});

export const listTaskEventsImpl = (
  ctx: EffectQueryCtx,
  args: { readonly taskId: string; readonly limit?: number | undefined },
) =>
  Effect.gen(function* () {
    const ownerId = yield* requireSubject(ctx);
    yield* assertTaskAccess(ctx, { taskId: args.taskId, ownerId });

    const limit =
      typeof args.limit === "number" && Number.isFinite(args.limit) ? Math.max(1, Math.min(500, Math.floor(args.limit))) : 200;

    const rows = (yield* tryPromise(() =>
      ctx.db
        .query("lightningTaskEvents")
        .withIndex("by_taskId_createdAtMs", (q) => q.eq("taskId", args.taskId))
        .order("asc")
        .take(limit),
    )) as ReadonlyArray<{
      readonly taskId: string;
      readonly ownerId: string;
      readonly fromStatus?: LightningTaskStatus;
      readonly toStatus: LightningTaskStatus;
      readonly actor: LightningTaskActor;
      readonly reason?: string;
      readonly requestId?: string;
      readonly errorCode?: string;
      readonly errorMessage?: string;
      readonly metadata?: unknown;
      readonly createdAtMs: number;
    }>;

    return {
      ok: true,
      events: rows
        .filter((row) => row.ownerId === ownerId)
        .map((row) => ({
          taskId: row.taskId,
          ownerId: row.ownerId,
          fromStatus: row.fromStatus,
          toStatus: row.toStatus,
          actor: row.actor,
          reason: row.reason,
          requestId: row.requestId,
          errorCode: row.errorCode,
          errorMessage: row.errorMessage,
          metadata: row.metadata,
          createdAtMs: row.createdAtMs,
        })),
    };
  });

export const listTaskEvents = effectQuery({
  args: {
    taskId: v.string(),
    limit: v.optional(v.number()),
  },
  returns: v.object({
    ok: v.boolean(),
    events: v.array(taskEventValidator),
  }),
  handler: listTaskEventsImpl,
});
