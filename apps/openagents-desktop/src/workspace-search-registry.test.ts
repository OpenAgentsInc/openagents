import { describe, expect, test } from "vite-plus/test"

import {
  decodeWorkspaceSearchBridgeRequest,
  decodeWorkspaceSearchCancelRequest,
  decodeWorkspaceSearchCancelResult,
  decodeWorkspaceSearchResponse,
  type DesktopWorkspaceSearchPage,
} from "./workspace-contract.ts"
import { makeWorkspaceSearchRegistry } from "./workspace-search-registry.ts"
import type { DesktopWorkspaceService } from "./workspace-service.ts"

type Pending = Readonly<{
  resolve: (page: DesktopWorkspaceSearchPage) => void
  cancelCount: () => number
}>

const fixtureWorkspace = (pending: Pending[]): DesktopWorkspaceService => ({
  grantRef: "workspace.grant.registry",
  summary: () => { throw new Error("unused") },
  tree: () => ({ state: "unavailable", message: "unused" }),
  search: request => {
    let cancelled = 0
    let settled = false
    let resolveResult!: (page: DesktopWorkspaceSearchPage) => void
    const result = new Promise<DesktopWorkspaceSearchPage>(resolve => {
      resolveResult = page => {
        if (settled) return
        settled = true
        resolve(page)
      }
    })
    pending.push({
      resolve: resolveResult,
      cancelCount: () => cancelled,
    })
    return {
      taskRef: `workspace.search.task.${pending.length}`,
      result,
      cancel: () => {
        cancelled += 1
        resolveResult({ state: "unavailable", message: `cancelled:${request.query}` })
      },
    }
  },
  createEntry: () => ({ state: "unavailable", message: "unused" }),
  renameEntry: () => ({ state: "unavailable", message: "unused" }),
  moveEntry: () => ({ state: "unavailable", message: "unused" }),
  copyEntry: () => ({ state: "unavailable", message: "unused" }),
  duplicateEntry: () => ({ state: "unavailable", message: "unused" }),
  deleteEntry: () => ({ state: "unavailable", message: "unused" }),
  revealEntry: async () => ({ state: "unavailable", message: "unused" }),
  openDocument: () => ({ state: "unavailable", reason: "unavailable", message: "unused" }),
    saveDocument: () => ({ state: "unavailable", reason: "unavailable", message: "unused" }),
    saveDocumentAs: () => ({ state: "unavailable", reason: "unavailable", message: "unused" }),
  refresh: () => undefined,
  subscribe: () => ({ close: () => undefined }),
  read: () => null,
  save: () => ({ state: "unavailable", message: "unused" }),
  gitStatus: () => ({ state: "unavailable" }),
  gitDiff: () => ({ state: "unavailable", message: "unused" }),
  dispose: () => undefined,
})

const request = (requestRef: string, query: string) => ({
  requestRef,
  query,
  mode: "path" as const,
})

describe("Workspace search webContents registry", () => {
  test("decodes only fixed bounded search and cancellation bridge shapes", () => {
    expect(decodeWorkspaceSearchBridgeRequest({
      requestRef: "workspace.search.request.fixture",
      query: "needle",
      mode: "content",
      offset: 0,
      limit: 40,
      root: "/private/root",
    })).toEqual({
      requestRef: "workspace.search.request.fixture",
      query: "needle",
      mode: "content",
      offset: 0,
      limit: 40,
    })
    expect(decodeWorkspaceSearchBridgeRequest({ requestRef: "bad", query: "needle", mode: "content" })).toBeNull()
    expect(decodeWorkspaceSearchBridgeRequest({
      requestRef: "workspace.search.request.fixture",
      query: "needle",
      mode: "content",
      limit: 101,
    })).toBeNull()
    expect(decodeWorkspaceSearchCancelRequest({ requestRef: "workspace.search.request.fixture" })).toEqual({
      requestRef: "workspace.search.request.fixture",
    })
    expect(decodeWorkspaceSearchCancelRequest({ requestRef: "workspace.search.request.fixture", root: "/private" })).toEqual({
      requestRef: "workspace.search.request.fixture",
    })
    expect(decodeWorkspaceSearchCancelResult({
      requestRef: "workspace.search.request.fixture",
      cancelled: true,
    })).toEqual({ requestRef: "workspace.search.request.fixture", cancelled: true })
    expect(decodeWorkspaceSearchResponse({
      requestRef: "workspace.search.request.fixture",
      page: {
        state: "available",
        grantRef: "workspace.grant.fixture",
        query: "needle",
        mode: "path",
        matches: [],
        nextOffset: null,
        truncated: false,
        cache: { key: "workspace.search.fixture", epoch: 1, freshness: "current" },
      },
    })?.requestRef).toBe("workspace.search.request.fixture")
    expect(decodeWorkspaceSearchResponse({
      requestRef: "workspace.search.request.fixture",
      page: { state: "available", root: "/private" },
    })).toBeNull()
  })

  test("replaces one owner's task while exact cancellation is fenced by owner and request", async () => {
    const pending: Pending[] = []
    const registry = makeWorkspaceSearchRegistry(() => fixtureWorkspace(pending))
    const first = registry.start("webContents.1", request("workspace.search.request.first", "first"))
    const second = registry.start("webContents.1", request("workspace.search.request.second", "second"))
    expect(await first).toEqual({
      requestRef: "workspace.search.request.first",
      page: { state: "unavailable", message: "cancelled:first" },
    })
    expect(pending[0]!.cancelCount()).toBe(1)
    expect(registry.activeCount()).toBe(1)

    expect(registry.cancel("webContents.2", "workspace.search.request.second")).toEqual({
      requestRef: "workspace.search.request.second",
      cancelled: false,
    })
    expect(registry.cancel("webContents.1", "workspace.search.request.first").cancelled).toBe(false)
    expect(registry.cancel("webContents.1", "workspace.search.request.second").cancelled).toBe(true)
    expect(await second).toEqual({
      requestRef: "workspace.search.request.second",
      page: { state: "unavailable", message: "cancelled:second" },
    })
    expect(pending[1]!.cancelCount()).toBe(1)
    expect(registry.activeCount()).toBe(0)
  })

  test("returns current results and closes every remaining owner exactly once", async () => {
    const pending: Pending[] = []
    const workspace = fixtureWorkspace(pending)
    const registry = makeWorkspaceSearchRegistry(() => workspace)
    const complete = registry.start("webContents.1", request("workspace.search.request.complete", "ready"))
    pending[0]!.resolve({
      state: "available",
      grantRef: workspace.grantRef,
      query: "ready",
      mode: "path",
      matches: [{ pathRef: "README.md", kind: "path", line: null, preview: null }],
      nextOffset: null,
      truncated: false,
      cache: { key: "workspace.search.cache", epoch: 3, freshness: "current" },
    })
    expect((await complete).page.state).toBe("available")
    expect(registry.activeCount()).toBe(0)

    const closingA = registry.start("webContents.1", request("workspace.search.request.a", "a"))
    const closingB = registry.start("webContents.2", request("workspace.search.request.b", "b"))
    registry.closeOwner("webContents.1")
    registry.closeOwner("webContents.1")
    expect((await closingA).page).toEqual({ state: "unavailable", message: "cancelled:a" })
    expect(pending[1]!.cancelCount()).toBe(1)
    registry.dispose()
    registry.dispose()
    expect((await closingB).page).toEqual({ state: "unavailable", message: "cancelled:b" })
    expect(pending[2]!.cancelCount()).toBe(1)
    expect(registry.activeCount()).toBe(0)
    expect((await registry.start("webContents.3", request("workspace.search.request.closed", "closed"))).page.state).toBe("unavailable")
  })

  test("fails closed when no WorkContext exists", async () => {
    const registry = makeWorkspaceSearchRegistry(() => null)
    expect(await registry.start("webContents.1", request("workspace.search.request.none", "none"))).toEqual({
      requestRef: "workspace.search.request.none",
      page: { state: "unavailable", message: "Choose a workspace folder before searching." },
    })
    expect(registry.activeCount()).toBe(0)
  })
})
