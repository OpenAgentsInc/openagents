import { FileTree, useFileTree } from "@pierre/trees/react"
import { useEffect, useRef, type ReactElement } from "react"

import type {
  IdeExplorerCommand,
  IdePierreTreeNode,
  IdePierreTreeProjection,
} from "../../ide/path-index-contract.ts"

const pierrePath = (pathRef: string, kind: "file" | "directory"): string =>
  kind === "directory" ? `${pathRef}/` : pathRef

const workspacePathRef = (path: string): string => path.endsWith("/") ? path.slice(0, -1) : path

const parentPathRef = (pathRef: string): string => {
  const slash = pathRef.lastIndexOf("/")
  return slash < 0 ? "" : pathRef.slice(0, slash)
}

export const pierreWorkspacePaths = (
  projection: IdePierreTreeProjection | null,
): ReadonlyArray<string> => projection === null
  ? []
  : projection.nodes.map(node => pierrePath(node.pathRef, node.kind))

export const pierreWorkspaceNodeAtPath = (
  projection: IdePierreTreeProjection,
  path: string,
): IdePierreTreeNode | null => {
  const pathRef = workspacePathRef(path)
  return projection.nodes.find(node => node.pathRef === pathRef) ?? null
}

const destinationParent = (directoryPath: string | null): string =>
  directoryPath === null ? "" : workspacePathRef(directoryPath)

const openCommand = (node: IdePierreTreeNode): IdeExplorerCommand => ({
  _tag: "Open",
  nodeRef: node.nodeRef,
  pathRef: node.pathRef,
})

/**
 * Pierre receives only the bounded generation-fenced projection and emits
 * typed intent. It has no root, grant, preload/IPC bridge, watcher, mutation,
 * persistence, or success authority.
 */
export const PierreWorkspaceTree = ({
  projection,
  onIntent,
}: Readonly<{
  projection: IdePierreTreeProjection
  onIntent: (intent: IdeExplorerCommand) => void
}>): ReactElement => {
  const paths = pierreWorkspacePaths(projection)
  const signature = `${projection.indexGeneration}\0${paths.join("\0")}`
  const intentRef = useRef(onIntent)
  intentRef.current = onIntent
  const reconcilingRef = useRef(false)
  const nodesByRef = new Map(projection.nodes.map(node => [node.nodeRef, node]))
  const nodesByPath = new Map(projection.nodes.map(node => [pierrePath(node.pathRef, node.kind), node]))
  const nodesByPathRef = useRef(nodesByPath)
  nodesByPathRef.current = nodesByPath
  const expandedPaths = projection.expandedNodeRefs.flatMap(nodeRef => {
    const node = nodesByRef.get(nodeRef)
    return node?.kind === "directory" ? [pierrePath(node.pathRef, node.kind)] : []
  })
  const selectedPath = projection.selectedNodeRef === null
    ? null
    : (() => {
        const node = nodesByRef.get(projection.selectedNodeRef)
        return node === undefined ? null : pierrePath(node.pathRef, node.kind)
      })()
  const { model } = useFileTree({
    density: "compact",
    dragAndDrop: {
      canDrag: draggedPaths => draggedPaths.every(path => nodesByPathRef.current.has(path)),
      canDrop: event => {
        const target = destinationParent(event.target.directoryPath)
        return event.draggedPaths.every(path => {
          const node = nodesByPathRef.current.get(path)
          return node !== undefined && node.pathRef !== target && !target.startsWith(`${node.pathRef}/`)
        })
      },
      onDropComplete: event => {
        const target = destinationParent(event.target.directoryPath)
        for (const path of event.draggedPaths) {
          const node = nodesByPathRef.current.get(path)
          if (node === undefined) continue
          intentRef.current({
            _tag: "Move",
            nodeRef: node.nodeRef,
            pathRef: node.pathRef,
            expectedRevisionRef: node.revisionRef,
            destinationParentPathRef: target,
          })
        }
      },
      onDropError: () => undefined,
      openOnDropDelay: 600,
    },
    fileTreeSearchMode: "hide-non-matches",
    flattenEmptyDirectories: true,
    initialExpandedPaths: expandedPaths,
    initialExpansion: "closed",
    initialSearchQuery: null,
    initialSelectedPaths: selectedPath === null ? [] : [selectedPath],
    initialVisibleRowCount: 24,
    onSelectionChange: selectedPaths => {
      if (reconcilingRef.current) return
      const selected = selectedPaths.at(-1)
      if (selected === undefined) return
      const node = nodesByPathRef.current.get(selected)
      if (node !== undefined) intentRef.current(openCommand(node))
    },
    overscan: 12,
    paths,
    renderRowDecoration: ({ item }) => {
      const node = nodesByPathRef.current.get(item.path)
      if (node === undefined) return null
      const labels = [...node.badgeLabels, ...(node.pendingLabel === null ? [] : [node.pendingLabel])]
      return labels.length === 0 ? null : { text: labels.join(" · "), title: labels.join("; ") }
    },
    renaming: {
      canRename: item => nodesByPathRef.current.has(item.path),
      onError: () => undefined,
      onRename: event => {
        const node = nodesByPathRef.current.get(event.sourcePath)
        if (node === undefined) return
        intentRef.current({
          _tag: "Rename",
          nodeRef: node.nodeRef,
          pathRef: node.pathRef,
          expectedRevisionRef: node.revisionRef,
          name: workspacePathRef(event.destinationPath).split("/").at(-1) ?? node.pathRef,
        })
      },
    },
    search: true,
    searchBlurBehavior: "retain",
    stickyFolders: true,
  })
  const reconciledSignatureRef = useRef(signature)
  useEffect(() => {
    if (reconciledSignatureRef.current === signature) return
    reconciledSignatureRef.current = signature
    reconcilingRef.current = true
    try {
      model.resetPaths(paths, { initialExpandedPaths: expandedPaths })
      if (selectedPath !== null) model.getItem(selectedPath)?.select()
      const focused = projection.focusedNodeRef === null ? undefined : nodesByRef.get(projection.focusedNodeRef)
      if (focused !== undefined) model.focusPath(pierrePath(focused.pathRef, focused.kind))
      const anchor = projection.scrollAnchorNodeRef === null ? undefined : nodesByRef.get(projection.scrollAnchorNodeRef)
      if (anchor !== undefined) model.scrollToPath(pierrePath(anchor.pathRef, anchor.kind), { offset: "nearest" })
    } finally {
      reconcilingRef.current = false
    }
  }, [expandedPaths, model, nodesByRef, paths, projection.focusedNodeRef, projection.scrollAnchorNodeRef, selectedPath, signature])

  return <FileTree
    aria-busy={projection.state._tag === "Scanning" ? "true" : "false"}
    aria-describedby="oa-workspace-index-status"
    aria-label="Workspace files"
    className="oa-react-pierre-tree"
    data-oa-index-generation={projection.indexGeneration}
    data-oa-pierre-tree="true"
    model={model}
    renderContextMenu={(item, context) => {
      const node = nodesByPath.get(item.path)
      if (node === undefined) return null
      const emit = (command: IdeExplorerCommand): void => {
        context.close()
        intentRef.current(command)
      }
      return <div aria-label={`Actions for ${node.pathRef}`} className="oa-react-pierre-context-menu" role="menu">
        <button onClick={() => emit(openCommand(node))} role="menuitem" type="button">Open</button>
        <button onClick={() => emit({ _tag: "Reveal", nodeRef: node.nodeRef, pathRef: node.pathRef })} role="menuitem" type="button">Reveal in Finder</button>
        <button onClick={() => { context.close({ restoreFocus: false }); model.startRenaming(item.path) }} role="menuitem" type="button">Rename</button>
        <button onClick={() => emit({ _tag: "Duplicate", nodeRef: node.nodeRef, pathRef: node.pathRef, expectedRevisionRef: node.revisionRef })} role="menuitem" type="button">Duplicate</button>
        <button onClick={() => emit({ _tag: "OpenTerminal", nodeRef: node.nodeRef, pathRef: node.pathRef })} role="menuitem" type="button">Open in Terminal</button>
        <button onClick={() => emit({ _tag: "Compare", nodeRef: node.nodeRef, pathRef: node.pathRef })} role="menuitem" type="button">Compare</button>
        <button onClick={() => emit({ _tag: "Delete", nodeRef: node.nodeRef, pathRef: node.pathRef, expectedRevisionRef: node.revisionRef, recursive: false })} role="menuitem" type="button">Delete</button>
      </div>
    }}
  />
}

export const pierreWorkspaceMoveDestination = (
  sourcePathRef: string,
  destinationDirectoryRef: string,
): string => `${destinationDirectoryRef}${destinationDirectoryRef === "" ? "" : "/"}${sourcePathRef.split("/").at(-1) ?? sourcePathRef}`

export const pierreWorkspaceRenameParent = parentPathRef
