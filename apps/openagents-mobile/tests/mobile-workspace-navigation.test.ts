import { describe, expect, test } from "vite-plus/test"
import type { ConfirmedRuntimeAttentionSnapshot } from "@openagentsinc/khala-sync-client"

import type { MobileControllerDirectory } from "../src/coding/mobile-controller-directory"
import { projectMobileWorkspaceNavigation } from "../src/screens/mobile-workspace-navigation"

const now = new Date("2026-07-17T20:00:00.000Z")
const threads = [{
  threadRef: "thread.coding",
  title: "Fix mobile transcript",
  status: "active" as const,
  messageCount: 4,
  lastMessageAt: "2026-07-17T19:58:00.000Z",
  updatedAt: "2026-07-17T19:58:00.000Z",
  version: 4,
}, {
  threadRef: "thread.chat",
  title: "Release checklist",
  status: "active" as const,
  messageCount: 2,
  lastMessageAt: "2026-07-16T20:00:00.000Z",
  updatedAt: "2026-07-16T20:00:00.000Z",
  version: 2,
}]
const archivedThreads = [{
  ...threads[1]!,
  threadRef: "thread.archived",
  title: "Old investigation",
  status: "archived" as const,
}]
const directory: MobileControllerDirectory = {
  authority: "confirmed",
  phase: "live",
  cacheState: "current",
  offlineCache: {
    accounting: "live_confirmed",
    ownerScopeRef: "scope.owner",
    cachedRepositoryCount: 1,
    cachedSessionCount: 1,
    lastConfirmedCursor: 8,
  },
  summary: { repositoryCount: 1, sessionCount: 1, attentionCount: 0 },
  repositories: [],
  attention: [],
  recent: [{
    sessionRef: "session.coding",
    projectRef: "project.openagents",
    repositoryRef: "repository.openagents",
    repositoryName: "openagents",
    worktreeRef: "worktree/codex/mobile-transcript",
    threadRef: "thread.coding",
    runRef: "run.coding",
    fleetRef: null,
    currentCheckpointRef: null,
    agentTopologyRef: null,
    canonicalEventCursor: 8,
    provider: { state: "known", providerRef: "provider.codex" },
    runtime: { state: "known", runtimeRef: "runtime.desktop" },
    state: "active",
    targetReadiness: "ready",
    attention: "none",
    lastActiveAt: "2026-07-17T19:59:00.000Z",
  }],
}
const attention: ConfirmedRuntimeAttentionSnapshot = {
  status: { phase: "live", cursor: 9 },
  pending: [{
    schema: "openagents.runtime_attention.v1",
    attentionRef: "attention.coding",
    ownerUserId: "owner.mobile",
    interactionRef: "interaction.coding",
    threadRef: "thread.coding",
    turnRef: "turn.coding",
    kind: "tool_approval",
    status: "pending",
    requestedAt: "2026-07-17T19:59:30.000Z",
    expiresAt: "2026-07-17T20:09:30.000Z",
    updatedAt: "2026-07-17T19:59:30.000Z",
  }],
  terminal: [],
  issues: [],
}

const project = (overrides: Partial<Parameters<typeof projectMobileWorkspaceNavigation>[0]> = {}) =>
  projectMobileWorkspaceNavigation({
    threads,
    archivedThreads,
    directory,
    attention,
    activeThreadRef: "thread.coding",
    search: "",
    status: "all",
    projectRef: null,
    now,
    ...overrides,
  })

describe("contract openagents_mobile.workspace_navigation.v1", () => {
  test("joins coding metadata into one selected project-aware row without primary raw refs", () => {
    const projection = project()
    const coding = projection.rows.find(row => row.kind === "coding_session")
    expect(coding).toMatchObject({
      title: "Fix mobile transcript",
      projectLabel: "openagents",
      worktreeLabel: "mobile-transcript",
      recencyLabel: "1m",
      state: "active",
      stateLabel: "Running",
      selected: true,
    })
    expect(coding?.title).not.toContain("thread.")
    expect(projection.rows.filter(row => row.threadRef === "thread.coding" && row.kind !== "attention"))
      .toHaveLength(1)
  })

  test("keeps exact causal attention identity in the shared row grammar", () => {
    const row = project({ status: "attention" }).rows[0]
    expect(row).toMatchObject({
      kind: "attention",
      title: "Fix mobile transcript",
      projectLabel: "openagents",
      stateLabel: "Approval",
      attentionTarget: {
        attentionRef: "attention.coding",
        threadRef: "thread.coding",
        turnRef: "turn.coding",
      },
    })
  })

  test("searches authorized labels, filters by exact project/status, and isolates archive", () => {
    expect(project({ search: "transcript" }).rows.map(row => row.kind))
      .toEqual(["attention", "coding_session"])
    expect(project({ projectRef: "project.openagents" }).rows.every(row =>
      row.projectRef === "project.openagents")).toBe(true)
    expect(project({ status: "idle" }).rows.map(row => row.title)).toEqual(["Release checklist"])
    expect(project({ status: "archived" }).rows).toMatchObject([{
      title: "Old investigation", state: "archived",
    }])
    expect(project({ search: "missing" })).toMatchObject({ rows: [], totalRowCount: 0 })
  })

  test("withholds attention on an invalid projection and reports unavailable project filters", () => {
    expect(project({ attention: { ...attention, issues: [{
      code: "malformed",
      affectedRef: "attention.coding",
    }] } }).rows.some(row => row.kind === "attention")).toBe(false)
    expect(project({ projectRef: "project.foreign" })).toMatchObject({
      rows: [], selectedProjectAvailable: false,
    })
  })
})
