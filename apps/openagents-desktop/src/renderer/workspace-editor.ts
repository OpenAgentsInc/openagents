/**
 * CUT-18 typed document lifecycle and replaceable CodeEditor projection.
 * Effect Native owns tabs, drafts, revisions, selection, find, history, and
 * conflict choices. The foreign editor receives serializable props only.
 */
import {
  Badge,
  Button,
  CodeEditor,
  CodeEditorEventSchema,
  ComponentValueBinding,
  Divider,
  EmptyMessage,
  IntentRef,
  ShimmerText,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type CodeEditorEvent,
  type View,
} from "@effect-native/core"
import { Effect, Exit, Schema, SubscriptionRef } from "@effect-native/core/effect"

import {
  DesktopWorkspaceChangeSchema,
  DesktopWorkspaceDocumentResultSchema,
  DesktopWorkspacePathRefSchema,
  decodeWorkspaceDocumentResult,
  type DesktopWorkspaceChange,
  type DesktopWorkspaceDocument,
  type DesktopWorkspaceDocumentResult,
} from "../workspace-contract.ts"

const maxTabs = 12
const maxHistory = 100
const maxFindMatches = 1_000

export type WorkspaceEditorTab = Readonly<{
  pathRef: string
  phase: "loading" | "ready" | "unavailable" | "conflict"
  document: DesktopWorkspaceDocument | null
  externalDocument: DesktopWorkspaceDocument | null
  draft: string
  selection: Readonly<{ start: number; end: number }>
  selectionVersion: number
  undo: ReadonlyArray<string>
  redo: ReadonlyArray<string>
  saveState: "idle" | "saving" | "saved" | "unavailable"
  reason: string | null
  findQuery: string
  findMatches: ReadonlyArray<number>
  findIndex: number
}>

export type WorkspaceEditorState = Readonly<{
  tabs: ReadonlyArray<WorkspaceEditorTab>
  activePathRef: string | null
  closeConfirmRef: string | null
  wordWrap: boolean
  minimap: boolean
  saveAsPathRef: string | null
}>

export const emptyWorkspaceEditorState = (): WorkspaceEditorState => ({
  tabs: [],
  activePathRef: null,
  closeConfirmRef: null,
  wordWrap: false,
  minimap: false,
  saveAsPathRef: null,
})

export const workspaceEditorTabDirty = (tab: WorkspaceEditorTab): boolean =>
  tab.document !== null && tab.draft !== tab.document.content

const tabFor = (state: WorkspaceEditorState, pathRef: string): WorkspaceEditorTab | null =>
  state.tabs.find(tab => tab.pathRef === pathRef) ?? null

const activeTab = (state: WorkspaceEditorState): WorkspaceEditorTab | null =>
  state.activePathRef === null ? null : tabFor(state, state.activePathRef)

const updateTab = (
  state: WorkspaceEditorState,
  pathRef: string,
  update: (tab: WorkspaceEditorTab) => WorkspaceEditorTab,
): WorkspaceEditorState => ({
  ...state,
  tabs: state.tabs.map(tab => tab.pathRef === pathRef ? update(tab) : tab),
})

const emptyTab = (pathRef: string): WorkspaceEditorTab => ({
  pathRef,
  phase: "loading",
  document: null,
  externalDocument: null,
  draft: "",
  selection: { start: 0, end: 0 },
  selectionVersion: 0,
  undo: [],
  redo: [],
  saveState: "idle",
  reason: null,
  findQuery: "",
  findMatches: [],
  findIndex: 0,
})

export const withWorkspaceEditorOpening = (
  state: WorkspaceEditorState,
  pathRef: string,
): WorkspaceEditorState => {
  const existing = tabFor(state, pathRef)
  if (existing !== null) return { ...state, activePathRef: pathRef, closeConfirmRef: null }
  if (state.tabs.length >= maxTabs) return state
  return {
    ...state,
    tabs: [...state.tabs, emptyTab(pathRef)],
    activePathRef: pathRef,
    closeConfirmRef: null,
  }
}

export const withWorkspaceEditorOpened = (
  state: WorkspaceEditorState,
  pathRef: string,
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => updateTab(state, pathRef, tab => {
  if (result.state !== "available" || result.document.pathRef !== pathRef) {
    return {
      ...tab,
      phase: "unavailable",
      reason: result.state === "unavailable" ? result.message : "The document response did not match this tab.",
      saveState: "unavailable",
    }
  }
  return {
    ...tab,
    phase: "ready",
    document: result.document,
    externalDocument: null,
    draft: result.document.content,
    selection: { start: 0, end: 0 },
    selectionVersion: 0,
    undo: [],
    redo: [],
    saveState: "idle",
    reason: null,
  }
})

/** Keeps open tabs attached to the same entry after a confirmed file/folder rename. */
export const withWorkspaceEditorRenamed = (
  state: WorkspaceEditorState,
  sourcePathRef: string,
  targetPathRef: string,
): WorkspaceEditorState => {
  const remap = (pathRef: string): string | null => pathRef === sourcePathRef
    ? targetPathRef
    : pathRef.startsWith(`${sourcePathRef}/`)
      ? `${targetPathRef}${pathRef.slice(sourcePathRef.length)}`
      : null
  const renamed = state.tabs.map(tab => {
    const pathRef = remap(tab.pathRef)
    if (pathRef === null) return tab
    return {
      ...tab,
      pathRef,
      document: tab.document === null ? null : { ...tab.document, pathRef },
      externalDocument: tab.externalDocument === null ? null : { ...tab.externalDocument, pathRef },
      reason: `Renamed to ${pathRef}.`,
    }
  })
  const pathRefs = renamed.map(tab => tab.pathRef)
  if (new Set(pathRefs).size !== pathRefs.length) return state
  return {
    ...state,
    tabs: renamed,
    activePathRef: state.activePathRef === null ? null : remap(state.activePathRef) ?? state.activePathRef,
    closeConfirmRef: state.closeConfirmRef === null ? null : remap(state.closeConfirmRef) ?? state.closeConfirmRef,
    saveAsPathRef: null,
  }
}

const findOffsets = (content: string, query: string): ReadonlyArray<number> => {
  const needle = query.toLocaleLowerCase()
  if (needle === "") return []
  const haystack = content.toLocaleLowerCase()
  const matches: number[] = []
  let offset = 0
  while (matches.length < maxFindMatches) {
    const found = haystack.indexOf(needle, offset)
    if (found < 0) break
    matches.push(found)
    offset = found + Math.max(1, needle.length)
  }
  return matches
}

const nextSelectionVersion = (version: number): number =>
  version >= Number.MAX_SAFE_INTEGER ? 0 : version + 1

export const withWorkspaceEditorEvent = (
  state: WorkspaceEditorState,
  event: CodeEditorEvent,
): WorkspaceEditorState => {
  const tab = activeTab(state)
  if (tab === null || (tab.phase !== "ready" && tab.phase !== "conflict")) return state
  if (event.type === "selection") {
    const maximum = tab.draft.length
    const start = Math.max(0, Math.min(maximum, Math.trunc(event.start)))
    const end = Math.max(start, Math.min(maximum, Math.trunc(event.end)))
    return updateTab(state, tab.pathRef, current => ({ ...current, selection: { start, end } }))
  }
  const value = event.value.slice(0, 1_000_000)
  if (value === tab.draft) return state
  const undo = [...tab.undo, tab.draft].slice(-maxHistory)
  return updateTab(state, tab.pathRef, current => ({
    ...current,
    draft: value,
    undo,
    redo: [],
    saveState: "idle",
    reason: null,
    findMatches: findOffsets(value, current.findQuery),
    findIndex: 0,
  }))
}

export const withWorkspaceEditorUndo = (state: WorkspaceEditorState): WorkspaceEditorState => {
  const tab = activeTab(state)
  const value = tab?.undo.at(-1)
  if (tab === null || value === undefined) return state
  return updateTab(state, tab.pathRef, current => ({
    ...current,
    draft: value,
    undo: current.undo.slice(0, -1),
    redo: [...current.redo, current.draft].slice(-maxHistory),
    selection: { start: value.length, end: value.length },
    selectionVersion: nextSelectionVersion(current.selectionVersion),
    saveState: "idle",
    findMatches: findOffsets(value, current.findQuery),
    findIndex: 0,
  }))
}

export const withWorkspaceEditorRedo = (state: WorkspaceEditorState): WorkspaceEditorState => {
  const tab = activeTab(state)
  const value = tab?.redo.at(-1)
  if (tab === null || value === undefined) return state
  return updateTab(state, tab.pathRef, current => ({
    ...current,
    draft: value,
    undo: [...current.undo, current.draft].slice(-maxHistory),
    redo: current.redo.slice(0, -1),
    selection: { start: value.length, end: value.length },
    selectionVersion: nextSelectionVersion(current.selectionVersion),
    saveState: "idle",
    findMatches: findOffsets(value, current.findQuery),
    findIndex: 0,
  }))
}

export const withWorkspaceEditorFind = (
  state: WorkspaceEditorState,
  query: string,
): WorkspaceEditorState => {
  const tab = activeTab(state)
  if (tab === null) return state
  const bounded = query.slice(0, 200)
  return updateTab(state, tab.pathRef, current => ({
    ...current,
    findQuery: bounded,
    findMatches: findOffsets(current.draft, bounded),
    findIndex: 0,
  }))
}

export const withWorkspaceEditorFindStep = (
  state: WorkspaceEditorState,
  delta: -1 | 1,
): WorkspaceEditorState => {
  const tab = activeTab(state)
  if (tab === null || tab.findMatches.length === 0) return state
  const findIndex = (tab.findIndex + delta + tab.findMatches.length) % tab.findMatches.length
  const start = tab.findMatches[findIndex]!
  return updateTab(state, tab.pathRef, current => ({
    ...current,
    findIndex,
    selection: { start, end: start + current.findQuery.length },
    selectionVersion: nextSelectionVersion(current.selectionVersion),
  }))
}

export const withWorkspaceEditorSaveResult = (
  state: WorkspaceEditorState,
  pathRef: string,
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => updateTab(state, pathRef, tab => {
  if (result.state === "saved" && result.document.pathRef === pathRef) {
    return {
      ...tab,
      phase: "ready",
      document: result.document,
      externalDocument: null,
      draft: result.document.content,
      saveState: "saved",
      reason: null,
      undo: [],
      redo: [],
    }
  }
  if (result.state === "conflict" && result.current.pathRef === pathRef) {
    return {
      ...tab,
      phase: "conflict",
      externalDocument: result.current,
      saveState: "idle",
      reason: "This document changed outside the editor.",
    }
  }
  return {
    ...tab,
    saveState: "unavailable",
    reason: result.state === "unavailable" ? result.message : "The save response did not match this tab.",
  }
})

export const withWorkspaceEditorSaveAsResult = (
  state: WorkspaceEditorState,
  sourcePathRef: string,
  targetPathRef: string,
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => {
  if (result.state !== "saved" || result.document.pathRef !== targetPathRef) {
    return updateTab(state, sourcePathRef, tab => ({
      ...tab,
      saveState: "unavailable",
      reason: result.state === "conflict"
        ? "Save As never overwrites an existing file. Choose another path."
        : result.state === "unavailable"
          ? result.message
          : "The Save As response did not match the requested path.",
    }))
  }
  const tabs = state.tabs
    .filter(tab => tab.pathRef === sourcePathRef || tab.pathRef !== targetPathRef)
    .map(tab => tab.pathRef !== sourcePathRef ? tab : {
      ...tab,
      pathRef: targetPathRef,
      phase: "ready" as const,
      document: result.document,
      externalDocument: null,
      draft: result.document.content,
      saveState: "saved" as const,
      reason: null,
      undo: [],
      redo: [],
    })
  return { ...state, tabs, activePathRef: targetPathRef, saveAsPathRef: null }
}

export const withWorkspaceEditorExternalResult = (
  state: WorkspaceEditorState,
  pathRef: string,
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => updateTab(state, pathRef, tab => {
  if (result.state !== "available" || result.document.pathRef !== pathRef) {
    if (!workspaceEditorTabDirty(tab)) {
      return {
        ...tab,
        phase: "unavailable",
        saveState: "unavailable",
        reason: result.state === "unavailable"
          ? result.message
          : "The external document response did not match this tab.",
      }
    }
    return {
      ...tab,
      phase: "conflict",
      externalDocument: null,
      saveState: "unavailable",
      reason: result.state === "unavailable"
        ? `${result.message} Your unsaved draft is preserved for recovery.`
        : "The external document response did not match this tab. Your unsaved draft is preserved for recovery.",
    }
  }
  if (tab.document?.revisionRef === result.document.revisionRef) return tab
  if (workspaceEditorTabDirty(tab)) {
    return {
      ...tab,
      phase: "conflict",
      externalDocument: result.document,
      saveState: "idle",
      reason: "This document changed outside the editor.",
    }
  }
  return {
    ...tab,
    phase: "ready",
    document: result.document,
    externalDocument: null,
    draft: result.document.content,
    selection: { start: 0, end: 0 },
    selectionVersion: nextSelectionVersion(tab.selectionVersion),
    undo: [],
    redo: [],
    saveState: "idle",
    reason: null,
    findMatches: findOffsets(result.document.content, tab.findQuery),
    findIndex: 0,
  }
})

export type WorkspaceEditorRecoverySnapshot = Readonly<{
  version: 2
  activePathRef: string | null
  tabs: ReadonlyArray<Readonly<{
    pathRef: string
    expectedRevisionRef: string
    draft: string
  }>>
}>

export const WorkspaceEditorRecoverySnapshotSchema = Schema.Struct({
  version: Schema.Literal(2),
  activePathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  tabs: Schema.Array(Schema.Struct({
    pathRef: DesktopWorkspacePathRefSchema,
    expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
    draft: Schema.String.check(Schema.isMaxLength(1_000_000)),
  })).check(Schema.isMaxLength(maxTabs)),
})

export const decodeWorkspaceEditorRecoverySnapshot = (value: unknown): WorkspaceEditorRecoverySnapshot | null => {
  const decoded = Schema.decodeUnknownExit(WorkspaceEditorRecoverySnapshotSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const workspaceEditorRecoverySnapshot = (
  state: WorkspaceEditorState,
): WorkspaceEditorRecoverySnapshot => ({
  version: 2,
  activePathRef: state.activePathRef,
  tabs: state.tabs.flatMap(tab => tab.document === null ? [] : [{
    pathRef: tab.pathRef,
    expectedRevisionRef: tab.document.revisionRef,
    draft: tab.draft.slice(0, 1_000_000),
  }]).slice(0, maxTabs),
})

const recoveryLanguageMode = (pathRef: string): DesktopWorkspaceDocument["languageMode"] => {
  const extension = pathRef.split(".").at(-1)?.toLocaleLowerCase()
  if (["ts", "tsx", "mts", "cts"].includes(extension ?? "")) return "typescript"
  if (["js", "jsx", "mjs", "cjs"].includes(extension ?? "")) return "javascript"
  if (["json", "jsonl"].includes(extension ?? "")) return "json"
  if (["md", "mdx"].includes(extension ?? "")) return "markdown"
  if (extension === "rs") return "rust"
  if (extension === "py") return "python"
  if (["sh", "bash", "zsh"].includes(extension ?? "")) return "shell"
  if (extension === "toml") return "toml"
  if (["yaml", "yml"].includes(extension ?? "")) return "yaml"
  if (extension === "css") return "css"
  if (["html", "htm"].includes(extension ?? "")) return "html"
  return "plaintext"
}

export const withWorkspaceEditorRecoveredTab = (
  state: WorkspaceEditorState,
  grantRef: string,
  recovered: WorkspaceEditorRecoverySnapshot["tabs"][number],
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => {
  const opening = withWorkspaceEditorOpening(state, recovered.pathRef)
  if (result.state === "available" && result.document.pathRef === recovered.pathRef) {
    const opened = withWorkspaceEditorOpened(opening, recovered.pathRef, result)
    return updateTab(opened, recovered.pathRef, tab => {
      if (result.document.revisionRef === recovered.expectedRevisionRef) {
        return { ...tab, draft: recovered.draft }
      }
      if (result.document.content === recovered.draft) return tab
      return {
        ...tab,
        phase: "conflict",
        draft: recovered.draft,
        externalDocument: result.document,
        reason: "This recovered draft changed on disk while the app was closed.",
      }
    })
  }
  const synthetic: DesktopWorkspaceDocument = {
    grantRef,
    pathRef: recovered.pathRef,
    content: "",
    revisionRef: recovered.expectedRevisionRef,
    languageMode: recoveryLanguageMode(recovered.pathRef),
    encoding: "utf-8",
    lineEnding: "none",
    sizeBytes: 0,
  }
  return updateTab(opening, recovered.pathRef, tab => ({
    ...tab,
    phase: "conflict",
    document: synthetic,
    externalDocument: null,
    draft: recovered.draft,
    saveState: "unavailable",
    reason: result.state === "unavailable"
      ? `${result.message} Your recovered draft remains available for Save As.`
      : "The recovered document could not be matched. Its draft remains available for Save As.",
  }))
}

const WorkspaceEditorOpenPayloadSchema = Schema.Struct({
  grantRef: Schema.String,
  pathRef: DesktopWorkspacePathRefSchema,
})
const WorkspaceEditorRecoveryPayloadSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  snapshot: WorkspaceEditorRecoverySnapshotSchema,
})

export const WorkspaceEditorOpenRequested = defineIntent("WorkspaceEditorOpenRequested", WorkspaceEditorOpenPayloadSchema)
export const WorkspaceEditorTabSelected = defineIntent("WorkspaceEditorTabSelected", DesktopWorkspacePathRefSchema)
export const WorkspaceEditorTabCloseRequested = defineIntent("WorkspaceEditorTabCloseRequested", DesktopWorkspacePathRefSchema)
export const WorkspaceEditorTabCloseConfirmed = defineIntent("WorkspaceEditorTabCloseConfirmed", DesktopWorkspacePathRefSchema)
export const WorkspaceEditorTabCloseCancelled = defineIntent("WorkspaceEditorTabCloseCancelled", Schema.Null)
export const WorkspaceEditorEventReceived = defineIntent("WorkspaceEditorEventReceived", CodeEditorEventSchema)
export const WorkspaceEditorSaveRequested = defineIntent("WorkspaceEditorSaveRequested", Schema.Null)
export const WorkspaceEditorUndoRequested = defineIntent("WorkspaceEditorUndoRequested", Schema.Null)
export const WorkspaceEditorRedoRequested = defineIntent("WorkspaceEditorRedoRequested", Schema.Null)
export const WorkspaceEditorFindChanged = defineIntent("WorkspaceEditorFindChanged", Schema.String)
export const WorkspaceEditorFindNext = defineIntent("WorkspaceEditorFindNext", Schema.Null)
export const WorkspaceEditorFindPrevious = defineIntent("WorkspaceEditorFindPrevious", Schema.Null)
export const WorkspaceEditorConflictReload = defineIntent("WorkspaceEditorConflictReload", Schema.Null)
export const WorkspaceEditorConflictKeepMine = defineIntent("WorkspaceEditorConflictKeepMine", Schema.Null)
export const WorkspaceEditorWordWrapToggled = defineIntent("WorkspaceEditorWordWrapToggled", Schema.Null)
export const WorkspaceEditorMinimapToggled = defineIntent("WorkspaceEditorMinimapToggled", Schema.Null)
export const WorkspaceEditorExternalChangeReceived = defineIntent("WorkspaceEditorExternalChangeReceived", DesktopWorkspaceChangeSchema)
export const WorkspaceEditorSaveAsStarted = defineIntent("WorkspaceEditorSaveAsStarted", Schema.Null)
export const WorkspaceEditorSaveAsChanged = defineIntent("WorkspaceEditorSaveAsChanged", Schema.String)
export const WorkspaceEditorSaveAsSubmitted = defineIntent("WorkspaceEditorSaveAsSubmitted", Schema.Null)
export const WorkspaceEditorSaveAsCancelled = defineIntent("WorkspaceEditorSaveAsCancelled", Schema.Null)
export const WorkspaceEditorRecoveryRequested = defineIntent("WorkspaceEditorRecoveryRequested", WorkspaceEditorRecoveryPayloadSchema)

export const workspaceEditorIntents = [
  WorkspaceEditorOpenRequested,
  WorkspaceEditorTabSelected,
  WorkspaceEditorTabCloseRequested,
  WorkspaceEditorTabCloseConfirmed,
  WorkspaceEditorTabCloseCancelled,
  WorkspaceEditorEventReceived,
  WorkspaceEditorSaveRequested,
  WorkspaceEditorUndoRequested,
  WorkspaceEditorRedoRequested,
  WorkspaceEditorFindChanged,
  WorkspaceEditorFindNext,
  WorkspaceEditorFindPrevious,
  WorkspaceEditorConflictReload,
  WorkspaceEditorConflictKeepMine,
  WorkspaceEditorWordWrapToggled,
  WorkspaceEditorMinimapToggled,
  WorkspaceEditorExternalChangeReceived,
  WorkspaceEditorSaveAsStarted,
  WorkspaceEditorSaveAsChanged,
  WorkspaceEditorSaveAsSubmitted,
  WorkspaceEditorSaveAsCancelled,
  WorkspaceEditorRecoveryRequested,
] as const

export type WorkspaceEditorCapableState = Readonly<{ workspaceEditor: WorkspaceEditorState }>

export type WorkspaceDocumentBridge = Readonly<{
  openWorkspaceDocument: (value: unknown) => Promise<unknown>
  saveWorkspaceDocument: (value: unknown) => Promise<unknown>
  saveWorkspaceDocumentAs: (value: unknown) => Promise<unknown>
}>

export const unavailableWorkspaceDocumentBridge: WorkspaceDocumentBridge = {
  openWorkspaceDocument: async () => ({ state: "unavailable", reason: "unavailable", message: "Workspace documents are unavailable." }),
  saveWorkspaceDocument: async () => ({ state: "unavailable", reason: "unavailable", message: "Workspace document saving is unavailable." }),
  saveWorkspaceDocumentAs: async () => ({ state: "unavailable", reason: "unavailable", message: "Workspace Save As is unavailable." }),
}

const suggestedSaveAsPathRef = (pathRef: string): string => {
  const slash = pathRef.lastIndexOf("/")
  const directory = slash < 0 ? "" : pathRef.slice(0, slash + 1)
  const name = slash < 0 ? pathRef : pathRef.slice(slash + 1)
  const dot = name.lastIndexOf(".")
  return dot <= 0
    ? `${directory}${name}-copy`
    : `${directory}${name.slice(0, dot)}-copy${name.slice(dot)}`
}

export const makeWorkspaceEditorHandlers = <S extends WorkspaceEditorCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: WorkspaceDocumentBridge = unavailableWorkspaceDocumentBridge,
  onStateChange?: (state: S) => void,
) => {
  const setEditor = (mutate: (editor: WorkspaceEditorState) => WorkspaceEditorState) =>
    Effect.gen(function* () {
      yield* SubscriptionRef.update(state, current => ({ ...current, workspaceEditor: mutate(current.workspaceEditor) }))
      if (onStateChange !== undefined) {
        onStateChange(yield* SubscriptionRef.get(state))
      }
    })

  const saveActive = Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const tab = activeTab(current.workspaceEditor)
    if (tab === null || tab.document === null || tab.phase === "loading" || tab.saveState === "saving" || !workspaceEditorTabDirty(tab)) return
    yield* setEditor(editor => updateTab(editor, tab.pathRef, value => ({ ...value, saveState: "saving", reason: null })))
    const raw = yield* Effect.promise(() => bridge.saveWorkspaceDocument({
      grantRef: tab.document!.grantRef,
      pathRef: tab.pathRef,
      content: tab.draft,
      expectedRevisionRef: tab.phase === "conflict" && tab.externalDocument !== null
        ? tab.externalDocument.revisionRef
        : tab.document!.revisionRef,
    }).catch(() => null))
    const result = decodeWorkspaceDocumentResult(raw) ?? {
      state: "unavailable" as const,
      reason: "unavailable" as const,
      message: "The document save response could not be read.",
    }
    yield* setEditor(editor => withWorkspaceEditorSaveResult(editor, tab.pathRef, result))
  })

  const refreshChangedDocuments = (change: DesktopWorkspaceChange) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const targets = current.workspaceEditor.tabs.filter(tab =>
      tab.document !== null && (change.pathRef === null || change.pathRef === tab.pathRef),
    )
    for (const tab of targets) {
      const raw = yield* Effect.promise(() => bridge.openWorkspaceDocument({
        grantRef: tab.document!.grantRef,
        pathRef: tab.pathRef,
      }).catch(() => null))
      const result = decodeWorkspaceDocumentResult(raw) ?? {
        state: "unavailable" as const,
        reason: "unavailable" as const,
        message: "The changed document response could not be read.",
      }
      yield* setEditor(editor => withWorkspaceEditorExternalResult(editor, tab.pathRef, result))
    }
  })

  const saveActiveAs = Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const tab = activeTab(current.workspaceEditor)
    const targetPathRef = current.workspaceEditor.saveAsPathRef?.trim() ?? ""
    if (tab === null || tab.document === null || targetPathRef === "" || tab.saveState === "saving") return
    yield* setEditor(editor => updateTab(editor, tab.pathRef, value => ({ ...value, saveState: "saving", reason: null })))
    const raw = yield* Effect.promise(() => bridge.saveWorkspaceDocumentAs({
      grantRef: tab.document!.grantRef,
      pathRef: targetPathRef,
      content: tab.draft,
    }).catch(() => null))
    const result = decodeWorkspaceDocumentResult(raw) ?? {
      state: "unavailable" as const,
      reason: "unavailable" as const,
      message: "The Save As response could not be read.",
    }
    yield* setEditor(editor => withWorkspaceEditorSaveAsResult(editor, tab.pathRef, targetPathRef, result))
  })

  return {
    WorkspaceEditorOpenRequested: ({ grantRef, pathRef }: { grantRef: string; pathRef: string }) =>
      Effect.gen(function* () {
        const before = yield* SubscriptionRef.get(state)
        if (tabFor(before.workspaceEditor, pathRef) !== null) {
          yield* setEditor(editor => ({ ...editor, activePathRef: pathRef, closeConfirmRef: null }))
          return
        }
        yield* setEditor(editor => withWorkspaceEditorOpening(editor, pathRef))
        const raw = yield* Effect.promise(() => bridge.openWorkspaceDocument({ grantRef, pathRef }).catch(() => null))
        const result = decodeWorkspaceDocumentResult(raw) ?? {
          state: "unavailable" as const,
          reason: "unavailable" as const,
          message: "The document response could not be read.",
        }
        yield* setEditor(editor => withWorkspaceEditorOpened(editor, pathRef, result))
      }),
    WorkspaceEditorTabSelected: (pathRef: string) =>
      setEditor(editor => tabFor(editor, pathRef) === null ? editor : { ...editor, activePathRef: pathRef, closeConfirmRef: null }),
    WorkspaceEditorTabCloseRequested: (pathRef: string) =>
      setEditor(editor => {
        const tab = tabFor(editor, pathRef)
        if (tab === null) return editor
        if (workspaceEditorTabDirty(tab)) return { ...editor, closeConfirmRef: pathRef }
        const tabs = editor.tabs.filter(value => value.pathRef !== pathRef)
        return { ...editor, tabs, activePathRef: editor.activePathRef === pathRef ? tabs.at(-1)?.pathRef ?? null : editor.activePathRef }
      }),
    WorkspaceEditorTabCloseConfirmed: (pathRef: string) =>
      setEditor(editor => {
        if (editor.closeConfirmRef !== pathRef) return editor
        const tabs = editor.tabs.filter(tab => tab.pathRef !== pathRef)
        return { ...editor, tabs, activePathRef: editor.activePathRef === pathRef ? tabs.at(-1)?.pathRef ?? null : editor.activePathRef, closeConfirmRef: null }
      }),
    WorkspaceEditorTabCloseCancelled: () => setEditor(editor => ({ ...editor, closeConfirmRef: null })),
    WorkspaceEditorEventReceived: (event: CodeEditorEvent) => event.type === "save"
      ? Effect.gen(function* () {
          yield* setEditor(editor => withWorkspaceEditorEvent(editor, event))
          yield* saveActive
        })
      : setEditor(editor => withWorkspaceEditorEvent(editor, event)),
    WorkspaceEditorSaveRequested: () => saveActive,
    WorkspaceEditorUndoRequested: () => setEditor(withWorkspaceEditorUndo),
    WorkspaceEditorRedoRequested: () => setEditor(withWorkspaceEditorRedo),
    WorkspaceEditorFindChanged: (query: string) => setEditor(editor => withWorkspaceEditorFind(editor, query)),
    WorkspaceEditorFindNext: () => setEditor(editor => withWorkspaceEditorFindStep(editor, 1)),
    WorkspaceEditorFindPrevious: () => setEditor(editor => withWorkspaceEditorFindStep(editor, -1)),
    WorkspaceEditorConflictReload: () => setEditor(editor => {
      const tab = activeTab(editor)
      if (tab === null || tab.externalDocument === null) return editor
      return updateTab(editor, tab.pathRef, current => ({
        ...current,
        phase: "ready",
        document: current.externalDocument,
        draft: current.externalDocument!.content,
        externalDocument: null,
        selection: { start: 0, end: 0 },
        selectionVersion: nextSelectionVersion(current.selectionVersion),
        undo: [],
        redo: [],
        saveState: "idle",
        reason: null,
      }))
    }),
    WorkspaceEditorConflictKeepMine: () => saveActive,
    WorkspaceEditorWordWrapToggled: () => setEditor(editor => ({ ...editor, wordWrap: !editor.wordWrap })),
    WorkspaceEditorMinimapToggled: () => setEditor(editor => ({ ...editor, minimap: !editor.minimap })),
    WorkspaceEditorExternalChangeReceived: refreshChangedDocuments,
    WorkspaceEditorSaveAsStarted: () => setEditor(editor => {
      const tab = activeTab(editor)
      return tab === null || tab.document === null
        ? editor
        : { ...editor, saveAsPathRef: suggestedSaveAsPathRef(tab.pathRef) }
    }),
    WorkspaceEditorSaveAsChanged: (pathRef: string) => setEditor(editor => ({
      ...editor,
      saveAsPathRef: editor.saveAsPathRef === null ? null : pathRef.slice(0, 1_024),
    })),
    WorkspaceEditorSaveAsSubmitted: () => saveActiveAs,
    WorkspaceEditorSaveAsCancelled: () => setEditor(editor => ({ ...editor, saveAsPathRef: null })),
    WorkspaceEditorRecoveryRequested: ({ grantRef, snapshot }: { grantRef: string; snapshot: WorkspaceEditorRecoverySnapshot }) =>
      Effect.gen(function* () {
        const before = yield* SubscriptionRef.get(state)
        let recovered: WorkspaceEditorState = {
          ...emptyWorkspaceEditorState(),
          wordWrap: before.workspaceEditor.wordWrap,
          minimap: before.workspaceEditor.minimap,
        }
        for (const tab of snapshot.tabs) {
          const raw = yield* Effect.promise(() => bridge.openWorkspaceDocument({
            grantRef,
            pathRef: tab.pathRef,
          }).catch(() => null))
          const result = decodeWorkspaceDocumentResult(raw) ?? {
            state: "unavailable" as const,
            reason: "unavailable" as const,
            message: "The recovered document response could not be read.",
          }
          recovered = withWorkspaceEditorRecoveredTab(recovered, grantRef, tab, result)
        }
        const activePathRef = snapshot.activePathRef !== null && tabFor(recovered, snapshot.activePathRef) !== null
          ? snapshot.activePathRef
          : recovered.tabs.at(-1)?.pathRef ?? null
        yield* setEditor(() => ({ ...recovered, activePathRef }))
      }),
  }
}

const tabLabel = (tab: WorkspaceEditorTab): string => {
  const name = tab.pathRef.split("/").at(-1) ?? tab.pathRef
  return workspaceEditorTabDirty(tab) ? `${name} •` : name
}

export const workspaceEditorView = (
  state: WorkspaceEditorState,
  options: Readonly<{ attachToChat?: ReturnType<typeof IntentRef> }> = {},
): View => {
  const tab = activeTab(state)
  if (tab === null) {
    return EmptyMessage({
      key: "workspace-editor-empty",
      icon: { name: "Code", tone: "secondary" },
      title: "No document open",
      description: "Select a text file from the workspace tree to open it in a grant-scoped editor tab.",
      style: { flex: 1, minHeight: 0 },
    })
  }
  const closing = state.closeConfirmRef === tab.pathRef
  return Stack({ key: "workspace-editor", direction: "column", gap: "2", style: { flex: 1, minWidth: 0, minHeight: 0, width: "full" } }, [
    Stack({ key: "workspace-editor-tabs", direction: "row", gap: "1", align: "center", a11y: { role: "tablist", label: "Open documents" }, style: { width: "full" } }, [
      ...state.tabs.map(item => Button({
        key: `workspace-editor-tab-${item.pathRef}`,
        label: tabLabel(item),
        variant: "ghost",
        selected: item.pathRef === state.activePathRef,
        onPress: IntentRef("WorkspaceEditorTabSelected", StaticPayload(item.pathRef)),
        a11y: { role: "tab", label: `${item.pathRef}${workspaceEditorTabDirty(item) ? ", unsaved changes" : ""}`, selected: item.pathRef === state.activePathRef },
      })),
      Spacer({ key: "workspace-editor-tabs-fill", flex: true }),
      Button({ key: "workspace-editor-close", label: closing ? "Discard changes" : "Close", variant: closing ? "primary" : "ghost", onPress: IntentRef(closing ? "WorkspaceEditorTabCloseConfirmed" : "WorkspaceEditorTabCloseRequested", StaticPayload(tab.pathRef)), a11y: { label: closing ? `Discard changes and close ${tab.pathRef}` : `Close ${tab.pathRef}` } }),
      ...(closing ? [Button({ key: "workspace-editor-close-cancel", label: "Keep editing", variant: "secondary", onPress: IntentRef("WorkspaceEditorTabCloseCancelled"), a11y: { label: `Keep editing ${tab.pathRef}` } })] : []),
    ]),
    Stack({ key: "workspace-editor-toolbar", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      Text({ key: "workspace-editor-path", content: tab.pathRef, variant: "caption", color: "textMuted" }),
      ...(tab.document === null ? [] : [
        Badge({ key: "workspace-editor-language", label: tab.document.languageMode, tone: "neutral" }),
        Badge({ key: "workspace-editor-encoding", label: tab.document.encoding, tone: "neutral" }),
      ]),
      Spacer({ key: "workspace-editor-toolbar-fill", flex: true }),
      Button({ key: "workspace-editor-undo", label: "Undo", variant: "ghost", disabled: tab.undo.length === 0, onPress: IntentRef("WorkspaceEditorUndoRequested") }),
      Button({ key: "workspace-editor-redo", label: "Redo", variant: "ghost", disabled: tab.redo.length === 0, onPress: IntentRef("WorkspaceEditorRedoRequested") }),
      Button({ key: "workspace-editor-wrap", label: "Wrap", variant: "ghost", selected: state.wordWrap, onPress: IntentRef("WorkspaceEditorWordWrapToggled"), a11y: { selected: state.wordWrap } }),
      Button({ key: "workspace-editor-minimap", label: "Minimap", variant: "ghost", selected: state.minimap, onPress: IntentRef("WorkspaceEditorMinimapToggled"), a11y: { selected: state.minimap } }),
      ...(options.attachToChat === undefined ? [] : [Button({
        key: "workspace-editor-attach-chat",
        label: "Mention in chat",
        variant: "secondary",
        disabled: tab.document === null || tab.phase !== "ready" || tab.draft.length > 200_000,
        onPress: options.attachToChat,
        a11y: {
          label: tab.draft.length > 200_000
            ? `Cannot mention ${tab.pathRef}; file exceeds the 200,000 character context limit`
            : `Mention ${tab.pathRef} in the next chat turn`,
        },
      })]),
      Button({ key: "workspace-editor-save", label: tab.saveState === "saved" ? "Saved" : "Save", variant: "primary", loading: tab.saveState === "saving", disabled: !workspaceEditorTabDirty(tab) || tab.saveState === "saving" || tab.phase === "unavailable", onPress: IntentRef("WorkspaceEditorSaveRequested"), a11y: { label: `Save ${tab.pathRef}` } }),
      Button({ key: "workspace-editor-save-as", label: "Save As", variant: "secondary", disabled: tab.saveState === "saving" || tab.phase === "unavailable", onPress: IntentRef("WorkspaceEditorSaveAsStarted"), a11y: { label: `Save ${tab.pathRef} as a new file` } }),
    ]),
    ...(state.saveAsPathRef === null ? [] : [Stack({ key: "workspace-editor-save-as-row", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      TextField({ key: "workspace-editor-save-as-path", value: state.saveAsPathRef, label: "New relative path", placeholder: "src/new-file.ts", onChange: IntentRef("WorkspaceEditorSaveAsChanged", ComponentValueBinding()), onSubmit: IntentRef("WorkspaceEditorSaveAsSubmitted"), a11y: { label: "New relative document path" }, style: { flex: 1, minWidth: 0 } }),
      Button({ key: "workspace-editor-save-as-submit", label: "Create copy", variant: "primary", disabled: state.saveAsPathRef.trim() === "", onPress: IntentRef("WorkspaceEditorSaveAsSubmitted") }),
      Button({ key: "workspace-editor-save-as-cancel", label: "Cancel", variant: "ghost", onPress: IntentRef("WorkspaceEditorSaveAsCancelled") }),
    ])]),
    Stack({ key: "workspace-editor-find", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      TextField({ key: "workspace-editor-find-query", value: tab.findQuery, placeholder: "Find in document", onChange: IntentRef("WorkspaceEditorFindChanged", ComponentValueBinding()), a11y: { label: `Find in ${tab.pathRef}` }, style: { flex: 1, minWidth: 0 } }),
      Text({ key: "workspace-editor-find-count", content: tab.findMatches.length === 0 ? "No matches" : `${tab.findIndex + 1} of ${tab.findMatches.length}`, variant: "caption", color: "textMuted" }),
      Button({ key: "workspace-editor-find-previous", label: "Previous", variant: "ghost", disabled: tab.findMatches.length === 0, onPress: IntentRef("WorkspaceEditorFindPrevious") }),
      Button({ key: "workspace-editor-find-next", label: "Next", variant: "ghost", disabled: tab.findMatches.length === 0, onPress: IntentRef("WorkspaceEditorFindNext") }),
    ]),
    ...(tab.phase === "loading" ? [ShimmerText({ key: "workspace-editor-loading", text: "Opening document…", typeScale: "body", style: { color: "textMuted" } })]
      : tab.phase === "unavailable" ? [Text({ key: "workspace-editor-unavailable", content: tab.reason ?? "This document is unavailable.", variant: "body", color: "warning" })]
      : [
          ...(tab.phase === "conflict" ? [Stack({ key: "workspace-editor-conflict", direction: "row", gap: "2", align: "center" }, [
            Text({
              key: "workspace-editor-conflict-copy",
              content: tab.externalDocument === null
                ? "The file is no longer available. Your draft remains open and is included in recovery data."
                : "Changed outside the editor. Reload their version or explicitly save yours over the new revision.",
              variant: "body",
              color: "warning",
            }),
            ...(tab.externalDocument === null ? [] : [
              Button({ key: "workspace-editor-conflict-reload", label: "Reload theirs", variant: "secondary", onPress: IntentRef("WorkspaceEditorConflictReload") }),
              Button({ key: "workspace-editor-conflict-keep", label: "Save mine", variant: "primary", onPress: IntentRef("WorkspaceEditorConflictKeepMine") }),
            ]),
          ])] : []),
          ...(tab.reason === null ? [] : [Text({ key: "workspace-editor-notice", content: tab.reason, variant: "caption", color: "warning" })]),
          Divider({ key: "workspace-editor-divider" }),
          CodeEditor({
            key: `workspace-editor-host-${tab.pathRef}`,
            value: tab.draft,
            language: tab.document?.languageMode ?? "plaintext",
            readOnly: tab.saveState === "saving",
            wordWrap: state.wordWrap,
            minimap: state.minimap,
            selection: { ...tab.selection, version: tab.selectionVersion },
            fontScale: "body",
            onEvent: IntentRef("WorkspaceEditorEventReceived", ComponentValueBinding()),
            a11y: { role: "region", label: `Editor for ${tab.pathRef}` },
            style: { flex: 1, minHeight: 0, width: "full" },
          }),
          Text({ key: "workspace-editor-selection", content: `Selection ${tab.selection.start}–${tab.selection.end}`, variant: "caption", color: "textMuted" }),
        ]),
  ])
}

// Keep the imported schema identity source-coupled in this module's public
// contract; the decoder remains the single boundary authority.
export const WorkspaceEditorDocumentResultSchema = DesktopWorkspaceDocumentResultSchema
