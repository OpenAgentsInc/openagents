/**
 * Memory on the retrieval rail (OpenAgentsInc/openagents#51): the note format,
 * the ranking integration with the owned ranking module, the bucket labels,
 * the `remember` write path, and the degrade-to-silence posture on an empty,
 * corrupt, or unreadable ledger. The rail itself is one call in the harness
 * (`memoryRecallNote(memory.recallable(), prompt, now)`), so these tests pin
 * that seam the way the knowledge-base tests pin theirs: the store on one
 * side, the pure note builder on the other, and the acceptance scenario end
 * to end across both.
 */

import { appendFileSync, chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { CoderMemory } from "../src/coder-memory.js";
import {
  MEMORY_ATTACH_FLOOR,
  MEMORY_NOTE_LIMIT,
  memoryRecallNote,
  rememberTool,
  type RecallableMemory,
} from "../src/coder-recall.js";

const NOW = 1_756_000_000_000;
const DAY = 24 * 60 * 60 * 1000;

const dirs: Array<string> = [];
const freshDir = (): string => {
  const dir = mkdtempSync(join(tmpdir(), "coder-recall-test-"));
  dirs.push(dir);
  return dir;
};

afterEach(() => {
  for (const dir of dirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

const memoryAt = (dir: string, nowMs = NOW): CoderMemory =>
  new CoderMemory({ directory: dir, projectScope: "project:test", now: () => nowMs });

const userMemory = (text: string, recordedAtMs = NOW - 3 * DAY): RecallableMemory => ({
  bucket: "user",
  ref: "user/abc123def456",
  text,
  recordedAtMs,
});

const learnedMemory = (
  text: string,
  overrides: Partial<RecallableMemory> = {},
): RecallableMemory => ({
  bucket: "learned",
  ref: "heuristic/ab12cd34ef56",
  text,
  recordedAtMs: NOW - 12 * DAY,
  confidence: 0.75,
  provenance: ["harvest/aaa", "harvest/bbb"],
  ...overrides,
});

describe("the memory note format", () => {
  it("labels a user note with its bucket, kind, and age", () => {
    const note = memoryRecallNote([userMemory("I use pnpm, not npm")], "install the deps", NOW);
    expect(note).toBeDefined();
    expect(note).toContain("[From memory");
    expect(note).toContain("- (user note, 3d old) I use pnpm, not npm");
  });

  it("labels a learned heuristic with its slug, age, and episode provenance", () => {
    const note = memoryRecallNote(
      [learnedMemory("prefer ranged reads over full file dumps")],
      "read the config file with a ranged read",
      NOW,
    );
    expect(note).toBeDefined();
    expect(note).toContain(
      "- (learned heuristic heuristic/ab12cd34ef56, 12d old, from 2 episodes) " +
        "prefer ranged reads over full file dumps",
    );
  });

  it("keeps the two buckets distinct in one note", () => {
    const note = memoryRecallNote(
      [
        userMemory("I use pnpm, not npm"),
        learnedMemory("install dependencies before running tests"),
      ],
      "install the project dependencies",
      NOW,
    );
    expect(note).toContain("(user note,");
    expect(note).toContain("(learned heuristic heuristic/");
  });

  it("says nothing when there are no memories at all", () => {
    expect(memoryRecallNote([], "install the deps", NOW)).toBeUndefined();
  });
});

describe("ranking on the rail", () => {
  it("always attaches a user note, even with no lexical overlap", () => {
    // The acceptance scenario: the remembered preference shares no vocabulary
    // with the message that needs it, and it must still arrive.
    const note = memoryRecallNote([userMemory("I use pnpm, not npm")], "install the deps", NOW);
    expect(note).toContain("pnpm");
  });

  it("stays silent on a learned heuristic with no overlap with the message", () => {
    const note = memoryRecallNote(
      [learnedMemory("prefer ranged reads over full file dumps")],
      "deploy the site",
      NOW,
    );
    expect(note).toBeUndefined();
  });

  it("drops a learned heuristic whose salience stays under the floor", () => {
    // One overlap hit at rock-bottom confidence: 0.1 + 0.25 < the 0.5 floor.
    const weak = learnedMemory("deploy carefully", { confidence: 0.1 });
    expect(memoryRecallNote([weak], "deploy the site", NOW)).toBeUndefined();
    expect(MEMORY_ATTACH_FLOOR).toBe(0.5);
  });

  it("ranks the memory that overlaps the message above the one that does not", () => {
    const note = memoryRecallNote(
      [
        userMemory("my timezone is UTC+2"),
        { ...userMemory("I use pnpm for installing deps"), ref: "user/fff" },
      ],
      "install the deps",
      NOW,
    );
    expect(note).toBeDefined();
    const pnpmAt = (note ?? "").indexOf("pnpm");
    const timezoneAt = (note ?? "").indexOf("timezone");
    expect(pnpmAt).toBeGreaterThan(-1);
    expect(timezoneAt).toBeGreaterThan(-1);
    expect(pnpmAt).toBeLessThan(timezoneAt);
  });

  it("carries at most the top-K memories and stays inside the token budget", () => {
    const many = Array.from({ length: 12 }, (_, index) => ({
      ...userMemory(`standing preference number ${index} about tooling`),
      ref: `user/${index}`,
      recordedAtMs: NOW - index * DAY,
    }));
    const note = memoryRecallNote(many, "anything at all", NOW);
    expect(note).toBeDefined();
    expect((note ?? "").split("\n- ").length - 1).toBeLessThanOrEqual(MEMORY_NOTE_LIMIT);
    // The budget bounds the note: 320 tokens at ~4 chars each, plus header.
    expect((note ?? "").length).toBeLessThan(2000);
  });

  it("clips a runaway memory value rather than letting it flood the turn", () => {
    const note = memoryRecallNote([userMemory(`x${"y".repeat(5000)}`)], "anything", NOW);
    expect(note).toBeDefined();
    expect((note ?? "").length).toBeLessThan(1000);
    expect(note).toContain("…");
  });
});

describe("the remember write path", () => {
  it("writes a user-bucket engram the rail reads back", async () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    const tool = rememberTool(memory);
    const reply = await tool.run({ fact: "I use pnpm, not npm" }, new AbortController().signal);
    expect(reply).toContain("Remembered");

    // A fresh handle on the same ledger sees the fact: the write is on disk,
    // not in the instance.
    const recalled = memoryAt(dir).recallable();
    expect(recalled).toHaveLength(1);
    expect(recalled[0]?.bucket).toBe("user");
    expect(recalled[0]?.ref.startsWith("user/")).toBe(true);
    expect(recalled[0]?.text).toBe("I use pnpm, not npm");
  });

  it("remembering the same fact twice supersedes instead of forking the chain", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    expect(memory.remember("I use pnpm, not npm")).toBeDefined();
    expect(memory.remember("I use pnpm, not npm")).toBeDefined();
    const recalled = memory.recallable();
    expect(recalled).toHaveLength(1);
    expect(recalled[0]?.text).toBe("I use pnpm, not npm");
  });

  it("refuses credential-shaped material in words, not by throwing", async () => {
    const dir = freshDir();
    const tool = rememberTool(memoryAt(dir));
    const reply = await tool.run(
      { fact: "the token is oa_pat_abc123def456ghi789jkl012" },
      new AbortController().signal,
    );
    expect(reply).toContain("not stored");
    expect(memoryAt(dir).recallable()).toHaveLength(0);
  });

  it("refuses an empty fact in words", async () => {
    const tool = rememberTool(memoryAt(freshDir()));
    expect(await tool.run({ fact: "   " }, new AbortController().signal)).toContain(
      "Nothing to remember",
    );
    expect(await tool.run({}, new AbortController().signal)).toContain("Nothing to remember");
  });
});

describe("recall from the ledger", () => {
  it("surfaces a distilled heuristic as learned, with confidence and provenance", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.record(
      "heuristic/ab12cd34ef56",
      "install dependencies before running the tests",
      "synth-1#0.750",
      ["harvest/aaa", "harvest/bbb"],
    );
    const recalled = memory.recallable();
    expect(recalled).toHaveLength(1);
    expect(recalled[0]?.bucket).toBe("learned");
    expect(recalled[0]?.confidence).toBeCloseTo(0.75);
    expect(recalled[0]?.provenance).toEqual(["harvest/aaa", "harvest/bbb"]);
  });

  it("leaves harvest episodes out of the rail", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.record("harvest/abc123def456", "some raw finding", "child-1");
    expect(memory.recallable()).toHaveLength(0);
  });

  it("an empty ledger recalls nothing, without an error", () => {
    const memory = memoryAt(freshDir());
    expect(memory.recallable()).toEqual([]);
    expect(memoryRecallNote(memory.recallable(), "install the deps", NOW)).toBeUndefined();
  });

  it("a corrupt ledger recalls nothing, without an error", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.remember("I use pnpm, not npm");
    appendFileSync(join(dir, "engrams.jsonl"), 'not json at all\n{"half": \n');
    const recalled = memoryAt(dir).recallable();
    // The junk lines are dropped; the verified engram survives them.
    expect(recalled).toHaveLength(1);
    // A ledger that is nothing but junk is simply no memories.
    writeFileSync(join(dir, "engrams.jsonl"), "garbage\n{}\n[1,2]\n");
    expect(memoryAt(dir).recallable()).toEqual([]);
  });

  it.skipIf(process.getuid?.() === 0)(
    "an unreadable ledger recalls nothing, without an error",
    () => {
      const dir = freshDir();
      const memory = memoryAt(dir);
      memory.remember("I use pnpm, not npm");
      chmodSync(join(dir, "engrams.jsonl"), 0o000);
      try {
        expect(memoryAt(dir).recallable()).toEqual([]);
      } finally {
        chmodSync(join(dir, "engrams.jsonl"), 0o600);
      }
    },
  );
});

describe("end to end across the seam", () => {
  it("a remembered preference answers a later unrelated-sounding turn", async () => {
    // The acceptance scenario for #51: the reader says "remember I use pnpm,
    // not npm"; the model calls the remember tool once. A later "install the
    // deps" turn gets the memory as attached context from the rail alone.
    const dir = freshDir();
    const tool = rememberTool(memoryAt(dir));
    await tool.run({ fact: "I use pnpm, not npm" }, new AbortController().signal);

    const later = memoryAt(dir, NOW + 2 * DAY);
    const note = memoryRecallNote(later.recallable(), "install the deps", NOW + 2 * DAY);
    expect(note).toBeDefined();
    expect(note).toContain("(user note, 2d old) I use pnpm, not npm");
  });
});
