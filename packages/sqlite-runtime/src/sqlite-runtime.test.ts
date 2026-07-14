import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"
import { sqliteDatabaseConformanceCases } from "./conformance.ts"
import { acquireSqliteDatabase, openSqliteDatabaseEffect } from "./effect.ts"
import { openSqliteDatabase } from "./open.ts"
import { detectSqliteRuntime, SqliteRuntimeError } from "./sqlite-database.ts"

/**
 * Canonical Node half of the former dual-runtime proof (BUN-1,
 * openagents#8779). The retained Bun adapter remains a bounded rollback seam,
 * while Vite Plus exercises the production `node:sqlite` client.
 */

describe("sqlite-runtime under Node", () => {
  test("detects the node runtime", () => {
    expect(detectSqliteRuntime()).toBe("node")
  })

  for (const conformanceCase of sqliteDatabaseConformanceCases(
    (path) => openSqliteDatabase(path),
    "node",
  )) {
    test(conformanceCase.name, () => {
      conformanceCase.run()
    })
  }
})

describe("sqlite-runtime Effect surface", () => {
  test("openSqliteDatabaseEffect succeeds and yields a usable handle", () => {
    const db = Effect.runSync(openSqliteDatabaseEffect(":memory:"))
    db.exec("CREATE TABLE t (n INTEGER);")
    db.run("INSERT INTO t (n) VALUES (?)", [7])
    expect(db.all<{ n: number }>("SELECT n FROM t")[0]?.n).toBe(7)
    db.close()
  })

  test("openSqliteDatabaseEffect fails typed on an unopenable path", () => {
    const error = Effect.runSync(
      Effect.flip(
        openSqliteDatabaseEffect("/nonexistent-root-dir-for-sqlite-runtime/db.sqlite"),
      ),
    )
    expect(error).toBeInstanceOf(SqliteRuntimeError)
    expect(error.reason).toBe("open_failure")
  })

  test("acquireSqliteDatabase closes the handle when the scope closes", () => {
    const db = Effect.runSync(
      Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* acquireSqliteDatabase(":memory:")
          handle.exec("CREATE TABLE t (n INTEGER);")
          return handle
        }),
      ),
    )
    expect(() => db.all("SELECT 1 AS one")).toThrow()
  })
})
