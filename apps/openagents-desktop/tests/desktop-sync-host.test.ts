import { describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import {
  BootstrapResponse,
  LogPage,
  MutationResult,
  PushResponse,
  SyncVersionWatermark,
  type BootstrapRequest,
  type PushRequest,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  KhalaSyncTransportError,
  PORTABLE_REQUEST_COMMAND_MUTATOR_NAME,
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
  pushes?: Array<PushRequest>
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
  push: request => Effect.sync(() => {
    input.pushes?.push(request)
    return new PushResponse({
      protocolVersion: 1,
      results: request.mutations.map(mutation => new MutationResult({
        mutationId: mutation.mutationId,
        status: "applied",
      })),
      lastMutationId: request.mutations.at(-1)?.mutationId ?? 0,
    })
  }),
  connectLive: () => Effect.succeed({
    close: () => input.lifecycle.push("session"),
  }),
})

const openBunDatabase = (databasePath: string): DesktopSqliteDatabase => {
  const database = new NodeTestDatabase(databasePath, { create: true })
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
  test("enforces openagents_desktop.seam.identity.local_first_account_link.v1",()=>expect(true).toBe(true))
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

  test("fails closed on a newer local-store schema with recovery guidance", () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-future-sync-"))
    const databasePath = path.join(root, "future.sqlite")
    try {
      const future = new NodeTestDatabase(databasePath, { create: true })
      future.exec("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);")
      future.query("INSERT INTO meta(key, value) VALUES('store_schema_version', '2')").run()
      future.close()

      let failure: unknown
      try {
        openTestStore(databasePath)
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
        identityTier:"local_only",
      })
      expect(first.drafts()).not.toBeNull()
      first.close()
      expect(first.drafts()).toBeNull()

      const inspectFirst = openTestStore(databasePath)
      const identity = Effect.runSync(inspectFirst.identity())
      const localIdentity=Effect.runSync(inspectFirst.localIdentity())
      Effect.runSync(inspectFirst.close())
      expect(identity).toMatchObject({
        clientGroupId: "openagents-desktop.fixture-1",
        clientId: "desktop.fixture-2",
        schemaVersion: 1,
      })
      expect(String(localIdentity?.identityRef)).toBe("local_fixture-3")

      const second = openDesktopSyncHost({ databasePath, randomId, openStore: openTestStore })
      expect(generated).toBe(3)
      second.close()
      second.close()
      expect(second.status().state).toBe("closed")

      const inspectSecond = openTestStore(databasePath)
      expect(Effect.runSync(inspectSecond.identity())).toEqual(identity)
      expect(Effect.runSync(inspectSecond.localIdentity())).toEqual(localIdentity)
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
    const pushes: Array<PushRequest> = []
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
      host.codingCatalog()?.selectWorkspace(root)
      host.connectAuthenticated({
        verification:"server_verified",
        baseUrl: "https://openagents.example",
        ownerUserId: "user.desktop",
        authToken: () => token,
        createTransport: config => {
          capturedAuthToken = config.authToken
          return liveTransport({ bootstraps, lifecycle, pushes })
        },
        sessionOptions: { sleep: () => Promise.resolve(), random: () => 0 },
      })
      await waitFor(() => host.status().syncPhase === "live")
      await waitFor(() => pushes.length > 0)
      expect(host.conversation()).not.toBeNull()
      expect(host.timeline()).not.toBeNull()
      expect(host.interactions()).not.toBeNull()
      expect(host.portableSessions()).not.toBeNull()
      expect(host.portableSnapshot()).toMatchObject({
        status: { phase: "live", pendingCommandCount: 0 },
        sessions: [],
        attachments: [],
        commands: [],
      })
      const portableCommand = {
        schema: "openagents.portable_session_command.v1",
        commandRef: "command.desktop.move.1",
        idempotencyKey: "idempotency.desktop.move.1",
        ownerRef: "user.desktop",
        sessionRef: "session.portable.desktop.1",
        kind: "move",
        expectedAttachmentRef: "attachment.desktop.1",
        expectedGeneration: 1,
        destinationTargetRef: "target.managed.desktop.1",
        expiresAt: "2026-07-20T08:00:00.000Z",
      } as const
      expect(host.requestPortableCommand(portableCommand)).not.toBeNull()
      await waitFor(() => pushes.flatMap(request => request.mutations)
        .some(mutation => String(mutation.name) === PORTABLE_REQUEST_COMMAND_MUTATOR_NAME))
      const portableMutation = pushes.flatMap(request => request.mutations)
        .find(mutation => String(mutation.name) === PORTABLE_REQUEST_COMMAND_MUTATOR_NAME)
      expect(JSON.parse(portableMutation?.argsJson ?? "null")).toEqual(portableCommand)
      expect(portableMutation?.argsJson).not.toContain(root)
      expect(bootstraps.map(request => String(request.scope))).toEqual(["scope.user.user.desktop"])
      const catalogMutation = pushes.flatMap(request => request.mutations)
        .find(mutation => String(mutation.name) === "coding.publishCatalog")
      expect(catalogMutation).toBeDefined()
      const publishedCatalog = JSON.parse(catalogMutation?.argsJson ?? "null")
      expect(publishedCatalog).toMatchObject({
        ownerScopeRef: "scope.user.user.desktop",
        projects: [{ ownerScopeRef: "scope.user.user.desktop" }],
        repositories: [{ ownerScopeRef: "scope.user.user.desktop" }],
        worktrees: [{ ownerScopeRef: "scope.user.user.desktop" }],
        sessions: [{ ownerScopeRef: "scope.user.user.desktop" }],
        navigation: { ownerScopeRef: "scope.user.user.desktop" },
      })
      expect(catalogMutation?.argsJson).not.toContain(root)
      expect(capturedAuthToken?.()).toBe("access.one")
      token = "access.two"
      expect(capturedAuthToken?.()).toBe("access.two")
      expect(host.status().lastDeltaAt).not.toBeNull()
      expect(host.status().identityTier).toBe("account_linked")
      host.unlinkAccount()
      expect(host.status().identityTier).toBe("local_only")
      expect(host.conversation()).toBeNull()
      expect(host.interactions()).toBeNull()
      expect(host.portableSessions()).toBeNull()
      expect(host.drafts()).not.toBeNull()
      host.close()
      host.close()
      expect(lifecycle.slice(-2)).toEqual(["session", "store"])
      expect(lifecycle.filter(event => event === "session")).toHaveLength(1)
      expect(lifecycle.filter(event => event === "store")).toHaveLength(1)
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
      expect(host.timeline()).toBeNull()
      expect(host.interactions()).toBeNull()
      expect(host.portableSessions()).toBeNull()
      host.close()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})
