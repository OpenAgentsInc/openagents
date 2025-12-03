import { describe, expect, it } from "bun:test";
import { nextThinkingLevel, supportsThinking } from "./thinking.js";

const mockModel = (reasoning: boolean) =>
  ({
    reasoning,
  }) as any;

describe("thinking helpers", () => {
  it("detects reasoning support", () => {
    expect(supportsThinking(mockModel(true))).toBe(true);
    expect(supportsThinking(mockModel(false))).toBe(false);
  });

  it("cycles thinking levels when supported", () => {
    expect(nextThinkingLevel("off", true)).toBe("minimal");
    expect(nextThinkingLevel("high", true)).toBe("off");
  });

  it("forces off when unsupported", () => {
    expect(nextThinkingLevel("medium", false)).toBe("off");
  });
});
