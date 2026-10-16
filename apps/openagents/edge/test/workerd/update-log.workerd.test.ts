import { env, runInDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { CHUNK_BYTES } from "../../src/blobs";
import { appendUpdateRow, ensureUpdateLog, readUpdateRows } from "../../src/update-log";

/** Real-runtime twin of src/update-log.test.ts: same seams, but against an
 * actual SQLite-backed DO so the ~2MB value cap is the platform's, not a
 * FakeSql constant. A cap change or a chunking regression fails here first. */

// Each test gets its own DO (fresh storage) keyed by test name.
const inRoom = <T>(name: string, fn: (sql: SqlStorage) => T): Promise<T> => {
  const stub = env.TEST_LOG.get(env.TEST_LOG.idFromName(name));
  return runInDurableObject(stub, (_instance, state) => fn(state.storage.sql));
};

const bytesOf = (len: number, seed: number): Uint8Array => {
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) out[i] = (seed + i * 31) & 0xff;
  return out;
};

// No Buffer in workerd; plain loop stays fast enough for a few MB.
const sameBytes = (a: Uint8Array | undefined, b: Uint8Array): boolean => {
  if (a === undefined || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
};

describe("update log on real DO SQLite", () => {
  it("the platform still caps SQL values around 2MB (the assumption FakeSql hardcodes)", async () => {
    await inRoom("cap-probe", (sql) => {
      sql.exec("CREATE TABLE probe (bytes BLOB)");
      // A chunk-sized value must fit — CHUNK_BYTES exists to stay under the cap.
      sql.exec("INSERT INTO probe (bytes) VALUES (?)", bytesOf(CHUNK_BYTES, 1).buffer);
      // An unchunked whale-sized value must still be rejected; if this ever
      // passes, the cap moved and CHUNK_BYTES/this tier need revisiting.
      expect(() =>
        sql.exec("INSERT INTO probe (bytes) VALUES (?)", bytesOf(3 * CHUNK_BYTES, 2).buffer)
      ).toThrowError(/too big|toobig/i);
    });
  });

  it("a whale update above the row cap round-trips byte-identically", async () => {
    await inRoom("whale-roundtrip", (sql) => {
      ensureUpdateLog(sql);
      const whale = bytesOf(3 * CHUNK_BYTES + 123, 7);
      appendUpdateRow(sql, whale, 42);
      const back = [...readUpdateRows(sql)];
      expect(back.length).toBe(1);
      expect(sameBytes(back[0], whale)).toBe(true);
    });
  });

  it("keeps interleaved small and chunked updates in seq order", async () => {
    await inRoom("interleaved", (sql) => {
      ensureUpdateLog(sql);
      const updates = [bytesOf(10, 1), bytesOf(CHUNK_BYTES + 5, 2), bytesOf(20, 3)];
      for (const u of updates) appendUpdateRow(sql, u, 42);
      const back = [...readUpdateRows(sql)];
      expect(back.length).toBe(3);
      for (let i = 0; i < updates.length; i++) expect(sameBytes(back[i], updates[i]!)).toBe(true);
    });
  });

  it("migrates a legacy pre-cont table via real ALTER TABLE", async () => {
    await inRoom("legacy-migration", (sql) => {
      // The table exactly as created before the cont column existed.
      sql.exec(
        "CREATE TABLE updates (seq INTEGER PRIMARY KEY AUTOINCREMENT, bytes BLOB NOT NULL, received_at INTEGER NOT NULL)"
      );
      sql.exec("INSERT INTO updates (bytes, received_at) VALUES (?, ?)", bytesOf(10, 1).buffer, 1);
      sql.exec("INSERT INTO updates (bytes, received_at) VALUES (?, ?)", bytesOf(11, 2).buffer, 2);
      ensureUpdateLog(sql);
      // Legacy rows read back as one update each (ALTER's DEFAULT 0)…
      expect([...readUpdateRows(sql)].length).toBe(2);
      // …and post-migration chunked appends land on the migrated table.
      appendUpdateRow(sql, bytesOf(CHUNK_BYTES + 1, 3), 3);
      expect([...readUpdateRows(sql)].length).toBe(3);
      // Idempotence on the (now migrated) real table.
      ensureUpdateLog(sql);
      expect([...readUpdateRows(sql)].length).toBe(3);
    });
  });
});
