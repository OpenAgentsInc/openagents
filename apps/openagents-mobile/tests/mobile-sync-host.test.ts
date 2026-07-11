import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import {
  BootstrapResponse,
  LogPage,
  PushResponse,
  SyncVersionWatermark,
  type BootstrapRequest,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import {
  KhalaSyncClientStoreError,
  KhalaSyncTransportError,
  type KhalaSyncTransport,
} from "@openagentsinc/khala-sync-client"
import { openMobileSyncHostCore } from "../src/sync/mobile-sync-host-core"

const waitFor = async (condition: () => boolean): Promise<void> => {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (condition()) return
    await new Promise(resolve => setTimeout(resolve, 0))
  }
  throw new Error("timed out waiting for Sync phase")
}

const liveTransport = (input: Readonly<{
  bootstraps: Array<BootstrapRequest>
  lifecycle: Array<string>
}>): KhalaSyncTransport => ({
  bootstrap: request => Effect.sync(() => {
    input.bootstraps.push(request)
    return new BootstrapResponse({
      protocolVersion: 1,
      scope: request.scope,
      entities: [],
      cursor: SyncVersionWatermark.make(0),
    })
  }),
  logPage: (scope: SyncScope, cursor: number) => Effect.succeed(new LogPage({
    protocolVersion: 1,
    scope,
    entries: [],
    nextCursor: SyncVersionWatermark.make(cursor),
    upToDate: true,
  })),
  push: () => Effect.succeed(new PushResponse({
    protocolVersion: 1,
    results: [],
    lastMutationId: 0,
  })),
  connectLive: () => Effect.succeed({
    close: () => input.lifecycle.push("session"),
  }),
})

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
  test("fails closed on a newer local-store schema with recovery guidance", () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-future-sync-"))
    const databaseName = join(root, "future.sqlite")
    try {
      const future = new Database(databaseName, { create: true })
      future.exec("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);")
      future.query("INSERT INTO meta(key, value) VALUES('store_schema_version', '2')").run()
      future.close()

      let failure: unknown
      try {
        openMobileSyncHostCore({
          databaseName,
          randomId: () => "unused",
          openStore: name => openExpoKhalaSyncStore(name, openBunDatabase),
        })
      } catch (error) {
        failure = error
      }
      expect(failure).toMatchObject({ reason: "incompatible_version" })
      expect((failure as Error).message).toContain(
        "update the app or reset its local Sync cache",
      )
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

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
        syncPhase: "idle",
        lastDeltaAt: null,
        schemaVersion: 1,
        identityState: "persisted",
        pendingMutationCount: 0,
        identityTier:"local_only",
      })
      first.close()

      const inspect = openStore(databaseName)
      const identity = Effect.runSync(inspect.identity())
      const localIdentity=Effect.runSync(inspect.localIdentity())
      Effect.runSync(inspect.close())
      expect(identity).toMatchObject({
        clientGroupId: "openagents-mobile.fixture-1",
        clientId: "mobile.fixture-2",
        schemaVersion: 1,
      })
      expect(String(localIdentity?.identityRef)).toBe("local_fixture-3")

      const second = openMobileSyncHostCore({ databaseName, randomId, openStore })
      expect(generated).toBe(3)
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

  test("production composition owns Expo SQLite and local identity outside the view program", () => {
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

  test("subscribes the verified owner's personal scope, re-reads rotated auth, and closes session before store", async () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-sync-auth-"))
    const databaseName = join(root, "mobile.sqlite")
    const lifecycle: Array<string> = []
    const bootstraps: Array<BootstrapRequest> = []
    let token = "access.one"
    let capturedAuthToken: (() => string) | undefined
    const openStore = (name: string) => {
      const store = openExpoKhalaSyncStore(name, openBunDatabase)
      return {
        ...store,
        close: () => Effect.sync(() => {
          lifecycle.push("store")
          Effect.runSync(store.close())
        }),
      }
    }
    try {
      const host = openMobileSyncHostCore({ databaseName, randomId: () => "auth", openStore })
      host.connectAuthenticated({
        verification:"server_verified",
        baseUrl: "https://openagents.example",
        ownerUserId: "user.mobile",
        authToken: () => token,
        createTransport: config => {
          capturedAuthToken = config.authToken
          return liveTransport({ bootstraps, lifecycle })
        },
        sessionOptions: { sleep: () => Promise.resolve(), random: () => 0 },
      })
      await waitFor(() => host.status().syncPhase === "live")
      expect(host.conversation()).not.toBeNull()
      expect(host.interactions()).not.toBeNull()
      expect(await host.coding().directory()).toEqual({
        authority: "confirmed",
        phase: "live",
        cacheState: "current",
        repositories: [],
        sessions: [],
      })
      expect(bootstraps.map(request => String(request.scope))).toEqual(["scope.user.user.mobile"])
      expect(capturedAuthToken?.()).toBe("access.one")
      token = "access.two"
      expect(capturedAuthToken?.()).toBe("access.two")
      expect(host.status().lastDeltaAt).not.toBeNull()
      expect(host.status().identityTier).toBe("account_linked")
      host.unlinkAccount()
      expect(host.status().identityTier).toBe("local_only")
      expect(host.conversation()).toBeNull()
      expect(host.interactions()).toBeNull()
      expect(await host.coding().directory()).toMatchObject({
        authority: "withheld",
        phase: "signed_out",
        repositories: [],
        sessions: [],
      })
      host.close()
      expect(lifecycle.slice(-2)).toEqual(["session", "store"])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("projects an authorization denial as a bounded terminal phase", async () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-sync-denied-"))
    try {
      const host = openMobileSyncHostCore({
        databaseName: join(root, "mobile.sqlite"),
        randomId: () => "denied",
        openStore: name => openExpoKhalaSyncStore(name, openBunDatabase),
      })
      const denied = liveTransport({ bootstraps: [], lifecycle: [] })
      host.connectAuthenticated({
        verification:"server_verified",
        baseUrl: "https://openagents.example",
        ownerUserId: "user.denied",
        authToken: () => "rejected",
        createTransport: () => ({
          ...denied,
          bootstrap: () => Effect.fail(new KhalaSyncTransportError(
            "http_status",
            false,
            "denied fixture",
            { status: 403 },
          )),
        }),
        sessionOptions: { sleep: () => Promise.resolve(), random: () => 0 },
      })
      await waitFor(() => host.status().syncPhase === "denied")
      expect(host.status()).toMatchObject({ syncPhase: "denied", lastDeltaAt: null })
      expect(host.conversation()).toBeNull()
      expect(host.interactions()).toBeNull()
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
