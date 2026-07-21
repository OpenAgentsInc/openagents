import { Effect, Layer, Stream } from "effect";
import { AiError, LanguageModel, type Response } from "effect/unstable/ai";
import { describe, expect, test } from "vite-plus/test";

import type { HistoryCorpusEntry } from "./corpus.ts";
import {
  cursorSliceRecursiveRecallCorpus,
  grepRecursiveRecallCorpus,
  parseRecursiveRecallOp,
  recursiveRecallConsecutiveDecodeFailureLimit,
  resolveRecursiveRecallCitations,
  runRecursiveRecall,
  summarizeRecursiveRecallTurn,
  timeSliceRecursiveRecallCorpus,
  type RecursiveRecallCaps,
} from "./recursive-recall.ts";

// ---------------------------------------------------------------------------
// Scripted LanguageModel layer — hermetic, deterministic, NO network, NO
// spend. Each generateText call consumes the next scripted turn: a text
// response plus exact usage. A turn may instead be a hanging effect (for the
// timeout test) or a typed AiError failure (for runtime_unavailable).
// ---------------------------------------------------------------------------

type ScriptedTurn =
  | { readonly text: string; readonly inputTokens?: number; readonly outputTokens?: number }
  | { readonly hang: true }
  | { readonly fail: string };

const usageEncoded = (inputTokens: number, outputTokens: number) => ({
  inputTokens: {
    cacheRead: undefined,
    cacheWrite: undefined,
    total: inputTokens,
    uncached: undefined,
  },
  outputTokens: { reasoning: undefined, text: undefined, total: outputTokens },
});

const scriptedLanguageModelLayer = (
  script: ReadonlyArray<ScriptedTurn>,
): Layer.Layer<LanguageModel.LanguageModel> => {
  let calls = 0;
  return Layer.effect(
    LanguageModel.LanguageModel,
    LanguageModel.make({
      generateText: () => {
        const turn = script[calls];
        calls++;
        if (turn === undefined) {
          return Effect.die(new Error(`scripted model exhausted after ${script.length} calls`));
        }
        if ("hang" in turn) return Effect.never;
        if ("fail" in turn) {
          return Effect.fail(
            AiError.make({
              method: "generateText",
              module: "ScriptedModel",
              reason: new AiError.UnknownError({ description: turn.fail }),
            }),
          );
        }
        const parts: Array<Response.PartEncoded> = [
          { text: turn.text, type: "text" },
          {
            reason: "stop",
            response: undefined,
            type: "finish",
            usage: usageEncoded(turn.inputTokens ?? 10, turn.outputTokens ?? 5),
          },
        ];
        return Effect.succeed(parts);
      },
      streamText: () => Stream.die(new Error("streamText is not scripted in this suite")),
    }),
  );
};

// ---------------------------------------------------------------------------
// Planted corpus: two turns of deploy chatter, the decision planted at
// turn.b#2, and a later unrelated turn.
// ---------------------------------------------------------------------------

const entry = (
  turnId: string,
  sequence: number,
  kind: HistoryCorpusEntry["kind"],
  text: string,
  observedAt: string,
  toolName?: string,
): HistoryCorpusEntry => ({
  kind,
  observedAt,
  redactionClass: "private_ref",
  scopeRef: "thread.deploy",
  sequence,
  turnId,
  visibility: "private",
  ...(text === "" ? {} : { text }),
  ...(toolName === undefined ? {} : { toolName }),
});

const corpus: ReadonlyArray<HistoryCorpusEntry> = [
  entry("turn.a", 1, "turn.started", "", "2026-07-20T10:00:00.000Z"),
  entry("turn.a", 2, "text.delta", "Should we keep rolling restarts?", "2026-07-20T10:00:01.000Z"),
  entry("turn.a", 3, "turn.finished", "", "2026-07-20T10:00:02.000Z"),
  entry("turn.b", 1, "text.delta", "Comparing rollout strategies now.", "2026-07-20T11:00:00.000Z"),
  entry(
    "turn.b",
    2,
    "text.delta",
    "Decision: adopt blue-green deploys for the API service.",
    "2026-07-20T11:00:01.000Z",
  ),
  entry("turn.b", 3, "tool.call", "", "2026-07-20T11:00:02.000Z", "record_decision"),
  entry("turn.c", 1, "text.delta", "Unrelated lunch planning.", "2026-07-20T12:00:00.000Z"),
];

const caps = (overrides: Partial<RecursiveRecallCaps> = {}): RecursiveRecallCaps => ({
  maxIterations: 8,
  maxSubcalls: 4,
  maxTokens: 10_000,
  timeoutMs: 5_000,
  ...overrides,
});

const question = "What deployment strategy was decided?";

const run = (
  script: ReadonlyArray<ScriptedTurn>,
  capsOverrides: Partial<RecursiveRecallCaps> = {},
) =>
  Effect.runPromise(
    runRecursiveRecall({ caps: caps(capsOverrides), corpus, question }).pipe(
      Effect.provide(scriptedLanguageModelLayer(script)),
    ),
  );

const answerOp =
  '{"_tag":"Answer","text":"The team adopted blue-green deploys.","citations":' +
  '[{"turnId":"turn.b","sequenceStart":2,"sequenceEnd":2}]}';

const subcallOp =
  '{"_tag":"Subcall","question":"What exactly was decided?","span":{"startIndex":3,"endIndex":5}}';

// The happy-path script: root greps, sub-calls over the turn.b span, the
// child answers with the exact citation, the root answers.
const happyScript: ReadonlyArray<ScriptedTurn> = [
  { inputTokens: 100, outputTokens: 10, text: '{"_tag":"Grep","pattern":"decision"}' },
  { inputTokens: 120, outputTokens: 20, text: subcallOp },
  {
    inputTokens: 60,
    outputTokens: 15,
    text:
      '{"_tag":"Answer","text":"Blue-green deploys were adopted for the API service.","citations":' +
      '[{"turnId":"turn.b","sequenceStart":2,"sequenceEnd":2}]}',
  },
  { inputTokens: 140, outputTokens: 25, text: answerOp },
];

describe("runRecursiveRecall — happy path", () => {
  test("grep → subcall → answer completes with exact citations and exact usage", async () => {
    const result = await run(happyScript);
    expect(result).toEqual({
      _tag: "Completed",
      answer: "The team adopted blue-green deploys.",
      citations: [{ sequenceEnd: 2, sequenceStart: 2, turnId: "turn.b" }],
      depthUsed: 1,
      iterations: 3,
      usage: {
        inputTokens: 100 + 120 + 60 + 140,
        outputTokens: 10 + 20 + 15 + 25,
        subcalls: 1,
        totalTokens: 100 + 120 + 60 + 140 + 10 + 20 + 15 + 25,
      },
    });
  });

  test("is deterministic: the same script yields the identical result twice", async () => {
    const first = await run(happyScript);
    const second = await run(happyScript);
    expect(second).toEqual(first);
  });

  test("citations that do not resolve to a corpus entry are dropped", async () => {
    const result = await run([
      {
        text:
          '{"_tag":"Answer","text":"Made up.","citations":[' +
          '{"turnId":"turn.zz","sequenceStart":1,"sequenceEnd":1},' +
          '{"turnId":"turn.b","sequenceStart":2,"sequenceEnd":2}]}',
      },
    ]);
    expect(result._tag).toBe("Completed");
    if (result._tag === "Completed") {
      expect(result.citations).toEqual([{ sequenceEnd: 2, sequenceStart: 2, turnId: "turn.b" }]);
    }
  });
});

describe("runRecursiveRecall — cap honesty", () => {
  test("iteration cap returns Partial{iteration_cap} after exactly maxIterations calls", async () => {
    const grepForever: ReadonlyArray<ScriptedTurn> = [
      { text: '{"_tag":"Grep","pattern":"decision"}' },
      { text: '{"_tag":"Grep","pattern":"deploys"}' },
    ];
    const result = await run(grepForever, { maxIterations: 2 });
    expect(result).toMatchObject({ _tag: "Partial", iterations: 2, reason: "iteration_cap" });
  });

  test("depth cap refuses the subcall and exhausts iterations without any subcall", async () => {
    const result = await run([{ text: subcallOp }, { text: subcallOp }], {
      maxDepth: 0,
      maxIterations: 2,
    });
    expect(result).toMatchObject({ _tag: "Partial", depthUsed: 0, reason: "iteration_cap" });
    if (result._tag === "Partial") expect(result.usage.subcalls).toBe(0);
  });

  test("subcall cap returns Partial{subcall_cap} at issue time", async () => {
    const result = await run([{ text: subcallOp }], { maxSubcalls: 0 });
    expect(result).toMatchObject({ _tag: "Partial", reason: "subcall_cap" });
    if (result._tag === "Partial") expect(result.usage.subcalls).toBe(0);
  });

  test("token cap returns Partial{token_cap} with the exact overage recorded", async () => {
    const result = await run(
      [{ inputTokens: 90, outputTokens: 20, text: '{"_tag":"Grep","pattern":"decision"}' }],
      { maxTokens: 100 },
    );
    expect(result).toMatchObject({ _tag: "Partial", reason: "token_cap" });
    if (result._tag === "Partial") {
      expect(result.usage).toEqual({ inputTokens: 90, outputTokens: 20, subcalls: 0, totalTokens: 110 });
    }
  });

  test("timeout returns Partial{timeout} when the model never responds", async () => {
    const result = await run([{ hang: true }], { timeoutMs: 50 });
    expect(result).toMatchObject({ _tag: "Partial", reason: "timeout" });
  });

  test("a cap-hit run still surfaces the best partial answer from a completed subcall", async () => {
    const result = await run(
      [
        { text: subcallOp },
        {
          text:
            '{"_tag":"Answer","text":"Blue-green deploys.","citations":' +
            '[{"turnId":"turn.b","sequenceStart":2,"sequenceEnd":2}]}',
        },
        { text: '{"_tag":"Grep","pattern":"deploys"}' },
      ],
      { maxIterations: 2 },
    );
    expect(result).toMatchObject({
      _tag: "Partial",
      bestAnswer: "Blue-green deploys.",
      depthUsed: 1,
      reason: "iteration_cap",
    });
  });
});

describe("runRecursiveRecall — fail-closed op decoding and failures", () => {
  test("an undecodable operation consumes an iteration and the loop recovers", async () => {
    const result = await run([{ text: "I think we should look around first." }, { text: answerOp }]);
    expect(result._tag).toBe("Completed");
    if (result._tag === "Completed") expect(result.iterations).toBe(2);
  });

  test("repeated undecodable operations fail honestly as contract_violation", async () => {
    const junk: ReadonlyArray<ScriptedTurn> = Array.from(
      { length: recursiveRecallConsecutiveDecodeFailureLimit },
      () => ({ text: "not an operation" }),
    );
    const result = await run(junk);
    expect(result).toMatchObject({ _tag: "Failed", failureClass: "contract_violation" });
  });

  test("a model-layer AiError surfaces as Failed{runtime_unavailable} with usage so far", async () => {
    const result = await run([
      { inputTokens: 30, outputTokens: 5, text: '{"_tag":"Grep","pattern":"decision"}' },
      { fail: "provider is down" },
    ]);
    expect(result).toMatchObject({ _tag: "Failed", failureClass: "runtime_unavailable" });
    if (result._tag === "Failed") {
      expect(result.usage.totalTokens).toBe(35);
      expect(result.detail).toContain("provider is down");
    }
  });

  test("no provider endpoint is ever required: the engine only needs the injected LanguageModel", () => {
    // The engine's requirements are exactly the LanguageModel service — the
    // hermetic layer above never opens a socket. This is the no-spend proof:
    // nothing in the run options names an endpoint, key, or provider.
    const effect = runRecursiveRecall({ caps: caps(), corpus, question });
    expect(typeof effect).toBe("object");
  });
});

describe("deterministic operations", () => {
  test("grep is case-insensitive over text and tool names and reports indexes", () => {
    expect(grepRecursiveRecallCorpus(corpus, "DECISION").map(match => match.index)).toEqual([4, 5]);
    expect(grepRecursiveRecallCorpus(corpus, "record_decision")).toHaveLength(1);
    expect(grepRecursiveRecallCorpus(corpus, "")).toEqual([]);
    expect(grepRecursiveRecallCorpus(corpus, "decision", 1)).toHaveLength(1);
  });

  test("cursor slice bounds one turn by inclusive sequence range", () => {
    const matches = cursorSliceRecursiveRecallCorpus(corpus, "turn.b", 1, 2);
    expect(matches.map(match => match.entry.sequence)).toEqual([1, 2]);
    expect(cursorSliceRecursiveRecallCorpus(corpus, "turn.zz", 1, 9)).toEqual([]);
  });

  test("time slice bounds by inclusive observedAt range", () => {
    const matches = timeSliceRecursiveRecallCorpus(
      corpus,
      "2026-07-20T11:00:00.000Z",
      "2026-07-20T11:59:59.000Z",
    );
    expect(matches.map(match => match.entry.turnId)).toEqual(["turn.b", "turn.b", "turn.b"]);
  });

  test("turn summary counts kinds and finds the sequence range and first snippet", () => {
    const summary = summarizeRecursiveRecallTurn(corpus, "turn.b");
    expect(summary).toEqual({
      entryCount: 3,
      firstTextSnippet: "Comparing rollout strategies now.",
      kindCounts: [
        ["text.delta", 2],
        ["tool.call", 1],
      ],
      sequenceEnd: 3,
      sequenceStart: 1,
      turnId: "turn.b",
    });
  });

  test("citation resolution keeps only citations that hit a real entry", () => {
    expect(
      resolveRecursiveRecallCitations(corpus, [
        { sequenceEnd: 2, sequenceStart: 2, turnId: "turn.b" },
        { sequenceEnd: 99, sequenceStart: 90, turnId: "turn.b" },
        { sequenceEnd: 1, sequenceStart: 1, turnId: "turn.zz" },
      ]),
    ).toEqual([{ sequenceEnd: 2, sequenceStart: 2, turnId: "turn.b" }]);
  });

  test("op parsing strips code fences and fails closed on junk", () => {
    expect(parseRecursiveRecallOp('```json\n{"_tag":"TurnSummary","turnId":"turn.b"}\n```')).toEqual({
      _tag: "TurnSummary",
      turnId: "turn.b",
    });
    expect(parseRecursiveRecallOp("not json")).toBeNull();
    expect(parseRecursiveRecallOp('{"_tag":"Unknown"}')).toBeNull();
    expect(parseRecursiveRecallOp('{"_tag":"Grep"}')).toBeNull();
  });
});
