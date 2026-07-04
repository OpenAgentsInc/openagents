import type { SqlDriver, SqlValue } from "../store-core.js"

/**
 * {@link SqlDriver} over a SQLite-WASM `oo1.DB`-style handle (KS-5.4) —
 * the `OpfsSAHPoolDb` the storage worker opens on the `opfs-sahpool` VFS.
 *
 * The handle is typed structurally ({@link SqliteWasmDbLike}: just
 * `exec`) so the mapping is unit-testable in bun with a recording fake,
 * and so this module never imports `@sqlite.org/sqlite-wasm` (that import
 * lives only in the worker entry, behind the `./web/worker` subpath).
 *
 * Transactions are explicit BEGIN/COMMIT/ROLLBACK rather than
 * `oo1.DB.transaction` — the store core needs the callback's return value
 * and rethrow-after-rollback, which the explicit form guarantees
 * independent of oo1 API details.
 */

export interface SqliteWasmExecOptions {
  readonly sql: string
  readonly bind?: ReadonlyArray<SqlValue>
  readonly rowMode?: "object"
  readonly returnValue?: "resultRows"
}

/** Structural slice of `sqlite3.oo1.DB` (and `OpfsSAHPoolDb`). */
export interface SqliteWasmDbLike {
  readonly exec: (options: SqliteWasmExecOptions) => unknown
}

export const sqliteWasmDriver = (db: SqliteWasmDbLike): SqlDriver => {
  const exec = (sql: string): void => {
    db.exec({ sql })
  }
  return {
    exec,
    run: (sql, params = []) => {
      db.exec({ sql, ...(params.length > 0 ? { bind: params } : {}) })
    },
    all: <Row>(sql: string, params: ReadonlyArray<SqlValue> = []) =>
      db.exec({
        sql,
        ...(params.length > 0 ? { bind: params } : {}),
        rowMode: "object",
        returnValue: "resultRows",
      }) as ReadonlyArray<Row>,
    transaction: (fn) => {
      exec("BEGIN")
      try {
        const result = fn()
        exec("COMMIT")
        return result
      } catch (error) {
        try {
          exec("ROLLBACK")
        } catch {
          // Rollback is best-effort; the original failure is the truth.
        }
        throw error
      }
    },
  }
}
