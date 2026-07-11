import { describe, expect, test } from "bun:test"

import {
  CODING_NAVIGATION_ENTITY_TYPE,
  CODING_PROJECT_ENTITY_TYPE,
  CODING_REPOSITORY_ENTITY_TYPE,
  CODING_SESSION_ENTITY_TYPE,
  CODING_WORKTREE_ENTITY_TYPE,
  CodingNavigationEntity,
  decodeCodingNavigationEntity,
  decodeCodingProjectEntity,
  decodeCodingRepositoryEntity,
  decodeCodingSessionEntity,
  decodeCodingWorktreeEntity,
  encodeCodingNavigationEntity,
  encodeCodingProjectEntity,
  encodeCodingRepositoryEntity,
  encodeCodingSessionEntity,
  encodeCodingWorktreeEntity,
  queryCodingSessions,
  resolveCodingNavigation,
  validateCodingSessionCatalog,
  type CodingSessionCatalog,
} from "./index.js"

const schema = "openagents.coding_catalog.v1" as const
const ownerScopeRef = "scope.user.owner-1"
const at = (day: number): string => `2026-07-${String(day).padStart(2, "0")}T12:00:00.000Z`
const granted = { state: "granted" as const, grantRef: "grant.owner.repo" }
const available = { state: "available" as const }

const project = (overrides: Record<string, unknown> = {}) => decodeCodingProjectEntity({
  schema,
  projectRef: "project.openagents",
  ownerScopeRef,
  displayName: "OpenAgents",
  aliasRefs: ["project.alias.openagents"],
  state: "active",
  createdAt: at(1),
  updatedAt: at(11),
  archivedAt: null,
  ...overrides,
})

const repository = (overrides: Record<string, unknown> = {}) => decodeCodingRepositoryEntity({
  schema,
  repositoryRef: "repository.openagents",
  projectRef: "project.openagents",
  ownerScopeRef,
  displayName: "openagents",
  aliasRefs: ["repository.former-name"],
  pinnedBaseRef: "commit.0123456789abcdef",
  availability: available,
  grant: granted,
  createdAt: at(1),
  updatedAt: at(11),
  ...overrides,
})

const worktree = (overrides: Record<string, unknown> = {}) => decodeCodingWorktreeEntity({
  schema,
  worktreeRef: "worktree.openagents.main",
  repositoryRef: "repository.openagents",
  projectRef: "project.openagents",
  ownerScopeRef,
  displayName: "main",
  aliasRefs: ["worktree.alias.local-main"],
  baseRef: "commit.0123456789abcdef",
  availability: available,
  grant: granted,
  createdAt: at(1),
  updatedAt: at(11),
  ...overrides,
})

const session = (overrides: Record<string, unknown> = {}) => decodeCodingSessionEntity({
  schema,
  sessionRef: "session.openagents.1",
  ownerScopeRef,
  projectRef: "project.openagents",
  repositoryRef: "repository.openagents",
  worktreeRef: "worktree.openagents.main",
  workContextRef: "work-context.openagents.main",
  threadRef: "thread.openagents.1",
  conversationRef: "conversation.openagents.1",
  runRef: "run.openagents.1",
  fleetRef: null,
  currentAttachmentRef: "attachment.local.1",
  currentCheckpointRef: null,
  agentTopologyRef: "graph.openagents.1",
  canonicalEventCursor: 8,
  activityCursors: [{ threadRef: "thread.openagents.1", cursor: 5 }],
  provider: { state: "known", providerRef: "provider.codex.owner" },
  runtime: { state: "known", runtimeRef: "runtime.codex.local" },
  grant: granted,
  state: "active",
  createdAt: at(1),
  updatedAt: at(11),
  lastActiveAt: at(11),
  archivedAt: null,
  ...overrides,
})

const navigation = (overrides: Record<string, unknown> = {}) => decodeCodingNavigationEntity({
  schema,
  navigationRef: "navigation.desktop.primary",
  ownerScopeRef,
  selectedProjectRef: "project.alias.openagents",
  selectedRepositoryRef: "repository.former-name",
  selectedWorktreeRef: "worktree.alias.local-main",
  selectedSessionRef: "session.openagents.1",
  openSessionRefs: ["session.openagents.1", "session.openagents.1"],
  focus: { kind: "agent", agentRef: "agent.openagents.child" },
  updatedAt: at(11),
  ...overrides,
})

const catalog = (overrides: Partial<CodingSessionCatalog> = {}): CodingSessionCatalog => ({
  projects: [project()],
  repositories: [repository()],
  worktrees: [worktree()],
  sessions: [session()],
  ...overrides,
})

describe("CUT-13 canonical coding catalog schemas", () => {
  test("registers distinct stable entity types and round-trips every post-image", () => {
    expect([
      CODING_PROJECT_ENTITY_TYPE,
      CODING_REPOSITORY_ENTITY_TYPE,
      CODING_WORKTREE_ENTITY_TYPE,
      CODING_SESSION_ENTITY_TYPE,
      CODING_NAVIGATION_ENTITY_TYPE,
    ]).toEqual([
      "coding_project",
      "coding_repository",
      "coding_worktree",
      "coding_session",
      "coding_navigation",
    ])
    expect(decodeCodingProjectEntity(encodeCodingProjectEntity(project()))).toEqual(project())
    expect(decodeCodingRepositoryEntity(encodeCodingRepositoryEntity(repository()))).toEqual(repository())
    expect(decodeCodingWorktreeEntity(encodeCodingWorktreeEntity(worktree()))).toEqual(worktree())
    expect(decodeCodingSessionEntity(encodeCodingSessionEntity(session()))).toEqual(session())
    expect(decodeCodingNavigationEntity(encodeCodingNavigationEntity(navigation()))).toEqual(navigation())
  })

  test("structurally strips placement, provider-session, credential, and raw-path fields", () => {
    const decoded = decodeCodingSessionEntity({
      ...encodeCodingSessionEntity(session()),
      hostname: "owner-imac",
      localPath: "/Users/owner/work/openagents",
      processId: 1234,
      providerSessionId: "vendor-thread-secret",
      credential: "bearer-secret",
      transportHandle: "socket-7",
    }) as unknown as Record<string, unknown>
    for (const forbidden of [
      "hostname", "localPath", "processId", "providerSessionId", "credential", "transportHandle",
    ]) expect(decoded).not.toHaveProperty(forbidden)
    expect(() => session({ sessionRef: "/Users/owner/work/openagents" })).toThrow()
    expect(() => worktree({ aliasRefs: ["/Users/owner/work/openagents"] })).toThrow()
  })

  test("validates owner scope and exact project/repository/worktree relationships", () => {
    expect(validateCodingSessionCatalog(catalog())).toEqual([])
    expect(validateCodingSessionCatalog(catalog({
      sessions: [session({ ownerScopeRef: "scope.user.other" })],
    }))).toContainEqual({ code: "owner_scope_mismatch", affectedRef: "session.openagents.1" })
    expect(validateCodingSessionCatalog(catalog({
      worktrees: [worktree({ repositoryRef: "repository.other" })],
    }))).toEqual(expect.arrayContaining([
      { code: "missing_repository", affectedRef: "worktree.openagents.main" },
      { code: "worktree_repository_mismatch", affectedRef: "session.openagents.1" },
    ]))
  })

  test("ambiguous opaque aliases fail closed instead of choosing by array order", () => {
    const second = repository({
      repositoryRef: "repository.other",
      displayName: "other",
      aliasRefs: ["repository.former-name"],
    })
    const input = catalog({ repositories: [repository(), second] })
    expect(validateCodingSessionCatalog(input)).toContainEqual({
      code: "ambiguous_alias",
      affectedRef: "repository:repository.former-name",
    })
    expect(resolveCodingNavigation(input, navigation())).toMatchObject({
      state: "recovery_required",
      reason: "ambiguous_alias",
    })
    const collidesWithRef = catalog({
      repositories: [
        repository({ aliasRefs: ["repository.other"] }),
        repository({ repositoryRef: "repository.other", displayName: "other", aliasRefs: [] }),
      ],
    })
    expect(validateCodingSessionCatalog(collidesWithRef)).toContainEqual({
      code: "ambiguous_alias",
      affectedRef: "repository:repository.other",
    })
  })
})

describe("CUT-13 restart navigation resolver", () => {
  test("canonicalizes renamed/path aliases, collapses duplicate tabs, and preserves typed focus", () => {
    const restored = resolveCodingNavigation(catalog(), navigation())
    expect(restored.state).toBe("ready")
    if (restored.state !== "ready") throw new Error("expected ready navigation")
    expect(restored.navigation).toMatchObject({
      selectedProjectRef: "project.openagents",
      selectedRepositoryRef: "repository.openagents",
      selectedWorktreeRef: "worktree.openagents.main",
      selectedSessionRef: "session.openagents.1",
      openSessionRefs: ["session.openagents.1"],
      focus: { kind: "agent", agentRef: "agent.openagents.child" },
    })
  })

  test("missing worktrees and revoked grants produce named recovery states", () => {
    expect(resolveCodingNavigation(catalog({
      worktrees: [worktree({ availability: { state: "missing", reason: "not_found" } })],
    }), navigation())).toMatchObject({
      state: "recovery_required",
      reason: "missing_worktree",
      affectedRef: "worktree.openagents.main",
    })
    expect(resolveCodingNavigation(catalog({
      sessions: [session({
        grant: { state: "revoked", grantRef: "grant.owner.repo", revokedAt: at(11) },
      })],
    }), navigation())).toMatchObject({
      state: "recovery_required",
      reason: "grant_revoked",
      affectedRef: "session.openagents.1",
    })
    expect(resolveCodingNavigation(catalog({
      repositories: [repository({ grant: { state: "unavailable", reason: "not_projected" } })],
    }), navigation())).toMatchObject({
      state: "recovery_required",
      reason: "grant_unavailable",
    })
    expect(resolveCodingNavigation(catalog(), navigation({
      ownerScopeRef: "scope.user.other",
    }))).toMatchObject({
      state: "recovery_required",
      reason: "owner_scope_mismatch",
    })
  })

  test("bounded state model permits ready only when every authority fact is eligible", () => {
    const booleans = [false, true]
    let cases = 0
    for (const projectArchived of booleans) {
      for (const sessionArchived of booleans) {
        for (const repositoryMissing of booleans) {
          for (const worktreeMissing of booleans) {
            for (const sessionRevoked of booleans) {
              for (const worktreeRevoked of booleans) {
                cases += 1
                const input = catalog({
                  projects: [project(projectArchived ? { state: "archived", archivedAt: at(11) } : {})],
                  repositories: [repository(repositoryMissing
                    ? { availability: { state: "missing", reason: "not_found" } }
                    : {})],
                  worktrees: [worktree({
                    ...(worktreeMissing ? { availability: { state: "missing", reason: "not_found" } } : {}),
                    ...(worktreeRevoked ? {
                      grant: { state: "revoked", grantRef: "grant.owner.repo", revokedAt: at(11) },
                    } : {}),
                  })],
                  sessions: [session({
                    ...(sessionArchived ? { state: "archived", archivedAt: at(11) } : {}),
                    ...(sessionRevoked ? {
                      grant: { state: "revoked", grantRef: "grant.owner.repo", revokedAt: at(11) },
                    } : {}),
                  })],
                })
                const result = resolveCodingNavigation(input, navigation())
                const eligible = !projectArchived && !sessionArchived && !repositoryMissing &&
                  !worktreeMissing && !sessionRevoked && !worktreeRevoked
                expect(result.state === "ready").toBe(eligible)
              }
            }
          }
        }
      }
    }
    expect(cases).toBe(64)
  })

  test("structured query sorts recent active sessions and never implements text retrieval", () => {
    const sessions = [
      session({ sessionRef: "session.old", lastActiveAt: at(8), updatedAt: at(8) }),
      session({ sessionRef: "session.archived", state: "archived", archivedAt: at(11) }),
      session({ sessionRef: "session.new", lastActiveAt: at(11) }),
      session({ sessionRef: "session.other", projectRef: "project.other", lastActiveAt: at(12) }),
    ]
    expect(queryCodingSessions(catalog({ sessions }), {
      projectRef: "project.openagents",
      updatedAtOrAfter: at(8),
    }).map(value => value.sessionRef)).toEqual([
      "session.new",
      "session.old",
      "session.archived",
    ])
    expect(queryCodingSessions(catalog({ sessions }), {
      states: ["archived"],
    }).map(value => value.sessionRef)).toEqual(["session.archived"])
  })

  test("an empty navigation remains usable without inventing a current session", () => {
    const empty = navigation({
      selectedProjectRef: null,
      selectedRepositoryRef: null,
      selectedWorktreeRef: null,
      selectedSessionRef: null,
      openSessionRefs: [],
      focus: { kind: "none" },
    })
    const result = resolveCodingNavigation(catalog(), empty)
    expect(result).toEqual({ state: "empty", navigation: new CodingNavigationEntity(empty) })
  })
})
