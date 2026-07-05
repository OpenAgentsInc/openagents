import { Database, type SQLQueryBindings } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import {
  ChangelogEntry,
  EntityId,
  EntityType,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncVersion,
  SyncVersionWatermark,
  personalScope
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import {
  KHALA_MOBILE_PERSISTENCE_KIND,
  KHALA_MOBILE_SQLITE_SCHEMA,
  KHALA_MOBILE_SYNC_STORE_KIND,
  openKhalaMobileSqlitePersistence,
  openKhalaMobileSyncStore,
  type ExpoSqliteDatabase,
  type ExpoSqliteModule
} from "../src/sync/expo-db-sqlite-persistence"

const expoSqliteFromBun = (): {
  module: ExpoSqliteModule
  statements: Array<string>
} => {
  const databases = new Map<string, Database>()
  const statements: Array<string> = []

  const open = (name: string): ExpoSqliteDatabase => {
    const db = databases.get(name) ?? new Database(":memory:")
    databases.set(name, db)
    return {
      execAsync: async statement => {
        statements.push(statement)
        db.exec(statement)
      },
      getAllAsync: async <T>(statement: string, ...params: ReadonlyArray<unknown>) =>
        db.query(statement).all(...(params as ReadonlyArray<SQLQueryBindings>)) as ReadonlyArray<T>,
      getFirstAsync: async <T>(statement: string, ...params: ReadonlyArray<unknown>) =>
        (db.query(statement).get(...(params as ReadonlyArray<SQLQueryBindings>)) as T | null) ?? null,
      runAsync: async (statement, ...params) => {
        statements.push(statement)
        db.query(statement).run(...(params as ReadonlyArray<SQLQueryBindings>))
      },
      withTransactionAsync: async task => task()
    }
  }

  return {
    module: {
      openDatabaseAsync: async name => open(name)
    },
    statements
  }
}

describe("Khala mobile Expo SQLite persistence", () => {
  test("initializes the checkpoint and projection cache tables", async () => {
    const sqlite = expoSqliteFromBun()
    const persistence = await openKhalaMobileSqlitePersistence({
      sqliteLoader: async () => sqlite.module
    })

    expect(persistence.kind).toBe(KHALA_MOBILE_PERSISTENCE_KIND)
    expect(sqlite.statements[0]).toContain("khala_sync_checkpoints")
    expect(KHALA_MOBILE_SQLITE_SCHEMA).toContain("khala_sync_projection_cache")
  })

  test("saves, reads, and clears scope checkpoints plus projection rows", async () => {
    const sqlite = expoSqliteFromBun()
    const persistence = await openKhalaMobileSqlitePersistence({
      sqliteLoader: async () => sqlite.module
    })
    await persistence.saveCheckpoint({
      cursor: 42,
      scope: "scope.user.owner",
      updatedAt: "2026-07-04T20:00:00.000Z"
    })
    await persistence.saveProjectionEntities(
      "scope.user.owner",
      [
        {
          entityId: "thread.mobile",
          entityType: "chat_thread",
          postImageJson: "{\"threadId\":\"thread.mobile\"}"
        }
      ],
      "2026-07-04T20:01:00.000Z"
    )

    expect(await persistence.readCheckpoint("scope.user.owner")).toEqual({
      cursor: 42,
      scope: "scope.user.owner",
      updatedAt: "2026-07-04T20:00:00.000Z"
    })
    expect(await persistence.readProjectionEntities("scope.user.owner")).toEqual([
      {
        entityId: "thread.mobile",
        entityType: "chat_thread",
        postImageJson: "{\"threadId\":\"thread.mobile\"}",
        scope: "scope.user.owner",
        updatedAt: "2026-07-04T20:01:00.000Z"
      }
    ])

    await persistence.clearScope("scope.user.owner")
    expect(await persistence.readCheckpoint("scope.user.owner")).toBeNull()
    expect(await persistence.readProjectionEntities("scope.user.owner")).toEqual([])
  })
})

describe("Khala mobile Expo SQLite Khala Sync store", () => {
  test("persists cursors, confirmed rows, identity, and pending mutation queue", async () => {
    const sqlite = expoSqliteFromBun()
    const store = await openKhalaMobileSyncStore({
      databaseName: "sync-store",
      sqliteLoader: async () => sqlite.module
    })
    const scope = personalScope("owner")
    const entry = new ChangelogEntry({
      committedAt: "2026-07-04T20:00:00.000Z",
      entityId: EntityId.make("thread.mobile"),
      entityType: EntityType.make("chat_thread"),
      op: "upsert",
      postImageJson: "{\"threadId\":\"thread.mobile\"}",
      scope,
      version: SyncVersion.make(1)
    })

    expect(store.kind).toBe(KHALA_MOBILE_SYNC_STORE_KIND)
    await Effect.runPromise(store.applyConfirmed(scope, [entry], SyncVersion.make(1)))
    expect(await Effect.runPromise(store.cursor(scope))).toBe(SyncVersion.make(1))
    expect(await Effect.runPromise(store.readEntities(scope))).toEqual([
      {
        entityId: "thread.mobile",
        entityType: "chat_thread",
        postImageJson: "{\"threadId\":\"thread.mobile\"}",
        version: SyncVersion.make(1)
      }
    ])

    await Effect.runPromise(
      store.enqueueMutation(
        new MutationEnvelope({
          argsJson: "{\"threadId\":\"thread.mobile\"}",
          mutationId: MutationId.make(1),
          name: MutatorName.make("chat.appendMessage")
        })
      )
    )
    expect(
      (await Effect.runPromise(store.pendingMutations())).map(mutation => Number(mutation.mutationId))
    ).toEqual([1])
    await Effect.runPromise(store.ackMutations(MutationId.make(1)))
    expect(await Effect.runPromise(store.pendingMutations())).toEqual([])

    await Effect.runPromise(store.resetScope(scope, [], SyncVersionWatermark.make(0)))
    expect(await Effect.runPromise(store.cursor(scope))).toBeNull()
  })

  test("reopening the same Expo database preserves the durable cursor", async () => {
    const sqlite = expoSqliteFromBun()
    const scope = personalScope("owner")
    const first = await openKhalaMobileSyncStore({
      databaseName: "sync-store",
      sqliteLoader: async () => sqlite.module
    })
    await Effect.runPromise(first.resetScope(scope, [], SyncVersion.make(9)))

    const reopened = await openKhalaMobileSyncStore({
      databaseName: "sync-store",
      sqliteLoader: async () => sqlite.module
    })
    expect(await Effect.runPromise(reopened.cursor(scope))).toBe(SyncVersion.make(9))
  })
})
