// An Effect-wrapped OpenRouter provider layer (openagents #6182).
//
// Why: the Khala autonomous-QA driver loop drives via a weak tool-caller
// (gpt-oss-20b truncates JSON). OpenRouter gives a reliable lane: route the
// JSON-action loop to a strong (or capable free) model when armed, while keeping
// gpt-oss for cheap/own-infra paths.
//
// Shape: a `Context.Service` (`OpenRouterClient`) wrapping `@openrouter/sdk`'s
// `new OpenRouter({ apiKey }).chat.send(...)`. The boundary is Effect: a single
// `chat(input): Effect<ChatResult, OpenRouterError>` plus a streaming variant,
// tagged errors, Effect Schema validation of the reply, bounded retry/backoff,
// and a per-call token cap.
//
// Spend discipline (critical — only a few $ of credits exist):
//   * the DEFAULT layer in tests/CI is the MOCK (`OpenRouterClientMock`) — no
//     network, no spend;
//   * the REAL layer is only constructed behind an explicit flag (`OPENROUTER_LIVE`)
//     by the caller; `OpenRouterClientLive` itself fails closed if no key;
//   * every request gets a small default `max_tokens` and a hard per-request cap;
//   * the API key is read from `OPENROUTER_API_KEY` only and is NEVER logged
//     (held as `Redacted`, never serialized into receipts/errors).

import { Config, Context, Effect, Layer, Redacted, Schedule, Schema as S } from "effect";
import { OpenRouter } from "@openrouter/sdk";
import {
  ConnectionError,
  PaymentRequiredResponseError,
  ProviderOverloadedResponseError,
  RequestAbortedError,
  RequestTimeoutError,
  RequestTimeoutResponseError,
  ServiceUnavailableResponseError,
  TooManyRequestsResponseError,
  UnauthorizedResponseError,
} from "@openrouter/sdk/models/errors";

// ----------------------------------------------------------------------------
// Spend-discipline constants. These bound how many tokens a careless run can buy.
// ----------------------------------------------------------------------------

/** Free Models Router: routes a request to a feature-capable free model ($0). */
export const OPENROUTER_FREE_MODEL = "openrouter/free" as const;

/**
 * The default model id. `openrouter/free` (cost=0) is the safe default so a live
 * smoke or careless arming cannot burn the few dollars of credits. Override with
 * `OPENROUTER_MODEL` when a stronger paid tool-caller is wanted.
 */
export const OPENROUTER_DEFAULT_MODEL = OPENROUTER_FREE_MODEL;

/**
 * Default `max_tokens` for a single completion. Kept generous enough that a
 * reasoning model (which spends tokens on a hidden reasoning channel before any
 * visible content — the same failure shape as gpt-oss) can still emit a complete
 * JSON action, but bounded so a single call cannot be unboundedly expensive.
 */
export const OPENROUTER_DEFAULT_MAX_TOKENS = 1024;

/**
 * Hard ceiling on `max_tokens` for ANY single request. A request asking for more
 * than this is clamped down to this value before it reaches the network. This is
 * a fail-safe spend guard, independent of the per-call default.
 */
export const OPENROUTER_MAX_TOKENS_CAP = 4096;

/** Default request timeout (ms). Aborts a stuck inference call. */
export const OPENROUTER_DEFAULT_TIMEOUT_MS = 60_000;

/** Default bounded retry attempts (in addition to the first try). */
export const OPENROUTER_DEFAULT_RETRIES = 2;

// ----------------------------------------------------------------------------
// Tagged errors. The error channel is a discriminated union; callers can match
// on `_tag`. NONE of these carry the API key.
// ----------------------------------------------------------------------------

/** Missing / invalid credentials, or fail-closed when no key is configured. */
export class OpenRouterAuthError extends S.TaggedErrorClass<OpenRouterAuthError>()(
  "OpenRouterAuthError",
  { reason: S.String },
) {}

/** HTTP 429 / rate-limit / provider overloaded. Transient; eligible for retry. */
export class OpenRouterRateLimitError extends S.TaggedErrorClass<OpenRouterRateLimitError>()(
  "OpenRouterRateLimitError",
  { reason: S.String },
) {}

/** A 4xx/5xx upstream failure or an unparseable response. */
export class OpenRouterUpstreamError extends S.TaggedErrorClass<OpenRouterUpstreamError>()(
  "OpenRouterUpstreamError",
  { reason: S.String, status: S.optional(S.Number) },
) {}

/** The request exceeded its timeout / was aborted. */
export class OpenRouterTimeoutError extends S.TaggedErrorClass<OpenRouterTimeoutError>()(
  "OpenRouterTimeoutError",
  { reason: S.String },
) {}

/** The full tagged-error union returned by the client. */
export type OpenRouterError =
  | OpenRouterAuthError
  | OpenRouterRateLimitError
  | OpenRouterUpstreamError
  | OpenRouterTimeoutError;

// ----------------------------------------------------------------------------
// Boundary schemas. The request is normalised before hitting the SDK; the reply
// is validated with Effect Schema so a malformed upstream body is a typed error.
// ----------------------------------------------------------------------------

export const OpenRouterRole = S.Literals(["system", "user", "assistant", "tool"]);
export type OpenRouterRole = typeof OpenRouterRole.Type;

export const OpenRouterMessage = S.Struct({
  role: OpenRouterRole,
  content: S.String,
});
export type OpenRouterMessage = typeof OpenRouterMessage.Type;

export interface OpenRouterChatInput {
  /** Override the configured model for this single request. */
  readonly model?: string;
  readonly messages: ReadonlyArray<OpenRouterMessage>;
  /** Sampling temperature; defaults to 0 (deterministic, best for JSON actions). */
  readonly temperature?: number;
  /** Per-request `max_tokens`; clamped to `OPENROUTER_MAX_TOKENS_CAP`. */
  readonly maxTokens?: number;
  /**
   * When true, ask OpenRouter to enforce a JSON object response. The Free Models
   * Router filters for models that support the requested features, so this nudges
   * `openrouter/free` toward a capable JSON/tool-caller (reliable actions at $0).
   */
  readonly jsonObject?: boolean;
}

/** Token usage for one completion (validated subset of the SDK's usage). */
export const OpenRouterUsage = S.Struct({
  promptTokens: S.optional(S.Number),
  completionTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  reasoningTokens: S.optional(S.Number),
});
export type OpenRouterUsage = typeof OpenRouterUsage.Type;

export interface OpenRouterChatResult {
  /** The model that actually served the request (the router may pick one). */
  readonly model: string;
  /** The assistant text content (empty string if the model emitted none). */
  readonly content: string;
  /** OpenAI-style finish reason, when present. */
  readonly finishReason?: string;
  readonly usage?: OpenRouterUsage;
}

/** A single streamed text delta. */
export interface OpenRouterStreamChunk {
  readonly content: string;
}

// ----------------------------------------------------------------------------
// The Effect service.
// ----------------------------------------------------------------------------

export class OpenRouterClient extends Context.Service<
  OpenRouterClient,
  {
    /** One non-streaming completion. */
    readonly chat: (input: OpenRouterChatInput) => Effect.Effect<OpenRouterChatResult, OpenRouterError>;
    /**
     * The streaming variant. Returns the accumulated result plus the ordered
     * deltas. `onChunk` is invoked as deltas arrive (no spend difference; this is
     * the same call billed once).
     */
    readonly chatStream: (
      input: OpenRouterChatInput & { readonly onChunk?: (chunk: OpenRouterStreamChunk) => void },
    ) => Effect.Effect<OpenRouterChatResult, OpenRouterError>;
  }
>()("openagents/probe-runtime/OpenRouterClient") {}

// ----------------------------------------------------------------------------
// Config. The key is read from `OPENROUTER_API_KEY` as a Redacted value and the
// model/cap/timeout/retry knobs from env, with safe defaults. `OpenRouterConfig`
// is resolved lazily so the mock layer never needs a key.
// ----------------------------------------------------------------------------

export interface OpenRouterResolvedConfig {
  readonly apiKey: Redacted.Redacted<string>;
  readonly model: string;
  readonly defaultMaxTokens: number;
  readonly timeoutMs: number;
  readonly retries: number;
}

/**
 * Effect `Config` for the real client. **Fails closed** when `OPENROUTER_API_KEY`
 * is absent (no implicit/empty key, no silent fallback to a billable account).
 */
export const OpenRouterConfig: Config.Config<OpenRouterResolvedConfig> = Config.all({
  apiKey: Config.redacted("OPENROUTER_API_KEY"),
  model: Config.string("OPENROUTER_MODEL").pipe(Config.withDefault(OPENROUTER_DEFAULT_MODEL)),
  defaultMaxTokens: Config.number("OPENROUTER_MAX_TOKENS").pipe(
    Config.withDefault(OPENROUTER_DEFAULT_MAX_TOKENS),
  ),
  timeoutMs: Config.number("OPENROUTER_TIMEOUT_MS").pipe(Config.withDefault(OPENROUTER_DEFAULT_TIMEOUT_MS)),
  retries: Config.number("OPENROUTER_RETRIES").pipe(Config.withDefault(OPENROUTER_DEFAULT_RETRIES)),
});

/** Clamp a requested `max_tokens` to the hard per-request cap (spend guard). */
export function clampMaxTokens(requested: number | undefined, fallback: number): number {
  const want = requested ?? fallback;
  if (!Number.isFinite(want) || want <= 0) return fallback;
  return Math.min(Math.floor(want), OPENROUTER_MAX_TOKENS_CAP);
}

// ----------------------------------------------------------------------------
// Error mapping. The Speakeasy SDK throws typed error classes; map them to the
// tagged union. Anything unrecognised becomes an upstream error (never leaks a
// key — we only read the class name / a short message).
// ----------------------------------------------------------------------------

/** Map a thrown unknown into a tagged `OpenRouterError`. Exported for tests. */
export function mapOpenRouterError(error: unknown): OpenRouterError {
  if (error instanceof UnauthorizedResponseError) {
    return new OpenRouterAuthError({ reason: "unauthorized (HTTP 401)" });
  }
  if (error instanceof PaymentRequiredResponseError) {
    return new OpenRouterAuthError({ reason: "payment required (HTTP 402): out of credits" });
  }
  if (error instanceof TooManyRequestsResponseError || error instanceof ProviderOverloadedResponseError) {
    return new OpenRouterRateLimitError({ reason: rateLimitReason(error) });
  }
  if (
    error instanceof RequestTimeoutError ||
    error instanceof RequestTimeoutResponseError ||
    error instanceof RequestAbortedError
  ) {
    return new OpenRouterTimeoutError({ reason: "request timed out or was aborted" });
  }
  if (error instanceof ServiceUnavailableResponseError) {
    return new OpenRouterUpstreamError({ reason: "service unavailable (HTTP 503)", status: 503 });
  }
  if (error instanceof ConnectionError) {
    return new OpenRouterUpstreamError({ reason: "connection error reaching OpenRouter" });
  }
  // Fall back: read a status if the error carries one, never the body verbatim.
  const status = readStatus(error);
  if (status === 401 || status === 403) return new OpenRouterAuthError({ reason: `auth failure (HTTP ${status})` });
  if (status === 402) return new OpenRouterAuthError({ reason: "payment required (HTTP 402): out of credits" });
  if (status === 429) return new OpenRouterRateLimitError({ reason: "rate limited (HTTP 429)" });
  if (status === 408 || status === 504) return new OpenRouterTimeoutError({ reason: `timeout (HTTP ${status})` });
  return new OpenRouterUpstreamError({
    reason: shortMessage(error),
    ...(status === undefined ? {} : { status }),
  });
}

function rateLimitReason(error: unknown): string {
  return error instanceof ProviderOverloadedResponseError ? "provider overloaded" : "rate limited (HTTP 429)";
}

function readStatus(error: unknown): number | undefined {
  if (typeof error === "object" && error !== null) {
    const anyErr = error as { statusCode?: unknown; status?: unknown };
    const raw = anyErr.statusCode ?? anyErr.status;
    if (typeof raw === "number" && Number.isFinite(raw)) return raw;
  }
  return undefined;
}

/** A short, key-free message for an unknown error. */
function shortMessage(error: unknown): string {
  if (error instanceof Error) {
    const name = error.name || "Error";
    const msg = (error.message || "").slice(0, 200);
    return `${name}: ${msg}`;
  }
  return "unknown OpenRouter error";
}

/** Which tagged errors are worth retrying (transient). */
function isRetryable(error: OpenRouterError): boolean {
  return error._tag === "OpenRouterRateLimitError" || error._tag === "OpenRouterTimeoutError" ||
    (error._tag === "OpenRouterUpstreamError" && (error.status === undefined || error.status >= 500));
}

// ----------------------------------------------------------------------------
// Response normalisation. Extract content/usage from the SDK's `ChatResult`.
// ----------------------------------------------------------------------------

const SdkUsage = S.Struct({
  promptTokens: S.optional(S.Number),
  completionTokens: S.optional(S.Number),
  totalTokens: S.optional(S.Number),
  completionTokensDetails: S.optional(
    S.Struct({ reasoningTokens: S.optional(S.NullOr(S.Number)) }),
  ),
});

function normalizeUsage(usage: unknown): OpenRouterUsage | undefined {
  const parsed = S.decodeUnknownOption(SdkUsage)(usage);
  if (parsed._tag === "None") return undefined;
  const u = parsed.value;
  const reasoning = u.completionTokensDetails?.reasoningTokens ?? undefined;
  return {
    ...(u.promptTokens === undefined ? {} : { promptTokens: u.promptTokens }),
    ...(u.completionTokens === undefined ? {} : { completionTokens: u.completionTokens }),
    ...(u.totalTokens === undefined ? {} : { totalTokens: u.totalTokens }),
    ...(reasoning === undefined || reasoning === null ? {} : { reasoningTokens: reasoning }),
  };
}

/** Pull plain text out of the SDK's `content` (string | parts | null). */
function extractContent(content: unknown): string {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return content
      .map((part) =>
        typeof part === "object" && part !== null && "text" in part && typeof (part as { text: unknown }).text === "string"
          ? (part as { text: string }).text
          : "",
      )
      .join("");
  }
  return "";
}

function normalizeResult(model: string, result: unknown): Effect.Effect<OpenRouterChatResult, OpenRouterError> {
  if (typeof result !== "object" || result === null || !("choices" in result)) {
    return Effect.fail(new OpenRouterUpstreamError({ reason: "OpenRouter reply missing choices" }));
  }
  const choices = (result as { choices?: unknown }).choices;
  if (!Array.isArray(choices) || choices.length === 0) {
    return Effect.fail(new OpenRouterUpstreamError({ reason: "OpenRouter reply has no completion choices" }));
  }
  const choice = choices[0] as { message?: { content?: unknown }; finishReason?: unknown };
  const content = extractContent(choice.message?.content);
  const finishReason = typeof choice.finishReason === "string" ? choice.finishReason : undefined;
  const usage = normalizeUsage((result as { usage?: unknown }).usage);
  const resultModel = (result as { model?: unknown }).model;
  const servedModel = typeof resultModel === "string" ? resultModel : model;
  return Effect.succeed({
    model: servedModel,
    content,
    ...(finishReason === undefined ? {} : { finishReason }),
    ...(usage === undefined ? {} : { usage }),
  });
}

// ----------------------------------------------------------------------------
// The live layer. Constructs the SDK client from `OpenRouterConfig` (fails
// closed). The key never leaves `Redacted` except where the SDK requires the raw
// string (passed straight into the SDK constructor, never logged).
// ----------------------------------------------------------------------------

/**
 * The real OpenRouter layer. Requires `OPENROUTER_API_KEY`; fails closed if
 * absent. Construct it ONLY behind an explicit live flag — never the test/CI
 * default.
 */
export const OpenRouterClientLive: Layer.Layer<OpenRouterClient, OpenRouterError> = Layer.effect(
  OpenRouterClient,
  Effect.gen(function* () {
    const config = yield* OpenRouterConfig.pipe(
      Effect.mapError(
        (cause) =>
          new OpenRouterAuthError({
            reason: `OPENROUTER_API_KEY not configured (fail-closed): ${String(cause)}`.slice(0, 200),
          }),
      ),
    );
    const sdk = new OpenRouter({ apiKey: Redacted.value(config.apiKey) });

    // Build the `chatRequest` payload for `sdk.chat.send`. The SDK types messages
    // as a role-discriminated union; we pass plain {role,content} objects (valid
    // for system/user/assistant/tool) and cast to the SDK's message type.
    const buildChatRequest = (input: OpenRouterChatInput) => ({
      model: input.model ?? config.model,
      messages: input.messages.map((m) => ({ role: m.role, content: m.content })) as never,
      temperature: input.temperature ?? 0,
      maxTokens: clampMaxTokens(input.maxTokens, config.defaultMaxTokens),
      ...(input.jsonObject ? { responseFormat: { type: "json_object" as const } } : {}),
    });

    const retrySchedule = Schedule.exponential(250).pipe(Schedule.either(Schedule.recurs(config.retries)));

    const chat: (input: OpenRouterChatInput) => Effect.Effect<OpenRouterChatResult, OpenRouterError> = (input) =>
      Effect.tryPromise({
        try: (signal) =>
          sdk.chat.send(
            { chatRequest: { ...buildChatRequest(input), stream: false } },
            { signal },
          ),
        catch: mapOpenRouterError,
      }).pipe(
        Effect.timeoutOrElse({
          duration: `${config.timeoutMs} millis`,
          orElse: () => Effect.fail(new OpenRouterTimeoutError({ reason: `exceeded ${config.timeoutMs}ms` })),
        }),
        Effect.flatMap((result) => normalizeResult(input.model ?? config.model, result)),
        Effect.retry({ schedule: retrySchedule, while: isRetryable }),
      );

    const chatStream: (
      input: OpenRouterChatInput & { readonly onChunk?: (chunk: OpenRouterStreamChunk) => void },
    ) => Effect.Effect<OpenRouterChatResult, OpenRouterError> = (input) =>
      Effect.tryPromise({
        try: async (signal) => {
          const stream = await sdk.chat.send(
            { chatRequest: { ...buildChatRequest(input), stream: true, streamOptions: { includeUsage: true } } },
            { signal },
          );
          let content = "";
          let finishReason: string | undefined;
          let usage: OpenRouterUsage | undefined;
          let servedModel = input.model ?? config.model;
          for await (const chunk of stream as AsyncIterable<unknown>) {
            const c = chunk as {
              model?: unknown;
              choices?: Array<{ delta?: { content?: unknown }; finishReason?: unknown }>;
              usage?: unknown;
            };
            if (typeof c.model === "string") servedModel = c.model;
            const delta = c.choices?.[0]?.delta?.content;
            if (typeof delta === "string" && delta.length > 0) {
              content += delta;
              input.onChunk?.({ content: delta });
            }
            const fr = c.choices?.[0]?.finishReason;
            if (typeof fr === "string") finishReason = fr;
            const u = normalizeUsage(c.usage);
            if (u) usage = u;
          }
          return { servedModel, content, finishReason, usage };
        },
        catch: mapOpenRouterError,
      }).pipe(
        Effect.timeoutOrElse({
          duration: `${config.timeoutMs} millis`,
          orElse: () => Effect.fail(new OpenRouterTimeoutError({ reason: `exceeded ${config.timeoutMs}ms` })),
        }),
        Effect.map((acc): OpenRouterChatResult => ({
          model: acc.servedModel,
          content: acc.content,
          ...(acc.finishReason === undefined ? {} : { finishReason: acc.finishReason }),
          ...(acc.usage === undefined ? {} : { usage: acc.usage }),
        })),
        Effect.retry({ schedule: retrySchedule, while: isRetryable }),
      );

    return { chat, chatStream };
  }),
);

// ----------------------------------------------------------------------------
// The mock layer — the DEFAULT in tests/CI. No network, no key, no spend.
// ----------------------------------------------------------------------------

export interface OpenRouterMockOptions {
  /**
   * Ordered canned replies. Each `chat`/`chatStream` call consumes the next one;
   * when exhausted, `fallback` (default `{}` → empty content) is returned.
   */
  readonly replies?: ReadonlyArray<Partial<OpenRouterChatResult> & { readonly content: string }>;
  /** A canned error to fail with on every call (for error-path tests). */
  readonly failWith?: OpenRouterError;
  /** Model id reported back in results (default "mock/echo"). */
  readonly model?: string;
}

/**
 * Build a mock `OpenRouterClient` layer from fixtures. This is what tests and CI
 * use by default: it never touches the network and needs no API key.
 */
export function makeOpenRouterClientMock(options: OpenRouterMockOptions = {}): Layer.Layer<OpenRouterClient> {
  const model = options.model ?? "mock/echo";
  let i = 0;
  const next = (): Effect.Effect<OpenRouterChatResult, OpenRouterError> => {
    if (options.failWith) return Effect.fail(options.failWith);
    const reply = options.replies?.[i++];
    if (!reply) return Effect.succeed({ model, content: "", finishReason: "stop" });
    return Effect.succeed({
      model: reply.model ?? model,
      content: reply.content,
      finishReason: reply.finishReason ?? "stop",
      ...(reply.usage === undefined ? {} : { usage: reply.usage }),
    });
  };
  return Layer.succeed(OpenRouterClient, {
    chat: (_input) => next(),
    chatStream: (input) =>
      next().pipe(
        Effect.tap((result) => Effect.sync(() => input.onChunk?.({ content: result.content }))),
      ),
  });
}

/** A zero-config mock layer (empty fixtures) for the common "no spend" default. */
export const OpenRouterClientMock: Layer.Layer<OpenRouterClient> = makeOpenRouterClientMock();
