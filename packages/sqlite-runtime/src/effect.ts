import { Effect, Scope } from "effect"
import { openSqliteDatabase } from "./open.ts"
import {
  SqliteRuntimeError,
  type SqliteDatabase,
  type SqliteDatabaseOptions,
} from "./sqlite-database.ts"

/**
 * Effect surface of the dual-runtime SQLite seam, matching how our stores
 * consume SQLite today (khala-sync-client): the database handle itself is
 * synchronous, and the Effect layer wraps lifecycle with typed errors —
 * `Effect.try` at open, acquire/release for scoped ownership (the shape T3
 * Code expresses with `Layer.unwrap` over its runtime-selected client).
 */

const toSqliteRuntimeError = (cause: unknown): SqliteRuntimeError =>
  cause instanceof SqliteRuntimeError
    ? cause
    : new SqliteRuntimeError("open_failure", "failed to open sqlite database", {
        cause,
      })

/** Open a database as an Effect with a typed {@link SqliteRuntimeError} channel. */
export const openSqliteDatabaseEffect = (
  path: string,
  options: SqliteDatabaseOptions = {},
): Effect.Effect<SqliteDatabase, SqliteRuntimeError> =>
  Effect.try({
    try: () => openSqliteDatabase(path, options),
    catch: toSqliteRuntimeError,
  })

/**
 * Scoped ownership: the database closes when the scope closes. A close
 * failure in the finalizer is a defect (mirrors T3's `orDie` close), not a
 * typed error — by finalization time there is no recovery story.
 */
export const acquireSqliteDatabase = (
  path: string,
  options: SqliteDatabaseOptions = {},
): Effect.Effect<SqliteDatabase, SqliteRuntimeError, Scope.Scope> =>
  Effect.acquireRelease(openSqliteDatabaseEffect(path, options), (db) =>
    Effect.sync(() => db.close()),
  )
