import * as BunContext from "@effect/platform-bun/BunContext";
import * as JSONSchema from "effect/JSONSchema";
import * as Context from "effect/Context";
import { Effect, Layer } from "effect";
import * as Secret from "effect/Secret";
import type { Tool } from "../tools/schema.js";
import type { ChatRequest, ChatResponse, ChatMessage } from "./openrouter.js";

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

const messagesToOpenAI = (messages: ChatMessage[]) =>
  messages.map((msg) => {
    if (msg.role === "tool" && msg.tool_call_id) {
      return {
        role: "tool",
        tool_call_id: msg.tool_call_id,
        content: msg.content,
        ...(msg.name ? { name: msg.name } : {}),
      };
    }
    return msg;
  });

const makeRequestBody = (config: OpenAIConfigShape, request: ChatRequest) => {
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

const sendOpenAI = (
  config: OpenAIConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> =>
  Effect.gen(function* () {
    const body = makeRequestBody(config, request);

    const response = yield* Effect.tryPromise({
      try: async () => {
        const res = await fetch(`${config.baseUrl}/chat/completions`, {
          method: "POST",
          headers: {
            "Authorization": `Bearer ${Secret.value(config.apiKey)}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify(body),
        });
        if (!res.ok) {
          const text = await res.text();
          throw new Error(`HTTP ${res.status}: ${text}`);
        }
        return res.json();
      },
      catch: (cause) => new Error(`OpenAI request failed: ${String(cause)}`),
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
            role: "assistant",
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

export const openAIClientLive = Layer.effect(
  OpenAIClient,
  Effect.gen(function* () {
    const config = yield* OpenAIConfig;
    return {
      chat: (request: ChatRequest) => sendOpenAI(config, request),
    };
  }),
);

export const openAILayers = Layer.mergeAll(openAIConfigLayer, BunContext.layer, openAIClientLive);
