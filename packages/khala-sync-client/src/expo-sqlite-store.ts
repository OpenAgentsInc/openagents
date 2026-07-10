import { Effect } from "effect"
import {
  createKhalaSyncStoreCore,
  localStoreFromCore,
  type SqlDriver,
  type SqlValue,
  toKhalaSyncStoreError,
} from "./store-core.js"
import {
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "./store.js"

/**
 * The synchronous subset of Expo SQLite used by Khala Sync. Keeping this
 * structural avoids loading a native Expo module from the shared package's
 * Bun and web entry points; the mobile host injects `openDatabaseSync`.
 */
export interface ExpoSqliteDatabase {
  readonly execSync: (sql: string) => void
  readonly runSync: (sql: string, ...params: ReadonlyArray<SqlValue>) => unknown
  readonly getAllSync: <Row>(
    sql: string,
    ...params: ReadonlyArray<SqlValue>
  ) => ReadonlyArray<Row>
  readonly withTransactionSync: (task: () => void) => void
  readonly closeSync: () => void
}

export interface KhalaSyncExpoSqliteStore extends KhalaSyncLocalStore {
  readonly close: () => Effect.Effect<void, KhalaSyncClientStoreError>
}

/** Thin driver only; every Sync semantic remains in `store-core.ts`. */
export const expoSqliteDriver = (database: ExpoSqliteDatabase): SqlDriver => ({
  exec: sql => database.execSync(sql),
  run: (sql, params = []) => {
    database.runSync(sql, ...params)
  },
  all: <Row>(sql: string, params: ReadonlyArray<SqlValue> = []) =>
    database.getAllSync<Row>(sql, ...params),
  transaction: <A>(fn: () => A): A => {
    let result: A | undefined
    database.withTransactionSync(() => {
      result = fn()
    })
    return result as A
  },
})

/**
 * Open a host-owned Expo SQLite store. The caller owns the database name and
 * native module import; initialization failure always closes an opened handle.
 */
export const openExpoKhalaSyncStore = (
  databaseName: string,
  openDatabase: (name: string) => ExpoSqliteDatabase,
): KhalaSyncExpoSqliteStore => {
  let database: ExpoSqliteDatabase | undefined
  try {
    const openedDatabase = openDatabase(databaseName)
    database = openedDatabase
    openedDatabase.execSync("PRAGMA journal_mode = WAL;")
    openedDatabase.execSync("PRAGMA foreign_keys = ON;")
    const core = createKhalaSyncStoreCore(expoSqliteDriver(openedDatabase))
    return {
      ...localStoreFromCore(core),
      close: () => Effect.try({
        try: () => openedDatabase.closeSync(),
        catch: toKhalaSyncStoreError,
      }),
    }
  } catch (error) {
    try {
      database?.closeSync()
    } catch {
      // Preserve the open/migration failure as the actionable typed error.
    }
    throw new KhalaSyncClientStoreError(
      "storage_failure",
      "failed to open khala-sync Expo SQLite store",
      { cause: error },
    )
  }
}
