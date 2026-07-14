import { createRequire } from "node:module"
import {
  SqliteRuntimeError,
  type SqliteDatabase,
  type SqliteDatabaseOptions,
  type SqliteValue,
} from "./sqlite-database.ts"

/**
 * `node:sqlite` client for the dual-runtime seam (BUN-1, openagents#8779) —
 * the Node-side counterpart of `bun-database.ts`, modeled on T3 Code's
 * `NodeSqliteClient.ts` (a port of the Bun client onto the native
 * `node:sqlite` bindings; reference clone `projects/repos/t3code`).
 *
 * `node:sqlite` is resolved lazily via `createRequire`, so this module also
 * loads cleanly under a Bun typecheck/bundle without pulling the builtin.
 * The workspace carries no `@types/node`, so the builtin's surface is typed
 * structurally below with exactly the members this client touches; the
 * Node-path test suite (`node-database.node-suite.ts`, run under real
 * `node --test`) is what keeps these shapes honest against the runtime.
 *
 * Differences from `bun:sqlite` papered over here so both clients meet the
 * same {@link SqliteDatabase} contract:
 *
 * - `node:sqlite` has no `db.query` statement cache — a `Map` keyed by SQL
 *   string provides the same reuse (SQLite re-prepares internally on schema
 *   change, matching Bun's cache semantics).
 * - `node:sqlite` has no `db.transaction(fn)` helper — BEGIN/COMMIT/ROLLBACK
 *   with SAVEPOINT-based nesting reproduces Bun's rollback-and-rethrow
 *   contract.
 */

interface NodeStatementSyncLike {
  run(...params: ReadonlyArray<SqliteValue>): unknown
  all(...params: ReadonlyArray<SqliteValue>): unknown
}

interface NodeDatabaseSyncLike {
  exec(sql: string): void
  prepare(sql: string): NodeStatementSyncLike
  close(): void
}

interface NodeSqliteModuleLike {
  DatabaseSync: new (
    path: string,
    options?: { readonly readOnly?: boolean },
  ) => NodeDatabaseSyncLike
}

const requireModule = createRequire(import.meta.url)

const loadNodeSqlite = (): NodeSqliteModuleLike => {
  try {
    return requireModule("node:sqlite") as NodeSqliteModuleLike
  } catch (cause) {
    throw new SqliteRuntimeError(
      "unsupported_runtime",
      "node:sqlite is not available in this runtime",
      { cause },
    )
  }
}

export const openNodeSqliteDatabase = (
  path: string,
  options: SqliteDatabaseOptions = {},
): SqliteDatabase => {
  const { DatabaseSync } = loadNodeSqlite()
  let db: NodeDatabaseSyncLike
  try {
    db = new DatabaseSync(path, { readOnly: options.readonly ?? false })
  } catch (cause) {
    throw new SqliteRuntimeError(
      "open_failure",
      `failed to open sqlite database at ${path}`,
      { cause },
    )
  }

  const statements = new Map<string, NodeStatementSyncLike>()
  const prepare = (sql: string): NodeStatementSyncLike => {
    const cached = statements.get(sql)
    if (cached !== undefined) {
      return cached
    }
    const statement = db.prepare(sql)
    statements.set(sql, statement)
    return statement
  }

  let transactionDepth = 0
  const transaction = <A>(fn: () => A): A => {
    const depth = transactionDepth
    const savepoint = `sqlite_runtime_sp_${depth}`
    db.exec(depth === 0 ? "BEGIN" : `SAVEPOINT ${savepoint}`)
    transactionDepth = depth + 1
    try {
      const result = fn()
      transactionDepth = depth
      db.exec(depth === 0 ? "COMMIT" : `RELEASE SAVEPOINT ${savepoint}`)
      return result
    } catch (error) {
      transactionDepth = depth
      db.exec(
        depth === 0
          ? "ROLLBACK"
          : `ROLLBACK TO SAVEPOINT ${savepoint}; RELEASE SAVEPOINT ${savepoint}`,
      )
      throw error
    }
  }

  return {
    runtime: "node",
    exec: (sql) => db.exec(sql),
    run: (sql, params = []) => {
      prepare(sql).run(...params)
    },
    all: <Row>(sql: string, params: ReadonlyArray<SqliteValue> = []) =>
      prepare(sql).all(...params) as ReadonlyArray<Row>,
    transaction,
    close: () => {
      statements.clear()
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
