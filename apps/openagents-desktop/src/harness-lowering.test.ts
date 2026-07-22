import { describe, expect, test } from "vite-plus/test";
import { Schema } from "effect";
import { ClaudeLocalEventSchema } from "./claude-local-contract";
import { makeClaudeLocalHarnessProjector } from "./harness-projection";
import { lowerHarnessEvent, lowerHarnessEvents } from "./harness-lowering";
import type { ClaudeLocalEvent } from "./claude-local-contract";

const ctx = {
  turnId: "turn-1",
  threadId: "thread-1",
  source: { lane: "codex_app_server" as const, adapterKind: "codex" as const },
};

// The HARN-03 forward projector is the fixture generator: every neutral
// event it produces must lower back onto a schema-valid renderer event of
// the matching kind (roundtrip at the kind level).
const forwardKinds: ReadonlyArray<[ClaudeLocalEvent, string]> = [
  [{ kind: "turn_started" }, "turn_started"],
  [{ kind: "text_delta", text: "hello" }, "text_delta"],
  [{ kind: "reasoning", text: "thinking" }, "reasoning"],
  [{ kind: "tool_use", toolName: "Bash", summary: "Bash", itemRef: "call-1" }, "tool_use"],
  [
    { kind: "tool_result", toolName: "Bash", ok: true, summary: "done", itemRef: "call-1" },
    "tool_result",
  ],
  [{ kind: "turn_completed", totalTokens: 42 }, "turn_completed"],
  [{ kind: "turn_failed", reason: "session_failed", detail: "boom" }, "turn_failed"],
];

describe("harness lowering (HARN-09 slice 0)", () => {
  test("round-trips every core kind through the forward projector", () => {
    const project = makeClaudeLocalHarnessProjector(ctx);
    for (const [renderer, kind] of forwardKinds) {
      const neutral = project(renderer);
      expect(neutral.length, kind).toBeGreaterThan(0);
      const lowered = lowerHarnessEvents(neutral);
      expect(lowered.length, kind).toBeGreaterThan(0);
      // turn_failed forward-projects to turn.interrupted, which lowers to
      // turn_failed with the canonical "interrupted" reason.
      const expectedKind = kind === "turn_failed" ? "turn_failed" : kind;
      expect(lowered[0].kind, kind).toBe(expectedKind);
      for (const event of lowered) {
        expect(() => Schema.decodeUnknownSync(ClaudeLocalEventSchema)(event), kind).not.toThrow();
      }
    }
  });

  test("tool identity survives the roundtrip", () => {
    const project = makeClaudeLocalHarnessProjector(ctx);
    const neutral = project({
      kind: "tool_use",
      toolName: "Bash",
      summary: "run tests",
      itemRef: "call-77",
    });
    const lowered = lowerHarnessEvents(neutral);
    expect(lowered[0]).toMatchObject({ kind: "tool_use", itemRef: "call-77" });
  });

  test("usage total lowers onto turn_completed totalTokens", () => {
    const project = makeClaudeLocalHarnessProjector(ctx);
    const neutral = project({ kind: "turn_completed", totalTokens: 1234 });
    const lowered = lowerHarnessEvents(neutral);
    expect(lowered[0]).toMatchObject({ kind: "turn_completed", totalTokens: 1234 });
  });

  test("neutral kinds outside the core subset lower to nothing", () => {
    const stepEvent = {
      kind: "step.started",
      turnId: "turn-1",
      threadId: "thread-1",
      sequence: 0,
      eventId: "evt.step.1",
      occurredAt: "2026-07-22T00:00:00.000Z",
      source: ctx.source,
      stepId: "step.1",
    };
    expect(lowerHarnessEvent(stepEvent as never)).toEqual([]);
  });

  test("bounds oversized text onto the envelope limits", () => {
    const lowered = lowerHarnessEvent({
      kind: "text.delta",
      turnId: "turn-1",
      threadId: "thread-1",
      sequence: 1,
      eventId: "evt.1",
      occurredAt: "2026-07-22T00:00:00.000Z",
      source: ctx.source,
      messageId: "msg.turn-1",
      text: "x".repeat(5000),
    } as never);
    expect(lowered).toHaveLength(1);
    expect((lowered[0] as { text: string }).text.length).toBeLessThanOrEqual(2000);
    expect(() => Schema.decodeUnknownSync(ClaudeLocalEventSchema)(lowered[0])).not.toThrow();
  });
});
