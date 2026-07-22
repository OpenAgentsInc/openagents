import { describe, expect, test } from "vite-plus/test";
import type { ClaudeLocalEvent } from "./claude-local-contract";
import type { HeadlessTurnResult } from "./desktop-headless-host";
import {
  deriveHeadlessReceipts,
  screenDelegatedTurn,
  screenHeadlessTurn,
} from "./desktop-headless-receipt";

const frame = (event: ClaudeLocalEvent) => ({ turnRef: "turn-1", event });

const result = (
  events: ReadonlyArray<ClaudeLocalEvent>,
  overrides: Partial<HeadlessTurnResult> = {},
): HeadlessTurnResult => ({
  dispatch: { ok: true },
  frames: events.map(frame),
  thread: null,
  fullAutoRecordCount: 0,
  ...overrides,
});

const COMPLETED: ReadonlyArray<ClaudeLocalEvent> = [
  { kind: "turn_started" },
  { kind: "model_effective", model: "gpt-5.6-terra" },
  { kind: "text_delta", text: "I am Codex." },
  { kind: "turn_completed", totalTokens: 42 },
];

describe("headless receipts (#9161 evidence)", () => {
  test("public receipt carries bounded facts and no raw text", () => {
    const { publicReceipt } = deriveHeadlessReceipts("turn-1", "thread-1", result(COMPLETED));
    expect(publicReceipt.dispatchOk).toBe(true);
    expect(publicReceipt.finishReason).toBe("completed");
    expect(publicReceipt.frameKinds).toEqual([
      "turn_started",
      "model_effective",
      "text_delta",
      "turn_completed",
    ]);
    expect(publicReceipt.totalTokens).toBe(42);
    expect(publicReceipt.fullAutoRecordCount).toBe(0);
    // The public receipt has no field carrying the answer text.
    expect(JSON.stringify(publicReceipt)).not.toContain("I am Codex");
  });

  test("private receipt keeps the ordered frames and the answer", () => {
    const { privateReceipt } = deriveHeadlessReceipts("turn-1", "thread-1", result(COMPLETED));
    expect(privateReceipt.answer).toBe("I am Codex.");
    expect(privateReceipt.frames).toHaveLength(4);
    expect(privateReceipt.public.finishReason).toBe("completed");
  });

  test("a clean completed ordinary turn screens pass", () => {
    expect(screenHeadlessTurn(result(COMPLETED)).disposition).toBe("pass");
  });

  test("an ordinary turn that created a Full Auto record trips the invariant", () => {
    const screen = screenHeadlessTurn(result(COMPLETED, { fullAutoRecordCount: 1 }));
    expect(screen.disposition).toBe("needs_review");
    expect(screen.tripwires).toContain("ordinary_turn_created_full_auto_record");
  });

  test("a completed turn with no answer trips", () => {
    const screen = screenHeadlessTurn(
      result([{ kind: "turn_started" }, { kind: "turn_completed", totalTokens: 0 }]),
    );
    expect(screen.tripwires).toContain("completed_turn_has_no_answer");
  });

  test("a failed turn screens fail (honest terminal)", () => {
    const screen = screenHeadlessTurn(
      result(
        [
          { kind: "turn_started" },
          { kind: "turn_failed", reason: "session_failed", detail: "down" },
        ],
        { dispatch: { ok: false, reason: "session_failed" } },
      ),
    );
    expect(screen.disposition).toBe("fail");
  });

  test("a delegated turn with route disclosed before the answer screens pass", () => {
    const screen = screenDelegatedTurn(
      result([
        { kind: "turn_started" },
        { kind: "child_started", childRef: "child-1", summary: "delegating to Codex" },
        {
          kind: "child_completed",
          childRef: "child-1",
          accountRef: "codex",
          summary: "done",
          usage: null,
          durationMs: 10,
        },
        { kind: "text_delta", text: "via Codex: done." },
        { kind: "turn_completed", totalTokens: 5 },
      ]),
    );
    expect(screen.disposition).toBe("pass");
  });

  test("a delegated turn that promotes the answer BEFORE the route trips (#9159)", () => {
    const screen = screenDelegatedTurn(
      result([
        { kind: "turn_started" },
        { kind: "text_delta", text: "Done — unrelated work." },
        { kind: "child_started", childRef: "child-1", summary: "hidden delegation" },
        { kind: "turn_completed", totalTokens: 5 },
      ]),
    );
    expect(screen.disposition).toBe("needs_review");
    expect(screen.tripwires).toContain("answer_promoted_before_route_disclosed");
  });

  test("a delegated turn missing the route frame trips", () => {
    const screen = screenDelegatedTurn(result(COMPLETED));
    expect(screen.tripwires).toContain("delegated_turn_missing_route_frame");
  });
});
