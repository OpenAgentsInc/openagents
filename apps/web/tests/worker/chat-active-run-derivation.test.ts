import { describe, expect, it } from "vitest";

import { deriveActiveRunIdFromSnapshotRows } from "../../src/effect/chat";

describe("apps/web chat active run derivation", () => {
  it("treats a streaming assistant message with a finish part as not active", () => {
    const messages = [
      {
        messageId: "m1",
        role: "assistant",
        status: "streaming",
        text: "",
        runId: "run-1",
      },
    ];

    const finishByMessageId = new Map([
      [
        "m1",
        {
          reason: "stop",
          usage: { inputTokens: 1, outputTokens: 1, totalTokens: 2 },
        },
      ],
    ]);

    expect(deriveActiveRunIdFromSnapshotRows({ messages, finishByMessageId })).toBe(null);
  });

  it("returns the runId for a streaming assistant message when no finish exists", () => {
    const messages = [
      {
        messageId: "m1",
        role: "assistant",
        status: "streaming",
        text: "",
        runId: "run-1",
      },
    ];

    const finishByMessageId = new Map();

    expect(deriveActiveRunIdFromSnapshotRows({ messages, finishByMessageId })).toBe("run-1");
  });

  it("selects the last active streaming run when multiple assistant messages stream", () => {
    const messages = [
      {
        messageId: "m1",
        role: "assistant",
        status: "streaming",
        text: "",
        runId: "run-1",
      },
      {
        messageId: "m2",
        role: "assistant",
        status: "streaming",
        text: "",
        runId: "run-2",
      },
    ];

    const finishByMessageId = new Map([
      [
        "m1",
        {
          reason: "stop",
        },
      ],
    ]);

    expect(deriveActiveRunIdFromSnapshotRows({ messages, finishByMessageId })).toBe("run-2");
  });
});
