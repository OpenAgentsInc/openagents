/**
 * CUT-17 workspace browser tests: grant fencing and bounded hierarchy state,
 * plus the pure Effect Native projection for explicit states and mutations.
 */
import { describe, expect, test } from "bun:test"
import type { View } from "@effect-native/core"

import type {
  DesktopWorkspaceSearchPage,
  DesktopWorkspaceTreeEntry,
  DesktopWorkspaceTreePage,
} from "../workspace-contract.ts"
import {
  emptyWorkspaceBrowserState,
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
  type WorkspaceBrowserState,
} from "./workspace-browser.ts"

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
    expect((nodeByKey(view, "workspace-browser-toggle-src")?.a11y as { expanded?: boolean }).expanded).toBe(false)
    expect((nodeByKey(view, "workspace-browser-toggle-src") as { accessibilityLabel?: string }).accessibilityLabel).toBe("Expand folder src")
    expect((nodeByKey(view, "workspace-browser-select-README.md")?.a11y as { label?: string }).label).toBe("File README.md")
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

  test("create and rename use inline standard controls", () => {
    const create = workspaceBrowserView(withWorkspaceBrowserEditor(readyState(), {
      kind: "create_file",
      parentRef: "",
      value: "notes.md",
    }))
    expect(nodeByKey(create, "workspace-browser-editor-name")?._tag).toBe("TextField")
    expect(nodeByKey(create, "workspace-browser-editor-submit")?.label).toBe("Create")
    expect(nodeByKey(create, "workspace-browser-editor-cancel")?.label).toBe("Cancel")

    const rename = workspaceBrowserView(withWorkspaceBrowserEditor(readyState(), {
      kind: "rename",
      pathRef: "README.md",
      expectedRevisionRef: "revision-README.md",
      value: "GUIDE.md",
    }))
    expect(nodeByKey(rename, "workspace-browser-editor-submit")?.label).toBe("Rename")
  })

  test("destructive action is inline, reversible before confirmation, and reveal is semantic", () => {
    const selected = { ...readyState(), selectedRef: "README.md" }
    const first = workspaceBrowserView(selected)
    expect(nodeByKey(first, "workspace-browser-delete")?.label).toBe("Delete")
    expect(nodeByKey(first, "workspace-browser-reveal")?.label).toBe("Reveal")
    expect(nodeByKey(first, "workspace-browser-delete-cancel")).toBeUndefined()

    const confirming = workspaceBrowserView({ ...selected, deleteConfirmRef: "README.md" })
    expect(nodeByKey(confirming, "workspace-browser-delete")?.label).toBe("Confirm delete")
    expect(nodeByKey(confirming, "workspace-browser-delete-cancel")?.label).toBe("Keep")
    expect(nodeByKey(confirming, "workspace-browser-delete-warning")?.content).toBe("This cannot be undone.")
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
