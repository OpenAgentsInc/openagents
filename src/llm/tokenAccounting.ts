import { Effect } from "effect";
import type { ChatResponse } from "./openrouter.js";

export type Pricing = {
  prompt: number; // price per 1K prompt tokens (USD)
  completion: number; // price per 1K completion tokens (USD)
};

export interface CostBreakdown {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  costUsd: number;
}

export const calculateCost = (
  response: ChatResponse,
  pricing: Pricing,
): Effect.Effect<CostBreakdown, Error, never> =>
  Effect.gen(function* () {
    const promptTokens = response.usage?.prompt_tokens ?? 0;
    const completionTokens = response.usage?.completion_tokens ?? 0;
    const totalTokens = response.usage?.total_tokens ?? promptTokens + completionTokens;

    const costUsd =
      (promptTokens / 1000) * pricing.prompt + (completionTokens / 1000) * pricing.completion;

    return {
      promptTokens,
      completionTokens,
      totalTokens,
      costUsd,
    };
  });
