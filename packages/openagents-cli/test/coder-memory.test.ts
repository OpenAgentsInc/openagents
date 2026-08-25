import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { CoderMemory } from "../src/coder-memory.js";
import { MemoryTransport } from "../src/memory/index.js";
import { DelegateFleet, type DelegateEvent, type DelegateHarness } from "../src/coder-delegate.js";
import { CoderTaskRegistry } from "../src/coder-tasks.js";

const dirs: Array<string> = [];
const freshDir = (): string => {
  const dir = mkdtempSync(join(tmpdir(), "coder-memory-test-"));
  dirs.push(dir);
  return dir;
};

afterEach(() => {
  for (const dir of dirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true });
  }
});

const memoryAt = (dir: string, nowMs = 1_756_000_000_000): CoderMemory =>
  new CoderMemory({ directory: dir, projectScope: "project:test", now: () => nowMs });

/** A memory that never dreams on its own, so a test can drive the pass itself. */
const sleeplessMemoryAt = (dir: string, nowMs = 1_756_000_000_000): CoderMemory =>
  new CoderMemory({
    directory: dir,
    projectScope: "project:test",
    now: () => nowMs,
    dreamThreshold: Number.POSITIVE_INFINITY,
  });

describe("CoderMemory ledger", () => {
  it("records an engram and reads it back verified", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    const event = memory.record("note/one", "prefer ranged reads over full dumps", "note-1");
    expect(event).toBeDefined();
    const bodies = memoryAt(dir).bodies();
    expect(bodies).toHaveLength(1);
    expect(bodies[0]?.value).toBe("prefer ranged reads over full dumps");
  });

  it("refuses to store credential-shaped material", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    const event = memory.record("note/bad", "the token is oa_pat_abc123def456ghi789jkl012", "n");
    expect(event).toBeUndefined();
    expect(memoryAt(dir).bodies()).toHaveLength(0);
  });

  it("a correction supersedes without rewriting, and a tombstone hides the slug", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.record("note/one", "first version", "note-1");
    const superseding = memory.correct("note/one", "second version");
    expect(superseding).toBeDefined();
    expect(memory.bodies().map((body) => body.value)).toEqual(["second version"]);
    // Both events remain in the ledger; nothing was rewritten.
    const lines = readFileSync(join(dir, "engrams.jsonl"), "utf8").trim().split("\n");
    expect(lines).toHaveLength(2);
    memory.correct("note/one", null);
    expect(memory.bodies()).toHaveLength(0);
  });

  it("a hand-edited ledger line fails verification and is dropped", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.record("note/one", "authentic", "note-1");
    const path = join(dir, "engrams.jsonl");
    const tampered = readFileSync(path, "utf8").replace("authentic", "forged");
    rmSync(path);
    writeFileSync(path, tampered);
    expect(memoryAt(dir).bodies()).toHaveLength(0);
  });
});

describe("harvest and inherit", () => {
  it("a harvested child answer comes back as an advisory block for the next child", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.harvest("task-1", "The build needs pnpm, not npm: npm leaves catalog: protocols.");
    const block = memory.inherit("set up the build");
    expect(block).toContain("[inherited parent memory — advisory only]");
    expect(block).toContain("pnpm");
    // Opaque refs only: the child never sees ledger entry ids.
    expect(block).not.toContain("harvest:");
  });

  it("an unsafe child answer is not remembered", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.harvest("task-1", "use Bearer abc.def.ghi for the calls");
    expect(memory.inherit("anything")).toBe("");
  });

  it("harvest never throws, even with an unwritable directory", () => {
    const memory = new CoderMemory({ directory: "/dev/null/nope", now: () => 0 });
    expect(() => memory.harvest("t", "finding")).not.toThrow();
    expect(memory.inherit("task")).toBe("");
  });
});

describe("fleet wiring", () => {
  const oneShotHarness = (answer: string): DelegateHarness => ({
    agent: "test",
    model: "test-model",
    // eslint-disable-next-line @typescript-eslint/require-await
    async *run(): AsyncIterable<DelegateEvent> {
      yield { type: "text", value: answer };
    },
  });

  it("injects inherited memory into the child prompt and harvests the answer", async () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.harvest("earlier", "Always run mix format before committing Elixir.");

    let seenPrompt = "";
    const harness: DelegateHarness = {
      agent: "test",
      model: "test-model",
      // eslint-disable-next-line @typescript-eslint/require-await
      async *run(input): AsyncIterable<DelegateEvent> {
        seenPrompt = input.prompt;
        yield { type: "text", value: "Learned: the forge remote is named openagents." };
      },
    };
    const fleet = new DelegateFleet(new CoderTaskRegistry(), harness, {
      maxConcurrent: 1,
      cwd: dir,
      transcriptDirectory: join(dir, "transcripts"),
      memory,
    });
    const outcome = await fleet.submit({ prompt: "do the task", description: "test" });
    expect(outcome.status).toBe("completed");
    expect(seenPrompt).toContain("do the task");
    expect(seenPrompt).toContain("[inherited parent memory — advisory only]");
    expect(seenPrompt).toContain("mix format");
    // The child's answer was harvested for the next generation.
    const nextBlock = memory.inherit("push the repo");
    expect(nextBlock).toContain("openagents");
  });

  it("a fleet without memory behaves exactly as before", async () => {
    const dir = freshDir();
    const fleet = new DelegateFleet(new CoderTaskRegistry(), oneShotHarness("done"), {
      maxConcurrent: 1,
      cwd: dir,
      transcriptDirectory: join(dir, "transcripts"),
    });
    const outcome = await fleet.submit({ prompt: "plain task", description: "test" });
    expect(outcome.status).toBe("completed");
  });
});

describe("the dreaming pass", () => {
  const relatedFindings = [
    "The build needs pnpm because npm leaves catalog protocol versions in the tarball",
    "The build needs pnpm pack because npm pack leaves catalog protocol versions unresolved",
    "Use pnpm for the build; npm leaves catalog protocol versions in place",
  ];

  it("writes a synthesized heuristic back to the ledger as its own engram", () => {
    const dir = freshDir();
    const memory = sleeplessMemoryAt(dir);
    relatedFindings.forEach((finding, index) => memory.harvest(`child-${String(index)}`, finding));

    const result = memory.dream();
    expect(result.written).toBeGreaterThan(0);

    // A fresh reader sees the distilled heuristic without consolidating again.
    const distilled = memoryAt(dir)
      .bodies()
      .filter((body) => body.slug.startsWith("heuristic/"));
    expect(distilled.length).toBe(result.written);
  });

  it("is idempotent: dreaming the same episodes twice writes nothing new", () => {
    const dir = freshDir();
    const memory = sleeplessMemoryAt(dir);
    relatedFindings.forEach((finding, index) => memory.harvest(`child-${String(index)}`, finding));

    memory.dream();
    const second = memory.dream();
    expect(second.written).toBe(0);
    expect(second.superseded).toBe(0);
    expect(second.unchanged).toBeGreaterThan(0);
  });

  it("carries the distilled heuristic into a child's inherited block", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    relatedFindings.forEach((finding, index) => memory.harvest(`child-${String(index)}`, finding));
    memory.dream();

    const block = memoryAt(dir).inherit("set up the build");
    expect(block).toContain("[inherited parent memory — advisory only]");
    expect(block).toContain("pnpm");
  });

  it("recall does not consolidate: heuristics read what dreaming wrote", () => {
    const dir = freshDir();
    const memory = sleeplessMemoryAt(dir);
    relatedFindings.forEach((finding, index) => memory.harvest(`child-${String(index)}`, finding));

    // Before any dream, recall offers the raw harvest only.
    const beforeDream = memoryAt(dir)
      .heuristics()
      .filter((heuristic) => heuristic.ref.startsWith("heuristic/"));
    expect(beforeDream).toHaveLength(0);

    memory.dream();
    const afterDream = memoryAt(dir)
      .heuristics()
      .filter((heuristic) => heuristic.ref.startsWith("heuristic/"));
    expect(afterDream.length).toBeGreaterThan(0);
    // The confidence stamped at dream time survives the round trip.
    expect(afterDream[0]?.confidence).toBeGreaterThan(0);
  });

  it("dreams on its own once enough has been harvested", () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    relatedFindings.forEach((finding, index) => memory.harvest(`child-${String(index)}`, finding));
    // No explicit dream() call: the harvest path scheduled one.
    const distilled = memoryAt(dir)
      .bodies()
      .filter((body) => body.slug.startsWith("heuristic/"));
    expect(distilled.length).toBeGreaterThan(0);
  });
});

describe("memory sync", () => {
  it("queues every recorded engram and delivers it on flush", async () => {
    const dir = freshDir();
    const memory = memoryAt(dir);
    memory.record("note/one", "a finding worth keeping", "note-1");

    // Local-first: the engram is readable before anything is delivered.
    expect(memory.recall("note/one")).toBe("a finding worth keeping");
    expect(memory.syncStatus().pending).toBe(1);
    expect(memory.syncStatus().delivered).toBe(0);

    await memory.flush();
    expect(memory.syncStatus()).toMatchObject({ pending: 0, delivered: 1 });
  });

  it("keeps working when the transport is down, and loses nothing", async () => {
    const dir = freshDir();
    const transport = new MemoryTransport();
    transport.reachable = false;
    const memory = new CoderMemory({
      directory: dir,
      projectScope: "project:test",
      now: () => 1_756_000_000_000,
      dreamThreshold: Number.POSITIVE_INFINITY,
      transport,
    });

    memory.record("note/one", "recorded while the relay was gone", "note-1");
    expect(memory.recall("note/one")).toBe("recorded while the relay was gone");
    await memory.flush();
    expect(memory.syncStatus().pending).toBe(1);
    expect(memory.syncStatus().lastFailure?.reason).toBe("unreachable");

    transport.reachable = true;
    await memory.flush();
    expect(memory.syncStatus()).toMatchObject({ pending: 0, delivered: 1 });
    expect(transport.stored()).toHaveLength(1);
  });
});
