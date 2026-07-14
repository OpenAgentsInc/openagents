/**
 * CUT-17 workspace browser tests: grant fencing and bounded hierarchy state,
 * plus the pure Effect Native projection for explicit states and mutations.
 */
import { describe, expect, test } from "vite-plus/test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import type {
  DesktopWorkspaceSearchPage,
  DesktopWorkspaceTreeEntry,
  DesktopWorkspaceTreePage,
} from "../workspace-contract.ts"
import {
  emptyWorkspaceBrowserState,
  makeWorkspaceBrowserHandlers,
  visibleWorkspaceRows,
  withWorkspaceBrowserEditor,
  withWorkspaceBrowserLoading,
  withWorkspaceBrowserOperation,
  withWorkspaceBrowserPage,
  withWorkspaceBrowserRoot,
  withWorkspaceBrowserSearch,
  withWorkspaceBrowserSearchStarted,
  withWorkspaceBrowserToggled,
  workspaceBrowserIntents,
  workspaceBrowserView,
  type WorkspaceBrowserBridge,
  type WorkspaceBrowserState,
} from "./workspace-browser.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) { for (const item of value) walk(item); return }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find(node => node.key === key)

const pressIntent = (view: View, key: string) => {
  const node = nodeByKey(view, key) as { onPress: Parameters<typeof resolveIntentRef>[0] }
  return resolveIntentRef(node.onPress, null)
}

const entry = (
  pathRef: string,
  kind: "file" | "directory" = "file",
  revisionRef = `revision-${pathRef}`,
): DesktopWorkspaceTreeEntry => ({
  name: pathRef.split("/").at(-1) ?? pathRef,
  pathRef,
  kind,
  expandable: kind === "directory",
  sizeBytes: kind === "file" ? 12 : null,
  revisionRef,
})

const treePage = (
  directoryRef: string,
  entries: ReadonlyArray<DesktopWorkspaceTreeEntry>,
  overrides: Partial<Extract<DesktopWorkspaceTreePage, { state: "available" }>> = {},
): Extract<DesktopWorkspaceTreePage, { state: "available" }> => ({
  state: "available",
  grantRef: "grant-1",
  directoryRef,
  entries,
  nextOffset: null,
  cache: { key: `tree:${directoryRef}`, epoch: 1, freshness: "current" },
  ...overrides,
})

const searchPage = (
  overrides: Partial<Extract<DesktopWorkspaceSearchPage, { state: "available" }>> = {},
): Extract<DesktopWorkspaceSearchPage, { state: "available" }> => ({
  state: "available",
  grantRef: "grant-1",
  query: "needle",
  mode: "content",
  matches: [{ pathRef: "src/main.ts", kind: "content", line: 7, preview: "const needle = true" }],
  nextOffset: null,
  truncated: false,
  cache: { key: "search:needle", epoch: 1, freshness: "current" },
  ...overrides,
})

const readyState = (
  entries: ReadonlyArray<DesktopWorkspaceTreeEntry> = [entry("src", "directory"), entry("README.md")],
): WorkspaceBrowserState => withWorkspaceBrowserRoot(emptyWorkspaceBrowserState(), treePage("", entries))

describe("workspace browser state", () => {
  test("moves through idle, loading, and a relative-ref root projection", () => {
    const idle = emptyWorkspaceBrowserState()
    expect(idle.phase).toBe("idle")
    expect(withWorkspaceBrowserLoading(idle).phase).toBe("loading")

    const ready = readyState()
    expect(ready.phase).toBe("ready")
    expect(ready.grantRef).toBe("grant-1")
    expect(ready.pages[""]?.entries.map(item => item.pathRef)).toEqual(["src", "README.md"])
  })

  test("expands lazy pages in depth-first order without duplicating paged entries", () => {
    let state = withWorkspaceBrowserToggled(readyState(), "src")
    expect(state.loadingRefs).toEqual(["src"])
    state = withWorkspaceBrowserPage(state, treePage("src", [entry("src/a.ts"), entry("src/b.ts")], { nextOffset: 2 }))
    state = withWorkspaceBrowserPage(state, treePage("src", [entry("src/b.ts"), entry("src/c.ts")], { nextOffset: null }))

    expect(visibleWorkspaceRows(state).rows.map(row => [row.entry.pathRef, row.depth])).toEqual([
      ["src", 0],
      ["src/a.ts", 1],
      ["src/b.ts", 1],
      ["src/c.ts", 1],
      ["README.md", 0],
    ])
    expect(state.loadingRefs).toEqual([])
  })

  test("rejects a page or search result from a different workspace grant", () => {
    const state = readyState()
    const wrongTree = withWorkspaceBrowserPage(state, treePage("src", [entry("src/private.ts")], { grantRef: "grant-2" }))
    expect(wrongTree.pages["src"]).toBeUndefined()
    expect(wrongTree.operation).toEqual({ state: "unavailable", message: "Workspace authority changed. Refresh files." })

    const searching = withWorkspaceBrowserSearchStarted(state)
    const wrongSearch = withWorkspaceBrowserSearch(searching, searchPage({ grantRef: "grant-2" }))
    expect(wrongSearch.searchState).toBe("unavailable")
    expect(wrongSearch.searchPage).toBeNull()
    expect(wrongSearch.operation?.state).toBe("unavailable")
  })

  test("an unavailable root clears all prior authority-bearing projections", () => {
    const prior = {
      ...readyState(),
      selectedRef: "README.md",
      searchPage: searchPage(),
      searchState: "ready" as const,
    }
    const unavailable = withWorkspaceBrowserRoot(prior, { state: "unavailable", message: "Folder access ended." })
    expect(unavailable.phase).toBe("unavailable")
    expect(unavailable.grantRef).toBeNull()
    expect(unavailable.pages).toEqual({})
    expect(unavailable.selectedRef).toBeNull()
    expect(unavailable.searchPage).toBeNull()
  })

  test("operation receipts close editors and update or clear the selection", () => {
    const selected = { ...readyState(), selectedRef: "README.md" }
    const editing = withWorkspaceBrowserEditor(selected, {
      kind: "rename",
      pathRef: "README.md",
      expectedRevisionRef: "revision-README.md",
      value: "GUIDE.md",
    })
    const renamed = withWorkspaceBrowserOperation(editing, { state: "renamed", entry: entry("GUIDE.md") })
    expect(renamed.editor).toBeNull()
    expect(renamed.selectedRef).toBe("GUIDE.md")

    const deleted = withWorkspaceBrowserOperation({ ...selected, deleteConfirmRef: "README.md" }, { state: "deleted", pathRef: "README.md" })
    expect(deleted.selectedRef).toBeNull()
    expect(deleted.deleteConfirmRef).toBeNull()
  })

  test("the visible hierarchy is bounded independently of input size", () => {
    const state = readyState(Array.from({ length: 501 }, (_, index) => entry(`file-${index}.ts`)))
    expect(visibleWorkspaceRows(state)).toMatchObject({ truncated: true })
    expect(visibleWorkspaceRows(state).rows).toHaveLength(500)
  })
})

describe("workspace browser Effect Native view", () => {
  test("idle and unavailable states are explicit and withhold the file controls", () => {
    const idle = workspaceBrowserView(emptyWorkspaceBrowserState())
    expect(nodeByKey(idle, "workspace-browser-idle")?.content).toContain("Choose a local folder")
    expect(nodeByKey(idle, "workspace-browser-tree-list")).toBeUndefined()

    const unavailable = workspaceBrowserView(withWorkspaceBrowserRoot(readyState(), { state: "unavailable", message: "Permission ended." }))
    expect(nodeByKey(unavailable, "workspace-browser-unavailable-reason")?.content).toBe("Permission ended.")
    expect(nodeByKey(unavailable, "workspace-browser-unavailable-retry")).toBeDefined()
    expect(nodeByKey(unavailable, "workspace-browser-tree-list")).toBeUndefined()
  })

  test("ready hierarchy is virtualized, accessible, and contains only relative refs", () => {
    const view = workspaceBrowserView(readyState())
    expect(nodeByKey(view, "workspace-browser-tree-list")?.virtualize).toBe(true)
    expect(nodeByKey(view, "workspace-browser-tree-list")?.a11y).toEqual({ role: "tree", label: "Workspace files" })
    expect((nodeByKey(view, "workspace-browser-toggle-src")?.a11y as { expanded?: boolean }).expanded).toBe(false)
    expect((nodeByKey(view, "workspace-browser-toggle-src") as { accessibilityLabel?: string }).accessibilityLabel).toBe("Expand folder src")
    expect((nodeByKey(view, "workspace-browser-select-README.md")?.a11y as { label?: string }).label).toBe("File README.md")
    expect(nodeByKey(view, "workspace-browser-select-README.md")?.a11y).toMatchObject({ role: "treeitem", level: 1 })
    expect(JSON.stringify(view)).not.toContain("/Users/")
  })

  test("search mode, cancellation, bounded result, and pagination states are visible", () => {
    const searching = workspaceBrowserView({ ...readyState(), query: "needle", searchState: "searching" })
    expect(nodeByKey(searching, "workspace-browser-search-submit")?.label).toBe("Cancel")
    expect((nodeByKey(searching, "workspace-browser-mode-path")?.a11y as { selected?: boolean }).selected).toBe(true)

    const result = workspaceBrowserView({
      ...readyState(),
      query: "needle",
      searchMode: "content",
      searchState: "ready",
      searchPage: searchPage({ truncated: true, nextOffset: 1 }),
    })
    expect(nodeByKey(result, "workspace-browser-search-list")?.virtualize).toBe(true)
    expect(nodeByKey(result, "workspace-browser-search-select-src/main.ts-0")?.label).toBe("src/main.ts:7")
    expect(nodeByKey(result, "workspace-browser-search-preview-src/main.ts-0")?.content).toBe("const needle = true")
    expect(nodeByKey(result, "workspace-browser-search-truncated")?.content).toContain("Refine the query")
    expect(nodeByKey(result, "workspace-browser-search-more")?.label).toBe("More results")
  })

  test("UX-4 (#8790): no filesystem mutation affordance renders — create/rename/delete/reveal stay substrate-only", () => {
    // The MVP spec grants a bounded file tree for review (CW-AC-14), not
    // grant-scoped file management. Even with an active editor/confirm state,
    // the view renders no mutation control.
    const create = workspaceBrowserView(withWorkspaceBrowserEditor(readyState(), {
      kind: "create_file",
      parentRef: "",
      value: "notes.md",
    }))
    expect(nodeByKey(create, "workspace-browser-editor-name")).toBeUndefined()
    expect(nodeByKey(create, "workspace-browser-editor-submit")).toBeUndefined()
    expect(nodeByKey(create, "workspace-browser-new-file")).toBeUndefined()
    expect(nodeByKey(create, "workspace-browser-new-folder")).toBeUndefined()

    const confirming = workspaceBrowserView({ ...readyState(), selectedRef: "README.md", deleteConfirmRef: "README.md" })
    expect(nodeByKey(confirming, "workspace-browser-delete")).toBeUndefined()
    expect(nodeByKey(confirming, "workspace-browser-delete-cancel")).toBeUndefined()
    expect(nodeByKey(confirming, "workspace-browser-delete-warning")).toBeUndefined()
    expect(nodeByKey(confirming, "workspace-browser-reveal")).toBeUndefined()
    expect(nodeByKey(confirming, "workspace-browser-rename")).toBeUndefined()
    // The read-only selection fact remains.
    expect(nodeByKey(confirming, "workspace-browser-selection-path")?.content).toBe("README.md")
  })

  test("large hierarchies disclose their render bound", () => {
    const state = readyState(Array.from({ length: 501 }, (_, index) => entry(`file-${index}.ts`)))
    const view = workspaceBrowserView(state)
    expect(nodeByKey(view, "workspace-browser-tree-truncated")?.content).toContain("first 500")
  })

  test("intent names are unique", () => {
    const names = workspaceBrowserIntents.map(intent => intent.name)
    expect(new Set(names).size).toBe(names.length)
  })
})

const makeFakeBridge = (
  overrides: Partial<WorkspaceBrowserBridge> = {},
): { bridge: WorkspaceBrowserBridge; calls: Array<{ op: string; value?: unknown }> } => {
  const calls: Array<{ op: string; value?: unknown }> = []
  const wrap = (op: string, implementation: (value?: unknown) => Promise<unknown>) =>
    async (value?: unknown): Promise<unknown> => {
      calls.push({ op, value })
      return implementation(value)
    }
  const bridge: WorkspaceBrowserBridge = {
    workspaceTree: wrap("tree", async (value) => {
      const directoryRef = (value as { directoryRef?: string })?.directoryRef ?? ""
      return treePage(directoryRef, directoryRef === "" ? [entry("src", "directory"), entry("README.md")] : [])
    }),
    workspaceSearch: wrap("search", async (value) => ({
      requestRef: (value as { requestRef: string }).requestRef,
      page: searchPage(),
    })),
    cancelWorkspaceSearch: wrap("cancel", async (value) => ({
      requestRef: (value as { requestRef: string }).requestRef,
      cancelled: true,
    })),
    createWorkspaceEntry: wrap("create", async () => ({ state: "created", entry: entry("notes.md") })),
    renameWorkspaceEntry: wrap("rename", async () => ({ state: "renamed", entry: entry("GUIDE.md") })),
    deleteWorkspaceEntry: wrap("delete", async (value) => ({ state: "deleted", pathRef: (value as { pathRef: string }).pathRef })),
    revealWorkspaceEntry: wrap("reveal", async (value) => ({ state: "revealed", pathRef: (value as { pathRef: string }).pathRef })),
    refreshWorkspace: wrap("refresh", async () => true) as () => Promise<unknown>,
    ...overrides,
  }
  return { bridge, calls }
}

const handlerHarness = (
  bridge: WorkspaceBrowserBridge,
  initial: WorkspaceBrowserState = emptyWorkspaceBrowserState(),
) => Effect.gen(function* () {
  const state = yield* SubscriptionRef.make({ workspaceBrowser: initial })
  const handlers = makeWorkspaceBrowserHandlers(state, bridge)
  const registry = yield* makeIntentRegistry(workspaceBrowserIntents, handlers)
  return { state, handlers, registry }
})

describe("workspace browser typed intent loop", () => {
  test("Refresh loads a decoded root through the fixed bridge", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge, calls } = makeFakeBridge()
      const { state, registry } = yield* handlerHarness(bridge)
      yield* registry.dispatch(pressIntent(workspaceBrowserView(emptyWorkspaceBrowserState()), "workspace-browser-refresh"))
      const browser = (yield* SubscriptionRef.get(state)).workspaceBrowser
      expect(browser.phase).toBe("ready")
      expect(browser.pages[""]?.entries.map(item => item.pathRef)).toEqual(["src", "README.md"])
      expect(calls.map(call => call.op)).toEqual(["refresh", "tree"])
    }))
  })

  test("Open loads the current root without advancing the host refresh epoch", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge, calls } = makeFakeBridge()
      const { state, handlers } = yield* handlerHarness(bridge)
      yield* handlers.WorkspaceBrowserOpened()
      expect((yield* SubscriptionRef.get(state)).workspaceBrowser.phase).toBe("ready")
      expect(calls.map(call => call.op)).toEqual(["tree"])
    }))
  })

  test("directory toggles lazy-load once and pagination uses the declared offset", async () => {
    const { bridge, calls } = makeFakeBridge({
      workspaceTree: async (value) => {
        calls.push({ op: "tree-script", value })
        const request = value as { directoryRef: string; offset: number }
        return request.offset === 0
          ? treePage("src", [entry("src/a.ts")], { nextOffset: 1 })
          : treePage("src", [entry("src/b.ts")], { nextOffset: null })
      },
    })
    await Effect.runPromise(Effect.gen(function* () {
      const { state, registry } = yield* handlerHarness(bridge, readyState())
      yield* registry.dispatch(pressIntent(workspaceBrowserView(readyState()), "workspace-browser-toggle-src"))
      let browser = (yield* SubscriptionRef.get(state)).workspaceBrowser
      expect(browser.pages.src?.entries.map(item => item.pathRef)).toEqual(["src/a.ts"])
      yield* registry.dispatch(pressIntent(workspaceBrowserView(browser), "workspace-browser-more-src"))
      browser = (yield* SubscriptionRef.get(state)).workspaceBrowser
      expect(browser.pages.src?.entries.map(item => item.pathRef)).toEqual(["src/a.ts", "src/b.ts"])
      const requests = calls.filter(call => call.op === "tree-script").map(call => call.value as { offset: number })
      expect(requests.map(request => request.offset)).toEqual([0, 1])
    }))
  })

  test("search pagination appends and deduplicates only matching owned responses", async () => {
    const { bridge, calls } = makeFakeBridge({
      workspaceSearch: async (value) => {
        calls.push({ op: "search-script", value })
        const request = value as { requestRef: string; offset: number }
        return {
          requestRef: request.requestRef,
          page: searchPage(request.offset === 0
            ? { matches: [{ pathRef: "src/a.ts", kind: "content", line: 1, preview: "needle" }], nextOffset: 1 }
            : { matches: [{ pathRef: "src/b.ts", kind: "content", line: 2, preview: "needle again" }], nextOffset: null }),
        }
      },
    })
    await Effect.runPromise(Effect.gen(function* () {
      const initial = { ...readyState(), query: "needle", searchMode: "content" as const }
      const { state, handlers } = yield* handlerHarness(bridge, initial)
      yield* handlers.WorkspaceBrowserSearchRequested()
      yield* handlers.WorkspaceBrowserSearchMoreRequested()
      const browser = (yield* SubscriptionRef.get(state)).workspaceBrowser
      expect(browser.searchPage?.state).toBe("available")
      if (browser.searchPage?.state === "available") {
        expect(browser.searchPage.matches.map(match => match.pathRef)).toEqual(["src/a.ts", "src/b.ts"])
      }
      const requests = calls.filter(call => call.op === "search-script").map(call => call.value as { requestRef: string; offset: number })
      expect(requests.map(request => request.offset)).toEqual([0, 1])
      expect(new Set(requests.map(request => request.requestRef)).size).toBe(2)
    }))
  })

  test("Cancel owns the exact active request and fences its late response", async () => {
    let resolveSearch!: (value: unknown) => void
    const pending = new Promise<unknown>(resolve => { resolveSearch = resolve })
    const cancelled: string[] = []
    const { bridge } = makeFakeBridge({
      workspaceSearch: async () => pending,
      cancelWorkspaceSearch: async (value) => {
        cancelled.push((value as { requestRef: string }).requestRef)
        return { requestRef: cancelled.at(-1), cancelled: true }
      },
    })
    const { state, handlers } = await Effect.runPromise(handlerHarness(bridge, { ...readyState(), query: "needle" }))
    const running = Effect.runPromise(handlers.WorkspaceBrowserSearchRequested())
    await new Promise(resolve => setTimeout(resolve, 0))
    expect((await Effect.runPromise(SubscriptionRef.get(state))).workspaceBrowser.searchState).toBe("searching")
    await Effect.runPromise(handlers.WorkspaceBrowserSearchCancelled())
    expect(cancelled).toEqual(["workspace.search.request.renderer-1"])
    resolveSearch({ requestRef: cancelled[0], page: searchPage() })
    await running
    const browser = (await Effect.runPromise(SubscriptionRef.get(state))).workspaceBrowser
    expect(browser.searchState).toBe("idle")
    expect(browser.searchPage).toBeNull()
  })

  test("create, rename, delete, and reveal dispatch bounded requests and honest receipts", async () => {
    const updatedRoot = [entry("src", "directory"), entry("GUIDE.md"), entry("notes.md")]
    const { bridge, calls } = makeFakeBridge({
      workspaceTree: async () => treePage("", updatedRoot),
    })
    await Effect.runPromise(Effect.gen(function* () {
      const { state, handlers } = yield* handlerHarness(bridge, readyState())
      yield* handlers.WorkspaceBrowserCreateStarted({ parentRef: "missing", kind: "file" })
      expect((yield* SubscriptionRef.get(state)).workspaceBrowser.editor).toBeNull()
      yield* handlers.WorkspaceBrowserCreateStarted({ parentRef: "", kind: "file" })
      yield* handlers.WorkspaceBrowserEditorChanged(" notes.md ")
      yield* handlers.WorkspaceBrowserEditorSubmitted()
      let browser = (yield* SubscriptionRef.get(state)).workspaceBrowser
      expect(browser.operation?.state).toBe("created")
      expect((calls.find(call => call.op === "create")?.value as { name: string }).name).toBe("notes.md")

      yield* handlers.WorkspaceBrowserRenameStarted({ pathRef: "GUIDE.md", name: "GUIDE.md", expectedRevisionRef: "wrong" })
      expect((yield* SubscriptionRef.get(state)).workspaceBrowser.editor).toBeNull()
      yield* handlers.WorkspaceBrowserRenameStarted({ pathRef: "GUIDE.md", name: "GUIDE.md", expectedRevisionRef: "revision-GUIDE.md" })
      yield* handlers.WorkspaceBrowserEditorChanged("MANUAL.md")
      yield* handlers.WorkspaceBrowserEditorSubmitted()
      expect((calls.find(call => call.op === "rename")?.value as { name: string }).name).toBe("MANUAL.md")

      yield* handlers.WorkspaceBrowserDeleteRequested({ pathRef: "GUIDE.md", expectedRevisionRef: "revision-GUIDE.md" })
      browser = (yield* SubscriptionRef.get(state)).workspaceBrowser
      expect(browser.deleteConfirmRef).toBe("GUIDE.md")
      yield* handlers.WorkspaceBrowserDeleteConfirmed({ pathRef: "GUIDE.md", expectedRevisionRef: "revision-GUIDE.md" })
      expect((yield* SubscriptionRef.get(state)).workspaceBrowser.operation?.state).toBe("deleted")

      yield* handlers.WorkspaceBrowserRevealRequested("notes.md")
      expect((yield* SubscriptionRef.get(state)).workspaceBrowser.operation).toEqual({ state: "revealed", pathRef: "notes.md" })
      expect(calls.map(call => call.op)).toContainAllValues(["create", "rename", "delete", "reveal"])
    }))
  })

  test("a watch change cancels search ownership before refreshing the tree", async () => {
    const { bridge, calls } = makeFakeBridge()
    await Effect.runPromise(Effect.gen(function* () {
      const { state, handlers } = yield* handlerHarness(bridge, readyState())
      yield* handlers.WorkspaceBrowserChangeReceived({ kind: "changed", pathRef: "README.md", epoch: 2 })
      expect((yield* SubscriptionRef.get(state)).workspaceBrowser.phase).toBe("ready")
      expect(calls.map(call => call.op)).toEqual(["tree"])
    }))
  })
})
