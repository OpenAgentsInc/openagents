import { openNodeSqliteDatabase } from "./node-database.ts"
import {
  type LegacySqliteDatabase,
  type SqliteDatabase,
  type SqliteDatabaseOptions,
} from "./sqlite-database.ts"

/**
 * Open (or create) an embedded SQLite database at `path` — a filesystem
 * path or `":memory:"` — on whichever runtime client matches the current
 * process (`node:sqlite` under the pinned Node runtime).
 *
 * Synchronous, like both underlying bindings; throws
 * {@link import("./sqlite-database.ts").SqliteRuntimeError} if the database
 * cannot be opened. Callers that want an Effect surface use `effect.ts`.
 */
export const openSqliteDatabase = (
  path: string,
  options: SqliteDatabaseOptions = {},
): SqliteDatabase => openNodeSqliteDatabase(path, options)

export const openLegacySqliteDatabase = (
  path: string,
  options: SqliteDatabaseOptions = {},
): LegacySqliteDatabase => {
  const database = openSqliteDatabase(path, options)
  return {
    exec: database.exec,
    run: (sql, ...params) => database.query(sql).run(...params),
    query: <Row = any, Params extends Array<any> = Array<any>>(sql: string) => database.query<Row, Params>(sql),
    transaction: <A>(fn: () => A) => {
      const run = () => database.transaction(fn)
      return Object.assign(run, { immediate: run })
    },
    close: database.close,
  }
}
