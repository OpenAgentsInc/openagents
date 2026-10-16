import { describe, expect, it } from "vitest";
import { LoroDoc } from "loro-crdt";
import { CHUNK_BYTES } from "./blobs";
import { appendUpdateRow, ensureUpdateLog, readUpdateRows } from "./update-log";

/** Minimal SqlStorage fake covering exactly the statements update-log.ts
 * issues, including the ~2MB row cap that motivated chunking. */
const ROW_CAP = 2 * 1024 * 1024;

class FakeSql {
  rows: { seq: number; bytes: ArrayBuffer; received_at: number; cont: number }[] = [];
  private seq = 0;
  private hasCont;

  constructor({ legacy = false } = {}) {
    // legacy=true simulates a table created before the `cont` column existed.
    this.hasCont = !legacy;
  }

  exec(query: string, ...params: unknown[]): Iterable<Record<string, unknown>> {
    if (query.startsWith("CREATE TABLE")) return [];
    if (query.startsWith("ALTER TABLE")) {
      if (this.hasCont) throw new Error("duplicate column name: cont");
      this.hasCont = true;
      for (const row of this.rows) row.cont = 0;
      return [];
    }
    if (query.startsWith("INSERT INTO updates")) {
      const bytes = params[0] as ArrayBuffer;
      if (bytes.byteLength > ROW_CAP) throw new Error("string or blob too big: SQLITE_TOOBIG");
      if (!this.hasCont && query.includes("cont")) throw new Error("table updates has no column named cont");
      this.rows.push({
        seq: ++this.seq,
        bytes,
        received_at: params[1] as number,
        cont: (params[2] as number) ?? 0
      });
      return [];
    }
    if (query.startsWith("SELECT bytes, cont FROM updates")) {
      return this.rows.map((r) => ({ bytes: r.bytes, cont: r.cont }));
    }
    if (query.startsWith("DELETE FROM updates")) {
      this.rows = [];
      return [];
    }
    throw new Error(`FakeSql: unhandled query: ${query}`);
  }
}

const asSql = (fake: FakeSql) => fake as unknown as SqlStorage;

const bytesOf = (len: number, seed: number): Uint8Array => {
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) out[i] = (seed + i * 31) & 0xff;
  return out;
};

const readAll = (fake: FakeSql): Uint8Array[] => [...readUpdateRows(asSql(fake))];

/** Byte-identical check that stays fast on multi-MB arrays (vitest's deep
 * equality diffing times out; a plain loop doesn't). No Node Buffer — the
 * tsconfig has workers types only. */
const sameBytes = (a: Uint8Array | undefined, b: Uint8Array): boolean => {
  if (a === undefined || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
};

describe("update log chunking", () => {
  it("stores a small update as a single non-continuation row", () => {
    const sql = new FakeSql();
    const update = bytesOf(1000, 1);
    appendUpdateRow(asSql(sql), update, 42);
    expect(sql.rows.length).toBe(1);
    expect(sql.rows[0]!.cont).toBe(0);
    expect(readAll(sql)).toEqual([update]);
  });

  it("splits an update above the row cap and reassembles it byte-identically", () => {
    const sql = new FakeSql();
    const whale = bytesOf(3 * CHUNK_BYTES + 123, 7); // >2 rows, ragged tail
    appendUpdateRow(asSql(sql), whale, 42);
    expect(sql.rows.length).toBe(4);
    expect(sql.rows.map((r) => r.cont)).toEqual([0, 1, 1, 1]);
    // The bug this guards: every row must fit under the SQL value cap.
    for (const row of sql.rows) expect(row.bytes.byteLength).toBeLessThanOrEqual(ROW_CAP);
    const back = readAll(sql);
    expect(back.length).toBe(1);
    expect(sameBytes(back[0], whale)).toBe(true);
  });

  it("keeps interleaved small and chunked updates in order", () => {
    const sql = new FakeSql();
    const a = bytesOf(10, 1);
    const b = bytesOf(CHUNK_BYTES + 5, 2);
    const c = bytesOf(20, 3);
    for (const u of [a, b, c]) appendUpdateRow(asSql(sql), u, 42);
    const back = readAll(sql);
    expect(back.length).toBe(3);
    expect(sameBytes(back[0], a)).toBe(true);
    expect(sameBytes(back[1], b)).toBe(true);
    expect(sameBytes(back[2], c)).toBe(true);
  });

  it("respects a subarray view's offset when slicing chunks", () => {
    const sql = new FakeSql();
    const backing = bytesOf(CHUNK_BYTES + 200, 9);
    const view = backing.subarray(100, CHUNK_BYTES + 150);
    appendUpdateRow(asSql(sql), view, 42);
    const back = readAll(sql);
    expect(back.length).toBe(1);
    expect(sameBytes(back[0], view)).toBe(true);
  });

  it("migrates a legacy table and reads its rows as one update each", () => {
    const sql = new FakeSql({ legacy: true });
    // Legacy rows written before the cont column existed.
    sql.rows.push(
      { seq: 1, bytes: bytesOf(10, 1).buffer as ArrayBuffer, received_at: 1, cont: 0 },
      { seq: 2, bytes: bytesOf(11, 2).buffer as ArrayBuffer, received_at: 2, cont: 0 }
    );
    ensureUpdateLog(asSql(sql));
    expect(readAll(sql).length).toBe(2);
    // And post-migration appends work, including chunked ones.
    appendUpdateRow(asSql(sql), bytesOf(CHUNK_BYTES + 1, 3), 3);
    expect(readAll(sql).length).toBe(3);
  });

  it("ensureUpdateLog is idempotent on an already-migrated table", () => {
    const sql = new FakeSql();
    ensureUpdateLog(asSql(sql));
    ensureUpdateLog(asSql(sql));
    appendUpdateRow(asSql(sql), bytesOf(10, 1), 1);
    expect(readAll(sql).length).toBe(1);
  });
});

describe("fold path over chunked rows", () => {
  // The session-room fold rides two seams of this module: a cold `ensureDoc`
  // replay imports each reassembled logical update (a chunked whale row group
  // must come back as ONE importable update — Loro rejects a partial blob),
  // and `foldLog` collapses the log into a snapshot with `DELETE FROM
  // updates` before appends resume. Exercise that full cycle with a real
  // LoroDoc, not just byte equality.
  it("replays a chunked whale update, folds it into a snapshot, and keeps appending", () => {
    const sql = new FakeSql();
    ensureUpdateLog(asSql(sql));

    // One commit big enough that its single update spans multiple rows.
    const writer = new LoroDoc();
    writer.getText("t").insert(0, "whale ".repeat(Math.ceil((2 * CHUNK_BYTES) / 6)));
    writer.commit();
    const whale = writer.export({ mode: "update" });
    expect(whale.byteLength).toBeGreaterThan(CHUNK_BYTES);
    appendUpdateRow(asSql(sql), whale, 1);
    expect(sql.rows.length).toBeGreaterThan(1);

    // Cold replay (ensureDoc): every reassembled update must import cleanly.
    const replayed = new LoroDoc();
    for (const update of readUpdateRows(asSql(sql))) replayed.import(update);
    expect(replayed.getText("t").toString() === writer.getText("t").toString()).toBe(true);

    // Fold (foldLog): snapshot the replayed doc, clear the log.
    const snapshot = replayed.export({ mode: "snapshot" });
    sql.exec("DELETE FROM updates");
    expect(readAll(sql).length).toBe(0);

    // Post-fold: new (chunked) appends land against the folded snapshot, and
    // the next cold replay converges to the full doc.
    const before = writer.version();
    writer.getText("t").insert(0, "post-fold ".repeat(Math.ceil(CHUNK_BYTES / 10)));
    writer.commit();
    appendUpdateRow(asSql(sql), writer.export({ mode: "update", from: before }), 2);
    const cold = new LoroDoc();
    cold.import(snapshot);
    for (const update of readUpdateRows(asSql(sql))) cold.import(update);
    expect(cold.getText("t").toString() === writer.getText("t").toString()).toBe(true);
  });
});
