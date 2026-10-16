/**
 * Chunked update-log rows. Durable Object SQL caps individual values at ~2MB
 * (the cap blobs.ts chunks snapshots around) — but a single reassembled client
 * update can exceed it: a bulk session import or a host re-uploading a long
 * session's full missing span arrives as ONE Loro update of arbitrary size.
 * Before chunking, that INSERT threw SQLITE_TOOBIG on every flush forever
 * while the update had already been imported, acked Ok, and relayed — the room
 * stayed live but stopped persisting, and every hibernation silently reverted
 * it to stale state (the 2026-08-05 "newest messages no longer sync" bug).
 *
 * An update wider than CHUNK_BYTES is stored as consecutive rows, the first
 * with cont=0 and the rest with cont=1; replay reassembles by concatenating
 * each cont=0 row with its cont=1 followers. Rows written before the `cont`
 * column existed read back as cont=0 (ALTER's DEFAULT), i.e. one update per
 * row — exactly what they were.
 */
import { CHUNK_BYTES } from "./blobs";

/** Create the log table (or add `cont` to a pre-chunking one). */
export const ensureUpdateLog = (sql: SqlStorage): void => {
  sql.exec(
    "CREATE TABLE IF NOT EXISTS updates (seq INTEGER PRIMARY KEY AUTOINCREMENT, bytes BLOB NOT NULL, received_at INTEGER NOT NULL, cont INTEGER NOT NULL DEFAULT 0)"
  );
  try {
    sql.exec("ALTER TABLE updates ADD COLUMN cont INTEGER NOT NULL DEFAULT 0");
  } catch {
    /* column already exists (fresh table above, or already migrated) */
  }
};

/** Append one logical update as one or more rows, none above the row cap. */
export const appendUpdateRow = (sql: SqlStorage, update: Uint8Array, receivedAt: number): void => {
  for (let off = 0; off < update.byteLength; off += CHUNK_BYTES) {
    const end = Math.min(off + CHUNK_BYTES, update.byteLength);
    sql.exec(
      "INSERT INTO updates (bytes, received_at, cont) VALUES (?, ?, ?)",
      update.buffer.slice(update.byteOffset + off, update.byteOffset + end),
      receivedAt,
      off === 0 ? 0 : 1
    );
  }
};

/** Reassembled logical updates in seq order, one Uint8Array per update. */
export function* readUpdateRows(sql: SqlStorage): Generator<Uint8Array> {
  let parts: Uint8Array[] = [];
  const join = (group: Uint8Array[]): Uint8Array => {
    if (group.length === 1) return group[0]!;
    const out = new Uint8Array(group.reduce((n, p) => n + p.length, 0));
    let off = 0;
    for (const p of group) {
      out.set(p, off);
      off += p.length;
    }
    return out;
  };
  for (const row of sql.exec("SELECT bytes, cont FROM updates ORDER BY seq")) {
    const bytes = new Uint8Array(row.bytes as ArrayBuffer);
    if (!(row.cont as number) && parts.length > 0) {
      yield join(parts);
      parts = [];
    }
    parts.push(bytes);
  }
  if (parts.length > 0) yield join(parts);
}
