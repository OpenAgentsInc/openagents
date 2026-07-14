import { createRequire } from "node:module"
import {
  SqliteRuntimeError,
  type SqliteDatabase,
  type SqliteDatabaseOptions,
  type SqliteValue,
} from "./sqlite-database.ts"

/**
 * `bun:sqlite` client for the dual-runtime seam (BUN-1, openagents#8779).
 *
 * This module is one of exactly two places in production source allowed to
 * touch `bun:sqlite` (the other being its type import) — it is the named
 * Bun perimeter for embedded SQLite. The module itself is loadable under
 * Node: `bun:sqlite` is resolved lazily via `createRequire` only when a
 * database is actually opened, and `openSqliteDatabase` never routes here
 * off-Bun.
 *
 * `db.query(sql)` caches prepared statements by SQL string, so per-call
 * `query(...)` keeps statement reuse without a separate cache here (same
 * behavior the khala-sync-client store relied on before the seam).
 */

type BunSqliteModule = typeof import("bun:sqlite")

const requireModule = createRequire(import.meta.url)

const loadBunSqlite = (): BunSqliteModule => {
  try {
    return requireModule("bun:sqlite") as BunSqliteModule
  } catch (cause) {
    throw new SqliteRuntimeError(
      "unsupported_runtime",
      "bun:sqlite is not available in this runtime",
      { cause },
    )
  }
}

export const openBunSqliteDatabase = (
  path: string,
  options: SqliteDatabaseOptions = {},
): SqliteDatabase => {
  const { Database } = loadBunSqlite()
  let db: InstanceType<BunSqliteModule["Database"]>
  try {
    db = options.readonly
      ? new Database(path, { readonly: true })
      : new Database(path, { create: true })
  } catch (cause) {
    throw new SqliteRuntimeError(
      "open_failure",
      `failed to open sqlite database at ${path}`,
      { cause },
    )
  }

  return {
    runtime: "bun",
    exec: (sql) => db.exec(sql),
    run: (sql, params = []) => {
      db.query(sql).run(...(params as Array<SqliteValue>))
    },
    all: <Row>(sql: string, params: ReadonlyArray<SqliteValue> = []) =>
      db.query(sql).all(...(params as Array<SqliteValue>)) as ReadonlyArray<Row>,
    query: (sql) => db.query(sql) as unknown as import("./sqlite-database.ts").SqliteStatement,
    transaction: (fn) => db.transaction(fn)(),
    close: () => {
      try {
        db.close()
      } catch (cause) {
        throw new SqliteRuntimeError(
          "close_failure",
          `failed to close sqlite database at ${path}`,
          { cause },
        )
      }
    },
  }
}
