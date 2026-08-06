import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils";
import {
  ClientOutboxStore,
  CommandReceipt,
  ObservationCacheEntry,
  OutboxStorageError,
  QuarantinedCommand,
  QueuedCommand,
} from "@openagentsinc/client-command-outbox";
import { Effect, Layer, Schema as S } from "effect";

type SqliteBinding = string | number | null | Uint8Array;

export interface ExpoSqliteTransaction {
  readonly runAsync: (sql: string, ...params: ReadonlyArray<SqliteBinding>) => Promise<unknown>;
  readonly getAllAsync: <Row>(sql: string, ...params: ReadonlyArray<SqliteBinding>) => Promise<Array<Row>>;
}

export interface ExpoSqliteDatabase extends ExpoSqliteTransaction {
  readonly execAsync: (sql: string) => Promise<void>;
  readonly withExclusiveTransactionAsync: (task: (transaction: ExpoSqliteTransaction) => Promise<void>) => Promise<void>;
}

interface RawRow {
  readonly raw: string;
}

interface QuarantineRow {
  readonly quarantine_ref: string;
  readonly raw: string;
  readonly reason: string;
  readonly recorded_at_ms: number;
}

interface CacheRow {
  readonly cache_key: string;
  readonly value_json: string;
  readonly observed_at_ms: number;
}

const decodeJson = <Schema extends S.ConstraintDecoder<unknown, never>>(schema: Schema, raw: string): Schema["Type"] =>
  S.decodeUnknownSync(schema)(JSON.parse(raw) as unknown, { onExcessProperty: "error" });

const opaqueRaw = (raw: string): string => `redacted:sha256:${bytesToHex(sha256(utf8ToBytes(raw)))}`;

const storageFailure = (action: string, error: unknown): OutboxStorageError =>
  new OutboxStorageError({ action, error });

export const initializeExpoSqliteOutbox = async (database: ExpoSqliteDatabase): Promise<void> => {
  await database.execAsync(`
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = FULL;
    CREATE TABLE IF NOT EXISTS client_command_outbox (
      command_id TEXT PRIMARY KEY NOT NULL,
      ordering_key TEXT NOT NULL,
      created_at_ms INTEGER NOT NULL,
      raw TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS client_command_outbox_delivery
      ON client_command_outbox (created_at_ms, command_id);
    CREATE TABLE IF NOT EXISTS client_command_receipts (
      command_id TEXT PRIMARY KEY NOT NULL,
      recorded_at_ms INTEGER NOT NULL,
      raw TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS client_command_quarantine (
      quarantine_ref TEXT PRIMARY KEY NOT NULL,
      raw TEXT NOT NULL,
      reason TEXT NOT NULL,
      recorded_at_ms INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS client_observation_cache (
      cache_key TEXT PRIMARY KEY NOT NULL,
      value_json TEXT NOT NULL,
      observed_at_ms INTEGER NOT NULL
    );
  `);
};

export const makeExpoSqliteOutboxLayer = (database: ExpoSqliteDatabase): Layer.Layer<ClientOutboxStore> =>
  Layer.succeed(ClientOutboxStore, {
    put: (command) =>
      Effect.tryPromise({
        try: () =>
          database.withExclusiveTransactionAsync(async (transaction) => {
            const existing = await transaction.getAllAsync<RawRow>(
              "SELECT raw FROM client_command_outbox WHERE command_id = ?",
              command.commandId,
            );
            if (existing[0] !== undefined) {
              const persisted = decodeJson(QueuedCommand, existing[0].raw);
              if (persisted.fingerprint !== command.fingerprint) {
                throw new Error("A command ID cannot identify two fingerprints.");
              }
              return;
            }
            await transaction.runAsync(
              "INSERT INTO client_command_outbox (command_id, ordering_key, created_at_ms, raw) VALUES (?, ?, ?, ?)",
              command.commandId,
              command.orderingKey,
              command.createdAtMs,
              JSON.stringify(command),
            );
          }),
        catch: (error) => storageFailure("put", error),
      }),
    list: () =>
      Effect.tryPromise({
        try: async () => {
          const decoded: Array<QueuedCommand> = [];
          await database.withExclusiveTransactionAsync(async (transaction) => {
            const rows = await transaction.getAllAsync<RawRow & { readonly command_id: string }>(
              "SELECT command_id, raw FROM client_command_outbox ORDER BY created_at_ms, command_id",
            );
            for (const row of rows) {
              try {
                decoded.push(decodeJson(QueuedCommand, row.raw));
              } catch (error) {
                const recordedAtMs = Date.now();
                const quarantineRef = `quarantine.client.${row.command_id}.${recordedAtMs}`;
                const rawDigest = `sha256:${bytesToHex(sha256(utf8ToBytes(row.raw)))}`;
                await transaction.runAsync(
                  "INSERT INTO client_command_quarantine (quarantine_ref, raw, reason, recorded_at_ms) VALUES (?, ?, ?, ?)",
                  quarantineRef,
                  opaqueRaw(row.raw),
                  error instanceof Error ? error.message : "Invalid command row.",
                  recordedAtMs,
                );
                await transaction.runAsync(
                  "INSERT OR REPLACE INTO client_command_receipts (command_id, recorded_at_ms, raw) VALUES (?, ?, ?)",
                  row.command_id,
                  recordedAtMs,
                  JSON.stringify({
                    commandId: row.command_id,
                    fingerprint: rawDigest,
                    operation: "unknown",
                    status: "corrupt",
                    receiptRef: quarantineRef,
                    code: "corrupt_persisted_command",
                    detail: "The invalid command was quarantined and was not delivered.",
                    recordedAtMs,
                  }),
                );
                await transaction.runAsync("DELETE FROM client_command_outbox WHERE command_id = ?", row.command_id);
              }
            }
          });
          return decoded;
        },
        catch: (error) => storageFailure("list", error),
      }),
    remove: (commandId) =>
      Effect.tryPromise({
        try: () => database.runAsync("DELETE FROM client_command_outbox WHERE command_id = ?", commandId).then(() => undefined),
        catch: (error) => storageFailure("remove", error),
      }),
    recordReceipt: (receipt) =>
      Effect.tryPromise({
        try: () =>
          database
            .runAsync(
              "INSERT OR REPLACE INTO client_command_receipts (command_id, recorded_at_ms, raw) VALUES (?, ?, ?)",
              receipt.commandId,
              receipt.recordedAtMs,
              JSON.stringify(receipt),
            )
            .then(() => undefined),
        catch: (error) => storageFailure("record_receipt", error),
      }),
    quarantine: (entry) =>
      Effect.tryPromise({
        try: () =>
          database
            .runAsync(
              "INSERT OR REPLACE INTO client_command_quarantine (quarantine_ref, raw, reason, recorded_at_ms) VALUES (?, ?, ?, ?)",
              entry.quarantineRef,
              opaqueRaw(entry.raw),
              entry.reason,
              entry.recordedAtMs,
            )
            .then(() => undefined),
        catch: (error) => storageFailure("quarantine", error),
      }),
  });

export const listExpoSqliteReceipts = async (database: ExpoSqliteDatabase): Promise<ReadonlyArray<CommandReceipt>> => {
  const rows = await database.getAllAsync<RawRow>(
    "SELECT raw FROM client_command_receipts ORDER BY recorded_at_ms, command_id",
  );
  return rows.map((row) => decodeJson(CommandReceipt, row.raw));
};

export const listExpoSqliteQuarantine = async (
  database: ExpoSqliteDatabase,
): Promise<ReadonlyArray<QuarantinedCommand>> => {
  const rows = await database.getAllAsync<QuarantineRow>(
    "SELECT quarantine_ref, raw, reason, recorded_at_ms FROM client_command_quarantine ORDER BY recorded_at_ms, quarantine_ref",
  );
  return rows.map(
    (row) =>
      new QuarantinedCommand({
        quarantineRef: row.quarantine_ref,
        raw: row.raw,
        reason: row.reason,
        recordedAtMs: row.recorded_at_ms,
      }),
  );
};

export const writeExpoObservationCache = async (
  database: ExpoSqliteDatabase,
  entry: ObservationCacheEntry,
): Promise<void> => {
  await database.runAsync(
    "INSERT OR REPLACE INTO client_observation_cache (cache_key, value_json, observed_at_ms) VALUES (?, ?, ?)",
    entry.key,
    entry.valueJson,
    entry.observedAtMs,
  );
};

export const readExpoObservationCache = async (
  database: ExpoSqliteDatabase,
  key: string,
): Promise<ObservationCacheEntry | null> => {
  const rows = await database.getAllAsync<CacheRow>(
    "SELECT cache_key, value_json, observed_at_ms FROM client_observation_cache WHERE cache_key = ?",
    key,
  );
  const row = rows[0];
  return row === undefined
    ? null
    : new ObservationCacheEntry({ key: row.cache_key, valueJson: row.value_json, observedAtMs: row.observed_at_ms });
};
