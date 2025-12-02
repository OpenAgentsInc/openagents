import * as FileSystem from "@effect/platform/FileSystem";
import * as HttpBody from "@effect/platform/HttpBody";
import * as HttpClient from "@effect/platform/HttpClient";
import * as HttpClientRequest from "@effect/platform/HttpClientRequest";
import * as HttpClientError from "@effect/platform/HttpClientError";
import * as FetchHttpClient from "@effect/platform/FetchHttpClient";
import * as Path from "@effect/platform/Path";
import * as JSONSchema from "effect/JSONSchema";
import * as Config from "effect/Config";
import * as Context from "effect/Context";
import { Effect, Option } from "effect";
import * as Layer from "effect/Layer";
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
  model: string;
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

export const openRouterConfigLayer = Layer.effect(
  OpenRouterConfig,
  Config.unwrap(
    Config.all({
      apiKey: Config.secret("OPENROUTER_API_KEY"),
      baseUrl: Config.withDefault(
        Config.nonEmptyString("OPENROUTER_BASE_URL"),
        "https://openrouter.ai/api/v1",
      ),
      referer: Config.option(Config.nonEmptyString("OPENROUTER_REFERER")),
      siteName: Config.option(Config.nonEmptyString("OPENROUTER_SITE_NAME")),
    }),
  ),
);

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
  const tools = request.tools?.map(toolToOpenAI);

  return {
    model: request.model,
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
    const url = new URL("/chat/completions", config.baseUrl).toString();
    const body = makeRequestBody(request);

    const bodyJson = yield* _(HttpBody.json(body));

    const httpRequest = HttpClientRequest.post(url, {
      body: bodyJson,
      headers: buildHeaders(config),
      acceptJson: true,
    });

    const response = yield* _(http.execute(httpRequest));

    return yield* _(
      HttpClient.schemaJson(ChatResponseSchema)(response).pipe(
        Effect.map((parsed) => ({
          ...parsed,
          choices: parsed.choices.map((choice) => ({
            message: {
              role: "assistant" as const,
              content: choice.message.content,
              tool_calls: choice.message.tool_calls?.map((call) => ({
                id: call.id,
                name: call.function.name,
                arguments: call.function.arguments,
              })),
            },
          })),
        })),
      ),
    );
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

const dotenvLocalLayer = Layer.scopedDiscard(
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

export const openRouterLive = openRouterClientLayer.pipe(
  Layer.provideMerge(dotenvLocalLayer),
  Layer.provide(openRouterConfigLayer),
  Layer.provide(FetchHttpClient.layer),
);
