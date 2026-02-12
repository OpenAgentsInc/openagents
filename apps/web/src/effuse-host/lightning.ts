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

const PaywallStatusSchema = Schema.Literal("active", "paused", "archived");
type PaywallStatus = typeof PaywallStatusSchema.Type;

const PaywallPolicySchema = Schema.Struct({
  pricingMode: Schema.Literal("fixed"),
  fixedAmountMsats: Schema.Number,
  maxPerRequestMsats: Schema.optional(Schema.Number),
  allowedHosts: Schema.optional(Schema.Array(Schema.String)),
  blockedHosts: Schema.optional(Schema.Array(Schema.String)),
  quotaPerMinute: Schema.optional(Schema.Number),
  quotaPerDay: Schema.optional(Schema.Number),
  killSwitch: Schema.optional(Schema.Boolean),
});

const PaywallRouteSchema = Schema.Struct({
  hostPattern: Schema.String,
  pathPattern: Schema.String,
  upstreamUrl: Schema.String,
  protocol: Schema.optional(Schema.Literal("http", "https")),
  timeoutMs: Schema.optional(Schema.Number),
  priority: Schema.optional(Schema.Number),
});

const CreatePaywallBodySchema = Schema.Struct({
  name: Schema.NonEmptyString,
  description: Schema.optional(Schema.String),
  status: Schema.optional(Schema.Literal("active", "paused")),
  policy: PaywallPolicySchema,
  routes: Schema.Array(PaywallRouteSchema),
  metadata: Schema.optional(Schema.Unknown),
});
type CreatePaywallBody = typeof CreatePaywallBodySchema.Type;

const UpdatePaywallBodySchema = Schema.Struct({
  name: Schema.optional(Schema.NonEmptyString),
  description: Schema.optional(Schema.String),
  policy: Schema.optional(PaywallPolicySchema),
  routes: Schema.optional(Schema.Array(PaywallRouteSchema)),
  metadata: Schema.optional(Schema.Unknown),
});
type UpdatePaywallBody = typeof UpdatePaywallBodySchema.Type;

const PauseResumePaywallBodySchema = Schema.Struct({
  reason: Schema.optional(Schema.String),
  metadata: Schema.optional(Schema.Unknown),
});
type PauseResumePaywallBody = typeof PauseResumePaywallBodySchema.Type;

const decodeCreatePaywallBody = (value: unknown): Effect.Effect<CreatePaywallBody, Error> =>
  Schema.decodeUnknown(CreatePaywallBodySchema)(value).pipe(Effect.mapError(() => new Error("invalid_input")));

const decodeUpdatePaywallBody = (value: unknown): Effect.Effect<UpdatePaywallBody, Error> =>
  Schema.decodeUnknown(UpdatePaywallBodySchema)(value).pipe(Effect.mapError(() => new Error("invalid_input")));

const decodePauseResumePaywallBody = (value: unknown): Effect.Effect<PauseResumePaywallBody, Error> =>
  Schema.decodeUnknown(PauseResumePaywallBodySchema)(value).pipe(Effect.mapError(() => new Error("invalid_input")));

const toMutablePaywallPolicy = (
  policy: CreatePaywallBody["policy"] | UpdatePaywallBody["policy"],
) => {
  if (!policy) return undefined;
  return {
    pricingMode: policy.pricingMode,
    fixedAmountMsats: policy.fixedAmountMsats,
    maxPerRequestMsats: policy.maxPerRequestMsats,
    allowedHosts: policy.allowedHosts ? [...policy.allowedHosts] : undefined,
    blockedHosts: policy.blockedHosts ? [...policy.blockedHosts] : undefined,
    quotaPerMinute: policy.quotaPerMinute,
    quotaPerDay: policy.quotaPerDay,
    killSwitch: policy.killSwitch,
  };
};

const toMutablePaywallRoutes = (routes: ReadonlyArray<CreatePaywallBody["routes"][number]>) =>
  routes.map((route) => ({
    hostPattern: route.hostPattern,
    pathPattern: route.pathPattern,
    upstreamUrl: route.upstreamUrl,
    protocol: route.protocol,
    timeoutMs: route.timeoutMs,
    priority: route.priority,
  }));

const readJsonBody = (request: Request): Effect.Effect<unknown, Error> =>
  Effect.tryPromise({
    try: () => request.json(),
    catch: () => new Error("invalid_input"),
  });

// Error taxonomy is intentionally deterministic so callers can branch safely:
// unauthorized -> 401, invalid_input -> 400, forbidden -> 403, not_found -> 404,
// invalid_transition / route_conflict -> 409, policy_violation -> 422.
const statusFromErrorMessage = (message: string): number => {
  const m = message.toLowerCase();
  if (m.includes("unauthorized")) return 401;
  if (m.includes("invalid_input")) return 400;
  if (m.includes("paywall_not_found") || m.includes("task_not_found")) return 404;
  if (m.includes("forbidden")) return 403;
  if (m.includes("invalid_transition")) return 409;
  if (m.includes("route_conflict")) return 409;
  if (m.includes("policy_violation")) return 422;
  return 500;
};

const parseLimit = (value: string | null): number | undefined => {
  if (!value) return undefined;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return undefined;
  return Math.max(1, Math.min(200, Math.floor(parsed)));
};

const parseBeforeCreatedAtMs = (value: string | null): number | undefined => {
  if (!value) return undefined;
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return undefined;
  return Math.floor(parsed);
};

const parseTaskStatus = (value: string | null): Effect.Effect<LightningTaskStatus | undefined, Error> => {
  if (!value || value.trim().length === 0) return Effect.succeed(undefined);
  return Schema.decodeUnknown(LightningTaskStatusSchema)(value.trim()).pipe(
    Effect.mapError(() => new Error("invalid_input")),
  );
};

const parsePaywallStatus = (value: string | null): Effect.Effect<PaywallStatus | undefined, Error> => {
  if (!value || value.trim().length === 0) return Effect.succeed(undefined);
  return Schema.decodeUnknown(PaywallStatusSchema)(value.trim()).pipe(
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

const createPaywallProgram = (request: Request) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const body = yield* readJsonBody(request);
    const decoded = yield* decodeCreatePaywallBody(body);
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? undefined;

    const convex = yield* ConvexService;
    const created = yield* convex.mutation(api.lightning.paywalls.createPaywall, {
      name: decoded.name,
      description: decoded.description,
      status: decoded.status,
      policy: toMutablePaywallPolicy(decoded.policy)!,
      routes: toMutablePaywallRoutes(decoded.routes),
      requestId,
      metadata: decoded.metadata,
    });

    return {
      ok: true as const,
      paywall: created.paywall,
      requestId: requestId ?? null,
    };
  });

const listPaywallsProgram = (request: Request) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const url = new URL(request.url);
    const status = yield* parsePaywallStatus(url.searchParams.get("status"));
    const limit = parseLimit(url.searchParams.get("limit"));
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? null;

    const convex = yield* ConvexService;
    const listed = yield* convex.query(api.lightning.paywalls.listPaywalls, {
      ...(status ? { status } : {}),
      ...(limit !== undefined ? { limit } : {}),
    });

    return {
      ok: true as const,
      paywalls: listed.paywalls,
      requestId,
    };
  });

const getSecurityStateProgram = (request: Request) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? null;
    const convex = yield* ConvexService;
    const state = yield* convex.query(api.lightning.security.getOwnerSecurityState, {});

    return {
      ok: true as const,
      security: state,
      requestId,
    };
  });

const listOwnerSettlementsProgram = (request: Request) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const url = new URL(request.url);
    const limit = parseLimit(url.searchParams.get("limit"));
    const beforeCreatedAtMs = parseBeforeCreatedAtMs(url.searchParams.get("beforeCreatedAtMs"));
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? null;

    const convex = yield* ConvexService;
    const listed = yield* convex.query(api.lightning.settlements.listOwnerSettlements, {
      ...(limit !== undefined ? { limit } : {}),
      ...(beforeCreatedAtMs !== undefined ? { beforeCreatedAtMs } : {}),
    });

    return {
      ok: true as const,
      settlements: listed.settlements,
      nextCursor: listed.nextCursor,
      requestId,
    };
  });

const listPaywallSettlementsProgram = (request: Request, paywallId: string) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const url = new URL(request.url);
    const limit = parseLimit(url.searchParams.get("limit"));
    const beforeCreatedAtMs = parseBeforeCreatedAtMs(url.searchParams.get("beforeCreatedAtMs"));
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? null;

    const convex = yield* ConvexService;
    const listed = yield* convex.query(api.lightning.settlements.listPaywallSettlements, {
      paywallId,
      ...(limit !== undefined ? { limit } : {}),
      ...(beforeCreatedAtMs !== undefined ? { beforeCreatedAtMs } : {}),
    });

    return {
      ok: true as const,
      settlements: listed.settlements,
      nextCursor: listed.nextCursor,
      requestId,
    };
  });

const getPaywallProgram = (request: Request, paywallId: string) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? null;
    const convex = yield* ConvexService;
    const result = yield* convex.query(api.lightning.paywalls.getPaywall, { paywallId });
    return {
      ok: true as const,
      paywall: result.paywall,
      requestId,
    };
  });

const updatePaywallProgram = (request: Request, paywallId: string) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const body = yield* readJsonBody(request);
    const decoded = yield* decodeUpdatePaywallBody(body);
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? undefined;
    const convex = yield* ConvexService;

    const result = yield* convex.mutation(api.lightning.paywalls.updatePaywall, {
      paywallId,
      name: decoded.name,
      description: decoded.description,
      policy: toMutablePaywallPolicy(decoded.policy),
      routes: decoded.routes ? toMutablePaywallRoutes(decoded.routes) : undefined,
      requestId,
      metadata: decoded.metadata,
    });

    return {
      ok: true as const,
      paywall: result.paywall,
      requestId: requestId ?? null,
    };
  });

const pausePaywallProgram = (request: Request, paywallId: string) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const body = yield* readJsonBody(request).pipe(
      Effect.orElseSucceed(() => ({})),
    );
    const decoded = yield* decodePauseResumePaywallBody(body);
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? undefined;
    const convex = yield* ConvexService;

    const result = yield* convex.mutation(api.lightning.paywalls.pausePaywall, {
      paywallId,
      requestId,
      reason: decoded.reason,
    });

    return {
      ok: true as const,
      changed: result.changed,
      paywall: result.paywall,
      requestId: requestId ?? null,
    };
  });

const resumePaywallProgram = (request: Request, paywallId: string) =>
  Effect.gen(function* () {
    const auth = yield* AuthService;
    const session = yield* auth.getSession();
    if (!session.userId) return yield* Effect.fail(new Error("unauthorized"));

    const body = yield* readJsonBody(request).pipe(
      Effect.orElseSucceed(() => ({})),
    );
    const decoded = yield* decodePauseResumePaywallBody(body);
    const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? undefined;
    const convex = yield* ConvexService;

    const result = yield* convex.mutation(api.lightning.paywalls.resumePaywall, {
      paywallId,
      requestId,
      reason: decoded.reason,
    });

    return {
      ok: true as const,
      changed: result.changed,
      paywall: result.paywall,
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

  if (url.pathname === "/api/lightning/paywalls") {
    if (request.method === "POST") {
      return runAuthedLightning(request, env, createPaywallProgram(request));
    }
    if (request.method === "GET") {
      return runAuthedLightning(request, env, listPaywallsProgram(request));
    }
    return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
  }

  if (url.pathname === "/api/lightning/settlements") {
    if (request.method === "GET") {
      return runAuthedLightning(request, env, listOwnerSettlementsProgram(request));
    }
    return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
  }

  if (url.pathname === "/api/lightning/security/state") {
    if (request.method === "GET") {
      return runAuthedLightning(request, env, getSecurityStateProgram(request));
    }
    return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
  }

  const paywallMatch = /^\/api\/lightning\/paywalls\/([^/]+)$/.exec(url.pathname);
  if (paywallMatch) {
    const paywallId = decodeURIComponent(paywallMatch[1] ?? "").trim();
    if (!paywallId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }
    if (request.method === "GET") {
      return runAuthedLightning(request, env, getPaywallProgram(request, paywallId));
    }
    if (request.method === "PATCH") {
      return runAuthedLightning(request, env, updatePaywallProgram(request, paywallId));
    }
    return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
  }

  const paywallSettlementsMatch = /^\/api\/lightning\/paywalls\/([^/]+)\/settlements$/.exec(url.pathname);
  if (paywallSettlementsMatch) {
    if (request.method !== "GET") {
      return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
    }
    const paywallId = decodeURIComponent(paywallSettlementsMatch[1] ?? "").trim();
    if (!paywallId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }
    return runAuthedLightning(request, env, listPaywallSettlementsProgram(request, paywallId));
  }

  const paywallPauseMatch = /^\/api\/lightning\/paywalls\/([^/]+)\/pause$/.exec(url.pathname);
  if (paywallPauseMatch) {
    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
    }
    const paywallId = decodeURIComponent(paywallPauseMatch[1] ?? "").trim();
    if (!paywallId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }
    return runAuthedLightning(request, env, pausePaywallProgram(request, paywallId));
  }

  const paywallResumeMatch = /^\/api\/lightning\/paywalls\/([^/]+)\/resume$/.exec(url.pathname);
  if (paywallResumeMatch) {
    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
    }
    const paywallId = decodeURIComponent(paywallResumeMatch[1] ?? "").trim();
    if (!paywallId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }
    return runAuthedLightning(request, env, resumePaywallProgram(request, paywallId));
  }

  return new Response("Not found", { status: 404, headers: { "cache-control": "no-store" } });
};
