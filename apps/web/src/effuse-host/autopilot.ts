import * as AiLanguageModel from "@effect/ai/LanguageModel";
import * as AiPrompt from "@effect/ai/Prompt";
import * as AiResponse from "@effect/ai/Response";
import * as AiToolkit from "@effect/ai/Toolkit";
import { Effect, Layer, Schema, Stream } from "effect";

import { makeWorkersAiLanguageModel } from "../../../autopilot-worker/src/effect/ai/languageModel";
import { makeOpenRouterLanguageModel } from "../../../autopilot-worker/src/effect/ai/openRouterLanguageModel";
import { makeFallbackLanguageModel } from "../../../autopilot-worker/src/effect/ai/fallbackLanguageModel";

import { BlobStore, Lm, Predict } from "@openagentsinc/dse";
import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";

import { api } from "../../convex/_generated/api";
import { AuthService } from "../effect/auth";
import { ConvexService, type ConvexServiceApi } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";
import { TelemetryService } from "../effect/telemetry";

import { layerDsePredictEnvForAutopilotRun, makeDseLmClientWithOpenRouterPrimary } from "./dse";
import { getWorkerRuntime } from "./runtime";
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import type { WorkerEnv } from "./env";
import type {
  CreateRunResult,
  GetBlueprintResult,
  GetThreadSnapshotResult,
  IsCancelRequestedResult,
  ThreadSnapshotMessage,
} from "./convexTypes";

/** Cloudflare Workers AI model (fallback when OpenRouter is used or only option when OPENROUTER_API_KEY is unset). */
const MODEL_ID_CF = "@cf/openai/gpt-oss-120b";
/** OpenRouter model used as primary when OPENROUTER_API_KEY is set. */
const PRIMARY_MODEL_OPENROUTER = "moonshotai/kimi-k2.5";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;

const predictRlmSummarizeThread = Predict.make(dseCatalogSignatures.rlm_summarize_thread);

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
  readonly text?: unknown;
};

type CancelBody = {
  readonly threadId?: unknown;
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

/** Blueprint shape (minimal) for bootstrap-aware prompt. */
type BlueprintHint = {
  readonly bootstrapState?: {
    readonly status?: string;
    readonly stage?: string;
  };
  readonly docs?: {
    readonly user?: { readonly addressAs?: string };
    readonly identity?: { readonly name?: string };
  };
} | null;

const BOOTSTRAP_ASK_USER_HANDLE_SYSTEM =
  "\n\nBootstrap (strict): You are collecting the user's preferred handle (what to call them). " +
  "Do NOT say generic greetings like \"Hello! How can I assist you today?\" or \"How can I help?\". " +
  "If the user has not given a name (e.g. they said \"hi\", \"hello\", or something that is not a name), " +
  "respond only by re-asking: \"What shall I call you?\" Do not add filler. " +
  "If they give a name/handle, confirm and immediately ask the next bootstrap question: " +
  "\"What should you call me?\" (Default: Autopilot). Ask one question at a time.";

const BOOTSTRAP_ASK_AGENT_NAME_SYSTEM =
  "\n\nBootstrap (strict): You are collecting what the user should call you (your name). " +
  "Ask only: \"What should you call me?\" (Default: Autopilot). " +
  "If they give a name, confirm and immediately ask the next bootstrap question: " +
  "\"Pick one short operating vibe for me.\"";

const BOOTSTRAP_ASK_VIBE_SYSTEM =
  "\n\nBootstrap (strict): You are collecting a short operating vibe for yourself (one short phrase). " +
  "Ask only: \"Pick one short operating vibe for me.\" " +
  "If they give a vibe, confirm and immediately ask the next bootstrap question: " +
  "\"Any boundaries or preferences? Reply 'none' or list a few bullets.\"";

const BOOTSTRAP_ASK_BOUNDARIES_SYSTEM =
  "\n\nBootstrap (strict): You are collecting optional boundaries/preferences. " +
  "Ask only: \"Any boundaries or preferences? Reply 'none' or list a few bullets.\" " +
  "If they say none, confirm setup is complete and ask: \"What would you like to do first?\" " +
  "If they provide boundaries, acknowledge them, confirm setup is complete, and ask: \"What would you like to do first?\"";

const concatTextFromPromptMessages = (
  messages: ReadonlyArray<{ readonly role: string; readonly text: string }>,
  blueprint: BlueprintHint = null,
  options?: { readonly extraSystem?: string | undefined },
): AiPrompt.RawInput => {
  const out: Array<{ role: string; content: string | Array<{ type: "text"; text: string }> }> = [];

  let systemContent =
    "You are Autopilot.\n" +
    "- Be concise, direct, and pragmatic.\n" +
    "- Do not claim web browsing capability.\n" +
    "- Do not reveal internal reasoning.\n";

  const status = blueprint?.bootstrapState?.status;
  const stage = blueprint?.bootstrapState?.stage;
  if (status !== "complete") {
    if (stage === "ask_user_handle") systemContent += BOOTSTRAP_ASK_USER_HANDLE_SYSTEM;
    if (stage === "ask_agent_name") systemContent += BOOTSTRAP_ASK_AGENT_NAME_SYSTEM;
    if (stage === "ask_vibe") systemContent += BOOTSTRAP_ASK_VIBE_SYSTEM;
    if (stage === "ask_boundaries") systemContent += BOOTSTRAP_ASK_BOUNDARIES_SYSTEM;
  }

  const extraSystem = options?.extraSystem;
  if (extraSystem && extraSystem.trim().length > 0) {
    systemContent += "\n\n" + extraSystem.trim();
  }

  out.push({ role: "system", content: systemContent });

  for (const m of messages) {
    const role = m.role === "assistant" ? "assistant" : "user";
    out.push({ role, content: [{ type: "text" as const, text: m.text }] });
  }

  return out as unknown as AiPrompt.RawInput;
};

const lastUserTextFromSnapshot = (messages: ReadonlyArray<ThreadSnapshotMessage>): string => {
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (!m || typeof m !== "object") continue;
    if (String(m.role ?? "") !== "user") continue;
    const text = String(m.text ?? "");
    if (text.trim()) return text;
  }
  return "";
};

const flushPartsToConvex = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly runId: string;
  readonly messageId: string;
  readonly parts: ReadonlyArray<{ readonly seq: number; readonly part: unknown }>;
}) =>
  input.convex.mutation(api.autopilot.messages.appendParts, {
    threadId: input.threadId,
    runId: input.runId,
    messageId: input.messageId,
    parts: input.parts.map((p) => ({ seq: p.seq, part: p.part })),
  });

const finalizeRunInConvex = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly runId: string;
  readonly messageId: string;
  readonly status: "final" | "error" | "canceled";
  readonly text?: string | undefined;
}) =>
  input.convex.mutation(api.autopilot.messages.finalizeRun, {
    threadId: input.threadId,
    runId: input.runId,
    messageId: input.messageId,
    status: input.status,
    ...(typeof input.text === "string" ? { text: input.text } : {}),
  });

const isCancelRequested = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly runId: string;
}) =>
  input.convex.query(api.autopilot.messages.isCancelRequested, {
    threadId: input.threadId,
    runId: input.runId,
  });

const runAutopilotStream = (input: {
  readonly env: WorkerEnv & { readonly AI: Ai };
  readonly request: Request;
  readonly threadId: string;
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

    // Load prompt context from Convex (messages only; omit parts). Owner-only.
    const rawSnapshot = yield* convex.query(api.autopilot.messages.getThreadSnapshot, {
      threadId: input.threadId,
      maxMessages: 120,
      maxParts: 0,
    });
    const snapshot: GetThreadSnapshotResult = rawSnapshot as GetThreadSnapshotResult;

    const bp = yield* convex
      .query(api.autopilot.blueprint.getBlueprint, {
        threadId: input.threadId,
      })
      .pipe(
        Effect.catchAll(() =>
          Effect.succeed({ ok: true as const, blueprint: null, updatedAtMs: 0 } satisfies GetBlueprintResult),
        ),
      );
    const blueprint = (bp as GetBlueprintResult).blueprint as BlueprintHint | null;

    const messagesRaw: ReadonlyArray<ThreadSnapshotMessage> = Array.isArray(snapshot.messages) ? snapshot.messages : [];

    // Bootstrap is a deterministic state machine for MVP: avoid inference until the Blueprint is complete.
    const bootstrapStatus = String(blueprint?.bootstrapState?.status ?? "pending");
    const bootstrapStage = String(blueprint?.bootstrapState?.stage ?? "");
    const lastUserText = lastUserTextFromSnapshot(messagesRaw).trim();

    const promptMessages = messagesRaw
      .filter((m) => m && typeof m === "object")
      .filter((m) => String(m.role ?? "") === "user" || String(m.role ?? "") === "assistant")
      .filter((m) => String(m.status ?? "") !== "streaming" || String(m.messageId ?? "") !== input.assistantMessageId)
      .map((m) => ({ role: String(m.role ?? "user"), text: String(m.text ?? "") }))
      .filter((m) => m.text.trim().length > 0);

    const tail = promptMessages.slice(-MAX_CONTEXT_MESSAGES);

    const workersAiModel = makeWorkersAiLanguageModel({
      binding: input.env.AI,
      model: MODEL_ID_CF,
      maxOutputTokens: MAX_OUTPUT_TOKENS,
    });
    const openRouterApiKey = typeof input.env.OPENROUTER_API_KEY === "string" && input.env.OPENROUTER_API_KEY.length > 0
      ? input.env.OPENROUTER_API_KEY
      : null;
    const modelLayer = Layer.effect(
      AiLanguageModel.LanguageModel,
      openRouterApiKey
        ? Effect.gen(function* () {
            const fallback = yield* workersAiModel;
            const primary = yield* makeOpenRouterLanguageModel({
              apiKey: openRouterApiKey,
              model: PRIMARY_MODEL_OPENROUTER,
              maxOutputTokens: MAX_OUTPUT_TOKENS,
            });
            return yield* makeFallbackLanguageModel(primary, fallback);
          })
        : workersAiModel
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
      const part: AiResponse.StreamPartEncoded = {
        type: "text-delta",
        id: crypto.randomUUID(),
        delta: bufferedDelta,
        metadata: {},
      };
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
          runId: input.runId,
        }).pipe(
          Effect.catchAll(() =>
            Effect.succeed({ ok: true as const, cancelRequested: false } satisfies IsCancelRequestedResult),
          ),
        );
        const cancelResult = cancel as IsCancelRequestedResult;
        if (cancelResult.cancelRequested) {
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
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: batch,
      }).pipe(
        Effect.catchAll((err) =>
          t.log("error", "convex.append_failed", { message: err instanceof Error ? err.message : String(err) }),
        ),
      );
    });

    let extraSystemContext: string | null = null;

    // Phase D: optional RLM-lite pre-summary for long-context runs.
    yield* Effect.gen(function* () {
      if (input.controller.signal.aborted) return;
      if (bootstrapStatus !== "complete") return;
      if (!openRouterApiKey) return;

      const userText = lastUserText.trim();
      if (!userText) return;

      const olderMessages = promptMessages.slice(0, Math.max(0, promptMessages.length - tail.length));
      const olderText = olderMessages.map((m) => `${m.role}: ${m.text}`).join("\n\n");
      const olderChars = olderText.length;

      const explicit =
        userText.startsWith("/rlm") ||
        /\b(recaps?|summari[sz]e|remind me|what did we (decide|agree)|earlier you said)\b/i.test(userText);
      const highPressure = olderChars >= 20_000 || olderMessages.length >= 40;
      if (!explicit && !highPressure) return;

      const strategyReason = explicit
        ? "explicit_request"
        : `context_pressure olderChars=${olderChars} olderMessages=${olderMessages.length}`;

      const signatureId = dseCatalogSignatures.rlm_summarize_thread.id;
      const partId = `dsepart_sig_${input.runId}_rlm_summarize_thread`;

      bufferedParts.push({
        seq: seq++,
        part: {
          type: "dse.signature",
          v: 1,
          id: partId,
          state: "start",
          tsMs: Date.now(),
          signatureId,
          strategyReason,
        },
      });
      yield* flush(true);

      type DseReceiptShape = {
        compiled_id?: string;
        strategyId?: string;
        timing?: { durationMs?: number };
        budget?: unknown;
        receiptId?: string;
        contextPressure?: unknown;
        promptRenderStats?: unknown;
        rlmTrace?: unknown;
      };
      let recordedReceipt: DseReceiptShape | null = null;
      const setRecordedReceipt = (r: unknown) => {
        recordedReceipt = r as DseReceiptShape;
      };

      const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
        env: input.env,
        defaultModelIdCf: MODEL_ID_CF,
        primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
      });
      const dseEnv = layerDsePredictEnvForAutopilotRun({
        threadId: input.threadId,
        runId: input.runId,
        onReceipt: setRecordedReceipt,
      });

      const exit = yield* Effect.exit(
        Effect.gen(function* () {
          const blobStore = yield* BlobStore.BlobStoreService;

          // Bound history size so Convex blobs remain reasonable.
          const MAX_HISTORY_CHARS = 200_000;
          const boundedHistory =
            olderText.length > MAX_HISTORY_CHARS ? olderText.slice(olderText.length - MAX_HISTORY_CHARS) : olderText;

          // Pre-chunk history into stable BlobRefs so the controller can use ExtractOverChunks.
          const chunkChars = 20_000;
          const maxChunks = 12;
          const chunkTexts: Array<string> = [];
          for (let i = 0; i < boundedHistory.length && chunkTexts.length < maxChunks; i += chunkChars) {
            chunkTexts.push(boundedHistory.slice(i, Math.min(boundedHistory.length, i + chunkChars)));
          }

          const threadChunks = yield* Effect.forEach(
            chunkTexts,
            (text) => blobStore.putText({ text, mime: "text/plain" }),
            { concurrency: 3, discard: false },
          );

          return yield* predictRlmSummarizeThread({ question: userText, threadChunks });
        }).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(dseEnv)),
      );

      const state = exit._tag === "Success" ? "ok" : "error";

      const errorText =
        exit._tag === "Failure"
          ? (() => {
              const cause = exit.cause as any;
              if (cause && typeof cause === "object" && "message" in cause) return String(cause.message);
              return String(cause ?? "DSE predict failed");
            })()
          : null;

      const receipt = recordedReceipt as DseReceiptShape | null;
      const promptRenderStats = receipt?.promptRenderStats as any;
      const trimmedPromptRenderStats =
        promptRenderStats &&
        typeof promptRenderStats === "object" &&
        promptRenderStats.context &&
        typeof promptRenderStats.context === "object" &&
        Array.isArray(promptRenderStats.context.blobs) &&
        promptRenderStats.context.blobs.length > 20
          ? {
              ...promptRenderStats,
              context: {
                ...promptRenderStats.context,
                blobsDropped:
                  Number(promptRenderStats.context.blobsDropped ?? 0) + (promptRenderStats.context.blobs.length - 20),
                blobs: promptRenderStats.context.blobs.slice(0, 20),
              },
            }
          : receipt?.promptRenderStats;

      if (exit._tag === "Success") {
        const summary = String((exit.value as any)?.summary ?? "").trim();
        if (summary) {
          const MAX_SUMMARY_CHARS = 1500;
          const bounded = summary.length > MAX_SUMMARY_CHARS ? summary.slice(0, MAX_SUMMARY_CHARS).trim() : summary;
          extraSystemContext = "Prior conversation summary (RLM-lite):\n" + bounded;
        }
      }

      bufferedParts.push({
        seq: seq++,
        part: {
          type: "dse.signature",
          v: 1,
          id: partId,
          state,
          tsMs: Date.now(),
          signatureId,
          compiled_id: receipt?.compiled_id,
          receiptId: receipt?.receiptId,
          timing: receipt?.timing,
          budget: receipt?.budget,
          strategyId: receipt?.strategyId,
          strategyReason,
          contextPressure: receipt?.contextPressure,
          promptRenderStats: trimmedPromptRenderStats,
          rlmTrace: receipt?.rlmTrace,
          ...(exit._tag === "Success" ? { outputPreview: exit.value } : {}),
          ...(errorText ? { errorText } : {}),
        },
      });
      yield* flush(true);
    });

    const rawPrompt = concatTextFromPromptMessages(tail, blueprint, {
      ...(extraSystemContext ? { extraSystem: extraSystemContext } : {}),
    });
    const prompt = AiPrompt.make(rawPrompt);

    const stream = AiLanguageModel.streamText({
      prompt,
      toolChoice: "none",
      disableToolCallResolution: true,
    });

    const runStream = Stream.runForEach(stream, (part) =>
      Effect.sync(() => {
        if (input.controller.signal.aborted) return;
        const encoded = encodeStreamPart(part) as AiResponse.StreamPartEncoded;
        if (shouldIgnoreWirePart(encoded)) return;

        if (encoded.type === "text-delta") {
          const delta = String(encoded.delta ?? "");
          bufferedDelta += delta;
          outputText += delta;
          return;
        }

        // For non-delta parts, materialize any pending delta first to preserve ordering.
        materializeDelta();

        // Encoded parts are small; we buffer them and flush on the same cadence as text.
        bufferedParts.push({ seq: seq++, part: encoded });
      }).pipe(Effect.zipRight(flush(false))),
    ).pipe(Effect.provide(modelLayer));

    let status: "final" | "error" | "canceled" = "final";

    try {
      if (bootstrapStatus !== "complete" && bootstrapStage) {
        const clamp = (s: string, max: number): string => {
          const t = s.trim();
          if (t.length <= max) return t;
          return t.slice(0, max).trim();
        };

        const looksLikeGreeting = (s: string): boolean => {
          const t = s.trim().toLowerCase();
          return (
            t === "hi" ||
            t === "hello" ||
            t === "hey" ||
            t === "yo" ||
            t === "sup" ||
            t === "howdy" ||
            t === "hiya" ||
            t === "hi!" ||
            t === "hello!" ||
            t === "hey!"
          );
        };

        const reply = yield* Effect.gen(function* () {
          // Always render an assistant response during bootstrap so users never hit a silent stall.
          if (bootstrapStage === "ask_user_handle") {
            const handle = clamp(lastUserText, 64);
            if (!handle || looksLikeGreeting(handle)) return "What shall I call you?";
            yield* convex
              .mutation(api.autopilot.blueprint.applyBootstrapUserHandle, {
                threadId: input.threadId,
                handle,
              })
              .pipe(Effect.catchAll(() => Effect.void));
            return `Confirmed, ${handle}. What should you call me?`;
          }

          if (bootstrapStage === "ask_agent_name") {
            const name = clamp(lastUserText, 64);
            if (!name || looksLikeGreeting(name)) return "What should you call me?";
            yield* convex
              .mutation(api.autopilot.blueprint.applyBootstrapAgentName, {
                threadId: input.threadId,
                name,
              })
              .pipe(Effect.catchAll(() => Effect.void));
            return `Got it. Pick one short operating vibe for me.`;
          }

          if (bootstrapStage === "ask_vibe") {
            const vibe = clamp(lastUserText, 140);
            if (!vibe) return "Pick one short operating vibe for me.";
            yield* convex
              .mutation(api.autopilot.blueprint.applyBootstrapAgentVibe, {
                threadId: input.threadId,
                vibe,
              })
              .pipe(Effect.catchAll(() => Effect.void));
            return `Noted. Any boundaries or preferences? Reply 'none' or list a few bullets.`;
          }

          if (bootstrapStage === "ask_boundaries") {
            const lowered = lastUserText.toLowerCase();
            const isNone =
              lowered === "none" ||
              lowered === "no" ||
              lowered === "nope" ||
              lowered === "nah" ||
              lowered === "nothing" ||
              lowered === "n/a" ||
              lowered === "na";

            const boundaries = isNone
              ? []
              : lastUserText
                  .split(/\n|,|;+/g)
                  .map((b) => b.trim())
                  .map((b) => (b.startsWith("- ") ? b.slice(2).trim() : b))
                  .filter((b) => b.length > 0)
                  .slice(0, 16);

            if (!isNone && boundaries.length === 0) {
              return "Any boundaries or preferences? Reply 'none' or list a few bullets.";
            }

            yield* convex
              .mutation(api.autopilot.blueprint.applyBootstrapComplete, {
                threadId: input.threadId,
                ...(boundaries.length > 0 ? { boundaries } : {}),
              } as any)
              .pipe(Effect.catchAll(() => Effect.void));
            return "Setup complete. What would you like to do first?";
          }

          return "What would you like to do first?";
        }).pipe(
          Effect.catchAllCause((cause) =>
            t.log("warn", "bootstrap.apply_failed", { message: String(cause) }).pipe(
              Effect.as("Bootstrap failed. Please retry."),
            ),
          ),
        );

        outputText = reply;
      } else {
        yield* runStream;
        yield* flush(true);

        if (input.controller.signal.aborted) {
          status = "canceled";
        }
      }
    } catch (err) {
      status = input.controller.signal.aborted ? "canceled" : "error";
      yield* t.log("error", "run.failed", { message: err instanceof Error ? err.message : String(err) });
      // Best-effort: append a terminal error part.
      const errorPart: AiResponse.StreamPartEncoded = { type: "error", error: String(err), metadata: {} };
      yield* flushPartsToConvex({
        convex,
        threadId: input.threadId,
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: [{ seq: seq++, part: errorPart }],
      }).pipe(Effect.catchAll(() => Effect.void));
    } finally {
      // Guardrail: never finalize a run with an invisible assistant message.
      // If the model produced no text/tool parts (e.g. only a finish part), fall back to a visible error.
      if (status === "final" && outputText.trim().length === 0) {
        outputText = "No response. Please try again."
      }

      yield* finalizeRunInConvex({
        convex,
        threadId: input.threadId,
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
        // Ensure auth is loaded (so ConvexService server client can setAuth token). Owner-only; no anon.
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        return yield* convex.mutation(api.autopilot.messages.createRun, {
          threadId,
          text,
        });
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(request)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (exit._tag === "Failure") {
      console.error(`[autopilot.send] ${formatRequestIdLogToken(requestId)} create_run_failed`, exit.cause);
      return json({ ok: false, error: "create_run_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const value = exit.value as CreateRunResult;
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
          runId,
        });
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
