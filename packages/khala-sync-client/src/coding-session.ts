import {
  CODING_NAVIGATION_ENTITY_TYPE,
  CODING_PROJECT_ENTITY_TYPE,
  CODING_REPOSITORY_ENTITY_TYPE,
  CODING_SESSION_ENTITY_TYPE,
  CODING_WORKTREE_ENTITY_TYPE,
  CodingOwnerScopeRef,
  decodeCodingNavigationEntity,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  resolveCodingNavigation,
  validateCodingSessionCatalog,
  type CodingCatalogIssue,
  type CodingNavigationEntity,
  type CodingNavigationResolution,
  type CodingProjectEntity,
  type CodingRepositoryEntity,
  type CodingSessionCatalog,
  type CodingSessionEntity,
  type CodingWorktreeEntity,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type { ConfirmedEntity, KhalaSyncClientStoreError, KhalaSyncLocalStore } from "./store.js"

export const MAX_CONFIRMED_CODING_PROJECTS = 128
export const MAX_CONFIRMED_CODING_REPOSITORIES = 512
export const MAX_CONFIRMED_CODING_WORKTREES = 1_024
export const MAX_CONFIRMED_CODING_SESSIONS = 2_048
export const MAX_CONFIRMED_CODING_NAVIGATIONS = 8

export type KhalaSyncCodingCatalogStatus = Readonly<{
  phase: ScopeSyncState["phase"]
  cursor: number | null
  pendingMutationCount: number
}>

export type ConfirmedCodingCatalogSnapshot = Readonly<{
  status: KhalaSyncCodingCatalogStatus
  catalog: CodingSessionCatalog
  navigation: CodingNavigationEntity | null
  resolution: CodingNavigationResolution | null
  issues: ReadonlyArray<CodingCatalogIssue>
}>

export type KhalaSyncCodingCatalog = Readonly<{
  status: () => KhalaSyncCodingCatalogStatus
  snapshot: () => Effect.Effect<ConfirmedCodingCatalogSnapshot, KhalaSyncClientStoreError>
}>

type Versioned<A> = Readonly<{ value: A; version: number }>

const emptyCatalog = (): CodingSessionCatalog => ({
  projects: [],
  repositories: [],
  worktrees: [],
  sessions: [],
})

const cursorFromState = (state: ScopeSyncState): number | null =>
  state.phase === "live" || state.phase === "catching_up" ? Number(state.cursor) : null

const bounded = <A>(values: ReadonlyArray<Versioned<A>>, max: number): ReadonlyArray<A> =>
  [...values]
    .sort((left, right) => right.version - left.version)
    .slice(0, max)
    .map(entry => entry.value)

const confirmedCatalog = (
  ownerScopeRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): Pick<ConfirmedCodingCatalogSnapshot, "catalog" | "navigation" | "resolution" | "issues"> => {
  const projects: Versioned<CodingProjectEntity>[] = []
  const repositories: Versioned<CodingRepositoryEntity>[] = []
  const worktrees: Versioned<CodingWorktreeEntity>[] = []
  const sessions: Versioned<CodingSessionEntity>[] = []
  const navigations: Versioned<CodingNavigationEntity>[] = []
  for (const row of rows) {
    try {
      const raw = JSON.parse(row.postImageJson)
      const version = Number(row.version)
      if (row.entityType === CODING_PROJECT_ENTITY_TYPE) {
        const value = decodeCodingProjectEntity(raw)
        if (value.projectRef === row.entityId && value.ownerScopeRef === ownerScopeRef) projects.push({ value, version })
      } else if (row.entityType === CODING_REPOSITORY_ENTITY_TYPE) {
        const value = decodeCodingRepositoryEntity(raw)
        if (value.repositoryRef === row.entityId && value.ownerScopeRef === ownerScopeRef) repositories.push({ value, version })
      } else if (row.entityType === CODING_WORKTREE_ENTITY_TYPE) {
        const value = decodeCodingWorktreeEntity(raw)
        if (value.worktreeRef === row.entityId && value.ownerScopeRef === ownerScopeRef) worktrees.push({ value, version })
      } else if (row.entityType === CODING_SESSION_ENTITY_TYPE) {
        const value = decodeCodingSessionEntity(raw)
        if (value.sessionRef === row.entityId && value.ownerScopeRef === ownerScopeRef) sessions.push({ value, version })
      } else if (row.entityType === CODING_NAVIGATION_ENTITY_TYPE) {
        const value = decodeCodingNavigationEntity(raw)
        if (value.navigationRef === row.entityId && value.ownerScopeRef === ownerScopeRef) navigations.push({ value, version })
      }
    } catch {
      // Ignore malformed or pre-contract rows; a confirmed replacement self-heals.
    }
  }
  const catalog: CodingSessionCatalog = {
    projects: bounded(projects, MAX_CONFIRMED_CODING_PROJECTS),
    repositories: bounded(repositories, MAX_CONFIRMED_CODING_REPOSITORIES),
    worktrees: bounded(worktrees, MAX_CONFIRMED_CODING_WORKTREES),
    sessions: bounded(sessions, MAX_CONFIRMED_CODING_SESSIONS),
  }
  const navigation = bounded(navigations, MAX_CONFIRMED_CODING_NAVIGATIONS)[0] ?? null
  const issues = validateCodingSessionCatalog(catalog)
  return {
    catalog,
    navigation,
    resolution: navigation === null ? null : resolveCodingNavigation(catalog, navigation),
    issues,
  }
}

/** Reads only server-confirmed post-images from one authorized user/team scope. */
export const createKhalaSyncCodingCatalog = (input: Readonly<{
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
  ownerScope: SyncScope
}>): KhalaSyncCodingCatalog => {
  const ownerScopeRef = String(CodingOwnerScopeRef.make(String(input.ownerScope)))
  const status = (): KhalaSyncCodingCatalogStatus => {
    const state = input.session.state(input.ownerScope)
    return {
      phase: state.phase,
      cursor: cursorFromState(state),
      pendingMutationCount: input.session.pending().length,
    }
  }
  return {
    status,
    snapshot: () => {
      const current = status()
      if (current.phase !== "live") {
        return Effect.succeed({
          status: current,
          catalog: emptyCatalog(),
          navigation: null,
          resolution: null,
          issues: [],
        })
      }
      return Effect.map(input.store.readEntities(input.ownerScope), rows => ({
        status: current,
        ...confirmedCatalog(ownerScopeRef, rows),
      }))
    },
  }
}
