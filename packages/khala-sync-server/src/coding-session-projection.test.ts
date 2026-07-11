import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import type { ChangelogEntry } from "@openagentsinc/khala-sync"

import {
  appendCodingCatalogChangeSet,
  decodeCodingCatalogChangeSet,
  projectCodingCatalogBestEffort,
} from "./coding-session-projection.js"
import type { AppendChangeInput, SyncTransactionWriter } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"
import { runMigrations } from "./migrate.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const at = "2026-07-11T12:00:00.000Z"
const schema = "openagents.coding_catalog.v1"
const ownerScopeRef = "scope.user.owner-1"
const grant = { state: "granted", grantRef: "grant.owner.repo" }
const availability = { state: "available" }

const changeSet = (overrides: Record<string, unknown> = {}) => ({
  ownerScopeRef,
  projects: [{
    schema,
    projectRef: "project.openagents",
    ownerScopeRef,
    displayName: "OpenAgents",
    aliasRefs: [],
    state: "active",
    createdAt: at,
    updatedAt: at,
    archivedAt: null,
  }],
  repositories: [{
    schema,
    repositoryRef: "repository.openagents",
    projectRef: "project.openagents",
    ownerScopeRef,
    displayName: "openagents",
    aliasRefs: [],
    pinnedBaseRef: "commit.base",
    availability,
    grant,
    createdAt: at,
    updatedAt: at,
  }],
  worktrees: [{
    schema,
    worktreeRef: "worktree.openagents.main",
    repositoryRef: "repository.openagents",
    projectRef: "project.openagents",
    ownerScopeRef,
    displayName: "main",
    aliasRefs: [],
    baseRef: "commit.base",
    availability,
    grant,
    createdAt: at,
    updatedAt: at,
  }],
  sessions: [{
    schema,
    sessionRef: "session.openagents.1",
    ownerScopeRef,
    projectRef: "project.openagents",
    repositoryRef: "repository.openagents",
    worktreeRef: "worktree.openagents.main",
    workContextRef: "work-context.openagents.main",
    threadRef: "thread.openagents.1",
    conversationRef: "conversation.openagents.1",
    runRef: null,
    fleetRef: null,
    currentAttachmentRef: null,
    currentCheckpointRef: null,
    agentTopologyRef: null,
    canonicalEventCursor: 0,
    activityCursors: [],
    provider: { state: "unavailable", reason: "not_selected" },
    runtime: { state: "unavailable", reason: "not_attached" },
    grant,
    state: "active",
    createdAt: at,
    updatedAt: at,
    lastActiveAt: at,
    archivedAt: null,
  }],
  navigation: {
    schema,
    navigationRef: "navigation.desktop.primary",
    ownerScopeRef,
    selectedProjectRef: "project.openagents",
    selectedRepositoryRef: "repository.openagents",
    selectedWorktreeRef: "worktree.openagents.main",
    selectedSessionRef: "session.openagents.1",
    openSessionRefs: ["session.openagents.1"],
    focus: { kind: "conversation", conversationRef: "conversation.openagents.1" },
    updatedAt: at,
  },
  ...overrides,
})

describe("CUT-13 owner-scoped coding catalog projection", () => {
  test("validates then appends every post-image at one exact owner scope", async () => {
    const writes: AppendChangeInput[] = []
    let inFlight = 0
    let maxInFlight = 0
    const writer = {
      appendChange: async (input: AppendChangeInput) => {
        inFlight += 1
        maxInFlight = Math.max(maxInFlight, inFlight)
        await Promise.resolve()
        writes.push(input)
        inFlight -= 1
        return {} as ChangelogEntry
      },
    } as SyncTransactionWriter
    const entries = await appendCodingCatalogChangeSet(
      writer,
      changeSet(),
      "mutation.coding-catalog.1",
    )
    expect(entries).toHaveLength(5)
    expect(writes.map(write => [String(write.scope), String(write.entityType), String(write.entityId)])).toEqual([
      [ownerScopeRef, "coding_project", "project.openagents"],
      [ownerScopeRef, "coding_repository", "repository.openagents"],
      [ownerScopeRef, "coding_worktree", "worktree.openagents.main"],
      [ownerScopeRef, "coding_session", "session.openagents.1"],
      [ownerScopeRef, "coding_navigation", "navigation.desktop.primary"],
    ])
    expect(writes.every(write => write.mutationRef === "mutation.coding-catalog.1")).toBe(true)
    expect(maxInFlight).toBe(1)
  })

  test("refuses cross-owner and broken relationship bundles before a write", () => {
    const crossOwner = changeSet({
      sessions: [{ ...changeSet().sessions[0], ownerScopeRef: "scope.user.other" }],
    })
    expect(() => decodeCodingCatalogChangeSet(crossOwner)).toThrow("owner scope mismatch")
    const broken = changeSet({
      worktrees: [{ ...changeSet().worktrees[0], repositoryRef: "repository.missing" }],
    })
    expect(() => decodeCodingCatalogChangeSet(broken)).toThrow("relation invalid")
    expect(() => decodeCodingCatalogChangeSet(changeSet({
      ownerScopeRef: "scope.device_local.local_fixture123",
      projects: changeSet().projects.map(value => ({
        ...value,
        ownerScopeRef: "scope.device_local.local_fixture123",
      })),
      repositories: [],
      worktrees: [],
      sessions: [],
      navigation: null,
    }))).toThrow()
  })

  test("refuses raw placement or credential-shaped material before storage", async () => {
    let touched = false
    const never = {
      begin: async () => {
        touched = true
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectCodingCatalogBestEffort(never, {
      ...changeSet(),
      localPath: "/Users/alice/private/openagents",
      token: "secret",
    })
    expect(outcome).toMatchObject({ ok: false, reason: "invalid" })
    expect(touched).toBe(false)
    if (!outcome.ok) {
      expect(outcome.messageSafe).not.toContain("alice")
      expect(outcome.messageSafe).not.toContain("secret")
    }
    expect(await projectCodingCatalogBestEffort(never, {
      ...changeSet(),
      ownerScopeRef: "not-a-scope",
    })).toEqual({
      ok: false,
      reason: "invalid",
      messageSafe: "coding catalog validation failed",
    })
    expect(touched).toBe(false)
  })

  test("storage failure emits a bounded diagnostic without leaking the cause", async () => {
    const broken = {
      begin: async () => { throw new Error("postgres://owner:secret@10.0.0.7") },
    } as unknown as SyncSql
    const outcome = await projectCodingCatalogBestEffort(broken, changeSet())
    expect(outcome).toEqual({
      ok: false,
      reason: "storage_failed",
      messageSafe: "coding catalog projection failed",
    })
  })
})

describe.skipIf(!hasLocalPostgres())("CUT-13 coding catalog projection against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_coding_catalog")
    await admin.end()
    const url = pg.urlFor("khala_sync_coding_catalog")
    await runMigrations({ databaseUrl: url })
    sql = new SQL({ url, max: 5 })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  test("commits a whole catalog at one dense owner-scope version", async () => {
    const first = await projectCodingCatalogBestEffort(sql as unknown as SyncSql, changeSet())
    const second = await projectCodingCatalogBestEffort(sql as unknown as SyncSql, changeSet())
    expect(first.ok).toBe(true)
    expect(second.ok).toBe(true)
    if (!first.ok || !second.ok) return
    expect(first.entries).toHaveLength(5)
    expect(new Set(first.entries.map(entry => Number(entry.version)))).toEqual(new Set([1]))
    expect(new Set(second.entries.map(entry => Number(entry.version)))).toEqual(new Set([2]))
    expect(first.entries.every(entry => String(entry.scope) === ownerScopeRef)).toBe(true)
  })
})
