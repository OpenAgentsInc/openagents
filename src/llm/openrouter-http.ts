import { Effect } from "effect"
import * as Context from "effect/Context"
import * as Layer from "effect/Layer"
import * as Secret from "effect/Secret"
import { OpenRouter } from "@openrouter/sdk"
import {
    loadOpenRouterEnv, logAtLevel, openRouterBaseLayer, OpenRouterConfig,
    parseLogLevel, resolveLogger
} from "./openrouter-config.js"
import type { OpenRouterConfigShape } from "./openrouter-config.js"
import { makeRequestBody } from "./openrouter-tools.js"
import { HttpError, isRetryableLlmError, retryWithBackoff } from "./retry.js"

import type { ChatRequest, ChatResponse } from "./openrouter-types.js";
import type { OpenRouterLogger } from "./openrouter-types.js";

const createOpenRouterClient = (config: OpenRouterConfigShape) =>
  new OpenRouter({
    apiKey: Secret.value(config.apiKey),
    serverURL: config.baseUrl,
    httpReferer: config.referer._tag === "Some" ? config.referer.value : undefined,
    xTitle: config.siteName._tag === "Some" ? config.siteName.value : undefined,
  });

const sendChatRaw = (
  config: OpenRouterConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> => {
  const timeoutMs = request.timeoutMs ?? config.timeoutMs;
  const logger: OpenRouterLogger = resolveLogger(
    config,
    request.logLevel ? parseLogLevel(request.logLevel, config.logLevel) : config.logLevel,
    request.logger,
  );
  const baseUrl = request.baseUrl ?? config.baseUrl;
  const apiKey = request.apiKey ? Secret.fromString(request.apiKey) : config.apiKey;
  const headers: Record<string, string> = {
    "Authorization": `Bearer ${Secret.value(apiKey)}`,
    "Content-Type": "application/json",
    ...(config.referer._tag === "Some" ? { "HTTP-Referer": config.referer.value } : {}),
    ...(config.siteName._tag === "Some" ? { "X-Title": config.siteName.value } : {}),
    ...(request.headers ?? {}),
  };

  const sendOnce = Effect.gen(function* () {
    const body = makeRequestBody(request);
    // Convert toolCallId back to tool_call_id for the API
    const apiMessages = body.messages.map((msg: any) => {
      if (msg.role === "tool" && msg.toolCallId) {
        const { toolCallId, ...rest } = msg;
        return { ...rest, tool_call_id: toolCallId };
      }
      return msg;
    });

    logAtLevel(
      logger,
      "debug",
      `[OpenRouter] Sending request to ${baseUrl}/chat/completions`,
    );
    logAtLevel(logger, "debug", `[OpenRouter] Model: ${body.model}, Messages: ${apiMessages.length}`);

    const response = yield* Effect.tryPromise({
      try: async () => {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        try {
          const res = await fetch(`${baseUrl}/chat/completions`, {
            method: "POST",
            headers,
            body: JSON.stringify({ ...body, messages: apiMessages }),
            signal: controller.signal,
          });
          if (!res.ok) {
            const text = await res.text();
            logAtLevel(logger, "warn", `[OpenRouter] HTTP ${res.status} ${text}`);
            throw new HttpError(`HTTP ${res.status}`, res.status, text);
          }
          return res.json();
        } catch (cause) {
          if (controller.signal.aborted) {
            logAtLevel(logger, "warn", `[OpenRouter] Request timed out after ${timeoutMs}ms`);
            throw new HttpError(`OpenRouter request timed out after ${timeoutMs}ms`);
          }
          throw cause;
        } finally {
          clearTimeout(timer);
        }
      },
      catch: (cause) =>
        cause instanceof HttpError
          ? cause
          : new HttpError(
            `OpenRouter request failed: ${String(cause instanceof Error ? cause.message : cause)}`,
          ),
    });

    logAtLevel(logger, "info", `[OpenRouter] Response received, id: ${(response as any).id}`);
    logAtLevel(logger, "debug", `[OpenRouter] Raw response: ${JSON.stringify(response, null, 2)}`);

    const choice = (response as any).choices?.[0];
    const message = choice?.message;

    const toolCalls: Array<{ id: string; function: { name: string; arguments: string } }> =
      message?.tool_calls ?? [];

    return {
      id: (response as any).id ?? "",
      model: (response as any).model, // Preserve model field from auto router
      usage: (response as any).usage,
      choices: [
        {
          message: {
            role: "assistant" as const,
            content: message?.content ?? null,
            tool_calls: toolCalls.map((call: any) => ({
              id: call.id,
              name: call.function.name,
              arguments: call.function.arguments,
            })),
          },
        },
      ],
    };
  });

  return retryWithBackoff(() => sendOnce, request.retry, isRetryableLlmError);
};

export interface OpenRouterClientShape {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error>;
}

export class OpenRouterClient extends Context.Tag("OpenRouterClient")<
  OpenRouterClient,
  OpenRouterClientShape
>() { }

const makeClient = Effect.gen(function* () {
  const config = yield* OpenRouterConfig;

  return {
    chat: (request: ChatRequest) => sendChatRaw(config, request),
  } satisfies OpenRouterClientShape;
});

export const openRouterClientLayer = Layer.effect(OpenRouterClient, makeClient);

export const openRouterLive = openRouterClientLayer.pipe(Layer.provideMerge(openRouterBaseLayer));

export const runOpenRouterChat = (request: ChatRequest) =>
  Effect.gen(function* () {
    const config = loadOpenRouterEnv();
    return yield* sendChatRaw(config, request);
  });

export { createOpenRouterClient, sendChatRaw };
