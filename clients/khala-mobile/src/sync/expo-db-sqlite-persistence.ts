export const KHALA_MOBILE_SYNC_DB_NAME = "khala-mobile-sync.db"
export const KHALA_MOBILE_PERSISTENCE_KIND = "expo-db-sqlite-persistence"

export type ExpoSqliteDatabase = Readonly<{
  execAsync: (statement: string) => Promise<void>
  getFirstAsync: <T>(statement: string, ...params: ReadonlyArray<unknown>) => Promise<T | null>
  runAsync: (statement: string, ...params: ReadonlyArray<unknown>) => Promise<unknown>
}>

export type ExpoSqliteModule = Readonly<{
  openDatabaseAsync: (name: string) => Promise<ExpoSqliteDatabase>
}>

export type KhalaMobileSyncCheckpoint = Readonly<{
  scope: string
  cursor: number
  updatedAt: string
}>

export type KhalaMobileSqlitePersistence = Readonly<{
  kind: typeof KHALA_MOBILE_PERSISTENCE_KIND
  databaseName: string
  db: ExpoSqliteDatabase
  readCheckpoint: (scope: string) => Promise<KhalaMobileSyncCheckpoint | null>
  saveCheckpoint: (checkpoint: KhalaMobileSyncCheckpoint) => Promise<void>
  clearScope: (scope: string) => Promise<void>
}>

const loadExpoSqlite = async (): Promise<ExpoSqliteModule> =>
  (await import("expo-sqlite")) as ExpoSqliteModule

export const KHALA_MOBILE_SQLITE_SCHEMA = `
CREATE TABLE IF NOT EXISTS khala_sync_checkpoints (
  scope TEXT PRIMARY KEY NOT NULL,
  cursor INTEGER NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS khala_sync_projection_cache (
  scope TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  post_image_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (scope, entity_type, entity_id)
);
`

export const openKhalaMobileSqlitePersistence = async (
  input: {
    readonly databaseName?: string
    readonly sqliteLoader?: () => Promise<ExpoSqliteModule>
  } = {},
): Promise<KhalaMobileSqlitePersistence> => {
  const databaseName = input.databaseName ?? KHALA_MOBILE_SYNC_DB_NAME
  const sqlite = await (input.sqliteLoader ?? loadExpoSqlite)()
  const db = await sqlite.openDatabaseAsync(databaseName)
  await db.execAsync(KHALA_MOBILE_SQLITE_SCHEMA)

  return {
    databaseName,
    db,
    kind: KHALA_MOBILE_PERSISTENCE_KIND,
    clearScope: scope =>
      db.runAsync(
        "DELETE FROM khala_sync_checkpoints WHERE scope = ?",
        scope
      ).then(() => undefined),
    readCheckpoint: scope =>
      db.getFirstAsync<{
        readonly scope: string
        readonly cursor: number
        readonly updated_at: string
      }>(
        "SELECT scope, cursor, updated_at FROM khala_sync_checkpoints WHERE scope = ?",
        scope
      ).then(row =>
        row === null
          ? null
          : {
              cursor: row.cursor,
              scope: row.scope,
              updatedAt: row.updated_at
            }
      ),
    saveCheckpoint: checkpoint =>
      db.runAsync(
        `INSERT INTO khala_sync_checkpoints (scope, cursor, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(scope) DO UPDATE SET
           cursor = excluded.cursor,
           updated_at = excluded.updated_at`,
        checkpoint.scope,
        checkpoint.cursor,
        checkpoint.updatedAt
      ).then(() => undefined)
  }
}
