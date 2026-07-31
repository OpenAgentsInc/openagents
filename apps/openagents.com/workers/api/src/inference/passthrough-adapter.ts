// Partner passthrough provider adapter for the inference gateway
// (EPIC #5474, #5481).
//
// This adapter is the breadth + burst lane behind the gateway: it forwards a
// normalized InferenceRequest directly to a partner LLM API and returns a
// receipt-first usage object (the inference gateway business doc §3c — "Direct
// Anthropic, OpenAI, and others as breadth + burst capacity beyond our Vertex
// quota"). It is the overflow target routing (#5482) falls through to when our
// own Vertex (#5480) / Fireworks (#5479) quota is exhausted or doesn't cover a
// model.
//
// Two partner wire formats are supported, selected per registered adapter id:
//   - OpenAI Chat Completions (`POST {base}/v1/chat/completions`)
//   - Anthropic Messages       (`POST {base}/v1/messages`)
// Both normalize OUT of our shared InferenceRequest and normalize the partner
// response back IN to our shared InferenceResult / InferenceStreamChunk, so the
// route and metering hook never see partner-specific shapes.
//
// Keys come from Worker secrets (ANTHROPIC_API_KEY / OPENAI_API_KEY), injected
// as Redacted values at registration time; this module never reads process env,
// never commits a key, and never logs key material. Transport problems and
// 429/5xx partner responses surface as a typed retryable InferenceAdapterError
// so routing can fail over to another adapter rather than 500-ing the request.

import { Effect, Redacted } from "effect";

import { parseJsonRecord, recordFromUnknown } from "../json-boundary";
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
  type InferenceStreamChunk,
  type InferenceStreamEvent,
  type InferenceStreamSource,
  type InferenceToolCallDelta,
  type InferenceUsage,
} from "./provider-adapter";
import {
  inferenceToolCallDeltasFromUnknown,
  inferenceToolCallsFromUnknown,
  openAiWireMessageFromInferenceMessage,
} from "./openai-chat-compat";

// Partner HTTP response. Aliased so the adapter's transport types stay distinct
// from the Worker's own Response-returning route surfaces (those are budgeted by
// the zero-debt architecture check; this is a partner client, not a route).
type PartnerResponse = Response;

// Injected fetch so tests can pass a mock without a real network. Matches the
// Worker global `fetch` signature closely enough for our POST-only use.
export type PassthroughFetch = (input: string, init: RequestInit) => Promise<PartnerResponse>;

// Partner wire format the adapter speaks.
export type PassthroughWireFormat = "anthropic" | "openai";

export type PassthroughAdapterConfig = Readonly<{
  // Stable adapter id, e.g. "passthrough-anthropic" / "passthrough-openai".
  id: string;
  wireFormat: PassthroughWireFormat;
  // Partner API key from a Worker secret, kept Redacted so it can't be logged.
  apiKey: Redacted.Redacted<string>;
  // Partner API origin (no trailing slash), e.g. "https://api.anthropic.com"
  // or "https://api.openai.com". The wire-format-specific path is appended.
  baseUrl: string;
  // Injected fetcher; defaults to the Worker global `fetch`.
  fetch?: PassthroughFetch | undefined;
  // Request timeout in ms. Defaults to 60s.
  timeoutMs?: number | undefined;
  // Anthropic Messages requires an explicit max_tokens; OpenAI treats it as
  // optional. Used as the default when the caller does not pass `max_tokens`
  // in passthroughParams. Defaults to 1024.
  defaultMaxTokens?: number | undefined;
  // Anthropic API version header value. Defaults to a known-good date.
  anthropicVersion?: string | undefined;
}>;

const DEFAULT_TIMEOUT_MS = 60_000;
const DEFAULT_MAX_TOKENS = 1_024;
const DEFAULT_ANTHROPIC_VERSION = "2023-06-01";

const fail = (id: string, reason: string): Effect.Effect<never, InferenceAdapterError> =>
  Effect.fail(new InferenceAdapterError({ adapterId: id, reason }));

// Build a retryable reason for 429/5xx so callers (routing/overflow) can tell a
// transient partner problem from a permanent one. The reason string is the
// stable surface the route maps to its JSON error; keep it bounded and free of
// key material or prompt content.
const transportFailureReason = (status: number): string =>
  status === 429
    ? "retryable: partner rate limited (429)"
    : `retryable: partner server error (${status})`;

const isRetryableStatus = (status: number): boolean => status === 429 || status >= 500;

// Pull a numeric passthrough param (max_tokens, etc.) when present and sane.
const numberParam = (
  params: Readonly<Record<string, unknown>>,
  key: string,
): number | undefined => {
  const value = params[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
};

// Sampling params we forward verbatim when present. We copy only a known,
// bounded allow-list rather than spreading arbitrary keys, so an unexpected
// field can't change auth/routing/streaming behavior.
const OPENAI_FORWARDED_PARAMS = [
  "temperature",
  "top_p",
  "frequency_penalty",
  "presence_penalty",
  "stop",
  "seed",
  "tools",
  "tool_choice",
  "parallel_tool_calls",
] as const;

// OpenAI's reasoning-model family served over Chat Completions REJECTS several
// of the legacy sampling/limit fields above with a hard, non-retryable 400
// instead of ignoring them. Verified upstream against `gpt-5.6-luna`
// (2026-07-31), each of these returns HTTP 400:
//   max_tokens          -> "Unsupported parameter: 'max_tokens' is not
//                           supported with this model. Use
//                           'max_completion_tokens' instead."
//   temperature (!= 1)  -> "Unsupported value: 'temperature' does not support
//                           0.7 with this model. Only the default (1) value is
//                           supported."
//   top_p (!= 1)        -> "Unsupported parameter: 'top_p' is not supported
//                           with this model."
//   frequency_penalty   -> unsupported parameter
//   presence_penalty    -> unsupported parameter
//   stop                -> unsupported parameter
// This is what dead-ended every hosted Omega Luna turn: the body ALWAYS carried
// `max_tokens`, so the very first upstream call 400-ed, and a 400 is (rightly)
// not retryable, so routing had no other lane to fall to.
//
// Bounded EXACT-ID set, deliberately not a `gpt-5*` prefix classifier — the same
// discipline the model router uses for this lane. A model only joins this
// profile after its rejection behavior is verified upstream.
const OPENAI_RESTRICTED_REASONING_MODELS: ReadonlySet<string> = new Set(["gpt-5.6-luna"]);

const isRestrictedReasoningModel = (model: string): boolean =>
  OPENAI_RESTRICTED_REASONING_MODELS.has(model.trim().toLowerCase());

// The subset of the OpenAI allow-list the restricted reasoning profile still
// accepts verbatim. Sampling knobs are dropped rather than clamped: sending
// `temperature: 1` is accepted upstream, but silently rewriting a caller's 0.2
// to 1 would misreport what was served. Dropping is the honest normalization.
const OPENAI_REASONING_FORWARDED_PARAMS = [
  "seed",
  "tools",
  "tool_choice",
  "parallel_tool_calls",
  "reasoning_effort",
] as const;

// Function tools on this family require `reasoning_effort: 'none'` over Chat
// Completions; upstream otherwise returns 400 "Function tools with
// reasoning_effort are not supported for <model> in /v1/chat/completions. To use
// function tools, use /v1/responses or set reasoning_effort to 'none'."
// A caller-supplied reasoning_effort still wins — we only supply the default
// that keeps a tool-carrying request from dead-ending.
const REASONING_EFFORT_FOR_TOOLS = "none";

const ANTHROPIC_FORWARDED_PARAMS = ["temperature", "top_p", "top_k", "stop_sequences"] as const;

const forwardParams = (
  params: Readonly<Record<string, unknown>>,
  allow: ReadonlyArray<string>,
): Record<string, unknown> => {
  const out: Record<string, unknown> = {};
  for (const key of allow) {
    if (params[key] !== undefined) {
      out[key] = params[key];
    }
  }
  return out;
};

// ---- OpenAI Chat Completions mapping ------------------------------------

type OpenAiUsage = Readonly<{
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
  prompt_tokens_details?: Readonly<{ cached_tokens?: number }>;
}>;

type OpenAiResponse = Readonly<{
  model?: string;
  choices?: ReadonlyArray<
    Readonly<{
      finish_reason?: string | null;
      message?: Readonly<{
        content?: string | null;
        tool_calls?: unknown;
      }>;
    }>
  >;
  usage?: OpenAiUsage;
}>;

const openAiBody = (
  request: InferenceRequest,
  defaultMaxTokens: number,
): Record<string, unknown> => {
  const base = {
    model: request.model,
    messages: request.messages.map(openAiWireMessageFromInferenceMessage),
    stream: request.stream,
    // RECEIPT-FIRST STREAMING. OpenAI-compatible partners OMIT the `usage`
    // object from a streamed response unless the caller opts in with
    // `stream_options.include_usage`. Without it a true pass-through stream has
    // no terminal usage frame at all, so metering would have to reconstruct
    // counts from deltas — an estimate, which the canonical token ledger
    // forbids. Asking for it makes a streamed turn meter exactly like a
    // non-streamed one. Only sent on streaming requests; a stray caller copy in
    // passthroughParams cannot override it (this load-bearing field is not on
    // either forward allow-list).
    ...(request.stream ? { stream_options: { include_usage: true } } : {}),
  };
  const outputTokenBudget =
    numberParam(request.passthroughParams, "max_tokens") ?? defaultMaxTokens;

  if (!isRestrictedReasoningModel(request.model)) {
    return {
      ...base,
      max_tokens: outputTokenBudget,
      ...forwardParams(request.passthroughParams, OPENAI_FORWARDED_PARAMS),
    };
  }

  const forwarded = forwardParams(request.passthroughParams, OPENAI_REASONING_FORWARDED_PARAMS);
  const needsToolReasoningEffort =
    forwarded["tools"] !== undefined && forwarded["reasoning_effort"] === undefined;
  return {
    ...base,
    // A caller's `max_completion_tokens` wins; otherwise carry their
    // `max_tokens` budget over to the field this family actually accepts, so
    // the caller's intended output cap is preserved rather than dropped.
    max_completion_tokens:
      numberParam(request.passthroughParams, "max_completion_tokens") ?? outputTokenBudget,
    ...forwarded,
    ...(needsToolReasoningEffort ? { reasoning_effort: REASONING_EFFORT_FOR_TOOLS } : {}),
  };
};

const openAiUsage = (usage: OpenAiUsage | undefined): InferenceUsage => {
  const promptTokens = usage?.prompt_tokens ?? 0;
  const completionTokens = usage?.completion_tokens ?? 0;
  const cached = usage?.prompt_tokens_details?.cached_tokens;
  return {
    promptTokens,
    completionTokens,
    totalTokens: usage?.total_tokens ?? promptTokens + completionTokens,
    ...(typeof cached === "number" ? { cachedPromptTokens: cached } : {}),
  };
};

const openAiResult = (request: InferenceRequest, payload: OpenAiResponse): InferenceResult => {
  const choice = payload.choices?.[0];
  const toolCalls = inferenceToolCallsFromUnknown(choice?.message?.tool_calls);
  return {
    content: choice?.message?.content ?? "",
    finishReason: choice?.finish_reason ?? "stop",
    servedModel: payload.model ?? request.model,
    ...(toolCalls === undefined || toolCalls.length === 0 ? {} : { toolCalls }),
    usage: openAiUsage(payload.usage),
  };
};

// ---- Anthropic Messages mapping -----------------------------------------

type AnthropicUsage = Readonly<{
  input_tokens?: number;
  output_tokens?: number;
  cache_read_input_tokens?: number;
}>;

type AnthropicResponse = Readonly<{
  model?: string;
  stop_reason?: string | null;
  content?: ReadonlyArray<Readonly<{ type?: string; text?: string }>>;
  usage?: AnthropicUsage;
}>;

// Anthropic Messages keeps the `system` prompt out of `messages`; split any
// system turns out so the request maps cleanly.
const anthropicBody = (
  request: InferenceRequest,
  defaultMaxTokens: number,
): Record<string, unknown> => {
  const systemParts: Array<string> = [];
  const turns: Array<{ role: string; content: string }> = [];
  for (const message of request.messages) {
    if (message.role === "system") {
      systemParts.push(message.content);
    } else {
      // Anthropic accepts only "user" / "assistant" roles.
      turns.push({
        content: message.content,
        role: message.role === "assistant" ? "assistant" : "user",
      });
    }
  }
  return {
    model: request.model,
    max_tokens: numberParam(request.passthroughParams, "max_tokens") ?? defaultMaxTokens,
    messages: turns,
    stream: request.stream,
    ...(systemParts.length > 0 ? { system: systemParts.join("\n\n") } : {}),
    ...forwardParams(request.passthroughParams, ANTHROPIC_FORWARDED_PARAMS),
  };
};

// Map Anthropic's stop_reason to the OpenAI-style finish_reason our envelope
// uses, so downstream consumers see one vocabulary.
const anthropicFinishReason = (stopReason: string | null | undefined): string => {
  switch (stopReason) {
    case "max_tokens":
      return "length";
    case "tool_use":
      return "tool_calls";
    case "end_turn":
    case "stop_sequence":
      return "stop";
    default:
      return stopReason ?? "stop";
  }
};

const anthropicUsage = (usage: AnthropicUsage | undefined): InferenceUsage => {
  const promptTokens = usage?.input_tokens ?? 0;
  const completionTokens = usage?.output_tokens ?? 0;
  const cached = usage?.cache_read_input_tokens;
  return {
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
    ...(typeof cached === "number" ? { cachedPromptTokens: cached } : {}),
  };
};

const anthropicText = (content: AnthropicResponse["content"] | undefined): string =>
  (content ?? [])
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text ?? "")
    .join("");

const anthropicResult = (
  request: InferenceRequest,
  payload: AnthropicResponse,
): InferenceResult => ({
  content: anthropicText(payload.content),
  finishReason: anthropicFinishReason(payload.stop_reason),
  servedModel: payload.model ?? request.model,
  usage: anthropicUsage(payload.usage),
});

// ---- HTTP plumbing -------------------------------------------------------

const requestPath = (wireFormat: PassthroughWireFormat): string =>
  wireFormat === "anthropic" ? "/v1/messages" : "/v1/chat/completions";

// Read the Redacted secret to a string at the network boundary only. The value
// is placed on an outbound header and never logged or returned.
const requestHeaders = (
  config: PassthroughAdapterConfig,
  stream: boolean,
): Record<string, string> => {
  const key = Redacted.value(config.apiKey);
  // A streamed partner call negotiates SSE, not JSON. Asking for
  // `application/json` on a `stream: true` request is how a partner is invited
  // to answer with one buffered body instead of a frame-by-frame stream.
  const accept = stream ? "text/event-stream" : "application/json";
  if (config.wireFormat === "anthropic") {
    return {
      accept,
      "anthropic-version": config.anthropicVersion ?? DEFAULT_ANTHROPIC_VERSION,
      "content-type": "application/json",
      "x-api-key": key,
    };
  }
  return {
    accept,
    authorization: `Bearer ${key}`,
    "content-type": "application/json",
  };
};

const safeSignal = (timeoutMs: number): AbortSignal | undefined => {
  try {
    return AbortSignal.timeout(timeoutMs);
  } catch {
    return undefined;
  }
};

const postToPartner = (
  config: PassthroughAdapterConfig,
  body: unknown,
  stream: boolean,
): Effect.Effect<PartnerResponse, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: (error) =>
      new InferenceAdapterError({
        adapterId: config.id,
        reason: `retryable: partner transport error (${
          error instanceof Error ? error.name : "unknown"
        })`,
      }),
    try: () => {
      const fetcher = config.fetch ?? (globalThis.fetch as PassthroughFetch);
      const signal = safeSignal(config.timeoutMs ?? DEFAULT_TIMEOUT_MS);
      return fetcher(`${config.baseUrl}${requestPath(config.wireFormat)}`, {
        body: JSON.stringify(body),
        headers: requestHeaders(config, stream),
        method: "POST",
        ...(signal === undefined ? {} : { signal }),
      });
    },
  });

const parseJson = (
  config: PassthroughAdapterConfig,
  response: PartnerResponse,
): Effect.Effect<unknown, InferenceAdapterError> =>
  Effect.tryPromise({
    catch: () =>
      new InferenceAdapterError({
        adapterId: config.id,
        reason: "partner returned a non-JSON response",
      }),
    try: () => response.json(),
  });

// Bounded upstream-rejection detail for a NON-retryable partner response.
//
// Without this, every partner 4xx collapsed to the same opaque
// "partner rejected request (400)" and the actual cause — an unsupported
// request field — was invisible in logs and to the caller. Diagnosing one such
// 400 cost three sessions. We surface only the partner's own bounded
// `error.message` / `error.code` (parameter-shape diagnostics), never headers,
// never key material, and never the request body or prompt.
const MAX_PARTNER_ERROR_DETAIL = 200;

const partnerErrorDetail = (payload: unknown): string | undefined => {
  if (typeof payload !== "object" || payload === null) {
    return undefined;
  }
  const error = (payload as { error?: unknown }).error;
  if (typeof error !== "object" || error === null) {
    return undefined;
  }
  const { code, message } = error as { code?: unknown; message?: unknown };
  const text = typeof message === "string" && message !== "" ? message : undefined;
  const codeText = typeof code === "string" && code !== "" ? code : undefined;
  const detail =
    text === undefined ? codeText : codeText === undefined ? text : `${codeText}: ${text}`;
  return detail === undefined ? undefined : detail.slice(0, MAX_PARTNER_ERROR_DETAIL);
};

// ---- OpenAI Chat Completions SSE (true incremental passthrough) ----------
//
// The buffered `stream` below asks the partner for ONE non-streamed body and
// then splits it into two frames, so nothing reaches the client until the whole
// answer exists upstream. That is why the hosted Luna lane "did not stream": the
// answer appeared all at once because it WAS produced all at once, from the
// caller's point of view. The helpers here parse the partner's real SSE off the
// response byte stream so each delta is forwarded the moment it arrives.

// Parse one SSE line (`data: {...}` / `data: [DONE]`). Blank lines, comments and
// the terminal sentinel yield undefined. JSON decoding goes through the
// json-boundary helper (no raw JSON.parse at this boundary).
const parseSseData = (line: string): Record<string, unknown> | undefined => {
  const trimmed = line.trim();
  if (!trimmed.startsWith("data:")) {
    return undefined;
  }
  const payload = trimmed.slice("data:".length).trim();
  if (payload === "" || payload === "[DONE]") {
    return undefined;
  }
  return parseJsonRecord(payload);
};

const firstStreamChoice = (
  frame: Record<string, unknown>,
): Record<string, unknown> | undefined => {
  const choices = frame["choices"];
  if (!Array.isArray(choices) || choices.length === 0) {
    return undefined;
  }
  return recordFromUnknown(choices[0]);
};

const streamDeltaOf = (
  frame: Record<string, unknown>,
): Record<string, unknown> | undefined =>
  recordFromUnknown(firstStreamChoice(frame)?.["delta"]);

const streamContentOf = (frame: Record<string, unknown>): string => {
  const content = streamDeltaOf(frame)?.["content"];
  return typeof content === "string" ? content : "";
};

// Provider-labeled reasoning/thinking, kept on its own channel so a client never
// has to infer it from prose.
const streamReasoningOf = (frame: Record<string, unknown>): string | undefined => {
  const delta = streamDeltaOf(frame);
  const direct =
    delta?.["reasoning_content"] ?? delta?.["reasoning"] ?? delta?.["reasoning_delta"];
  return typeof direct === "string" && direct !== "" ? direct : undefined;
};

// Tool-call ARGUMENT FRAGMENTS arrive split across many frames; a partner may
// send `{"pa`, then `th":"src/m`, then `ain.rs"}`. They are forwarded VERBATIM,
// per frame — never concatenated, parsed, or validated here. Accumulating by
// index is the client's job (`InferenceStreamEvent.toolCallDeltas`), and holding
// fragments back to assemble them would re-introduce exactly the buffering this
// change removes. The lenient per-frame decoder is used deliberately: a mid-call
// fragment carries only `index` plus a partial `function.arguments`, which the
// strict whole-call decoder (used by the buffered path) would reject outright.
const streamToolCallDeltasOf = (
  frame: Record<string, unknown>,
): ReadonlyArray<InferenceToolCallDelta> | undefined => {
  const deltas = inferenceToolCallDeltasFromUnknown(streamDeltaOf(frame)?.["tool_calls"]);
  return deltas === undefined || deltas.length === 0 ? undefined : deltas;
};

const streamFinishReasonOf = (frame: Record<string, unknown>): string | undefined => {
  const reason = firstStreamChoice(frame)?.["finish_reason"];
  return typeof reason === "string" && reason !== "" ? reason : undefined;
};

const finiteNumber = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;

// Receipt-first: a usage frame is surfaced only when the partner actually sent
// real prompt/completion counts. A missing or malformed `usage` object yields
// undefined so the route discloses an unmetered stream rather than settling on
// a reconstructed estimate.
const streamUsageOf = (frame: Record<string, unknown>): InferenceUsage | undefined => {
  const usage = recordFromUnknown(frame["usage"]);
  if (usage === undefined) {
    return undefined;
  }
  const promptTokens = finiteNumber(usage["prompt_tokens"]);
  const completionTokens = finiteNumber(usage["completion_tokens"]);
  if (promptTokens === undefined || completionTokens === undefined) {
    return undefined;
  }
  const details = recordFromUnknown(usage["prompt_tokens_details"]);
  const cached = details === undefined ? undefined : finiteNumber(details["cached_tokens"]);
  return {
    completionTokens,
    promptTokens,
    totalTokens: finiteNumber(usage["total_tokens"]) ?? promptTokens + completionTokens,
    ...(cached === undefined ? {} : { cachedPromptTokens: cached }),
  };
};

const streamEventForFrame = (frame: Record<string, unknown>): InferenceStreamEvent => {
  const reasoningDelta = streamReasoningOf(frame);
  const toolCallDeltas = streamToolCallDeltasOf(frame);
  const finishReason = streamFinishReasonOf(frame);
  const usage = streamUsageOf(frame);
  const model = frame["model"];
  return {
    contentDelta: streamContentOf(frame),
    ...(reasoningDelta === undefined ? {} : { reasoningDelta }),
    ...(toolCallDeltas === undefined ? {} : { toolCallDeltas }),
    ...(finishReason === undefined ? {} : { finishReason }),
    ...(usage === undefined ? {} : { usage }),
    ...(typeof model === "string" && model !== "" ? { servedModel: model } : {}),
  };
};

// Build a true incremental SSE source over the partner's response body. Frames
// are decoded and parsed AS BYTES ARRIVE — a line split across two reads is held
// in `buffer` until it completes — and yielded one at a time, so the route pumps
// each to the client instead of waiting for the whole completion. The running
// terminal state (finishReason / usage / servedModel) is captured during
// iteration and exposed by `terminal()` once the source is drained, so metering
// still settles receipt-first without re-buffering any content.
const makeSseSource = (
  body: ReadableStream<Uint8Array>,
  fallbackModel: string,
): InferenceStreamSource => {
  let finishReason: string | undefined;
  let usage: InferenceUsage | undefined;
  let servedModel: string | undefined = fallbackModel;

  const captureTerminal = (event: InferenceStreamEvent): void => {
    if (event.finishReason !== undefined) {
      finishReason = event.finishReason;
    }
    if (event.usage !== undefined) {
      usage = event.usage;
    }
    if (event.servedModel !== undefined) {
      servedModel = event.servedModel;
    }
  };

  const frames = (async function* (): AsyncIterable<InferenceStreamEvent> {
    const reader = body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (value !== undefined) {
          buffer += decoder.decode(value, { stream: true });
        }
        // Emit every COMPLETE line currently buffered. OpenAI-compatible chunk
        // framing keeps one `data:` payload on one line, so line-at-a-time
        // parsing never holds a frame back waiting for the blank separator.
        let newlineIndex = buffer.indexOf("\n");
        while (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex);
          buffer = buffer.slice(newlineIndex + 1);
          const frame = parseSseData(line);
          if (frame !== undefined) {
            const event = streamEventForFrame(frame);
            captureTerminal(event);
            yield event;
          }
          newlineIndex = buffer.indexOf("\n");
        }
        if (done) {
          // Flush a trailing partial line (some partners omit the final \n).
          const tail = parseSseData(buffer);
          if (tail !== undefined) {
            const event = streamEventForFrame(tail);
            captureTerminal(event);
            yield event;
          }
          break;
        }
      }
    } finally {
      reader.releaseLock();
    }
  })();

  return {
    frames,
    terminal: () => ({ finishReason, servedModel, usage }),
  };
};

// ---- Adapter factory -----------------------------------------------------

const buildBody = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
): Record<string, unknown> => {
  const defaultMaxTokens = config.defaultMaxTokens ?? DEFAULT_MAX_TOKENS;
  return config.wireFormat === "anthropic"
    ? anthropicBody(request, defaultMaxTokens)
    : openAiBody(request, defaultMaxTokens);
};

// Project a buffered assistant message's tool calls onto the incremental
// tool-call delta shape the streamed contract uses.
//
// A buffered completion holds each tool call WHOLE, so one delta per call is
// the accurate encoding: it carries the id, the type and the complete argument
// JSON at its own index. That is exactly what an OpenAI streaming client
// accumulates by index, so a consumer cannot tell this from a partner stream
// that split the same call across many frames — it only ever sees one frame
// arrive with everything already in it.
const toolCallDeltasFromResult = (
  toolCalls: InferenceResult["toolCalls"],
): ReadonlyArray<InferenceToolCallDelta> | undefined =>
  toolCalls === undefined || toolCalls.length === 0
    ? undefined
    : toolCalls.map((toolCall, index) => ({
        function: {
          arguments: toolCall.function.arguments,
          name: toolCall.function.name,
        },
        id: toolCall.id,
        index,
        type: "function" as const,
      }));

const toResult = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
  payload: unknown,
): InferenceResult =>
  config.wireFormat === "anthropic"
    ? anthropicResult(request, payload as AnthropicResponse)
    : openAiResult(request, payload as OpenAiResponse);

// Shared request → response path for both complete and (collected) stream.
const runCompletion = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<InferenceResult, InferenceAdapterError> =>
  Effect.gen(function* () {
    const response = yield* postToPartner(config, buildBody(config, request), false);

    if (isRetryableStatus(response.status)) {
      return yield* fail(config.id, transportFailureReason(response.status));
    }

    const payload = yield* parseJson(config, response);

    if (!response.ok) {
      const detail = partnerErrorDetail(payload);
      return yield* fail(
        config.id,
        detail === undefined
          ? `partner rejected request (${response.status})`
          : `partner rejected request (${response.status}): ${detail}`,
      );
    }

    return toResult(config, request, payload);
  });

// Streamed request → lazily consumed SSE source. Connect-time failures (429/5xx
// and partner rejections) surface with exactly the same typed reasons the
// buffered path uses, so routing can still overflow BEFORE any byte is emitted.
const runSseStream = (
  config: PassthroughAdapterConfig,
  request: InferenceRequest,
): Effect.Effect<InferenceStreamSource, InferenceAdapterError> =>
  Effect.gen(function* () {
    const streamedRequest: InferenceRequest = { ...request, stream: true };
    const response = yield* postToPartner(
      config,
      buildBody(config, streamedRequest),
      true,
    );

    if (isRetryableStatus(response.status)) {
      return yield* fail(config.id, transportFailureReason(response.status));
    }

    if (!response.ok) {
      const payload = yield* parseJson(config, response).pipe(
        Effect.orElseSucceed(() => undefined),
      );
      const detail = partnerErrorDetail(payload);
      return yield* fail(
        config.id,
        detail === undefined
          ? `partner rejected request (${response.status})`
          : `partner rejected request (${response.status}): ${detail}`,
      );
    }

    const body = response.body;
    if (body === null) {
      return yield* fail(config.id, "partner stream had no response body");
    }

    return makeSseSource(body, request.model);
  });

// Build a passthrough adapter for one partner. Each registered partner gets one
// adapter id. The adapter is pure data + Effects; it touches the network only
// when `complete`/`stream` actually run, so registering it under a disabled
// flag keeps it fully INERT.
export const makePassthroughAdapter = (
  config: PassthroughAdapterConfig,
): InferenceProviderAdapter => ({
  id: config.id,
  complete: (request: InferenceRequest) => runCompletion(config, request),
  // TRUE PASS-THROUGH STREAM for the OpenAI Chat Completions wire format. The
  // buffered `stream` below asks for a non-streamed body, so the client saw the
  // entire answer materialize at once — the owner-visible "luna does not
  // stream" defect. This asks the partner for real SSE and hands the route a
  // lazily consumed source, so each delta reaches the client the moment the
  // partner produces it (and every chunk resets the edge idle-timer, so a long
  // generation cannot time out mid-answer). `stream_options.include_usage`
  // (set in `openAiBody`) keeps the terminal usage frame receipt-first.
  //
  // Only the OpenAI format is wired: Anthropic Messages streams a DIFFERENT
  // event vocabulary (`content_block_delta` / `message_delta`), which this
  // parser does not speak. Omitting `streamSse` there is the honest encoding —
  // the route falls back to the buffered path for that lane rather than
  // silently dropping every Anthropic frame on the floor.
  ...(config.wireFormat === "openai"
    ? { streamSse: (request: InferenceRequest) => runSseStream(config, request) }
    : {}),
  // Buffered fallback. Maps to a single non-streamed partner call whose result
  // is split into a content frame + a terminal usage frame, forcing
  // `stream: false` on the partner request so metering settles from the
  // partner's real, receipt-first usage rather than reconstructed counts. It
  // stays for tests, metering reconstruction, the overflow dispatcher, and the
  // component-channel path, all of which need the WHOLE assembled completion.
  //
  // The content frame carries the assistant's tool calls as well as its text.
  // Dropping them is what made the hosted Omega Luna lane produce NOTHING: a
  // coding client always sends tools, an assistant turn that calls one has
  // `content: null` and its whole answer in `tool_calls`, so a content-only
  // frame reduced the entire turn to two empty deltas. The request succeeded,
  // the usage was real, no error was raised anywhere — and the client rendered
  // a spinner, then nothing. `complete` mapped tool calls from the first day
  // (`openAiResult`); only this streamed projection of the same result lost
  // them, which is why the non-streaming path looked healthy while every
  // streamed turn dead-ended. See `InferenceStreamChunk.toolCallDeltas`, which
  // the route already serializes into `delta.tool_calls`.
  stream: (request: InferenceRequest) =>
    runCompletion(config, { ...request, stream: false }).pipe(
      Effect.map((result): ReadonlyArray<InferenceStreamChunk> => {
        const toolCallDeltas = toolCallDeltasFromResult(result.toolCalls);
        const contentChunk: InferenceStreamChunk = {
          contentDelta: result.content,
          ...(toolCallDeltas === undefined ? {} : { toolCallDeltas }),
        };
        const terminalChunk: InferenceStreamChunk = {
          contentDelta: "",
          finishReason: result.finishReason,
          servedModel: result.servedModel,
          usage: result.usage,
        };
        return [contentChunk, terminalChunk];
      }),
    ),
});
