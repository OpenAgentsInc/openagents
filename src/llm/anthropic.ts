import * as BunContext from "@effect/platform-bun/BunContext";
import * as JSONSchema from "effect/JSONSchema";
import * as Context from "effect/Context";
import { Effect, Layer } from "effect";
import * as Secret from "effect/Secret";
import type { Tool } from "../tools/schema.js";
import type { ChatRequest, ChatResponse, ChatMessage } from "./openrouter.js";

export interface AnthropicConfigShape {
  apiKey: Secret.Secret;
  baseUrl: string;
  defaultModel: string;
}

export class AnthropicConfig extends Context.Tag("AnthropicConfig")<
  AnthropicConfig,
  AnthropicConfigShape
>() {}

export const loadAnthropicEnv = (): AnthropicConfigShape => {
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const apiKey = env.ANTHROPIC_API_KEY;
  if (!apiKey) {
    throw new Error("Missing ANTHROPIC_API_KEY");
  }

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.ANTHROPIC_BASE_URL || "https://api.anthropic.com/v1/messages",
    defaultModel: env.ANTHROPIC_MODEL || "claude-3-5-sonnet-latest",
  };
};

export const anthropicConfigLayer = Layer.effect(AnthropicConfig, Effect.sync(loadAnthropicEnv));

export interface AnthropicClientShape {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error>;
}

export class AnthropicClient extends Context.Tag("AnthropicClient")<
  AnthropicClient,
  AnthropicClientShape
>() {}

const toolToAnthropicDefinition = (tool: Tool<any>): Record<string, unknown> => {
  const schema = JSONSchema.make(tool.schema) as unknown as Record<string, unknown>;
  const { $schema, ...inputSchema } = schema;
  return {
    name: tool.name,
    description: tool.description,
    input_schema: inputSchema,
  };
};

const messagesToAnthropic = (messages: ChatMessage[]): Array<{ role: "user" | "assistant"; content: any[] }> => {
  const anthropicMessages: Array<{ role: "user" | "assistant"; content: any[] }> = [];

  for (const message of messages) {
    if (message.role === "system") {
      continue;
    }
    if (message.role === "tool") {
      // Represent tool results as user messages with a text block
      anthropicMessages.push({
        role: "user",
        content: [{ type: "text", text: message.content }],
      });
      continue;
    }
    anthropicMessages.push({
      role: message.role,
      content: [{ type: "text", text: message.content ?? "" }],
    });
  }

  return anthropicMessages;
};

const makeRequestBody = (config: AnthropicConfigShape, request: ChatRequest) => {
  const systemMsg = request.messages.find((m) => m.role === "system")?.content;
  const messages = messagesToAnthropic(request.messages);
  const tools = request.tools?.map(toolToAnthropicDefinition);

  return {
    model: request.model ?? config.defaultModel,
    system: systemMsg,
    messages,
    tools,
    temperature: request.temperature,
    max_tokens: request.maxTokens ?? 1024,
  };
};

const sendAnthropic = (
  config: AnthropicConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> =>
  Effect.gen(function* () {
    const body = makeRequestBody(config, request);

    const response = yield* Effect.tryPromise({
      try: async () => {
        const res = await fetch(config.baseUrl, {
          method: "POST",
          headers: {
            "x-api-key": Secret.value(config.apiKey),
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
          },
          body: JSON.stringify(body),
        });

        if (!res.ok) {
          const text = await res.text();
          throw new Error(`HTTP ${res.status}: ${text}`);
        }

        return res.json();
      },
      catch: (cause) => new Error(`Anthropic request failed: ${String(cause)}`),
    });

    const message = (response as any).content?.[0];
    const text = message?.type === "text" ? message.text : "";

    return {
      id: (response as any).id ?? "",
      choices: [
        {
          message: {
            role: "assistant",
            content: text ?? "",
            tool_calls: [],
          },
        },
      ],
    };
  });

export const anthropicClientLive = Layer.effect(
  AnthropicClient,
  Effect.gen(function* () {
    const config = yield* AnthropicConfig;
    return {
      chat: (request: ChatRequest) => sendAnthropic(config, request),
    };
  }),
);

export const anthropicLive = Layer.provideMerge(
  Layer.mergeAll(anthropicConfigLayer),
  Layer.mergeAll(BunContext.layer),
);

export const anthropicLayers = Layer.mergeAll(anthropicConfigLayer, BunContext.layer, anthropicClientLive);
