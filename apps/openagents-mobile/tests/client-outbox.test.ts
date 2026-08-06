import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test";
import {
  ClientCommandTransport,
  ClientOutboxStore,
  ObservationCacheEntry,
  OutboxTransportError,
  buildQueuedCommand,
  drainClientOutbox,
  enqueueClientCommand,
  projectObservation,
} from "@openagentsinc/client-command-outbox";
import { Effect, Layer } from "effect";
import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  initializeExpoSqliteOutbox,
  listExpoSqliteQuarantine,
  listExpoSqliteReceipts,
  makeExpoSqliteOutboxLayer,
  putExpoSqliteCommandAndDeleteObservation,
  readExpoObservationCache,
  writeExpoObservationCache,
  type ExpoSqliteDatabase,
  type ExpoSqliteTransaction,
} from "../src/outbox/expo-sqlite-outbox-store.ts";

const databases: Array<NodeTestDatabase> = [];

const openTestDatabase = (): ExpoSqliteDatabase => {
  const node = new NodeTestDatabase(":memory:");
  databases.push(node);
  const transaction = (database: NodeTestDatabase): ExpoSqliteTransaction => ({
    runAsync: async (sql, ...params) => database.run(sql, ...params),
    getAllAsync: async <Row>(
      sql: string,
      ...params: ReadonlyArray<string | number | null | Uint8Array>
    ) => database.query<Row>(sql).all(...params),
  });
  return {
    ...transaction(node),
    execAsync: async (sql) => node.exec(sql),
    withExclusiveTransactionAsync: async (task) => {
      node.exec("BEGIN IMMEDIATE");
      try {
        await task(transaction(node));
        node.exec("COMMIT");
      } catch (error) {
        node.exec("ROLLBACK");
        throw error;
      }
    },
  };
};

afterEach(() => {
  for (const database of databases.splice(0)) database.close();
});

describe("Expo SQLite client outbox", () => {
  it("survives relaunch and sends an airplane-mode command exactly once", async () => {
    const database = openTestDatabase();
    await initializeExpoSqliteOutbox(database);
    const storeLayer = makeExpoSqliteOutboxLayer(database);
    let attempts = 0;
    let effects = 0;
    const transportLayer = Layer.succeed(ClientCommandTransport, {
      send: (command) =>
        Effect.suspend(() => {
          attempts += 1;
          if (attempts === 1) {
            effects += 1;
            return Effect.fail(
              new OutboxTransportError({ detail: "Response lost after commit.", retryable: true }),
            );
          }
          return Effect.succeed({
            status: "duplicate" as const,
            receiptRef: `receipt.convex.${command.commandId}`,
          });
        }),
    });
    const layer = Layer.merge(storeLayer, transportLayer);
    await Effect.runPromise(
      enqueueClientCommand({
        commandId: "cmd-mobile-airplane",
        operation: "thread.message.send",
        orderingKey: "thread-demo",
        payload: { text: "Queued from airplane mode" },
        createdAtMs: 100,
      }).pipe(Effect.provide(layer)),
    );

    const afterRelaunch = makeExpoSqliteOutboxLayer(database);
    const reloadedLayer = Layer.merge(afterRelaunch, transportLayer);
    const pending = await Effect.runPromise(
      drainClientOutbox(
        { convexConnected: false, shellLive: true, decisionRevisions: {} },
        110,
      ).pipe(Effect.provide(reloadedLayer)),
    );
    expect(pending.pending).toBe(1);
    expect(attempts).toBe(0);

    await Effect.runPromise(
      drainClientOutbox(
        { convexConnected: true, shellLive: true, decisionRevisions: {} },
        120,
      ).pipe(Effect.provide(reloadedLayer)),
    );
    await Effect.runPromise(
      drainClientOutbox(
        { convexConnected: true, shellLive: true, decisionRevisions: {} },
        130,
      ).pipe(Effect.provide(reloadedLayer)),
    );
    await Effect.runPromise(
      drainClientOutbox(
        { convexConnected: true, shellLive: true, decisionRevisions: {} },
        140,
      ).pipe(Effect.provide(reloadedLayer)),
    );
    expect(attempts).toBe(2);
    expect(effects).toBe(1);
    expect(await listExpoSqliteReceipts(database)).toMatchObject([
      { commandId: "cmd-mobile-airplane", status: "duplicate" },
    ]);
  });

  it("quarantines a corrupt row atomically without blocking valid work", async () => {
    const database = openTestDatabase();
    await initializeExpoSqliteOutbox(database);
    const valid = buildQueuedCommand({
      commandId: "cmd-valid",
      operation: "thread.message.send",
      orderingKey: "thread-demo",
      payload: { text: "valid" },
      createdAtMs: 2,
    });
    await database.runAsync(
      "INSERT INTO client_command_outbox (command_id, ordering_key, created_at_ms, raw) VALUES (?, ?, ?, ?)",
      "cmd-corrupt",
      "thread-demo",
      1,
      "{ definitely not json",
    );
    await database.runAsync(
      "INSERT INTO client_command_outbox (command_id, ordering_key, created_at_ms, raw) VALUES (?, ?, ?, ?)",
      valid.commandId,
      valid.orderingKey,
      valid.createdAtMs,
      JSON.stringify(valid),
    );

    const store = makeExpoSqliteOutboxLayer(database);
    const commands = await Effect.runPromise(
      Effect.gen(function* () {
        const outbox = yield* ClientOutboxStore;
        return yield* outbox.list();
      }).pipe(Effect.provide(store)),
    );
    expect(commands.map((command) => command.commandId)).toEqual(["cmd-valid"]);
    expect(await listExpoSqliteQuarantine(database)).toMatchObject([
      { raw: expect.stringMatching(/^redacted:sha256:/u), reason: expect.any(String) },
    ]);
    expect(await listExpoSqliteReceipts(database)).toMatchObject([
      { commandId: "cmd-corrupt", status: "corrupt", code: "corrupt_persisted_command" },
    ]);
  });

  it("persists last-known observations with explicit freshness", async () => {
    const database = openTestDatabase();
    await initializeExpoSqliteOutbox(database);
    await writeExpoObservationCache(
      database,
      new ObservationCacheEntry({
        key: "shell",
        valueJson: '{"state":"running"}',
        observedAtMs: 100,
      }),
    );
    const entry = await readExpoObservationCache(database, "shell");
    expect(
      projectObservation({ entry, connected: false, synchronizing: false, nowMs: 175 }),
    ).toEqual({
      phase: "cached",
      ageMs: 75,
      value: { state: "running" },
    });
  });

  it("admits a message and clears its composer draft atomically", async () => {
    const database = openTestDatabase();
    await initializeExpoSqliteOutbox(database);
    await writeExpoObservationCache(
      database,
      new ObservationCacheEntry({
        key: "composer:thread:demo",
        valueJson: '{"text":"durable"}',
        observedAtMs: 100,
      }),
    );
    const command = buildQueuedCommand({
      commandId: "cmd-draft-atomic",
      operation: "thread.message.send",
      orderingKey: "thread:demo",
      payload: { text: "durable" },
      createdAtMs: 101,
    });
    await putExpoSqliteCommandAndDeleteObservation(database, command, "composer:thread:demo");
    expect(await readExpoObservationCache(database, "composer:thread:demo")).toBeNull();
    const rows = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* ClientOutboxStore;
        return yield* store.list();
      }).pipe(Effect.provide(makeExpoSqliteOutboxLayer(database))),
    );
    expect(rows.map((row) => row.commandId)).toEqual(["cmd-draft-atomic"]);
  });
});
