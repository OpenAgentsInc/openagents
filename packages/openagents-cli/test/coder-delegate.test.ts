import { describe, expect, it } from "vitest";

import {
  DelegateFleet,
  type DelegateEvent,
  type DelegateHarness,
  describePrompt,
  parseDelegateCommand,
  parseOpencodeEvent,
} from "../src/coder-delegate.js";
import { fleetPhrase, fleetRows, formatTokens, taskActivity } from "../src/coder-fleet.js";
import { CoderSession, type ReplySource } from "../src/coder-session.js";
import { CoderTaskRegistry } from "../src/coder-tasks.js";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

/** A harness whose events and timing the test controls. */
const harness = (
  events: ReadonlyArray<DelegateEvent>,
  options: { readonly fail?: string; readonly hold?: boolean } = {},
): DelegateHarness => ({
  agent: "fake",
  model: "fake/model",
  async *run(_input, signal) {
    for (const event of events) {
      if (signal.aborted) return;
      yield event;
    }
    if (options.hold === true) {
      await new Promise<void>((resolve) => {
        if (signal.aborted) return resolve();
        signal.addEventListener("abort", () => resolve(), { once: true });
      });
      return;
    }
    if (options.fail !== undefined) throw new Error(options.fail);
  },
});

const fleetOf = (
  agent: DelegateHarness,
  maxConcurrent = 4,
): { readonly registry: CoderTaskRegistry; readonly fleet: DelegateFleet } => {
  const registry = new CoderTaskRegistry();
  const transcriptDirectory = mkdtempSync(join(tmpdir(), "delegate-test-"));
  return {
    registry,
    fleet: new DelegateFleet(registry, agent, { maxConcurrent, transcriptDirectory }),
  };
};

const silent: ReplySource = {
  model: "scripted",
  // eslint-disable-next-line require-yield
  async *reply() {
    return;
  },
};

describe("parseDelegateCommand", () => {
  it("leaves an ordinary prompt alone", () => {
    expect(parseDelegateCommand("delegate this to someone")).toBeUndefined();
    expect(parseDelegateCommand("/delegates are people")).toBeUndefined();
  });

  it("reads a single child", () => {
    expect(parseDelegateCommand("/delegate add tests to the parser")).toEqual({
      count: 1,
      prompt: "add tests to the parser",
      description: "add tests to the parser",
    });
  });

  it("reads a fan-out count and keeps it off the prompt", () => {
    const parsed = parseDelegateCommand("/delegate 6x survey the repository for dead code");
    expect(parsed?.count).toBe(6);
    expect(parsed?.prompt).toBe("survey the repository for dead code");
    expect(parsed?.description).toBe("survey the repository for dead");
  });

  it("caps the count rather than launching what was asked for", () => {
    expect(parseDelegateCommand("/delegate 900x go")?.count).toBe(32);
  });

  it("reports a bare command so the caller can explain the grammar", () => {
    expect(parseDelegateCommand("/delegate")).toEqual({
      count: 1,
      prompt: "",
      description: "delegated task",
    });
  });
});

describe("describePrompt", () => {
  it("takes the first few words", () => {
    expect(describePrompt("fix the flaky retry test in the api client")).toBe(
      "fix the flaky retry test",
    );
  });
});

describe("parseOpencodeEvent", () => {
  it("ignores blank lines, prose, and unknown events", () => {
    expect(parseOpencodeEvent("")).toBeUndefined();
    expect(parseOpencodeEvent("thinking…")).toBeUndefined();
    expect(parseOpencodeEvent('{"type":"step_start","part":{}}')).toBeUndefined();
    expect(parseOpencodeEvent('{"type":"future_event","part":{"x":1}}')).toBeUndefined();
  });

  it("reads a tool use and prefers the harness's own title", () => {
    const line = JSON.stringify({
      type: "tool_use",
      part: {
        type: "tool",
        tool: "read",
        callID: "call-1",
        state: { status: "completed", title: "src/cli.ts", input: { filePath: "/tmp/x" } },
      },
    });
    expect(parseOpencodeEvent(line)).toEqual({
      type: "tool",
      callId: "call-1",
      name: "read",
      target: "src/cli.ts",
    });
  });

  it("falls back to an input field when the tool set no title", () => {
    const line = JSON.stringify({
      type: "tool_use",
      part: { tool: "bash", callID: "call-2", state: { input: { command: "pnpm test" } } },
    });
    expect(parseOpencodeEvent(line)).toMatchObject({ name: "bash", target: "pnpm test" });
  });

  it("reads text and token usage", () => {
    expect(parseOpencodeEvent('{"type":"text","part":{"text":"banana"}}')).toEqual({
      type: "text",
      value: "banana",
    });
    expect(
      parseOpencodeEvent('{"type":"step_finish","part":{"tokens":{"input":7913,"output":11}}}'),
    ).toEqual({ type: "tokens", input: 7913, output: 11 });
  });

  it("survives a truncated line", () => {
    expect(parseOpencodeEvent('{"type":"text","part":{"text":"half')).toBeUndefined();
  });
});

describe("DelegateFleet", () => {
  it("runs a child, aggregates its progress, and returns its answer", async () => {
    const { registry, fleet } = fleetOf(
      harness([
        { type: "tool", callId: "a", name: "read", target: "hello.txt" },
        { type: "tool", callId: "a", name: "read", target: "hello.txt" },
        { type: "tokens", input: 100, output: 5 },
        { type: "tokens", input: 200, output: 7 },
        { type: "text", value: "banana" },
      ]),
    );

    const outcome = await fleet.submit({ description: "read a file", prompt: "read hello.txt" });
    expect(outcome).toMatchObject({ status: "completed", result: "banana" });

    const task = registry.list()[0];
    // The same call reported twice is one tool use, and cumulative input usage
    // replaces rather than adds: 200 + (5 + 7).
    expect(task?.progress.toolUseCount).toBe(1);
    expect(task?.progress.tokenCount).toBe(212);
    expect(task?.progress.lastActivity?.toolName).toBe("read");
    expect(task?.status).toBe("completed");
    expect(task?.transcriptPath).toMatch(/\.jsonl$/);
  });

  it("reports a harness that cannot run as a failed child, not a throw", async () => {
    const { registry, fleet } = fleetOf(harness([], { fail: "opencode is not on the path" }));
    const outcome = await fleet.submit({ description: "x", prompt: "go" });
    expect(outcome).toMatchObject({ status: "failed", error: "opencode is not on the path" });
    expect(registry.list()[0]?.status).toBe("failed");
  });

  it("refuses an empty prompt without registering a child", async () => {
    const { registry, fleet } = fleetOf(harness([]));
    expect(await fleet.submit({ description: "x", prompt: "   " })).toMatchObject({
      status: "refused",
      code: "empty_prompt",
    });
    expect(registry.list()).toHaveLength(0);
  });

  it("holds children over the cap as pending rather than starting them", async () => {
    const { registry, fleet } = fleetOf(harness([], { hold: true }), 2);
    const running = [
      fleet.submit({ description: "one", prompt: "go" }),
      fleet.submit({ description: "two", prompt: "go" }),
      fleet.submit({ description: "three", prompt: "go" }),
    ];

    await new Promise((resolve) => setTimeout(resolve, 10));
    const statuses = registry.list().map((task) => task.status);
    expect(statuses.filter((status) => status === "running")).toHaveLength(2);
    expect(statuses.filter((status) => status === "pending")).toHaveLength(1);

    registry.stopAll();
    expect((await Promise.all(running)).every((outcome) => outcome.status === "stopped")).toBe(
      true,
    );
  });

  it("refuses once the queue is full so a fan-out cannot grow without bound", async () => {
    const registry = new CoderTaskRegistry();
    const transcriptDirectory = mkdtempSync(join(tmpdir(), "delegate-test-"));
    const fleet = new DelegateFleet(registry, harness([], { hold: true }), {
      maxConcurrent: 1,
      maxQueued: 1,
      transcriptDirectory,
    });

    const first = fleet.submit({ description: "one", prompt: "go" });
    const second = fleet.submit({ description: "two", prompt: "go" });
    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(await fleet.submit({ description: "three", prompt: "go" })).toMatchObject({
      status: "refused",
      code: "fleet_full",
    });

    registry.stopAll();
    await Promise.all([first, second]);
  });

  it("stops a running child on request", async () => {
    const { registry, fleet } = fleetOf(harness([], { hold: true }));
    const pending = fleet.submit({ description: "one", prompt: "go" });
    await new Promise((resolve) => setTimeout(resolve, 10));

    const id = registry.list()[0]?.id ?? "";
    expect(registry.stop(id)).toBe(true);
    expect(await pending).toEqual({ status: "stopped", taskId: id });
    // A stopped child is terminal, so stopping it again does nothing.
    expect(registry.stop(id)).toBe(false);
  });
});

describe("CoderSession delegation", () => {
  it("launches children from a /delegate line without spending a turn", async () => {
    const { registry, fleet } = fleetOf(harness([{ type: "text", value: "done" }]));
    const session = new CoderSession(silent, "repo", "main", {
      registry,
      fleet,
      label: "fake (fake/model)",
    });

    await session.submit("/delegate 3x tidy the imports");
    await new Promise((resolve) => setTimeout(resolve, 20));

    const snapshot = session.snapshot();
    expect(snapshot.turns).toBe(0);
    expect(snapshot.tasks).toHaveLength(3);
    expect(snapshot.tasks.every((task) => task.status === "completed")).toBe(true);
    // Every child is reported, and reporting clears the badge.
    const notices = snapshot.entries.filter((entry) => entry.role === "notice");
    expect(notices.some((entry) => entry.text.includes("Delegating 3 children"))).toBe(true);
    expect(notices.filter((entry) => entry.text.includes("finished"))).toHaveLength(3);
    expect(snapshot.tasks.some((task) => task.unread)).toBe(false);
  });

  it("says so when the session was started without a child model", async () => {
    const session = new CoderSession(silent, "repo", "main");
    await session.submit("/delegate write the docs");

    expect(session.canDelegate).toBe(false);
    expect(session.snapshot().entries.at(-1)?.text).toContain("cannot delegate");
  });

  it("explains the grammar rather than launching an empty child", async () => {
    const { registry, fleet } = fleetOf(harness([]));
    const session = new CoderSession(silent, "repo", "main", { registry, fleet, label: "fake" });
    await session.submit("/delegate");

    expect(session.snapshot().tasks).toHaveLength(0);
    expect(session.snapshot().entries.at(-1)?.text).toContain("Usage: /delegate");
  });

  it("stops every running child at once", async () => {
    const { registry, fleet } = fleetOf(harness([], { hold: true }));
    const session = new CoderSession(silent, "repo", "main", { registry, fleet, label: "fake" });
    await session.submit("/delegate 2x go");
    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(session.stopTasks()).toBe(2);
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(session.snapshot().tasks.every((task) => task.status === "stopped")).toBe(true);
    session.close();
  });
});

describe("fleet rendering", () => {
  const registry = new CoderTaskRegistry();
  const task = registry.register(
    {
      id: "d1",
      description: "fix the retry test",
      prompt: "fix it",
      agent: "opencode",
      model: "fake/model",
      cwd: "/tmp",
      background: true,
    },
    1000,
  );
  registry.start(task.id, new AbortController());
  registry.recordToolUse(task.id, { toolName: "bash", target: "pnpm test" });
  registry.recordTokens(task.id, { input: 8000, output: 214 });

  it("says what a running child is doing", () => {
    const running = registry.list()[0];
    expect(running).toBeDefined();
    if (running === undefined) return;
    expect(taskActivity(running)).toBe("bash(pnpm test)");
    expect(fleetPhrase([running])).toBe("1 agent");
    const rows = fleetRows([running], 80);
    expect(rows[0]?.branch).toBe("└─");
    expect(rows[0]?.mark).toBe("◐");
    expect(rows[0]?.text).toContain("bash(pnpm test)");
    expect(rows[0]?.text).toContain("8.2k");
  });

  it("replaces the counters with a total once the child is done", () => {
    registry.complete(task.id, "done", 4000);
    const done = registry.list()[0];
    expect(done).toBeDefined();
    if (done === undefined) return;
    expect(taskActivity(done)).toBe("Done (1 tool use · 8.2k tokens · 3s)");
    expect(fleetPhrase([done])).toBe("1 done · 1 unread");
  });

  it("shortens token counts", () => {
    expect(formatTokens(999)).toBe("999");
    expect(formatTokens(8214)).toBe("8.2k");
    expect(formatTokens(2_000_000)).toBe("2M");
  });

  it("cuts a row to the width it was given", () => {
    const rows = fleetRows(registry.list(), 30);
    expect(rows.every((row) => [...row.text].length <= 30)).toBe(true);
  });
});
