import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { CoderMemory } from "../src/coder-memory.js";
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
