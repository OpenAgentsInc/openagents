import assert from "node:assert/strict"
import { SqliteRuntimeError, type SqliteDatabase } from "./sqlite-database.ts"

/**
 * Runtime-agnostic conformance suite for {@link SqliteDatabase}
 * implementations. Uses `node:assert/strict` (available under both Bun and
 * Node), so the SAME cases run under `bun test` against the Bun client and
 * under `node --test` against the Node client — the dual-runtime proof the
 * seam exists to provide (BUN-1, openagents#8779).
 */

export interface SqliteDatabaseConformanceCase {
  readonly name: string
  readonly run: () => void
}

export const sqliteDatabaseConformanceCases = (
  open: (path: string) => SqliteDatabase,
  expectedRuntime: "bun" | "node",
): ReadonlyArray<SqliteDatabaseConformanceCase> => [
  {
    name: `reports runtime "${expectedRuntime}"`,
    run: () => {
      const db = open(":memory:")
      assert.equal(db.runtime, expectedRuntime)
      db.close()
    },
  },
  {
    name: "exec runs DDL and pragmas; run/all round-trip typed rows",
    run: () => {
      const db = open(":memory:")
      db.exec("PRAGMA foreign_keys = ON;")
      db.exec(
        "CREATE TABLE entries (id INTEGER PRIMARY KEY, name TEXT NOT NULL, blob BLOB, score REAL);",
      )
      db.run("INSERT INTO entries (name, blob, score) VALUES (?, ?, ?)", [
        "alpha",
        new Uint8Array([1, 2, 3]),
        1.5,
      ])
      db.run("INSERT INTO entries (name, blob, score) VALUES (?, ?, ?)", [
        "beta",
        null,
        2,
      ])
      const rows = db.all<{ id: number; name: string; score: number }>(
        "SELECT id, name, score FROM entries ORDER BY id",
      )
      assert.equal(rows.length, 2)
      assert.equal(rows[0]?.name, "alpha")
      assert.equal(rows[0]?.score, 1.5)
      assert.equal(rows[1]?.name, "beta")
      db.close()
    },
  },
  {
    name: "prepared statements are reused across calls with different binds",
    run: () => {
      const db = open(":memory:")
      db.exec("CREATE TABLE kv (k TEXT PRIMARY KEY, v TEXT NOT NULL);")
      for (let index = 0; index < 5; index += 1) {
        db.run("INSERT INTO kv (k, v) VALUES (?, ?)", [`k${index}`, `v${index}`])
      }
      const row = db.all<{ v: string }>("SELECT v FROM kv WHERE k = ?", ["k3"])
      assert.equal(row[0]?.v, "v3")
      db.close()
    },
  },
  {
    name: "transaction commits atomically on success",
    run: () => {
      const db = open(":memory:")
      db.exec("CREATE TABLE t (n INTEGER NOT NULL);")
      const result = db.transaction(() => {
        db.run("INSERT INTO t (n) VALUES (?)", [1])
        db.run("INSERT INTO t (n) VALUES (?)", [2])
        return "done"
      })
      assert.equal(result, "done")
      const rows = db.all<{ n: number }>("SELECT n FROM t ORDER BY n")
      assert.deepEqual(
        rows.map((row) => row.n),
        [1, 2],
      )
      db.close()
    },
  },
  {
    name: "transaction rolls back and rethrows on failure (byte-for-byte unchanged)",
    run: () => {
      const db = open(":memory:")
      db.exec("CREATE TABLE t (n INTEGER NOT NULL UNIQUE);")
      db.run("INSERT INTO t (n) VALUES (?)", [1])
      assert.throws(() =>
        db.transaction(() => {
          db.run("INSERT INTO t (n) VALUES (?)", [2])
          db.run("INSERT INTO t (n) VALUES (?)", [1])
        }),
      )
      const rows = db.all<{ n: number }>("SELECT n FROM t ORDER BY n")
      assert.deepEqual(
        rows.map((row) => row.n),
        [1],
      )
      db.close()
    },
  },
  {
    name: "nested transactions roll back the inner savepoint only",
    run: () => {
      const db = open(":memory:")
      db.exec("CREATE TABLE t (n INTEGER NOT NULL);")
      db.transaction(() => {
        db.run("INSERT INTO t (n) VALUES (?)", [1])
        assert.throws(() =>
          db.transaction(() => {
            db.run("INSERT INTO t (n) VALUES (?)", [2])
            throw new Error("inner failure")
          }),
        )
        db.run("INSERT INTO t (n) VALUES (?)", [3])
      })
      const rows = db.all<{ n: number }>("SELECT n FROM t ORDER BY n")
      assert.deepEqual(
        rows.map((row) => row.n),
        [1, 3],
      )
      db.close()
    },
  },
  {
    name: "open failure throws typed SqliteRuntimeError with reason open_failure",
    run: () => {
      assert.throws(
        () => open("/nonexistent-root-dir-for-sqlite-runtime/db.sqlite"),
        (error: unknown) =>
          error instanceof SqliteRuntimeError && error.reason === "open_failure",
      )
    },
  },
  {
    name: "close is terminal: later statements throw",
    run: () => {
      const db = open(":memory:")
      db.exec("CREATE TABLE t (n INTEGER);")
      db.close()
      assert.throws(() => db.all("SELECT 1 AS one"))
    },
  },
]
