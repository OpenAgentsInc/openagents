import { describe, expect, test } from "vite-plus/test";
import {
  classifyCodexFailure,
  codexExecArgs,
  parseCodexExecOutput,
  summarizeCodexRun,
} from "./headless-harness-core";

// Observed wire fixtures from codex-cli 0.145.0-alpha.27 (2026-07-22 smoke).
const successOutput = [
  `{"type":"thread.started","thread_id":"019f87d4-111c-7263-9cc1-deaef824787e"}`,
  `{"type":"turn.started"}`,
  `{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"I'm Codex - an AI collaborator."}}`,
  `{"type":"turn.completed","usage":{"input_tokens":14267,"cached_input_tokens":11008,"cache_write_input_tokens":0,"output_tokens":37,"reasoning_output_tokens":0}}`,
].join("\n");

const authFailureOutput = [
  `{"type":"thread.started","thread_id":"019f87d3-3547-7210-adb4-7c32bcf3a01f"}`,
  `{"type":"turn.started"}`,
  `{"type":"error","message":"Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again."}`,
  `{"type":"turn.failed","error":{"message":"Your access token could not be refreshed because your refresh token was revoked. Please log out and sign in again."}}`,
].join("\n");

describe("parseCodexExecOutput", () => {
  test("parses the observed success wire", () => {
    const events = parseCodexExecOutput(successOutput);
    expect(events.map((event) => event.type)).toEqual([
      "thread.started",
      "turn.started",
      "item.completed",
      "turn.completed",
    ]);
  });

  test("ignores blank and non-JSON lines", () => {
    const events = parseCodexExecOutput(`\nnot json\n${successOutput}\n`);
    expect(events).toHaveLength(4);
  });
});

describe("summarizeCodexRun", () => {
  test("summarizes a completed identity turn with exact usage", () => {
    const summary = summarizeCodexRun(parseCodexExecOutput(successOutput));
    expect(summary.status).toBe("completed");
    expect(summary.threadId).toBe("019f87d4-111c-7263-9cc1-deaef824787e");
    expect(summary.finalAnswer).toContain("Codex");
    expect(summary.usage).toEqual({
      inputTokens: 14267,
      cachedInputTokens: 11008,
      outputTokens: 37,
      reasoningOutputTokens: 0,
    });
    expect(summary.failureClass).toBeNull();
    expect(summary.itemCounts.agent_message).toBe(1);
  });

  test("classifies the observed revoked-token failure as account_auth_failed", () => {
    const summary = summarizeCodexRun(parseCodexExecOutput(authFailureOutput));
    expect(summary.status).toBe("failed");
    expect(summary.failureClass).toBe("account_auth_failed");
    expect(summary.finalAnswer).toBeNull();
  });

  test("a turn without turn.completed fails as execution_failed", () => {
    const summary = summarizeCodexRun(
      parseCodexExecOutput(`{"type":"thread.started","thread_id":"t"}\n{"type":"turn.started"}`),
    );
    expect(summary.status).toBe("failed");
    expect(summary.failureClass).toBe("execution_failed");
  });
});

describe("classifyCodexFailure", () => {
  test("maps capacity and rate-limit messages to typed classes", () => {
    expect(classifyCodexFailure("You have hit your usage limit")).toBe("account_exhausted");
    expect(classifyCodexFailure("429 Too Many Requests")).toBe("account_rate_limited");
    expect(classifyCodexFailure("something else broke")).toBe("execution_failed");
  });
});

describe("codexExecArgs", () => {
  test("builds a bounded read-only exec invocation", () => {
    const argv = codexExecArgs({
      model: "gpt-5.6-terra",
      effort: "medium",
      workdir: "/tmp/w",
      prompt: "hey who are you",
    });
    expect(argv).toContain("--json");
    expect(argv).toContain("--skip-git-repo-check");
    expect(argv).toContain("gpt-5.6-terra");
    expect(argv).toContain(`model_reasoning_effort="medium"`);
    expect(argv[argv.indexOf("-s") + 1]).toBe("read-only");
    expect(argv[argv.length - 1]).toBe("hey who are you");
  });
});
