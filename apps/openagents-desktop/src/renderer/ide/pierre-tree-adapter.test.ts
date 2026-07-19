import { describe, expect, test } from "vite-plus/test"

import { emptyWorkspaceBrowserState, type WorkspaceBrowserState } from "../workspace-browser.ts"
import { pierreWorkspacePaths } from "./pierre-tree-adapter.tsx"

const readyBrowser = (): WorkspaceBrowserState => ({
  ...emptyWorkspaceBrowserState(),
  phase: "ready",
  grantRef: "workspace.grant.pierre",
  pages: {
    "": {
      state: "available",
      grantRef: "workspace.grant.pierre",
      directoryRef: "",
      entries: [
        { name: "src", pathRef: "src", kind: "directory", expandable: true, sizeBytes: null, revisionRef: "revision-src" },
        { name: "README.md", pathRef: "README.md", kind: "file", expandable: false, sizeBytes: 12, revisionRef: "revision-readme" },
      ],
      nextOffset: null,
      cache: { key: "workspace.tree.root", epoch: 1, freshness: "current" },
    },
    src: {
      state: "available",
      grantRef: "workspace.grant.pierre",
      directoryRef: "src",
      entries: [
        { name: "index.ts", pathRef: "src/index.ts", kind: "file", expandable: false, sizeBytes: 18, revisionRef: "revision-index" },
      ],
      nextOffset: null,
      cache: { key: "workspace.tree.src", epoch: 1, freshness: "current" },
    },
  },
})

describe("Pierre workspace tree adapter", () => {
  test("projects only unique canonical relative paths and marks directories", () => {
    expect(pierreWorkspacePaths(readyBrowser())).toEqual(["src/", "README.md", "src/index.ts"])
  })

  test("projects no root, grant, document bytes, or unloaded path", () => {
    const paths = pierreWorkspacePaths(readyBrowser())
    expect(JSON.stringify(paths)).not.toContain("/Users/")
    expect(JSON.stringify(paths)).not.toContain("workspace.grant")
    expect(paths).not.toContain("src/unloaded.ts")
  })
})
