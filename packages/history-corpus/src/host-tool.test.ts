import { Effect } from "effect";
import {
  HISTORY_RECALL_TOOL_NAME,
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  historyRecallHostToolSpec,
  makeInMemoryEventLogStore,
  type HarnessEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import {
  buildHistoryCorpus,
  type HistoryCorpusBuildResult,
} from "./builder.ts";
import type { HistoryCorpusPolicy } from "./corpus.ts";
import {
  dispatchHistoryRecallHostTool,
  historyRecallRequestFromHostParams,
  historyRecallToolWireSpec,
  historyRecallWireNameMatchesRegistration,
  HistoryRecallTool,
  HistoryRecallToolkit,
  resolveHistoryRecallHostToolCall,
  summarizeHistoryRecallAnswer,
} from "./host-tool.ts";
import { makeHistoryRecallTierD } from "./recall-tier-d.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };
const BUILT_AT = "2026-07-21T12:00:00.000Z";
const THREAD_ID = "thread.host-tool";
const PLANTED = "DECISION: adopt the blue protocol";

const ownerPolicy: HistoryCorpusPolicy = {
  includeVisibilities: ["public", "operator", "private"],
  includeRedactionClasses: ["public_ref", "redacted_summary", "operator_summary", "private_ref"],
};

const run = Effect.runPromise;

const scriptTurn = (
  turnId: string,
  words: ReadonlyArray<string>,
): Array<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = [];
  let seq = 0;
  events.push(
    buildTurnStarted({
      turnId,
      threadId: THREAD_ID,
      sequence: seq,
      source: SOURCE,
      observedAt: "2026-07-21T10:00:00.000Z",
    }),
  );
  seq += 1;
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId: THREAD_ID,
        sequence: seq,
        source: SOURCE,
        observedAt: "2026-07-21T10:00:01.000Z",
        messageId: `msg.${turnId}`,
        text: word,
      }),
    );
    seq += 1;
  }
  events.push(
    buildTurnFinished({
      turnId,
      threadId: THREAD_ID,
      sequence: seq,
      source: SOURCE,
      observedAt: "2026-07-21T10:00:02.000Z",
      finishReason: "stop",
    }),
  );
  return events;
};

const buildFixture = async (): Promise<{
  readonly store: HarnessEventLogStore;
  readonly turnIds: ReadonlyArray<string>;
  readonly corpus: HistoryCorpusBuildResult;
}> => {
  const store = makeInMemoryEventLogStore();
  const turnIds = ["turn.a", "turn.b", "turn.c"];
  const scripts = [
    scriptTurn("turn.a", ["hello", "world"]),
    scriptTurn("turn.b", [PLANTED]),
    scriptTurn("turn.c", ["later", "note"]),
  ];
  for (const events of scripts) {
    for (const event of events) {
      await run(store.append(event));
    }
  }
  const corpus = await run(
    buildHistoryCorpus({
      scope: { _tag: "Thread", threadId: THREAD_ID },
      eventLog: store,
      turnIds,
      policy: ownerPolicy,
      builtAt: BUILT_AT,
    }),
  );
  return { store, turnIds, corpus };
};

describe("history_recall host tool (RLM-03)", () => {
  test("Effect Tool projects to the registered wire name", () => {
    expect(HistoryRecallTool.name).toBe(HISTORY_RECALL_TOOL_NAME);
    expect(historyRecallToolWireSpec.name).toBe(historyRecallHostToolSpec.name);
    expect(historyRecallWireNameMatchesRegistration).toBe(true);
    expect(HistoryRecallToolkit.tools.history_recall).toBeDefined();
  });

  test("request builder always uses Scope corpus input (no model-supplied corpus)", () => {
    const request = historyRecallRequestFromHostParams({
      scope: { _tag: "Thread", threadId: THREAD_ID },
      question: { _tag: "Grep", pattern: "DECISION" },
      caps: { maxSpans: 3 },
    });
    expect(request.corpus).toEqual({
      _tag: "Scope",
      scope: { _tag: "Thread", threadId: THREAD_ID },
    });
    expect(request.question).toEqual({ _tag: "Grep", pattern: "DECISION" });
    expect(request.caps).toEqual({ maxSpans: 3 });
  });

  test("dispatcher resolves a Grep call through HistoryRecall and returns cited spans", async () => {
    const fixture = await buildFixture();
    const recall = makeHistoryRecallTierD({
      corpusForScope: () => Effect.succeed(fixture.corpus),
    });
    const result = await run(
      resolveHistoryRecallHostToolCall({
        recall,
        call: {
          toolCallId: "toolcall.hr.1",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "DECISION: adopt" },
            caps: { maxSpans: 10 },
          },
        },
      }),
    );
    expect(result.isError).toBeUndefined();
    expect(result.toolCallId).toBe("toolcall.hr.1");
    const output = result.output as {
      answers: ReadonlyArray<{ excerpt: string; turnId: string }>;
      honesty: { tier: string; truncated: boolean; modelCalls?: number };
      cost: { modelCalls: number };
    };
    expect(output.answers.length).toBeGreaterThan(0);
    expect(output.answers.some((span) => span.excerpt.includes("DECISION"))).toBe(true);
    expect(output.honesty.tier).toBe("deterministic");
    expect(output.cost.modelCalls).toBe(0);
  });

  test("invalid params produce isError, not a defect", async () => {
    const fixture = await buildFixture();
    const recall = makeHistoryRecallTierD({
      corpusForScope: () => Effect.succeed(fixture.corpus),
    });
    const result = await run(
      resolveHistoryRecallHostToolCall({
        recall,
        call: {
          toolCallId: "toolcall.hr.bad",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: { not: "valid" },
        },
      }),
    );
    expect(result.isError).toBe(true);
    expect(result.toolCallId).toBe("toolcall.hr.bad");
  });

  test("unknown tool name produces isError", async () => {
    const fixture = await buildFixture();
    const recall = makeHistoryRecallTierD({
      corpusForScope: () => Effect.succeed(fixture.corpus),
    });
    const result = await run(
      resolveHistoryRecallHostToolCall({
        recall,
        call: {
          toolCallId: "toolcall.other",
          toolName: "not_history_recall",
          input: {},
        },
      }),
    );
    expect(result.isError).toBe(true);
    expect((result.output as { error: string }).error).toBe("unknown_host_tool");
  });

  test("dispatch emits neutral tool.call + tool.result re-entry", async () => {
    const fixture = await buildFixture();
    const recall = makeHistoryRecallTierD({
      corpusForScope: () => Effect.succeed(fixture.corpus),
    });
    const dispatched = await run(
      dispatchHistoryRecallHostTool({
        recall,
        call: {
          toolCallId: "toolcall.hr.stream",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "blue protocol" },
          },
        },
        stream: {
          turnId: "turn.dispatch",
          threadId: THREAD_ID,
          source: SOURCE,
          sequence: 10,
          toolCallId: "toolcall.hr.stream",
          observedAt: "2026-07-21T11:00:00.000Z",
        },
      }),
    );
    expect(dispatched.result.isError).toBeUndefined();
    expect(dispatched.answer).not.toBeNull();
    expect(dispatched.neutralEvents.map((e) => e.kind)).toEqual([
      "tool.call",
      "tool.result",
    ]);
    const call = dispatched.neutralEvents[0]!;
    const result = dispatched.neutralEvents[1]!;
    expect(call.kind).toBe("tool.call");
    if (call.kind === "tool.call") {
      expect(call.toolName).toBe(HISTORY_RECALL_TOOL_NAME);
      expect(call.toolCallId).toBe("toolcall.hr.stream");
      expect(call.sequence).toBe(10);
      expect(call.authority.allowed).toBe(true);
    }
    expect(result.kind).toBe("tool.result");
    if (result.kind === "tool.result") {
      expect(result.toolName).toBe(HISTORY_RECALL_TOOL_NAME);
      expect(result.providerExecuted).toBe(false);
      expect(result.resultRef).toContain("toolcall.hr.stream");
      expect(result.sequence).toBe(11);
    }
    // Payload does not ride the neutral event — only resultRef.
    expect(JSON.stringify(dispatched.neutralEvents)).not.toContain(PLANTED);
  });

  test("summarizeHistoryRecallAnswer is bounded and cites cursors", async () => {
    const fixture = await buildFixture();
    const recall = makeHistoryRecallTierD({
      corpusForScope: () => Effect.succeed(fixture.corpus),
    });
    const result = await run(
      resolveHistoryRecallHostToolCall({
        recall,
        call: {
          toolCallId: "toolcall.hr.sum",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "DECISION" },
          },
        },
      }),
    );
    const answer = result.output as Parameters<typeof summarizeHistoryRecallAnswer>[0];
    const summary = summarizeHistoryRecallAnswer(answer);
    expect(summary).toContain("history_recall");
    expect(summary).toContain("span");
    expect(summary).toContain("complete");
    expect(summary.length).toBeLessThan(2_000);
  });

  test("error path emits tool.call + tool.error on the neutral stream", async () => {
    const okFixture = await buildFixture();
    const tierD = makeHistoryRecallTierD({
      corpusForScope: () => Effect.succeed(okFixture.corpus),
    });
    const dispatched = await run(
      dispatchHistoryRecallHostTool({
        recall: tierD,
        call: {
          toolCallId: "toolcall.hr.err",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "(" },
          },
        },
        stream: {
          turnId: "turn.err",
          threadId: THREAD_ID,
          source: SOURCE,
          sequence: 0,
          toolCallId: "toolcall.hr.err",
        },
      }),
    );
    expect(dispatched.result.isError).toBe(true);
    expect(dispatched.answer).toBeNull();
    expect(dispatched.neutralEvents.map((e) => e.kind)).toEqual([
      "tool.call",
      "tool.error",
    ]);
  });
});
