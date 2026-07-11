import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncSchemaVersion,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  expoSqliteDriver,
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "./expo-sqlite-store.js"
import { KhalaSyncClientStoreError } from "./store.js"

const roots: Array<string> = []
afterEach(() => {
  while (roots.length > 0) rmSync(roots.pop()!, { recursive: true, force: true })
})

const expoDatabaseFromBun = (database: Database): ExpoSqliteDatabase => ({
  execSync: sql => database.exec(sql),
  runSync: (sql, ...params) => database.query(sql).run(...params),
  getAllSync: <Row>(sql: string, ...params: ReadonlyArray<string | number>) =>
    database.query(sql).all(...params) as ReadonlyArray<Row>,
  withTransactionSync: task => database.transaction(task)(),
  closeSync: () => database.close(),
})

const openBunExpoDatabase = (databasePath: string): ExpoSqliteDatabase =>
  expoDatabaseFromBun(new Database(databasePath, { create: true }))

describe("Expo SQLite Khala Sync adapter", () => {
  test("preserves identity and the durable mutation queue across reopen", () => {
    const root = mkdtempSync(join(tmpdir(), "khala-sync-expo-"))
    roots.push(root)
    const databasePath = join(root, "mobile.sqlite")
    const identity = {
      clientGroupId: ClientGroupId.make("openagents-mobile.fixture-group"),
      clientId: ClientId.make("mobile.fixture-client"),
      schemaVersion: SyncSchemaVersion.make(1),
    }
    const mutation = new MutationEnvelope({
      mutationId: MutationId.make(1),
      name: MutatorName.make("chat.composeTurn"),
      argsJson: canonicalJson({ text: "continue on mobile" }),
    })

    const first = openExpoKhalaSyncStore(databasePath, openBunExpoDatabase)
    Effect.runSync(first.setIdentity(identity))
    Effect.runSync(first.enqueueMutation(mutation))
    Effect.runSync(first.close())

    const second = openExpoKhalaSyncStore(databasePath, openBunExpoDatabase)
    expect(Effect.runSync(second.identity())).toEqual(identity)
    expect(Effect.runSync(second.pendingMutations())).toEqual([mutation])
    Effect.runSync(second.close())
  })

  test("rolls back the whole synchronous transaction on failure", () => {
    const database = new Database(":memory:")
    const driver = expoSqliteDriver(expoDatabaseFromBun(database))
    driver.exec("CREATE TABLE receipt (id INTEGER PRIMARY KEY);")
    expect(() => driver.transaction(() => {
      driver.run("INSERT INTO receipt (id) VALUES (?)", [1])
      throw new Error("stop")
    })).toThrow("stop")
    expect(driver.all<{ id: number }>("SELECT id FROM receipt")).toEqual([])
    database.close()
  })

  test("closes an opened native handle when initialization fails", () => {
    let closed = false
    expect(() => openExpoKhalaSyncStore("fixture.sqlite", () => ({
      execSync: () => {
        throw new Error("pragma failed")
      },
      runSync: () => undefined,
      getAllSync: () => [],
      withTransactionSync: () => undefined,
      closeSync: () => {
        closed = true
      },
    }))).toThrow("failed to open khala-sync Expo SQLite store")
    expect(closed).toBe(true)
  })

  test("preserves typed incompatible-version recovery guidance at the mobile adapter", () => {
    const database = new Database(":memory:")
    database.exec("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);")
    database.query("INSERT INTO meta(key, value) VALUES('store_schema_version', '2')").run()
    let closed = false
    let failure: unknown
    try {
      openExpoKhalaSyncStore("future.sqlite", () => ({
        ...expoDatabaseFromBun(database),
        closeSync: () => {
          closed = true
          database.close()
        },
      }))
    } catch (error) {
      failure = error
    }
    expect(failure).toBeInstanceOf(KhalaSyncClientStoreError)
    expect((failure as KhalaSyncClientStoreError).reason).toBe("incompatible_version")
    expect((failure as Error).message).toContain("update the app or reset its local Sync cache")
    expect(closed).toBe(true)
  })
})
