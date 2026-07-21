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
import { describe, expect, test } from "vite-plus/test";

import {
  buildHistoryCorpus,
  corpusEntriesToJsonl,
  type BuildHistoryCorpusInput,
  type NeutralThreadSnapshot,
} from "./builder.ts";
import { HistoryCorpusError, type HistoryCorpusPolicy } from "./corpus.ts";

const SOURCE: KhalaRuntimeSource = { lane: "test_fixture" };
const BUILT_AT = "2026-07-21T12:00:00.000Z";

const ownerPolicy: HistoryCorpusPolicy = {
  includeVisibilities: ["public", "operator", "private"],
  includeRedactionClasses: ["public_ref", "redacted_summary", "operator_summary", "private_ref"],
};

const publicPolicy: HistoryCorpusPolicy = {
  includeVisibilities: ["public"],
  includeRedactionClasses: ["public_ref"],
};

/** One scripted turn: turn.started, N text.delta, turn.finished (all private/private_ref). */
const scriptTurn = (
  turnId: string,
  threadId: string,
  words: ReadonlyArray<string>,
): ReadonlyArray<HarnessStreamEvent> => {
  const events: Array<HarnessStreamEvent> = [];
  let seq = 0;
  events.push(buildTurnStarted({ turnId, threadId, sequence: seq++, source: SOURCE }));
  for (const word of words) {
    events.push(
      buildTextDelta({
        turnId,
        threadId,
        sequence: seq++,
        source: SOURCE,
        messageId: `msg.${turnId}`,
        text: word,
      }),
    );
  }
  events.push(
    buildTurnFinished({ turnId, threadId, sequence: seq++, source: SOURCE, finishReason: "stop" }),
  );
  return events;
};

/** A text.delta with explicit visibility/redaction, for filter-split proofs. */
const classifiedTextDelta = (params: {
  readonly turnId: string;
  readonly threadId: string;
  readonly sequence: number;
  readonly text: string;
  readonly visibility: "public" | "operator" | "private";
  readonly redactionClass: "public_ref" | "redacted_summary" | "operator_summary" | "private_ref";
}): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    schema: KhalaRuntimeEventSchemaLiteral,
    eventId: `evt.${params.turnId}.${params.sequence}.text`,
    turnId: params.turnId,
    threadId: params.threadId,
    sequence: params.sequence,
    observedAt: "2026-07-21T00:00:00.000Z",
    source: SOURCE,
    visibility: params.visibility,
    redactionClass: params.redactionClass,
    causalityRefs: [],
    kind: "text.delta",
    messageId: `msg.${params.turnId}`,
    chunkId: `chunk.${params.turnId}.${params.sequence}`,
    text: params.text,
  });

const seedStore = (
  store: HarnessEventLogStore,
  events: ReadonlyArray<HarnessStreamEvent>,
): Effect.Effect<void, unknown> => Effect.forEach(events, (event) => store.append(event));

const run = Effect.runPromise;

describe("buildHistoryCorpus — determinism", () => {
  test("identical inputs build a deep-equal corpus, independent of turn-id input order", async () => {
    const store = makeInMemoryEventLogStore();
    await run(
      Effect.gen(function* () {
        yield* seedStore(store, scriptTurn("turn-a", "t1", ["one", "two"]));
        yield* seedStore(store, scriptTurn("turn-b", "t1", ["three"]));
      }),
    );
    const threads: ReadonlyArray<NeutralThreadSnapshot> = [
      {
        id: "t1",
        title: "fixture thread",
        updatedAt: "2026-07-21T00:00:05.000Z",
        notes: [
          { key: "m1", role: "user", text: "start", timestamp: "2026-07-21T00:00:01.000Z" },
          { key: "m2", role: "assistant", text: "done", timestamp: "2026-07-21T00:00:02.000Z" },
        ],
      },
    ];
    const input = (turnIds: ReadonlyArray<string>): BuildHistoryCorpusInput => ({
      scope: { _tag: "Thread", threadId: "t1" },
      eventLog: store,
      turnIds,
      threads,
      policy: ownerPolicy,
      builtAt: BUILT_AT,
    });

    const first = await run(buildHistoryCorpus(input(["turn-a", "turn-b"])));
    const second = await run(buildHistoryCorpus(input(["turn-b", "turn-a"])));
    expect(second).toEqual(first);
    expect(first.manifest.builtAt).toBe(BUILT_AT);
    expect(first.manifest.corpusRef).toBe(`corpus.thread.t1.${BUILT_AT}`);
    expect(first.manifest.entryCount).toBe(first.entries.length);
    expect(first.manifest.byteLength).toBe(
      new TextEncoder().encode(corpusEntriesToJsonl(first.entries)).length,
    );
  });
});

describe("buildHistoryCorpus — cursor addressing", () => {
  test("every event entry's (turnId, sequence) round-trips to the source store", async () => {
    const store = makeInMemoryEventLogStore();
    await run(seedStore(store, scriptTurn("turn-a", "t1", ["alpha", "beta", "gamma"])));

    const { entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        eventLog: store,
        turnIds: ["turn-a"],
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }),
    );
    expect(entries.length).toBe(5);

    for (const entry of entries) {
      const tail = await run(store.read({ turnId: entry.turnId, fromCursor: entry.sequence - 1 }));
      const sourceEvent = tail[0];
      expect(sourceEvent).toBeDefined();
      if (sourceEvent === undefined) continue;
      expect(sourceEvent.sequence).toBe(entry.sequence);
      expect(sourceEvent.kind).toBe(entry.kind);
      expect(sourceEvent.threadId).toBe(entry.scopeRef);
      if (sourceEvent.kind === "text.delta") {
        expect(entry.text).toBe(sourceEvent.text);
      }
    }
  });
});

describe("buildHistoryCorpus — redaction and visibility exclusion", () => {
  test("private_ref events are excluded under a public policy and counted, never silently dropped", async () => {
    const store = makeInMemoryEventLogStore();
    await run(
      seedStore(store, [
        classifiedTextDelta({
          turnId: "turn-mixed",
          threadId: "t1",
          sequence: 0,
          text: "public safe",
          visibility: "public",
          redactionClass: "public_ref",
        }),
        // Passes visibility, fails redaction class → excludedByRedaction.
        classifiedTextDelta({
          turnId: "turn-mixed",
          threadId: "t1",
          sequence: 1,
          text: "public visibility, private ref",
          visibility: "public",
          redactionClass: "private_ref",
        }),
        // Fails visibility outright → excludedByVisibility.
        classifiedTextDelta({
          turnId: "turn-mixed",
          threadId: "t1",
          sequence: 2,
          text: "owner private",
          visibility: "private",
          redactionClass: "private_ref",
        }),
      ]),
    );

    const { manifest, entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        eventLog: store,
        turnIds: ["turn-mixed"],
        policy: publicPolicy,
        builtAt: BUILT_AT,
      }),
    );

    expect(entries.length).toBe(1);
    expect(entries[0]!.text).toBe("public safe");
    expect(entries.some((entry) => entry.text?.includes("private"))).toBe(false);
    expect(manifest.exclusions.excludedByVisibility).toBe(1);
    expect(manifest.exclusions.excludedByRedaction).toBe(1);
    expect(manifest.exclusions.policy).toEqual(publicPolicy);
    expect(manifest.entryCount).toBe(1);
  });

  test("thread notes are private/private_ref and a public policy excludes and counts them", async () => {
    const { manifest, entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        threads: [
          {
            id: "t1",
            title: "fixture",
            updatedAt: "2026-07-21T00:00:05.000Z",
            notes: [
              {
                key: "m1",
                role: "user",
                text: "raw owner text",
                timestamp: "2026-07-21T00:00:01.000Z",
              },
            ],
          },
        ],
        policy: publicPolicy,
        builtAt: BUILT_AT,
      }),
    );
    expect(entries.length).toBe(0);
    expect(manifest.exclusions.excludedByVisibility).toBe(1);
    expect(manifest.exclusions.excludedByRedaction).toBe(0);
  });
});

describe("buildHistoryCorpus — empty scope", () => {
  test("no sources yields an empty, honest corpus", async () => {
    const { manifest, entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "ThreadSet", threadIds: [] },
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }),
    );
    expect(entries).toEqual([]);
    expect(manifest.entryCount).toBe(0);
    expect(manifest.byteLength).toBe(0);
    expect(manifest.coverage.eventKindsIncluded).toEqual([]);
    expect(manifest.coverage.eventKindsExcluded.length).toBe(23);
    expect(manifest.exclusions.excludedByVisibility).toBe(0);
    expect(manifest.exclusions.excludedByRedaction).toBe(0);
  });

  test("out-of-scope events and threads are out of scope, not exclusions", async () => {
    const store = makeInMemoryEventLogStore();
    await run(seedStore(store, scriptTurn("turn-x", "other-thread", ["ignored"])));

    const { manifest, entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        eventLog: store,
        turnIds: ["turn-x"],
        threads: [
          {
            id: "other-thread",
            title: "not in scope",
            updatedAt: "2026-07-21T00:00:05.000Z",
            notes: [{ key: "m1", role: "user", text: "hi", timestamp: "2026-07-21T00:00:01.000Z" }],
          },
        ],
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }),
    );
    expect(entries).toEqual([]);
    expect(manifest.exclusions.excludedByVisibility).toBe(0);
    expect(manifest.exclusions.excludedByRedaction).toBe(0);
  });
});

describe("buildHistoryCorpus — large scope", () => {
  test("hundreds of entries across threads stay ordered, addressed, and exactly counted", async () => {
    const store = makeInMemoryEventLogStore();
    const threadIds = ["t1", "t2", "t3"];
    const turnIds: Array<string> = [];
    await run(
      Effect.gen(function* () {
        for (const threadId of threadIds) {
          for (let turn = 0; turn < 10; turn++) {
            const turnId = `${threadId}.turn-${String(turn).padStart(2, "0")}`;
            turnIds.push(turnId);
            const words = Array.from({ length: 12 }, (_, i) => `w${i}`);
            yield* seedStore(store, scriptTurn(turnId, threadId, words));
          }
        }
      }),
    );

    const { manifest, entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Run", runRef: "run-1", threadIds },
        eventLog: store,
        turnIds,
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }),
    );

    // 3 threads x 10 turns x (1 start + 12 deltas + 1 finish) = 420 entries.
    expect(entries.length).toBe(420);
    expect(manifest.entryCount).toBe(420);
    expect(manifest.corpusRef).toBe(`corpus.run.run-1.${BUILT_AT}`);

    // Stable global ordering: (scopeRef, turnId, sequence) strictly ascending.
    for (let i = 1; i < entries.length; i++) {
      const prev = entries[i - 1]!;
      const next = entries[i]!;
      const ordered =
        prev.scopeRef < next.scopeRef ||
        (prev.scopeRef === next.scopeRef &&
          (prev.turnId < next.turnId ||
            (prev.turnId === next.turnId && prev.sequence < next.sequence)));
      expect(ordered).toBe(true);
    }

    expect(manifest.coverage.eventKindsIncluded).toEqual([
      "text.delta",
      "turn.finished",
      "turn.started",
    ]);
    expect(manifest.coverage.eventKindsExcluded).toContain("tool.call");
    expect(manifest.coverage.eventKindsExcluded).toContain("compaction.recorded");
    expect(manifest.byteLength).toBe(
      new TextEncoder().encode(corpusEntriesToJsonl(entries)).length,
    );
  });
});

describe("buildHistoryCorpus — thread-note addressing", () => {
  test("notes use the note key, or a synthetic note.<threadId>.<index> ref, with index as cursor", async () => {
    const { entries } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        threads: [
          {
            id: "t1",
            title: "fixture",
            createdAt: "2026-07-21T00:00:00.000Z",
            updatedAt: "2026-07-21T00:00:05.000Z",
            notes: [
              { key: "m1", role: "user", text: "first", timestamp: "2026-07-21T00:00:01.000Z" },
              { key: "", role: "assistant", text: "second", timestamp: "2026-07-21T00:00:02.000Z" },
              { key: "m3", role: "system", text: "third", timestamp: "2026-07-21T00:00:03.000Z" },
            ],
          },
        ],
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }),
    );

    expect(entries.length).toBe(3);
    expect(entries.map((entry) => entry.turnId)).toEqual(["m1", "m3", "note.t1.1"]);
    const byText = new Map(entries.map((entry) => [entry.text, entry]));
    expect(byText.get("first")!.sequence).toBe(0);
    expect(byText.get("first")!.role).toBe("user");
    expect(byText.get("second")!.turnId).toBe("note.t1.1");
    expect(byText.get("second")!.sequence).toBe(1);
    expect(byText.get("third")!.sequence).toBe(2);
    for (const entry of entries) {
      expect(entry.kind).toBe("thread.note");
      expect(entry.visibility).toBe("private");
      expect(entry.redactionClass).toBe("private_ref");
      expect(entry.scopeRef).toBe("t1");
    }
  });

  test("a duplicate corpus address is a typed build error, never a silent overwrite", async () => {
    const thread: NeutralThreadSnapshot = {
      id: "t1",
      title: "fixture",
      updatedAt: "2026-07-21T00:00:05.000Z",
      notes: [{ key: "m1", role: "user", text: "once", timestamp: "2026-07-21T00:00:01.000Z" }],
    };
    const error = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        threads: [thread, thread],
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }).pipe(Effect.flip),
    );
    expect(error).toBeInstanceOf(HistoryCorpusError);
    expect(error.operation).toBe("address_entries");
  });
});

describe("buildHistoryCorpus — coverage statement", () => {
  test("the manifest names included kinds, excluded kinds, and the seven-kind bound", async () => {
    const store = makeInMemoryEventLogStore();
    await run(seedStore(store, scriptTurn("turn-a", "t1", ["hello"])));

    const { manifest } = await run(
      buildHistoryCorpus({
        scope: { _tag: "Thread", threadId: "t1" },
        eventLog: store,
        turnIds: ["turn-a"],
        threads: [
          {
            id: "t1",
            title: "fixture",
            updatedAt: "2026-07-21T00:00:05.000Z",
            notes: [{ key: "m1", role: "user", text: "hi", timestamp: "2026-07-21T00:00:01.000Z" }],
          },
        ],
        policy: ownerPolicy,
        builtAt: BUILT_AT,
      }),
    );

    expect(manifest.coverage.eventKindsIncluded).toEqual([
      "text.delta",
      "thread.note",
      "turn.finished",
      "turn.started",
    ]);
    // Every neutral kind not present is stated as excluded — 23 minus the 3 event kinds.
    expect(manifest.coverage.eventKindsExcluded.length).toBe(20);
    expect(manifest.coverage.eventKindsExcluded).not.toContain("thread.note");
    expect(manifest.coverage.note).toContain("seven core kinds");
    expect(manifest.coverage.note).toContain("do not reach the neutral log");
  });
});
