import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { Effect } from "effect"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { KhalaSyncClientStoreError } from "@openagentsinc/khala-sync-client"
import { openMobileSyncHostCore } from "../src/sync/mobile-sync-host-core"

const openBunDatabase = (databasePath: string): ExpoSqliteDatabase => {
  const database = new Database(databasePath, { create: true })
  return {
    execSync: sql => database.exec(sql),
    runSync: (sql, ...params) => database.query(sql).run(...params),
    getAllSync: <Row>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.query(sql).all(...params) as ReadonlyArray<Row>,
    withTransactionSync: task => database.transaction(task)(),
    closeSync: () => database.close(),
  }
}

describe("contract openagents_mobile.sync.host_owned_expo_sqlite.v1", () => {
  test("closes the store when installation identity initialization fails", () => {
    const base = openExpoKhalaSyncStore(":memory:", openBunDatabase)
    let closed = false
    expect(() => openMobileSyncHostCore({
      databaseName: ":memory:",
      randomId: () => "unused",
      openStore: () => ({
        ...base,
        identity: () => Effect.fail(new KhalaSyncClientStoreError(
          "storage_failure",
          "fixture identity failure",
        )),
        close: () => Effect.sync(() => {
          closed = true
          Effect.runSync(base.close())
        }),
      }),
    })).toThrow("fixture identity failure")
    expect(closed).toBe(true)
  })

  test("writes one installation identity, reuses it after restart, and closes idempotently", () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-sync-"))
    const databaseName = join(root, "mobile.sqlite")
    const openStore = (name: string) => openExpoKhalaSyncStore(name, openBunDatabase)
    let generated = 0
    const randomId = () => `fixture-${++generated}`
    try {
      const first = openMobileSyncHostCore({ databaseName, randomId, openStore })
      expect(first.status()).toEqual({
        state: "local_ready",
        schemaVersion: 1,
        identityState: "persisted",
        pendingMutationCount: 0,
      })
      first.close()

      const inspect = openStore(databaseName)
      const identity = Effect.runSync(inspect.identity())
      Effect.runSync(inspect.close())
      expect(identity).toMatchObject({
        clientGroupId: "openagents-mobile.fixture-1",
        clientId: "mobile.fixture-2",
        schemaVersion: 1,
      })

      const second = openMobileSyncHostCore({ databaseName, randomId, openStore })
      expect(generated).toBe(2)
      second.close()
      second.close()
      expect(second.status().state).toBe("closed")
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("projects no database name, identity ref, token, or native handle", () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-sync-projection-"))
    const databaseName = join(root, "secret-name.sqlite")
    try {
      const host = openMobileSyncHostCore({
        databaseName,
        randomId: () => "private-ref",
        openStore: name => openExpoKhalaSyncStore(name, openBunDatabase),
      })
      const status = JSON.stringify(host.status())
      expect(status).not.toContain(databaseName)
      expect(status).not.toContain("private-ref")
      expect(status).not.toContain("token")
      expect(status).not.toContain("sqlite")
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("production composition owns Expo SQLite and cryptographic identity outside the view program", () => {
    const source = readFileSync(
      new URL("../src/sync/mobile-sync-host.ts", import.meta.url),
      "utf8",
    )
    expect(source).toContain('from "expo-sqlite"')
    expect(source).toContain('from "expo-crypto"')
    expect(source).toContain("openExpoKhalaSyncStore")
    expect(source).not.toContain("home-core")

    const appSource = readFileSync(
      new URL("../src/app.tsx", import.meta.url),
      "utf8",
    )
    expect(appSource).toContain("beforeReload: () => syncHost?.close()")
  })
})
