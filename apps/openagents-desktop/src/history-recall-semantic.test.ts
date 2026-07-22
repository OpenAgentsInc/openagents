/**
 * RLM-05 (#9141) — hermetic fixture eval for Tier S semantic recall.
 *
 * Everything here is deterministic and spend-free: the "model" is a scripted
 * root/leaf plan that emits fixed RlmProgram JSON and fixed token counts.
 * The suite plants decisions at 100/400/1000-turn depths and compares three
 * paths over the SAME corpus:
 *
 * - the bounded 12-message window baseline (misses the planted decision),
 * - Tier D deterministic recall (finds it, zero model calls),
 * - Tier S through the first-class `@openagentsinc/rlm` engine (finds it,
 *   cited against the exact corpus digest, exact usage rows recorded).
 */

import { Effect } from "effect";
import {
  HISTORY_RECALL_TOOL_NAME,
  makeInMemoryEventLogStore,
  type HarnessEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
} from "@openagentsinc/agent-harness-contract";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import type { HistoryCorpusPolicy } from "@openagentsinc/history-corpus";
import { makeRlm } from "@openagentsinc/rlm";
import { describe, expect, test } from "vite-plus/test";

import {
  resolveDesktopHistoryCorpus,
  toDesktopHistoryCorpusSourceInput,
  type HistoryRecallHostSources,
} from "./history-recall-host.ts";
import { desktopHistoryCorpusSourceLayer } from "./desktop-history-corpus-source.ts";
import {
  citedSpansFromSemanticResult,
  clampDesktopSemanticBudget,
  desktopRlmSemanticBudgetCeilings,
  desktopSemanticRlmRequest,
  deterministicRecallInsufficient,
  dispatchDesktopHistoryRecallTiered,
  makeCountedDesktopRlmModelPlan,
  makeDesktopRlmUsageRecorder,
  rlmUsageRowKey,
  runDesktopRlmSemanticRecall,
  selectDesktopRecallTier,
  semanticTerminalSummaryFromResult,
  usageLedgerInputsFromRlmUsage,
  type DesktopRlmCompleteFn,
  type DesktopSemanticRecallAdmission,
  type DesktopSemanticRecallProgress,
} from "./history-recall-semantic.ts";
import { makeUsageLedger } from "./usage-ledger.ts";
import { projectSemanticRecallToolCard } from "./renderer/history-recall-semantic-card.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };
const THREAD_ID = "thread.desktop-hr-s";
const PLANTED = "DECISION: adopt tier-s semantic recall";
const PLANTED_TURN = "turn.8";
const ADMISSION: DesktopSemanticRecallAdmission = {
  admitted: true,
  basis: "user_explicit",
  grantRef: "grant.test.semantic.1",
};

// ---------------------------------------------------------------------------
// Fixture: N turns with one planted decision early (turn.8).
// ---------------------------------------------------------------------------

interface Fixture {
  readonly sources: HistoryRecallHostSources;
  /** Every text message in corpus order (for the bounded-window baseline). */
  readonly messages: ReadonlyArray<string>;
}

const turnEvents = (turnId: string, words: ReadonlyArray<string>): Array<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = [];
  let seq = 0;
  events.push(
    buildTurnStarted({
      turnId,
      threadId: THREAD_ID,
      sequence: seq++,
      source: SOURCE,
      observedAt: "2026-07-21T09:00:00.000Z",
    }),
  );
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId: THREAD_ID,
        sequence: seq++,
        source: SOURCE,
        observedAt: "2026-07-21T09:00:01.000Z",
        messageId: `msg.${turnId}`,
        text: word,
      }),
    );
  }
  events.push(
    buildTurnFinished({
      turnId,
      threadId: THREAD_ID,
      sequence: seq,
      source: SOURCE,
      observedAt: "2026-07-21T09:00:02.000Z",
      finishReason: "stop",
    }),
  );
  return events;
};

const makeFixture = async (turnCount: number): Promise<Fixture> => {
  const eventLog: HarnessEventLogStore = makeInMemoryEventLogStore();
  const turnIds: Array<string> = [];
  const messages: Array<string> = [];
  await Effect.runPromise(
    Effect.gen(function* () {
      for (let i = 1; i <= turnCount; i++) {
        const turnId = `turn.${i}`;
        turnIds.push(turnId);
        const words =
          turnId === PLANTED_TURN
            ? [PLANTED, "confirmed by the owner"]
            : [`routine update ${i}`, `nothing decided in step ${i}`];
        messages.push(...words);
        for (const event of turnEvents(turnId, words)) {
          yield* eventLog.append(event);
        }
      }
    }),
  );
  return {
    sources: {
      eventLog,
      turnIdsForThread: (threadId) => (threadId === THREAD_ID ? turnIds : []),
      builtAt: () => "2026-07-21T12:00:00.000Z",
      source: SOURCE,
    },
    messages,
  };
};

// ---------------------------------------------------------------------------
// Scripted model plans (no spend, fixed tokens).
// ---------------------------------------------------------------------------

/** Root that emits a Grep→Commit program citing the planted decision. */
const grepCommitRoot = (options?: {
  readonly pattern?: string;
  readonly tokens?: { readonly input: number; readonly output: number } | null;
}): DesktopRlmCompleteFn => {
  const pattern = options?.pattern ?? "DECISION:";
  const tokens = options?.tokens === undefined ? { input: 100, output: 20 } : options.tokens;
  return () =>
    Effect.succeed({
      text: JSON.stringify({
        schemaId: "openagents.ai.rlm_program.v1",
        programRef: "program.grep-commit",
        nodes: [
          {
            _tag: "CorpusOp",
            nodeRef: "n1",
            operator: "Grep",
            params: { pattern },
            inputValueRefs: [],
            outputValueRef: "v1",
          },
          { _tag: "Commit", nodeRef: "n2", valueRef: "v1", citationValueRefs: [] },
        ],
      }),
      ...(tokens === null ? {} : { inputTokens: tokens.input, outputTokens: tokens.output }),
    });
};

/** Root that fans grep hits through a leaf ModelMap before committing. */
const grepModelMapRoot = (): DesktopRlmCompleteFn => () =>
  Effect.succeed({
    text: JSON.stringify({
      schemaId: "openagents.ai.rlm_program.v1",
      programRef: "program.grep-map-commit",
      nodes: [
        {
          _tag: "CorpusOp",
          nodeRef: "n1",
          operator: "Grep",
          params: { pattern: "DECISION:" },
          inputValueRefs: [],
          outputValueRef: "v1",
        },
        {
          _tag: "ModelMap",
          nodeRef: "n2",
          inputCollectionRef: "v1",
          promptTemplate: "Label this span: {{item}}",
          outputValueRef: "v2",
          maxConcurrency: 1,
        },
        { _tag: "Commit", nodeRef: "n3", valueRef: "v2", citationValueRefs: ["v1"] },
      ],
    }),
    inputTokens: 100,
    outputTokens: 20,
  });

/** Root that commits an uncited value (InspectMetadata carries no citations). */
const uncitedCommitRoot = (): DesktopRlmCompleteFn => () =>
  Effect.succeed({
    text: JSON.stringify({
      schemaId: "openagents.ai.rlm_program.v1",
      programRef: "program.uncited-commit",
      nodes: [
        {
          _tag: "CorpusOp",
          nodeRef: "n1",
          operator: "InspectMetadata",
          params: {},
          inputValueRefs: [],
          outputValueRef: "v1",
        },
        { _tag: "Commit", nodeRef: "n2", valueRef: "v1", citationValueRefs: [] },
      ],
    }),
    inputTokens: 40,
    outputTokens: 10,
  });

const scriptedLeaf = (): DesktopRlmCompleteFn => (prompt) =>
  Effect.succeed({
    text: `labelled: ${prompt.slice(0, 40)}`,
    inputTokens: 5,
    outputTokens: 2,
  });

// ---------------------------------------------------------------------------
// Tier policy — admission is host-owned, model args admit nothing.
// ---------------------------------------------------------------------------

describe("tier selection policy (RLM-05)", () => {
  test("deterministic by default; semantic needs explicit escalation AND admission", () => {
    expect(selectDesktopRecallTier({}).tier).toBe("deterministic");
    expect(selectDesktopRecallTier({ request: { requestedTier: "deterministic" } }).tier).toBe(
      "deterministic",
    );
    // Explicit request without admission is a typed refusal, not a run.
    expect(selectDesktopRecallTier({ request: { requestedTier: "semantic" } })).toEqual({
      tier: "semantic_refused",
      reason: "not_admitted",
    });
    // Explicit request with admission runs semantic.
    const explicit = selectDesktopRecallTier({
      request: { requestedTier: "semantic" },
      admission: ADMISSION,
    });
    expect(explicit.tier).toBe("semantic");
    if (explicit.tier === "semantic") expect(explicit.basis).toBe("explicit_request");
  });

  test("insufficient Tier D escalates only with caller opt-in plus admission", () => {
    const emptyResponse = {
      answers: [],
      honesty: {
        tier: "deterministic" as const,
        entriesScanned: 10,
        entriesTotal: 10,
        truncated: false,
        capsHit: [],
        coverageNote: "note",
      },
      cost: { modelCalls: 0 },
    };
    expect(deterministicRecallInsufficient(emptyResponse)).toBe(true);
    // Opt-in without admission refuses.
    expect(
      selectDesktopRecallTier({
        request: { escalateOnInsufficient: true },
        deterministicResponse: emptyResponse,
      }),
    ).toEqual({ tier: "semantic_refused", reason: "not_admitted" });
    // Opt-in with admission escalates.
    const escalated = selectDesktopRecallTier({
      request: { escalateOnInsufficient: true },
      admission: ADMISSION,
      deterministicResponse: emptyResponse,
    });
    expect(escalated.tier).toBe("semantic");
    if (escalated.tier === "semantic") {
      expect(escalated.basis).toBe("insufficient_deterministic");
    }
    // A cited answer does not auto-escalate.
    const cited = {
      ...emptyResponse,
      answers: [
        {
          scopeRef: "thread:x",
          turnId: "turn.1",
          sequenceStart: 1,
          sequenceEnd: 1,
          excerpt: "hit",
          kind: "text.delta" as const,
        },
      ],
    };
    expect(
      selectDesktopRecallTier({
        request: { escalateOnInsufficient: true },
        admission: ADMISSION,
        deterministicResponse: cited,
      }).tier,
    ).toBe("deterministic");
  });

  test("budget clamps downward to finite ceilings; depth at most one; exact usage forced", () => {
    expect(desktopRlmSemanticBudgetCeilings.maxDepth).toBe(1);
    expect(desktopRlmSemanticBudgetCeilings.requireExactUsage).toBe(true);
    expect(desktopRlmSemanticBudgetCeilings.maxArtifactOutputBytes).toBe(0);
    const clamped = clampDesktopSemanticBudget({
      maxDepth: 5,
      maxModelCalls: 10_000,
      maxIterationsPerLoop: 999,
      timeoutMs: 999_999_999,
      requireExactUsage: false,
      maxArtifactOutputBytes: 1_000_000,
    });
    expect(clamped.maxDepth).toBe(1);
    expect(clamped.maxModelCalls).toBe(desktopRlmSemanticBudgetCeilings.maxModelCalls);
    expect(clamped.maxIterationsPerLoop).toBe(
      desktopRlmSemanticBudgetCeilings.maxIterationsPerLoop,
    );
    expect(clamped.timeoutMs).toBe(desktopRlmSemanticBudgetCeilings.timeoutMs);
    expect(clamped.requireExactUsage).toBe(true);
    expect(clamped.maxArtifactOutputBytes).toBe(0);
    // Narrowing below the ceiling is allowed.
    expect(clampDesktopSemanticBudget({ maxModelCalls: 2 }).maxModelCalls).toBe(2);
  });
});

// ---------------------------------------------------------------------------
// Fixture eval — planted decisions at 100/400/1000 turns.
// ---------------------------------------------------------------------------

describe("planted-decision fixture eval — baseline vs Tier D vs Tier S", () => {
  for (const turnCount of [100, 400, 1000]) {
    test(`${turnCount} turns: window misses, Tier D cites, Tier S cites against the exact digest`, async () => {
      const fixture = await makeFixture(turnCount);

      // Bounded 12-message window baseline: the planted decision is gone.
      const window = fixture.messages.slice(-12);
      expect(window.some((message) => message.includes(PLANTED))).toBe(false);

      // Tier D + tier policy through the host-tool seam (semantic requested).
      const recorder = makeDesktopRlmUsageRecorder();
      const outcome = await Effect.runPromise(
        dispatchDesktopHistoryRecallTiered(fixture.sources, {
          call: {
            toolCallId: `toolcall.s.${turnCount}`,
            toolName: HISTORY_RECALL_TOOL_NAME,
            input: {
              scope: { _tag: "Thread", threadId: THREAD_ID },
              question: { _tag: "Grep", pattern: "DECISION:" },
              caps: { maxSpans: 5 },
            },
          },
          turnId: "turn.active",
          threadId: THREAD_ID,
          sequence: 0,
          tierRequest: { requestedTier: "semantic" },
          admission: ADMISSION,
          semantic: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: "What decision was made about semantic recall?",
            runRef: `run.s.${turnCount}`,
            completeRoot: grepCommitRoot(),
            recorder,
          },
        }),
      );

      // Tier D found and cited the planted decision.
      expect(outcome.deterministic.result.isError).toBeUndefined();
      const deterministicSpans = outcome.deterministic.citedSpans;
      expect(deterministicSpans.some((span) => span.turnId === PLANTED_TURN)).toBe(true);
      expect(outcome.deterministic.answer?.cost.modelCalls).toBe(0);

      // Tier S completed with citations resolving to the exact corpus digest.
      expect(outcome.decision.tier).toBe("semantic");
      expect(outcome.semantic?._tag).toBe("result");
      if (outcome.semantic?._tag !== "result") return;
      const result = outcome.semantic.result;
      expect(result._tag).toBe("Completed");
      if (result._tag !== "Completed") return;

      const handle = await Effect.runPromise(
        resolveDesktopHistoryCorpus(toDesktopHistoryCorpusSourceInput(fixture.sources), {
          _tag: "Thread",
          threadId: THREAD_ID,
        }),
      );
      expect(result.run.contentDigest).toBe(handle.identity.contentDigest);
      expect(result.citations.length).toBeGreaterThan(0);
      for (const citation of result.citations) {
        expect(citation.contentDigest).toBe(handle.identity.contentDigest);
      }
      expect(result.honesty.citationInvalid).toBe(0);
      expect(result.honesty.strategyRef).toBe("openagents.desktop.rlm.history.v1");

      // The cited spans decode to the planted turn and match Tier D's rows.
      const semanticSpans = outcome.semantic.citedSpans;
      expect(semanticSpans.some((span) => span.turnId === PLANTED_TURN)).toBe(true);
      const planted = semanticSpans.find((span) => span.turnId === PLANTED_TURN);
      expect(planted?.excerpt).toContain(PLANTED);
      const deterministicPlanted = deterministicSpans.find((span) => span.turnId === PLANTED_TURN);
      expect(planted?.sequenceStart).toBe(deterministicPlanted?.sequenceStart);

      // Cost/latency honesty: exact usage rows for exactly one root call.
      expect(outcome.semantic.usageRows.map((row) => row.key)).toEqual([
        rlmUsageRowKey(`run.s.${turnCount}`, "root.1"),
      ]);
      expect(outcome.semantic.summary.state).toBe("completed");
      expect(outcome.semantic.summary.modelCalls).toBe(1);
      expect(outcome.semantic.summary.totalTokens).toBe(120);
      expect(outcome.semantic.summary.usageCompleteness).toBe("complete");
    }, 30_000);
  }
});

// ---------------------------------------------------------------------------
// Admission enforcement.
// ---------------------------------------------------------------------------

describe("semantic admission enforcement", () => {
  test("model tool arguments cannot self-authorize Tier S", async () => {
    const fixture = await makeFixture(20);
    const recorder = makeDesktopRlmUsageRecorder();
    const outcome = await Effect.runPromise(
      dispatchDesktopHistoryRecallTiered(fixture.sources, {
        call: {
          toolCallId: "toolcall.self-auth",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "DECISION:" },
            // Model-shaped self-authorization attempts — never consulted.
            tier: "semantic",
            mode: "semantic",
          },
        },
        turnId: "turn.active",
        threadId: THREAD_ID,
        sequence: 0,
        // No host tier request and no admission.
        semantic: {
          scope: { _tag: "Thread", threadId: THREAD_ID },
          question: "planted?",
          runRef: "run.self-auth",
          completeRoot: grepCommitRoot(),
          recorder,
        },
      }),
    );
    expect(outcome.decision.tier).toBe("deterministic");
    expect(outcome.semantic).toBeNull();
    expect(recorder.rows()).toEqual([]);
  });

  test("without admission the engine returns the typed Refused terminal and spends nothing", async () => {
    const fixture = await makeFixture(20);
    const recorder = makeDesktopRlmUsageRecorder();
    const plan = makeCountedDesktopRlmModelPlan({
      runRef: "run.refused",
      completeRoot: grepCommitRoot(),
      recorder,
    });
    const result = await Effect.runPromise(
      runDesktopRlmSemanticRecall(fixture.sources, {
        scope: { _tag: "Thread", threadId: THREAD_ID },
        question: "planted?",
        runRef: "run.refused",
        admission: null,
        plan,
      }),
    );
    expect(result._tag).toBe("Refused");
    if (result._tag === "Refused") expect(result.reason).toBe("semantic_not_admitted");
    expect(result.usage.modelCalls).toBe(0);
    expect(recorder.rows()).toEqual([]);
    // Distinct renderer state: refused, no cited spans, no fabricated usage.
    const card = projectSemanticRecallToolCard({
      toolCallId: "toolcall.refused",
      phase: "terminal",
      terminal: semanticTerminalSummaryFromResult(result, recorder.totals()),
    });
    expect(card.state).toBe("refused");
    expect(card.citedSpans).toEqual([]);
    expect(card.headline).toContain("refused");
  });

  test("insufficient Tier D plus caller opt-in escalates to a semantic run", async () => {
    const fixture = await makeFixture(30);
    const recorder = makeDesktopRlmUsageRecorder();
    const outcome = await Effect.runPromise(
      dispatchDesktopHistoryRecallTiered(fixture.sources, {
        call: {
          toolCallId: "toolcall.escalate",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            // No lexical hit — deterministic recall is insufficient.
            question: { _tag: "Grep", pattern: "ZZZ_NO_SUCH_TOKEN" },
          },
        },
        turnId: "turn.active",
        threadId: THREAD_ID,
        sequence: 0,
        tierRequest: { escalateOnInsufficient: true },
        admission: ADMISSION,
        semantic: {
          scope: { _tag: "Thread", threadId: THREAD_ID },
          question: "What decision was made?",
          runRef: "run.escalate",
          completeRoot: grepCommitRoot(),
          recorder,
        },
      }),
    );
    expect(outcome.deterministic.answer?.answers.length).toBe(0);
    expect(outcome.decision.tier).toBe("semantic");
    if (outcome.decision.tier === "semantic") {
      expect(outcome.decision.basis).toBe("insufficient_deterministic");
    }
    expect(outcome.semantic?._tag).toBe("result");
    if (outcome.semantic?._tag === "result") {
      expect(outcome.semantic.result._tag).toBe("Completed");
      expect(outcome.semantic.citedSpans.some((span) => span.turnId === PLANTED_TURN)).toBe(true);
    }
  });
});

// ---------------------------------------------------------------------------
// Exact usage — idempotent per-call rows, ledger projection, typed refusals.
// ---------------------------------------------------------------------------

describe("exact usage ledger (RLM-05)", () => {
  test("root and leaf calls record idempotent rlm:<runRef>:<callRef> rows", async () => {
    const fixture = await makeFixture(40);
    const recorder = makeDesktopRlmUsageRecorder();
    const plan = makeCountedDesktopRlmModelPlan({
      runRef: "run.map",
      completeRoot: grepModelMapRoot(),
      completeLeaf: scriptedLeaf(),
      recorder,
    });
    const result = await Effect.runPromise(
      runDesktopRlmSemanticRecall(fixture.sources, {
        scope: { _tag: "Thread", threadId: THREAD_ID },
        question: "label the planted decision",
        runRef: "run.map",
        admission: ADMISSION,
        plan,
      }),
    );
    expect(result._tag).toBe("Completed");
    // One grep hit fans through the leaf map exactly once.
    expect(recorder.rows().map((row) => row.key)).toEqual([
      "rlm:run.map:root.1",
      "rlm:run.map:leaf.1",
    ]);
    expect(result.usage.modelCalls).toBe(2);
    const totals = recorder.totals();
    expect(totals).toEqual({
      modelCalls: 2,
      inputTokens: 105,
      outputTokens: 22,
      totalTokens: 127,
    });
    // Idempotency: a replayed row cannot double-count.
    expect(
      recorder.record({
        runRef: "run.map",
        callRef: "leaf.1",
        role: "leaf",
        inputTokens: 5,
        outputTokens: 2,
      }),
    ).toBe(false);
    expect(recorder.totals().totalTokens).toBe(127);
    // The summary reconciles engine call counts with exact recorder totals.
    const summary = semanticTerminalSummaryFromResult(result, recorder.totals());
    expect(summary.usageCompleteness).toBe("complete");
    expect(summary.totalTokens).toBe(127);
  });

  test("exact rows project into the existing session usage ledger", () => {
    const recorder = makeDesktopRlmUsageRecorder();
    recorder.record({
      runRef: "run.ledger",
      callRef: "root.1",
      role: "root",
      inputTokens: 100,
      outputTokens: 20,
    });
    recorder.record({
      runRef: "run.ledger",
      callRef: "leaf.1",
      role: "leaf",
      inputTokens: 5,
      outputTokens: 2,
    });
    const inputs = usageLedgerInputsFromRlmUsage({
      rows: recorder.rows(),
      provider: "codex",
      accountRef: "codex",
      requestedModel: "gpt-5.5",
    });
    expect(inputs).toHaveLength(2);
    const ledger = makeUsageLedger(() => new Date("2026-07-21T12:00:00.000Z"));
    for (const input of inputs) ledger.record(input);
    const row = ledger.snapshot().rows.find((r) => r.accountRef === "codex");
    expect(row?.children).toBe(2);
    expect(row?.inputTokens).toBe(105);
    expect(row?.outputTokens).toBe(22);
    expect(row?.totalTokens).toBe(127);
    ledger.dispose();
  });

  test("missing exact usage fails typed — unavailable, never zero", async () => {
    const fixture = await makeFixture(20);
    const recorder = makeDesktopRlmUsageRecorder();
    const outcome = await Effect.runPromise(
      dispatchDesktopHistoryRecallTiered(fixture.sources, {
        call: {
          toolCallId: "toolcall.no-usage",
          toolName: HISTORY_RECALL_TOOL_NAME,
          input: {
            scope: { _tag: "Thread", threadId: THREAD_ID },
            question: { _tag: "Grep", pattern: "DECISION:" },
          },
        },
        turnId: "turn.active",
        threadId: THREAD_ID,
        sequence: 0,
        tierRequest: { requestedTier: "semantic" },
        admission: ADMISSION,
        semantic: {
          scope: { _tag: "Thread", threadId: THREAD_ID },
          question: "planted?",
          runRef: "run.no-usage",
          // Provider returns no token counts at all.
          completeRoot: grepCommitRoot({ tokens: null }),
          recorder,
        },
      }),
    );
    // Tier D evidence is unaffected; the semantic outcome is a typed failure.
    expect(outcome.deterministic.result.isError).toBeUndefined();
    expect(outcome.semantic?._tag).toBe("failure");
    if (outcome.semantic?._tag === "failure") {
      expect(outcome.semantic.reason).toBe("usage_required_but_unavailable");
      // No fabricated zero rows exist for the unusable call.
      expect(outcome.semantic.usageRows).toEqual([]);
    }
    const card = projectSemanticRecallToolCard({
      toolCallId: "toolcall.no-usage",
      phase: "failed",
      failureReason: "usage_required_but_unavailable",
    });
    expect(card.state).toBe("failed");
    expect(card.usageLine).toBe("usage unavailable");
  });
});

// ---------------------------------------------------------------------------
// Citation integrity, redaction, progress/replay.
// ---------------------------------------------------------------------------

describe("citation integrity and honesty (RLM-05)", () => {
  test("an uncited semantic answer cannot render as completed", async () => {
    const fixture = await makeFixture(20);
    const recorder = makeDesktopRlmUsageRecorder();
    const plan = makeCountedDesktopRlmModelPlan({
      runRef: "run.uncited",
      completeRoot: uncitedCommitRoot(),
      recorder,
    });
    const result = await Effect.runPromise(
      runDesktopRlmSemanticRecall(fixture.sources, {
        scope: { _tag: "Thread", threadId: THREAD_ID },
        question: "planted?",
        runRef: "run.uncited",
        admission: ADMISSION,
        plan,
      }),
    );
    // requireCitations + minimumCitations force Partial, never Completed.
    expect(result._tag).toBe("Partial");
    if (result._tag === "Partial") expect(result.reason).toBe("invalid_citations");
    const card = projectSemanticRecallToolCard({
      toolCallId: "toolcall.uncited",
      phase: "terminal",
      terminal: semanticTerminalSummaryFromResult(result, recorder.totals()),
      citedSpans: citedSpansFromSemanticResult(result),
    });
    expect(card.state).toBe("partial");
    expect(card.headline).toContain("partial");
    expect(card.headline).toContain("cited candidate");
  });

  test("citations with foreign address schemas are not navigable", () => {
    const foreign = {
      _tag: "Completed" as const,
      run: {
        runRef: "run.x",
        depth: 0,
        iterations: 1,
        corpusRef: "corpus.x",
        contentDigest: "digest.x",
      },
      output: {
        _tag: "InlineValue" as const,
        value: "answer",
        valueRef: "v1",
        digest: "d",
      },
      citations: [
        {
          corpusRef: "corpus.x",
          contentDigest: "digest.x",
          scopeRef: "thread:x",
          sourcePlane: "event_log" as const,
          sourceAddress: {
            addressSchemaId: "some.other.scheme.v1",
            encodedAddress: "opaque",
          },
          sourceOrigin: {
            sourcePlane: "event_log" as const,
            sourceKind: "text.delta",
            sourceAddress: {
              addressSchemaId: "some.other.scheme.v1",
              encodedAddress: "opaque",
            },
            corpusRef: "corpus.x",
            contentDigest: "digest.x",
            entryRef: "turn.1#1",
          },
          supportingSources: [],
          entryRefStart: "turn.1#1",
        },
      ],
      usage: {
        completeness: "unavailable" as const,
        modelCalls: 1,
        subcalls: 0,
      },
      honesty: {
        capsHit: [],
        usageCompleteness: "unavailable" as const,
        citationValidated: 1,
        citationInvalid: 0,
        programNodes: 1,
        valuesPublished: 1,
        modelMapCalls: 0,
        rlmMapCalls: 0,
      },
    };
    expect(citedSpansFromSemanticResult(foreign)).toEqual([]);
  });

  test("the corpus mount honors visibility policy through the Tier S path", async () => {
    const fixture = await makeFixture(12);
    // Host policy for this run: only public events are admitted. Every
    // builder fixture event is private, so the semantic engine must see an
    // EMPTY corpus with counted exclusions — the secret is unreachable.
    const restrictedPolicy: HistoryCorpusPolicy = {
      includeVisibilities: ["public"],
      includeRedactionClasses: ["public_ref"],
    };
    const sourceInput = {
      ...toDesktopHistoryCorpusSourceInput(fixture.sources),
      policy: restrictedPolicy,
    };
    const handle = await Effect.runPromise(
      resolveDesktopHistoryCorpus(sourceInput, {
        _tag: "Thread",
        threadId: THREAD_ID,
      }),
    );
    expect(handle.manifest.coverage.entryCount).toBe(0);
    expect(
      handle.manifest.coverage.exclusions.some(
        (exclusion) => exclusion.reason === "excluded_by_visibility" && exclusion.count > 0,
      ),
    ).toBe(true);

    const recorder = makeDesktopRlmUsageRecorder();
    const plan = makeCountedDesktopRlmModelPlan({
      runRef: "run.redacted",
      completeRoot: grepCommitRoot(),
      recorder,
    });
    const rlm = await Effect.runPromise(
      makeRlm({ admitSemantic: true, model: plan }).pipe(
        Effect.provide(desktopHistoryCorpusSourceLayer(sourceInput)),
      ),
    );
    const result = await Effect.runPromise(
      rlm.run(
        desktopSemanticRlmRequest({
          scope: { _tag: "Thread", threadId: THREAD_ID },
          question: "planted?",
          runRef: "run.redacted",
        }),
      ),
    );
    // Nothing excluded by policy can be cited or surfaced.
    if (result._tag === "Refused") throw new Error("unexpected refusal");
    expect(result.citations).toEqual([]);
    const output = result._tag === "Completed" ? result.output : result.bestOutput;
    if (output !== undefined && output._tag === "InlineValue") {
      expect(output.value).not.toContain(PLANTED);
    }
  });

  test("progress rows are transient; replay renders from the bounded terminal alone", async () => {
    const fixture = await makeFixture(60);
    const recorder = makeDesktopRlmUsageRecorder();
    const plan = makeCountedDesktopRlmModelPlan({
      runRef: "run.progress",
      completeRoot: grepCommitRoot(),
      recorder,
    });
    const progress: Array<DesktopSemanticRecallProgress> = [];
    const result = await Effect.runPromise(
      runDesktopRlmSemanticRecall(fixture.sources, {
        scope: { _tag: "Thread", threadId: THREAD_ID },
        question: "planted?",
        runRef: "run.progress",
        admission: ADMISSION,
        plan,
        onProgress: (row) => progress.push(row),
      }),
    );
    expect(result._tag).toBe("Completed");
    expect(progress.length).toBeGreaterThan(0);
    expect(progress[0]?.label).toContain("run started");
    expect(progress.every((row) => row.runRef === "run.progress")).toBe(true);
    // Replay path: the card is rebuilt with ONLY the bounded terminal data.
    const card = projectSemanticRecallToolCard({
      toolCallId: "toolcall.replay",
      phase: "terminal",
      terminal: semanticTerminalSummaryFromResult(result, recorder.totals()),
      citedSpans: citedSpansFromSemanticResult(result),
    });
    expect(card.state).toBe("completed");
    expect(card.headline).toContain("cited candidate");
    expect(card.headline).toContain("not verified");
    expect(card.citedSpansLine).toContain(PLANTED_TURN);
    expect(card.usageLine).toContain("model calls");
  });
});
