/**
 * Ollama client for local LLM inference.
 *
 * Uses Ollama's OpenAI-compatible API at /v1/chat/completions
 * for tool-calling capable models like codellama, deepseek-coder, qwen2.5-coder.
 *
 * Usage:
 *   const client = createOllamaClient({ endpoint: "http://localhost:11434", model: "codellama:34b" });
 *   const response = await client.chat({ messages: [...] });
 */

import { Effect, Context, Layer } from "effect";
import * as JSONSchema from "effect/JSONSchema";
import type { ChatRequest, ChatResponse, ChatMessage, ChatToolCall } from "./openrouter-types.js";
import type { Tool } from "../tools/schema.js";

// --- Configuration ---

export interface OllamaConfig {
  /** Ollama server endpoint (default: http://localhost:11434) */
  endpoint: string;
  /** Model name (e.g., "codellama:34b", "deepseek-coder:33b") */
  model: string;
  /** Request timeout in ms (default: 300000 = 5 minutes) */
  timeoutMs?: number;
}

export const DEFAULT_OLLAMA_ENDPOINT = "http://localhost:11434";
export const DEFAULT_OLLAMA_TIMEOUT_MS = 300_000;

// --- Error Types ---

export class OllamaError extends Error {
  readonly _tag = "OllamaError";
  constructor(
    readonly reason: "connection_failed" | "request_failed" | "invalid_response" | "timeout",
    message: string,
    readonly statusCode?: number,
  ) {
    super(message);
    this.name = "OllamaError";
  }
}

// --- Ollama API Types ---

interface OllamaToolFunction {
  name: string;
  description?: string;
  parameters?: Record<string, unknown>;
}

interface OllamaTool {
  type: "function";
  function: OllamaToolFunction;
}

interface OllamaMessage {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  tool_calls?: Array<{
    id: string;
    type: "function";
    function: {
      name: string;
      arguments: string;
    };
  }>;
  tool_call_id?: string;
  name?: string;
}

interface OllamaRequest {
  model: string;
  messages: OllamaMessage[];
  tools?: OllamaTool[];
  tool_choice?: "auto" | "required" | "none" | { type: "function"; function: { name: string } };
  temperature?: number;
  max_tokens?: number;
  stream?: boolean;
}

interface OllamaResponse {
  id: string;
  object: string;
  created: number;
  model: string;
  choices: Array<{
    index: number;
    message: {
      role: "assistant";
      content: string | null;
      tool_calls?: Array<{
        id: string;
        type: "function";
        function: {
          name: string;
          arguments: string;
        };
      }>;
    };
    finish_reason: string;
  }>;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
  };
}

// --- Conversion Functions ---

const convertMessage = (msg: ChatMessage): OllamaMessage => {
  const content = typeof msg.content === "string"
    ? msg.content
    : msg.content.map(block => block.type === "text" ? block.text : "[image]").join("\n");

  return {
    role: msg.role,
    content,
    ...(msg.tool_call_id ? { tool_call_id: msg.tool_call_id } : {}),
    ...(msg.name ? { name: msg.name } : {}),
  };
};

const convertTool = (tool: Tool<unknown>): OllamaTool => ({
  type: "function",
  function: {
    name: tool.name,
    description: tool.description,
    parameters: JSONSchema.make(tool.schema) as unknown as Record<string, unknown>,
  },
});

const convertResponse = (resp: OllamaResponse): ChatResponse => {
  const choice = resp.choices[0];
  const toolCalls: ChatToolCall[] | undefined = choice?.message.tool_calls?.map(tc => ({
    id: tc.id,
    name: tc.function.name,
    arguments: tc.function.arguments,
  }));

  const baseResponse = {
    id: resp.id,
    choices: [{
      message: {
        role: "assistant",
        content: choice?.message.content ?? null,
        ...(toolCalls?.length ? { tool_calls: toolCalls } : {}),
      },
    }],
  };

  return resp.usage
    ? { ...baseResponse, usage: {
        prompt_tokens: resp.usage.prompt_tokens,
        completion_tokens: resp.usage.completion_tokens,
        total_tokens: resp.usage.total_tokens,
      }}
    : baseResponse;
};

// --- Client Implementation ---

export interface OllamaClient {
  readonly config: OllamaConfig;
  chat: (request: ChatRequest) => Effect.Effect<ChatResponse, OllamaError>;
}

export class OllamaClientTag extends Context.Tag("OllamaClient")<
  OllamaClientTag,
  OllamaClient
>() {}

/**
 * Create an Ollama client for chat completions.
 */
export const createOllamaClient = (config: OllamaConfig): OllamaClient => {
  const endpoint = config.endpoint || DEFAULT_OLLAMA_ENDPOINT;
  const timeoutMs = config.timeoutMs ?? DEFAULT_OLLAMA_TIMEOUT_MS;

  const chat = (request: ChatRequest): Effect.Effect<ChatResponse, OllamaError> =>
    Effect.gen(function* () {
      const model = request.model ?? config.model;
      const url = `${endpoint}/v1/chat/completions`;

      const ollamaRequest: OllamaRequest = {
        model,
        messages: request.messages.map(convertMessage),
        stream: false,
        ...(request.tools?.length ? { tools: request.tools.map(convertTool) } : {}),
        ...(request.toolChoice ? { tool_choice: request.toolChoice } : {}),
        ...(request.temperature !== undefined ? { temperature: request.temperature } : {}),
        ...(request.maxTokens !== undefined ? { max_tokens: request.maxTokens } : {}),
      };

      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeoutMs);

      try {
        const response = yield* Effect.tryPromise({
          try: async () => {
            const resp = await fetch(url, {
              method: "POST",
              headers: {
                "Content-Type": "application/json",
              },
              body: JSON.stringify(ollamaRequest),
              signal: controller.signal,
            });

            if (!resp.ok) {
              const text = await resp.text();
              throw new OllamaError(
                "request_failed",
                `Ollama request failed: ${resp.status} ${resp.statusText} - ${text}`,
                resp.status,
              );
            }

            return resp.json() as Promise<OllamaResponse>;
          },
          catch: (e) => {
            if (e instanceof OllamaError) return e;
            if (e instanceof Error && e.name === "AbortError") {
              return new OllamaError("timeout", `Request timed out after ${timeoutMs}ms`);
            }
            if (e instanceof TypeError && e.message.includes("fetch")) {
              return new OllamaError(
                "connection_failed",
                `Failed to connect to Ollama at ${endpoint}: ${e.message}`,
              );
            }
            return new OllamaError(
              "request_failed",
              `Ollama request failed: ${e instanceof Error ? e.message : String(e)}`,
            );
          },
        });

        clearTimeout(timeoutId);

        // Validate response structure
        if (!response.choices?.length) {
          return yield* Effect.fail(
            new OllamaError("invalid_response", "Ollama response has no choices"),
          );
        }

        return convertResponse(response);
      } finally {
        clearTimeout(timeoutId);
      }
    });

  return {
    config: { ...config, endpoint },
    chat,
  };
};

/**
 * Create a Layer that provides OllamaClient from config.
 */
export const ollamaClientLayer = (config: OllamaConfig): Layer.Layer<OllamaClient> =>
  Layer.succeed(OllamaClientTag, createOllamaClient(config));

// --- Health Check ---

/**
 * Check if Ollama is running and the specified model is available.
 */
export const checkOllamaHealth = (
  endpoint = DEFAULT_OLLAMA_ENDPOINT,
): Effect.Effect<{ available: boolean; models: string[] }, OllamaError> =>
  Effect.gen(function* () {
    const response = yield* Effect.tryPromise({
      try: async () => {
        const resp = await fetch(`${endpoint}/api/tags`);
        if (!resp.ok) {
          throw new Error(`HTTP ${resp.status}`);
        }
        return resp.json() as Promise<{ models?: Array<{ name: string }> }>;
      },
      catch: (e) =>
        new OllamaError(
          "connection_failed",
          `Failed to connect to Ollama at ${endpoint}: ${e instanceof Error ? e.message : String(e)}`,
        ),
    });

    const models = response.models?.map(m => m.name) ?? [];
    return { available: true, models };
  });

/**
 * Check if a specific model is available in Ollama.
 */
export const isModelAvailable = (
  model: string,
  endpoint = DEFAULT_OLLAMA_ENDPOINT,
): Effect.Effect<boolean, OllamaError> =>
  Effect.gen(function* () {
    const health = yield* checkOllamaHealth(endpoint);
    // Check both exact match and prefix match (e.g., "codellama:34b" matches "codellama:34b-instruct")
    return health.models.some(m =>
      m === model || m.startsWith(model.split(":")[0] + ":"),
    );
  });
