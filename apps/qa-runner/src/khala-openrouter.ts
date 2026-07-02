// OpenRouter backend for the Khala driver (openagents #6182).
//
// The Khala driver loop talks to a `ChatClient` (`complete(messages) => string`).
// gpt-oss-20b is a weak tool-caller (truncates JSON), so this module adapts the
// Effect-wrapped `OpenRouterClient` (in `@openagentsinc/probe-runtime`) into that
// same `ChatClient` seam, giving the JSON-action loop a reliable lane.
//
// Spend discipline (only a few $ of credits exist):
//   * the REAL OpenRouter client is constructed ONLY when explicitly armed
//     (`OPENROUTER_LIVE=1` + an `OPENROUTER_API_KEY`); otherwise a MOCK client is
//     used (no network, no spend) — this is the default in tests/CI;
//   * the default model is `openrouter/free` (cost=0, Free Models Router) so an
//     armed-but-careless run still cannot burn paid credits;
//   * a generous-but-bounded `max_tokens` lets a reasoning model still finish a
//     JSON action, and the wrapper hard-caps `max_tokens` regardless;
//   * the API key is read from `OPENROUTER_API_KEY` only and never logged.

import { Effect, Layer } from "effect";
import {
  OPENROUTER_DEFAULT_MAX_TOKENS,
  OPENROUTER_DEFAULT_MODEL,
  OpenRouterClient,
  OpenRouterClientLive,
  makeOpenRouterClientMock,
  type OpenRouterChatResult,
  type OpenRouterError,
  type OpenRouterMessage,
  type OpenRouterMockOptions,
} from "@openagentsinc/probe-runtime/openrouter";
import type { ChatClient, ChatMessage } from "./khala-driver";

/** A larger default token budget for the agentic loop: a reasoning model spends
 * tokens on a hidden channel before any visible JSON, so a tiny budget yields an
 * empty reply (the same failure as gpt-oss). Still bounded by the wrapper's hard
 * cap. Overridable via `OPENROUTER_DRIVER_MAX_TOKENS`. */
export const OPENROUTER_DRIVER_DEFAULT_MAX_TOKENS = 2048;

/** The set of backends the Khala coordinator can select for the driver loop. */
export type KhalaDriverBackend = "openrouter" | "gpt-oss";

export interface KhalaBackendSelection {
  readonly backend: KhalaDriverBackend;
  /** A neutral, key-free label explaining why this backend was chosen. */
  readonly reason: string;
  /** The model id that will drive the loop (label only; never a secret). */
  readonly model: string;
}

export interface SelectKhalaBackendOptions {
  readonly env?: Readonly<Record<string, string | undefined>>;
}

/**
 * Choose the Khala driver backend from env. OpenRouter is the reliable lane and
 * is selected when armed (`KHALA_DRIVER_BACKEND=openrouter`, or `OPENROUTER_LIVE`
 * truthy). Otherwise the loop falls back to the gpt-oss / OpenAI-compatible lane
 * (`khala-config.ts`). Selection is env-only and never touches the network.
 */
export function selectKhalaBackend(options: SelectKhalaBackendOptions = {}): KhalaBackendSelection {
  const env = options.env ?? process.env;
  const explicit = (env.KHALA_DRIVER_BACKEND ?? "").trim().toLowerCase();
  const model = nonEmpty(env.OPENROUTER_MODEL) ?? OPENROUTER_DEFAULT_MODEL;

  if (explicit === "gpt-oss" || explicit === "gpt_oss") {
    return { backend: "gpt-oss", reason: "KHALA_DRIVER_BACKEND=gpt-oss (explicit)", model: "openagents/khala" };
  }
  if (explicit === "openrouter") {
    return { backend: "openrouter", reason: "KHALA_DRIVER_BACKEND=openrouter (explicit)", model };
  }
  if (isTruthy(env.OPENROUTER_LIVE)) {
    return { backend: "openrouter", reason: "OPENROUTER_LIVE armed", model };
  }
  return {
    backend: "gpt-oss",
    reason: "default (OpenRouter not armed; set KHALA_DRIVER_BACKEND=openrouter or OPENROUTER_LIVE=1)",
    model: "openagents/khala",
  };
}

export interface OpenRouterChatClientOptions {
  readonly env?: Readonly<Record<string, string | undefined>>;
  /**
   * Force the underlying Effect layer. When omitted, the LIVE layer is used ONLY
   * if `OPENROUTER_LIVE` is truthy; otherwise the mock layer is used. Tests pass
   * an explicit mock layer (or mock fixtures) so they never hit the network.
   */
  readonly layer?: Layer.Layer<OpenRouterClient, OpenRouterError>;
  /** Mock fixtures, used to build a mock layer when `layer` is not supplied and
   * live is not armed. Lets tests script canned model replies. */
  readonly mock?: OpenRouterMockOptions;
  /** Per-request model override (defaults to env `OPENROUTER_MODEL` or free). */
  readonly model?: string;
  /** Per-request max_tokens (defaults to the generous driver budget). */
  readonly maxTokens?: number;
  /** Ask OpenRouter to enforce a JSON object response (default true for the
   * JSON-action loop, so the Free Models Router picks a JSON-capable model). */
  readonly jsonObject?: boolean;
  /** Use the streaming variant (reasoning tokens arrive in the final chunk). */
  readonly stream?: boolean;
  /** Optional key-free log sink (defaults to no-op). Never receives the key. */
  readonly log?: (line: string) => void;
}

/**
 * Build a `ChatClient` (the driver seam) backed by the Effect `OpenRouterClient`.
 * Each `complete(messages)` runs one Effect program with the resolved layer and
 * returns the assistant content (a tagged `OpenRouterError` becomes a thrown
 * error, which the driver records as an honest failure).
 *
 * Spend safety: the LIVE layer is built ONLY when armed; the default is the mock.
 */
export function makeOpenRouterChatClient(options: OpenRouterChatClientOptions = {}): ChatClient {
  const env = options.env ?? process.env;
  const armed = isTruthy(env.OPENROUTER_LIVE);
  const log = options.log ?? (() => undefined);
  const layer: Layer.Layer<OpenRouterClient, OpenRouterError> =
    options.layer ?? (armed ? OpenRouterClientLive : makeOpenRouterClientMock(options.mock));
  const model = options.model ?? nonEmpty(env.OPENROUTER_MODEL) ?? OPENROUTER_DEFAULT_MODEL;
  const maxTokens =
    options.maxTokens ?? parsePositiveInt(env.OPENROUTER_DRIVER_MAX_TOKENS) ?? OPENROUTER_DRIVER_DEFAULT_MAX_TOKENS;
  const jsonObject = options.jsonObject ?? true;
  const useStream = options.stream ?? false;

  log(`[khala/openrouter] backend armed=${armed} model=${model} max_tokens=${maxTokens} stream=${useStream}`);

  return {
    complete: async (messages: ReadonlyArray<ChatMessage>): Promise<string> => {
      const program = Effect.gen(function* () {
        const client = yield* OpenRouterClient;
        const input = {
          model,
          messages: messages.map(toOpenRouterMessage),
          temperature: 0,
          maxTokens,
          jsonObject,
        };
        const result: OpenRouterChatResult = useStream
          ? yield* client.chatStream(input)
          : yield* client.chat(input);
        return result;
      }).pipe(Effect.provide(layer));

      const result = await Effect.runPromise(program);
      // A reasoning model can finish_reason=length with empty content (reasoning
      // tokens consumed the budget). Surface that honestly instead of returning
      // an empty string the parser would silently reject.
      if (result.content.length === 0) {
        const fr = result.finishReason ?? "unknown";
        const rt = result.usage?.reasoningTokens;
        throw new Error(
          `OpenRouter returned empty content (finish_reason=${fr}` +
            (rt === undefined ? "" : `, reasoning_tokens=${rt}`) +
            "); increase OPENROUTER_DRIVER_MAX_TOKENS or pick a non-reasoning model.",
        );
      }
      return result.content;
    },
  };
}

function toOpenRouterMessage(m: ChatMessage): OpenRouterMessage {
  return { role: m.role, content: m.content };
}

function nonEmpty(value: string | undefined): string | undefined {
  if (value === undefined) return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function isTruthy(value: string | undefined): boolean {
  if (value === undefined) return false;
  const v = value.trim().toLowerCase();
  return v === "1" || v === "true" || v === "yes" || v === "on";
}

function parsePositiveInt(value: string | undefined): number | undefined {
  if (value === undefined) return undefined;
  const n = Number.parseInt(value.trim(), 10);
  return Number.isFinite(n) && n > 0 ? n : undefined;
}

// Re-export the defaults so callers/tests can reference them without reaching
// into the runtime package directly.
export { OPENROUTER_DEFAULT_MAX_TOKENS, OPENROUTER_DEFAULT_MODEL };
