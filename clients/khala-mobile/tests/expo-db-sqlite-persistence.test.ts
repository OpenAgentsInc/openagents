import { describe, expect, test } from "bun:test"

import {
  KHALA_MOBILE_PERSISTENCE_KIND,
  KHALA_MOBILE_SQLITE_SCHEMA,
  openKhalaMobileSqlitePersistence,
  type ExpoSqliteDatabase,
  type ExpoSqliteModule
} from "../src/sync/expo-db-sqlite-persistence"

const fakeSqlite = (): {
  module: ExpoSqliteModule
  statements: Array<string>
  checkpoints: Map<string, { scope: string; cursor: number; updated_at: string }>
} => {
  const statements: Array<string> = []
  const checkpoints = new Map<string, { scope: string; cursor: number; updated_at: string }>()
  const db: ExpoSqliteDatabase = {
    execAsync: async statement => {
      statements.push(statement)
    },
    getFirstAsync: async <T>(_statement: string, scope: unknown): Promise<T | null> =>
      (checkpoints.get(String(scope)) ?? null) as T | null,
    runAsync: async (statement, ...params) => {
      statements.push(statement)
      if (statement.startsWith("INSERT INTO khala_sync_checkpoints")) {
        checkpoints.set(String(params[0]), {
          cursor: Number(params[1]),
          scope: String(params[0]),
          updated_at: String(params[2])
        })
      }
      if (statement.startsWith("DELETE FROM khala_sync_checkpoints")) {
        checkpoints.delete(String(params[0]))
      }
    }
  }

  return {
    checkpoints,
    module: {
      openDatabaseAsync: async () => db
    },
    statements
  }
}

describe("Khala mobile Expo SQLite persistence", () => {
  test("initializes the checkpoint and projection cache tables", async () => {
    const sqlite = fakeSqlite()
    const persistence = await openKhalaMobileSqlitePersistence({
      sqliteLoader: async () => sqlite.module
    })

    expect(persistence.kind).toBe(KHALA_MOBILE_PERSISTENCE_KIND)
    expect(sqlite.statements[0]).toContain("khala_sync_checkpoints")
    expect(KHALA_MOBILE_SQLITE_SCHEMA).toContain("khala_sync_projection_cache")
  })

  test("saves, reads, and clears scope checkpoints", async () => {
    const sqlite = fakeSqlite()
    const persistence = await openKhalaMobileSqlitePersistence({
      sqliteLoader: async () => sqlite.module
    })
    await persistence.saveCheckpoint({
      cursor: 42,
      scope: "scope.user.owner",
      updatedAt: "2026-07-04T20:00:00.000Z"
    })

    expect(await persistence.readCheckpoint("scope.user.owner")).toEqual({
      cursor: 42,
      scope: "scope.user.owner",
      updatedAt: "2026-07-04T20:00:00.000Z"
    })

    await persistence.clearScope("scope.user.owner")
    expect(await persistence.readCheckpoint("scope.user.owner")).toBeNull()
  })
})
