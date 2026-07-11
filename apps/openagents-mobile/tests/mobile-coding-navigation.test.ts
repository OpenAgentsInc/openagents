import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import {
  LocalIdentityRef,
  deviceLocalScope,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  personalScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import type {
  ConfirmedCodingCatalogSnapshot,
  KhalaSyncCodingCatalog,
  ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  MobileCodingTargetSchemaVersion,
  decodeMobileCodingDeepLink,
  decodeMobileCodingNotification,
  openMobileCodingNavigation,
  type MobileCodingTarget,
  type MobileCodingThreadLease,
} from "../src/coding/mobile-coding-navigation"

const at = "2026-07-11T20:00:00.000Z"
const ownerScope = personalScope("owner.fixture")
const ownerScopeRef = String(ownerScope)
const schema = "openagents.coding_catalog.v1" as const
const granted = { state: "granted" as const, grantRef: "grant.mobile" }

const project = decodeCodingProjectEntity({
  schema,
  projectRef: "project.mobile",
  ownerScopeRef,
  displayName: "OpenAgents",
  aliasRefs: [],
  state: "active",
  createdAt: at,
  updatedAt: at,
  archivedAt: null,
})
const repository = decodeCodingRepositoryEntity({
  schema,
  repositoryRef: "repository.mobile",
  projectRef: project.projectRef,
  ownerScopeRef,
  displayName: "openagents",
  aliasRefs: ["repository.former"],
  pinnedBaseRef: "commit.main",
  availability: { state: "available" },
  grant: granted,
  createdAt: at,
  updatedAt: at,
})
const worktree = decodeCodingWorktreeEntity({
  schema,
  worktreeRef: "worktree.mobile",
  repositoryRef: repository.repositoryRef,
  projectRef: project.projectRef,
  ownerScopeRef,
  displayName: "main",
  aliasRefs: [],
  baseRef: "commit.main",
  availability: { state: "available" },
  grant: granted,
  createdAt: at,
  updatedAt: at,
})
const session = decodeCodingSessionEntity({
  schema,
  sessionRef: "session.mobile",
  ownerScopeRef,
  projectRef: project.projectRef,
  repositoryRef: repository.repositoryRef,
  worktreeRef: worktree.worktreeRef,
  workContextRef: "context.mobile",
  threadRef: "thread.mobile",
  conversationRef: "conversation.mobile",
  runRef: null,
  fleetRef: null,
  currentAttachmentRef: null,
  currentCheckpointRef: null,
  agentTopologyRef: null,
  canonicalEventCursor: 4,
  activityCursors: [],
  provider: { state: "known", providerRef: "provider.codex" },
  runtime: { state: "known", runtimeRef: "runtime.desktop" },
  grant: granted,
  state: "active",
  createdAt: at,
  updatedAt: at,
  lastActiveAt: at,
  archivedAt: null,
})

const target: MobileCodingTarget = {
  schema: MobileCodingTargetSchemaVersion,
  repositoryRef: repository.repositoryRef,
  sessionRef: session.sessionRef,
  threadRef: session.threadRef,
}

const status = (phase: ScopeSyncState["phase"]): ConfirmedCodingCatalogSnapshot["status"] => ({
  phase,
  cursor: phase === "live" ? 4 : null,
  pendingMutationCount: 0,
})

const snapshot = (
  phase: ScopeSyncState["phase"] = "live",
  sessionOverride = session,
): ConfirmedCodingCatalogSnapshot => ({
  status: status(phase),
  catalog: {
    projects: [project],
    repositories: [repository],
    worktrees: [worktree],
    sessions: [sessionOverride],
  },
  navigation: null,
  resolution: null,
  issues: [],
})

const catalog = (value: ConfirmedCodingCatalogSnapshot): KhalaSyncCodingCatalog => ({
  status: () => value.status,
  snapshot: () => Effect.succeed(value),
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

const withNavigation = async <Value>(
  task: (input: Readonly<{
    navigation: ReturnType<typeof openMobileCodingNavigation>
    setCatalog: (value: KhalaSyncCodingCatalog | null) => void
    setOwnerScope: (value: SyncScope | null) => void
    databaseName: string
  }>) => Promise<Value>,
): Promise<Value> => {
  const root = mkdtempSync(join(tmpdir(), "openagents-mobile-coding-"))
  const databaseName = join(root, "mobile.sqlite")
  const store = openExpoKhalaSyncStore(databaseName, openBunDatabase)
  const localIdentity = LocalIdentityRef.make("local_mobile_fixture")
  let currentCatalog: KhalaSyncCodingCatalog | null = catalog(snapshot())
  let currentOwnerScope: SyncScope | null = ownerScope
  const navigation = openMobileCodingNavigation({
    store,
    deviceScope: deviceLocalScope(localIdentity),
    catalog: () => currentCatalog,
    ownerScope: () => currentOwnerScope,
    now: () => at,
  })
  try {
    return await task({
      navigation,
      setCatalog: value => { currentCatalog = value },
      setOwnerScope: value => { currentOwnerScope = value },
      databaseName,
    })
  } finally {
    await navigation.clearActive()
    Effect.runSync(store.close())
    rmSync(root, { recursive: true, force: true })
  }
}

describe("contract openagents_mobile.coding.authenticated_navigation.v1", () => {
  test("lists only current authorized repositories and recent sessions", async () => {
    await withNavigation(async ({ navigation, setCatalog }) => {
      expect(await navigation.directory()).toEqual({
        authority: "confirmed",
        phase: "live",
        cacheState: "current",
        repositories: [{
          repositoryRef: "repository.mobile",
          projectRef: "project.mobile",
          displayName: "openagents",
          sessionCount: 1,
        }],
        sessions: [{
          sessionRef: "session.mobile",
          repositoryRef: "repository.mobile",
          threadRef: "thread.mobile",
          state: "active",
          lastActiveAt: at,
        }],
      })

      setCatalog(catalog(snapshot("must_refetch")))
      expect(await navigation.directory()).toEqual({
        authority: "withheld",
        phase: "must_refetch",
        cacheState: "hidden_until_reconnect",
        repositories: [],
        sessions: [],
      })
      setCatalog(catalog(snapshot("denied")))
      expect((await navigation.directory()).cacheState).toBe("purged_after_denial")
      setCatalog(null)
      expect(await navigation.directory()).toMatchObject({
        authority: "withheld",
        phase: "signed_out",
        repositories: [],
        sessions: [],
      })
    })
  })

  test("parses exact deep-link and notification schemas, then rejects stale targets", async () => {
    expect(decodeMobileCodingDeepLink(
      "openagents://coding/session/session.mobile?repository=repository.mobile&thread=thread.mobile",
    )).toEqual(target)
    expect(decodeMobileCodingNotification(target)).toEqual(target)
    expect(() => decodeMobileCodingDeepLink("openagents://coding/session/session.mobile")).toThrow()
    expect(() => decodeMobileCodingNotification({ ...target, threadRef: "/private/path" })).toThrow()

    await withNavigation(async ({ navigation }) => {
      expect(await navigation.accept({
        source: "deep_link",
        url: "openagents://coding/session/session.mobile?repository=repository.mobile&thread=thread.mobile",
      })).toMatchObject({ state: "ready", session: { sessionRef: "session.mobile" } })
      expect(await navigation.accept({
        source: "notification",
        payload: { ...target, repositoryRef: "repository.stale" },
      })).toEqual({
        state: "rejected",
        reason: "repository_mismatch",
        affectedRef: "repository.stale",
      })
    })
  })

  test("rejects cross-owner, revoked, stale-thread, and unavailable authority", async () => {
    await withNavigation(async ({ navigation, setCatalog, setOwnerScope }) => {
      expect(await navigation.resolve({ ...target, threadRef: "thread.stale" })).toEqual({
        state: "rejected",
        reason: "stale_thread",
        affectedRef: "thread.stale",
      })
      setOwnerScope(personalScope("other.owner"))
      expect(await navigation.resolve(target)).toMatchObject({
        state: "rejected",
        reason: "owner_scope_mismatch",
      })
      setOwnerScope(ownerScope)
      setCatalog(catalog(snapshot("live", decodeCodingSessionEntity({
        ...session,
        grant: { state: "revoked", grantRef: "grant.mobile", revokedAt: at },
      }))))
      expect(await navigation.resolve(target)).toMatchObject({
        state: "rejected",
        reason: "grant_revoked",
      })
      setCatalog(catalog(snapshot("idle")))
      expect(await navigation.resolve(target)).toMatchObject({
        state: "rejected",
        reason: "authority_unavailable",
      })
    })
  })

  test("persists only stable refs and restores them after a real SQLite reopen", async () => {
    const root = mkdtempSync(join(tmpdir(), "openagents-mobile-coding-restart-"))
    const databaseName = join(root, "mobile.sqlite")
    const deviceScope = deviceLocalScope(LocalIdentityRef.make("local_mobile_restart"))
    const make = () => {
      const store = openExpoKhalaSyncStore(databaseName, openBunDatabase)
      const navigation = openMobileCodingNavigation({
        store,
        deviceScope,
        catalog: () => catalog(snapshot()),
        ownerScope: () => ownerScope,
        now: () => at,
      })
      return { store, navigation }
    }
    try {
      const first = make()
      expect(await first.navigation.activate({
        target,
        source: "directory",
        bindThread: async () => ({ close: async () => undefined }),
      })).toMatchObject({ state: "active", selection: { sessionRef: "session.mobile" } })
      await first.navigation.clearActive()
      Effect.runSync(first.store.close())

      const second = make()
      expect(await second.navigation.restore()).toMatchObject({
        state: "ready",
        repository: { repositoryRef: "repository.mobile" },
        session: { sessionRef: "session.mobile", threadRef: "thread.mobile" },
      })
      const rows = Effect.runSync(second.store.readLocalEntities(deviceScope))
      expect(rows).toHaveLength(1)
      expect(rows[0]?.postImageJson).not.toContain("/")
      expect(rows[0]?.postImageJson).not.toContain("token")
      await second.navigation.clearActive()
      Effect.runSync(second.store.close())
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("closes the old lease and generation-fences late selection updates", async () => {
    await withNavigation(async ({ navigation }) => {
      const closed: string[] = []
      let firstReady!: (lease: MobileCodingThreadLease) => void
      let firstUpdate: (() => void) | undefined
      let acceptedUpdates = 0
      const first = navigation.activate({
        target,
        source: "directory",
        bindThread: (_threadRef, onUpdate) => {
          firstUpdate = onUpdate
          return new Promise(resolve => { firstReady = resolve })
        },
        onUpdate: () => { acceptedUpdates += 1 },
      })
      for (let attempt = 0; attempt < 20 && firstReady === undefined; attempt += 1) {
        await new Promise(resolve => setTimeout(resolve, 0))
      }
      expect(firstReady).toBeDefined()
      const second = navigation.activate({
        target,
        source: "notification",
        bindThread: async (_threadRef, onUpdate) => {
          onUpdate()
          return { close: async () => { closed.push("second") } }
        },
        onUpdate: () => { acceptedUpdates += 1 },
      })
      expect(await second).toMatchObject({ state: "active", selection: { source: "notification" } })
      firstReady({ close: async () => { closed.push("first") } })
      expect(await first).toEqual({ state: "superseded" })
      firstUpdate?.()
      expect(acceptedUpdates).toBe(1)
      expect(closed).toEqual(["first"])
      await navigation.clearActive()
      expect(closed).toEqual(["first", "second"])
    })
  })
})
