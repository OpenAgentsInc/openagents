/**
 * Minimal Coding Subagent Tests
 *
 * Following pi-mono's insight: models are RL-trained for coding,
 * they don't need 10K tokens of instructions.
 */
import { describe, test, expect } from "bun:test";
import {
  SUBAGENT_SYSTEM_PROMPT,
  buildSubagentPrompt,
  type Subtask,
} from "./types.js";

describe("SUBAGENT_SYSTEM_PROMPT", () => {
  test("is minimal (~50 tokens or less)", () => {
    // Rough token estimate: ~4 chars per token
    const estimatedTokens = Math.ceil(SUBAGENT_SYSTEM_PROMPT.length / 4);
    expect(estimatedTokens).toBeLessThan(100);
  });

  test("contains core instructions", () => {
    expect(SUBAGENT_SYSTEM_PROMPT).toContain("expert coding assistant");
    expect(SUBAGENT_SYSTEM_PROMPT).toContain("SUBTASK_COMPLETE");
  });

  test("lists only core tools", () => {
    expect(SUBAGENT_SYSTEM_PROMPT).toContain("read");
    expect(SUBAGENT_SYSTEM_PROMPT).toContain("write");
    expect(SUBAGENT_SYSTEM_PROMPT).toContain("edit");
    expect(SUBAGENT_SYSTEM_PROMPT).toContain("bash");
  });

  test("does NOT contain orchestrator concerns", () => {
    // These belong in the orchestrator, not the coding subagent
    const prompt = SUBAGENT_SYSTEM_PROMPT.toLowerCase();
    expect(prompt).not.toContain("git commit");
    expect(prompt).not.toContain("git push");
    expect(prompt).not.toContain("typecheck");
    expect(prompt).not.toContain("bun test");
    expect(prompt).not.toContain("effect.gen");
    expect(prompt).not.toContain("yield*");
  });
});

describe("buildSubagentPrompt", () => {
  test("includes subtask description", () => {
    const subtask: Subtask = {
      id: "sub-1",
      description: "Add a new function to calculate fibonacci numbers",
      status: "pending",
    };

    const prompt = buildSubagentPrompt(subtask);

    expect(prompt).toContain("fibonacci");
    expect(prompt).toContain("SUBTASK_COMPLETE");
  });

  test("is concise for simple tasks", () => {
    const subtask: Subtask = {
      id: "sub-2",
      description: "Fix typo in README",
      status: "pending",
    };

    const prompt = buildSubagentPrompt(subtask);

    // Should be short
    expect(prompt.length).toBeLessThan(200);
  });

  test("handles multi-line descriptions", () => {
    const subtask: Subtask = {
      id: "sub-3",
      description: `Implement feature X:
1. Add new file src/feature.ts
2. Export main function
3. Add tests`,
      status: "pending",
    };

    const prompt = buildSubagentPrompt(subtask);

    expect(prompt).toContain("1. Add new file");
    expect(prompt).toContain("2. Export main function");
    expect(prompt).toContain("3. Add tests");
  });
});

describe("subagent prompt design", () => {
  test("total prompt size (system + user) is minimal", () => {
    const subtask: Subtask = {
      id: "sub-test",
      description: "Implement a helper function",
      status: "pending",
    };

    const userPrompt = buildSubagentPrompt(subtask);
    const totalChars = SUBAGENT_SYSTEM_PROMPT.length + userPrompt.length;
    const estimatedTokens = Math.ceil(totalChars / 4);

    // Total should be under 200 tokens for a simple subtask
    expect(estimatedTokens).toBeLessThan(200);
  });

  test("prompt follows pi-mono minimal pattern", () => {
    // The key insight: models are RL-trained for coding
    // They don't need verbose instructions

    const systemPrompt = SUBAGENT_SYSTEM_PROMPT;

    // Should be 3 parts max: role, tools, completion signal
    const lines = systemPrompt
      .split("\n")
      .filter((l) => l.trim().length > 0);
    expect(lines.length).toBeLessThanOrEqual(4);
  });
});
