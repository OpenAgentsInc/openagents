import { openBunSqliteDatabase } from "./bun-database.ts"
import { openNodeSqliteDatabase } from "./node-database.ts"
import {
  detectSqliteRuntime,
  type SqliteDatabase,
  type SqliteDatabaseOptions,
} from "./sqlite-database.ts"

/**
 * Open (or create) an embedded SQLite database at `path` — a filesystem
 * path or `":memory:"` — on whichever runtime client matches the current
 * process (`bun:sqlite` under Bun, `node:sqlite` under Node).
 *
 * Synchronous, like both underlying bindings; throws
 * {@link import("./sqlite-database.ts").SqliteRuntimeError} if the database
 * cannot be opened. Callers that want an Effect surface use `effect.ts`.
 */
export const openSqliteDatabase = (
  path: string,
  options: SqliteDatabaseOptions = {},
): SqliteDatabase =>
  detectSqliteRuntime() === "bun"
    ? openBunSqliteDatabase(path, options)
    : openNodeSqliteDatabase(path, options)
