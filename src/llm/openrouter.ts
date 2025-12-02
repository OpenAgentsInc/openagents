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

export interface OpenRouterConfigShape {
  apiKey: Secret.Secret;
  baseUrl: string;
  referer: Option.Option<string>;
  siteName: Option.Option<string>;
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

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.OPENROUTER_BASE_URL || "https://openrouter.ai/api/v1",
    referer: env.OPENROUTER_REFERER ? Option.some(env.OPENROUTER_REFERER) : Option.none(),
    siteName: env.OPENROUTER_SITE_NAME ? Option.some(env.OPENROUTER_SITE_NAME) : Option.none(),
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
  const defaultModel = "x-ai/grok-4.1-fast";
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

const sendChat = (
  client: OpenRouter,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: () =>
        client.chat.send({
          ...makeRequestBody(request),
        } as any),
      catch: (cause) => new Error(`OpenRouter request failed: ${String(cause)}`),
    });

    const choice = (response as any).choices?.[0];
    const message = choice?.message;

    const toolCalls: Array<{ id: string; function: { name: string; arguments: string } }> =
      message?.toolCalls ??
      (Array.isArray((message as any)?.tool_calls) ? (message as any).tool_calls : []);

    return {
      id: (response as any).id ?? "",
      usage: (response as any).usage,
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
  const client = new OpenRouter({
    apiKey: Secret.value(config.apiKey),
    serverURL: config.baseUrl,
    httpReferer: Option.getOrUndefined(config.referer),
    xTitle: Option.getOrUndefined(config.siteName),
  });

  return {
    chat: (request: ChatRequest) => sendChat(client, request),
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
    const client = createOpenRouterClient(config);
    return yield* sendChat(client, request);
  });
