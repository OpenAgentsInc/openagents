import { describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { Database } from "bun:sqlite"
import {
  BootstrapResponse,
  LogPage,
  PushResponse,
  SyncVersionWatermark,
  type BootstrapRequest,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  KhalaSyncTransportError,
  type KhalaSyncTransport,
} from "@openagentsinc/khala-sync-client"

import { Effect } from "effect"

import { openDesktopSyncHost } from "../src/desktop-sync-host.ts"
import {
  openDesktopSyncStore,
  type DesktopSqliteDatabase,
  type DesktopSyncStore,
} from "../src/desktop-sync-store.ts"

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

const openBunDatabase = (databasePath: string): DesktopSqliteDatabase => {
  const database = new Database(databasePath, { create: true })
  return {
    exec: sql => database.exec(sql),
    prepare: sql => {
      const statement = database.query(sql)
      return {
        run: (...params) => statement.run(...params),
        all: (...params) => statement.all(...params),
      }
    },
    close: () => database.close(),
  }
}

const openTestStore = (databasePath: string): DesktopSyncStore =>
  openDesktopSyncStore(databasePath, openBunDatabase)

describe("openagents_desktop.sync.host_owned_sqlite.v1", () => {
  test("closes the database when initialization fails", () => {
    let closed = false
    expect(() =>
      openDesktopSyncStore("fixture.sqlite", () => ({
        exec: () => {
          throw new Error("migration failed")
        },
        prepare: () => {
          throw new Error("unexpected prepare")
        },
        close: () => {
          closed = true
        },
      })),
    ).toThrow()
    expect(closed).toBe(true)
  })

  test("persists one installation identity and reuses it after restart", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-"))
    const databasePath = path.join(root, "private", "sync.sqlite")
    let generated = 0
    const randomId = () => `fixture-${++generated}`
    try {
      const first = openDesktopSyncHost({ databasePath, randomId, openStore: openTestStore })
      expect(first.status()).toEqual({
        state: "local_ready",
        syncPhase: "idle",
        lastDeltaAt: null,
        schemaVersion: 1,
        identityState: "persisted",
        pendingMutationCount: 0,
      })
      first.close()

      const inspectFirst = openTestStore(databasePath)
      const identity = Effect.runSync(inspectFirst.identity())
      Effect.runSync(inspectFirst.close())
      expect(identity).toMatchObject({
        clientGroupId: "openagents-desktop.fixture-1",
        clientId: "desktop.fixture-2",
        schemaVersion: 1,
      })

      const second = openDesktopSyncHost({ databasePath, randomId, openStore: openTestStore })
      expect(generated).toBe(2)
      second.close()
      second.close()
      expect(second.status().state).toBe("closed")

      const inspectSecond = openTestStore(databasePath)
      expect(Effect.runSync(inspectSecond.identity())).toEqual(identity)
      Effect.runSync(inspectSecond.close())
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("creates an owner-private directory and database on POSIX hosts", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-mode-"))
    const databasePath = path.join(root, "private", "sync.sqlite")
    try {
      const host = openDesktopSyncHost({ databasePath, randomId: () => "mode", openStore: openTestStore })
      if (process.platform !== "win32") {
        expect(statSync(path.dirname(databasePath)).mode & 0o777).toBe(0o700)
        expect(statSync(databasePath).mode & 0o777).toBe(0o600)
      }
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("projects no path, client identity, token, or database handle", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-projection-"))
    try {
      const host = openDesktopSyncHost({
        databasePath: path.join(root, "private", "sync.sqlite"),
        randomId: () => "private-ref",
        openStore: openTestStore,
      })
      const serialized = JSON.stringify(host.status())
      expect(serialized).not.toContain(root)
      expect(serialized).not.toContain("private-ref")
      expect(serialized).not.toContain("token")
      expect(serialized).not.toContain("sqlite")
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("subscribes the verified owner's personal scope, re-reads rotated auth, and closes session before store", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-auth-"))
    const lifecycle: Array<string> = []
    const bootstraps: Array<BootstrapRequest> = []
    let token = "access.one"
    let capturedAuthToken: (() => string) | undefined
    const databasePath = path.join(root, "private", "sync.sqlite")
    const openStore = (filePath: string): DesktopSyncStore => {
      const store = openTestStore(filePath)
      return {
        ...store,
        close: () => Effect.sync(() => {
          lifecycle.push("store")
          Effect.runSync(store.close())
        }),
      }
    }
    try {
      const host = openDesktopSyncHost({ databasePath, randomId: () => "auth", openStore })
      host.connectAuthenticated({
        baseUrl: "https://openagents.example",
        ownerUserId: "user.desktop",
        authToken: () => token,
        createTransport: config => {
          capturedAuthToken = config.authToken
          return liveTransport({ bootstraps, lifecycle })
        },
        sessionOptions: { sleep: () => Promise.resolve(), random: () => 0 },
      })
      await waitFor(() => host.status().syncPhase === "live")
      expect(host.conversation()).not.toBeNull()
      expect(bootstraps.map(request => String(request.scope))).toEqual(["scope.user.user.desktop"])
      expect(capturedAuthToken?.()).toBe("access.one")
      token = "access.two"
      expect(capturedAuthToken?.()).toBe("access.two")
      expect(host.status().lastDeltaAt).not.toBeNull()
      host.close()
      expect(lifecycle.slice(-2)).toEqual(["session", "store"])
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("projects an authorization denial as a bounded terminal phase", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-sync-denied-"))
    try {
      const host = openDesktopSyncHost({
        databasePath: path.join(root, "private", "sync.sqlite"),
        randomId: () => "denied",
        openStore: openTestStore,
      })
      const denied = liveTransport({ bootstraps: [], lifecycle: [] })
      host.connectAuthenticated({
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
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
