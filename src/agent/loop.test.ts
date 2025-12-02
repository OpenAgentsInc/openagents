import { describe, expect, test } from "bun:test";
import { Effect, Layer } from "effect";
import * as S from "effect/Schema";
import { agentLoop, type AgentTurn, type LoopEvent } from "./loop.js";
import type { Tool, ToolResult } from "../tools/schema.js";
import type { ChatRequest, ChatResponse } from "../llm/openrouter.js";
import { OpenRouterClient } from "../llm/openrouter.js";

const stubTool = (name: string, result: ToolResult): Tool<any> => ({
  name,
  label: name,
  description: "",
  schema: S.Struct({ msg: S.optional(S.String) }),
  execute: () => Effect.succeed(result),
});

const makeClient = (responses: ChatResponse[]) => {
  const calls: ChatRequest[] = [];
  return {
    chat: (req: ChatRequest) => {
      calls.push(req);
      const next = responses.shift();
      return next ? Effect.succeed(next) : Effect.fail(new Error("no response"));
    },
    calls,
  };
};

const withClient = (client: any) => Layer.succeed(OpenRouterClient, client);

const runLoop = (responses: ChatResponse[], tools: Tool<any>[], events: LoopEvent[] = []) =>
  Effect.runPromise(
    agentLoop("hello", tools, {
      maxTurns: 5,
      onEvent: (e) => events.push(e),
    }).pipe(Effect.provide(withClient(makeClient(responses))))
  );

describe("agentLoop", () => {
  test("emits events and executes tools", async () => {
    const events: LoopEvent[] = [];

    const responses: ChatResponse[] = [
      {
        id: "1",
        choices: [
          {
            message: {
              role: "assistant",
              content: null,
              tool_calls: [{ id: "tc1", name: "echo", arguments: "{\"msg\":\"hi\"}" }],
            },
          },
        ],
      },
      {
        id: "2",
        choices: [{ message: { role: "assistant", content: "done", tool_calls: [] } }],
      },
    ];

    const toolResult: ToolResult = { content: [{ type: "text", text: "ok" }] };
    const result = await runLoop(responses, [stubTool("echo", toolResult)], events);

    expect(result.turns[0].toolResults?.[0].result.content[0]?.text).toContain("ok");
    expect(events.map((e) => e.type)).toContain("tool_result");
    expect(events.map((e) => e.type)).toContain("llm_response");
  });

  test("handles invalid tool arguments", async () => {
    const events: LoopEvent[] = [];

    const responses: ChatResponse[] = [
      {
        id: "1",
        choices: [
          {
            message: {
              role: "assistant",
              content: null,
              tool_calls: [{ id: "tc1", name: "echo", arguments: "not-json" }],
            },
          },
        ],
      },
      {
        id: "2",
        choices: [{ message: { role: "assistant", content: "done", tool_calls: [] } }],
      },
    ];

    const toolResult: ToolResult = { content: [{ type: "text", text: "ok" }] };
    const result = await runLoop(responses, [stubTool("echo", toolResult)], events);

    expect(result.turns[0].toolResults?.[0].isError).toBe(true);
    expect(result.turns[0].toolResults?.[0].result.content[0]?.text).toContain("Invalid arguments");
  });

  test("tracks verification state via bash output", async () => {
    const events: LoopEvent[] = [];

    const responses: ChatResponse[] = [
      {
        id: "1",
        choices: [
          {
            message: {
              role: "assistant",
              content: null,
              tool_calls: [{ id: "tc1", name: "bash", arguments: "{\"command\":\"bun test\"}" }],
            },
          },
        ],
        usage: { prompt_tokens: 1, completion_tokens: 1 },
      },
      {
        id: "2",
        choices: [{ message: { role: "assistant", content: "done", tool_calls: [] } }],
      },
    ];

    const bashTool: Tool<any> = {
      name: "bash",
      label: "bash",
      description: "",
      schema: S.Struct({ command: S.String }),
      execute: () =>
        Effect.succeed({
          content: [{ type: "text", text: "bun test\npass\n0 fail" }],
        }),
    };

    const result = await runLoop(responses, [bashTool], events);

    expect(result.verifyState.testsOk).toBe(true);
    expect(result.verifyState.typecheckOk).toBe(false);
  });

  test("returns no_response error when client yields no choice", async () => {
    const responses: ChatResponse[] = [{ id: "1", choices: [] }];

    await expect(runLoop(responses, [], [])).rejects.toThrow("No response from LLM");
  });
});
