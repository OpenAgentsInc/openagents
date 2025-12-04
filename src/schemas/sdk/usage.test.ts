import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import { TokenUsage, CostBreakdown, NonNullableUsage } from "./usage.js";

describe("SDK Usage Schemas", () => {
  test("TokenUsage accepts optional token fields", () => {
    const decoded = S.decodeUnknownSync(TokenUsage)({
      input_tokens: 1200,
      cache_read_input_tokens: 50,
    });

    expect(decoded.input_tokens).toBe(1200);
    expect(decoded.output_tokens).toBeUndefined();
    expect(decoded.cache_read_input_tokens).toBe(50);
  });

  test("CostBreakdown accepts partial cost data", () => {
    const decoded = S.decodeUnknownSync(CostBreakdown)({
      input_cost_usd: 0.001,
      total_cost_usd: 0.0015,
    });

    expect(decoded.input_cost_usd).toBeCloseTo(0.001);
    expect(decoded.total_cost_usd).toBeCloseTo(0.0015);
    expect(decoded.cache_creation_cost_usd).toBeUndefined();
  });

  test("NonNullableUsage defaults missing fields to zero and derives total cost", () => {
    const decoded = S.decodeUnknownSync(NonNullableUsage)({
      input_tokens: 10,
      output_tokens: 5,
      input_cost_usd: 0.0005,
      output_cost_usd: 0.0007,
    });

    expect(decoded.cache_read_input_tokens).toBe(0);
    expect(decoded.cache_creation_input_tokens).toBe(0);
    expect(decoded.total_cost_usd).toBeCloseTo(0.0012);
  });

  test("NonNullableUsage honors provided total_cost_usd", () => {
    const decoded = S.decodeUnknownSync(NonNullableUsage)({
      total_cost_usd: 0.05,
    });

    expect(decoded.total_cost_usd).toBeCloseTo(0.05);
    expect(decoded.input_cost_usd).toBe(0);
  });
});
