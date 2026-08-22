import { describe, expect, test } from "vite-plus/test";

import { PostgresCloudComputerCommandJournal } from "./cloud-computer-command-postgres-journal.js";
import {
  CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  type CloudComputerCommandStreamCursor,
  type CloudComputerCommandStreamScope,
} from "./cloud-computer-command-stream.js";
import type { SyncSql } from "./sql.js";

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

const cursor = (sequence: number, retentionEpoch: number): CloudComputerCommandStreamCursor => ({
  schema: CLOUD_COMPUTER_COMMAND_STREAM_CURSOR_SCHEMA,
  sessionRef: scope.sessionRef,
  commandRef: scope.commandRef,
  runtimeGeneration: scope.runtimeGeneration,
  sequence,
  retentionEpoch,
});

type State = {
  command: Record<string, unknown>;
  events: Array<Record<string, unknown>>;
  artifacts: Array<Record<string, unknown>>;
};

const makeState = (): State => ({
  command: {
    owner_ref: scope.ownerRef,
    tenant_ref: scope.tenantRef,
    computer_ref: scope.computerRef,
    workspace_ref: scope.workspaceRef,
    session_ref: scope.sessionRef,
    command_ref: scope.commandRef,
    runtime_generation: scope.runtimeGeneration,
    retention_epoch: 2,
    next_command_sequence: 4,
    first_retained_command_sequence: 1,
    status: "running",
    terminal_ref: null,
    terminal_digest: null,
    terminal_reason: null,
    terminal_exit_code: null,
    terminal_output_digest: null,
    terminal_payload_json: null,
    terminal_command_sequence: null,
    completed_at: null,
  },
  events: [1, 2, 3].map((sequence) => ({
    event_ref: `event.stream.${sequence}`,
    command_sequence: sequence,
    event_digest: digest(String(sequence)),
    kind: sequence === 1 ? "accepted" : "stdout",
    payload_json: { text: `line ${sequence}` },
    artifact_refs_json: [],
    observed_at: new Date(Date.UTC(2026, 7, 22, 12, 0, sequence)),
  })),
  artifacts: [],
});

const makeSql = (durable: State): SyncSql => {
  const query = async (
    snapshot: State,
    strings: TemplateStringsArray,
    values: ReadonlyArray<unknown>,
  ): Promise<ReadonlyArray<Record<string, unknown>>> => {
    const text = strings.join("?");
    if (text.includes("FROM khala_sync_cloud_computer_commands AS command")) {
      const expected = [
        scope.ownerRef,
        scope.tenantRef,
        scope.computerRef,
        scope.workspaceRef,
        scope.sessionRef,
        scope.commandRef,
        scope.runtimeGeneration,
      ];
      return values.every((value, index) => value === expected[index]) ? [snapshot.command] : [];
    }
    if (text.includes("FROM khala_sync_cloud_computer_command_events")) {
      const [commandRef, after, limit] = values;
      if (commandRef !== scope.commandRef) return [];
      const selected = snapshot.events.filter(
        (event) => Number(event.command_sequence) > Number(after),
      );
      // eslint-disable-next-line unicorn/no-array-sort -- This package's TypeScript target lacks toSorted.
      selected.sort(
        (left, right) => Number(left.command_sequence) - Number(right.command_sequence),
      );
      return selected.slice(0, Number(limit));
    }
    if (text.includes("FROM khala_sync_cloud_computer_command_artifacts")) {
      return values[0] === scope.commandRef ? snapshot.artifacts : [];
    }
    throw new Error(`unexpected query: ${text}`);
  };

  const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) =>
    query(durable, strings, values)) as unknown as SyncSql;
  return Object.assign(sql, {
    begin: async <A>(_options: string, run: (tx: SyncSql) => Promise<A>): Promise<A> => {
      const snapshot = structuredClone(durable);
      const tx = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) =>
        query(snapshot, strings, values)) as unknown as SyncSql;
      return run(tx);
    },
  }) as SyncSql;
};

describe("Postgres cloud computer command journal", () => {
  test("pages by dense command sequence and returns only the requested scope", async () => {
    const state = makeState();
    const journal = new PostgresCloudComputerCommandJournal(makeSql(state));

    const first = await journal.read({ scope, cursor: cursor(0, 2), limit: 2 });
    expect(first).toMatchObject({
      kind: "page",
      cursor: { sequence: 2, retentionEpoch: 2 },
      hasMore: true,
      retentionWatermark: 0,
    });
    if (first.kind !== "page") throw new Error("expected page");
    expect(first.events.map((event) => event.sequence)).toEqual([1, 2]);
    expect(first.events[0]?.payload).toEqual({});
    expect(first.events[1]?.payload).toEqual({ text: "line 2" });

    const second = await journal.read({ scope, cursor: first.cursor, limit: 2 });
    expect(second).toMatchObject({ kind: "page", cursor: { sequence: 3 }, hasMore: false });
    if (second.kind !== "page") throw new Error("expected page");
    expect(second.events.map((event) => event.sequence)).toEqual([3]);
  });

  test("returns typed resets for compacted and prior-epoch cursors", async () => {
    const state = makeState();
    state.events.splice(0, 2);
    state.command.first_retained_command_sequence = 3;
    state.command.retention_epoch = 5;
    const journal = new PostgresCloudComputerCommandJournal(makeSql(state));

    await expect(journal.read({ scope, cursor: cursor(1, 5), limit: 10 })).resolves.toMatchObject({
      kind: "stale_cursor",
      reason: "cursor_behind_retention",
      retentionWatermark: 2,
      resetCursor: { sequence: 2, retentionEpoch: 5 },
    });
    await expect(journal.read({ scope, cursor: cursor(2, 4), limit: 10 })).resolves.toMatchObject({
      kind: "stale_cursor",
      reason: "retention_epoch_changed",
      resetCursor: { sequence: 2, retentionEpoch: 5 },
    });
  });

  test("withholds terminal evidence until the terminal page and includes durable artifacts", async () => {
    const state = makeState();
    state.command = {
      ...state.command,
      status: "completed",
      terminal_ref: "terminal.stream.six",
      terminal_digest: digest("a"),
      terminal_reason: "exit_zero",
      terminal_exit_code: 0,
      terminal_output_digest: digest("d"),
      terminal_payload_json: { reason: "exit_zero", outputDigest: digest("d") },
      terminal_command_sequence: 3,
      completed_at: "2026-08-22T12:00:03.000Z",
    };
    state.artifacts.push({
      artifact_ref: "artifact.stream.stdout",
      kind: "stdout",
      content_digest: digest("b"),
      byte_count: 12,
      retain_until: "2026-08-23T12:00:00.000Z",
    });
    const journal = new PostgresCloudComputerCommandJournal(makeSql(state));

    const first = await journal.read({ scope, cursor: cursor(0, 2), limit: 2 });
    expect(first).toMatchObject({ kind: "page", terminal: null, hasMore: true });
    if (first.kind !== "page") throw new Error("expected page");
    const terminal = await journal.read({ scope, cursor: first.cursor, limit: 2 });
    expect(terminal).toMatchObject({
      kind: "page",
      terminal: {
        outcome: "completed",
        reason: "exit_zero",
        exitCode: 0,
        outputDigest: digest("d"),
      },
      commandStatus: "completed",
      artifacts: [{ artifactRef: "artifact.stream.stdout", byteCount: 12 }],
    });
  });

  test("a fresh adapter resumes from the same durable SQL state", async () => {
    const state = makeState();
    const sql = makeSql(state);
    const first = new PostgresCloudComputerCommandJournal(sql);
    const initial = await first.read({ scope, cursor: cursor(0, 2), limit: 2 });
    if (initial.kind !== "page") throw new Error("expected page");

    state.events.push({
      event_ref: "event.stream.4",
      command_sequence: 4,
      event_digest: digest("4"),
      kind: "stdout",
      payload_json: { text: "line 4" },
      artifact_refs_json: [],
      observed_at: "2026-08-22T12:00:04.000Z",
    });
    state.command.next_command_sequence = 5;
    const restarted = new PostgresCloudComputerCommandJournal(sql);
    const resumed = await restarted.read({ scope, cursor: initial.cursor, limit: 10 });

    if (resumed.kind !== "page") throw new Error("expected page");
    expect(resumed.events.map((event) => event.sequence)).toEqual([3, 4]);
    expect(resumed.cursor.sequence).toBe(4);
  });

  test("does not let host-loss evidence skip retained event pages", async () => {
    const state = makeState();
    state.command = {
      ...state.command,
      status: "lost",
      terminal_ref: "terminal.stream.lost",
      terminal_digest: digest("c"),
      terminal_reason: "host_lost",
      terminal_exit_code: null,
      terminal_output_digest: null,
      terminal_payload_json: { reason: "host_lost", outputDigest: null },
      terminal_command_sequence: null,
      completed_at: "2026-08-22T12:00:05.000Z",
    };
    const journal = new PostgresCloudComputerCommandJournal(makeSql(state));

    const first = await journal.read({ scope, cursor: cursor(0, 2), limit: 2 });
    expect(first).toMatchObject({ kind: "page", hasMore: true, terminal: null });
    if (first.kind !== "page") throw new Error("expected page");
    await expect(journal.read({ scope, cursor: first.cursor, limit: 2 })).resolves.toMatchObject({
      kind: "page",
      hasMore: false,
      terminal: { outcome: "lost", reason: "host_lost" },
      commandStatus: "lost",
    });
  });

  test("fails closed if retained command events contain an omission", async () => {
    const state = makeState();
    state.events.splice(1, 1);
    const journal = new PostgresCloudComputerCommandJournal(makeSql(state));
    await expect(journal.read({ scope, cursor: cursor(0, 2), limit: 10 })).rejects.toMatchObject({
      code: "invalid_journal",
    });
  });

  test("rejects a scope that does not bind the durable command row", async () => {
    const journal = new PostgresCloudComputerCommandJournal(makeSql(makeState()));
    await expect(
      journal.read({
        scope: { ...scope, ownerRef: "owner.stream.mallory" },
        cursor: null,
        limit: 10,
      }),
    ).rejects.toMatchObject({ code: "invalid_input" });
  });

  test("does not project provider handles from durable event payloads", async () => {
    const state = makeState();
    state.events[0]!.payload_json = {
      reservationRef: "reservation.private.one",
      providerExecutionRef: "execution.private.one",
      rawProviderDocument: { address: "10.0.0.1" },
    };
    const journal = new PostgresCloudComputerCommandJournal(makeSql(state));
    const result = await journal.read({ scope, cursor: cursor(0, 2), limit: 1 });
    if (result.kind !== "page") throw new Error("expected page");
    expect(result.events[0]?.kind).toBe("accepted");
    expect(result.events[0]?.payload).toEqual({});
  });
});
