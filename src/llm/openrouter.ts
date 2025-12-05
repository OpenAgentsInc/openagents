import { OpenRouter } from "@openrouter/sdk";
import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as JSONSchema from "effect/JSONSchema";
import * as Context from "effect/Context";
import { Effect, Option } from "effect";
import * as Layer from "effect/Layer";
import * as DefaultServices from "effect/DefaultServices";
import * as Secret from "effect/Secret";

import type { Tool } from "../tools/schema.js";
import { HttpError, retryWithBackoff, isRetryableLlmError } from "./retry.js";

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface OpenRouterLogger {
  level: LogLevel;
  debug: (message: string) => void;
  info: (message: string) => void;
  warn: (message: string) => void;
  error: (message: string) => void;
}

const levelOrder: Record<LogLevel, number> = {
  debug: 10,
  info: 20,
  warn: 30,
  error: 40,
};

const parseLogLevel = (value: string | undefined, fallback: LogLevel): LogLevel => {
  if (!value) return fallback;
  const normalized = value.toLowerCase();
  if (normalized === "debug" || normalized === "info" || normalized === "warn" || normalized === "error") {
    return normalized;
  }
  return fallback;
};

const log = (logger: OpenRouterLogger, level: LogLevel, message: string) => {
  if (levelOrder[level] < levelOrder[logger.level]) return;
  logger[level](message);
};

const consoleLogger = (level: LogLevel = "warn"): OpenRouterLogger => ({
  level,
  debug: (msg) => console.debug(msg),
  info: (msg) => console.info(msg),
  warn: (msg) => console.warn(msg),
  error: (msg) => console.error(msg),
});

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "image"; data: string; mimeType: string };

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | ContentBlock[];
  tool_call_id?: string;
  name?: string;
}

export interface ChatRequest {
  model?: string;
  messages: ChatMessage[];
  tools?: Tool<any>[];
  temperature?: number;
  maxTokens?: number;
  toolChoice?: "auto" | "required" | { type: "function"; function: { name: string } };
  baseUrl?: string;
  apiKey?: string;
  headers?: Record<string, string>;
  retry?: Partial<import("./retry.js").RetryConfig>;
  timeoutMs?: number;
  logLevel?: LogLevel;
  logger?: OpenRouterLogger;
}

export interface ChatToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface ChatChoice {
  message: {
    role: "assistant";
    content: string | null;
    tool_calls?: ChatToolCall[];
  };
}

export interface ChatResponse {
  id: string;
  choices: ChatChoice[];
  usage?: {
    prompt_tokens?: number;
    completion_tokens?: number;
    total_tokens?: number;
  };
}

export interface OpenRouterConfigShape {
  apiKey: Secret.Secret;
  baseUrl: string;
  referer: Option.Option<string>;
  siteName: Option.Option<string>;
  timeoutMs: number;
  logLevel: LogLevel;
}

export class OpenRouterConfig extends Context.Tag("OpenRouterConfig")<
  OpenRouterConfig,
  OpenRouterConfigShape
>() {}

export const loadOpenRouterEnv = (): OpenRouterConfigShape => {
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const apiKey = env.OPENROUTER_API_KEY;
  if (!apiKey) {
    throw new Error("Missing OPENROUTER_API_KEY");
  }

  const parsedTimeout = Number(env.OPENROUTER_TIMEOUT_MS);
  const timeoutMs = Number.isFinite(parsedTimeout) && parsedTimeout > 0 ? parsedTimeout : 120_000;

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.OPENROUTER_BASE_URL || "https://openrouter.ai/api/v1",
    referer: env.OPENROUTER_REFERER ? Option.some(env.OPENROUTER_REFERER) : Option.none(),
    siteName: env.OPENROUTER_SITE_NAME ? Option.some(env.OPENROUTER_SITE_NAME) : Option.none(),
    timeoutMs,
    logLevel: parseLogLevel(env.OPENROUTER_LOG_LEVEL, "warn"),
  };
};

export const openRouterConfigLayer = Layer.effect(OpenRouterConfig, Effect.sync(loadOpenRouterEnv));

export const createOpenRouterClient = (config: OpenRouterConfigShape) =>
  new OpenRouter({
    apiKey: Secret.value(config.apiKey),
    serverURL: config.baseUrl,
    httpReferer: Option.getOrUndefined(config.referer),
    xTitle: Option.getOrUndefined(config.siteName),
  });

export const toolToOpenRouterDefinition = (tool: Tool<any>): Record<string, unknown> => {
  const schema = JSONSchema.make(tool.schema) as unknown as Record<string, unknown>;
  const { $schema, ...parameters } = schema;

  return {
    type: "function",
    function: {
      name: tool.name,
      description: tool.description,
      parameters,
      strict: true,
    },
  };
};

const makeRequestBody = (request: ChatRequest) => {
  const defaultModel = "x-ai/grok-4.1-fast:free";
  const tools = request.tools?.map(toolToOpenRouterDefinition);

  const messages = request.messages.map((msg) => {
    if (msg.role === "tool" && msg.tool_call_id) {
      return {
        role: "tool" as const,
        toolCallId: msg.tool_call_id,
        content: msg.content,
        ...(msg.name ? { name: msg.name } : {}),
      };
    }
    return msg;
  });

  return {
    model: request.model ?? defaultModel,
    messages,
    tools,
    tool_choice: request.toolChoice ?? (tools && tools.length > 0 ? "auto" : undefined),
    temperature: request.temperature,
    max_tokens: request.maxTokens,
    stream: false,
  };
};

const sendChatRaw = (
  config: OpenRouterConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> => {
  const timeoutMs = request.timeoutMs ?? config.timeoutMs;
  const logger =
    request.logger ??
    consoleLogger(
      request.logLevel ? parseLogLevel(request.logLevel, config.logLevel) : config.logLevel,
    );

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

    log(logger, "debug", `[OpenRouter] Sending request to ${config.baseUrl}/chat/completions`);
    log(logger, "debug", `[OpenRouter] Model: ${body.model}, Messages: ${apiMessages.length}`);

    const response = yield* Effect.tryPromise({
      try: async () => {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        try {
          const res = await fetch(`${config.baseUrl}/chat/completions`, {
            method: "POST",
            headers: {
              "Authorization": `Bearer ${Secret.value(config.apiKey)}`,
              "Content-Type": "application/json",
              ...(Option.isSome(config.referer) ? { "HTTP-Referer": config.referer.value } : {}),
              ...(Option.isSome(config.siteName) ? { "X-Title": config.siteName.value } : {}),
              ...(request.headers ?? {}),
            },
            body: JSON.stringify({ ...body, messages: apiMessages }),
            signal: controller.signal,
          });
          if (!res.ok) {
            const text = await res.text();
            log(logger, "warn", `[OpenRouter] HTTP ${res.status} ${text}`);
            throw new HttpError(`HTTP ${res.status}`, res.status, text);
          }
          return res.json();
        } catch (cause) {
          if (controller.signal.aborted) {
            log(logger, "warn", `[OpenRouter] Request timed out after ${timeoutMs}ms`);
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

    log(logger, "info", `[OpenRouter] Response received, id: ${(response as any).id}`);

    const choice = (response as any).choices?.[0];
    const message = choice?.message;

    const toolCalls: Array<{ id: string; function: { name: string; arguments: string } }> =
      message?.tool_calls ?? [];

    return {
      id: (response as any).id ?? "",
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
>() {}

const parseDotEnv = (contents: string) =>
  contents
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .reduce<Record<string, string>>((acc, line) => {
      const eq = line.indexOf("=");
      if (eq === -1) return acc;
      const key = line.slice(0, eq).trim();
      const raw = line.slice(eq + 1).trim();
      const unquoted = raw.replace(/^['"]|['"]$/g, "");
      acc[key] = unquoted;
      return acc;
    }, {});

export const dotenvLocalLayer = Layer.scopedDiscard(
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const path = yield* Path.Path;
    const envPath = path.join(process.cwd(), ".env.local");
    const exists = yield* fs.exists(envPath);
    if (!exists) return;

    const contents = yield* fs.readFileString(envPath);
    const parsed = parseDotEnv(contents);

    for (const [key, value] of Object.entries(parsed)) {
      if (process.env[key] === undefined) {
        process.env[key] = value;
      }
      if (typeof Bun !== "undefined" && Bun.env[key] === undefined) {
        Bun.env[key] = value;
      }
    }
  }),
);

const makeClient = Effect.gen(function* () {
  const config = yield* OpenRouterConfig;

  return {
    chat: (request: ChatRequest) => sendChatRaw(config, request),
  } satisfies OpenRouterClientShape;
});

export const openRouterClientLayer = Layer.effect(OpenRouterClient, makeClient);

const defaultServicesLayer = Layer.syncContext(() => DefaultServices.liveServices);

const platformLayer = Layer.mergeAll(defaultServicesLayer, BunContext.layer);
const envLayer = dotenvLocalLayer.pipe(Layer.provideMerge(platformLayer));
const baseLayer = Layer.mergeAll(platformLayer, envLayer, openRouterConfigLayer);

export const openRouterLive = openRouterClientLayer.pipe(Layer.provideMerge(baseLayer));

export const runOpenRouterChat = (request: ChatRequest) =>
  Effect.gen(function* () {
    const config = loadOpenRouterEnv();
    return yield* sendChatRaw(config, request);
  });
