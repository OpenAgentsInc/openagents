import { Schema as S } from "effect";
import { khalaRuntimeEventKinds } from "@openagentsinc/agent-runtime-schema";
import { describe, expect, test } from "vite-plus/test";

import {
  HistoryCorpusEntry,
  HistoryCorpusError,
  HistoryCorpusManifest,
  HistoryCorpusScope,
  historyCorpusCoverageNote,
  historyCorpusEventKindVocabulary,
  neutralLogCoreKinds,
} from "./corpus.ts";

const decodeEntry = S.decodeUnknownSync(HistoryCorpusEntry);
const decodeManifest = S.decodeUnknownSync(HistoryCorpusManifest);
const decodeScope = S.decodeUnknownSync(HistoryCorpusScope);

describe("HistoryCorpusEntry schema", () => {
  test("accepts an event-shaped entry with only the safe fields", () => {
    const entry = decodeEntry({
      scopeRef: "thread-1",
      turnId: "turn-1",
      sequence: 3,
      kind: "text.delta",
      text: "hello",
      observedAt: "2026-07-21T00:00:00.000Z",
      visibility: "private",
      redactionClass: "private_ref",
    });
    expect(entry.sequence).toBe(3);
    expect(entry.kind).toBe("text.delta");
    expect(entry.role).toBeUndefined();
  });

  test("accepts a thread-note entry with role and synthetic addressing", () => {
    const entry = decodeEntry({
      scopeRef: "thread-1",
      turnId: "note.thread-1.0",
      sequence: 0,
      kind: "thread.note",
      role: "user",
      text: "please fix the bug",
      observedAt: "2026-07-21T00:00:01.000Z",
      visibility: "private",
      redactionClass: "private_ref",
    });
    expect(entry.kind).toBe("thread.note");
    expect(entry.role).toBe("user");
  });

  test("rejects an unknown kind, visibility, or role", () => {
    const base = {
      scopeRef: "thread-1",
      turnId: "turn-1",
      sequence: 0,
      kind: "text.delta",
      observedAt: "2026-07-21T00:00:00.000Z",
      visibility: "private",
      redactionClass: "private_ref",
    };
    expect(() => decodeEntry({ ...base, kind: "plan.updated" })).toThrow();
    expect(() => decodeEntry({ ...base, visibility: "everyone" })).toThrow();
    expect(() => decodeEntry({ ...base, role: "moderator" })).toThrow();
  });
});

describe("HistoryCorpusScope schema", () => {
  test("decodes all three scope variants", () => {
    expect(decodeScope({ _tag: "Thread", threadId: "t1" })._tag).toBe("Thread");
    expect(decodeScope({ _tag: "Run", runRef: "run-1", threadIds: ["t1", "t2"] })._tag).toBe("Run");
    expect(decodeScope({ _tag: "ThreadSet", threadIds: ["t1"] })._tag).toBe("ThreadSet");
  });

  test("rejects an unknown scope tag", () => {
    expect(() => decodeScope({ _tag: "Owner", ownerId: "o1" })).toThrow();
  });
});

describe("HistoryCorpusManifest schema", () => {
  test("round-trips a full manifest", () => {
    const manifest = decodeManifest({
      corpusRef: "corpus.thread.t1.2026-07-21T00:00:00.000Z",
      scope: { _tag: "Thread", threadId: "t1" },
      builtAt: "2026-07-21T00:00:00.000Z",
      entryCount: 2,
      byteLength: 420,
      coverage: {
        eventKindsIncluded: ["text.delta", "turn.started"],
        eventKindsExcluded: ["tool.call"],
        note: historyCorpusCoverageNote,
      },
      exclusions: {
        excludedByVisibility: 1,
        excludedByRedaction: 0,
        policy: {
          includeVisibilities: ["private"],
          includeRedactionClasses: ["private_ref"],
        },
      },
    });
    expect(manifest.entryCount).toBe(2);
    expect(manifest.exclusions.excludedByVisibility).toBe(1);
  });
});

describe("coverage constants", () => {
  test("the seven neutral-log core kinds are exactly the projection bound", () => {
    expect(neutralLogCoreKinds).toEqual([
      "turn.started",
      "turn.finished",
      "turn.interrupted",
      "text.delta",
      "reasoning.delta",
      "tool.call",
      "tool.result",
    ]);
  });

  test("the honesty note names every one of the seven core kinds", () => {
    for (const kind of neutralLogCoreKinds) {
      expect(historyCorpusCoverageNote).toContain(kind);
    }
    expect(historyCorpusCoverageNote).toContain("seven core kinds");
  });

  test("the coverage vocabulary is the full neutral event-kind union", () => {
    expect(historyCorpusEventKindVocabulary).toEqual(khalaRuntimeEventKinds);
    expect(historyCorpusEventKindVocabulary.length).toBe(23);
  });
});

describe("HistoryCorpusError", () => {
  test("is a tagged error with the package tag", () => {
    const error = new HistoryCorpusError({ operation: "assemble_corpus", detail: "boom" });
    expect(error._tag).toBe("HistoryCorpus.Error");
    expect(error.operation).toBe("assemble_corpus");
  });
});
