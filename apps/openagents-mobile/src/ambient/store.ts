import type { ShareInboxItem } from "./contracts";
import { decodeShareInboxItem } from "./contracts";

type SqliteBinding = string | number | null | Uint8Array;

export interface AmbientSqliteTransaction {
  readonly runAsync: (sql: string, ...params: ReadonlyArray<SqliteBinding>) => Promise<unknown>;
  readonly getAllAsync: <Row>(
    sql: string,
    ...params: ReadonlyArray<SqliteBinding>
  ) => Promise<Array<Row>>;
}

export interface AmbientSqliteDatabase extends AmbientSqliteTransaction {
  readonly execAsync: (sql: string) => Promise<void>;
  readonly withExclusiveTransactionAsync: (
    task: (transaction: AmbientSqliteTransaction) => Promise<void>,
  ) => Promise<void>;
}

export const initializeAmbientStore = async (database: AmbientSqliteDatabase): Promise<void> => {
  await database.execAsync(`
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = FULL;
    CREATE TABLE IF NOT EXISTS ambient_notification_receipts (
      notification_id TEXT PRIMARY KEY NOT NULL,
      processed_at_ms INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS ambient_share_inbox (
      intake_id TEXT PRIMARY KEY NOT NULL,
      received_at_ms INTEGER NOT NULL,
      raw TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS ambient_share_inbox_received
      ON ambient_share_inbox (received_at_ms DESC, intake_id DESC);
  `);
};

export const claimAmbientNotification = async (
  database: AmbientSqliteDatabase,
  notificationId: string,
  processedAt = Date.now(),
): Promise<boolean> => {
  let claimed = false;
  await database.withExclusiveTransactionAsync(async (transaction) => {
    const existing = await transaction.getAllAsync<{ readonly notification_id: string }>(
      "SELECT notification_id FROM ambient_notification_receipts WHERE notification_id = ?",
      notificationId,
    );
    if (existing.length > 0) return;
    await transaction.runAsync(
      "INSERT INTO ambient_notification_receipts (notification_id, processed_at_ms) VALUES (?, ?)",
      notificationId,
      processedAt,
    );
    claimed = true;
  });
  return claimed;
};

export const putShareInboxItem = async (
  database: AmbientSqliteDatabase,
  item: ShareInboxItem,
): Promise<void> => {
  const decoded = decodeShareInboxItem(item);
  await database.runAsync(
    "INSERT OR IGNORE INTO ambient_share_inbox (intake_id, received_at_ms, raw) VALUES (?, ?, ?)",
    decoded.intakeId,
    decoded.receivedAt,
    JSON.stringify(decoded),
  );
};

export const listShareInboxItems = async (
  database: AmbientSqliteDatabase,
  limit = 100,
): Promise<ReadonlyArray<ShareInboxItem>> => {
  const boundedLimit = Math.max(1, Math.min(200, Math.floor(limit)));
  const rows = await database.getAllAsync<{ readonly raw: string }>(
    "SELECT raw FROM ambient_share_inbox ORDER BY received_at_ms DESC, intake_id DESC LIMIT ?",
    boundedLimit,
  );
  return rows.map((row) => decodeShareInboxItem(JSON.parse(row.raw) as unknown));
};

export const deleteShareInboxItem = async (
  database: AmbientSqliteDatabase,
  intakeId: string,
): Promise<void> => {
  await database.runAsync("DELETE FROM ambient_share_inbox WHERE intake_id = ?", intakeId);
};
