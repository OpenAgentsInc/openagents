import { describe, expect, test } from "bun:test";
import { calculateCost } from "./tokenAccounting.js";
import type { ChatResponse } from "./openrouter.js";
import { Effect } from "effect";

describe("tokenAccounting", () => {
  test("calculates costs from usage", async () => {
    const response: ChatResponse = {
      id: "1",
      choices: [],
      usage: { prompt_tokens: 1000, completion_tokens: 500, total_tokens: 1500 },
    };

    const result = await Effect.runPromise(calculateCost(response, { prompt: 0.002, completion: 0.004 }));

    expect(result.promptTokens).toBe(1000);
    expect(result.completionTokens).toBe(500);
    expect(result.totalTokens).toBe(1500);
    expect(result.costUsd).toBeCloseTo(0.002 * 1 + 0.004 * 0.5, 6);
  });

  test("handles missing usage gracefully", async () => {
    const response: ChatResponse = {
      id: "2",
      choices: [],
    };

    const result = await Effect.runPromise(calculateCost(response, { prompt: 0.001, completion: 0.001 }));
    expect(result.totalTokens).toBe(0);
    expect(result.costUsd).toBe(0);
  });
});
