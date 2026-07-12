/**
 * CUT-17 relative-ref workspace browser. This module is deliberately pure and
 * shell-independent: typed state, intents, transitions, and Effect Native view
 * projection only. Host composition supplies the already-landed fixed bridge.
 */
import {
  Badge,
  Button,
  ComponentValueBinding,
  Divider,
  Icon,
  IconButton,
  IntentRef,
  List,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type KeyedView,
  type View,
} from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"

import {
  DesktopWorkspacePathRefSchema,
  type DesktopWorkspaceOperationResult,
  type DesktopWorkspaceSearchPage,
  type DesktopWorkspaceTreeEntry,
  type DesktopWorkspaceTreePage,
} from "../workspace-contract.ts"

type AvailableTreePage = Extract<DesktopWorkspaceTreePage, { state: "available" }>

export type WorkspaceBrowserEditor =
  | Readonly<{ kind: "create_file" | "create_directory"; parentRef: string; value: string }>
  | Readonly<{ kind: "rename"; pathRef: string; expectedRevisionRef: string; value: string }>

export type WorkspaceBrowserState = Readonly<{
  phase: "idle" | "loading" | "ready" | "unavailable"
  grantRef: string | null
  reason: string | null
  pages: Readonly<Record<string, AvailableTreePage>>
  expandedRefs: ReadonlyArray<string>
  loadingRefs: ReadonlyArray<string>
  selectedRef: string | null
  query: string
  searchMode: "path" | "content"
  searchState: "idle" | "searching" | "ready" | "unavailable"
  searchPage: DesktopWorkspaceSearchPage | null
  editor: WorkspaceBrowserEditor | null
  deleteConfirmRef: string | null
  operation: DesktopWorkspaceOperationResult | null
}>

export const emptyWorkspaceBrowserState = (): WorkspaceBrowserState => ({
  phase: "idle",
  grantRef: null,
  reason: null,
  pages: {},
  expandedRefs: [],
  loadingRefs: [],
  selectedRef: null,
  query: "",
  searchMode: "path",
  searchState: "idle",
  searchPage: null,
  editor: null,
  deleteConfirmRef: null,
  operation: null,
})

const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> => [...new Set(values)]

export const withWorkspaceBrowserLoading = (
  state: WorkspaceBrowserState,
): WorkspaceBrowserState => ({ ...state, phase: "loading", reason: null })

export const withWorkspaceBrowserRoot = (
  state: WorkspaceBrowserState,
  page: DesktopWorkspaceTreePage,
): WorkspaceBrowserState => page.state === "unavailable"
  ? {
      ...emptyWorkspaceBrowserState(),
      phase: "unavailable",
      reason: page.message,
      query: state.query,
      searchMode: state.searchMode,
    }
  : {
      ...state,
      phase: "ready",
      grantRef: page.grantRef,
      reason: null,
      pages: { "": page },
      expandedRefs: [],
      loadingRefs: [],
      selectedRef: null,
      searchState: "idle",
      searchPage: null,
      editor: null,
      deleteConfirmRef: null,
      operation: null,
    }

export const withWorkspaceBrowserPage = (
  state: WorkspaceBrowserState,
  page: DesktopWorkspaceTreePage,
): WorkspaceBrowserState => {
  if (page.state === "unavailable" || page.grantRef !== state.grantRef) {
    return { ...state, loadingRefs: [], operation: { state: "unavailable", message: page.state === "unavailable" ? page.message : "Workspace authority changed. Refresh files." } }
  }
  const prior = state.pages[page.directoryRef]
  const entries = prior === undefined
    ? page.entries
    : unique([...prior.entries.map(entry => entry.pathRef), ...page.entries.map(entry => entry.pathRef)])
        .flatMap(pathRef => page.entries.find(entry => entry.pathRef === pathRef) ?? prior.entries.find(entry => entry.pathRef === pathRef) ?? [])
  return {
    ...state,
    pages: { ...state.pages, [page.directoryRef]: { ...page, entries } },
    loadingRefs: state.loadingRefs.filter(pathRef => pathRef !== page.directoryRef),
  }
}

export const withWorkspaceBrowserToggled = (
  state: WorkspaceBrowserState,
  directoryRef: string,
): WorkspaceBrowserState => state.expandedRefs.includes(directoryRef)
  ? { ...state, expandedRefs: state.expandedRefs.filter(pathRef => pathRef !== directoryRef) }
  : {
      ...state,
      expandedRefs: [...state.expandedRefs, directoryRef],
      loadingRefs: state.pages[directoryRef] === undefined
        ? unique([...state.loadingRefs, directoryRef])
        : state.loadingRefs,
    }

export const withWorkspaceBrowserSearchStarted = (
  state: WorkspaceBrowserState,
): WorkspaceBrowserState => ({
  ...state,
  searchState: "searching",
  searchPage: null,
  operation: null,
})

export const withWorkspaceBrowserSearch = (
  state: WorkspaceBrowserState,
  page: DesktopWorkspaceSearchPage,
): WorkspaceBrowserState => page.state === "available" && page.grantRef === state.grantRef
  ? { ...state, searchState: "ready", searchPage: page }
  : {
      ...state,
      searchState: "unavailable",
      searchPage: page.state === "unavailable" ? page : null,
      operation: {
        state: "unavailable",
        message: page.state === "unavailable" ? page.message : "Workspace authority changed. Run the search again.",
      },
    }

export const withWorkspaceBrowserEditor = (
  state: WorkspaceBrowserState,
  editor: WorkspaceBrowserEditor | null,
): WorkspaceBrowserState => ({ ...state, editor, deleteConfirmRef: null, operation: null })

export const withWorkspaceBrowserOperation = (
  state: WorkspaceBrowserState,
  operation: DesktopWorkspaceOperationResult,
): WorkspaceBrowserState => ({
  ...state,
  operation,
  editor: operation.state === "created" || operation.state === "renamed" ? null : state.editor,
  deleteConfirmRef: operation.state === "deleted" ? null : state.deleteConfirmRef,
  selectedRef: operation.state === "renamed"
    ? operation.entry.pathRef
    : operation.state === "deleted" && state.selectedRef === operation.pathRef
      ? null
      : state.selectedRef,
})

export type WorkspaceBrowserRow = Readonly<{
  entry: DesktopWorkspaceTreeEntry
  depth: number
}>

export const visibleWorkspaceRows = (
  state: WorkspaceBrowserState,
  maximum = 500,
): Readonly<{ rows: ReadonlyArray<WorkspaceBrowserRow>; truncated: boolean }> => {
  const rows: WorkspaceBrowserRow[] = []
  let truncated = false
  const append = (directoryRef: string, depth: number): void => {
    const page = state.pages[directoryRef]
    if (page === undefined) return
    for (const entry of page.entries) {
      if (rows.length >= maximum) { truncated = true; return }
      rows.push({ entry, depth })
      if (entry.kind === "directory" && state.expandedRefs.includes(entry.pathRef)) {
        append(entry.pathRef, depth + 1)
      }
      if (truncated) return
    }
  }
  append("", 0)
  return { rows, truncated }
}

const WorkspaceCreatePayloadSchema = Schema.Struct({
  parentRef: DesktopWorkspacePathRefSchema,
  kind: Schema.Literals(["file", "directory"]),
})
const WorkspaceRenamePayloadSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  name: Schema.String,
  expectedRevisionRef: Schema.String,
})
const WorkspaceRevisionPayloadSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  expectedRevisionRef: Schema.String,
})

export const WorkspaceBrowserRefreshRequested = defineIntent("WorkspaceBrowserRefreshRequested", Schema.Null)
export const WorkspaceBrowserTreeToggled = defineIntent("WorkspaceBrowserTreeToggled", DesktopWorkspacePathRefSchema)
export const WorkspaceBrowserTreeMoreRequested = defineIntent("WorkspaceBrowserTreeMoreRequested", DesktopWorkspacePathRefSchema)
export const WorkspaceBrowserEntrySelected = defineIntent("WorkspaceBrowserEntrySelected", DesktopWorkspacePathRefSchema)
export const WorkspaceBrowserQueryChanged = defineIntent("WorkspaceBrowserQueryChanged", Schema.String)
export const WorkspaceBrowserSearchModeSelected = defineIntent("WorkspaceBrowserSearchModeSelected", Schema.Literals(["path", "content"]))
export const WorkspaceBrowserSearchRequested = defineIntent("WorkspaceBrowserSearchRequested", Schema.Null)
export const WorkspaceBrowserSearchCancelled = defineIntent("WorkspaceBrowserSearchCancelled", Schema.Null)
export const WorkspaceBrowserSearchMoreRequested = defineIntent("WorkspaceBrowserSearchMoreRequested", Schema.Null)
export const WorkspaceBrowserCreateStarted = defineIntent("WorkspaceBrowserCreateStarted", WorkspaceCreatePayloadSchema)
export const WorkspaceBrowserRenameStarted = defineIntent("WorkspaceBrowserRenameStarted", WorkspaceRenamePayloadSchema)
export const WorkspaceBrowserEditorChanged = defineIntent("WorkspaceBrowserEditorChanged", Schema.String)
export const WorkspaceBrowserEditorSubmitted = defineIntent("WorkspaceBrowserEditorSubmitted", Schema.Null)
export const WorkspaceBrowserEditorCancelled = defineIntent("WorkspaceBrowserEditorCancelled", Schema.Null)
export const WorkspaceBrowserDeleteRequested = defineIntent("WorkspaceBrowserDeleteRequested", WorkspaceRevisionPayloadSchema)
export const WorkspaceBrowserDeleteConfirmed = defineIntent("WorkspaceBrowserDeleteConfirmed", WorkspaceRevisionPayloadSchema)
export const WorkspaceBrowserDeleteCancelled = defineIntent("WorkspaceBrowserDeleteCancelled", Schema.Null)
export const WorkspaceBrowserRevealRequested = defineIntent("WorkspaceBrowserRevealRequested", DesktopWorkspacePathRefSchema)

export const workspaceBrowserIntents = [
  WorkspaceBrowserRefreshRequested,
  WorkspaceBrowserTreeToggled,
  WorkspaceBrowserTreeMoreRequested,
  WorkspaceBrowserEntrySelected,
  WorkspaceBrowserQueryChanged,
  WorkspaceBrowserSearchModeSelected,
  WorkspaceBrowserSearchRequested,
  WorkspaceBrowserSearchCancelled,
  WorkspaceBrowserSearchMoreRequested,
  WorkspaceBrowserCreateStarted,
  WorkspaceBrowserRenameStarted,
  WorkspaceBrowserEditorChanged,
  WorkspaceBrowserEditorSubmitted,
  WorkspaceBrowserEditorCancelled,
  WorkspaceBrowserDeleteRequested,
  WorkspaceBrowserDeleteConfirmed,
  WorkspaceBrowserDeleteCancelled,
  WorkspaceBrowserRevealRequested,
] as const

const parentRefFor = (entry: DesktopWorkspaceTreeEntry | null): string => {
  if (entry === null) return ""
  if (entry.kind === "directory") return entry.pathRef
  const parts = entry.pathRef.split("/")
  parts.pop()
  return parts.join("/")
}

const selectedEntry = (state: WorkspaceBrowserState): DesktopWorkspaceTreeEntry | null => {
  if (state.selectedRef === null) return null
  for (const page of Object.values(state.pages)) {
    const entry = page.entries.find(candidate => candidate.pathRef === state.selectedRef)
    if (entry !== undefined) return entry
  }
  return null
}

const operationNotice = (operation: DesktopWorkspaceOperationResult | null): View[] => {
  if (operation === null) return []
  const success = operation.state === "created" || operation.state === "renamed" || operation.state === "deleted" || operation.state === "revealed"
  const message = operation.state === "created"
    ? `Created ${operation.entry.pathRef}`
    : operation.state === "renamed"
      ? `Renamed to ${operation.entry.pathRef}`
      : operation.state === "deleted"
        ? `Deleted ${operation.pathRef}`
        : operation.state === "revealed"
          ? `Revealed ${operation.pathRef}`
          : operation.message
  return [Text({
    key: "workspace-browser-operation",
    content: message,
    variant: "caption",
    color: success ? "success" : "warning",
  })]
}

const editorView = (editor: WorkspaceBrowserEditor | null): View[] => editor === null ? [] : [
  Stack(
    { key: "workspace-browser-editor", direction: "row", gap: "2", align: "center", style: { width: "full" } },
    [
      TextField({
        key: "workspace-browser-editor-name",
        value: editor.value,
        label: editor.kind === "rename" ? "Rename entry" : editor.kind === "create_directory" ? "New folder name" : "New file name",
        placeholder: editor.kind === "create_directory" ? "folder-name" : "filename.ts",
        onChange: IntentRef("WorkspaceBrowserEditorChanged", ComponentValueBinding()),
        onSubmit: IntentRef("WorkspaceBrowserEditorSubmitted"),
        a11y: { label: editor.kind === "rename" ? "New workspace entry name" : "Workspace entry name" },
        style: { flex: 1, minWidth: 0 },
      }),
      Button({
        key: "workspace-browser-editor-submit",
        label: editor.kind === "rename" ? "Rename" : "Create",
        variant: "primary",
        disabled: editor.value.trim() === "",
        onPress: IntentRef("WorkspaceBrowserEditorSubmitted"),
        a11y: { label: editor.kind === "rename" ? "Rename workspace entry" : "Create workspace entry" },
      }),
      Button({
        key: "workspace-browser-editor-cancel",
        label: "Cancel",
        variant: "ghost",
        onPress: IntentRef("WorkspaceBrowserEditorCancelled"),
        a11y: { label: "Cancel workspace entry change" },
      }),
    ],
  ),
]

const treeRow = (state: WorkspaceBrowserState, row: WorkspaceBrowserRow): View => {
  const { entry, depth } = row
  const expanded = state.expandedRefs.includes(entry.pathRef)
  const loading = state.loadingRefs.includes(entry.pathRef)
  return Stack(
    {
      key: `workspace-browser-row-${entry.pathRef}`,
      direction: "row",
      gap: "1",
      align: "center",
      style: { width: "full", minWidth: 0, paddingLeft: depth === 0 ? "0" : depth === 1 ? "3" : "5" },
    },
    [
      ...(entry.kind === "directory" ? [IconButton({
        key: `workspace-browser-toggle-${entry.pathRef}`,
        icon: expanded ? "ChevronDown" : "ChevronRight",
        accessibilityLabel: `${expanded ? "Collapse" : "Expand"} folder ${entry.name}`,
        onPress: IntentRef("WorkspaceBrowserTreeToggled", StaticPayload(entry.pathRef)),
        disabled: loading,
        a11y: { expanded },
      })] : [Spacer({ key: `workspace-browser-spacer-${entry.pathRef}`, size: "5" })]),
      Icon({
        key: `workspace-browser-icon-${entry.pathRef}`,
        name: entry.kind === "directory" ? "Folder" : "Code",
        size: "sm",
        color: state.selectedRef === entry.pathRef ? "accent" : "textMuted",
        label: entry.kind,
      }),
      Button({
        key: `workspace-browser-select-${entry.pathRef}`,
        label: entry.name,
        variant: state.selectedRef === entry.pathRef ? "secondary" : "ghost",
        onPress: IntentRef("WorkspaceBrowserEntrySelected", StaticPayload(entry.pathRef)),
        style: { flex: 1, minWidth: 0 },
        a11y: {
          label: `${entry.kind === "directory" ? "Folder" : "File"} ${entry.pathRef}`,
          selected: state.selectedRef === entry.pathRef,
        },
      }),
      ...(loading ? [Text({ key: `workspace-browser-loading-${entry.pathRef}`, content: "Loading…", variant: "caption", color: "textMuted" })] : []),
    ],
  )
}

const treeView = (state: WorkspaceBrowserState): View => {
  const visible = visibleWorkspaceRows(state)
  const rootPage = state.pages[""]
  const items: KeyedView[] = visible.rows.map(row => treeRow(state, row) as KeyedView)
  for (const directoryRef of ["", ...state.expandedRefs]) {
    const page = state.pages[directoryRef]
    if (page?.nextOffset !== null && page?.nextOffset !== undefined) {
      items.push(Button({
        key: `workspace-browser-more-${directoryRef || "root"}`,
        label: "Load more",
        variant: "ghost",
        onPress: IntentRef("WorkspaceBrowserTreeMoreRequested", StaticPayload(directoryRef)),
        a11y: { label: `Load more entries in ${directoryRef || "workspace root"}` },
      }) as KeyedView)
    }
  }
  if (rootPage !== undefined && rootPage.entries.length === 0) {
    items.push(Text({
      key: "workspace-browser-tree-empty",
      content: "This folder has no visible files. Hidden, ignored, secret-shaped, and unsafe entries stay withheld.",
      variant: "body",
      color: "textMuted",
    }) as KeyedView)
  }
  if (visible.truncated) {
    items.push(Text({
      key: "workspace-browser-tree-truncated",
      content: "Showing the first 500 visible entries. Narrow the workspace or use search to continue.",
      variant: "caption",
      color: "warning",
    }) as KeyedView)
  }
  return Stack(
    { key: "workspace-browser-tree", direction: "column", gap: "1", style: { minWidth: 240, maxWidth: 360, flex: 1, minHeight: 0 } },
    [
      Text({ key: "workspace-browser-tree-title", content: "Workspace", variant: "caption", color: "textMuted" }),
      List({ key: "workspace-browser-tree-list", virtualize: true, estimatedItemSize: 36, style: { flex: 1, minHeight: 0 } }, items),
    ],
  )
}

const searchResultsView = (state: WorkspaceBrowserState): View => {
  if (state.searchState === "searching") {
    return Text({ key: "workspace-browser-searching", content: "Searching the selected workspace…", variant: "body", color: "textMuted" })
  }
  if (state.searchPage?.state !== "available") {
    return Text({ key: "workspace-browser-search-empty", content: "Search paths or bounded text content. Results never expose the selected root.", variant: "body", color: "textMuted" })
  }
  const page = state.searchPage
  if (page.matches.length === 0) {
    return Text({ key: "workspace-browser-search-none", content: `No ${page.mode} matches for “${page.query}”.`, variant: "body", color: "textMuted" })
  }
  return Stack(
    { key: "workspace-browser-search-results", direction: "column", gap: "2", style: { flex: 1, minHeight: 0 } },
    [
      Text({ key: "workspace-browser-search-count", content: `${page.matches.length} result${page.matches.length === 1 ? "" : "s"}`, variant: "caption", color: "textMuted" }),
      List(
        { key: "workspace-browser-search-list", virtualize: true, estimatedItemSize: 48, style: { flex: 1, minHeight: 0 } },
        page.matches.map((match, index) => Stack(
          { key: `workspace-browser-search-${match.pathRef}-${index}`, direction: "column", gap: "1", style: { width: "full" } },
          [
            Button({
              key: `workspace-browser-search-select-${match.pathRef}-${index}`,
              label: match.line === null ? match.pathRef : `${match.pathRef}:${match.line}`,
              variant: "ghost",
              onPress: IntentRef("WorkspaceBrowserEntrySelected", StaticPayload(match.pathRef)),
              a11y: { label: `Open ${match.pathRef}${match.line === null ? "" : ` line ${match.line}`}` },
            }),
            ...(match.preview === null ? [] : [Text({ key: `workspace-browser-search-preview-${match.pathRef}-${index}`, content: match.preview, variant: "caption", color: "textMuted" })]),
          ],
        ) as KeyedView),
      ),
      ...(page.truncated ? [Text({ key: "workspace-browser-search-truncated", content: "Results are bounded. Refine the query to search a narrower set.", variant: "caption", color: "warning" })] : []),
      ...(page.nextOffset === null ? [] : [Button({ key: "workspace-browser-search-more", label: "More results", variant: "ghost", onPress: IntentRef("WorkspaceBrowserSearchMoreRequested"), a11y: { label: "Load more workspace search results" } })]),
    ],
  )
}

const selectionActions = (state: WorkspaceBrowserState): View[] => {
  const entry = selectedEntry(state)
  if (entry === null) return []
  const deleting = state.deleteConfirmRef === entry.pathRef
  return [
    Divider({ key: "workspace-browser-selection-divider" }),
    Text({ key: "workspace-browser-selection-path", content: entry.pathRef, variant: "caption", color: "textPrimary" }),
    Stack({ key: "workspace-browser-selection-actions", direction: "row", gap: "2", align: "center" }, [
      Button({ key: "workspace-browser-reveal", label: "Reveal", variant: "ghost", onPress: IntentRef("WorkspaceBrowserRevealRequested", StaticPayload(entry.pathRef)), a11y: { label: `Reveal ${entry.pathRef} in the system file browser` } }),
      Button({ key: "workspace-browser-rename", label: "Rename", variant: "ghost", onPress: IntentRef("WorkspaceBrowserRenameStarted", StaticPayload({ pathRef: entry.pathRef, name: entry.name, expectedRevisionRef: entry.revisionRef })), a11y: { label: `Rename ${entry.pathRef}` } }),
      Button({ key: "workspace-browser-delete", label: deleting ? "Confirm delete" : "Delete", variant: deleting ? "primary" : "ghost", onPress: IntentRef(deleting ? "WorkspaceBrowserDeleteConfirmed" : "WorkspaceBrowserDeleteRequested", StaticPayload({ pathRef: entry.pathRef, expectedRevisionRef: entry.revisionRef })), a11y: { label: deleting ? `Confirm deletion of ${entry.pathRef}` : `Delete ${entry.pathRef}` } }),
      ...(deleting ? [Button({ key: "workspace-browser-delete-cancel", label: "Keep", variant: "secondary", onPress: IntentRef("WorkspaceBrowserDeleteCancelled"), a11y: { label: `Cancel deletion of ${entry.pathRef}` } })] : []),
    ]),
    ...(deleting ? [Text({ key: "workspace-browser-delete-warning", content: entry.kind === "directory" ? "Only an empty folder can be deleted. This cannot be undone." : "This cannot be undone.", variant: "caption", color: "warning" })] : []),
  ]
}

export const workspaceBrowserView = (state: WorkspaceBrowserState): View => {
  const entry = selectedEntry(state)
  const parentRef = parentRefFor(entry)
  return Stack(
    { key: "workspace-browser", direction: "column", gap: "3", style: { width: "full", minWidth: 0, flex: 1, minHeight: 0 } },
    [
      Stack({ key: "workspace-browser-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
        Text({ key: "workspace-browser-title", content: "Files", variant: "heading", color: "textPrimary" }),
        Badge({ key: "workspace-browser-boundary", label: "Relative workspace access", tone: "neutral", a11y: { label: "Workspace access is grant scoped and relative" } }),
        Spacer({ key: "workspace-browser-heading-space", size: "1" }),
        Button({ key: "workspace-browser-refresh", label: "Refresh", variant: "ghost", disabled: state.phase === "loading", onPress: IntentRef("WorkspaceBrowserRefreshRequested"), a11y: { label: "Refresh workspace files" } }),
        Button({ key: "workspace-browser-choose", label: state.phase === "idle" ? "Choose folder" : "Change folder", variant: "secondary", onPress: IntentRef("DesktopWorkspacePickerRequested"), a11y: { label: state.phase === "idle" ? "Choose a local workspace folder" : "Change the local workspace folder" } }),
      ]),
      ...(state.phase === "idle" ? [Text({ key: "workspace-browser-idle", content: "Choose a local folder to browse a safe, grant-scoped tree. Hidden, ignored, secret-shaped, binary, and escaping entries remain unavailable.", variant: "body", color: "textMuted" })]
        : state.phase === "loading" ? [Text({ key: "workspace-browser-loading", content: "Loading the selected workspace…", variant: "body", color: "textMuted" })]
        : state.phase === "unavailable" ? [Stack({ key: "workspace-browser-unavailable", direction: "column", gap: "2" }, [
            Text({ key: "workspace-browser-unavailable-title", content: "Workspace unavailable", variant: "title", color: "warning" }),
            Text({ key: "workspace-browser-unavailable-reason", content: state.reason ?? "The selected workspace could not be read.", variant: "body", color: "textMuted" }),
            Button({ key: "workspace-browser-unavailable-retry", label: "Try again", variant: "secondary", onPress: IntentRef("WorkspaceBrowserRefreshRequested"), a11y: { label: "Try loading workspace files again" } }),
          ])]
        : [
            Stack({ key: "workspace-browser-search-controls", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
              TextField({ key: "workspace-browser-query", value: state.query, placeholder: state.searchMode === "path" ? "Search file and folder names" : "Search bounded text content", onChange: IntentRef("WorkspaceBrowserQueryChanged", ComponentValueBinding()), onSubmit: IntentRef("WorkspaceBrowserSearchRequested"), disabled: state.searchState === "searching", a11y: { label: state.searchMode === "path" ? "Search workspace paths" : "Search workspace file content" }, style: { flex: 1, minWidth: 0 } }),
              Button({ key: "workspace-browser-mode-path", label: "Path", variant: state.searchMode === "path" ? "secondary" : "ghost", onPress: IntentRef("WorkspaceBrowserSearchModeSelected", StaticPayload("path")), a11y: { label: "Search workspace paths", selected: state.searchMode === "path" } }),
              Button({ key: "workspace-browser-mode-content", label: "Content", variant: state.searchMode === "content" ? "secondary" : "ghost", onPress: IntentRef("WorkspaceBrowserSearchModeSelected", StaticPayload("content")), a11y: { label: "Search workspace file content", selected: state.searchMode === "content" } }),
              Button({ key: "workspace-browser-search-submit", label: state.searchState === "searching" ? "Cancel" : "Search", variant: "primary", disabled: state.searchState !== "searching" && state.query.trim() === "", onPress: IntentRef(state.searchState === "searching" ? "WorkspaceBrowserSearchCancelled" : "WorkspaceBrowserSearchRequested"), a11y: { label: state.searchState === "searching" ? "Cancel workspace search" : "Search workspace" } }),
            ]),
            Stack({ key: "workspace-browser-create-actions", direction: "row", gap: "2", align: "center" }, [
              Button({ key: "workspace-browser-new-file", label: "New file", variant: "ghost", onPress: IntentRef("WorkspaceBrowserCreateStarted", StaticPayload({ parentRef, kind: "file" })), a11y: { label: `Create a new file in ${parentRef || "workspace root"}` } }),
              Button({ key: "workspace-browser-new-folder", label: "New folder", variant: "ghost", onPress: IntentRef("WorkspaceBrowserCreateStarted", StaticPayload({ parentRef, kind: "directory" })), a11y: { label: `Create a new folder in ${parentRef || "workspace root"}` } }),
            ]),
            ...editorView(state.editor),
            ...operationNotice(state.operation),
            Stack({ key: "workspace-browser-layout", direction: "row", gap: "3", style: { width: "full", flex: 1, minHeight: 0 } }, [
              treeView(state),
              Divider({ key: "workspace-browser-layout-divider", orientation: "vertical" }),
              Stack({ key: "workspace-browser-detail", direction: "column", gap: "2", style: { flex: 2, minWidth: 0, minHeight: 0 } }, [
                searchResultsView(state),
                ...selectionActions(state),
              ]),
            ]),
          ]),
    ],
  )
}
