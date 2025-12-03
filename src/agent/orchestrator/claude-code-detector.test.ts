import { describe, test, expect } from "bun:test";
import { detectClaudeCode, type ClaudeCodeAvailability } from "./claude-code-detector.js";

describe("detectClaudeCode", () => {
  test("returns available when SDK present and API key set", async () => {
    const result = await detectClaudeCode({
      env: { ANTHROPIC_API_KEY: "test-key" },
      sdkResolver: async () => ({ version: "0.1.0" }),
    });

    const expected: ClaudeCodeAvailability = {
      available: true,
      version: "0.1.0",
      apiKeySource: "env",
    };

    expect(result).toEqual(expected);
  });

  test("returns unavailable when SDK is missing", async () => {
    const result = await detectClaudeCode({
      env: { ANTHROPIC_API_KEY: "test-key" },
      sdkResolver: async () => {
        throw new Error("MODULE_NOT_FOUND");
      },
    });

    expect(result.available).toBe(false);
    expect(result.reason).toContain("SDK not installed");
  });

  test("returns unavailable when API key is missing", async () => {
    const result = await detectClaudeCode({
      sdkResolver: async () => ({ version: "0.1.0" }),
      env: {},
    });

    expect(result.available).toBe(false);
    expect(result.apiKeySource).toBe("none");
    expect(result.reason).toContain("ANTHROPIC_API_KEY");
  });

  test("health check failure surfaces as unavailable", async () => {
    const result = await detectClaudeCode({
      env: { ANTHROPIC_API_KEY: "test-key" },
      sdkResolver: async () => ({ version: "0.1.0" }),
      healthCheck: true,
      healthCheckFn: async () => {
        throw new Error("boom");
      },
    });

    expect(result.available).toBe(false);
    expect(result.reason).toContain("Health check failed");
  });
});
