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

const mapMessages = (messages: ChatMessage[], modelSupportsImages: boolean) =>
  messages.flatMap((m) => {
    if (m.role === "user") {
      if (typeof m.content === "string") {
        return [{ role: "user", parts: [{ text: m.content }] }];
      }
      const parts =
        m.content?.flatMap((c) => {
          if (c.type === "text") return [{ text: c.text }];
          if (c.type === "image" && modelSupportsImages) {
            return [{ inlineData: { mimeType: c.mimeType, data: c.data } }];
          }
          return [];
        }) ?? [];
      if (parts.length === 0) return [];
      return [{ role: "user", parts }];
    }

    if (m.role === "assistant") {
      const parts: Array<Record<string, unknown>> = [];
      const toolCalls = (m as any).tool_calls as
        | Array<{ id: string; name: string; arguments: string }>
        | undefined;
      if (toolCalls && toolCalls.length > 0) {
        for (const call of toolCalls) {
          parts.push({
            functionCall: {
              id: call.id,
              name: call.name,
              args: JSON.parse(call.arguments || "{}"),
            },
          });
        }
      } else if (m.content) {
        parts.push({ text: m.content as string });
      }
      if (parts.length === 0) return [];
      return [{ role: "model", parts }];
    }

    if (m.role === "tool") {
      // Map tool results to functionResponse parts
      return [
        {
          role: "user",
          parts: [
            {
              functionResponse: {
                name: m.name ?? "tool",
                response: { name: m.name ?? "tool", content: m.content },
              },
            },
          ],
        },
      ];
    }

    return [];
  });

export const makeGeminiRequestBody = (config: GeminiConfigShape, request: ChatRequest) => {
  const tools = request.tools?.map(toolToFunctionDeclaration);
  const model = request.model ?? config.defaultModel;
  const contents = mapMessages(request.messages, Boolean(model.includes("pro") || model.includes("flash")));

  const generationConfig: Record<string, unknown> = {};
  if (request.temperature !== undefined) generationConfig.temperature = request.temperature;
  if (request.maxTokens !== undefined) generationConfig.maxOutputTokens = request.maxTokens;

  const toolConfig =
    tools && tools.length > 0
      ? {
          functionCallingConfig: {
            mode: "AUTO" as const,
          },
        }
      : undefined;

  return {
    model,
    contents,
    tools: tools ? [{ functionDeclarations: tools }] : undefined,
    generationConfig: Object.keys(generationConfig).length ? generationConfig : undefined,
    toolConfig,
  };
};

const sendGemini = (
  config: GeminiConfigShape,
  request: ChatRequest,
): Effect.Effect<ChatResponse, Error> =>
  Effect.gen(function* () {
    const body = makeGeminiRequestBody(config, request);
    const url = `${config.baseUrl}/models/${body.model}:generateContent?key=${Secret.value(config.apiKey)}`;

    const response = yield* Effect.tryPromise({
      try: async () => {
        const res = await fetch(url, {
          method: "POST",
          headers: {
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
      catch: (cause) => new Error(`Gemini request failed: ${String(cause)}`),
    });

    const candidate = (response as any).candidates?.[0];
    const textPart = candidate?.content?.parts?.find((p: any) => p.text)?.text ?? "";

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
