import { describe, expect, test } from "vite-plus/test"
import {
  ChangelogEntry,
  EntityId,
  EntityType,
  SyncVersion,
  SyncVersionWatermark,
  decodeCodingNavigationEntity,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  personalScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import { createKhalaSyncCodingCatalog } from "./coding-session.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const scope = personalScope("owner-1")
const ownerScopeRef = String(scope)
const at = "2026-07-11T12:00:00.000Z"
const schema = "openagents.coding_catalog.v1"
const grant = { state: "granted" as const, grantRef: "grant.owner.repo" }
const availability = { state: "available" as const }

const entities = () => ({
  project: decodeCodingProjectEntity({ schema, projectRef: "project.oa", ownerScopeRef, displayName: "OpenAgents", aliasRefs: [], state: "active", createdAt: at, updatedAt: at, archivedAt: null }),
  repository: decodeCodingRepositoryEntity({ schema, repositoryRef: "repository.oa", projectRef: "project.oa", ownerScopeRef, displayName: "openagents", aliasRefs: ["repository.old-name"], pinnedBaseRef: "commit.base", availability, grant, createdAt: at, updatedAt: at }),
  worktree: decodeCodingWorktreeEntity({ schema, worktreeRef: "worktree.oa.main", repositoryRef: "repository.oa", projectRef: "project.oa", ownerScopeRef, displayName: "main", aliasRefs: ["worktree.alias.main"], baseRef: "commit.base", availability, grant, createdAt: at, updatedAt: at }),
  session: decodeCodingSessionEntity({ schema, sessionRef: "session.oa.1", ownerScopeRef, projectRef: "project.oa", repositoryRef: "repository.oa", worktreeRef: "worktree.oa.main", workContextRef: "work-context.oa.main", threadRef: "thread.oa.1", conversationRef: "conversation.oa.1", runRef: null, fleetRef: null, currentAttachmentRef: null, currentCheckpointRef: null, agentTopologyRef: null, canonicalEventCursor: 0, activityCursors: [], provider: { state: "unavailable", reason: "not_selected" }, runtime: { state: "unavailable", reason: "not_attached" }, grant, state: "active", createdAt: at, updatedAt: at, lastActiveAt: at, archivedAt: null }),
  navigation: decodeCodingNavigationEntity({ schema, navigationRef: "navigation.desktop", ownerScopeRef, selectedProjectRef: "project.oa", selectedRepositoryRef: "repository.old-name", selectedWorktreeRef: "worktree.alias.main", selectedSessionRef: "session.oa.1", openSessionRefs: ["session.oa.1", "session.oa.1"], focus: { kind: "editor", artifactRef: "artifact.readme" }, updatedAt: at }),
})

const entry = (version: number, entityType: string, entityId: string, value: unknown) => new ChangelogEntry({
  scope,
  version: SyncVersion.make(version),
  entityType: EntityType.make(entityType),
  entityId: EntityId.make(entityId),
  op: "upsert",
  postImageJson: JSON.stringify(value),
  mutationRef: `mutation.catalog.${version}`,
  committedAt: at,
})

const session = (phase: ScopeSyncState): KhalaSyncSession => ({
  state: (_scope: SyncScope) => phase,
  pending: () => [],
}) as unknown as KhalaSyncSession

describe("CUT-13 confirmed coding catalog read model", () => {
  test("reads exact owner-scope post-images and resolves restart navigation", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const value = entities()
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, "coding_project", value.project.projectRef, value.project),
        entry(1, "coding_repository", value.repository.repositoryRef, value.repository),
        entry(1, "coding_worktree", value.worktree.worktreeRef, value.worktree),
        entry(1, "coding_session", value.session.sessionRef, value.session),
        entry(1, "coding_navigation", value.navigation.navigationRef, value.navigation),
      ], SyncVersion.make(1)))
      const model = createKhalaSyncCodingCatalog({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(1) }),
        ownerScope: scope,
      })
      const snapshot = Effect.runSync(model.snapshot())
      expect(snapshot.status).toEqual({ phase: "live", cursor: 1, pendingMutationCount: 0 })
      expect(snapshot.issues).toEqual([])
      expect(snapshot.resolution).toMatchObject({
        state: "ready",
        navigation: {
          selectedRepositoryRef: "repository.oa",
          selectedWorktreeRef: "worktree.oa.main",
          openSessionRefs: ["session.oa.1"],
          focus: { kind: "editor", artifactRef: "artifact.readme" },
        },
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("ignores malformed, identity-mismatched, and cross-owner rows", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const value = entities()
      const other = { ...value.project, ownerScopeRef: "scope.user.other", projectRef: "project.other" }
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, "coding_project", "project.wrong-id", value.project),
        entry(2, "coding_project", "project.other", other),
        entry(3, "coding_project", "project.malformed", { schema: "wrong" }),
      ], SyncVersion.make(3)))
      const snapshot = Effect.runSync(createKhalaSyncCodingCatalog({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(3) }),
        ownerScope: scope,
      }).snapshot())
      expect(snapshot.catalog.projects).toEqual([])
      expect(snapshot.navigation).toBeNull()
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("hides cached authority until the owner scope is live", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const value = entities()
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, "coding_project", value.project.projectRef, value.project),
      ], SyncVersion.make(1)))
      const snapshot = Effect.runSync(createKhalaSyncCodingCatalog({
        store,
        session: session({ phase: "must_refetch", reason: "retention_gap" }),
        ownerScope: scope,
      }).snapshot())
      expect(snapshot).toMatchObject({
        status: { phase: "must_refetch", cursor: null },
        catalog: { projects: [], repositories: [], worktrees: [], sessions: [] },
        navigation: null,
        resolution: null,
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("selects navigation by confirmed scope version, not row order", () => {
    const store = openKhalaSyncStore(":memory:")
    try {
      const value = entities()
      const newer = decodeCodingNavigationEntity({
        ...value.navigation,
        navigationRef: "navigation.desktop.newer",
        focus: { kind: "agent", agentRef: "agent.newer" },
      })
      Effect.runSync(store.applyConfirmed(scope, [
        entry(1, "coding_project", value.project.projectRef, value.project),
        entry(1, "coding_repository", value.repository.repositoryRef, value.repository),
        entry(1, "coding_worktree", value.worktree.worktreeRef, value.worktree),
        entry(1, "coding_session", value.session.sessionRef, value.session),
        entry(1, "coding_navigation", value.navigation.navigationRef, value.navigation),
        entry(2, "coding_navigation", newer.navigationRef, newer),
      ], SyncVersion.make(2)))
      const snapshot = Effect.runSync(createKhalaSyncCodingCatalog({
        store,
        session: session({ phase: "live", cursor: SyncVersionWatermark.make(2) }),
        ownerScope: scope,
      }).snapshot())
      expect(snapshot.navigation?.navigationRef).toBe("navigation.desktop.newer")
      expect(snapshot.resolution).toMatchObject({
        state: "ready",
        navigation: { focus: { kind: "agent", agentRef: "agent.newer" } },
      })
    } finally {
      Effect.runSync(store.close())
    }
  })

  test("rejects non-owner scopes at construction", () => {
    expect(() => createKhalaSyncCodingCatalog({
      store: {} as never,
      session: {} as never,
      ownerScope: "scope.public.catalog" as SyncScope,
    })).toThrow()
  })
})
