import { Effect } from "effect";
import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  makeInMemoryEventLogStore,
  type HarnessEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import {
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeSource,
} from "@openagentsinc/agent-runtime-schema";
import { khalaRuntimeToolAuthorityFixture } from "@openagentsinc/agent-runtime-schema/fixtures";
import { describe, expect, test } from "vite-plus/test";

import { buildHistoryCorpus, type HistoryCorpusBuildResult } from "./builder.ts";
import { HistoryCorpusError, type HistoryCorpusPolicy } from "./corpus.ts";
import {
  HistoryRecallError,
  historyRecallDefaultCaps,
  type HistoryRecallCaps,
  type HistoryRecallQuestion,
} from "./recall.ts";
import {
  HistoryRecall,
  historyRecallTierDLayer,
  recallTierD,
  type HistoryRecallCorpusProvider,
} from "./recall-tier-d.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };
const BUILT_AT = "2026-07-21T12:00:00.000Z";
const THREAD_ID = "t1";
const TURN_COUNT = 300;
const PLANTED_TURN = 217;
const TOOL_TURN = 100;
const LONG_TURN = 250;
const PLANTED_TEXT = "DECISION: adopt the blue protocol";
const LONG_TEXT = `LONGSPAN ${"x".repeat(600)}`;

const ownerPolicy: HistoryCorpusPolicy = {
  includeVisibilities: ["public", "operator", "private"],
  includeRedactionClasses: ["public_ref", "redacted_summary", "operator_summary", "private_ref"],
};

const turnIdOf = (turn: number): string => `t1.turn-${String(turn).padStart(3, "0")}`;

/** Deterministic timestamps from a fixed epoch — never a wall clock read. */
const iso = (turn: number, seq: number): string =>
  new Date(Date.UTC(2026, 6, 1) + turn * 60_000 + seq * 1000).toISOString();

const run = Effect.runPromise;

/** One scripted turn: turn.started, one text.delta per word, turn.finished. */
const scriptTurn = (turn: number, words: ReadonlyArray<string>): Array<HarnessStreamEvent> => {
  const turnId = turnIdOf(turn);
  const events: Array<HarnessStreamEvent> = [];
  let seq = 0;
  events.push(
    buildTurnStarted({
      turnId,
      threadId: THREAD_ID,
      sequence: seq,
      source: SOURCE,
      observedAt: iso(turn, seq),
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
        observedAt: iso(turn, seq),
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
      observedAt: iso(turn, seq),
      finishReason: "stop",
    }),
  );
  return events;
};

const toolCallEvent = (turn: number, sequence: number, toolName: string): HarnessStreamEvent => {
  const turnId = turnIdOf(turn);
  return decodeKhalaRuntimeEvent({
    schema: KhalaRuntimeEventSchemaLiteral,
    eventId: `evt.${turnId}.${sequence}.tool`,
    turnId,
    threadId: THREAD_ID,
    sequence,
    observedAt: iso(turn, sequence),
    source: SOURCE,
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: [],
    kind: "tool.call",
    toolCallId: `tool_call.${turnId}.${sequence}`,
    toolName,
    authority: khalaRuntimeToolAuthorityFixture,
  });
};

interface DeepFixture {
  readonly store: HarnessEventLogStore;
  readonly turnIds: ReadonlyArray<string>;
  readonly corpus: HistoryCorpusBuildResult;
}

/**
 * 300 turns on one thread, with the decision text planted at turn 217
 * sequence 2, a tool.call turn at 100, and a 600-plus-character text at 250.
 */
const buildDeepFixture = async (): Promise<DeepFixture> => {
  const store = makeInMemoryEventLogStore();
  const turnIds: Array<string> = [];
  const events: Array<HarnessStreamEvent> = [];
  for (let turn = 0; turn < TURN_COUNT; turn++) {
    turnIds.push(turnIdOf(turn));
    if (turn === PLANTED_TURN) {
      events.push(...scriptTurn(turn, ["alpha", PLANTED_TEXT, "beta"]));
    } else if (turn === TOOL_TURN) {
      const turnId = turnIdOf(turn);
      events.push(
        buildTurnStarted({
          turnId,
          threadId: THREAD_ID,
          sequence: 0,
          source: SOURCE,
          observedAt: iso(turn, 0),
        }),
        toolCallEvent(turn, 1, "workspaceRead"),
        buildTextDelta({
          turnId,
          threadId: THREAD_ID,
          sequence: 2,
          source: SOURCE,
          observedAt: iso(turn, 2),
          messageId: `msg.${turnId}`,
          text: "ran the tool",
        }),
        buildTurnFinished({
          turnId,
          threadId: THREAD_ID,
          sequence: 3,
          source: SOURCE,
          observedAt: iso(turn, 3),
          finishReason: "stop",
        }),
      );
    } else if (turn === LONG_TURN) {
      events.push(...scriptTurn(turn, [LONG_TEXT]));
    } else {
      events.push(...scriptTurn(turn, ["alpha", "beta"]));
    }
  }
  await run(Effect.forEach(events, (event) => store.append(event)));
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

let cachedFixture: Promise<DeepFixture> | undefined;
const deepFixture = (): Promise<DeepFixture> => (cachedFixture ??= buildDeepFixture());

const recallDeep = async (question: HistoryRecallQuestion, caps?: HistoryRecallCaps) => {
  const { corpus } = await deepFixture();
  return run(
    recallTierD({
      entries: corpus.entries,
      coverageNote: corpus.manifest.coverage.note,
      question,
      caps,
    }),
  );
};

describe("recallTierD — Grep", () => {
  test("finds a decision planted hundreds of turns deep with its exact cursor span", async () => {
    const { corpus } = await deepFixture();
    const response = await recallDeep({ _tag: "Grep", pattern: "DECISION: adopt" });

    expect(response.answers.length).toBe(1);
    expect(response.answers[0]).toEqual({
      scopeRef: THREAD_ID,
      turnId: turnIdOf(PLANTED_TURN),
      sequenceStart: 2,
      sequenceEnd: 2,
      excerpt: PLANTED_TEXT,
      kind: "text.delta",
    });
    expect(response.honesty.tier).toBe("deterministic");
    expect(response.honesty.entriesScanned).toBe(corpus.entries.length);
    expect(response.honesty.entriesTotal).toBe(corpus.entries.length);
    expect(response.honesty.truncated).toBe(false);
    expect(response.honesty.capsHit).toEqual([]);
    expect(response.cost.modelCalls).toBe(0);
  });

  test("is case-insensitive by default and case-sensitive on request", async () => {
    const insensitive = await recallDeep({ _tag: "Grep", pattern: "decision: adopt" });
    expect(insensitive.answers.length).toBe(1);

    const sensitive = await recallDeep({
      _tag: "Grep",
      pattern: "decision: adopt",
      caseSensitive: true,
    });
    expect(sensitive.answers.length).toBe(0);
    expect(sensitive.honesty.truncated).toBe(false);
  });

  test("an invalid pattern is a typed invalid_pattern error, never a crash", async () => {
    const { corpus } = await deepFixture();
    const error = await run(
      recallTierD({
        entries: corpus.entries,
        coverageNote: corpus.manifest.coverage.note,
        question: { _tag: "Grep", pattern: "(unclosed" },
      }).pipe(Effect.flip),
    );
    expect(error).toBeInstanceOf(HistoryRecallError);
    expect(error.reason).toBe("invalid_pattern");
  });
});

describe("recallTierD — CursorSlice", () => {
  test("returns exactly the inclusive sequence range of one turn", async () => {
    const response = await recallDeep({
      _tag: "CursorSlice",
      turnId: turnIdOf(PLANTED_TURN),
      fromSequence: 1,
      toSequence: 3,
    });
    expect(response.answers.map((span) => span.sequenceStart)).toEqual([1, 2, 3]);
    for (const span of response.answers) {
      expect(span.turnId).toBe(turnIdOf(PLANTED_TURN));
      expect(span.sequenceEnd).toBe(span.sequenceStart);
    }
    expect(response.answers[1]!.excerpt).toBe(PLANTED_TEXT);
    expect(response.honesty.truncated).toBe(false);
  });
});

describe("recallTierD — TimeSlice", () => {
  test("returns exactly the inclusive observedAt range, boundaries included", async () => {
    const response = await recallDeep({
      _tag: "TimeSlice",
      fromObservedAt: iso(PLANTED_TURN, 0),
      toObservedAt: iso(PLANTED_TURN, 4),
    });
    expect(response.answers.length).toBe(5);
    expect(response.answers.map((span) => span.sequenceStart)).toEqual([0, 1, 2, 3, 4]);
    for (const span of response.answers) {
      expect(span.turnId).toBe(turnIdOf(PLANTED_TURN));
    }
    expect(response.honesty.truncated).toBe(false);
  });
});

describe("recallTierD — KeyTurns", () => {
  test("returns the first limit turns with their full cursor span and first/last text", async () => {
    const response = await recallDeep({ _tag: "KeyTurns", limit: 3 });
    expect(response.answers.length).toBe(3);
    expect(response.answers.map((span) => span.turnId)).toEqual([
      turnIdOf(0),
      turnIdOf(1),
      turnIdOf(2),
    ]);
    for (const span of response.answers) {
      expect(span.sequenceStart).toBe(0);
      expect(span.sequenceEnd).toBe(3);
      expect(span.excerpt).toBe("alpha ... beta");
      expect(span.kind).toBe("turn.started");
    }
    // The limit is part of the question, not a cap: no truncation reported.
    expect(response.honesty.truncated).toBe(false);
    expect(response.honesty.capsHit).toEqual([]);
  });
});

describe("recallTierD — TurnSummary", () => {
  test("summarizes one turn structurally: counts by kind, tools, texts, cursor span", async () => {
    const response = await recallDeep({ _tag: "TurnSummary", turnId: turnIdOf(TOOL_TURN) });
    expect(response.answers.length).toBe(1);
    const span = response.answers[0]!;
    expect(span.turnId).toBe(turnIdOf(TOOL_TURN));
    expect(span.sequenceStart).toBe(0);
    expect(span.sequenceEnd).toBe(3);
    expect(span.kind).toBe("turn.started");
    expect(span.excerpt).toContain("entries=4");
    expect(span.excerpt).toContain("cursor=0..3");
    expect(span.excerpt).toContain("tool.call=1");
    expect(span.excerpt).toContain("text.delta=1");
    expect(span.excerpt).toContain("turn.started=1");
    expect(span.excerpt).toContain("turn.finished=1");
    expect(span.excerpt).toContain("tools[workspaceRead]");
    expect(span.excerpt).toContain('first="ran the tool"');
    expect(response.honesty.truncated).toBe(false);
  });

  test("an unknown turn yields zero answers after an honest full scan", async () => {
    const { corpus } = await deepFixture();
    const response = await recallDeep({ _tag: "TurnSummary", turnId: "no-such-turn" });
    expect(response.answers).toEqual([]);
    expect(response.honesty.entriesScanned).toBe(corpus.entries.length);
    expect(response.honesty.truncated).toBe(false);
  });
});

describe("recallTierD — cap enforcement and honesty", () => {
  test("maxEntriesScanned stops the scan and honesty names the cap, across cap values", async () => {
    const { corpus } = await deepFixture();
    const total = corpus.entries.length;
    for (const cap of [1, 5, 137, total, total + 1000]) {
      const response = await recallDeep(
        { _tag: "Grep", pattern: "zzz-no-such-text" },
        { maxEntriesScanned: cap },
      );
      expect(response.honesty.entriesScanned).toBe(Math.min(cap, total));
      expect(response.honesty.entriesTotal).toBe(total);
      const expectTruncated = cap < total;
      expect(response.honesty.truncated).toBe(expectTruncated);
      expect(response.honesty.capsHit).toEqual(expectTruncated ? ["maxEntriesScanned"] : []);
    }
  });

  test("maxSpans caps the answers and honesty names the cap", async () => {
    // Every turn matches sequence 0, so 300 candidate spans exist.
    const question: HistoryRecallQuestion = {
      _tag: "CursorSlice",
      fromSequence: 0,
      toSequence: 0,
    };
    const capped = await recallDeep(question, { maxSpans: 7 });
    expect(capped.answers.length).toBe(7);
    expect(capped.answers.map((span) => span.turnId)).toEqual(
      Array.from({ length: 7 }, (_, i) => turnIdOf(i)),
    );
    expect(capped.honesty.truncated).toBe(true);
    expect(capped.honesty.capsHit).toEqual(["maxSpans"]);

    const defaulted = await recallDeep(question);
    expect(defaulted.answers.length).toBe(historyRecallDefaultCaps.maxSpans);
    expect(defaulted.honesty.capsHit).toEqual(["maxSpans"]);
  });

  test("maxSpans also bounds KeyTurns output", async () => {
    const response = await recallDeep({ _tag: "KeyTurns", limit: 20 }, { maxSpans: 4 });
    expect(response.answers.length).toBe(4);
    expect(response.honesty.truncated).toBe(true);
    expect(response.honesty.capsHit).toEqual(["maxSpans"]);
  });

  test("maxCharsPerSpan truncates the excerpt and honesty names the cap", async () => {
    const defaulted = await recallDeep({ _tag: "Grep", pattern: "LONGSPAN" });
    expect(defaulted.answers.length).toBe(1);
    expect(defaulted.answers[0]!.excerpt.length).toBe(historyRecallDefaultCaps.maxCharsPerSpan);
    expect(defaulted.honesty.truncated).toBe(true);
    expect(defaulted.honesty.capsHit).toEqual(["maxCharsPerSpan"]);

    const tight = await recallDeep({ _tag: "Grep", pattern: "LONGSPAN" }, { maxCharsPerSpan: 100 });
    expect(tight.answers[0]!.excerpt.length).toBe(100);
    expect(tight.answers[0]!.excerpt).toBe(LONG_TEXT.slice(0, 100));
    expect(tight.honesty.capsHit).toEqual(["maxCharsPerSpan"]);
  });

  test("honesty carries the corpus manifest coverage note through", async () => {
    const { corpus } = await deepFixture();
    const response = await recallDeep({ _tag: "Grep", pattern: "alpha" }, { maxSpans: 1 });
    expect(response.honesty.coverageNote).toBe(corpus.manifest.coverage.note);
    expect(response.honesty.coverageNote).toContain("seven core kinds");
  });

  test("every question kind answers with zero model calls", async () => {
    const questions: ReadonlyArray<HistoryRecallQuestion> = [
      { _tag: "Grep", pattern: "alpha" },
      { _tag: "CursorSlice", fromSequence: 0, toSequence: 1 },
      { _tag: "TimeSlice", fromObservedAt: iso(0, 0), toObservedAt: iso(1, 0) },
      { _tag: "KeyTurns", limit: 2 },
      { _tag: "TurnSummary", turnId: turnIdOf(0) },
    ];
    for (const question of questions) {
      const response = await recallDeep(question, { maxSpans: 3 });
      expect(response.cost.modelCalls).toBe(0);
      expect(response.honesty.tier).toBe("deterministic");
    }
  });
});

describe("HistoryRecall service — Tier D layer", () => {
  test("a Scope request resolves through the corpus provider and matches the prebuilt path", async () => {
    const { store, turnIds, corpus } = await deepFixture();
    const provider: HistoryRecallCorpusProvider = {
      corpusForScope: (scope) =>
        buildHistoryCorpus({
          scope,
          eventLog: store,
          turnIds,
          policy: ownerPolicy,
          builtAt: BUILT_AT,
        }),
    };
    const question: HistoryRecallQuestion = { _tag: "Grep", pattern: "DECISION: adopt" };

    const viaScope = await run(
      Effect.gen(function* () {
        const recall = yield* HistoryRecall;
        return yield* recall.recall({
          corpus: { _tag: "Scope", scope: { _tag: "Thread", threadId: THREAD_ID } },
          question,
        });
      }).pipe(Effect.provide(historyRecallTierDLayer(provider))),
    );
    const viaPrebuilt = await run(
      Effect.gen(function* () {
        const recall = yield* HistoryRecall;
        return yield* recall.recall({
          corpus: { _tag: "Corpus", manifest: corpus.manifest, entries: corpus.entries },
          question,
        });
      }).pipe(Effect.provide(historyRecallTierDLayer(provider))),
    );

    expect(viaScope).toEqual(viaPrebuilt);
    expect(viaScope.answers[0]!.turnId).toBe(turnIdOf(PLANTED_TURN));
    expect(viaScope.honesty.coverageNote).toBe(corpus.manifest.coverage.note);
    expect(viaScope.cost.modelCalls).toBe(0);
  });

  test("a failing corpus provider is a typed corpus_unavailable error", async () => {
    const failing: HistoryRecallCorpusProvider = {
      corpusForScope: () =>
        Effect.fail(new HistoryCorpusError({ operation: "read_event_log", detail: "offline" })),
    };
    const error = await run(
      Effect.gen(function* () {
        const recall = yield* HistoryRecall;
        return yield* recall.recall({
          corpus: { _tag: "Scope", scope: { _tag: "Thread", threadId: THREAD_ID } },
          question: { _tag: "KeyTurns", limit: 1 },
        });
      }).pipe(Effect.provide(historyRecallTierDLayer(failing)), Effect.flip),
    );
    expect(error).toBeInstanceOf(HistoryRecallError);
    expect(error.reason).toBe("corpus_unavailable");
  });
});
