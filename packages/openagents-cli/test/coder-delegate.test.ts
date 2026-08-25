import { describe, expect, it } from "vitest";

import {
  DelegateFleet,
  type DelegateEvent,
  type DelegateHarness,
  describePrompt,
  parseDelegateCommand,
  parseOpencodeEvent,
  transientProviderFailure,
} from "../src/coder-delegate.js";
import {
  activityPhrase,
  fleetRows,
  formatTokens,
  latestActivities,
  taskActivity,
} from "../src/coder-fleet.js";
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
      Date.now(),
    );
  registry.start(task.id, new AbortController());
  registry.recordToolUse(task.id, { toolName: "bash", target: "pnpm test" });
  registry.recordTokens(task.id, { input: 8000, output: 214 });

  it("says what a running child is doing", () => {
    const running = registry.list()[0];
    expect(running).toBeDefined();
    if (running === undefined) return;
    // A running child says how long it has been running, so the reader can
    // tell a slow child from a stuck one.
    const now = running.startedAt + 95_000;
    expect(taskActivity(running, now)).toBe("bash(pnpm test) (1m 35s)");
    const rows = fleetRows([running], 80, now);
    expect(rows[0]?.branch).toBe("└─");
    expect(rows[0]?.mark).toBe("◐");
    expect(rows[0]?.text).toContain("bash(pnpm test)");
    expect(rows[0]?.text).toContain("8.2k tokens");
  });

  it("replaces the counters with a total once the child is done", () => {
    registry.complete(task.id, "done", 4000);
    const done = registry.list()[0];
    expect(done).toBeDefined();
    if (done === undefined) return;
    expect(taskActivity(done)).toBe("Done (1 tool use · 8.2k tokens · 3s)");
  });

  it("leaves usage out of a row until the harness has reported some", () => {
    const fresh = new CoderTaskRegistry();
    const started = fresh.register(
      {
        id: "d2",
        description: "read a file",
        prompt: "read it",
        agent: "opencode",
        model: "fake/model",
        cwd: "/tmp",
        background: true,
      },
      0,
    );
    fresh.start(started.id, new AbortController());
    fresh.recordToolUse(started.id, { toolName: "read", target: "x.ts" });

    const row = fleetRows(fresh.list(), 80)[0];
    expect(row?.text).toContain("1 tool");
    expect(row?.text).not.toContain(" 0");
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

  it("says an activity on its own, with no clock around it", () => {
    expect(activityPhrase({ toolName: "read", target: "src/a.ts" })).toBe("read(src/a.ts)");
    expect(activityPhrase({ toolName: "think", target: undefined })).toBe("think");
  });
});

describe("latestActivities", () => {
  const startedRegistry = () => {
    const registry = new CoderTaskRegistry();
    const task = registry.register(
      {
        id: "d1",
        description: "inspect the repo",
        prompt: "look around",
        agent: "opencode",
        model: "fake/model",
        cwd: "/tmp",
        background: true,
      },
      0,
    );
    registry.start(task.id, new AbortController());
    return { registry, id: task.id };
  };

  it("returns nothing when no child has done anything", () => {
    expect(latestActivities([], 3)).toEqual([]);

    const { registry, id } = startedRegistry();
    expect(latestActivities(registry.list(), 3)).toEqual([]);
    registry.complete(id, "done", 10);
    expect(latestActivities(registry.list(), 3)).toEqual([]);
  });

  it("keeps the newest few, oldest first, across the fleet", () => {
    const { registry, id } = startedRegistry();
    for (const [name, target] of [
      ["read", "a.ts"],
      ["grep", "needle"],
      ["bash", "ls"],
      ["edit", "b.ts"],
    ] as const) {
      registry.recordToolUse(id, { toolName: name, target });
    }

    expect(latestActivities(registry.list(), 3)).toEqual([
      { toolName: "grep", target: "needle" },
      { toolName: "bash", target: "ls" },
      { toolName: "edit", target: "b.ts" },
    ]);
  });

  it("leaves a finished child's steps out of a live preview", () => {
    const first = startedRegistry();
    first.registry.recordToolUse(first.id, { toolName: "bash", target: "stale" });
    first.registry.complete(first.id, "done", 10);

    const second = startedRegistry();
    second.registry.recordToolUse(second.id, { toolName: "grep", target: "fresh" });

    expect(latestActivities([...first.registry.list(), ...second.registry.list()], 3)).toEqual([
      { toolName: "grep", target: "fresh" },
    ]);
  });
});

describe("classifying a child's failure", () => {
  it("recognises the provider going away, in the shape opencode reports it", () => {
    // The message seen in practice, verbatim.
    expect(
      transientProviderFailure(
        "APIError: Error from provider (Console): Upstream request failed: Endpoint is unavailable.",
      ),
    ).toBe(true);
  });

  it("recognises the other ways a provider drops", () => {
    for (const message of [
      "503 Service Unavailable",
      "Provider overloaded, try again",
      "Rate limit exceeded",
      "socket hang up",
      "ECONNRESET",
      "502 Bad Gateway",
    ]) {
      expect(transientProviderFailure(message)).toBe(true);
    }
  });

  it("does not retry a failure that will recur", () => {
    for (const message of [
      "Permission denied by the user",
      "Model not found: nonsense/model",
      "The prompt could not be carried out",
      "Unauthorized: invalid API key",
      // Wording that overlaps a transient shape but names the request.
      "Quota exceeded: too many requests this month",
    ]) {
      expect(transientProviderFailure(message)).toBe(false);
    }
  });
});

describe("retrying a child whose provider dropped", () => {
  /** A harness that fails the first `failures` runs, then succeeds. */
  const flaky = (
    failures: number,
    message: string,
    options: { readonly session?: string; readonly toolsBeforeFailing?: number } = {},
  ) => {
    const runs: Array<string | undefined> = [];
    let seen = 0;

    const built: DelegateHarness = {
      agent: "fake",
      model: "fake/model",
      async *run(input) {
        runs.push(input.resumeSessionId);
        seen += 1;
        if (options.session !== undefined) {
          yield { type: "session", sessionId: options.session };
        }
        for (let index = 0; index < (options.toolsBeforeFailing ?? 0); index += 1) {
          yield { type: "tool", callId: `c${String(seen)}-${String(index)}`, name: "read", target: "f" };
        }
        if (seen <= failures) {
          yield { type: "error", message };
          return;
        }
        yield { type: "text", value: "finished" };
      },
    };

    return { harness: built, runs };
  };

  const fleetFor = (built: DelegateHarness) =>
    new DelegateFleet(new CoderTaskRegistry(), built, {
      maxConcurrent: 1,
      cwd: mkdtempSync(join(tmpdir(), "oa-retry-")),
    });

  it("resumes the session rather than starting the work again", async () => {
    const { harness: built, runs } = flaky(1, "Endpoint is unavailable", {
      session: "ses_abc",
      toolsBeforeFailing: 3,
    });

    const outcome = await fleetFor(built).submit({
      description: "flaky",
      prompt: "do the thing",
      cwd: ".",
      background: false,
    });

    expect(outcome.status).toBe("completed");
    // The retry carried the session, so the three tools already run are not
    // run again from the prompt.
    expect(runs).toEqual([undefined, "ses_abc"]);
  }, 20_000);

  it("gives up after a bounded number of attempts", async () => {
    const { harness: built, runs } = flaky(9, "Endpoint is unavailable", { session: "ses_abc" });

    const outcome = await fleetFor(built).submit({
      description: "always failing",
      prompt: "do the thing",
      cwd: ".",
      background: false,
    });

    expect(outcome.status).toBe("failed");
    expect(runs).toHaveLength(3);
  }, 20_000);

  it("does not retry a failure that will recur", async () => {
    const { harness: built, runs } = flaky(1, "Permission denied by the user", {
      session: "ses_abc",
    });

    const outcome = await fleetFor(built).submit({
      description: "refused",
      prompt: "do the thing",
      cwd: ".",
      background: false,
    });

    expect(outcome.status).toBe("failed");
    expect(runs).toHaveLength(1);
  });

  it("refuses to redo work it cannot resume", async () => {
    // No session reported, and the child already ran tools: re-running from
    // the prompt would apply its edits a second time.
    const { harness: built, runs } = flaky(1, "Endpoint is unavailable", {
      toolsBeforeFailing: 2,
    });

    const outcome = await fleetFor(built).submit({
      description: "unresumable",
      prompt: "do the thing",
      cwd: ".",
      background: false,
    });

    expect(outcome.status).toBe("failed");
    expect(runs).toHaveLength(1);
  });

  it("retries a child that had done nothing yet, session or not", async () => {
    const { harness: built, runs } = flaky(1, "Endpoint is unavailable");

    const outcome = await fleetFor(built).submit({
      description: "nothing done",
      prompt: "do the thing",
      cwd: ".",
      background: false,
    });

    expect(outcome.status).toBe("completed");
    expect(runs).toHaveLength(2);
  }, 20_000);
});
