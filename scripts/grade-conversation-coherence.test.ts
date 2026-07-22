import { describe, expect, test } from "vite-plus/test";
import {
  aggregateBySource,
  detectUserSignals,
  parseClaudeConversation,
  parseCodexConversation,
  scoreConversation,
} from "./coherence-core";

const codexLine = (payload: Record<string, unknown>, type = "event_msg"): string =>
  JSON.stringify({ timestamp: "2026-07-21T00:00:00Z", type, payload });

const claudeUser = (content: unknown, extra: Record<string, unknown> = {}): string =>
  JSON.stringify({
    type: "user",
    timestamp: "2026-07-21T00:00:00Z",
    message: { role: "user", content },
    ...extra,
  });

const claudeAssistant = (content: unknown): string =>
  JSON.stringify({
    type: "assistant",
    timestamp: "2026-07-21T00:00:01Z",
    message: { role: "assistant", content },
  });

describe("detectUserSignals", () => {
  test("flags profanity and corrections, once per kind per turn", () => {
    const signals = detectUserSignals("no, you did this wrong. this is fucking broken", 2);
    const kinds = signals.map((signal) => signal.kind).sort();
    expect(kinds).toEqual(["correction", "profanity"]);
    expect(signals.every((signal) => signal.userTurnIndex === 2)).toBe(true);
  });

  test("clean text produces no signals", () => {
    expect(detectUserSignals("please add a test for the parser", 1)).toEqual([]);
  });
});

describe("parseCodexConversation", () => {
  test("counts turns, tools, interrupts, and later-turn signals", () => {
    const content = [
      codexLine({ type: "user_message", message: "build the feature" }),
      codexLine({ type: "agent_message", message: "done" }),
      codexLine({ type: "turn_aborted", turn_id: "t1" }),
      codexLine({ type: "user_message", message: "wtf, that's not what I asked" }),
      codexLine({ type: "patch_apply_end", ok: true }),
      codexLine({ type: "agent_message", message: "fixed" }),
    ].join("\n");
    const parsed = parseCodexConversation("/tmp/a.jsonl", content);
    expect(parsed.userTurnCount).toBe(2);
    expect(parsed.assistantTurnCount).toBe(2);
    expect(parsed.interruptCount).toBe(1);
    expect(parsed.fileChangeCount).toBe(1);
    expect(parsed.signals.map((signal) => signal.kind).sort()).toEqual([
      "correction",
      "profanity",
    ]);
  });

  test("ignores injected user lines and corrupt JSON", () => {
    const content = [
      codexLine({ type: "user_message", message: "<environment_context>injected</environment_context>" }),
      "not json at all",
      codexLine({ type: "user_message", message: "real request" }),
      codexLine({ type: "agent_message", message: "ok" }),
    ].join("\n");
    const parsed = parseCodexConversation("/tmp/b.jsonl", content);
    expect(parsed.userTurnCount).toBe(1);
    expect(parsed.signals).toEqual([]);
  });
});

describe("parseClaudeConversation", () => {
  test("counts turns and tool calls, skips sidechains and interrupts", () => {
    const content = [
      claudeUser("please run the tests"),
      claudeAssistant([
        { type: "text", text: "running" },
        { type: "tool_use", name: "Bash", input: {} },
        { type: "tool_use", name: "Write", input: {} },
      ]),
      claudeUser([{ type: "text", text: "[Request interrupted by user]" }]),
      claudeUser("no, you broke the build again"),
      claudeAssistant([{ type: "text", text: "fixing" }]),
      claudeUser("hidden", { isSidechain: true }),
    ].join("\n");
    const parsed = parseClaudeConversation("/tmp/c.jsonl", content);
    expect(parsed.userTurnCount).toBe(2);
    expect(parsed.assistantTurnCount).toBe(2);
    expect(parsed.toolCallCount).toBe(2);
    expect(parsed.fileChangeCount).toBe(1);
    expect(parsed.interruptCount).toBe(1);
    expect(parsed.signals.map((signal) => signal.kind)).toEqual(["correction"]);
  });
});

describe("scoreConversation", () => {
  test("clean conversation scores 100 and passes", () => {
    const parsed = parseCodexConversation(
      "/tmp/clean.jsonl",
      [
        codexLine({ type: "user_message", message: "add a parser" }),
        codexLine({ type: "agent_message", message: "added" }),
      ].join("\n"),
    );
    const scored = scoreConversation(parsed);
    expect(scored.score).toBe(100);
    expect(scored.grade).toBe("A");
    expect(scored.disposition).toBe("screening_pass");
  });

  test("signals deduct with caps and set needs_review", () => {
    const messages = [
      codexLine({ type: "user_message", message: "start" }),
      codexLine({ type: "agent_message", message: "ok" }),
    ];
    for (let index = 0; index < 5; index += 1) {
      messages.push(codexLine({ type: "user_message", message: "no, you did this wrong, this is fucking bad" }));
      messages.push(codexLine({ type: "agent_message", message: "retrying" }));
    }
    const scored = scoreConversation(parseCodexConversation("/tmp/angry.jsonl", messages.join("\n")));
    expect(scored.deductions.profanity).toBe(45);
    expect(scored.deductions.correction).toBe(40);
    expect(scored.score).toBe(15);
    expect(scored.grade).toBe("F");
    expect(scored.disposition).toBe("needs_review");
  });

  test("threads without a full exchange are skipped", () => {
    const scored = scoreConversation(
      parseCodexConversation("/tmp/empty.jsonl", codexLine({ type: "user_message", message: "hi" })),
    );
    expect(scored.disposition).toBe("skipped");
  });
});

describe("aggregateBySource", () => {
  test("aggregates grades and signal counts per source", () => {
    const clean = scoreConversation(
      parseCodexConversation(
        "/tmp/a.jsonl",
        [
          codexLine({ type: "user_message", message: "task" }),
          codexLine({ type: "agent_message", message: "done" }),
        ].join("\n"),
      ),
    );
    const angry = scoreConversation(
      parseClaudeConversation(
        "/tmp/b.jsonl",
        [
          claudeUser("task"),
          claudeAssistant([{ type: "text", text: "done" }]),
          claudeUser("that's not what I asked"),
          claudeAssistant([{ type: "text", text: "redone" }]),
        ].join("\n"),
      ),
    );
    const aggregates = aggregateBySource([clean, angry]);
    const codex = aggregates.find((item) => item.source === "codex");
    const claude = aggregates.find((item) => item.source === "claude-code");
    expect(codex?.graded).toBe(1);
    expect(codex?.meanScore).toBe(100);
    expect(claude?.graded).toBe(1);
    expect(claude?.signalCounts.correction).toBe(1);
    expect(claude?.needsReview).toBe(0);
    expect(claude?.meanScore).toBe(90);
  });
});
