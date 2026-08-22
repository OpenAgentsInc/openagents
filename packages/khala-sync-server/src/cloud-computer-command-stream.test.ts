import { describe, expect, test } from "vite-plus/test";

import {
  CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA,
  CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA,
  createCloudComputerCommandStream,
  type CloudComputerCommandJournalPort,
  type CloudComputerCommandStreamCursor,
  type CloudComputerCommandStreamEvent,
  type CloudComputerCommandStreamPage,
  type CloudComputerCommandStreamRead,
  type CloudComputerCommandStreamScope,
  type CloudComputerCommandWakeupPort,
} from "./cloud-computer-command-stream.js";

const digest = (character: string): string => `sha256:${character.repeat(64)}`;
const scope: CloudComputerCommandStreamScope = {
  ownerRef: "owner.stream.alice",
  tenantRef: "tenant.stream.acme",
  computerRef: "computer.stream.six",
  workspaceRef: "workspace.stream.six",
  sessionRef: "session.stream.six",
  commandRef: "command.stream.six",
  runtimeGeneration: 4,
};

const cursor = (sequence: number, retentionEpoch = 1): CloudComputerCommandStreamCursor => ({
  schema: CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  sessionRef: scope.sessionRef,
  commandRef: scope.commandRef,
  runtimeGeneration: scope.runtimeGeneration,
  sequence,
  retentionEpoch,
});

const event = (sequence: number): CloudComputerCommandStreamEvent => ({
  eventRef: `event.stream.${sequence}`,
  sequence,
  eventDigest: digest(String(sequence)),
  kind: "stdout",
  payload: { text: `line ${sequence}` },
  artifactRefs: [],
  observedAt: new Date(Date.UTC(2026, 7, 22, 12, 0, sequence)).toISOString(),
});

const page = (
  sequence: number,
  events: ReadonlyArray<CloudComputerCommandStreamEvent> = [],
  overrides: Partial<CloudComputerCommandStreamPage> = {},
): CloudComputerCommandStreamPage => ({
  schema: CLOUD_COMPUTER_COMMAND_STREAM_PAGE_SCHEMA,
  kind: "page",
  scope,
  commandStatus: "running",
  cursor: cursor(sequence),
  events,
  hasMore: false,
  retentionWatermark: 0,
  retentionEpoch: 1,
  terminal: null,
  artifacts: [],
  ...overrides,
});

class Journal implements CloudComputerCommandJournalPort {
  reads = 0;
  cursors: Array<CloudComputerCommandStreamCursor | null> = [];
  current: CloudComputerCommandStreamRead = page(0);
  onRead: ((read: number) => void) | undefined;

  async read(input: { cursor: CloudComputerCommandStreamCursor | null }) {
    this.reads += 1;
    this.cursors.push(input.cursor);
    this.onRead?.(this.reads);
    return this.current;
  }
}

class Wakeups implements CloudComputerCommandWakeupPort {
  wake: (() => void) | undefined;
  subscriptions = 0;
  unsubscriptions = 0;
  duringSubscribe: (() => void) | undefined;

  async subscribe(_topic: string, wake: () => void) {
    this.subscriptions += 1;
    this.wake = wake;
    this.duringSubscribe?.();
    return () => {
      this.unsubscriptions += 1;
      if (this.wake === wake) this.wake = undefined;
    };
  }
}

class Poll {
  resolve: (() => void) | undefined;

  async wait(signal: AbortSignal) {
    await new Promise<void>((resolve) => {
      this.resolve = resolve;
      signal.addEventListener("abort", () => resolve(), { once: true });
    });
  }

  tick() {
    this.resolve?.();
    this.resolve = undefined;
  }
}

const harness = () => {
  const journal = new Journal();
  const wakeups = new Wakeups();
  const poll = new Poll();
  const adapter = createCloudComputerCommandStream({ journal, wakeups, poll });
  return { adapter, journal, wakeups, poll };
};

describe("cloud computer durable command stream", () => {
  test("polls the journal after a lost wakeup", async () => {
    const { adapter, journal, poll } = harness();
    const stream = adapter.open({ scope, cursor: null });
    expect((await stream.next()).value).toEqual(page(0));

    const next = stream.next();
    await Promise.resolve();
    await Promise.resolve();
    journal.current = page(1, [event(1)]);
    poll.tick();
    expect((await next).value).toEqual(journal.current);
    await stream.return();
  });

  test("emits a status-only transition after a lost wakeup", async () => {
    const { adapter, journal, poll } = harness();
    journal.current = page(1, [], { commandStatus: "dispatched" });
    const stream = adapter.open({ scope, cursor: cursor(1) });
    expect((await stream.next()).value).toEqual(journal.current);

    const next = stream.next();
    await Promise.resolve();
    await Promise.resolve();
    journal.current = page(1, [], { commandStatus: "running" });
    poll.tick();
    expect((await next).value).toEqual(journal.current);
    await stream.return();
  });

  test("closes a write race when a wakeup arrives during subscribe", async () => {
    const { adapter, journal, wakeups } = harness();
    const stream = adapter.open({ scope, cursor: null });
    expect((await stream.next()).value).toEqual(page(0));
    wakeups.duringSubscribe = () => {
      journal.current = page(1, [event(1)]);
      wakeups.wake?.();
    };
    expect((await stream.next()).value).toEqual(journal.current);
    expect(journal.reads).toBe(2);
    await stream.return();
  });

  test("restarts from the durable cursor without relying on prior subscription state", async () => {
    const first = harness();
    first.journal.current = page(2, [event(1), event(2)]);
    const initial = first.adapter.open({ scope, cursor: null });
    expect((await initial.next()).value).toEqual(first.journal.current);
    await initial.return();

    const restarted = harness();
    restarted.journal.current = page(3, [event(3)]);
    const resumed = restarted.adapter.open({ scope, cursor: cursor(2) });
    expect((await resumed.next()).value).toEqual(restarted.journal.current);
    expect(restarted.journal.cursors[0]).toEqual(cursor(2));
    await resumed.return();
  });

  test("preserves a typed stale-cursor reset with retention state", async () => {
    const { adapter, journal } = harness();
    journal.current = {
      schema: CLOUD_COMPUTER_COMMAND_STREAM_RESET_SCHEMA,
      kind: "stale_cursor",
      scope,
      commandStatus: "running",
      reason: "cursor_behind_retention",
      resetCursor: cursor(7, 3),
      retentionWatermark: 7,
      retentionEpoch: 3,
      terminal: null,
      artifacts: [],
    };
    const stream = adapter.open({ scope, cursor: cursor(2) });
    const reset = await stream.next();
    expect(reset.value).toBe(journal.current);
    expect((await stream.next()).done).toBe(true);
  });

  test("reads a terminal summary and artifacts after stream wakeups are lost", async () => {
    const { adapter, journal, poll } = harness();
    const stream = adapter.open({ scope, cursor: null });
    await stream.next();
    const next = stream.next();
    await Promise.resolve();
    await Promise.resolve();
    journal.current = page(1, [event(1)], {
      terminal: {
        terminalRef: "terminal.stream.six",
        terminalDigest: digest("a"),
        outcome: "completed",
        reason: "exit_zero",
        exitCode: 0,
        outputDigest: digest("c"),
        completedAt: "2026-08-22T12:00:02.000Z",
      },
      artifacts: [
        {
          artifactRef: "artifact.stream.stdout",
          kind: "stdout",
          contentDigest: digest("b"),
          byteCount: 6,
          retainUntil: "2026-08-23T12:00:00.000Z",
        },
      ],
    });
    poll.tick();
    const terminal = await next;
    expect(terminal.value).toEqual(journal.current);
    expect((terminal.value as CloudComputerCommandStreamPage).terminal?.outcome).toBe("completed");
    expect((await stream.next()).done).toBe(true);
  });

  test("rejects journal rows from another owner scope", async () => {
    const { adapter, journal } = harness();
    journal.current = page(0, [], {
      scope: { ...scope, ownerRef: "owner.stream.mallory" },
    });
    const stream = adapter.open({ scope, cursor: null });
    await expect(stream.next()).rejects.toMatchObject({ code: "invalid_journal" });
  });
});
