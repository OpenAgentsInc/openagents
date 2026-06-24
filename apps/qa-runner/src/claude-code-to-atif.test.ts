// Unit tests for the Claude Code .jsonl -> ATIF-v1.7 converter (#6220). The
// output must validate against atif-validate.ts and preserve the message /
// tool_use / tool_result / metrics mapping.

import { describe, expect, test } from "bun:test";
import { convertClaudeCodeEvents, convertClaudeCodeJsonl, type CcEvent } from "./claude-code-to-atif";
import { validateAtif } from "./atif-validate";

function asstEvent(
  uuid: string,
  msgId: string,
  ts: string,
  content: unknown,
  usage?: Record<string, number>,
): CcEvent {
  return {
    uuid,
    type: "assistant",
    timestamp: ts,
    sessionId: "sess-1",
    version: "2.0.0",
    cwd: "/Users/x/work",
    gitBranch: "main",
    message: {
      id: msgId,
      role: "assistant",
      model: "openagents/khala",
      content,
      ...(usage ? { usage } : {}),
    },
  };
}

function userEvent(uuid: string, ts: string, content: unknown): CcEvent {
  return { uuid, type: "user", timestamp: ts, sessionId: "sess-1", message: { role: "user", content } };
}

describe("convertClaudeCodeEvents", () => {
  test("a minimal user→agent→tool→result session converts to valid ATIF", () => {
    const events: CcEvent[] = [
      userEvent("u1", "2026-06-24T10:00:00.000Z", "Find the README and summarize it"),
      asstEvent(
        "a1",
        "msg_1",
        "2026-06-24T10:00:01.000Z",
        [
          { type: "thinking", thinking: "I should read the file first." },
          { type: "text", text: "Reading the README." },
          { type: "tool_use", id: "toolu_1", name: "Read", input: { file_path: "/Users/x/work/README.md" } },
        ],
        { input_tokens: 100, output_tokens: 20, cache_read_input_tokens: 50, cache_creation_input_tokens: 10 },
      ),
      userEvent("u2", "2026-06-24T10:00:02.000Z", [
        { type: "tool_result", tool_use_id: "toolu_1", content: "# README\nHello" },
      ]),
      asstEvent("a2", "msg_2", "2026-06-24T10:00:03.000Z", [
        { type: "text", text: "The README says hello." },
      ]),
    ];

    const traj = convertClaudeCodeEvents(events, { defaultModelName: "openagents/khala" });
    const v = validateAtif(traj);
    expect(v.errors).toEqual([]);
    expect(v.valid).toBe(true);

    expect(traj.schema_version).toBe("ATIF-v1.7");
    expect(traj.steps.length).toBe(3); // user goal, agent w/ tool, agent reply

    const goal = traj.steps[0];
    expect(goal.source).toBe("user");
    expect(goal.step_id).toBe(1);
    expect(goal.message).toBe("Find the README and summarize it");
    // validator: non-agent steps carry no tool_calls/reasoning/metrics
    expect(goal.tool_calls).toBeUndefined();

    const agent = traj.steps[1];
    expect(agent.source).toBe("agent");
    expect(agent.message).toBe("Reading the README.");
    expect(agent.reasoning_content).toBe("I should read the file first.");
    expect(agent.tool_calls?.[0]?.tool_call_id).toBe("toolu_1");
    expect(agent.tool_calls?.[0]?.function_name).toBe("Read");
    expect(agent.tool_calls?.[0]?.arguments.file_path).toBe("/Users/x/work/README.md");
    // tool_result correlated as observation on the SAME step
    expect(agent.observation?.results?.[0]?.source_call_id).toBe("toolu_1");
    expect(agent.observation?.results?.[0]?.content).toContain("Hello");
    // metrics mapped: prompt = input + cache_read + cache_creation
    expect(agent.metrics?.prompt_tokens).toBe(160);
    expect(agent.metrics?.completion_tokens).toBe(20);
  });

  test("multiple events sharing a message.id bundle into ONE step", () => {
    const events: CcEvent[] = [
      userEvent("u1", "2026-06-24T10:00:00.000Z", "go"),
      asstEvent("a1", "msg_1", "2026-06-24T10:00:01.000Z", [{ type: "text", text: "part one" }], {
        input_tokens: 10,
        output_tokens: 5,
      }),
      asstEvent("a2", "msg_1", "2026-06-24T10:00:01.500Z", [
        { type: "tool_use", id: "toolu_9", name: "Bash", input: { command: "ls" } },
      ]),
    ];
    const traj = convertClaudeCodeEvents(events);
    const agentSteps = traj.steps.filter((s) => s.source === "agent");
    expect(agentSteps.length).toBe(1);
    expect(agentSteps[0].message).toBe("part one");
    expect(agentSteps[0].tool_calls?.length).toBe(1);
    expect(validateAtif(traj).valid).toBe(true);
  });

  test("dedup by uuid drops repeated events", () => {
    const dup = userEvent("dup", "2026-06-24T10:00:00.000Z", "hello");
    const traj = convertClaudeCodeEvents([dup, dup]);
    expect(traj.steps.filter((s) => s.source === "user").length).toBe(1);
  });

  test("string-input tool_use is wrapped as { input }", () => {
    const events: CcEvent[] = [
      asstEvent("a1", "msg_1", "2026-06-24T10:00:01.000Z", [
        { type: "tool_use", id: "toolu_s", name: "X", input: "raw-string-arg" },
      ]),
    ];
    const traj = convertClaudeCodeEvents(events);
    const agent = traj.steps.find((s) => s.source === "agent");
    expect(agent?.tool_calls?.[0]?.arguments.input).toBe("raw-string-arg");
    expect(validateAtif(traj).valid).toBe(true);
  });

  test("orphan tool_result (no matching tool_use) still becomes a correlated observation", () => {
    const events: CcEvent[] = [
      userEvent("u1", "2026-06-24T10:00:02.000Z", [
        { type: "tool_result", tool_use_id: "toolu_missing", content: "late output" },
      ]),
    ];
    const traj = convertClaudeCodeEvents(events);
    const agent = traj.steps.find((s) => s.source === "agent");
    expect(agent).toBeDefined();
    expect(agent?.observation?.results?.[0]?.content).toBe("late output");
    // source_call_id must reference a tool_call_id in the SAME step
    const callId = agent?.tool_calls?.[0]?.tool_call_id;
    expect(agent?.observation?.results?.[0]?.source_call_id).toBe(callId);
    expect(validateAtif(traj).valid).toBe(true);
  });

  test("redacted_thinking blocks are dropped (not surfaced)", () => {
    const events: CcEvent[] = [
      asstEvent("a1", "msg_1", "2026-06-24T10:00:01.000Z", [
        { type: "redacted_thinking", data: "ciphertext-blob" },
        { type: "text", text: "answer" },
      ]),
    ];
    const traj = convertClaudeCodeEvents(events);
    const agent = traj.steps.find((s) => s.source === "agent");
    expect(agent?.message).toBe("answer");
    expect(agent?.reasoning_content).toBeUndefined();
    expect(JSON.stringify(traj)).not.toContain("ciphertext-blob");
  });

  test("final metrics aggregate per-step token counts", () => {
    const events: CcEvent[] = [
      userEvent("u1", "2026-06-24T10:00:00.000Z", "go"),
      asstEvent("a1", "msg_1", "2026-06-24T10:00:01.000Z", [{ type: "text", text: "a" }], {
        input_tokens: 10,
        output_tokens: 3,
      }),
      asstEvent("a2", "msg_2", "2026-06-24T10:00:02.000Z", [{ type: "text", text: "b" }], {
        input_tokens: 20,
        output_tokens: 4,
      }),
    ];
    const traj = convertClaudeCodeEvents(events);
    expect(traj.final_metrics?.total_prompt_tokens).toBe(30);
    expect(traj.final_metrics?.total_completion_tokens).toBe(7);
    expect(traj.final_metrics?.total_steps).toBe(traj.steps.length);
  });

  test("convertClaudeCodeJsonl parses a .jsonl string, skipping malformed lines", () => {
    const jsonl = [
      JSON.stringify(userEvent("u1", "2026-06-24T10:00:00.000Z", "hello")),
      "   ",
      "{ not valid json",
      JSON.stringify(asstEvent("a1", "msg_1", "2026-06-24T10:00:01.000Z", [{ type: "text", text: "hi" }])),
    ].join("\n");
    const traj = convertClaudeCodeJsonl(jsonl);
    expect(traj.steps.length).toBe(2);
    expect(validateAtif(traj).valid).toBe(true);
  });

  test("step_ids are sequential from 1", () => {
    const events: CcEvent[] = [
      userEvent("u1", "2026-06-24T10:00:00.000Z", "go"),
      asstEvent("a1", "msg_1", "2026-06-24T10:00:01.000Z", [{ type: "text", text: "a" }]),
      asstEvent("a2", "msg_2", "2026-06-24T10:00:02.000Z", [{ type: "text", text: "b" }]),
    ];
    const traj = convertClaudeCodeEvents(events);
    traj.steps.forEach((s, i) => expect(s.step_id).toBe(i + 1));
  });
});
