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
  EmptyMessage,
  Icon,
  IconButton,
  IntentRef,
  List,
  SegmentedControl,
  ShimmerText,
  Spacer,
  Spinner,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type KeyedView,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  IdeAttachmentGenerationSchema,
  IdeAttachmentRefSchema,
  IdePathIndexGenerationSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "./ide-path-index.ts"
import {
  IdeExplorerCommandSchema,
  IdePathIndexIdentitySchema,
  IdePathOperationRefSchema,
  IdePathScanRefSchema,
  type IdeExplorerCommand,
  type IdePathIndexIdentity,
  type IdePathIndexInteractionUpdate,
  type IdePathIndexOperationUpdate,
  type IdePathIndexReconcileRequest,
  type IdePathIndexScanRequest,
  type IdePathIndexSnapshot,
  type IdePierreTreeProjection,
} from "./ide-path-index.ts"
import {
  IdePathIndexService,
  emptyIdePathIndexSnapshot,
  makeIdePathIndexLayer,
  type IdePathIndexSource,
} from "./ide-path-index.ts"
import {
  DesktopWorkspaceChangeSchema,
  DesktopWorkspacePathRefSchema,
  decodeWorkspaceOperationResult,
  decodeWorkspaceSearchResponse,
  decodeWorkspaceTreePage,
  workspaceChangePathRefs,
  type DesktopWorkspaceChange,
  type DesktopWorkspaceOperationResult,
  type DesktopWorkspaceSearchResponse,
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
  pathIndexSnapshot: IdePathIndexSnapshot | null
  pathIndexProjection: IdePierreTreeProjection | null
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
  pathIndexSnapshot: null,
  pathIndexProjection: null,
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
      pathIndexSnapshot: null,
      pathIndexProjection: null,
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
  preservePage = false,
): WorkspaceBrowserState => ({
  ...state,
  searchState: "searching",
  searchPage: preservePage ? state.searchPage : null,
  operation: null,
})

export const withWorkspaceBrowserSearch = (
  state: WorkspaceBrowserState,
  page: DesktopWorkspaceSearchPage,
  append = false,
): WorkspaceBrowserState => page.state === "available" && page.grantRef === state.grantRef
  ? {
      ...state,
      searchState: "ready",
      searchPage: append && state.searchPage?.state === "available" &&
        state.searchPage.grantRef === page.grantRef &&
        state.searchPage.query === page.query &&
        state.searchPage.mode === page.mode
        ? {
            ...page,
            matches: [...new Map(
              [...state.searchPage.matches, ...page.matches]
                .map(match => [`${match.kind}:${match.pathRef}:${match.line ?? ""}`, match]),
            ).values()],
          }
        : page,
    }
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
export const WorkspaceBrowserOpened = defineIntent("WorkspaceBrowserOpened", Schema.Null)
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
export const WorkspaceBrowserChangeReceived = defineIntent("WorkspaceBrowserChangeReceived", DesktopWorkspaceChangeSchema)
export const WorkspaceBrowserExplorerCommandRequested = defineIntent("WorkspaceBrowserExplorerCommandRequested", IdeExplorerCommandSchema)

export const workspaceBrowserIntents = [
  WorkspaceBrowserRefreshRequested,
  WorkspaceBrowserOpened,
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
  WorkspaceBrowserChangeReceived,
  WorkspaceBrowserExplorerCommandRequested,
] as const

export type WorkspaceBrowserCapableState = Readonly<{
  workspaceBrowser: WorkspaceBrowserState
  workspace?: string
}>

export type WorkspaceBrowserIndexScope = Readonly<{
  projectRef: string
  rootRef: string
  worktreeRef: string
  attachmentRef: string
  attachmentGeneration: number
  pathIndexGeneration: number
}>

const opaqueSuffix = (value: string): string => {
  const normalized = value.replaceAll(/[^A-Za-z0-9._-]/gu, "-").replaceAll(/-+/gu, "-")
  return (normalized.slice(0, 120) || "unavailable").replace(/^[^A-Za-z0-9]/u, "x")
}

export const workspaceBrowserIndexIdentity = (
  scope: WorkspaceBrowserIndexScope,
): IdePathIndexIdentity => IdePathIndexIdentitySchema.make({
  projectRef: IdeProjectRefSchema.make(`ide.project.${opaqueSuffix(scope.projectRef)}`),
  rootRef: IdeRootRefSchema.make(`ide.root.${opaqueSuffix(scope.rootRef)}`),
  worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.${opaqueSuffix(scope.worktreeRef)}`),
  attachmentRef: IdeAttachmentRefSchema.make(`ide.attachment.${opaqueSuffix(scope.attachmentRef)}`),
  attachmentGeneration: IdeAttachmentGenerationSchema.make(Math.max(1, scope.attachmentGeneration)),
  pathIndexGeneration: IdePathIndexGenerationSchema.make(Math.max(1, scope.pathIndexGeneration)),
})

export type WorkspaceBrowserIndexIdentityResolver<S extends WorkspaceBrowserCapableState> = (
  state: S,
  grantRef: string,
  pathIndexGeneration: number,
) => IdePathIndexIdentity

const defaultWorkspaceBrowserIndexIdentity = <S extends WorkspaceBrowserCapableState>(
  _state: S,
  grantRef: string,
  pathIndexGeneration: number,
): IdePathIndexIdentity => workspaceBrowserIndexIdentity({
  projectRef: grantRef,
  rootRef: grantRef,
  worktreeRef: grantRef,
  attachmentRef: grantRef,
  attachmentGeneration: pathIndexGeneration,
  pathIndexGeneration,
})

export type WorkspaceBrowserBridge = Readonly<{
  workspaceTree: (value: unknown) => Promise<unknown>
  workspaceSearch: (value: unknown) => Promise<unknown>
  cancelWorkspaceSearch: (value: unknown) => Promise<unknown>
  createWorkspaceEntry: (value: unknown) => Promise<unknown>
  renameWorkspaceEntry: (value: unknown) => Promise<unknown>
  moveWorkspaceEntry: (value: unknown) => Promise<unknown>
  copyWorkspaceEntry: (value: unknown) => Promise<unknown>
  duplicateWorkspaceEntry: (value: unknown) => Promise<unknown>
  deleteWorkspaceEntry: (value: unknown) => Promise<unknown>
  revealWorkspaceEntry: (value: unknown) => Promise<unknown>
  refreshWorkspace: () => Promise<unknown>
}>

export const unavailableWorkspaceBrowserBridge: WorkspaceBrowserBridge = {
  workspaceTree: async () => ({ state: "unavailable", message: "Workspace files are unavailable." }),
  workspaceSearch: async (value) => ({
    requestRef: typeof value === "object" && value !== null && typeof (value as { requestRef?: unknown }).requestRef === "string"
      ? (value as { requestRef: string }).requestRef
      : "workspace.search.request.unavailable",
    page: { state: "unavailable", message: "Workspace search is unavailable." },
  }),
  cancelWorkspaceSearch: async (value) => ({
    requestRef: typeof value === "object" && value !== null && typeof (value as { requestRef?: unknown }).requestRef === "string"
      ? (value as { requestRef: string }).requestRef
      : "workspace.search.request.unavailable",
    cancelled: false,
  }),
  createWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace changes are unavailable." }),
  renameWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace changes are unavailable." }),
  moveWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace changes are unavailable." }),
  copyWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace changes are unavailable." }),
  duplicateWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace changes are unavailable." }),
  deleteWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace changes are unavailable." }),
  revealWorkspaceEntry: async () => ({ state: "unavailable", message: "Workspace reveal is unavailable." }),
  refreshWorkspace: async () => false,
}

const unavailablePage = (message: string): DesktopWorkspaceTreePage => ({ state: "unavailable", message })
const unavailableOperation = (message: string): DesktopWorkspaceOperationResult => ({ state: "unavailable", message })

const treePageFrom = async (
  bridge: WorkspaceBrowserBridge,
  directoryRef: string,
  offset = 0,
  limit = 200,
): Promise<DesktopWorkspaceTreePage> => decodeWorkspaceTreePage(
  await bridge.workspaceTree({ directoryRef, offset, limit }).catch(() => null),
) ?? unavailablePage("The workspace tree response could not be read.")

const workspacePathIndexSource = (
  bridge: WorkspaceBrowserBridge,
  grantRef: string,
): IdePathIndexSource => ({
  grantRef,
  readPage: ({ directoryRef, offset, limit }) =>
    Effect.promise(() => treePageFrom(bridge, directoryRef, offset, limit)),
})

const scanWorkspacePathIndex = (
  seed: IdePathIndexSnapshot,
  source: IdePathIndexSource,
  request: IdePathIndexScanRequest,
) => Effect.gen(function* () {
  const index = yield* IdePathIndexService
  const snapshot = yield* index.scan(request)
  const projection = yield* index.projectPierre()
  return { snapshot, projection }
}).pipe(Effect.provide(makeIdePathIndexLayer(seed, source)))

const reconcileWorkspacePathIndex = (
  seed: IdePathIndexSnapshot,
  source: IdePathIndexSource,
  request: IdePathIndexReconcileRequest,
) => Effect.gen(function* () {
  const index = yield* IdePathIndexService
  const snapshot = yield* index.reconcile(request)
  const projection = yield* index.projectPierre()
  return { snapshot, projection }
}).pipe(Effect.provide(makeIdePathIndexLayer(seed, source)))

const interactWorkspacePathIndex = (
  seed: IdePathIndexSnapshot,
  source: IdePathIndexSource,
  update: IdePathIndexInteractionUpdate,
) => Effect.gen(function* () {
  const index = yield* IdePathIndexService
  const snapshot = yield* index.interact(update)
  const projection = yield* index.projectPierre()
  return { snapshot, projection }
}).pipe(Effect.provide(makeIdePathIndexLayer(seed, source)))

const updateWorkspacePathIndexOperation = (
  seed: IdePathIndexSnapshot,
  source: IdePathIndexSource,
  update: IdePathIndexOperationUpdate,
) => Effect.gen(function* () {
  const index = yield* IdePathIndexService
  const snapshot = yield* index.updateOperation(update)
  const projection = yield* index.projectPierre()
  return { snapshot, projection }
}).pipe(Effect.provide(makeIdePathIndexLayer(seed, source)))

const operationFrom = async (
  call: () => Promise<unknown>,
): Promise<DesktopWorkspaceOperationResult> => decodeWorkspaceOperationResult(
  await call().catch(() => null),
) ?? unavailableOperation("The workspace operation response could not be read.")

const entryForRef = (
  state: WorkspaceBrowserState,
  pathRef: string,
): DesktopWorkspaceTreeEntry | null => {
  for (const page of Object.values(state.pages)) {
    const found = page.entries.find(entry => entry.pathRef === pathRef)
    if (found !== undefined) return found
  }
  const indexed = state.pathIndexSnapshot?.nodes.find(node => node.pathRef === pathRef)
  if (indexed !== undefined) {
    return {
      name: indexed.name,
      pathRef: indexed.pathRef,
      kind: indexed.kind,
      expandable: indexed.expandable,
      sizeBytes: indexed.sizeBytes,
      revisionRef: indexed.revisionRef,
    }
  }
  return null
}

export const makeWorkspaceBrowserHandlers = <S extends WorkspaceBrowserCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: WorkspaceBrowserBridge = unavailableWorkspaceBrowserBridge,
  resolveIndexIdentity: WorkspaceBrowserIndexIdentityResolver<S> = defaultWorkspaceBrowserIndexIdentity,
) => {
  let searchSequence = 0
  let activeSearchRef: string | null = null
  let changeSequence = 0
  let pathIndexSequence = 0
  let pathIndexGeneration = 0
  let operationSequence = 0

  const setBrowser = (mutate: (browser: WorkspaceBrowserState) => WorkspaceBrowserState) =>
    SubscriptionRef.update(state, next => ({ ...next, workspaceBrowser: mutate(next.workspaceBrowser) }))

  const cancelActiveSearch = Effect.gen(function* () {
    const requestRef = activeSearchRef
    if (requestRef === null) return
    activeSearchRef = null
    yield* setBrowser(browser => ({ ...browser, searchState: "idle", searchPage: null }))
    yield* Effect.promise(() => bridge.cancelWorkspaceSearch({ requestRef }).catch(() => null))
  })

  const loadRoot = (
    operation: DesktopWorkspaceOperationResult | null = null,
    reason: "initial" | "explicit_rescan" | "root_refresh" = "initial",
    advanceGeneration = false,
  ) =>
    Effect.gen(function* () {
      const page = yield* Effect.promise(() => treePageFrom(bridge, ""))
      if (page.state === "unavailable") {
        pathIndexSequence += 1
        yield* setBrowser(browser => ({ ...withWorkspaceBrowserRoot(browser, page), operation }))
        return
      }
      if (pathIndexGeneration === 0) pathIndexGeneration = 1
      else if (advanceGeneration) pathIndexGeneration += 1
      const sequence = ++pathIndexSequence
      const current = yield* SubscriptionRef.get(state)
      const identity = resolveIndexIdentity(current, page.grantRef, pathIndexGeneration)
      const prior = current.workspaceBrowser.pathIndexSnapshot
      const sameIdentity = prior !== null &&
        prior.identity.projectRef === identity.projectRef &&
        prior.identity.rootRef === identity.rootRef &&
        prior.identity.worktreeRef === identity.worktreeRef &&
        prior.identity.attachmentRef === identity.attachmentRef &&
        prior.identity.attachmentGeneration === identity.attachmentGeneration &&
        prior.identity.pathIndexGeneration === identity.pathIndexGeneration
      const seed = sameIdentity ? prior : emptyIdePathIndexSnapshot(identity)
      const source = workspacePathIndexSource(bridge, page.grantRef)
      const partial = yield* scanWorkspacePathIndex(seed, source, {
        identity,
        scanRef: IdePathScanRefSchema.make(`ide.path-scan.browser-${sequence}-root`),
        reason,
        mode: "root_and_expanded",
        chunkSize: 200,
        maximumNodes: 250_000,
      }).pipe(Effect.result)
      if (sequence !== pathIndexSequence) return
      if (partial._tag === "Failure") {
        yield* setBrowser(browser => ({
          ...withWorkspaceBrowserRoot(browser, page),
          operation: { state: "unavailable", message: "The complete workspace index could not be started." },
        }))
        return
      }
      yield* setBrowser(browser => ({
        ...withWorkspaceBrowserRoot(browser, page),
        operation,
        pathIndexSnapshot: partial.success.snapshot,
        pathIndexProjection: partial.success.projection,
      }))
      const complete = yield* scanWorkspacePathIndex(partial.success.snapshot, source, {
        identity,
        scanRef: IdePathScanRefSchema.make(`ide.path-scan.browser-${sequence}-complete`),
        reason,
        mode: "complete",
        chunkSize: 200,
        maximumNodes: 250_000,
      }).pipe(Effect.result)
      if (sequence !== pathIndexSequence) return
      if (complete._tag === "Failure") {
        yield* setBrowser(browser => ({
          ...browser,
          operation: { state: "unavailable", message: "Workspace indexing is partial. Retry or rescan to continue." },
        }))
        return
      }
      yield* setBrowser(browser => ({
        ...browser,
        pathIndexSnapshot: complete.success.snapshot,
        pathIndexProjection: complete.success.projection,
      }))
    })

  const refresh = Effect.gen(function* () {
    yield* cancelActiveSearch
    yield* setBrowser(withWorkspaceBrowserLoading)
    const refreshed = yield* Effect.promise(() => bridge.refreshWorkspace().catch(() => false))
    if (refreshed !== true) {
      yield* setBrowser(browser => withWorkspaceBrowserRoot(browser, unavailablePage("The selected workspace could not be refreshed.")))
      return
    }
    yield* loadRoot(null, "explicit_rescan", true)
  })

  const reloadFromChange = (change: DesktopWorkspaceChange) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    if (current.workspace !== undefined && current.workspace !== "files") return
    if (current.workspaceBrowser.phase !== "ready") return
    const changedRefs = workspaceChangePathRefs(change)
    const loadedRefs = Object.keys(current.workspaceBrowser.pages)
    const affectedRefs = changedRefs === null ? [""] : loadedRefs.filter(directoryRef =>
      changedRefs.some(pathRef => {
        const slash = pathRef.lastIndexOf("/")
        const parentRef = slash < 0 ? "" : pathRef.slice(0, slash)
        return directoryRef === parentRef || directoryRef === pathRef
      }))
    const sequence = ++changeSequence
    yield* cancelActiveSearch
    const indexed = current.workspaceBrowser.pathIndexSnapshot
    if (indexed !== null && current.workspaceBrowser.grantRef !== null) {
      const reconciled = yield* reconcileWorkspacePathIndex(
        indexed,
        workspacePathIndexSource(bridge, current.workspaceBrowser.grantRef),
        {
          identity: indexed.identity,
          change,
          scanRef: IdePathScanRefSchema.make(`ide.path-scan.watch-${sequence}`),
        },
      ).pipe(Effect.result)
      if (sequence !== changeSequence) return
      if (reconciled._tag === "Success") {
        yield* setBrowser(browser => ({
          ...browser,
          pathIndexSnapshot: reconciled.success.snapshot,
          pathIndexProjection: reconciled.success.projection,
        }))
      } else {
        yield* setBrowser(browser => ({
          ...browser,
          operation: { state: "unavailable", message: "Workspace changes require an explicit index rescan." },
        }))
      }
    }
    if (affectedRefs.length === 0) return
    const pages = yield* Effect.promise(() => Promise.all(affectedRefs.map(directoryRef => treePageFrom(bridge, directoryRef))))
    if (sequence !== changeSequence) return
    const latest = yield* SubscriptionRef.get(state)
    if (latest.workspace !== undefined && latest.workspace !== "files") return
    yield* setBrowser(browser => {
      const replacements = { ...browser.pages }
      for (const page of pages) {
        if (page.state === "available" && page.grantRef === browser.grantRef) replacements[page.directoryRef] = page
      }
      return { ...browser, pages: replacements }
    })
  })

  const runSearch = (offset: number, append: boolean) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const query = current.workspaceBrowser.query.trim()
      if (current.workspaceBrowser.phase !== "ready" || query === "") return
      yield* cancelActiveSearch
      const requestRef = `workspace.search.request.renderer-${++searchSequence}`
      activeSearchRef = requestRef
      yield* setBrowser(browser => withWorkspaceBrowserSearchStarted(browser, append))
      const raw = yield* Effect.promise(() => bridge.workspaceSearch({
        requestRef,
        query,
        mode: current.workspaceBrowser.searchMode,
        offset,
        limit: 100,
      }).catch(() => null))
      if (activeSearchRef !== requestRef) return
      activeSearchRef = null
      const response: DesktopWorkspaceSearchResponse | null = decodeWorkspaceSearchResponse(raw)
      if (response === null || response.requestRef !== requestRef) {
        yield* setBrowser(browser => withWorkspaceBrowserSearch(browser, {
          state: "unavailable",
          message: "The workspace search response could not be read.",
        }))
        return
      }
      yield* setBrowser(browser => withWorkspaceBrowserSearch(browser, response.page, append))
    })

  const reloadAfterOperation = (result: DesktopWorkspaceOperationResult) =>
    result.state === "created" || result.state === "renamed" || result.state === "deleted"
      ? loadRoot(result, "root_refresh")
      : setBrowser(browser => withWorkspaceBrowserOperation(browser, result))

  const runExplorerMutation = (command: IdeExplorerCommand) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const indexed = current.workspaceBrowser.pathIndexSnapshot
    const grantRef = current.workspaceBrowser.grantRef
    if (indexed === null || grantRef === null) return
    const operationRef = IdePathOperationRefSchema.make(`ide.path-operation.browser-${++operationSequence}`)
    const source = workspacePathIndexSource(bridge, grantRef)
    const pending = yield* updateWorkspacePathIndexOperation(indexed, source, {
      _tag: "Pending",
      identity: indexed.identity,
      operationRef,
      command,
    }).pipe(Effect.result)
    if (pending._tag === "Failure") return
    yield* setBrowser(browser => ({
      ...browser,
      pathIndexSnapshot: pending.success.snapshot,
      pathIndexProjection: pending.success.projection,
    }))
    const result = command._tag === "Rename"
      ? yield* Effect.promise(() => operationFrom(() => bridge.renameWorkspaceEntry({
          pathRef: command.pathRef,
          name: command.name,
          expectedRevisionRef: command.expectedRevisionRef,
        })))
      : command._tag === "Delete"
        ? yield* Effect.promise(() => operationFrom(() => bridge.deleteWorkspaceEntry({
            pathRef: command.pathRef,
            expectedRevisionRef: command.expectedRevisionRef,
          })))
        : command._tag === "Move"
          ? yield* Effect.promise(() => operationFrom(() => bridge.moveWorkspaceEntry({
              pathRef: command.pathRef,
              destinationParentRef: command.destinationParentPathRef,
              expectedRevisionRef: command.expectedRevisionRef,
            })))
          : command._tag === "Copy"
            ? yield* Effect.promise(() => operationFrom(() => bridge.copyWorkspaceEntry({
                pathRef: command.pathRef,
                destinationParentRef: command.destinationParentPathRef,
                expectedRevisionRef: command.expectedRevisionRef,
              })))
            : command._tag === "Duplicate"
              ? yield* Effect.promise(() => operationFrom(() => bridge.duplicateWorkspaceEntry({
                  pathRef: command.pathRef,
                  expectedRevisionRef: command.expectedRevisionRef,
                })))
              : command._tag === "CreateFile" || command._tag === "CreateFolder"
          ? yield* Effect.promise(() => operationFrom(() => bridge.createWorkspaceEntry({
              parentRef: command.parentPathRef,
              name: command.name,
              kind: command._tag === "CreateFile" ? "file" : "directory",
            })))
          : unavailableOperation("This expected-version operation is typed but not admitted by the current workspace host.")
    const latest = yield* SubscriptionRef.get(state)
    const latestIndex = latest.workspaceBrowser.pathIndexSnapshot
    if (latestIndex === null || latestIndex.identity.pathIndexGeneration !== indexed.identity.pathIndexGeneration) return
    const sourceNodeRef = "nodeRef" in command ? command.nodeRef : null
    const update = result.state === "created" || result.state === "renamed"
      ? {
          _tag: "Confirmed" as const,
          identity: latestIndex.identity,
          operationRef,
          sourceNodeRef,
          entry: result.entry,
        }
      : result.state === "deleted"
        ? {
            _tag: "Confirmed" as const,
            identity: latestIndex.identity,
            operationRef,
            sourceNodeRef,
            entry: null,
          }
        : {
            _tag: "Refused" as const,
            identity: latestIndex.identity,
            operationRef,
            reason: result.state === "conflict"
              ? "stale_revision" as const
              : result.state === "permission_denied"
                ? "permission_denied" as const
                : "unavailable" as const,
            message: "message" in result ? result.message : "The expected-version operation was not admitted.",
          }
    const settled = yield* updateWorkspacePathIndexOperation(latestIndex, source, update).pipe(Effect.result)
    if (settled._tag === "Success") {
      yield* setBrowser(browser => ({
        ...browser,
        pathIndexSnapshot: settled.success.snapshot,
        pathIndexProjection: settled.success.projection,
      }))
    }
    yield* reloadAfterOperation(result)
  })

  return {
    WorkspaceBrowserOpened: () => loadRoot(),
    WorkspaceBrowserRefreshRequested: () => refresh,

    WorkspaceBrowserTreeToggled: (directoryRef: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const entry = entryForRef(current.workspaceBrowser, directoryRef)
        if (entry?.kind !== "directory") return
        const expanding = !current.workspaceBrowser.expandedRefs.includes(directoryRef)
        const needsPage = expanding && current.workspaceBrowser.pages[directoryRef] === undefined
        yield* setBrowser(browser => withWorkspaceBrowserToggled(browser, directoryRef))
        const indexed = current.workspaceBrowser.pathIndexSnapshot
        const indexedNode = indexed?.nodes.find(node => node.pathRef === directoryRef)
        if (indexed !== null && indexed !== undefined && indexedNode !== undefined && current.workspaceBrowser.grantRef !== null) {
          const interaction = yield* interactWorkspacePathIndex(
            indexed,
            workspacePathIndexSource(bridge, current.workspaceBrowser.grantRef),
            { _tag: expanding ? "Expand" : "Collapse", nodeRef: indexedNode.nodeRef },
          ).pipe(Effect.result)
          if (interaction._tag === "Success") {
            yield* setBrowser(browser => ({
              ...browser,
              pathIndexSnapshot: interaction.success.snapshot,
              pathIndexProjection: interaction.success.projection,
            }))
          }
        }
        if (!needsPage) return
        const page = yield* Effect.promise(() => treePageFrom(bridge, directoryRef))
        yield* setBrowser(browser => withWorkspaceBrowserPage(browser, page))
      }),

    WorkspaceBrowserTreeMoreRequested: (directoryRef: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const offset = current.workspaceBrowser.pages[directoryRef]?.nextOffset
        if (offset === null || offset === undefined) return
        const page = yield* Effect.promise(() => treePageFrom(bridge, directoryRef, offset))
        yield* setBrowser(browser => withWorkspaceBrowserPage(browser, page))
      }),

    WorkspaceBrowserEntrySelected: (pathRef: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (entryForRef(current.workspaceBrowser, pathRef) === null) return
        yield* setBrowser(browser => ({
          ...browser,
          selectedRef: pathRef,
          editor: null,
          deleteConfirmRef: null,
          operation: null,
        }))
        const indexed = current.workspaceBrowser.pathIndexSnapshot
        const indexedNode = indexed?.nodes.find(node => node.pathRef === pathRef)
        if (indexed === null || indexed === undefined || indexedNode === undefined || current.workspaceBrowser.grantRef === null) return
        const interaction = yield* interactWorkspacePathIndex(
          indexed,
          workspacePathIndexSource(bridge, current.workspaceBrowser.grantRef),
          { _tag: "Reveal", nodeRef: indexedNode.nodeRef },
        ).pipe(Effect.result)
        if (interaction._tag === "Success") {
          yield* setBrowser(browser => ({
            ...browser,
            pathIndexSnapshot: interaction.success.snapshot,
            pathIndexProjection: interaction.success.projection,
          }))
        }
      }),

    WorkspaceBrowserQueryChanged: (query: string) =>
      setBrowser(browser => ({ ...browser, query: query.slice(0, 200) })),

    WorkspaceBrowserSearchModeSelected: (mode: "path" | "content") =>
      Effect.gen(function* () {
        yield* cancelActiveSearch
        yield* setBrowser(browser => ({ ...browser, searchMode: mode, searchState: "idle", searchPage: null }))
      }),

    WorkspaceBrowserSearchRequested: () => runSearch(0, false),
    WorkspaceBrowserSearchCancelled: () => cancelActiveSearch,

    WorkspaceBrowserSearchMoreRequested: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const offset = current.workspaceBrowser.searchPage?.state === "available"
          ? current.workspaceBrowser.searchPage.nextOffset
          : null
        if (offset === null) return
        yield* runSearch(offset, true)
      }),

    WorkspaceBrowserCreateStarted: (payload: { parentRef: string; kind: "file" | "directory" }) =>
      setBrowser(browser => browser.phase !== "ready" ||
        (payload.parentRef !== "" && entryForRef(browser, payload.parentRef)?.kind !== "directory")
        ? browser
        : withWorkspaceBrowserEditor(browser, {
            kind: payload.kind === "file" ? "create_file" : "create_directory",
            parentRef: payload.parentRef,
            value: "",
          })),

    WorkspaceBrowserRenameStarted: (payload: { pathRef: string; name: string; expectedRevisionRef: string }) =>
      setBrowser(browser => entryForRef(browser, payload.pathRef)?.revisionRef !== payload.expectedRevisionRef
        ? browser
        : withWorkspaceBrowserEditor(browser, { kind: "rename", ...payload, value: payload.name })),

    WorkspaceBrowserEditorChanged: (value: string) =>
      setBrowser(browser => browser.editor === null
        ? browser
        : { ...browser, editor: { ...browser.editor, value: value.slice(0, 120) }, operation: null }),

    WorkspaceBrowserEditorSubmitted: () =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        const editor = current.workspaceBrowser.editor
        if (editor === null || editor.value.trim() === "") return
        const name = editor.value.trim()
        const result = editor.kind === "rename"
          ? yield* Effect.promise(() => operationFrom(() => bridge.renameWorkspaceEntry({
              pathRef: editor.pathRef,
              name,
              expectedRevisionRef: editor.expectedRevisionRef,
            })))
          : yield* Effect.promise(() => operationFrom(() => bridge.createWorkspaceEntry({
              parentRef: editor.parentRef,
              name,
              kind: editor.kind === "create_file" ? "file" : "directory",
            })))
        yield* reloadAfterOperation(result)
      }),

    WorkspaceBrowserEditorCancelled: () => setBrowser(browser => withWorkspaceBrowserEditor(browser, null)),

    WorkspaceBrowserDeleteRequested: (payload: { pathRef: string; expectedRevisionRef: string }) =>
      setBrowser(browser => entryForRef(browser, payload.pathRef)?.revisionRef !== payload.expectedRevisionRef
        ? browser
        : { ...browser, deleteConfirmRef: payload.pathRef, editor: null, operation: null }),

    WorkspaceBrowserDeleteConfirmed: (payload: { pathRef: string; expectedRevisionRef: string }) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (current.workspaceBrowser.deleteConfirmRef !== payload.pathRef ||
            entryForRef(current.workspaceBrowser, payload.pathRef)?.revisionRef !== payload.expectedRevisionRef) return
        const result = yield* Effect.promise(() => operationFrom(() => bridge.deleteWorkspaceEntry(payload)))
        yield* reloadAfterOperation(result)
      }),

    WorkspaceBrowserDeleteCancelled: () =>
      setBrowser(browser => ({ ...browser, deleteConfirmRef: null })),

    WorkspaceBrowserRevealRequested: (pathRef: string) =>
      Effect.gen(function* () {
        const current = yield* SubscriptionRef.get(state)
        if (entryForRef(current.workspaceBrowser, pathRef) === null) return
        const result = yield* Effect.promise(() => operationFrom(() => bridge.revealWorkspaceEntry({ pathRef })))
        yield* setBrowser(browser => withWorkspaceBrowserOperation(browser, result))
      }),

    WorkspaceBrowserChangeReceived: (change: DesktopWorkspaceChange) => reloadFromChange(change),

    WorkspaceBrowserExplorerCommandRequested: (command: IdeExplorerCommand) => {
      switch (command._tag) {
        case "Open": return Effect.gen(function* () {
          yield* setBrowser(browser => entryForRef(browser, command.pathRef) === null
            ? browser
            : { ...browser, selectedRef: command.pathRef })
        })
        case "Reveal": return Effect.gen(function* () {
          const result = yield* Effect.promise(() => operationFrom(() =>
            bridge.revealWorkspaceEntry({ pathRef: command.pathRef })))
          yield* setBrowser(browser => withWorkspaceBrowserOperation(browser, result))
        })
        case "Rename":
        case "Delete":
        case "CreateFile":
        case "CreateFolder":
        case "Move":
        case "Copy":
        case "Duplicate": return runExplorerMutation(command)
        case "Refresh":
        case "Retry":
        case "Rescan": return refresh
        case "OpenTerminal":
        case "Compare": return setBrowser(browser => ({ ...browser, selectedRef: command.pathRef }))
      }
    },
  }
}

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

// UX-4 (#8790): the inline create/rename name form rendered no more — file
// creation and renaming are filesystem mutation affordances not called for by
// the MVP spec (CW-AC-14 grants a bounded file tree for review, not
// grant-scoped file management). The typed intents/handlers remain internal
// substrate and authorize no visible control.

const treeRow =(state: WorkspaceBrowserState, row: WorkspaceBrowserRow): View => {
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
        variant: "ghost",
        selected: state.selectedRef === entry.pathRef,
        onPress: IntentRef("WorkspaceBrowserEntrySelected", StaticPayload(entry.pathRef)),
        style: { flex: 1, minWidth: 0 },
        a11y: {
          role: "treeitem",
          label: `${entry.kind === "directory" ? "Folder" : "File"} ${entry.pathRef}`,
          selected: state.selectedRef === entry.pathRef,
          level: depth + 1,
        },
      }),
      ...(loading ? [Spinner({ key: `workspace-browser-loading-${entry.pathRef}`, size: "sm", label: "Loading" })] : []),
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
    items.push(EmptyMessage({
      key: "workspace-browser-tree-empty",
      icon: { name: "FolderOpen", tone: "secondary" },
      title: "This folder has no visible files. Hidden, ignored, secret-shaped, and unsafe entries stay withheld.",
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
    { key: "workspace-browser-tree", direction: "column", gap: "1", style: { minWidth: "sm", maxWidth: "md", flex: 1, minHeight: 0 } },
    [
      Text({ key: "workspace-browser-tree-title", content: "Workspace", variant: "caption", color: "textMuted" }),
      List({ key: "workspace-browser-tree-list", virtualize: true, estimatedItemSize: 36, a11y: { role: "tree", label: "Workspace files" }, style: { flex: 1, minHeight: 0 } }, items),
    ],
  )
}

const searchResultsView = (state: WorkspaceBrowserState): View => {
  if (state.searchState === "searching") {
    return ShimmerText({ key: "workspace-browser-searching", text: "Searching the selected workspace…", typeScale: "body", style: { color: "textMuted" } })
  }
  if (state.searchPage?.state !== "available") {
    return EmptyMessage({
      key: "workspace-browser-search-empty",
      icon: { name: "Search", tone: "secondary" },
      title: "Search paths or bounded text content. Results never expose the selected root.",
    })
  }
  const page = state.searchPage
  if (page.matches.length === 0) {
    return EmptyMessage({
      key: "workspace-browser-search-none",
      icon: { name: "Search", tone: "secondary" },
      title: `No ${page.mode} matches for “${page.query}”.`,
    })
  }
  return Stack(
    { key: "workspace-browser-search-results", direction: "column", gap: "2", style: { flex: 1, minHeight: 0 } },
    [
      Text({ key: "workspace-browser-search-count", content: `${page.matches.length} result${page.matches.length === 1 ? "" : "s"}`, variant: "caption", color: "textMuted" }),
      List(
        { key: "workspace-browser-search-list", virtualize: true, estimatedItemSize: 48, a11y: { role: "list", label: "Workspace search results" }, style: { flex: 1, minHeight: 0 } },
        page.matches.map((match, index) => Stack(
          { key: `workspace-browser-search-${match.pathRef}-${index}`, direction: "column", gap: "1", a11y: { role: "listitem" }, style: { width: "full" } },
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

/**
 * UX-4 (#8790): the selection footer keeps only the read-only path fact.
 * Reveal/Rename/Delete were filesystem mutation affordances not called for by
 * the MVP spec; their typed intents remain internal substrate only.
 */
const selectionActions = (state: WorkspaceBrowserState): View[] => {
  const entry = selectedEntry(state)
  if (entry === null) return []
  return [
    Divider({ key: "workspace-browser-selection-divider" }),
    Text({ key: "workspace-browser-selection-path", content: entry.pathRef, variant: "caption", color: "textPrimary" }),
  ]
}

export const workspaceBrowserView = (state: WorkspaceBrowserState): View => {
  const entry = selectedEntry(state)
  const parentRef = parentRefFor(entry)
  return Stack(
    { key: "workspace-browser", direction: "column", gap: "3", style: { width: "full", minWidth: 0, flex: 1, minHeight: 0 } },
    [
      Stack({ key: "workspace-browser-heading", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
        // UX-4 (#8790) design pass: title scale, matching the other panels.
        Text({ key: "workspace-browser-title", content: "Files", variant: "title", color: "textPrimary" }),
        Badge({ key: "workspace-browser-boundary", label: "Relative workspace access", tone: "neutral", a11y: { label: "Workspace access is grant scoped and relative" } }),
        Spacer({ key: "workspace-browser-heading-space", size: "1" }),
        Button({ key: "workspace-browser-refresh", label: "Refresh", variant: "ghost", disabled: state.phase === "loading", onPress: IntentRef("WorkspaceBrowserRefreshRequested"), a11y: { label: "Refresh workspace files" } }),
        Button({ key: "workspace-browser-choose", label: state.phase === "idle" ? "Choose folder" : "Change folder", variant: "secondary", onPress: IntentRef("DesktopWorkspacePickerRequested"), a11y: { label: state.phase === "idle" ? "Choose a local workspace folder" : "Change the local workspace folder" } }),
      ]),
      ...(state.phase === "idle" ? [EmptyMessage({
            key: "workspace-browser-idle",
            icon: { name: "Folder", tone: "secondary" },
            title: "Choose a local folder to browse a safe, grant-scoped tree. Hidden, ignored, secret-shaped, binary, and escaping entries remain unavailable.",
          })]
        : state.phase === "loading" ? [ShimmerText({ key: "workspace-browser-loading", text: "Loading the selected workspace…", typeScale: "body", style: { color: "textMuted" } })]
        : state.phase === "unavailable" ? [EmptyMessage({
            key: "workspace-browser-unavailable",
            icon: { name: "AlertTriangle", tone: "warning" },
            title: "Workspace unavailable",
            description: state.reason ?? "The selected workspace could not be read.",
            action: Button({ key: "workspace-browser-unavailable-retry", label: "Try again", variant: "secondary", onPress: IntentRef("WorkspaceBrowserRefreshRequested"), a11y: { label: "Try loading workspace files again" } }),
          })]
        : [
            Stack({ key: "workspace-browser-search-controls", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
              TextField({ key: "workspace-browser-query", value: state.query, placeholder: state.searchMode === "path" ? "Search file and folder names" : "Search bounded text content", onChange: IntentRef("WorkspaceBrowserQueryChanged", ComponentValueBinding()), onSubmit: IntentRef("WorkspaceBrowserSearchRequested"), disabled: state.searchState === "searching", a11y: { label: state.searchMode === "path" ? "Search workspace paths" : "Search workspace file content" }, style: { flex: 1, minWidth: 0 } }),
              SegmentedControl({
                key: "workspace-browser-search-mode",
                options: [
                  { id: "path", label: "Path" },
                  { id: "content", label: "Content" },
                ],
                value: state.searchMode,
                onChange: IntentRef("WorkspaceBrowserSearchModeSelected", ComponentValueBinding()),
                a11y: { label: "Workspace search mode" },
              }),
              Button({ key: "workspace-browser-search-submit", label: state.searchState === "searching" ? "Cancel" : "Search", variant: "primary", disabled: state.searchState !== "searching" && state.query.trim() === "", onPress: IntentRef(state.searchState === "searching" ? "WorkspaceBrowserSearchCancelled" : "WorkspaceBrowserSearchRequested"), a11y: { label: state.searchState === "searching" ? "Cancel workspace search" : "Search workspace" } }),
            ]),
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
