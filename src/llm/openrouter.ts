import * as FileSystem from "@effect/platform/FileSystem";
import * as HttpBody from "@effect/platform/HttpBody";
import * as HttpClient from "@effect/platform/HttpClient";
import * as HttpClientRequest from "@effect/platform/HttpClientRequest";
import * as HttpClientResponse from "@effect/platform/HttpClientResponse";
import * as HttpClientError from "@effect/platform/HttpClientError";
import * as FetchHttpClient from "@effect/platform/FetchHttpClient";
import * as Path from "@effect/platform/Path";
import * as BunContext from "@effect/platform-bun/BunContext";
import * as JSONSchema from "effect/JSONSchema";
import * as Context from "effect/Context";
import { Effect, Option } from "effect";
import * as Layer from "effect/Layer";
import * as DefaultServices from "effect/DefaultServices";
import * as Secret from "effect/Secret";
import * as S from "effect/Schema";

import type { Tool } from "../tools/schema.js";

export interface ChatMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
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

export interface OpenRouterConfig {
  apiKey: Secret.Secret;
  baseUrl: string;
  referer: Option.Option<string>;
  siteName: Option.Option<string>;
}

export const OpenRouterConfig = Context.Tag<OpenRouterConfig>("OpenRouterConfig");

const loadEnv = () => {
  const env = (globalThis as any).Bun?.env ?? process.env;
  const apiKey = env.OPENROUTER_API_KEY;
  if (!apiKey) {
    throw new Error("Missing OPENROUTER_API_KEY");
  }

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.OPENROUTER_BASE_URL || "https://openrouter.ai/api/v1",
    referer: env.OPENROUTER_REFERER ? Option.some(env.OPENROUTER_REFERER) : Option.none(),
    siteName: env.OPENROUTER_SITE_NAME ? Option.some(env.OPENROUTER_SITE_NAME) : Option.none(),
  };
};

export const openRouterConfigLayer = Layer.effect(OpenRouterConfig, Effect.sync(loadEnv));

const toolToOpenAI = (tool: Tool<any>) => {
  const schema = JSONSchema.make(tool.schema) as Record<string, unknown>;
  const { $schema, ...parameters } = schema;

  return {
    type: "function",
    function: {
      name: tool.name,
      description: tool.description,
      parameters,
    },
    strict: true,
  } as const;
};

const ChatResponseSchema = S.Struct({
  id: S.String,
  choices: S.Array(
    S.Struct({
      message: S.Struct({
        role: S.Literal("assistant"),
        content: S.Union(S.String, S.Null),
        tool_calls: S.optional(
          S.Array(
            S.Struct({
              id: S.String,
              type: S.Literal("function"),
              function: S.Struct({
                name: S.String,
                arguments: S.String,
              }),
            }),
          ),
        ),
      }),
    }),
  ),
  usage: S.optional(
    S.Struct({
      prompt_tokens: S.optional(S.Number),
      completion_tokens: S.optional(S.Number),
      total_tokens: S.optional(S.Number),
    }),
  ),
});

const makeRequestBody = (request: ChatRequest) => {
  const defaultModel = "x-ai/grok-4.1-fast";
  const tools = request.tools?.map(toolToOpenAI);

  return {
    model: request.model ?? defaultModel,
    messages: request.messages,
    tools,
    tool_choice: request.toolChoice ?? (tools && tools.length > 0 ? "auto" : undefined),
    temperature: request.temperature,
    max_tokens: request.maxTokens,
    stream: false,
  };
};

const buildHeaders = (config: OpenRouterConfig) => {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${Secret.value(config.apiKey)}`,
    "Content-Type": "application/json",
  };

  const referer = Option.getOrUndefined(config.referer);
  if (referer) headers["HTTP-Referer"] = referer;

  const site = Option.getOrUndefined(config.siteName);
  if (site) headers["X-Title"] = site;

  return headers;
};

const sendChat = (
  http: HttpClient.HttpClient,
  config: OpenRouterConfig,
  request: ChatRequest,
): Effect.Effect<ChatResponse, HttpClientError.HttpClientError | S.ParseError> =>
  Effect.gen(function* (_) {
    const url = `${config.baseUrl.replace(/\/$/, "")}/chat/completions`;
    const body = makeRequestBody(request);

    const bodyJson = yield* _(HttpBody.json(body));

    const httpRequest = HttpClientRequest.post(url, {
      body: bodyJson,
      headers: buildHeaders(config),
      acceptJson: true,
    });

    const response = yield* _(http.execute(httpRequest));

    const parsed: any = yield* _(
      response.json.pipe(
        Effect.catchAll(() =>
          Effect.flatMap(response.text, (text) =>
            Effect.fail(new Error(`Failed to parse OpenRouter response: ${text}`)),
          ),
        ),
      ),
    );

    return {
      id: parsed.id,
      usage: parsed.usage,
      choices:
        parsed.choices?.map((choice: any) => ({
          message: {
            role: "assistant" as const,
            content: choice.message?.content ?? null,
            tool_calls:
              choice.message?.tool_calls?.map((call: any) => ({
                id: call.id,
                name: call.function?.name ?? "",
                arguments: call.function?.arguments ?? "",
              })) ?? [],
          },
        })) ?? [],
    };
  });

export interface OpenRouterClient {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, HttpClientError.HttpClientError | S.ParseError>;
}

export const OpenRouterClient = Context.Tag<OpenRouterClient>("OpenRouterClient");

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
  Effect.gen(function* (_) {
    const fs = yield* _(FileSystem.FileSystem);
    const path = yield* _(Path.Path);
    const envPath = path.join(process.cwd(), ".env.local");
    const exists = yield* _(fs.exists(envPath));
    if (!exists) return;

    const contents = yield* _(fs.readFileString(envPath));
    const parsed = parseDotEnv(contents);

    for (const [key, value] of Object.entries(parsed)) {
      if (process.env[key] === undefined) {
        process.env[key] = value;
      }
      if ((globalThis as any).Bun?.env && (globalThis as any).Bun.env[key] === undefined) {
        (globalThis as any).Bun.env[key] = value;
      }
    }
  }),
);

const makeClient = Effect.gen(function* (_) {
  const http = yield* _(HttpClient.HttpClient);
  const config = yield* _(OpenRouterConfig);

  return {
    chat: (request: ChatRequest) => sendChat(http, config, request),
  } satisfies OpenRouterClient;
});

export const openRouterClientLayer = Layer.effect(OpenRouterClient, makeClient);

const defaultServicesLayer = Layer.syncContext(() => DefaultServices.liveServices);

const platformLayer = Layer.mergeAll(defaultServicesLayer, BunContext.layer, FetchHttpClient.layer);
const envLayer = dotenvLocalLayer.pipe(Layer.provideMerge(platformLayer));
const baseLayer = Layer.mergeAll(platformLayer, envLayer, openRouterConfigLayer);

export const openRouterLive = openRouterClientLayer.pipe(Layer.provideMerge(baseLayer));

export const runOpenRouterChat = (request: ChatRequest) =>
  Effect.gen(function* (_) {
    const http = yield* _(HttpClient.HttpClient);
    const config = loadEnv();
    return yield* _(sendChat(http, config, request));
  });
