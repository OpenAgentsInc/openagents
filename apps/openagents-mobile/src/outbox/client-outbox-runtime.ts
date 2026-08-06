import {
  ClientCommandTransport,
  ObservationCacheEntry,
  OutboxTransportError,
  authorizeImmediateCommand,
  buildQueuedCommand,
  drainClientOutbox,
  enqueueClientCommand,
  projectObservation,
  type DeliveryGate,
  type DestructiveGitAuthorization,
  type EnqueueInput,
  type ObservationProjection,
  type QueuedCommand,
  type TransportResult,
} from "@openagentsinc/client-command-outbox";
import { Effect, Layer } from "effect";
import { openDatabaseAsync, type SQLiteDatabase } from "expo-sqlite";

import {
  initializeExpoSqliteOutbox,
  deleteExpoObservationCache,
  listExpoSqliteQuarantine,
  listExpoSqliteReceipts,
  makeExpoSqliteOutboxLayer,
  putExpoSqliteCommandAndDeleteObservation,
  readExpoObservationCache,
  writeExpoObservationCache,
  type ExpoSqliteDatabase,
  type ExpoSqliteTransaction,
} from "./expo-sqlite-outbox-store";

export interface MobileCommandTransport {
  readonly send: (command: QueuedCommand) => Promise<TransportResult>;
}

const adaptTransaction = (database: SQLiteDatabase): ExpoSqliteTransaction => ({
  runAsync: (sql, ...params) => database.runAsync(sql, ...params),
  getAllAsync: (sql, ...params) => database.getAllAsync(sql, ...params),
});

const adaptDatabase = (database: SQLiteDatabase): ExpoSqliteDatabase => ({
  ...adaptTransaction(database),
  execAsync: (sql) => database.execAsync(sql),
  withExclusiveTransactionAsync: (task) =>
    database.withExclusiveTransactionAsync((transaction) => task(adaptTransaction(transaction))),
});

export class MobileClientOutboxRuntime {
  readonly #database: SQLiteDatabase;
  readonly #adapter: ExpoSqliteDatabase;
  readonly #storeLayer: ReturnType<typeof makeExpoSqliteOutboxLayer>;

  constructor(database: SQLiteDatabase) {
    this.#database = database;
    this.#adapter = adaptDatabase(database);
    this.#storeLayer = makeExpoSqliteOutboxLayer(this.#adapter);
  }

  enqueue(input: EnqueueInput) {
    return Effect.runPromise(enqueueClientCommand(input).pipe(Effect.provide(this.#storeLayer)));
  }

  async enqueueAndClearObservation(input: EnqueueInput, key: string) {
    const command = buildQueuedCommand(input);
    await putExpoSqliteCommandAndDeleteObservation(this.#adapter, command, key);
    return command;
  }

  drain(transport: MobileCommandTransport, gate: DeliveryGate, nowMs = Date.now()) {
    const transportLayer = Layer.succeed(ClientCommandTransport, {
      send: (command) =>
        Effect.tryPromise({
          try: () => transport.send(command),
          catch: (error) =>
            new OutboxTransportError({
              detail: error instanceof Error ? error.message : "Command transport failed.",
              retryable: true,
            }),
        }),
    });
    return Effect.runPromise(
      drainClientOutbox(gate, nowMs).pipe(
        Effect.provide(Layer.merge(this.#storeLayer, transportLayer)),
      ),
    );
  }

  authorizeImmediate(input: {
    readonly operation: string;
    readonly online: boolean;
    readonly destructiveGit?: DestructiveGitAuthorization;
  }): void {
    authorizeImmediateCommand(input);
  }

  async receipts() {
    return listExpoSqliteReceipts(this.#adapter);
  }

  async quarantine() {
    return listExpoSqliteQuarantine(this.#adapter);
  }

  async cacheObservation(key: string, value: unknown, observedAtMs = Date.now()): Promise<void> {
    await writeExpoObservationCache(
      this.#adapter,
      new ObservationCacheEntry({ key, valueJson: JSON.stringify(value), observedAtMs }),
    );
  }

  async observation(input: {
    readonly key: string;
    readonly connected: boolean;
    readonly synchronizing: boolean;
    readonly nowMs?: number;
  }): Promise<ObservationProjection | null> {
    const entry = await readExpoObservationCache(this.#adapter, input.key);
    return projectObservation({
      entry,
      connected: input.connected,
      synchronizing: input.synchronizing,
      nowMs: input.nowMs ?? Date.now(),
    });
  }

  async removeObservation(key: string): Promise<void> {
    await deleteExpoObservationCache(this.#adapter, key);
  }

  async close(): Promise<void> {
    await this.#database.closeAsync();
  }
}

export const openMobileClientOutboxRuntime = async (): Promise<MobileClientOutboxRuntime> => {
  const database = await openDatabaseAsync("openagents-client-outbox.db");
  const adapter = adaptDatabase(database);
  try {
    await initializeExpoSqliteOutbox(adapter);
    return new MobileClientOutboxRuntime(database);
  } catch (error) {
    await database.closeAsync();
    throw error;
  }
};
