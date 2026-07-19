import { FileTree, useFileTree } from "@pierre/trees/react"
import { useEffect, useRef, type ReactElement } from "react"

import type { WorkspaceBrowserState } from "../workspace-browser.ts"

export type PierreWorkspaceTreeActivation = Readonly<{
  kind: "file" | "directory"
  pathRef: string
}>

const pierrePath = (pathRef: string, kind: "file" | "directory"): string =>
  kind === "directory" ? `${pathRef}/` : pathRef

const workspacePathRef = (path: string): PierreWorkspaceTreeActivation => path.endsWith("/")
  ? { kind: "directory", pathRef: path.slice(0, -1) }
  : { kind: "file", pathRef: path }

/**
 * Projects only already-admitted relative refs into Pierre. The adapter has no
 * bridge, root, grant, filesystem, mutation, DnD, rename, or unsafe CSS access.
 */
export const pierreWorkspacePaths = (
  browser: WorkspaceBrowserState,
): ReadonlyArray<string> => [...new Set(Object.values(browser.pages).flatMap(page =>
  page.entries.map(entry => pierrePath(entry.pathRef, entry.kind))))]

export const PierreWorkspaceTree = ({
  browser,
  onActivate,
}: Readonly<{
  browser: WorkspaceBrowserState
  onActivate: (activation: PierreWorkspaceTreeActivation) => void
}>): ReactElement => {
  const paths = pierreWorkspacePaths(browser)
  const signature = paths.join("\0")
  const activationRef = useRef(onActivate)
  activationRef.current = onActivate
  const reconcilingRef = useRef(false)
  const { model } = useFileTree({
    density: "compact",
    dragAndDrop: false,
    flattenEmptyDirectories: false,
    initialExpandedPaths: browser.expandedRefs.map(pathRef => pierrePath(pathRef, "directory")),
    initialExpansion: "closed",
    initialSelectedPaths: browser.selectedRef === null ? [] : [
      pierrePath(
        browser.selectedRef,
        paths.includes(pierrePath(browser.selectedRef, "directory")) ? "directory" : "file",
      ),
    ],
    initialVisibleRowCount: 16,
    onSelectionChange: selectedPaths => {
      if (reconcilingRef.current) return
      const selected = selectedPaths.at(-1)
      if (selected !== undefined) activationRef.current(workspacePathRef(selected))
    },
    overscan: 8,
    paths,
    renaming: false,
    search: false,
    stickyFolders: true,
  })
  const reconciledSignatureRef = useRef(signature)
  useEffect(() => {
    if (reconciledSignatureRef.current === signature) return
    reconciledSignatureRef.current = signature
    reconcilingRef.current = true
    try {
      model.resetPaths(paths, {
        initialExpandedPaths: browser.expandedRefs.map(pathRef => pierrePath(pathRef, "directory")),
      })
    } finally {
      reconcilingRef.current = false
    }
  }, [browser.expandedRefs, model, paths, signature])

  return <FileTree
    aria-label="Workspace files"
    className="oa-react-pierre-tree"
    data-oa-pierre-tree="true"
    model={model}
  />
}
