import { describe, expect, test } from "vite-plus/test"

import {
  IdePathIndexGenerationSchema,
} from "../../ide/project-contract.ts"
import {
  IdePathNodeRefSchema,
  IdePierreTreeProjectionSchema,
  type IdePierreTreeProjection,
} from "../../ide/path-index-contract.ts"
import {
  pierreWorkspaceMoveDestination,
  pierreWorkspaceNodeAtPath,
  pierreWorkspacePaths,
  pierreWorkspaceRenameParent,
} from "./pierre-tree-adapter.tsx"

const projection = (): IdePierreTreeProjection => IdePierreTreeProjectionSchema.make({
  schemaVersion: "openagents.desktop.pierre-tree-projection.v1",
  indexGeneration: IdePathIndexGenerationSchema.make(4),
  state: { _tag: "Ready", sourceEpoch: 9, nodeCount: 3 },
  nodes: [
    { nodeRef: IdePathNodeRefSchema.make("ide.path-node.1"), pathRef: "src", kind: "directory", revisionRef: "revision-src", badgeLabels: [], pendingLabel: null },
    { nodeRef: IdePathNodeRefSchema.make("ide.path-node.2"), pathRef: "README.md", kind: "file", revisionRef: "revision-readme", badgeLabels: ["Git modified"], pendingLabel: null },
    { nodeRef: IdePathNodeRefSchema.make("ide.path-node.3"), pathRef: "src/index.ts", kind: "file", revisionRef: "revision-index", badgeLabels: ["2 error diagnostics"], pendingLabel: "Rename pending" },
  ],
  expandedNodeRefs: [IdePathNodeRefSchema.make("ide.path-node.1")],
  selectedNodeRef: IdePathNodeRefSchema.make("ide.path-node.3"),
  focusedNodeRef: IdePathNodeRefSchema.make("ide.path-node.3"),
  scrollAnchorNodeRef: IdePathNodeRefSchema.make("ide.path-node.3"),
  stickyAncestorNodeRefs: [IdePathNodeRefSchema.make("ide.path-node.1")],
  truncated: false,
})

describe("Pierre workspace tree adapter", () => {
  test("projects the complete bounded index and marks directories", () => {
    expect(pierreWorkspacePaths(projection())).toEqual(["src/", "README.md", "src/index.ts"])
    expect(pierreWorkspaceNodeAtPath(projection(), "src/")?.nodeRef).toBe("ide.path-node.1")
  })

  test("projects no root, grant, bridge, watcher, mutation, or document bytes", () => {
    const serialized = JSON.stringify(projection())
    expect(serialized).not.toContain("/Users/")
    expect(serialized).not.toContain("workspace.grant")
    expect(serialized).not.toContain("workspaceTree")
    expect(serialized).not.toContain("content")
  })

  test("resolves typed rename and move destinations without filesystem authority", () => {
    expect(pierreWorkspaceRenameParent("src/domain/model.ts")).toBe("src/domain")
    expect(pierreWorkspaceMoveDestination("src/domain/model.ts", "packages/core")).toBe("packages/core/model.ts")
    expect(pierreWorkspaceMoveDestination("README.md", "")).toBe("README.md")
  })
})
