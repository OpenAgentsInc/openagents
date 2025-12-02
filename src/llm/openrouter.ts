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
import * as S from "effect/Schema";
import type { ChatResponse, ChatMessageToolCall } from "@openrouter/sdk/esm/models/index.js";
import type { ToolDefinitionJson } from "@openrouter/sdk/esm/models/tooldefinitionjson.js";

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

export const loadOpenRouterEnv = (): OpenRouterConfig => {
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
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

export const openRouterConfigLayer = Layer.effect(OpenRouterConfig, Effect.sync(loadOpenRouterEnv));

export const createOpenRouterClient = (config: OpenRouterConfig) =>
  new OpenRouter({
    apiKey: Secret.value(config.apiKey),
    baseURL: config.baseUrl,
    headers: buildHeaders(config),
  });

export const toolToOpenRouterDefinition = (tool: Tool<any>): ToolDefinitionJson => {
  const schema = JSONSchema.make(tool.schema) as Record<string, unknown>;
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
  const tools = request.tools?.map(toolToOpenRouterDefinition);

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
  client: OpenRouter,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> =>
  Effect.gen(function* (_) {
    const response = yield* _(
      Effect.tryPromise({
        try: () =>
          client.chat.send({
            ...makeRequestBody(request),
          }),
        catch: (cause) => new Error(`OpenRouter request failed: ${String(cause)}`),
      }),
    );

    const choice = response.choices?.[0];
    const message = choice?.message;

    const toolCalls: ChatMessageToolCall[] =
      message?.toolCalls ??
      (Array.isArray((message as any)?.tool_calls) ? (message as any).tool_calls : []);

    return {
      id: response.id ?? "",
      usage: response.usage,
      choices: [
        {
          message: {
            role: "assistant",
            content: message?.content ?? null,
            tool_calls: toolCalls.map((call) => ({
              id: call.id,
              name: call.function.name,
              arguments: call.function.arguments,
            })),
          },
        },
      ],
    };
  });

export interface OpenRouterClient {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error>;
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
      if (typeof Bun !== "undefined" && Bun.env[key] === undefined) {
        Bun.env[key] = value;
      }
    }
  }),
);

const makeClient = Effect.gen(function* (_) {
  const config = yield* _(OpenRouterConfig);
  const client = new OpenRouter({
    apiKey: Secret.value(config.apiKey),
    baseURL: config.baseUrl,
    headers: buildHeaders(config),
  });

  return {
    chat: (request: ChatRequest) => sendChat(client, request),
  } satisfies OpenRouterClient;
});

export const openRouterClientLayer = Layer.effect(OpenRouterClient, makeClient);

const defaultServicesLayer = Layer.syncContext(() => DefaultServices.liveServices);

const platformLayer = Layer.mergeAll(defaultServicesLayer, BunContext.layer);
const envLayer = dotenvLocalLayer.pipe(Layer.provideMerge(platformLayer));
const baseLayer = Layer.mergeAll(platformLayer, envLayer, openRouterConfigLayer);

export const openRouterLive = openRouterClientLayer.pipe(Layer.provideMerge(baseLayer));

export const runOpenRouterChat = (request: ChatRequest) =>
  Effect.gen(function* (_) {
    const config = loadOpenRouterEnv();
    const client = createOpenRouterClient(config);
    return yield* _(sendChat(client, request));
  });
