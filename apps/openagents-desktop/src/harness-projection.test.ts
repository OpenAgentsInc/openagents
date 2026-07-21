import {
  decodeKhalaRuntimeEvent,
  type KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import type { ClaudeLocalEvent } from "./claude-local-contract.ts";
import {
  makeClaudeLocalHarnessProjector,
  type HarnessProjectionContext,
} from "./harness-projection.ts";

const SOURCE: KhalaRuntimeSource = { lane: "claude_pylon", adapterKind: "claude_code" };

const CTX: HarnessProjectionContext = {
  turnId: "turn.full-auto.abc123",
  threadId: "thread.xyz",
  source: SOURCE,
};

const projectAll = (events: ReadonlyArray<ClaudeLocalEvent>) => {
  const project = makeClaudeLocalHarnessProjector(CTX);
  return events.flatMap((e) => project(e));
};

describe("HARN-03 ClaudeLocalEvent -> KhalaRuntimeEvent projection", () => {
  test("projects a core turn to a contiguous, valid neutral stream", () => {
    const out = projectAll([
      { kind: "turn_started" },
      { kind: "text_delta", text: "Hello " },
      { kind: "reasoning", text: "thinking" },
      { kind: "text_delta", text: "world" },
      { kind: "turn_completed", totalTokens: 42 },
    ]);

    // Every projected event is a valid KhalaRuntimeEvent.
    for (const e of out) {
      expect(() => decodeKhalaRuntimeEvent(e)).not.toThrow();
    }
    // Kinds map correctly and sequences are contiguous from 0.
    expect(out.map((e) => e.kind)).toEqual([
      "turn.started",
      "text.delta",
      "reasoning.delta",
      "text.delta",
      "turn.finished",
    ]);
    expect(out.map((e) => e.sequence)).toEqual([0, 1, 2, 3, 4]);
  });

  test("carries usage from turn_completed into turn.finished", () => {
    const out = projectAll([
      { kind: "turn_started" },
      {
        kind: "turn_completed",
        totalTokens: 100,
        usage: { inputTokens: 60, outputTokens: 30, reasoningTokens: 10 } as never,
      },
    ]);
    const finished = out.find((e) => e.kind === "turn.finished");
    expect(finished).toBeDefined();
    // @ts-expect-error narrow at runtime
    expect(finished.usage?.totalTokens).toBe(100);
    // @ts-expect-error narrow at runtime
    expect(finished.usage?.inputTokens).toBe(60);
  });

  test("normalizes tool names and emits an owner-local authority", () => {
    const out = projectAll([
      { kind: "tool_use", toolName: "Bash", summary: "run tests" },
      { kind: "tool_result", toolName: "Bash", ok: true, summary: "passed" },
    ]);
    const call = out.find((e) => e.kind === "tool.call");
    const result = out.find((e) => e.kind === "tool.result");
    expect(call).toBeDefined();
    expect(result).toBeDefined();
    // Bash normalizes to the common name "bash".
    // @ts-expect-error narrow at runtime
    expect(call.toolName).toBe("bash");
    // @ts-expect-error narrow at runtime
    expect(call.authority.allowed).toBe(true);
    // @ts-expect-error narrow at runtime
    expect(result.providerExecuted).toBe(true);
  });

  test("turn_failed projects to turn.interrupted", () => {
    const out = projectAll([{ kind: "turn_failed", reason: "timeout", detail: "slow" }]);
    expect(out.map((e) => e.kind)).toEqual(["turn.interrupted"]);
  });

  test("desktop-display-only events project to nothing (they stay on the renderer envelope)", () => {
    const out = projectAll([
      { kind: "plan_updated", entries: [] },
      { kind: "meter_updated", outputTokens: 5 },
      { kind: "lane_notice", severity: "info", message: "rotated account" } as never,
    ]);
    expect(out).toEqual([]);
  });

  test("a fresh projector can start from a non-zero sequence for a continued turn", () => {
    const project = makeClaudeLocalHarnessProjector({ ...CTX, startSequence: 10 });
    const out = project({ kind: "text_delta", text: "resumed" });
    expect(out[0]?.sequence).toBe(10);
  });
});
