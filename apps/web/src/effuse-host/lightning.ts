import { L402FetchRequest } from "@openagentsinc/lightning-effect";
import { Effect, Schema } from "effect";

import { api } from "../../convex/_generated/api";
import { AuthService } from "../effect/auth";
import { ConvexService } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";

import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import { getWorkerRuntime } from "./runtime";
import type { WorkerEnv } from "./env";

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  });

const CreateL402TaskBodySchema = Schema.Struct({
  request: L402FetchRequest,
  idempotencyKey: Schema.optional(Schema.NonEmptyString),
  source: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
});

type CreateL402TaskBody = typeof CreateL402TaskBodySchema.Type;

const decodeCreateBody = (value: unknown): Effect.Effect<CreateL402TaskBody, Error> =>
  Schema.decodeUnknown(CreateL402TaskBodySchema)(value).pipe(Effect.mapError(() => new Error("invalid_input")));

const LightningTaskStatusSchema = Schema.Literal(
  "queued",
  "approved",
  "running",
  "paid",
  "cached",
  "blocked",
  "failed",
  "completed",
);
type LightningTaskStatus = typeof LightningTaskStatusSchema.Type;

const LightningTaskActorSchema = Schema.Literal("web_worker", "desktop_executor", "system");

const TransitionTaskBodySchema = Schema.Struct({
  toStatus: LightningTaskStatusSchema,
  actor: Schema.optional(LightningTaskActorSchema),
  reason: Schema.optional(Schema.String),
  errorCode: Schema.optional(Schema.String),
  errorMessage: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
});
type TransitionTaskBody = typeof TransitionTaskBodySchema.Type;

const decodeTransitionBody = (value: unknown): Effect.Effect<TransitionTaskBody, Error> =>
  Schema.decodeUnknown(TransitionTaskBodySchema)(value).pipe(
    Effect.mapError(() => new Error("invalid_input")),
  );

const readJsonBody = (request: Request): Effect.Effect<unknown, Error> =>
  Effect.tryPromise({
    try: () => request.json(),
    catch: () => new Error("invalid_input"),
  });

const statusFromErrorMessage = (message: string): number => {
  const m = message.toLowerCase();
  if (m.includes("unauthorized")) return 401;
  if (m.includes("invalid_input")) return 400;
  if (m.includes("forbidden")) return 403;
  return 500;
};

const parseLimit = (value: string | null): number | undefined => {
  if (!value) return undefined;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return undefined;
  return Math.max(1, Math.min(200, Math.floor(parsed)));
};

const parseTaskStatus = (value: string | null): Effect.Effect<LightningTaskStatus | undefined, Error> => {
  if (!value || value.trim().length === 0) return Effect.succeed(undefined);
  return Schema.decodeUnknown(LightningTaskStatusSchema)(value.trim()).pipe(
    Effect.mapError(() => new Error("invalid_input")),
  );
};

const runAuthedLightning = async <A>(
  request: Request,
  env: WorkerEnv,
  program: Effect.Effect<A, unknown, ConvexService | AuthService | RequestContextService>,
): Promise<Response> => {
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";
  const { runtime } = getWorkerRuntime(env);
  const exit = await runtime.runPromiseExit(
    program.pipe(Effect.provideService(RequestContextService, makeServerRequestContext(request))),
  );

  if (exit._tag === "Failure") {
    const message = String(exit.cause);
    const status = statusFromErrorMessage(message);
    console.error(`[lightning] ${formatRequestIdLogToken(requestId)}`, message);
    return json({ ok: false, error: message }, { status, headers: { "cache-control": "no-store" } });
  }

  return json(exit.value, { status: 200, headers: { "cache-control": "no-store" } });
};

const createTaskProgram = (request: Request) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const body = yield* readJsonBody(request);
    const decoded = yield* decodeCreateBody(body);
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? undefined;

    const convex = yield* ConvexService;
    const created = yield* convex.mutation(api.lightning.tasks.createTask, {
      request: decoded.request,
      idempotencyKey: decoded.idempotencyKey,
      source: decoded.source ?? "web_worker_api",
      requestId,
      metadata: decoded.metadata,
    });

    return {
      ok: true as const,
      task: created.task,
      existed: created.existed,
      requestId: requestId ?? null,
    };
  });

const listTasksProgram = (request: Request) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const url = new URL(request.url);
    const status = yield* parseTaskStatus(url.searchParams.get("status"));
    const limit = parseLimit(url.searchParams.get("limit"));
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? null;

    const convex = yield* ConvexService;
    const listed = yield* convex.query(api.lightning.tasks.listTasks, {
      ...(status ? { status } : {}),
      ...(limit !== undefined ? { limit } : {}),
    });

    return {
      ok: true as const,
      tasks: listed.tasks,
      requestId,
    };
  });

const transitionTaskProgram = (request: Request, taskId: string) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const body = yield* readJsonBody(request);
    const decoded = yield* decodeTransitionBody(body);
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? undefined;

    const convex = yield* ConvexService;
    const transitioned = yield* convex.mutation(api.lightning.tasks.transitionTask, {
      taskId,
      toStatus: decoded.toStatus,
      actor: decoded.actor ?? "desktop_executor",
      reason: decoded.reason,
      requestId,
      errorCode: decoded.errorCode,
      errorMessage: decoded.errorMessage,
      metadata: decoded.metadata,
    });

    return {
      ok: true as const,
      changed: transitioned.changed,
      task: transitioned.task,
      event: transitioned.event ?? null,
      requestId: requestId ?? null,
    };
  });

export const handleLightningRequest = async (request: Request, env: WorkerEnv): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/lightning/")) return null;

  if (url.pathname === "/api/lightning/l402/tasks") {
    if (request.method === "POST") {
      return runAuthedLightning(request, env, createTaskProgram(request));
    }
    if (request.method === "GET") {
      return runAuthedLightning(request, env, listTasksProgram(request));
    }
    return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
  }

  const transitionMatch = /^\/api\/lightning\/l402\/tasks\/([^/]+)\/transition$/.exec(url.pathname);
  if (transitionMatch) {
    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
    }
    const taskId = decodeURIComponent(transitionMatch[1] ?? "").trim();
    if (!taskId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }
    return runAuthedLightning(request, env, transitionTaskProgram(request, taskId));
  }

  return new Response("Not found", { status: 404, headers: { "cache-control": "no-store" } });
};
