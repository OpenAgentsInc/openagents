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

export const handleLightningRequest = async (request: Request, env: WorkerEnv): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/lightning/")) return null;

  if (url.pathname === "/api/lightning/l402/tasks") {
    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405, headers: { "cache-control": "no-store" } });
    }
    return runAuthedLightning(request, env, createTaskProgram(request));
  }

  return new Response("Not found", { status: 404, headers: { "cache-control": "no-store" } });
};
