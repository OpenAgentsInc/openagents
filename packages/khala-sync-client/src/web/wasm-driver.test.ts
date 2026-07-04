import { describe, expect, test } from "bun:test"
import {
  type SqliteWasmDbLike,
  type SqliteWasmExecOptions,
  sqliteWasmDriver,
} from "./wasm-driver.js"

/**
 * SqlDriver mapping onto the SQLite-WASM `oo1.DB` exec surface (KS-5.4),
 * verified against a recording fake handle. The SQL semantics themselves
 * are covered by the shared store suite (store-core.test.ts,
 * web-store.test.ts); this file pins the exec-option mapping and the
 * explicit BEGIN/COMMIT/ROLLBACK transaction contract.
 */

const createRecordingDb = (
  onExec?: (options: SqliteWasmExecOptions) => unknown,
): SqliteWasmDbLike & { readonly calls: Array<SqliteWasmExecOptions> } => {
  const calls: Array<SqliteWasmExecOptions> = []
  return {
    calls,
    exec: (options) => {
      calls.push(options)
      return onExec?.(options)
    },
  }
}

describe("sqliteWasmDriver", () => {
  test("run binds positional parameters; omits bind when empty", () => {
    const db = createRecordingDb()
    const driver = sqliteWasmDriver(db)
    driver.run("INSERT INTO t VALUES (?, ?)", ["a", 1])
    driver.run("DELETE FROM t")
    expect(db.calls[0]).toEqual({
      sql: "INSERT INTO t VALUES (?, ?)",
      bind: ["a", 1],
    })
    expect(db.calls[1]).toEqual({ sql: "DELETE FROM t" })
    expect("bind" in db.calls[1]!).toBe(false)
  })

  test("all requests object rows as the return value", () => {
    const rows = [{ version: 4 }]
    const db = createRecordingDb(() => rows)
    const driver = sqliteWasmDriver(db)
    const result = driver.all<{ version: number }>(
      "SELECT version FROM cursors WHERE scope = ?",
      ["scope.team.alpha"],
    )
    expect(result).toEqual(rows)
    expect(db.calls[0]).toEqual({
      sql: "SELECT version FROM cursors WHERE scope = ?",
      bind: ["scope.team.alpha"],
      rowMode: "object",
      returnValue: "resultRows",
    })
  })

  test("transaction commits on success and returns the callback value", () => {
    const db = createRecordingDb()
    const driver = sqliteWasmDriver(db)
    const result = driver.transaction(() => {
      driver.run("INSERT INTO t VALUES (1)")
      return "done"
    })
    expect(result).toBe("done")
    expect(db.calls.map((c) => c.sql)).toEqual([
      "BEGIN",
      "INSERT INTO t VALUES (1)",
      "COMMIT",
    ])
  })

  test("transaction rolls back and rethrows on failure", () => {
    const db = createRecordingDb()
    const driver = sqliteWasmDriver(db)
    expect(() =>
      driver.transaction(() => {
        driver.run("INSERT INTO t VALUES (1)")
        throw new Error("mid-batch failure")
      }),
    ).toThrow("mid-batch failure")
    expect(db.calls.map((c) => c.sql)).toEqual([
      "BEGIN",
      "INSERT INTO t VALUES (1)",
      "ROLLBACK",
    ])
  })

  test("a failing rollback does not mask the original error", () => {
    const db = createRecordingDb((options) => {
      if (options.sql === "ROLLBACK") throw new Error("rollback failed")
      return undefined
    })
    const driver = sqliteWasmDriver(db)
    expect(() =>
      driver.transaction(() => {
        throw new Error("original failure")
      }),
    ).toThrow("original failure")
  })
})
