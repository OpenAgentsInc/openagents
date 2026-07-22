import { describe, expect, test } from "vite-plus/test";
import type { ClaudeCodeMessage, ClaudeCodeQuery } from "@openagentsinc/agent-harness-contract";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import { runClaudeHarnessAttempt } from "./claude-harness-attempt";

/** Scripted query that captures options and streams the given messages. */
const scriptedQuery = (
  messages: ReadonlyArray<ClaudeCodeMessage>,
): { query: ClaudeCodeQuery; calls: Array<{ options: Record<string, unknown> }> } => {
  const calls: Array<{ options: Record<string, unknown> }> = [];
  const query: ClaudeCodeQuery = (params) => {
    calls.push({ options: params.options as Record<string, unknown> });
    return (async function* () {
      for (const message of messages) yield message;
    })();
  };
  return { query, calls };
};

const SUCCESS: ReadonlyArray<ClaudeCodeMessage> = [
  { type: "system", subtype: "init", session_id: "sess-1", model: "claude-haiku-4-5-20251001" },
  {
    type: "stream_event",
    event: {
      type: "content_block_delta",
      delta: { type: "text_delta", text: "hi there" },
    },
  },
  {
    type: "result",
    subtype: "success",
    is_error: false,
    result: "hi there",
    session_id: "sess-1",
    usage: { input_tokens: 200, output_tokens: 5, cache_read_input_tokens: 180 },
  },
] as unknown as ReadonlyArray<ClaudeCodeMessage>;

const baseInput = (
  overrides: Partial<Parameters<typeof runClaudeHarnessAttempt>[0]> & {
    query: ClaudeCodeQuery;
  },
) => ({
  threadRef: "thread-1",
  turnRef: "turn-1",
  workspace: "/tmp/w",
  prompt: "hey",
  model: "claude-haiku-4-5-20251001",
  resumeSessionId: null,
  queryOverrides: {},
  emit: () => {},
  ...overrides,
});

describe("claude harness attempt (HARN-09 slice 2)", () => {
  test("emits lowered renderer events, tees usage/session/model", async () => {
    const { query } = scriptedQuery(SUCCESS);
    const emitted: ClaudeLocalEvent[] = [];
    const result = await runClaudeHarnessAttempt(
      baseInput({ query, emit: (event) => emitted.push(event) }),
    );
    expect(result.outcome).toBe("success");
    expect(result.text).toBe("hi there");
    expect(result.sessionId).toBe("sess-1");
    expect(result.effectiveModel).toBe("claude-haiku-4-5-20251001");
    expect(result.usage).toEqual({
      inputTokens: 200,
      cachedInputTokens: 180,
      outputTokens: 5,
      reasoningTokens: 0,
      // input + cachedInput + output (200 + 180 + 5), matching the legacy
      // claude-local usage math for ledger exactness (#9167 slice 2).
      totalTokens: 385,
    });
    expect(emitted.map((event) => event.kind)).toContain("text_delta");
    expect(emitted.map((event) => event.kind)).toContain("turn_completed");
  });

  test("host queryOverrides reach the query call (custody preserved)", async () => {
    const { query, calls } = scriptedQuery(SUCCESS);
    const hostCanUseTool = async () => ({ behavior: "allow" as const, updatedInput: {} });
    await runClaudeHarnessAttempt(
      baseInput({
        query,
        queryOverrides: {
          canUseTool: hostCanUseTool,
          mcpServers: { delegate: { command: "node" } },
          pathToClaudeCodeExecutable: "/bundle/claude",
          maxTurns: 12,
        },
      }),
    );
    const options = calls[0]?.options;
    expect(options?.canUseTool).toBe(hostCanUseTool);
    expect(options?.pathToClaudeCodeExecutable).toBe("/bundle/claude");
    expect(options?.maxTurns).toBe(12);
  });

  test("resume passes the claude session id through", async () => {
    const { query, calls } = scriptedQuery(SUCCESS);
    await runClaudeHarnessAttempt(baseInput({ query, resumeSessionId: "sess-old" }));
    expect(calls[0]?.options.resume).toBe("sess-old");
  });

  test("a not-logged-in result classifies as reconnect_required", async () => {
    const { query } = scriptedQuery([
      { type: "system", subtype: "init", session_id: "sess-1", model: "claude-haiku-4-5-20251001" },
      {
        type: "result",
        subtype: "error",
        is_error: true,
        result: "Not logged in. Please run /login",
        session_id: "sess-1",
      },
    ] as unknown as ReadonlyArray<ClaudeCodeMessage>);
    const result = await runClaudeHarnessAttempt(baseInput({ query }));
    expect(result.outcome).toBe("reconnect_required");
  });
});
