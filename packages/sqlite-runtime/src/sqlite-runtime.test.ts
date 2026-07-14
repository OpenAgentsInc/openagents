import { describe, expect, test } from "vite-plus/test"
import { Effect } from "effect"
import { sqliteDatabaseConformanceCases } from "./conformance.ts"
import { acquireSqliteDatabase, openSqliteDatabaseEffect } from "./effect.ts"
import { openSqliteDatabase } from "./open.ts"
import { detectSqliteRuntime, SqliteRuntimeError } from "./sqlite-database.ts"

/**
 * Bun half of the dual-runtime proof (BUN-1, openagents#8779): under
 * `bun test`, `openSqliteDatabase` must select the `bun:sqlite` client and
 * pass the full runtime-agnostic conformance suite. The Node half runs the
 * SAME conformance cases against the `node:sqlite` client under real
 * `node --test` (`node-database.node-suite.ts`) — honestly, not via a
 * mocked `process.versions.bun`.
 */

describe("sqlite-runtime under Bun", () => {
  test("detects the bun runtime", () => {
    expect(detectSqliteRuntime()).toBe("bun")
  })

  for (const conformanceCase of sqliteDatabaseConformanceCases(
    (path) => openSqliteDatabase(path),
    "bun",
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
