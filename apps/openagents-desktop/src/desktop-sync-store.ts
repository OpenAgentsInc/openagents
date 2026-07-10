import { createRequire } from "node:module"

import {
  createKhalaSyncStoreCore,
  localStoreFromCore,
  toKhalaSyncStoreError,
  type KhalaSyncLocalStore,
  type SqlDriver,
  type SqlValue,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

export type DesktopSqliteStatement = Readonly<{
  run: (...params: ReadonlyArray<SqlValue>) => unknown
  all: (...params: ReadonlyArray<SqlValue>) => ReadonlyArray<unknown>
}>

export type DesktopSqliteDatabase = Readonly<{
  exec: (sql: string) => void
  prepare: (sql: string) => DesktopSqliteStatement
  close: () => void
}>

export type DesktopSyncStore = KhalaSyncLocalStore & Readonly<{
  close: () => Effect.Effect<void, import("@openagentsinc/khala-sync-client").KhalaSyncClientStoreError>
}>

const openNodeDatabase = (databasePath: string): DesktopSqliteDatabase => {
  const nodeRequire = createRequire(import.meta.url)
  const sqlite = nodeRequire("node:sqlite") as {
    readonly DatabaseSync: new (path: string) => DesktopSqliteDatabase
  }
  return new sqlite.DatabaseSync(databasePath)
}

export const desktopSqliteDriver = (database: DesktopSqliteDatabase): SqlDriver => ({
  exec: sql => database.exec(sql),
  run: (sql, params = []) => {
    database.prepare(sql).run(...params)
  },
  all: <Row>(sql: string, params: ReadonlyArray<SqlValue> = []) =>
    database.prepare(sql).all(...params) as ReadonlyArray<Row>,
  transaction: fn => {
    database.exec("BEGIN IMMEDIATE")
    try {
      const result = fn()
      database.exec("COMMIT")
      return result
    } catch (error) {
      database.exec("ROLLBACK")
      throw error
    }
  },
})

export const openDesktopSyncStore = (
  databasePath: string,
  openDatabase: (path: string) => DesktopSqliteDatabase = openNodeDatabase,
): DesktopSyncStore => {
  let database: DesktopSqliteDatabase | undefined
  try {
    const openedDatabase = openDatabase(databasePath)
    database = openedDatabase
    openedDatabase.exec("PRAGMA journal_mode = WAL;")
    openedDatabase.exec("PRAGMA foreign_keys = ON;")
    const core = createKhalaSyncStoreCore(desktopSqliteDriver(openedDatabase))
    return {
      ...localStoreFromCore(core),
      close: () => Effect.try({ try: () => openedDatabase.close(), catch: toKhalaSyncStoreError }),
    }
  } catch (error) {
    try {
      database?.close()
    } catch {
      // Preserve the open/migration error as the actionable failure.
    }
    throw toKhalaSyncStoreError(error)
  }
}
