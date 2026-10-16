import { env, runInDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { createBlobStore } from "../../src/blobs";
import {
  appendRow,
  CHECKPOINT_BLOB,
  commitCheckpoint,
  ensureChatLog,
  FRONTIER_BLOB,
  headSeq,
  logStats,
  MAX_ROW_BYTES,
  rowsAfter,
  seqFloor
} from "../../src/chat-log";

/** chat2 log model (docs/chat2-sync.md B) against real DO SQLite — the same
 * runtime whose ~2MB value cap sank s2's unchunked whale rows. chat2 rows are
 * capped at 1MB by design, so the platform cap must never be reachable. */

const inRoom = <T>(name: string, fn: (sql: SqlStorage) => T): Promise<T> => {
  const stub = env.TEST_LOG.get(env.TEST_LOG.idFromName(`chat-${name}`));
  return runInDurableObject(stub, (_instance, state) => {
    ensureChatLog(state.storage.sql);
    return fn(state.storage.sql);
  });
};

const bytesOf = (len: number, seed: number): Uint8Array => {
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) out[i] = (seed + i * 31) & 0xff;
  return out;
};

const sameBytes = (a: Uint8Array | undefined, b: Uint8Array): boolean => {
  if (a === undefined || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
};

describe("chat2 log on real DO SQLite", () => {
  it("appends assign dense seqs and read back byte-identical", async () => {
    await inRoom("append", (sql) => {
      const a = appendRow(sql, "dev-a", "b1", bytesOf(1000, 1), 10);
      const b = appendRow(sql, "dev-b", "b2", bytesOf(100_000, 2), 11);
      expect(a).toEqual({ ok: true, seq: 1, dup: false });
      expect(b).toEqual({ ok: true, seq: 2, dup: false });
      const rows = [...rowsAfter(sql, 0)];
      expect(rows.map((r) => [r.seq, r.device, r.batchId])).toEqual([
        [1, "dev-a", "b1"],
        [2, "dev-b", "b2"]
      ]);
      expect(sameBytes(rows[1]!.bytes, bytesOf(100_000, 2))).toBe(true);
      expect(headSeq(sql)).toBe(2);
    });
  });

  it("batchId replay dedupes to the original seq without appending", async () => {
    await inRoom("dedupe", (sql) => {
      appendRow(sql, "dev-a", "b1", bytesOf(10, 1), 10);
      // The reconnect re-push path: same batch, same bytes, later clock.
      const replay = appendRow(sql, "dev-a", "b1", bytesOf(10, 1), 99);
      expect(replay).toEqual({ ok: true, seq: 1, dup: true });
      expect(headSeq(sql)).toBe(1);
      expect([...rowsAfter(sql, 0)].length).toBe(1);
    });
  });

  it("caps rows at 1MB — comfortably under the platform's ~2MB value cap", async () => {
    await inRoom("cap", (sql) => {
      expect(appendRow(sql, "d", "big", bytesOf(MAX_ROW_BYTES + 1, 3), 10)).toEqual({
        ok: false,
        error: "too_large"
      });
      expect(appendRow(sql, "d", "empty", new Uint8Array(0), 10)).toEqual({
        ok: false,
        error: "empty"
      });
      // A max-size row must actually fit in real SQLite (the cap's headroom
      // claim, proven on the platform rather than assumed).
      expect(appendRow(sql, "d", "max", bytesOf(MAX_ROW_BYTES, 4), 10).ok).toBe(true);
      expect(sameBytes([...rowsAfter(sql, 0)][0]!.bytes, bytesOf(MAX_ROW_BYTES, 4))).toBe(true);
    });
  });

  it("rowsAfter honors the cursor and the exclude-own-device reconnect path", async () => {
    await inRoom("cursor", (sql) => {
      appendRow(sql, "dev-a", "b1", bytesOf(8, 1), 10);
      appendRow(sql, "dev-b", "b2", bytesOf(8, 2), 11);
      appendRow(sql, "dev-a", "b3", bytesOf(8, 3), 12);
      expect([...rowsAfter(sql, 1)].map((r) => r.seq)).toEqual([2, 3]);
      // A reconnecting dev-a never re-downloads its own offline writes.
      expect([...rowsAfter(sql, 0, "dev-a")].map((r) => r.batchId)).toEqual(["b2"]);
    });
  });

  it("checkpoint commit prunes covered rows and stores blob + frontier", async () => {
    await inRoom("checkpoint", (sql) => {
      const blobs = createBlobStore(sql);
      for (let i = 1; i <= 5; i++) appendRow(sql, "dev-a", `b${i}`, bytesOf(50, i), i);
      const checkpoint = bytesOf(200_000, 9);
      const frontier = bytesOf(40, 8);
      const outcome = commitCheckpoint(sql, blobs, 3, frontier, checkpoint, 1000);
      expect(outcome).toEqual({ ok: true, seqFloor: 3, pruned: 3 });
      expect([...rowsAfter(sql, 0)].map((r) => r.seq)).toEqual([4, 5]);
      expect(seqFloor(sql)).toBe(3);
      expect(sameBytes(blobs.get(CHECKPOINT_BLOB), checkpoint)).toBe(true);
      expect(sameBytes(blobs.get(FRONTIER_BLOB), frontier)).toBe(true);
      const stats = logStats(sql);
      expect(stats.checkpointSeq).toBe(3);
      expect(stats.checkpointSize).toBe(200_000);
      expect(stats.rowCount).toBe(2);
    });
  });

  it("checkpoint guard is floor-monotonic and head-bounded", async () => {
    await inRoom("guard", (sql) => {
      const blobs = createBlobStore(sql);
      for (let i = 1; i <= 4; i++) appendRow(sql, "dev-a", `b${i}`, bytesOf(50, i), i);
      expect(commitCheckpoint(sql, blobs, 3, bytesOf(4, 1), bytesOf(100, 1), 1000).ok).toBe(true);
      // Regressing the floor (a stale device posting an old rebuild) is
      // rejected — the dumb replacement for s2's VV-monotonic R2 guard.
      expect(commitCheckpoint(sql, blobs, 2, bytesOf(4, 2), bytesOf(100, 2), 2000)).toEqual({
        ok: false,
        error: "floor_regression"
      });
      // Claiming rows that don't exist yet is rejected.
      expect(commitCheckpoint(sql, blobs, 99, bytesOf(4, 3), bytesOf(100, 3), 3000)).toEqual({
        ok: false,
        error: "ahead_of_head"
      });
      // A rejected commit must not clobber the stored blob (the guard runs
      // BEFORE the write).
      expect(sameBytes(blobs.get(CHECKPOINT_BLOB), bytesOf(100, 1))).toBe(true);
      // Same-floor re-post (the benign two-identical-rebuilds race in M5) wins.
      expect(commitCheckpoint(sql, blobs, 3, bytesOf(4, 4), bytesOf(100, 4), 4000).ok).toBe(true);
      expect(sameBytes(blobs.get(CHECKPOINT_BLOB), bytesOf(100, 4))).toBe(true);
    });
  });

  it("a churn cycle stays bounded: push → checkpoint → push never accumulates", async () => {
    // The exit criterion shape from the plan (churn_stays_bounded): N rounds
    // of streaming with periodic checkpoints leave rows ≤ one round's worth.
    await inRoom("churn", (sql) => {
      const blobs = createBlobStore(sql);
      let pushed = 0;
      for (let round = 0; round < 10; round++) {
        for (let i = 0; i < 20; i++) {
          pushed += 1;
          appendRow(sql, "dev-a", `r${round}-${i}`, bytesOf(2000, pushed), pushed);
        }
        commitCheckpoint(sql, blobs, headSeq(sql), bytesOf(8, round), bytesOf(50_000, round), round);
        expect(logStats(sql).rowCount).toBe(0);
      }
      expect(headSeq(sql)).toBe(200);
      expect(seqFloor(sql)).toBe(200);
    });
  });
});
