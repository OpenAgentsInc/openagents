import { describe, expect, test } from "vite-plus/test";
import { multiTranscriptToConversation } from "./multi-transcript-to-conversation";

describe("multiTranscriptToConversation", () => {
  test("projects the combined transcript onto conversation messages", () => {
    const lines = [
      JSON.stringify({ type: "user_message", text: "build it" }),
      JSON.stringify({ type: "agent.child.started", lane: "codex-local", model: "gpt-5.6-terra" }),
      JSON.stringify({ type: "khala", lane: "codex-local", event: { kind: "tool.call", toolName: "shell" } }),
      JSON.stringify({ type: "khala", lane: "codex-local", event: { kind: "text.delta", text: "ignored" } }),
      JSON.stringify({ type: "agent.child.finished", lane: "codex-local", finishReason: "stop" }),
      JSON.stringify({ type: "agent.child.interacted", lane: "claude-local", fromLane: "codex-local", note: "handoff" }),
      JSON.stringify({ type: "assistant_message", text: "done" }),
    ].join("\n");
    const conversation = multiTranscriptToConversation(lines, { id: "t1" });
    const messages = (conversation as { messages: Array<{ role: string; content: string }> }).messages;
    expect(messages[0]).toEqual({ role: "user", content: "build it" });
    expect(messages.at(-1)).toEqual({ role: "assistant", content: "done" });
    expect(messages.some((m) => m.content.includes("delegate started: codex-local (gpt-5.6-terra)"))).toBe(true);
    expect(messages.some((m) => m.content.includes("tool: shell"))).toBe(true);
    expect(messages.some((m) => m.content.includes("handoff: codex-local -> claude-local"))).toBe(true);
    expect(messages.some((m) => m.content.includes("ignored"))).toBe(false);
  });
});
