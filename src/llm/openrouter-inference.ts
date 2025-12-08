import { Context, Effect, Layer } from "effect"
import { InferenceStore } from "./inference-store.js"
import { OpenRouterClient } from "./openrouter-http.js"

import type { ChatRequest, ChatResponse } from "./openrouter-types.js";
// ============================================================================
// Constants
// ============================================================================

/**
 * Default free model to use when free option is enabled.
 * This model is free to use and doesn't require credits.
 *
 * Note: arcee-ai/trinity-mini:free and qwen/qwen3-4b:free were returning empty content.
 * Using mistralai/mistral-7b-instruct:free as it should return actual content.
 */
export const DEFAULT_FREE_MODEL = "mistralai/mistral-7b-instruct:free";

// ============================================================================
// Interface
// ============================================================================

export interface IOpenRouterInference {
  /**
   * Send an inference request to OpenRouter.
   *
   * @param model - Model ID (defaults to 'openrouter/auto' for auto-selection)
   * @param messages - Array of chat messages
   * @param options - Optional request parameters (temperature, maxTokens, etc.)
   * @returns Effect that resolves to the chat response
   */
  readonly send: (
    model: string,
    messages: Array<{ role: "user" | "system" | "assistant"; content: string }>,
    options?: {
      free?: boolean; // If true, uses DEFAULT_FREE_MODEL (unless model is explicitly set to override)
      temperature?: number;
      maxTokens?: number;
      tools?: ChatRequest["tools"];
      toolChoice?: ChatRequest["toolChoice"];
    },
  ) => Effect.Effect<ChatResponse, Error, OpenRouterClient | InferenceStore>;
}

// ============================================================================
// Error Type
// ============================================================================

export class OpenRouterInferenceError extends Error {
  readonly _tag = "OpenRouterInferenceError";
  constructor(
    readonly reason: string,
    message: string,
    override readonly cause?: Error,
  ) {
    super(message);
    this.name = "OpenRouterInferenceError";
  }
}

// ============================================================================
// Context Tag
// ============================================================================

/**
 * Context.Tag for OpenRouterInference dependency injection.
 *
 * Usage:
 * ```typescript
 * const program = Effect.gen(function* () {
 *   const inference = yield* OpenRouterInference;
 *   const response = yield* inference.send("openrouter/auto", [
 *     { role: "user", content: "Hello!" }
 *   ]);
 * });
 * ```
 */
export class OpenRouterInference extends Context.Tag("OpenRouterInference")<
  OpenRouterInference,
  IOpenRouterInference
>() { }

// ============================================================================
// Implementation
// ============================================================================

const makeOpenRouterInference = (): IOpenRouterInference => ({
  send: (model, messages, options = {}) =>
    Effect.gen(function* () {
      const client = yield* OpenRouterClient;
      const store = yield* InferenceStore;

      // If free option is enabled, use default free model (unless model is explicitly set)
      // If model is already a free model (contains ":free"), respect it
      // Otherwise, if free=true, override with default free model
      const actualModel =
        options.free && !model.includes(":free") ? DEFAULT_FREE_MODEL : model;

      const request: ChatRequest = {
        model: actualModel,
        messages: messages.map((msg) => ({
          role: msg.role,
          content: msg.content,
        })),
        ...(options.temperature !== undefined && { temperature: options.temperature }),
        ...(options.maxTokens !== undefined && { maxTokens: options.maxTokens }),
        ...(options.tools && { tools: options.tools }),
        ...(options.toolChoice && { toolChoice: options.toolChoice }),
      };

      const response = yield* client.chat(request).pipe(
        Effect.mapError(
          (error) =>
            new OpenRouterInferenceError(
              "inference_failed",
              `OpenRouter inference failed: ${error.message}`,
              error,
            ),
        ),
      );

      // Save to database (fire and forget - don't fail the request if save fails)
      // Use actualModel for tracking (the model that was actually used)
      yield* store.save(actualModel, request, response).pipe(
        Effect.tapError((error) =>
          Effect.sync(() =>
            console.warn(`Failed to save inference to database: ${error.message}`),
          ),
        ),
        Effect.ignore,
      );

      return response;
    }),
});

// ============================================================================
// Layer
// ============================================================================

export const OpenRouterInferenceLive: Layer.Layer<
  OpenRouterInference,
  never,
  OpenRouterClient | InferenceStore
> = Layer.effect(OpenRouterInference, Effect.sync(() => makeOpenRouterInference()));
