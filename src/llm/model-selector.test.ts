import { describe, expect, it } from "bun:test";
import { cycleModel, getDefaultModelForProvider } from "./model-selector.js";

describe("model selector", () => {
  it("returns default model for known provider", () => {
    const model = getDefaultModelForProvider("openai");
    expect(model).toBeTruthy();
  });

  it("cycles models for provider list", () => {
    const next = cycleModel("openai", "gpt-5.1-codex");
    expect(next).toBeTruthy();
    expect(next).not.toBe("gpt-5.1-codex");
  });
});
