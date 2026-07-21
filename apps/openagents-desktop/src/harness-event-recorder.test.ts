import { describe, expect, test } from "vite-plus/test";

import type { ClaudeLocalEvent } from "./claude-local-contract.ts";
import { makeHarnessEventRecorder } from "./harness-event-recorder.ts";

const TURN = "turn.full-auto.rec1";

const feed = (
  recorder: ReturnType<typeof makeHarnessEventRecorder>,
  events: ReadonlyArray<ClaudeLocalEvent>,
) => {
  for (const event of events) {
    recorder.observe({ threadRef: "thread.a", turnRef: TURN, graphLaneRef: "claude_local", event });
  }
};

describe("HARN-03/06 harness event recorder", () => {
  test("records a turn's neutral event log and exposes cursor-exact liveness", () => {
    const recorder = makeHarnessEventRecorder();
    feed(recorder, [
      { kind: "turn_started" },
      { kind: "text_delta", text: "Hello " },
      { kind: "tool_use", toolName: "Bash", summary: "run" },
      { kind: "tool_result", toolName: "Bash", ok: true, summary: "ok" },
      { kind: "text_delta", text: "done" },
      { kind: "turn_completed", totalTokens: 12 },
    ]);

    const liveness = recorder.liveness(TURN);
    expect(liveness).toBeDefined();
    // 6 core events projected -> cursor 5, last kind turn.finished.
    expect(liveness!.cursor).toBe(5);
    expect(liveness!.eventCount).toBe(6);
    expect(liveness!.lastEventKind).toBe("turn.finished");

    // The recorded log replays as a contiguous neutral stream.
    const replayed = recorder.replay(TURN);
    expect(replayed.map((e) => e.sequence)).toEqual([0, 1, 2, 3, 4, 5]);
    expect(replayed.map((e) => e.kind)).toEqual([
      "turn.started",
      "text.delta",
      "tool.call",
      "tool.result",
      "text.delta",
      "turn.finished",
    ]);
  });

  test("display-only events do not advance the cursor", () => {
    const recorder = makeHarnessEventRecorder();
    feed(recorder, [
      { kind: "turn_started" },
      { kind: "plan_updated", entries: [] },
      { kind: "meter_updated", outputTokens: 3 },
      { kind: "text_delta", text: "hi" },
    ]);
    const liveness = recorder.liveness(TURN);
    // Only turn_started + text_delta produced neutral events -> cursor 1, count 2.
    expect(liveness!.cursor).toBe(1);
    expect(liveness!.eventCount).toBe(2);
  });

  test("an unseen turn has no liveness", () => {
    const recorder = makeHarnessEventRecorder();
    expect(recorder.liveness("turn.unknown")).toBeUndefined();
    expect(recorder.replay("turn.unknown")).toEqual([]);
  });
});
