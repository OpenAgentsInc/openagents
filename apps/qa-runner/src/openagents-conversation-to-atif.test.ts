// Unit tests for the OpenAgents Desktop conversation -> ATIF converter.
//
// The desktop store keeps conversations as objects with a `messages` array
// (Anthropic-style content blocks). We assert the converter produces a
// structurally valid ATIF-v1.7 trajectory: sequential step ids, agent-only
// fields only on agent steps, and observations that reference real tool calls.

import { describe, expect, it } from "vite-plus/test";

import {
  ATIF_PINNED_SCHEMA_VERSION,
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from "@openagentsinc/atif/trace";

import { convertOpenAgentsConversationToAtif } from "./openagents-conversation-to-atif";

describe("convertOpenAgentsConversationToAtif", () => {
  it("maps roles, reasoning, tool calls, and tool results onto valid ATIF steps", () => {
    const conversation = {
      id: "C292D324-2BD7-4355-8B53-8D483151F04A",
      title: "Fix the login flow",
      createdAt: "2026-07-19T00:00:00.000Z",
      messages: [
        { role: "user", content: "Fix the failing login test." },
        {
          role: "assistant",
          model: "claude-opus-4-8",
          content: [
            { type: "thinking", thinking: "I should read the test first." },
            { type: "text", text: "Reading the test now." },
            {
              type: "tool_use",
              id: "toolu_1",
              name: "read_file",
              input: { path: "login.test.ts" },
            },
          ],
        },
        {
          role: "user",
          content: [
            { type: "tool_result", tool_use_id: "toolu_1", content: "expect(login()).toBe(true)" },
          ],
        },
        { role: "assistant", content: [{ type: "text", text: "Found it. Patching." }] },
      ],
    };

    const trajectory = convertOpenAgentsConversationToAtif(conversation);

    expect(trajectory.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
    expect(trajectory.session_id).toBe(conversation.id);
    // step ids are sequential from 1.
    expect(trajectory.steps.map((s) => s.step_id)).toEqual([1, 2, 3, 4]);
    expect(trajectory.steps[0]?.source).toBe("user");
    expect(trajectory.steps[1]?.source).toBe("agent");
    expect(trajectory.steps[1]?.reasoning_content).toContain("read the test");
    expect(trajectory.steps[1]?.tool_calls?.[0]?.tool_call_id).toBe("toolu_1");
    // the tool result becomes an observation referencing the emitted call.
    expect(trajectory.steps[2]?.observation?.results?.[0]?.source_call_id).toBe("toolu_1");

    // Strictly decodes + structurally valid + public-safe.
    const strict = decodeAtifTrajectorySync(trajectory);
    expect(validateAtifTrajectory(strict)).toEqual([]);
    expect(atifTraceTripwire(strict)).toEqual([]);
  });

  it("emits a single explanatory system step for an empty conversation", () => {
    const trajectory = convertOpenAgentsConversationToAtif({
      id: "empty",
      messages: [],
    });
    expect(trajectory.steps.length).toBe(1);
    expect(trajectory.steps[0]?.source).toBe("system");
    expect(validateAtifTrajectory(decodeAtifTrajectorySync(trajectory))).toEqual([]);
  });

  it("does not attach an observation for a tool_result with no matching call", () => {
    const trajectory = convertOpenAgentsConversationToAtif({
      id: "orphan",
      messages: [
        {
          role: "user",
          content: [{ type: "tool_result", tool_use_id: "missing", content: "orphaned" }],
        },
      ],
    });
    // No dangling observation => structural validator stays clean.
    expect(validateAtifTrajectory(decodeAtifTrajectorySync(trajectory))).toEqual([]);
  });
});
