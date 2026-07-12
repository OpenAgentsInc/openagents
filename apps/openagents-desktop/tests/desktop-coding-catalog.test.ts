import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { LocalIdentityRef } from "@openagentsinc/khala-sync"
import { Effect } from "effect"

import { openDesktopCodingCatalog } from "../src/desktop-coding-catalog.ts"
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
        repositories: [{ displayName: "first-workspace" }],
        worktrees: [{ displayName: "first-workspace", availability: { state: "available" } }],
        sessions: [{ state: "active" }],
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

  test("restart and duplicate workspace opens retain one stable session", () => {
    const h = fixture()
    let restartedStore: DesktopSyncStore | null = null
    let originalClosed = false
    try {
      const first = h.open().selectWorkspace(h.firstWorkspace)
      const sessionRef = first.catalog.sessions[0]!.sessionRef
      Effect.runSync(h.store.close())
      originalClosed = true
      restartedStore = openStore(path.join(h.root, "sync.sqlite"))
      const afterRestart = openDesktopCodingCatalog({
        store: restartedStore,
        identityRef: LocalIdentityRef.make("local_catalogfixture"),
        bindingFile: path.join(h.root, "private", "coding-bindings.json"),
        randomId: () => "must-not-generate",
        now: () => "2026-07-11T12:01:00.000Z",
      }).selectWorkspace(h.firstWorkspace)
      expect(afterRestart.catalog.sessions).toHaveLength(1)
      expect(afterRestart.catalog.sessions[0]?.sessionRef).toBe(sessionRef)
      expect(afterRestart.navigation?.openSessionRefs).toEqual([sessionRef])
    } finally {
      if (!originalClosed) Effect.runSync(h.store.close())
      if (restartedStore !== null) Effect.runSync(restartedStore.close())
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
      sessions: [{
        sessionRef: "session.desktop.fixture",
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
    ]).toEqual(["chat", "files", "terminal", "home", "home"])
  })
})
