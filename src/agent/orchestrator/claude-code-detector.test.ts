import { describe, test, expect } from "bun:test";
import { detectClaudeCode, type ClaudeCodeAvailability } from "./claude-code-detector.js";

describe("detectClaudeCode", () => {
  test("returns available when CLI found and SDK present", async () => {
    const result = await detectClaudeCode({
      cliChecker: () => ({ available: true, path: "/usr/bin/claude" }),
      sdkResolver: async () => ({ version: "0.1.0" }),
    });

    const expected: ClaudeCodeAvailability = {
      available: true,
      version: "0.1.0",
      cliPath: "/usr/bin/claude",
    };

    expect(result).toEqual(expected);
  });

  test("returns unavailable when CLI is not found", async () => {
    const result = await detectClaudeCode({
      cliChecker: () => ({ available: false }),
      sdkResolver: async () => ({ version: "0.1.0" }),
    });

    expect(result.available).toBe(false);
    expect(result.reason).toContain("Claude CLI not found");
  });

  test("returns unavailable when SDK is missing", async () => {
    const result = await detectClaudeCode({
      cliChecker: () => ({ available: true, path: "/usr/bin/claude" }),
      sdkResolver: async () => {
        throw new Error("MODULE_NOT_FOUND");
      },
    });

    expect(result.available).toBe(false);
    expect(result.reason).toContain("SDK not installed");
    expect(result.cliPath).toBe("/usr/bin/claude");
  });

  test("health check failure surfaces as unavailable", async () => {
    const result = await detectClaudeCode({
      cliChecker: () => ({ available: true, path: "/usr/bin/claude" }),
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
