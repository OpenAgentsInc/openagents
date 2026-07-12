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
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  defineIntent,
  type CodeEditorEvent,
  type View,
} from "@effect-native/core"
import { Effect, Schema, SubscriptionRef } from "@effect-native/core/effect"

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
}>

export const emptyWorkspaceEditorState = (): WorkspaceEditorState => ({
  tabs: [],
  activePathRef: null,
  closeConfirmRef: null,
  wordWrap: false,
  minimap: false,
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
  version: 1
  activePathRef: string | null
  tabs: ReadonlyArray<Readonly<{
    pathRef: string
    grantRef: string
    expectedRevisionRef: string
    draft: string
  }>>
}>

export const workspaceEditorRecoverySnapshot = (
  state: WorkspaceEditorState,
): WorkspaceEditorRecoverySnapshot => ({
  version: 1,
  activePathRef: state.activePathRef,
  tabs: state.tabs.flatMap(tab => tab.document === null ? [] : [{
    pathRef: tab.pathRef,
    grantRef: tab.document.grantRef,
    expectedRevisionRef: tab.document.revisionRef,
    draft: tab.draft.slice(0, 1_000_000),
  }]).slice(0, maxTabs),
})

const WorkspaceEditorOpenPayloadSchema = Schema.Struct({
  grantRef: Schema.String,
  pathRef: DesktopWorkspacePathRefSchema,
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
] as const

export type WorkspaceEditorCapableState = Readonly<{ workspaceEditor: WorkspaceEditorState }>

export type WorkspaceDocumentBridge = Readonly<{
  openWorkspaceDocument: (value: unknown) => Promise<unknown>
  saveWorkspaceDocument: (value: unknown) => Promise<unknown>
}>

export const unavailableWorkspaceDocumentBridge: WorkspaceDocumentBridge = {
  openWorkspaceDocument: async () => ({ state: "unavailable", reason: "unavailable", message: "Workspace documents are unavailable." }),
  saveWorkspaceDocument: async () => ({ state: "unavailable", reason: "unavailable", message: "Workspace document saving is unavailable." }),
}

export const makeWorkspaceEditorHandlers = <S extends WorkspaceEditorCapableState>(
  state: SubscriptionRef.SubscriptionRef<S>,
  bridge: WorkspaceDocumentBridge = unavailableWorkspaceDocumentBridge,
) => {
  const setEditor = (mutate: (editor: WorkspaceEditorState) => WorkspaceEditorState) =>
    SubscriptionRef.update(state, current => ({ ...current, workspaceEditor: mutate(current.workspaceEditor) }))

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
  }
}

const tabLabel = (tab: WorkspaceEditorTab): string => {
  const name = tab.pathRef.split("/").at(-1) ?? tab.pathRef
  return workspaceEditorTabDirty(tab) ? `${name} •` : name
}

export const workspaceEditorView = (state: WorkspaceEditorState): View => {
  const tab = activeTab(state)
  if (tab === null) {
    return Stack({ key: "workspace-editor-empty", direction: "column", gap: "2", style: { flex: 1, minHeight: 0 } }, [
      Text({ key: "workspace-editor-empty-title", content: "No document open", variant: "title", color: "textPrimary" }),
      Text({ key: "workspace-editor-empty-copy", content: "Select a text file from the workspace tree to open it in a grant-scoped editor tab.", variant: "body", color: "textMuted" }),
    ])
  }
  const closing = state.closeConfirmRef === tab.pathRef
  return Stack({ key: "workspace-editor", direction: "column", gap: "2", style: { flex: 1, minWidth: 0, minHeight: 0, width: "full" } }, [
    Stack({ key: "workspace-editor-tabs", direction: "row", gap: "1", align: "center", a11y: { role: "tablist", label: "Open documents" }, style: { width: "full" } }, [
      ...state.tabs.map(item => Button({
        key: `workspace-editor-tab-${item.pathRef}`,
        label: tabLabel(item),
        variant: item.pathRef === state.activePathRef ? "secondary" : "ghost",
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
      Button({ key: "workspace-editor-wrap", label: "Wrap", variant: state.wordWrap ? "secondary" : "ghost", onPress: IntentRef("WorkspaceEditorWordWrapToggled"), a11y: { selected: state.wordWrap } }),
      Button({ key: "workspace-editor-minimap", label: "Minimap", variant: state.minimap ? "secondary" : "ghost", onPress: IntentRef("WorkspaceEditorMinimapToggled"), a11y: { selected: state.minimap } }),
      Button({ key: "workspace-editor-save", label: tab.saveState === "saving" ? "Saving…" : tab.saveState === "saved" ? "Saved" : "Save", variant: "primary", disabled: !workspaceEditorTabDirty(tab) || tab.saveState === "saving" || tab.phase === "unavailable", onPress: IntentRef("WorkspaceEditorSaveRequested"), a11y: { label: `Save ${tab.pathRef}` } }),
    ]),
    Stack({ key: "workspace-editor-find", direction: "row", gap: "2", align: "center", style: { width: "full" } }, [
      TextField({ key: "workspace-editor-find-query", value: tab.findQuery, placeholder: "Find in document", onChange: IntentRef("WorkspaceEditorFindChanged", ComponentValueBinding()), a11y: { label: `Find in ${tab.pathRef}` }, style: { flex: 1, minWidth: 0 } }),
      Text({ key: "workspace-editor-find-count", content: tab.findMatches.length === 0 ? "No matches" : `${tab.findIndex + 1} of ${tab.findMatches.length}`, variant: "caption", color: "textMuted" }),
      Button({ key: "workspace-editor-find-previous", label: "Previous", variant: "ghost", disabled: tab.findMatches.length === 0, onPress: IntentRef("WorkspaceEditorFindPrevious") }),
      Button({ key: "workspace-editor-find-next", label: "Next", variant: "ghost", disabled: tab.findMatches.length === 0, onPress: IntentRef("WorkspaceEditorFindNext") }),
    ]),
    ...(tab.phase === "loading" ? [Text({ key: "workspace-editor-loading", content: "Opening document…", variant: "body", color: "textMuted" })]
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
            onEvent: IntentRef("WorkspaceEditorEventReceived"),
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
