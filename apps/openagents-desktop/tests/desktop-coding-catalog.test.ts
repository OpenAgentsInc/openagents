import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { LocalIdentityRef } from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import { openDesktopCodingCatalog } from "../src/desktop-coding-catalog.ts"
import { openAdmittedDesktopWorkspace } from "../src/desktop-workspace-admission.ts"
import {
  decodeDesktopCodingCatalogProjection,
  desktopWorkspaceForCodingFocus,
  filterDesktopCodingCatalog,
  parseDesktopCodingCatalogQuery,
  projectDesktopCodingCatalog,
} from "../src/coding-catalog-contract.ts"
import {
  openDesktopSyncStore,
  type DesktopSqliteDatabase,
  type DesktopSyncStore,
} from "../src/desktop-sync-store.ts"

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

const openStore = (file: string): DesktopSyncStore => openDesktopSyncStore(file, openBunDatabase)

const fixture = () => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-desktop-catalog-"))
  const firstWorkspace = path.join(root, "first-workspace")
  const secondWorkspace = path.join(root, "second-workspace")
  mkdirSync(firstWorkspace)
  mkdirSync(secondWorkspace)
  const store = openStore(path.join(root, "sync.sqlite"))
  let id = 0
  let tick = 0
  const open = () => openDesktopCodingCatalog({
    store,
    identityRef: LocalIdentityRef.make("local_catalogfixture"),
    bindingFile: path.join(root, "private", "coding-bindings.json"),
    randomId: () => `fixture-${++id}`,
    now: () => `2026-07-11T12:00:${String(++tick).padStart(2, "0")}.000Z`,
  })
  return { root, firstWorkspace, secondWorkspace, store, open }
}

describe("contract openagents_desktop.coding_catalog.restart_safe_navigation.v1", () => {
  test("persists canonical refs locally while raw paths remain in the private binding only", () => {
    const h = fixture()
    try {
      const selected = h.open().selectWorkspace(h.firstWorkspace)
      expect(selected.resolution?.state).toBe("ready")
      expect(selected.catalog).toMatchObject({
        projects: [{ displayName: "first-workspace" }],
        repositories: [{ displayName: "first-workspace", grant: { state: "granted" } }],
        worktrees: [{ displayName: "first-workspace", availability: { state: "available" }, grant: { state: "granted" } }],
        sessions: [{ state: "active", grant: { state: "granted" } }],
      })
      const rows = JSON.stringify(selected.catalog)
      expect(rows).not.toContain(h.root)
      const projected = projectDesktopCodingCatalog(selected)
      expect(decodeDesktopCodingCatalogProjection(projected)).toEqual(projected)
      expect(JSON.stringify(projected)).not.toContain(h.root)
      const bindingFile = path.join(h.root, "private", "coding-bindings.json")
      expect(readFileSync(bindingFile, "utf8")).toContain(h.firstWorkspace)
      if (process.platform !== "win32") {
        expect(statSync(path.dirname(bindingFile)).mode & 0o777).toBe(0o700)
        expect(statSync(bindingFile).mode & 0o777).toBe(0o600)
      }
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("projects bounded metadata pages across the complete age-unbounded catalog", () => {
    const h = fixture()
    try {
      const snapshot = h.open().selectWorkspace(h.firstWorkspace)
      const seed = snapshot.catalog.sessions[0]!
      const sessions = Array.from({ length: 205 }, (_, index) => ({
        ...seed,
        sessionRef: `session.desktop.page-${String(index).padStart(3, "0")}`,
        workContextRef: `work-context.desktop.page-${index}`,
        threadRef: `thread.desktop.page-${index}`,
        conversationRef: `conversation.desktop.page-${index}`,
        lastActiveAt: new Date(Date.parse("2026-07-11T12:00:00.000Z") - index * 1_000).toISOString(),
      }))
      const paged = { ...snapshot, catalog: { ...snapshot.catalog, sessions } }
      const first = projectDesktopCodingCatalog(paged)
      const second = projectDesktopCodingCatalog(paged, first.nextOffset!)
      const last = projectDesktopCodingCatalog(paged, second.nextOffset!)
      expect(first.sessions).toHaveLength(100)
      expect(first.pageOffset).toBe(0)
      expect(first.totalSessions).toBe(205)
      expect(first.nextOffset).toBe(100)
      expect(second.sessions).toHaveLength(100)
      expect(second.pageOffset).toBe(100)
      expect(second.nextOffset).toBe(200)
      expect(last.sessions).toHaveLength(5)
      expect(last.pageOffset).toBe(200)
      expect(last.nextOffset).toBeNull()
      expect(new Set([...first.sessions, ...second.sessions, ...last.sessions].map(value => value.sessionRef)).size).toBe(205)
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("restart and duplicate workspace opens retain one stable session", () => {
    const h = fixture()
    let restartedStore: DesktopSyncStore | null = null
    let originalClosed = false
    try {
      const firstCatalog = h.open()
      const first = firstCatalog.admitWorkspace(h.firstWorkspace)
      const firstAdmission = first.admission
      const sessionRef = first.snapshot.catalog.sessions[0]!.sessionRef
      Effect.runSync(h.store.close())
      originalClosed = true
      restartedStore = openStore(path.join(h.root, "sync.sqlite"))
      const afterRestartCatalog = openDesktopCodingCatalog({
        store: restartedStore,
        identityRef: LocalIdentityRef.make("local_catalogfixture"),
        bindingFile: path.join(h.root, "private", "coding-bindings.json"),
        randomId: () => "must-not-generate",
        now: () => "2026-07-11T12:01:00.000Z",
      })
      const afterRestart = afterRestartCatalog.admitWorkspace(h.firstWorkspace)
      expect(afterRestart.snapshot.catalog.sessions).toHaveLength(1)
      expect(afterRestart.snapshot.catalog.sessions[0]?.sessionRef).toBe(sessionRef)
      expect(afterRestart.snapshot.navigation?.openSessionRefs).toEqual([sessionRef])
      expect(afterRestart.admission).toEqual(firstAdmission)

      const opened = openAdmittedDesktopWorkspace(afterRestartCatalog, h.firstWorkspace)
      try {
        expect(opened.workspace.grantRef).toBe(firstAdmission.grantRef)
        expect(opened.admission.workContextRef).toBe(firstAdmission.workContextRef)
        expect(opened.admission.sessionRef).toBe(firstAdmission.sessionRef)
      } finally {
        opened.workspace.dispose()
      }
    } finally {
      if (!originalClosed) Effect.runSync(h.store.close())
      if (restartedStore !== null) Effect.runSync(restartedStore.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("one opaque admission binds repository, worktree, WorkContext, session, and picker grant", () => {
    const h = fixture()
    try {
      const catalog = h.open()
      const opened = openAdmittedDesktopWorkspace(catalog, h.firstWorkspace)
      try {
        const { admission } = opened
        const resolved = opened.catalog.resolution
        expect(resolved?.state).toBe("ready")
        if (resolved?.state !== "ready") return
        expect(resolved.repository.grant).toEqual({ state: "granted", grantRef: admission.grantRef })
        expect(resolved.worktree.grant).toEqual({ state: "granted", grantRef: admission.grantRef })
        expect(resolved.session.grant).toEqual({ state: "granted", grantRef: admission.grantRef })
        expect(resolved.session.workContextRef).toBe(admission.workContextRef)
        expect(resolved.session.sessionRef).toBe(admission.sessionRef)
        expect(opened.workspace.grantRef).toBe(admission.grantRef)

        const publicIdentity = JSON.stringify(admission)
        expect(publicIdentity).not.toContain(h.root)
        expect(publicIdentity).not.toContain("first-workspace")
        expect(publicIdentity).not.toContain(String(process.pid))
        expect(publicIdentity).not.toContain("localhost")
        expect(publicIdentity).not.toContain("provider")
        expect(publicIdentity).not.toContain("thread")

        const projection = projectDesktopCodingCatalog(opened.catalog)
        expect(projection.sessions[0]).toMatchObject({
          sessionRef: admission.sessionRef,
          workContextRef: admission.workContextRef,
          grantRef: admission.grantRef,
        })
        expect(JSON.stringify(projection)).not.toContain(h.root)
      } finally {
        opened.workspace.dispose()
      }
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("conversation focus rebinds the selected coding session to the exact chat thread", () => {
    const h = fixture()
    try {
      const catalog = h.open()
      const selected = catalog.selectWorkspace(h.firstWorkspace).catalog.sessions[0]!
      const threadRef = "thread.desktop.real-chat"
      const rebound = catalog.saveFocus(selected.sessionRef, {
        kind: "conversation",
        conversationRef: threadRef,
      })
      expect(rebound.catalog.sessions[0]).toMatchObject({
        sessionRef: selected.sessionRef,
        threadRef,
        conversationRef: threadRef,
      })
      expect(rebound.navigation?.focus).toEqual({
        kind: "conversation",
        conversationRef: threadRef,
      })
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("open, focus, recent sort, and archive survive as typed navigation", () => {
    const h = fixture()
    try {
      const catalog = h.open()
      const first = catalog.selectWorkspace(h.firstWorkspace).catalog.sessions[0]!
      const second = catalog.selectWorkspace(h.secondWorkspace).catalog.sessions.find(
        value => value.sessionRef !== first.sessionRef,
      )!
      expect(catalog.query({ states: ["active"] }).map(value => value.sessionRef)).toEqual([
        second.sessionRef,
        first.sessionRef,
      ])
      expect(catalog.openSession(first.sessionRef).navigation?.selectedSessionRef).toBe(first.sessionRef)
      expect(catalog.saveFocus(first.sessionRef, {
        kind: "editor",
        artifactRef: "artifact.readme",
      }).navigation?.focus).toEqual({ kind: "editor", artifactRef: "artifact.readme" })
      const archived = catalog.archiveSession(first.sessionRef)
      expect(archived.catalog.sessions.find(value => value.sessionRef === first.sessionRef)?.state).toBe("archived")
      expect(archived.navigation?.selectedSessionRef).toBe(second.sessionRef)
      expect(archived.navigation?.openSessionRefs).toEqual([second.sessionRef])
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("permanent deletion requires archive and removes only orphaned local identities", () => {
    const h = fixture()
    try {
      const catalog = h.open()
      const first = catalog.selectWorkspace(h.firstWorkspace).catalog.sessions[0]!
      const second = catalog.selectWorkspace(h.secondWorkspace).catalog.sessions.find(
        value => value.sessionRef !== first.sessionRef,
      )!
      expect(catalog.deleteSession(first.sessionRef).catalog.sessions).toHaveLength(2)

      catalog.archiveSession(first.sessionRef)
      const deleted = catalog.deleteSession(first.sessionRef)
      expect(deleted.catalog.sessions.map(value => value.sessionRef)).toEqual([second.sessionRef])
      expect(deleted.catalog.projects).toHaveLength(1)
      expect(deleted.catalog.repositories).toHaveLength(1)
      expect(deleted.catalog.worktrees).toHaveLength(1)
      expect(deleted.navigation?.selectedSessionRef).toBe(second.sessionRef)
      expect(deleted.navigation?.openSessionRefs).toEqual([second.sessionRef])

      const bindings = readFileSync(path.join(h.root, "private", "coding-bindings.json"), "utf8")
      expect(bindings).not.toContain(h.firstWorkspace)
      expect(bindings).toContain(h.secondWorkspace)
      expect(catalog.deleteSession(first.sessionRef)).toEqual(catalog.snapshot())
      expect(h.open().snapshot().catalog.sessions.map(value => value.sessionRef)).toEqual([second.sessionRef])
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("missing worktree recovery preserves canonical session and worktree refs", () => {
    const h = fixture()
    try {
      const catalog = h.open()
      const initial = catalog.selectWorkspace(h.firstWorkspace)
      const sessionRef = initial.catalog.sessions[0]!.sessionRef
      const worktreeRef = initial.catalog.worktrees[0]!.worktreeRef
      rmSync(h.firstWorkspace, { recursive: true, force: true })
      expect(catalog.snapshot().resolution).toMatchObject({
        state: "recovery_required",
        reason: "missing_worktree",
      })
      const recovered = catalog.recoverSession(sessionRef, h.secondWorkspace)
      expect(recovered.resolution?.state).toBe("ready")
      expect(recovered.catalog.sessions.find(value => value.sessionRef === sessionRef)?.worktreeRef).toBe(worktreeRef)
      expect(recovered.catalog.worktrees.find(value => value.worktreeRef === worktreeRef)).toMatchObject({
        displayName: "second-workspace",
        availability: { state: "available" },
      })
    } finally {
      Effect.runSync(h.store.close())
      rmSync(h.root, { recursive: true, force: true })
    }
  })

  test("structured search and every persisted focus kind are explicit and deterministic", () => {
    expect(parseDesktopCodingCatalogQuery("project:project.desktop.fixture state:active")).toEqual({
      state: "valid",
      plan: { projectRef: "project.desktop.fixture", state: "active" },
    })
    expect(parseDesktopCodingCatalogQuery("free text")).toEqual({
      state: "invalid",
      reason: "Use field:value filters.",
    })
    expect(filterDesktopCodingCatalog({
      authority: "device_local",
      authorityLabel: "This Mac",
      selectedSessionRef: null,
      focus: { kind: "none" },
      pageOffset: 0,
      totalSessions: 1,
      nextOffset: null,
      activeCount: 1,
      recoveryCount: 0,
      archivedCount: 0,
      sessions: [{
        sessionRef: "session.desktop.fixture",
        workContextRef: "work-context.desktop.fixture",
        grantRef: "workspace.grant.desktop.fixture",
        projectRef: "project.desktop.fixture",
        repositoryRef: "repository.desktop.fixture",
        worktreeRef: "worktree.desktop.fixture",
        projectLabel: "Fixture",
        repositoryLabel: "fixture",
        worktreeLabel: "main",
        state: "active",
        lastActiveAt: "2026-07-11T12:00:00.000Z",
        recoveryReason: null,
      }],
    }, { projectRef: "project.desktop.fixture", state: "active" })).toHaveLength(1)
    expect([
      desktopWorkspaceForCodingFocus({ kind: "conversation", conversationRef: "conversation.fixture" }),
      desktopWorkspaceForCodingFocus({ kind: "editor", artifactRef: "artifact.fixture" }),
      desktopWorkspaceForCodingFocus({ kind: "terminal", terminalRef: "terminal.fixture" }),
      desktopWorkspaceForCodingFocus({ kind: "agent", agentRef: "agent.fixture" }),
      desktopWorkspaceForCodingFocus({ kind: "none" }),
    ]).toEqual(["chat", "files", "home", "home", "home"])
  })
})
