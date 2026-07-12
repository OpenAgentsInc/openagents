import {
  CODING_REPOSITORY_ENTITY_TYPE,
  CODING_SESSION_ENTITY_TYPE,
  CodingNavigationEntity,
  CodingOwnerScopeRef,
  CodingRef,
  LocalRevision,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  resolveCodingNavigation,
  type CodingRepositoryEntity,
  type CodingSessionEntity,
  type CodingWorktreeEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import type {
  ConfirmedCodingCatalogSnapshot,
  KhalaSyncCodingCatalog,
  KhalaSyncLocalStore,
  ScopeSyncState,
} from "@openagentsinc/khala-sync-client"
import { Effect, Schema } from "effect"

export const MOBILE_CODING_SELECTION_ENTITY_TYPE = "mobile_coding_selection"
export const MOBILE_CODING_SELECTION_ENTITY_ID = "current"
export const MobileCodingTargetSchemaVersion = "openagents.mobile.coding_target.v1"
export const MobileCodingSelectionSchemaVersion = "openagents.mobile.coding_selection.v1"

const IsoTimestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

export const MobileCodingTarget = Schema.Struct({
  schema: Schema.Literal(MobileCodingTargetSchemaVersion),
  repositoryRef: CodingRef,
  sessionRef: CodingRef,
  threadRef: CodingRef,
})
export type MobileCodingTarget = typeof MobileCodingTarget.Type

export const MobileCodingSelection = Schema.Struct({
  schema: Schema.Literal(MobileCodingSelectionSchemaVersion),
  ownerScopeRef: CodingOwnerScopeRef,
  repositoryRef: CodingRef,
  sessionRef: CodingRef,
  threadRef: CodingRef,
  source: Schema.Literals(["directory", "deep_link", "notification", "restore"]),
  savedAt: IsoTimestamp,
})
export type MobileCodingSelection = typeof MobileCodingSelection.Type

const decodeTarget = Schema.decodeUnknownSync(MobileCodingTarget)
const decodeSelection = Schema.decodeUnknownSync(MobileCodingSelection)

/**
 * Loss accounting for the device-local confirmed coding cache. While hosted
 * authority is withheld, the directory names exactly how many confirmed
 * repository/session rows remain cached for the current owner scope and the
 * durable cursor they were confirmed through, without exposing row content.
 * Signed-out state is explicitly unaccounted: no owner's cache is read
 * without a live owner-scope handle.
 */
export type MobileCodingOfflineCacheAccounting = Readonly<{
  accounting: "live_confirmed" | "withheld_counted" | "unaccounted_signed_out"
  ownerScopeRef: string | null
  cachedRepositoryCount: number
  cachedSessionCount: number
  lastConfirmedCursor: number | null
}>

export const UNACCOUNTED_SIGNED_OUT_OFFLINE_CACHE: MobileCodingOfflineCacheAccounting = {
  accounting: "unaccounted_signed_out",
  ownerScopeRef: null,
  cachedRepositoryCount: 0,
  cachedSessionCount: 0,
  lastConfirmedCursor: null,
}

export type MobileCodingDirectory = Readonly<{
  authority: "confirmed" | "withheld"
  phase: ScopeSyncState["phase"] | "signed_out"
  cacheState: "current" | "hidden_until_reconnect" | "purged_after_denial"
  offlineCache: MobileCodingOfflineCacheAccounting
  repositories: ReadonlyArray<Readonly<{
    repositoryRef: string
    projectRef: string
    displayName: string
    sessionCount: number
  }>>
  sessions: ReadonlyArray<Readonly<{
    sessionRef: string
    repositoryRef: string
    threadRef: string
    state: CodingSessionEntity["state"]
    lastActiveAt: string
  }>>
}>

export type MobileCodingTargetResolution =
  | Readonly<{
      state: "ready"
      target: MobileCodingTarget
      repository: CodingRepositoryEntity
      worktree: CodingWorktreeEntity
      session: CodingSessionEntity
    }>
  | Readonly<{
      state: "rejected"
      reason:
        | "authority_unavailable"
        | "catalog_invalid"
        | "grant_revoked"
        | "grant_unavailable"
        | "owner_scope_mismatch"
        | "repository_mismatch"
        | "stale_repository"
        | "stale_session"
        | "stale_thread"
      affectedRef: string
    }>

export type MobileCodingInput =
  | Readonly<{ source: "deep_link"; url: string }>
  | Readonly<{ source: "notification"; payload: unknown }>

type MobileCodingTargetRejection = Extract<MobileCodingTargetResolution, { state: "rejected" }>

export type MobileCodingActivation =
  | Readonly<{ state: "active"; selection: MobileCodingSelection }>
  | MobileCodingTargetRejection
  | Readonly<{ state: "superseded" }>

export type MobileCodingThreadLease = Readonly<{
  close: () => Promise<void>
}>

export type MobileCodingNavigation = Readonly<{
  directory: () => Promise<MobileCodingDirectory>
  resolve: (target: MobileCodingTarget) => Promise<MobileCodingTargetResolution>
  accept: (input: MobileCodingInput) => Promise<MobileCodingTargetResolution>
  restore: () => Promise<MobileCodingTargetResolution | null>
  activate: (input: Readonly<{
    target: MobileCodingTarget
    source: MobileCodingSelection["source"]
    bindThread: (
      threadRef: string,
      onUpdate: () => void,
    ) => Promise<MobileCodingThreadLease | null>
    onUpdate?: () => void
  }>) => Promise<MobileCodingActivation>
  clearActive: () => Promise<void>
}>

const emptyDirectory = (
  phase: MobileCodingDirectory["phase"],
  offlineCache: MobileCodingOfflineCacheAccounting,
): MobileCodingDirectory => ({
  authority: "withheld",
  phase,
  cacheState: phase === "denied" ? "purged_after_denial" : "hidden_until_reconnect",
  offlineCache,
  repositories: [],
  sessions: [],
})

const grantEligible = (grant: CodingRepositoryEntity["grant"]): boolean =>
  grant.state === "granted" ||
  (grant.state === "unavailable" && grant.reason === "not_required")

const directoryFromSnapshot = (
  snapshot: ConfirmedCodingCatalogSnapshot,
  offlineCache: MobileCodingOfflineCacheAccounting,
): MobileCodingDirectory => {
  if (snapshot.status.phase !== "live") return emptyDirectory(snapshot.status.phase, offlineCache)
  const repositories = snapshot.catalog.repositories.filter(repository =>
    repository.availability.state === "available" && grantEligible(repository.grant)
  )
  const repositoryRefs = new Set(repositories.map(repository => repository.repositoryRef))
  const sessions = snapshot.catalog.sessions
    .filter(session => session.state !== "archived" && grantEligible(session.grant) && repositoryRefs.has(session.repositoryRef))
    .sort((left, right) => right.lastActiveAt.localeCompare(left.lastActiveAt))
  return {
    authority: "confirmed",
    phase: "live",
    cacheState: "current",
    offlineCache,
    repositories: repositories
      .map(repository => ({
        repositoryRef: repository.repositoryRef,
        projectRef: repository.projectRef,
        displayName: repository.displayName,
        sessionCount: sessions.filter(session => session.repositoryRef === repository.repositoryRef).length,
      }))
      .sort((left, right) => left.displayName.localeCompare(right.displayName)),
    sessions: sessions.map(session => ({
      sessionRef: session.sessionRef,
      repositoryRef: session.repositoryRef,
      threadRef: session.threadRef,
      state: session.state,
      lastActiveAt: session.lastActiveAt,
    })),
  }
}

export const decodeMobileCodingDeepLink = (url: string): MobileCodingTarget => {
  const parsed = new URL(url)
  const segments = parsed.pathname.split("/").filter(Boolean)
  if (
    parsed.protocol !== "openagents:" ||
    parsed.hostname !== "coding" ||
    segments.length !== 2 ||
    segments[0] !== "session"
  ) throw new Error("unsupported OpenAgents coding deep link")
  return decodeTarget({
    schema: MobileCodingTargetSchemaVersion,
    repositoryRef: parsed.searchParams.get("repository"),
    sessionRef: segments[1],
    threadRef: parsed.searchParams.get("thread"),
  })
}

export const decodeMobileCodingNotification = (payload: unknown): MobileCodingTarget =>
  decodeTarget(payload)

const rejected = (
  reason: MobileCodingTargetRejection["reason"],
  affectedRef: string,
): MobileCodingTargetRejection => ({ state: "rejected", reason, affectedRef })

const resolveAgainstSnapshot = (
  ownerScope: SyncScope,
  snapshot: ConfirmedCodingCatalogSnapshot,
  target: MobileCodingTarget,
): MobileCodingTargetResolution => {
  if (snapshot.status.phase !== "live") return rejected("authority_unavailable", target.sessionRef)
  if (snapshot.issues.length > 0) return rejected("catalog_invalid", snapshot.issues[0]!.affectedRef)
  const session = snapshot.catalog.sessions.find(value => value.sessionRef === target.sessionRef)
  if (session === undefined) return rejected("stale_session", target.sessionRef)
  if (session.ownerScopeRef !== String(ownerScope)) return rejected("owner_scope_mismatch", target.sessionRef)
  if (session.repositoryRef !== target.repositoryRef) return rejected("repository_mismatch", target.repositoryRef)
  if (session.threadRef !== target.threadRef) return rejected("stale_thread", target.threadRef)
  const navigation = new CodingNavigationEntity({
    schema: "openagents.coding_catalog.v1",
    navigationRef: `navigation.mobile.${target.sessionRef}`,
    ownerScopeRef: session.ownerScopeRef,
    selectedProjectRef: session.projectRef,
    selectedRepositoryRef: session.repositoryRef,
    selectedWorktreeRef: session.worktreeRef,
    selectedSessionRef: session.sessionRef,
    openSessionRefs: [session.sessionRef],
    focus: { kind: "conversation", conversationRef: session.conversationRef },
    updatedAt: session.updatedAt,
  })
  const resolution = resolveCodingNavigation(snapshot.catalog, navigation)
  if (resolution.state !== "ready") {
    if (resolution.state === "recovery_required") {
      if (resolution.reason === "grant_revoked") return rejected("grant_revoked", resolution.affectedRef)
      if (resolution.reason === "grant_unavailable") return rejected("grant_unavailable", resolution.affectedRef)
      if (resolution.reason === "owner_scope_mismatch") return rejected("owner_scope_mismatch", resolution.affectedRef)
      if (resolution.reason === "missing_repository") return rejected("stale_repository", resolution.affectedRef)
    }
    return rejected("stale_session", target.sessionRef)
  }
  if (resolution.repository.repositoryRef !== target.repositoryRef) {
    return rejected("repository_mismatch", target.repositoryRef)
  }
  return {
    state: "ready",
    target,
    repository: resolution.repository,
    worktree: resolution.worktree,
    session: resolution.session,
  }
}

/**
 * Device-local selection memory around a confirmed hosted catalog. Stable refs
 * may survive process death; cached hosted rows never become mobile authority.
 */
export const openMobileCodingNavigation = (input: Readonly<{
  store: KhalaSyncLocalStore
  deviceScope: SyncScope
  catalog: () => KhalaSyncCodingCatalog | null
  ownerScope: () => SyncScope | null
  now?: () => string
}>): MobileCodingNavigation => {
  const now = input.now ?? (() => new Date().toISOString())
  let generation = 0
  let activeLease: MobileCodingThreadLease | null = null

  const readSelection = async (): Promise<MobileCodingSelection | null> => {
    const rows = await Effect.runPromise(
      input.store.readLocalEntities(input.deviceScope, MOBILE_CODING_SELECTION_ENTITY_TYPE),
    )
    const row = rows.find(value => value.entityId === MOBILE_CODING_SELECTION_ENTITY_ID)
    if (row === undefined) return null
    try {
      return decodeSelection(JSON.parse(row.postImageJson))
    } catch {
      return null
    }
  }

  const saveSelection = async (selection: MobileCodingSelection): Promise<void> => {
    const rows = await Effect.runPromise(
      input.store.readLocalEntities(input.deviceScope, MOBILE_CODING_SELECTION_ENTITY_TYPE),
    )
    const revision = Math.max(0, ...rows.map(row => Number(row.revision))) + 1
    await Effect.runPromise(input.store.writeLocalEntities(input.deviceScope, [{
      entityType: MOBILE_CODING_SELECTION_ENTITY_TYPE,
      entityId: MOBILE_CODING_SELECTION_ENTITY_ID,
      postImageJson: JSON.stringify(selection),
      revision: LocalRevision.make(revision),
    }]))
  }

  const snapshot = async (): Promise<ConfirmedCodingCatalogSnapshot | null> => {
    const catalog = input.catalog()
    return catalog === null ? null : Effect.runPromise(catalog.snapshot())
  }

  const countDecodableCachedRows = <Entity extends Readonly<{ ownerScopeRef: string }>>(
    ownerScopeRef: string,
    rows: ReadonlyArray<Readonly<{ entityId: string; postImageJson: string }>>,
    decode: (raw: unknown) => Entity,
    refOf: (entity: Entity) => string,
  ): number =>
    rows.filter(row => {
      try {
        const entity = decode(JSON.parse(row.postImageJson))
        return refOf(entity) === row.entityId && entity.ownerScopeRef === ownerScopeRef
      } catch {
        return false
      }
    }).length

  const accountOfflineCache = async (live: boolean): Promise<MobileCodingOfflineCacheAccounting> => {
    const ownerScope = input.ownerScope()
    if (ownerScope === null) return UNACCOUNTED_SIGNED_OUT_OFFLINE_CACHE
    const ownerScopeRef = String(CodingOwnerScopeRef.make(String(ownerScope)))
    const [repositoryRows, sessionRows, cursor] = await Effect.runPromise(Effect.all([
      input.store.readEntities(ownerScope, CODING_REPOSITORY_ENTITY_TYPE),
      input.store.readEntities(ownerScope, CODING_SESSION_ENTITY_TYPE),
      input.store.cursor(ownerScope),
    ]))
    return {
      accounting: live ? "live_confirmed" : "withheld_counted",
      ownerScopeRef,
      cachedRepositoryCount: countDecodableCachedRows(
        ownerScopeRef,
        repositoryRows,
        decodeCodingRepositoryEntity,
        entity => entity.repositoryRef,
      ),
      cachedSessionCount: countDecodableCachedRows(
        ownerScopeRef,
        sessionRows,
        decodeCodingSessionEntity,
        entity => entity.sessionRef,
      ),
      lastConfirmedCursor: cursor === null ? null : Number(cursor),
    }
  }

  const resolve = async (target: MobileCodingTarget): Promise<MobileCodingTargetResolution> => {
    const ownerScope = input.ownerScope()
    const current = await snapshot()
    return ownerScope === null || current === null
      ? rejected("authority_unavailable", target.sessionRef)
      : resolveAgainstSnapshot(ownerScope, current, target)
  }

  return {
    directory: async () => {
      const catalog = input.catalog()
      if (catalog === null) return emptyDirectory("signed_out", UNACCOUNTED_SIGNED_OUT_OFFLINE_CACHE)
      const current = await Effect.runPromise(catalog.snapshot())
      return directoryFromSnapshot(current, await accountOfflineCache(current.status.phase === "live"))
    },
    resolve,
    accept: async candidate => {
      try {
        const target = candidate.source === "deep_link"
          ? decodeMobileCodingDeepLink(candidate.url)
          : decodeMobileCodingNotification(candidate.payload)
        return resolve(target)
      } catch {
        return rejected("stale_session", "invalid_target")
      }
    },
    restore: async () => {
      const selection = await readSelection()
      if (selection === null) return null
      const ownerScope = input.ownerScope()
      if (ownerScope === null || selection.ownerScopeRef !== String(ownerScope)) {
        return rejected("owner_scope_mismatch", selection.sessionRef)
      }
      return resolve({
        schema: MobileCodingTargetSchemaVersion,
        repositoryRef: selection.repositoryRef,
        sessionRef: selection.sessionRef,
        threadRef: selection.threadRef,
      })
    },
    activate: async activation => {
      const ownGeneration = ++generation
      const resolution = await resolve(activation.target)
      if (resolution.state === "rejected") return resolution
      const prior = activeLease
      activeLease = null
      await prior?.close()
      if (ownGeneration !== generation) return { state: "superseded" }
      const lease = await activation.bindThread(activation.target.threadRef, () => {
        if (ownGeneration === generation) activation.onUpdate?.()
      })
      if (ownGeneration !== generation) {
        await lease?.close()
        return { state: "superseded" }
      }
      if (lease === null) return rejected("stale_thread", activation.target.threadRef)
      activeLease = lease
      const ownerScope = input.ownerScope()
      if (ownerScope === null) {
        await lease.close()
        activeLease = null
        return rejected("authority_unavailable", activation.target.sessionRef)
      }
      const selection: MobileCodingSelection = {
        schema: MobileCodingSelectionSchemaVersion,
        ownerScopeRef: CodingOwnerScopeRef.make(String(ownerScope)),
        repositoryRef: activation.target.repositoryRef,
        sessionRef: activation.target.sessionRef,
        threadRef: activation.target.threadRef,
        source: activation.source,
        savedAt: now(),
      }
      await saveSelection(selection)
      return { state: "active", selection }
    },
    clearActive: async () => {
      generation += 1
      const lease = activeLease
      activeLease = null
      await lease?.close()
    },
  }
}
