// Unit tests for the Full Auto `threads.json` -> OpenAgentsConversation adapter.

import { describe, expect, it } from "vite-plus/test";

import {
  ATIF_PINNED_SCHEMA_VERSION,
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from "@openagentsinc/atif/trace";

import { convertOpenAgentsConversationToAtif } from "./openagents-conversation-to-atif";
import {
  findThreadConversationInThreadsFile,
  threadNotesToMessages,
  threadToConversation,
} from "./threads-to-conversation";

describe("threadNotesToMessages", () => {
  it("maps notes onto messages, preserving role and text as content/text", () => {
    const messages = threadNotesToMessages([
      { role: "system", text: "Full Auto run started.", timestamp: "02:26 PM" },
      { role: "user", text: "Execute the mission packet.", timestamp: "02:26 PM" },
      { role: "assistant", text: "On it." },
    ]);
    expect(messages).toEqual([
      { role: "system", content: "Full Auto run started.", text: "Full Auto run started." },
      { role: "user", content: "Execute the mission packet.", text: "Execute the mission packet." },
      { role: "assistant", content: "On it.", text: "On it." },
    ]);
  });

  it("preserves an ISO 8601 timestamp but drops a display-only time-of-day", () => {
    const [iso, display] = threadNotesToMessages([
      { role: "user", text: "with iso", timestamp: "2026-07-19T14:26:00.000Z" },
      { role: "user", text: "with display", timestamp: "02:26 PM" },
    ]);
    expect(iso.timestamp).toBe("2026-07-19T14:26:00.000Z");
    expect(display.timestamp).toBeUndefined();
  });

  it("returns [] for non-array notes", () => {
    expect(threadNotesToMessages(undefined)).toEqual([]);
    expect(threadNotesToMessages({})).toEqual([]);
  });
});

describe("threadToConversation", () => {
  it("carries id, title, and notes-as-messages", () => {
    const conversation = threadToConversation({
      id: "72d6ef5c-cc29-4472-bf5b-534632728184",
      title: "NIP-34 GitReply (Full Auto)",
      notes: [{ role: "user", text: "hello", timestamp: "02:26 PM" }],
    });
    expect(conversation.id).toBe("72d6ef5c-cc29-4472-bf5b-534632728184");
    expect(conversation.title).toBe("NIP-34 GitReply (Full Auto)");
    expect(Array.isArray(conversation.messages)).toBe(true);
    expect((conversation.messages as unknown[]).length).toBe(1);
  });

  it("adapts into a valid, tripwire-clean ATIF trajectory", () => {
    const conversation = threadToConversation({
      id: "thread-1",
      title: "Full Auto thread",
      notes: [
        { role: "system", text: "Full Auto run started.", timestamp: "02:26 PM" },
        { role: "user", text: "Do the work.", timestamp: "02:26 PM" },
        { role: "assistant", text: "Done.", timestamp: "02:27 PM" },
      ],
    });
    const trajectory = convertOpenAgentsConversationToAtif(conversation);
    const strict = decodeAtifTrajectorySync(trajectory);
    expect(trajectory.schema_version).toBe(ATIF_PINNED_SCHEMA_VERSION);
    expect(validateAtifTrajectory(strict)).toEqual([]);
    expect(atifTraceTripwire(strict)).toEqual([]);
  });
});

describe("findThreadConversationInThreadsFile", () => {
  const threadsFile = {
    version: 1,
    threads: [
      { id: "AAAA-1111", title: "one", notes: [{ role: "user", text: "a" }] },
      { id: "BBBB-2222", title: "two", notes: [{ role: "user", text: "b" }] },
    ],
  };

  it("finds a thread by id, case-insensitively", () => {
    const hit = findThreadConversationInThreadsFile(threadsFile, "bbbb-2222");
    expect(hit?.id).toBe("BBBB-2222");
    expect(hit?.title).toBe("two");
  });

  it("returns undefined for a non-threads document or an unknown id", () => {
    expect(findThreadConversationInThreadsFile([], "AAAA-1111")).toBeUndefined();
    expect(findThreadConversationInThreadsFile(threadsFile, "nope")).toBeUndefined();
  });
});
