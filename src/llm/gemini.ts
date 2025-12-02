import * as BunContext from "@effect/platform-bun/BunContext";
import * as JSONSchema from "effect/JSONSchema";
import * as Context from "effect/Context";
import { Effect, Layer } from "effect";
import * as Secret from "effect/Secret";
import type { Tool } from "../tools/schema.js";
import type { ChatRequest, ChatResponse, ChatMessage } from "./openrouter.js";

export interface GeminiConfigShape {
  apiKey: Secret.Secret;
  baseUrl: string;
  defaultModel: string;
}

export class GeminiConfig extends Context.Tag("GeminiConfig")<
  GeminiConfig,
  GeminiConfigShape
>() {}

export const loadGeminiEnv = (): GeminiConfigShape => {
  const env = typeof Bun !== "undefined" ? Bun.env : process.env;
  const apiKey = env.GEMINI_API_KEY || env.GOOGLE_API_KEY;
  if (!apiKey) {
    throw new Error("Missing GEMINI_API_KEY");
  }

  return {
    apiKey: Secret.fromString(apiKey),
    baseUrl: env.GEMINI_BASE_URL || "https://generativelanguage.googleapis.com/v1beta",
    defaultModel: env.GEMINI_MODEL || "gemini-1.5-flash",
  };
};

export const geminiConfigLayer = Layer.effect(GeminiConfig, Effect.sync(loadGeminiEnv));

export interface GeminiClientShape {
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, Error>;
}

export class GeminiClient extends Context.Tag("GeminiClient")<
  GeminiClient,
  GeminiClientShape
>() {}

export const toolToFunctionDeclaration = (tool: Tool<any>): Record<string, unknown> => {
  const schema = JSONSchema.make(tool.schema) as unknown as Record<string, unknown>;
  const { $schema, ...properties } = schema;

  return {
    name: tool.name,
    description: tool.description,
    parameters: properties,
  };
};

const mapMessages = (messages: ChatMessage[]) =>
  messages
    .filter((m) => m.role !== "tool")
    .map((m) => ({
      role: m.role === "user" ? "user" : "model",
      parts: [{ text: m.content ?? "" }],
    }));

const makeRequestBody = (config: GeminiConfigShape, request: ChatRequest) => {
  const tools = request.tools?.map(toolToFunctionDeclaration);

  return {
    contents: mapMessages(request.messages),
    tools: tools ? [{ functionDeclarations: tools }] : undefined,
    generationConfig: {
      temperature: request.temperature,
      maxOutputTokens: request.maxTokens,
    },
    model: request.model ?? config.defaultModel,
  };
};

const sendGemini = (
  config: GeminiConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> =>
  Effect.gen(function* () {
    const body = makeRequestBody(config, request);
    const url = `${config.baseUrl}/models/${body.model}:generateContent?key=${Secret.value(config.apiKey)}`;

    const response = yield* Effect.tryPromise({
      try: async () => {
        const res = await fetch(url, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            contents: body.contents,
            tools: body.tools,
            generationConfig: body.generationConfig,
          }),
        });

        if (!res.ok) {
          const text = await res.text();
          throw new Error(`HTTP ${res.status}: ${text}`);
        }

        return res.json();
      },
      catch: (cause) => new Error(`Gemini request failed: ${String(cause)}`),
    });

    const candidate = (response as any).candidates?.[0];
    const textPart = candidate?.content?.parts?.[0]?.text ?? "";

    return {
      id: (response as any).id ?? "",
      choices: [
        {
          message: {
            role: "assistant",
            content: textPart,
            tool_calls: [],
          },
        },
      ],
    };
  });

export const geminiClientLive = Layer.effect(
  GeminiClient,
  Effect.gen(function* () {
    const config = yield* GeminiConfig;
    return {
      chat: (request: ChatRequest) => sendGemini(config, request),
    };
  }),
);

export const geminiLayers = Layer.mergeAll(geminiConfigLayer, BunContext.layer, geminiClientLive);
