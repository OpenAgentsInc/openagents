import { describe, expect, it } from "bun:test";
import { normalizeUsage } from "./accounting.js";

describe("normalizeUsage", () => {
  it("fills missing fields with zeros", () => {
    const normalized = normalizeUsage(undefined);
    expect(normalized).toEqual({
      input: 0,
      output: 0,
      cacheRead: 0,
      cacheWrite: 0,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    });
  });

  it("preserves provided usage and cost fields", () => {
    const normalized = normalizeUsage({
      input: 10,
      output: 5,
      cost: { input: 1, output: 2, cacheRead: 0, cacheWrite: 0, total: 3 },
    });
    expect(normalized.input).toBe(10);
    expect(normalized.output).toBe(5);
    expect(normalized.cost.total).toBe(3);
  });
});
