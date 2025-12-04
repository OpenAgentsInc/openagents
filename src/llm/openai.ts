import * as BunContext from "@effect/platform-bun/BunContext";
import * as JSONSchema from "effect/JSONSchema";
import * as Context from "effect/Context";
import { Effect, Layer } from "effect";
import * as Secret from "effect/Secret";
import type { Tool } from "../tools/schema.js";
import type { ChatRequest, ChatResponse, ChatMessage } from "./openrouter.js";
import { HttpError, retryWithBackoff, isRetryableLlmError } from "./retry.js";

export interface OpenAIConfigShape {
  apiKey: Secret.Secret;
  baseUrl: string;
  defaultModel: string;
}

export class OpenAIConfig extends Context.Tag("OpenAIConfig")<
  OpenAIConfig,
  OpenAIConfigShape
>() {}

export const loadOpenAIEnv = (): OpenAIConfigShape => {
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const apiKey = env.OPENAI_API_KEY;
  if (!apiKey) {
    throw new Error("Missing OPENAI_API_KEY");
  }

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.OPENAI_BASE_URL || "https://api.openai.com/v1",
    defaultModel: env.OPENAI_MODEL || "gpt-4o-mini",
  };
};

export const openAIConfigLayer = Layer.effect(OpenAIConfig, Effect.sync(loadOpenAIEnv));

export interface OpenAIClientShape {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error>;
}

export class OpenAIClient extends Context.Tag("OpenAIClient")<
  OpenAIClient,
  OpenAIClientShape
>() {}

export const toolToOpenAIDefinition = (tool: Tool<any>): Record<string, unknown> => {
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

const contentBlocksToOpenAI = (content: string | any) => {
  if (!content) return "";
  if (typeof content === "string") return content;
  return content.map((block: any) => {
    if (block.type === "text") {
      return { type: "text", text: block.text };
    }
    return {
      type: "image_url",
      image_url: { url: `data:${block.mimeType};base64,${block.data}` },
    };
  });
};

const messagesToOpenAI = (messages: ChatMessage[]) =>
  messages.map((msg) => {
    if (msg.role === "tool" && msg.tool_call_id) {
      return {
        role: "tool",
        tool_call_id: msg.tool_call_id,
        content: typeof msg.content === "string" ? msg.content : "",
        ...(msg.name ? { name: msg.name } : {}),
      };
    }
    return { ...msg, content: contentBlocksToOpenAI(msg.content) };
  });

export const buildOpenAIRequestBody = (config: OpenAIConfigShape, request: ChatRequest) => {
  const tools = request.tools?.map(toolToOpenAIDefinition);

  return {
    model: request.model ?? config.defaultModel,
    messages: messagesToOpenAI(request.messages),
    tools,
    tool_choice: request.toolChoice ?? (tools && tools.length > 0 ? "auto" : undefined),
    temperature: request.temperature,
    max_tokens: request.maxTokens,
  };
};

const resolveProviderApiKey = (request: ChatRequest, config: OpenAIConfigShape): Secret.Secret => {
  if (request.apiKey) return Secret.fromString(request.apiKey);

  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const model = request.model ?? config.defaultModel;

  const isGroq = model.includes("groq");
  const isCerebras = model.includes("cerebras");
  const isXAI = model.includes("grok") || model.includes("xai");

  if (isGroq && env.GROQ_API_KEY) return Secret.fromString(env.GROQ_API_KEY);
  if (isCerebras && env.CEREBRAS_API_KEY) return Secret.fromString(env.CEREBRAS_API_KEY);
  if (isXAI && env.XAI_API_KEY) return Secret.fromString(env.XAI_API_KEY);

  return config.apiKey;
};

const resolveBaseUrl = (request: ChatRequest, config: OpenAIConfigShape): string => {
  if (request.baseUrl) return request.baseUrl;
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const model = request.model ?? config.defaultModel;

  if (model.includes("groq")) return env.GROQ_BASE_URL || "https://api.groq.com/openai/v1";
  if (model.includes("cerebras")) return env.CEREBRAS_BASE_URL || "https://api.cerebras.ai/v1";
  if (model.includes("grok") || model.includes("xai")) return env.XAI_BASE_URL || "https://api.x.ai/v1";

  return config.baseUrl;
};

const sendCompletions = (
  config: OpenAIConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> => {
  const sendOnce = Effect.gen(function* () {
    const body = buildOpenAIRequestBody(config, request);
    const baseUrl = resolveBaseUrl(request, config);
    const apiKey = resolveProviderApiKey(request, config);

    const response = yield* Effect.tryPromise({
      try: async () => {
        const res = await fetch(`${baseUrl}/chat/completions`, {
          method: "POST",
          headers: {
            "Authorization": `Bearer ${Secret.value(apiKey)}`,
            "Content-Type": "application/json",
            ...(request.headers ?? {}),
          },
          body: JSON.stringify(body),
        });
        if (!res.ok) {
          const text = await res.text();
          throw new HttpError(`HTTP ${res.status}`, res.status, text);
        }
        return res.json();
      },
      catch: (cause) =>
        cause instanceof HttpError ? cause : new HttpError(`OpenAI request failed: ${String(cause)}`),
    });

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

export const openAIClientLive = Layer.effect(
  OpenAIClient,
  Effect.gen(function* () {
    const config = yield* OpenAIConfig;
    return {
      chat: (request: ChatRequest) => sendCompletions(config, request),
    };
  }),
);

export const openAILayers = Layer.mergeAll(openAIConfigLayer, BunContext.layer, openAIClientLive);
