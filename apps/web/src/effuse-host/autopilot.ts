import * as AiLanguageModel from "@effect/ai/LanguageModel";
import * as AiPrompt from "@effect/ai/Prompt";
import * as AiResponse from "@effect/ai/Response";
import * as AiToolkit from "@effect/ai/Toolkit";
import { Effect, Layer, Schema, Stream } from "effect";

import { makeWorkersAiLanguageModel } from "../../../autopilot-worker/src/effect/ai/languageModel";
import { Lm, Predict } from "@openagentsinc/dse";
import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";

import { api } from "../../convex/_generated/api";
import { AuthService } from "../effect/auth";
import { ConvexService, type ConvexServiceApi } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";
import { TelemetryService } from "../effect/telemetry";

import { layerDsePredictEnvForAutopilotRun, makeWorkersAiDseLmClient } from "./dse";
import { getWorkerRuntime } from "./runtime";
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import type { WorkerEnv } from "./env";

const MODEL_ID = "@cf/openai/gpt-oss-120b";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;

const predictBlueprintSelectTool = Predict.make(dseCatalogSignatures.blueprint_select_tool);

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  });

const readJson = async (request: Request): Promise<any> => {
  try {
    return await request.json();
  } catch {
    return null;
  }
};

type SendBody = {
  readonly threadId?: unknown;
  readonly anonKey?: unknown;
  readonly text?: unknown;
};

type CancelBody = {
  readonly threadId?: unknown;
  readonly anonKey?: unknown;
  readonly runId?: unknown;
};

type RunHandle = {
  readonly controller: AbortController;
  readonly startedAtMs: number;
};

// Best-effort per-isolate cancel map. We also persist cancelRequested in Convex so
// the streaming loop can observe cancellation even if the cancel request hits a
// different isolate.
const activeRuns = new Map<string, RunHandle>();

const emptyToolkit = AiToolkit.make();
const encodeStreamPart = Schema.encodeSync(AiResponse.StreamPart(emptyToolkit));

const shouldIgnoreWirePart = (part: AiResponse.StreamPartEncoded): boolean =>
  part.type === "reasoning-start" || part.type === "reasoning-delta" || part.type === "reasoning-end";

const concatTextFromPromptMessages = (
  messages: ReadonlyArray<{ readonly role: string; readonly text: string }>,
): AiPrompt.RawInput => {
  const out: Array<any> = [];

  // Minimal system prompt (MVP). Blueprint context/tool prompts can be layered later.
  out.push({
    role: "system",
    content:
      "You are Autopilot.\n" +
      "- Be concise, direct, and pragmatic.\n" +
      "- Do not claim web browsing capability.\n" +
      "- Do not reveal internal reasoning.\n",
  });

  for (const m of messages) {
    const role = m.role === "assistant" ? "assistant" : "user";
    out.push({ role, content: [{ type: "text", text: m.text }] } as any);
  }

  return out as unknown as AiPrompt.RawInput;
};

const flushPartsToConvex = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly anonKey: string | null;
  readonly runId: string;
  readonly messageId: string;
  readonly parts: ReadonlyArray<{ readonly seq: number; readonly part: unknown }>;
}) =>
  input.convex.mutation(api.autopilot.messages.appendParts, {
    threadId: input.threadId,
    ...(input.anonKey ? { anonKey: input.anonKey } : {}),
    runId: input.runId,
    messageId: input.messageId,
    parts: input.parts.map((p) => ({ seq: p.seq, part: p.part })),
  } as any);

const finalizeRunInConvex = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly anonKey: string | null;
  readonly runId: string;
  readonly messageId: string;
  readonly status: "final" | "error" | "canceled";
  readonly text?: string | undefined;
}) =>
  input.convex.mutation(api.autopilot.messages.finalizeRun, {
    threadId: input.threadId,
    ...(input.anonKey ? { anonKey: input.anonKey } : {}),
    runId: input.runId,
    messageId: input.messageId,
    status: input.status,
    ...(typeof input.text === "string" ? { text: input.text } : {}),
  } as any);

const isCancelRequested = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly anonKey: string | null;
  readonly runId: string;
}) =>
  input.convex.query(api.autopilot.messages.isCancelRequested, {
    threadId: input.threadId,
    ...(input.anonKey ? { anonKey: input.anonKey } : {}),
    runId: input.runId,
  } as any);

const runAutopilotStream = (input: {
  readonly env: WorkerEnv & { readonly AI: Ai };
  readonly request: Request;
  readonly threadId: string;
  readonly anonKey: string | null;
  readonly runId: string;
  readonly assistantMessageId: string;
  readonly controller: AbortController;
}) => {
  const { runtime } = getWorkerRuntime(input.env);
  const url = new URL(input.request.url);
  const requestId = input.request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";
  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService;
    }),
  );
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: input.request.method,
    pathname: url.pathname,
  });

  const effect = Effect.gen(function* () {
    const telemetry = yield* TelemetryService;
    const convex = yield* ConvexService;

    const t = telemetry.withNamespace("autopilot.stream");
    yield* t.event("run.started", { threadId: input.threadId, runId: input.runId });

    // Load prompt context from Convex (messages only; omit parts).
    const snapshot = yield* convex.query(api.autopilot.messages.getThreadSnapshot, {
      threadId: input.threadId,
      ...(input.anonKey ? { anonKey: input.anonKey } : {}),
      maxMessages: 120,
      maxParts: 0,
    } as any);

    const messagesRaw = Array.isArray((snapshot as any)?.messages) ? ((snapshot as any).messages as any[]) : [];

    const promptMessages = messagesRaw
      .filter((m) => m && typeof m === "object")
      .filter((m) => String(m.role ?? "") === "user" || String(m.role ?? "") === "assistant")
      .filter((m) => String(m.status ?? "") !== "streaming" || String(m.messageId ?? "") !== input.assistantMessageId)
      .map((m) => ({ role: String(m.role ?? "user"), text: String(m.text ?? "") }))
      .filter((m) => m.text.trim().length > 0);

    const tail = promptMessages.slice(-MAX_CONTEXT_MESSAGES);
    const rawPrompt = concatTextFromPromptMessages(tail);
    const prompt = AiPrompt.make(rawPrompt);

    const modelLayer = Layer.effect(
      AiLanguageModel.LanguageModel,
      makeWorkersAiLanguageModel({
        binding: input.env.AI,
        model: MODEL_ID,
        maxOutputTokens: MAX_OUTPUT_TOKENS,
      }),
    );

    // Chunking policy.
    const FLUSH_INTERVAL_MS = 350;
    const FLUSH_MAX_TEXT_CHARS = 1200;
    const FLUSH_MAX_PARTS = 32;

    let seq = 0;
    let bufferedDelta = "";
    let bufferedParts: Array<{ readonly seq: number; readonly part: unknown }> = [];
    let lastFlushAtMs = Date.now();
    let outputText = "";
    let cancelCheckAtMs = 0;

    const materializeDelta = () => {
      if (bufferedDelta.length === 0) return;
      const part: AiResponse.StreamPartEncoded = { type: "text-delta", delta: bufferedDelta } as any;
      bufferedParts.push({ seq: seq++, part });
      bufferedDelta = "";
    };

    const flush = Effect.fn("autopilot.flush")(function* (force: boolean) {
      if (input.controller.signal.aborted) return;

      const now = Date.now();
      const elapsed = now - lastFlushAtMs;

      const shouldFlushNow =
        force ||
        bufferedDelta.length >= FLUSH_MAX_TEXT_CHARS ||
        bufferedParts.length >= FLUSH_MAX_PARTS ||
        elapsed >= FLUSH_INTERVAL_MS;

      if (!shouldFlushNow) return;

      materializeDelta();

      if (bufferedParts.length === 0) return;

      // Check cancellation at most once per flush interval.
      if (now - cancelCheckAtMs >= FLUSH_INTERVAL_MS) {
        cancelCheckAtMs = now;
        const cancel = yield* isCancelRequested({
          convex,
          threadId: input.threadId,
          anonKey: input.anonKey,
          runId: input.runId,
        }).pipe(Effect.catchAll(() => Effect.succeed({ ok: true, cancelRequested: false } as any)));
        if ((cancel as any)?.cancelRequested) {
          input.controller.abort();
          return;
        }
      }

      const batch = bufferedParts;
      bufferedParts = [];
      lastFlushAtMs = now;

      yield* flushPartsToConvex({
        convex,
        threadId: input.threadId,
        anonKey: input.anonKey,
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: batch,
      }).pipe(
        Effect.catchAll((err) =>
          t.log("error", "convex.append_failed", { message: err instanceof Error ? err.message : String(err) }),
        ),
      );
    });

    // Stage 3: Execute one DSE signature in the MVP hot path (Convex-first).
    // This is best-effort and must never block the main chat response.
    yield* Effect.gen(function* () {
      if (input.controller.signal.aborted) return;

      const lastUserMessageText = (() => {
        for (let i = messagesRaw.length - 1; i >= 0; i--) {
          const m = messagesRaw[i];
          if (!m || typeof m !== "object") continue;
          if (String((m as any).role ?? "") !== "user") continue;
          const text = String((m as any).text ?? "");
          if (text.trim()) return text;
        }
        return "";
      })();
      if (!lastUserMessageText) return;

      // Fetch Blueprint hint from Convex (tiny, stable context).
      const bp = yield* convex
        .query(api.autopilot.blueprint.getBlueprint, {
          threadId: input.threadId,
          ...(input.anonKey ? { anonKey: input.anonKey } : {}),
        } as any)
        .pipe(Effect.catchAll(() => Effect.succeed({ ok: true, blueprint: null } as any)));

      const blueprint = (bp as any)?.blueprint ?? null;
      const userHandle = String(blueprint?.docs?.user?.addressAs ?? "Unknown");
      const agentName = String(blueprint?.docs?.identity?.name ?? "Autopilot");

      const signatureId = dseCatalogSignatures.blueprint_select_tool.id;
      const partId = `dsepart_sig_${input.runId}_blueprint_select_tool`;

      bufferedParts.push({
        seq: seq++,
        part: {
          type: "dse.signature",
          v: 1,
          id: partId,
          state: "start",
          tsMs: Date.now(),
          signatureId,
        },
      });
      yield* flush(true);

      let recordedReceipt: any | null = null;

      const dseLmClient = makeWorkersAiDseLmClient({ binding: input.env.AI, defaultModelId: MODEL_ID });
      const dseEnv = layerDsePredictEnvForAutopilotRun({
        threadId: input.threadId,
        anonKey: input.anonKey,
        runId: input.runId,
        onReceipt: (r) => {
          recordedReceipt = r as any;
        },
      });

      const exit = yield* Effect.exit(
        predictBlueprintSelectTool({
          message: lastUserMessageText,
          blueprintHint: { userHandle, agentName },
        }).pipe(
          Effect.provideService(Lm.LmClientService, dseLmClient),
          Effect.provide(dseEnv),
        ),
      );

      const state = exit._tag === "Success" ? "ok" : "error";

      const errorText =
        exit._tag === "Failure"
          ? (() => {
              const cause = (exit as any)?.cause;
              if (cause && typeof cause === "object" && "message" in cause) return String((cause as any).message);
              return String(cause ?? "DSE predict failed");
            })()
          : null;

      bufferedParts.push({
        seq: seq++,
        part: {
          type: "dse.signature",
          v: 1,
          id: partId,
          state,
          tsMs: Date.now(),
          signatureId,
          ...(recordedReceipt?.compiled_id ? { compiled_id: String(recordedReceipt.compiled_id) } : {}),
          ...(recordedReceipt?.timing?.durationMs ? { timing: { durationMs: Number(recordedReceipt.timing.durationMs) } } : {}),
          ...(recordedReceipt?.budget ? { budget: recordedReceipt.budget } : {}),
          ...(recordedReceipt?.receiptId ? { receiptId: String(recordedReceipt.receiptId) } : {}),
          ...(exit._tag === "Success" ? { outputPreview: (exit as any).value } : {}),
          ...(errorText ? { errorText } : {}),
        },
      });
      yield* flush(true);
    }).pipe(
      // Must never block the main chat response.
      Effect.catchAllCause((cause) => t.log("warn", "dse.router_failed", { message: String(cause) })),
    );

    const stream = AiLanguageModel.streamText({
      prompt,
      toolChoice: "none",
      disableToolCallResolution: true,
    });

    const runStream = Stream.runForEach(stream, (part) =>
      Effect.sync(() => {
        if (input.controller.signal.aborted) return;
        const encoded = encodeStreamPart(part as any);
        if (shouldIgnoreWirePart(encoded as any)) return;

        if (encoded.type === "text-delta") {
          const delta = String((encoded as any).delta ?? "");
          bufferedDelta += delta;
          outputText += delta;
          return;
        }

        // For non-delta parts, materialize any pending delta first to preserve ordering.
        materializeDelta();

        // Encoded parts are small; we buffer them and flush on the same cadence as text.
        bufferedParts.push({ seq: seq++, part: encoded as any });
      }).pipe(Effect.zipRight(flush(false))),
    ).pipe(Effect.provide(modelLayer));

    let status: "final" | "error" | "canceled" = "final";

    try {
      yield* runStream;
      yield* flush(true);

      if (input.controller.signal.aborted) {
        status = "canceled";
      }
    } catch (err) {
      status = input.controller.signal.aborted ? "canceled" : "error";
      yield* t.log("error", "run.failed", { message: err instanceof Error ? err.message : String(err) });
      // Best-effort: append a terminal error part.
      const errorPart: AiResponse.StreamPartEncoded = { type: "error", error: String(err) } as any;
      yield* flushPartsToConvex({
        convex,
        threadId: input.threadId,
        anonKey: input.anonKey,
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: [{ seq: seq++, part: errorPart }],
      }).pipe(Effect.catchAll(() => Effect.void));
    } finally {
      yield* finalizeRunInConvex({
        convex,
        threadId: input.threadId,
        anonKey: input.anonKey,
        runId: input.runId,
        messageId: input.assistantMessageId,
        status,
        text: outputText,
      }).pipe(Effect.catchAll(() => Effect.void));

      yield* t.event("run.finished", { threadId: input.threadId, runId: input.runId, status });
    }
  }).pipe(
    Effect.provideService(RequestContextService, makeServerRequestContext(input.request)),
    Effect.provideService(TelemetryService, requestTelemetry),
    Effect.catchAll((err) => {
      console.error(`[autopilot.stream] ${formatRequestIdLogToken(requestId)}`, err);
      return Effect.void;
    }),
  );

  return runtime.runPromise(effect).finally(() => {
    activeRuns.delete(input.runId);
  });
};

export const handleAutopilotRequest = async (
  request: Request,
  env: WorkerEnv,
  ctx: ExecutionContext,
): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/autopilot/")) return null;
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";

  if (url.pathname === "/api/autopilot/send") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = (await readJson(request)) as SendBody | null;
    const threadId = typeof body?.threadId === "string" ? body.threadId : "";
    const anonKey = typeof body?.anonKey === "string" ? body.anonKey : null;
    const text = typeof body?.text === "string" ? body.text : "";

    if (!threadId || !text.trim()) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    if (!env.AI) {
      return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });
    const exit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        // Ensure auth is loaded (so ConvexService server client can setAuth token).
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        return yield* convex.mutation(api.autopilot.messages.createRun, {
          threadId,
          ...(anonKey ? { anonKey } : {}),
          text,
        } as any);
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(request)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (exit._tag === "Failure") {
      console.error(`[autopilot.send] ${formatRequestIdLogToken(requestId)} create_run_failed`, exit.cause);
      return json({ ok: false, error: "create_run_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const value = exit.value as any;
    const runId = String(value.runId ?? "");
    const assistantMessageId = String(value.assistantMessageId ?? "");

	    const controller = new AbortController();
	    activeRuns.set(runId, { controller, startedAtMs: Date.now() });

	    const envWithAi = env as WorkerEnv & { readonly AI: Ai };
	    ctx.waitUntil(
	      runAutopilotStream({
	        env: envWithAi,
	        request,
	        threadId,
	        anonKey,
	        runId,
	        assistantMessageId,
	        controller,
	      }),
	    );

    return json(
      { ok: true, threadId, runId, assistantMessageId },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/cancel") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = (await readJson(request)) as CancelBody | null;
    const threadId = typeof body?.threadId === "string" ? body.threadId : "";
    const anonKey = typeof body?.anonKey === "string" ? body.anonKey : null;
    const runId = typeof body?.runId === "string" ? body.runId : "";
    if (!threadId || !runId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    // Best-effort abort (in-isolate).
    activeRuns.get(runId)?.controller.abort();

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });
    const exit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));
        return yield* convex.mutation(api.autopilot.messages.requestCancel, {
          threadId,
          ...(anonKey ? { anonKey } : {}),
          runId,
        } as any);
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(request)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (exit._tag === "Failure") {
      console.error(`[autopilot.cancel] ${formatRequestIdLogToken(requestId)} cancel_failed`, exit.cause);
      return json({ ok: false, error: "cancel_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    return json({ ok: true }, { status: 200, headers: { "cache-control": "no-store" } });
  }

  return new Response("Not found", { status: 404, headers: { "cache-control": "no-store" } });
};
