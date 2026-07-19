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
  DesktopWorkspaceDocumentSchema,
  DesktopWorkspaceDocumentResultSchema,
  DesktopWorkspacePathRefSchema,
  decodeWorkspaceDocumentResult,
  workspaceChangePathRefs,
  type DesktopWorkspaceChange,
  type DesktopWorkspaceDocument,
  type DesktopWorkspaceDocumentResult,
} from "../workspace-contract.ts"
import {
  IdeDocumentGeneration,
  IdeDocumentRef,
  IdeDocumentSequence,
  IdeEditorSelectionSchema,
  IdeMonacoModelVersion,
  IdeMonacoDocumentEventSchema,
  makeIdeDocumentRef,
  type IdeMonacoDocumentEvent,
} from "../ide/monaco-document-contract.ts"
import { IdeEditorGroupRefSchema } from "../ide/project-contract.ts"
import { IdePathIndexIdentitySchema, type IdePathIndexIdentity } from "../ide/path-index-contract.ts"
import {
  IdeNavigationEntryRefSchema,
  IdeEditorSettingIdSchema,
  IdeEditorSettingOverrideSchema,
  IdeWorkbenchStateSchema,
  breadcrumbsForPath,
  emptyIdeWorkbenchState,
  importIdeEditorSettings,
  markIdeNavigationUnavailable,
  pushIdeNavigation,
  rankIdeQuickOpen,
  resetIdeEditorSetting,
  resolveIdeEditorSetting,
  setIdeEditorSetting,
  stepIdeNavigation,
  type IdeEditorSettingId,
  type IdeEditorSettingOverride,
  type IdeWorkbenchState,
} from "../ide/workbench-contract.ts"

const maxTabs = 12
const maxHistory = 100
const maxFindMatches = 1_000

const EditorCountSchema = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))
const EditorContentSchema = Schema.String.check(Schema.isMaxLength(1_000_000))

export const WorkspaceEditorTabSchema = Schema.Struct({
  /** Required on production opens; optional only for retained fixture compatibility. */
  documentRef: Schema.optional(IdeDocumentRef),
  generation: Schema.optional(IdeDocumentGeneration),
  pathRef: DesktopWorkspacePathRefSchema,
  phase: Schema.Literals(["loading", "ready", "unavailable", "conflict"]),
  document: Schema.NullOr(DesktopWorkspaceDocumentSchema),
  externalDocument: Schema.NullOr(DesktopWorkspaceDocumentSchema),
  draft: EditorContentSchema,
  selection: IdeEditorSelectionSchema,
  selectionVersion: EditorCountSchema,
  undo: Schema.Array(EditorContentSchema).check(Schema.isMaxLength(maxHistory)),
  redo: Schema.Array(EditorContentSchema).check(Schema.isMaxLength(maxHistory)),
  saveState: Schema.Literals(["idle", "saving", "saved", "unavailable"]),
  reason: Schema.NullOr(Schema.String.check(Schema.isMaxLength(400))),
  findQuery: Schema.String.check(Schema.isMaxLength(200)),
  findMatches: Schema.Array(EditorCountSchema).check(Schema.isMaxLength(maxFindMatches)),
  findIndex: EditorCountSchema,
  incrementalSequence: Schema.optional(IdeDocumentSequence),
  modelVersion: Schema.optional(IdeMonacoModelVersion),
  gapRecoveries: Schema.optional(EditorCountSchema),
  tabMode: Schema.optional(Schema.Literals(["preview", "pinned"])),
}).annotate({ identifier: "WorkspaceEditorTab" })
export type WorkspaceEditorTab = typeof WorkspaceEditorTabSchema.Type

export const WorkspaceEditorStateSchema = Schema.Struct({
  tabs: Schema.Array(WorkspaceEditorTabSchema).check(Schema.isMaxLength(maxTabs)),
  activePathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  closeConfirmRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  wordWrap: Schema.Boolean,
  minimap: Schema.Boolean,
  split: Schema.Boolean,
  vimEnabled: Schema.Boolean,
  nextDocumentOrdinal: EditorCountSchema,
  saveAsPathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  closedPathRefs: Schema.Array(DesktopWorkspacePathRefSchema).check(Schema.isMaxLength(20)),
  workbench: IdeWorkbenchStateSchema,
}).annotate({ identifier: "WorkspaceEditorState" })
export type WorkspaceEditorState = typeof WorkspaceEditorStateSchema.Type

export const emptyWorkspaceEditorState = (
  options: Readonly<{ vimEnabled?: boolean }> = {},
): WorkspaceEditorState => ({
  tabs: [],
  activePathRef: null,
  closeConfirmRef: null,
  wordWrap: false,
  minimap: false,
  split: false,
  vimEnabled: options.vimEnabled ?? false,
  nextDocumentOrdinal: 0,
  saveAsPathRef: null,
  closedPathRefs: [],
  workbench: emptyIdeWorkbenchState(),
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

const emptyTab = (
  pathRef: string,
  documentRef: IdeDocumentRef,
  generation: IdeDocumentGeneration,
): WorkspaceEditorTab => ({
  documentRef,
  generation,
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
  incrementalSequence: IdeDocumentSequence.make(0),
  modelVersion: IdeMonacoModelVersion.make(1),
  gapRecoveries: 0,
  tabMode: "pinned",
})

const syncWorkbenchGroups = (
  workbench: IdeWorkbenchState,
  tabs: ReadonlyArray<WorkspaceEditorTab>,
  activePathRef: string | null,
  split: boolean,
): IdeWorkbenchState => {
  const documentRefs = tabs.flatMap(tab => tab.documentRef === undefined ? [] : [tab.documentRef])
  const activeDocumentRef = tabs.find(tab => tab.pathRef === activePathRef)?.documentRef ?? null
  const primary = {
    groupRef: workbench.groups[0]?.groupRef ?? workbench.focusedGroupRef,
    documentRefs,
    activeDocumentRef,
    direction: "primary" as const,
    viewStates: documentRefs.map(documentRef => workbench.groups[0]?.viewStates.find(view => view.documentRef === documentRef) ?? {
      documentRef,
      selection: tabs.find(tab => tab.documentRef === documentRef)?.selection ?? { start: 0, end: 0 },
      scrollTop: 0,
      foldedLineStarts: [],
    }),
  }
  const groups = split
    ? [primary, {
        groupRef: workbench.groups[1]?.groupRef ?? IdeEditorGroupRefSchema.make("ide.editor-group.secondary"),
        documentRefs: activeDocumentRef === null ? [] : [activeDocumentRef],
        activeDocumentRef,
        direction: "right" as const,
        viewStates: activeDocumentRef === null ? [] : [workbench.groups[1]?.viewStates.find(view => view.documentRef === activeDocumentRef) ?? {
          documentRef: activeDocumentRef,
          selection: tabs.find(tab => tab.documentRef === activeDocumentRef)?.selection ?? { start: 0, end: 0 },
          scrollTop: 0,
          foldedLineStarts: [],
        }],
      }]
    : [primary]
  return IdeWorkbenchStateSchema.make({
    ...workbench,
    groups,
    focusedGroupRef: groups.some(group => group.groupRef === workbench.focusedGroupRef)
      ? workbench.focusedGroupRef
      : primary.groupRef,
    breadcrumbs: activePathRef === null ? [] : breadcrumbsForPath(activePathRef),
  })
}

const withSynchronizedWorkbench = (state: WorkspaceEditorState): WorkspaceEditorState => ({
  ...state,
  workbench: syncWorkbenchGroups(state.workbench, state.tabs, state.activePathRef, state.split),
})

const withWorkspaceEditorNavigation = (
  state: WorkspaceEditorState,
  pathRef: string,
  source: "explorer" | "quick_open" | "workspace_search" | "recent_restore",
  identity: IdePathIndexIdentity | undefined,
): WorkspaceEditorState => {
  const tab = tabFor(state, pathRef)
  if (tab?.documentRef === undefined || tab.generation === undefined || identity === undefined) return state
  const ordinal = state.workbench.navigation.entries.length
  return {
    ...state,
    workbench: {
      ...state.workbench,
      navigation: pushIdeNavigation(state.workbench.navigation, {
        entryRef: IdeNavigationEntryRefSchema.make(`ide.navigation.${ordinal}.${tab.generation}`),
        source,
        projectRef: identity.projectRef,
        rootRef: identity.rootRef,
        worktreeRef: identity.worktreeRef,
        documentRef: tab.documentRef,
        generation: tab.generation,
        pathRef: tab.pathRef,
        selection: tab.selection,
        state: "ready",
        reason: null,
      }),
    },
  }
}

const stepEditorNavigation = (
  editor: WorkspaceEditorState,
  direction: "back" | "forward",
): WorkspaceEditorState => {
  const stepped = stepIdeNavigation(editor.workbench.navigation, direction)
  if (stepped.entry === null) return editor
  const target = editor.tabs.find(tab =>
    tab.documentRef === stepped.entry!.documentRef && tab.generation === stepped.entry!.generation)
  if (target === undefined) return {
    ...editor,
    workbench: {
      ...editor.workbench,
      navigation: markIdeNavigationUnavailable(
        stepped.history,
        stepped.entry.entryRef,
        "The exact document generation is no longer open.",
      ),
    },
  }
  return withSynchronizedWorkbench({
    ...editor,
    activePathRef: target.pathRef,
    workbench: { ...editor.workbench, navigation: stepped.history },
  })
}

export const withWorkspaceEditorOpening = (
  state: WorkspaceEditorState,
  pathRef: string,
  grantGenerationRef = "compatibility",
  recoveredIdentity?: Readonly<{ documentRef: IdeDocumentRef; generation: IdeDocumentGeneration }>,
): WorkspaceEditorState => {
  const existing = tabFor(state, pathRef)
  if (existing !== null) return withSynchronizedWorkbench({ ...state, activePathRef: pathRef, closeConfirmRef: null })
  if (state.tabs.length >= maxTabs) return state
  const replaceablePreview = state.tabs.find(tab => (tab.tabMode ?? "pinned") === "preview" && !workspaceEditorTabDirty(tab))
  const retainedTabs = replaceablePreview === undefined
    ? state.tabs
    : state.tabs.filter(tab => tab.documentRef !== replaceablePreview.documentRef)
  return withSynchronizedWorkbench({
    ...state,
    tabs: [...retainedTabs, emptyTab(
      pathRef,
      recoveredIdentity?.documentRef ?? makeIdeDocumentRef(grantGenerationRef, state.nextDocumentOrdinal),
      recoveredIdentity?.generation ?? IdeDocumentGeneration.make(state.nextDocumentOrdinal),
    )],
    activePathRef: pathRef,
    closeConfirmRef: null,
    nextDocumentOrdinal: state.nextDocumentOrdinal + 1,
  })
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
  return withSynchronizedWorkbench({
    ...state,
    tabs: renamed,
    activePathRef: state.activePathRef === null ? null : remap(state.activePathRef) ?? state.activePathRef,
    closeConfirmRef: state.closeConfirmRef === null ? null : remap(state.closeConfirmRef) ?? state.closeConfirmRef,
    saveAsPathRef: null,
  })
}

export const withWorkspaceEditorTabMode = (
  state: WorkspaceEditorState,
  pathRef: string,
  tabMode: "preview" | "pinned",
): WorkspaceEditorState => updateTab(state, pathRef, tab => ({ ...tab, tabMode }))

export const withWorkspaceEditorTabMoved = (
  state: WorkspaceEditorState,
  pathRef: string,
  delta: -1 | 1,
): WorkspaceEditorState => {
  const index = state.tabs.findIndex(tab => tab.pathRef === pathRef)
  const target = index + delta
  if (index < 0 || target < 0 || target >= state.tabs.length) return state
  const tabs = [...state.tabs]
  const [tab] = tabs.splice(index, 1)
  tabs.splice(target, 0, tab!)
  return withSynchronizedWorkbench({ ...state, tabs })
}

const closeEditorTabs = (
  state: WorkspaceEditorState,
  pathRefs: ReadonlyArray<string>,
): WorkspaceEditorState => {
  const requested = new Set(pathRefs)
  const blocked = state.tabs.filter(tab => requested.has(tab.pathRef) && workspaceEditorTabDirty(tab))
  const closable = state.tabs.filter(tab => requested.has(tab.pathRef) && !workspaceEditorTabDirty(tab))
  const closed = new Set(closable.map(tab => tab.pathRef))
  const tabs = state.tabs.map(tab => blocked.some(candidate => candidate.pathRef === tab.pathRef)
    ? { ...tab, reason: "Close refused until the dirty document is saved or explicitly discarded." }
    : tab).filter(tab => !closed.has(tab.pathRef))
  const activePathRef = tabs.some(tab => tab.pathRef === state.activePathRef)
    ? state.activePathRef
    : tabs.at(-1)?.pathRef ?? null
  return withSynchronizedWorkbench({
    ...state,
    tabs,
    activePathRef,
    closeConfirmRef: blocked[0]?.pathRef ?? null,
    closedPathRefs: [...state.closedPathRefs, ...closable.map(tab => tab.pathRef)].slice(-20),
  })
}

export const withWorkspaceEditorTabsClosed = (
  state: WorkspaceEditorState,
  operation: "active" | "others" | "right" | "all",
): WorkspaceEditorState => {
  const activeIndex = state.tabs.findIndex(tab => tab.pathRef === state.activePathRef)
  if (activeIndex < 0) return state
  const pathRefs = operation === "active" ? [state.tabs[activeIndex]!.pathRef]
    : operation === "others" ? state.tabs.filter((_, index) => index !== activeIndex).map(tab => tab.pathRef)
    : operation === "right" ? state.tabs.slice(activeIndex + 1).map(tab => tab.pathRef)
    : state.tabs.map(tab => tab.pathRef)
  return closeEditorTabs(state, pathRefs)
}

export const withWorkspaceEditorQuickOpen = (
  state: WorkspaceEditorState,
  query: string,
  paths: ReadonlyArray<string>,
): WorkspaceEditorState => ({
  ...state,
  workbench: {
    ...state.workbench,
    quickOpen: rankIdeQuickOpen(query, paths, 50),
  },
})

export const withWorkspaceEditorSetting = (
  state: WorkspaceEditorState,
  override: IdeEditorSettingOverride,
): WorkspaceEditorState => withWorkspaceEditorSettingsState(
  state,
  setIdeEditorSetting(state.workbench.settings, override),
)

const withWorkspaceEditorSettingsState = (
  state: WorkspaceEditorState,
  settings: IdeWorkbenchState["settings"],
): WorkspaceEditorState => {
  const boolean = (id: IdeEditorSettingId): boolean => {
    const value = resolveIdeEditorSetting(settings, id).value
    return value._tag === "Boolean" && value.value
  }
  return withSynchronizedWorkbench({
    ...state,
    wordWrap: boolean("editor.wordWrap"),
    minimap: boolean("editor.minimap.enabled"),
    vimEnabled: boolean("editor.vim.enabled"),
    workbench: { ...state.workbench, settings },
  })
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
    tabMode: "pinned",
    findMatches: findOffsets(value, current.findQuery),
    findIndex: 0,
  }))
}

/**
 * Applies only events fenced to the active opaque document generation.
 * Monaco supplies mechanics and a complete edit snapshot; the Effect-owned
 * state remains the canonical dirty/recovery copy. A sequence gap is accepted
 * only as an explicit full-value resync and is counted for diagnostics.
 */
export const withWorkspaceEditorMonacoEvent = (
  state: WorkspaceEditorState,
  event: IdeMonacoDocumentEvent,
): WorkspaceEditorState => {
  const tab = activeTab(state)
  if (tab === null || tab.documentRef === undefined || tab.generation === undefined || tab.documentRef !== event.documentRef || tab.generation !== event.generation) return state
  if (event._tag === "Save" || event._tag === "Close") return state
  if (event._tag === "Selection") {
    const maximum = tab.draft.length
    const start = Math.max(0, Math.min(maximum, Math.trunc(event.selection.start)))
    const end = Math.max(start, Math.min(maximum, Math.trunc(event.selection.end)))
    if (start === tab.selection.start && end === tab.selection.end) return state
    return updateTab(state, tab.pathRef, current => ({ ...current, selection: { start, end } }))
  }
  const sequence = event.sequence as number
  const currentSequence = (tab.incrementalSequence ?? 0) as number
  if (sequence <= currentSequence) return state
  const value = event.value.slice(0, 1_000_000)
  const gap = sequence !== currentSequence + 1
  const undo = value === tab.draft ? tab.undo : [...tab.undo, tab.draft].slice(-maxHistory)
  return updateTab(state, tab.pathRef, current => ({
    ...current,
    draft: value,
    undo,
    redo: value === current.draft ? current.redo : [],
    incrementalSequence: IdeDocumentSequence.make(sequence),
    modelVersion: event.modelVersion,
    gapRecoveries: (current.gapRecoveries ?? 0) + (gap ? 1 : 0),
    saveState: "idle",
    reason: gap ? `Editor sequence gap recovered from the complete model snapshot at sequence ${sequence}.` : null,
    tabMode: value === current.draft ? current.tabMode : "pinned",
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

const WorkspaceEditorRecoverySnapshotV2Schema = Schema.Struct({
  version: Schema.Literal(2),
  activePathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  tabs: Schema.Array(Schema.Struct({
    pathRef: DesktopWorkspacePathRefSchema,
    expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
    draft: Schema.String.check(Schema.isMaxLength(1_000_000)),
  })).check(Schema.isMaxLength(maxTabs)),
})

const WorkspaceEditorRecoverySnapshotV3Schema = Schema.Struct({
  version: Schema.Literal(3),
  activePathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  tabs: Schema.Array(Schema.Struct({
    documentRef: IdeDocumentRef,
    generation: IdeDocumentGeneration,
    incrementalSequence: IdeDocumentSequence,
    selection: IdeEditorSelectionSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
    draft: Schema.String.check(Schema.isMaxLength(1_000_000)),
  })).check(Schema.isMaxLength(maxTabs)),
})

export const WorkspaceEditorRecoverySnapshotSchema = Schema.Struct({
  version: Schema.Literal(4),
  activePathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  split: Schema.Boolean,
  closedPathRefs: Schema.Array(DesktopWorkspacePathRefSchema).check(Schema.isMaxLength(20)),
  workbench: IdeWorkbenchStateSchema,
  tabs: Schema.Array(Schema.Struct({
    documentRef: IdeDocumentRef,
    generation: IdeDocumentGeneration,
    incrementalSequence: IdeDocumentSequence,
    selection: IdeEditorSelectionSchema,
    pathRef: DesktopWorkspacePathRefSchema,
    expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
    draft: Schema.String.check(Schema.isMaxLength(1_000_000)),
    tabMode: Schema.Literals(["preview", "pinned"]),
  })).check(Schema.isMaxLength(maxTabs)),
})
export type WorkspaceEditorRecoverySnapshot = typeof WorkspaceEditorRecoverySnapshotSchema.Type

export const decodeWorkspaceEditorRecoverySnapshot = (value: unknown): WorkspaceEditorRecoverySnapshot | null => {
  const decoded = Schema.decodeUnknownExit(WorkspaceEditorRecoverySnapshotSchema)(value)
  if (Exit.isSuccess(decoded)) return decoded.value
  const v3 = Schema.decodeUnknownExit(WorkspaceEditorRecoverySnapshotV3Schema)(value)
  if (Exit.isSuccess(v3)) return WorkspaceEditorRecoverySnapshotSchema.make({
    version: 4,
    activePathRef: v3.value.activePathRef,
    split: false,
    closedPathRefs: [],
    workbench: emptyIdeWorkbenchState(),
    tabs: v3.value.tabs.map(tab => ({ ...tab, tabMode: "pinned" as const })),
  })
  const legacy = Schema.decodeUnknownExit(WorkspaceEditorRecoverySnapshotV2Schema)(value)
  if (Exit.isFailure(legacy)) return null
  return WorkspaceEditorRecoverySnapshotSchema.make({
    version: 4,
    activePathRef: legacy.value.activePathRef,
    split: false,
    closedPathRefs: [],
    workbench: emptyIdeWorkbenchState(),
    tabs: legacy.value.tabs.map((tab, index) => ({
      ...tab,
      documentRef: makeIdeDocumentRef("recovery-v2", index),
      generation: IdeDocumentGeneration.make(index),
      incrementalSequence: IdeDocumentSequence.make(0),
      selection: { start: 0, end: 0 },
      tabMode: "pinned" as const,
    })),
  })
}

export const workspaceEditorRecoverySnapshot = (
  state: WorkspaceEditorState,
): WorkspaceEditorRecoverySnapshot => ({
  version: 4,
  activePathRef: state.activePathRef,
  split: state.split,
  closedPathRefs: state.closedPathRefs,
  workbench: state.workbench,
  tabs: state.tabs.flatMap(tab => tab.document === null ? [] : [{
    documentRef: tab.documentRef ?? makeIdeDocumentRef(tab.document?.grantRef ?? "compatibility-recovery", 0),
    generation: tab.generation ?? IdeDocumentGeneration.make(0),
    incrementalSequence: tab.incrementalSequence ?? IdeDocumentSequence.make(0),
    selection: tab.selection,
    pathRef: tab.pathRef,
    expectedRevisionRef: tab.document.revisionRef,
    draft: tab.draft.slice(0, 1_000_000),
    tabMode: tab.tabMode ?? "pinned",
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
  recovered: WorkspaceEditorRecoverySnapshot["tabs"][number] | Readonly<{
    pathRef: string
    expectedRevisionRef: string
    draft: string
  }>,
  result: DesktopWorkspaceDocumentResult,
): WorkspaceEditorState => {
  const documentRef = "documentRef" in recovered
    ? recovered.documentRef
    : makeIdeDocumentRef(grantRef, state.nextDocumentOrdinal)
  const generation = "generation" in recovered
    ? recovered.generation
    : IdeDocumentGeneration.make(state.nextDocumentOrdinal)
  const incrementalSequence = "incrementalSequence" in recovered
    ? recovered.incrementalSequence
    : IdeDocumentSequence.make(0)
  const selection = "selection" in recovered ? recovered.selection : { start: 0, end: 0 }
  const tabMode = "tabMode" in recovered ? recovered.tabMode : "pinned"
  const opening = withWorkspaceEditorOpening(state, recovered.pathRef, grantRef, {
    documentRef,
    generation,
  })
  if (result.state === "available" && result.document.pathRef === recovered.pathRef) {
    const opened = withWorkspaceEditorOpened(opening, recovered.pathRef, result)
    return updateTab(opened, recovered.pathRef, tab => {
      if (result.document.revisionRef === recovered.expectedRevisionRef) {
        return { ...tab, draft: recovered.draft, incrementalSequence, selection, tabMode }
      }
      if (result.document.content === recovered.draft) return {
        ...tab,
        incrementalSequence,
        selection,
        tabMode,
      }
      return {
        ...tab,
        phase: "conflict",
        draft: recovered.draft,
        incrementalSequence,
        selection,
        externalDocument: result.document,
        reason: "This recovered draft changed on disk while the app was closed.",
        tabMode,
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
    incrementalSequence,
    selection,
    saveState: "unavailable",
    reason: result.state === "unavailable"
      ? `${result.message} Your recovered draft remains available for Save As.`
      : "The recovered document could not be matched. Its draft remains available for Save As.",
    tabMode,
  }))
}

const WorkspaceEditorOpenPayloadSchema = Schema.Struct({
  grantRef: Schema.String,
  pathRef: DesktopWorkspacePathRefSchema,
  preview: Schema.optional(Schema.Boolean),
  source: Schema.optional(Schema.Literals(["explorer", "quick_open", "workspace_search", "recent_restore"])),
  identity: Schema.optional(IdePathIndexIdentitySchema),
})
const WorkspaceEditorRecoveryPayloadSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  snapshot: WorkspaceEditorRecoverySnapshotSchema,
})
const WorkspaceEditorTabModePayloadSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  tabMode: Schema.Literals(["preview", "pinned"]),
})
const WorkspaceEditorTabMovePayloadSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  delta: Schema.Literals([-1, 1]),
})
const WorkspaceEditorSettingResetPayloadSchema = Schema.Struct({
  id: IdeEditorSettingIdSchema,
  scope: Schema.Literals(["user", "workspace"]),
})

export const WorkspaceEditorOpenRequested = defineIntent("WorkspaceEditorOpenRequested", WorkspaceEditorOpenPayloadSchema)
export const WorkspaceEditorTabSelected = defineIntent("WorkspaceEditorTabSelected", DesktopWorkspacePathRefSchema)
export const WorkspaceEditorTabModeChanged = defineIntent("WorkspaceEditorTabModeChanged", WorkspaceEditorTabModePayloadSchema)
export const WorkspaceEditorTabMoved = defineIntent("WorkspaceEditorTabMoved", WorkspaceEditorTabMovePayloadSchema)
export const WorkspaceEditorTabsClosed = defineIntent("WorkspaceEditorTabsClosed", Schema.Literals(["active", "others", "right", "all"]))
export const WorkspaceEditorClosedTabReopened = defineIntent("WorkspaceEditorClosedTabReopened", Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)))
export const WorkspaceEditorTabCloseRequested = defineIntent("WorkspaceEditorTabCloseRequested", DesktopWorkspacePathRefSchema)
export const WorkspaceEditorTabCloseConfirmed = defineIntent("WorkspaceEditorTabCloseConfirmed", DesktopWorkspacePathRefSchema)
export const WorkspaceEditorTabCloseCancelled = defineIntent("WorkspaceEditorTabCloseCancelled", Schema.Null)
export const WorkspaceEditorEventReceived = defineIntent("WorkspaceEditorEventReceived", CodeEditorEventSchema)
export const WorkspaceEditorMonacoEventReceived = defineIntent("WorkspaceEditorMonacoEventReceived", IdeMonacoDocumentEventSchema)
export const WorkspaceEditorSaveRequested = defineIntent("WorkspaceEditorSaveRequested", Schema.Null)
export const WorkspaceEditorSaveAllRequested = defineIntent("WorkspaceEditorSaveAllRequested", Schema.Null)
export const WorkspaceEditorUndoRequested = defineIntent("WorkspaceEditorUndoRequested", Schema.Null)
export const WorkspaceEditorRedoRequested = defineIntent("WorkspaceEditorRedoRequested", Schema.Null)
export const WorkspaceEditorFindChanged = defineIntent("WorkspaceEditorFindChanged", Schema.String)
export const WorkspaceEditorFindNext = defineIntent("WorkspaceEditorFindNext", Schema.Null)
export const WorkspaceEditorFindPrevious = defineIntent("WorkspaceEditorFindPrevious", Schema.Null)
export const WorkspaceEditorConflictReload = defineIntent("WorkspaceEditorConflictReload", Schema.Null)
export const WorkspaceEditorConflictKeepMine = defineIntent("WorkspaceEditorConflictKeepMine", Schema.Null)
export const WorkspaceEditorWordWrapToggled = defineIntent("WorkspaceEditorWordWrapToggled", Schema.Null)
export const WorkspaceEditorMinimapToggled = defineIntent("WorkspaceEditorMinimapToggled", Schema.Null)
export const WorkspaceEditorSplitToggled = defineIntent("WorkspaceEditorSplitToggled", Schema.Null)
export const WorkspaceEditorVimToggled = defineIntent("WorkspaceEditorVimToggled", Schema.Null)
export const WorkspaceEditorQuickOpenChanged = defineIntent("WorkspaceEditorQuickOpenChanged", Schema.Struct({
  query: Schema.String.check(Schema.isMaxLength(200)),
  paths: Schema.Array(DesktopWorkspacePathRefSchema).check(Schema.isMaxLength(100_000)),
}))
export const WorkspaceEditorQuickOpenClosed = defineIntent("WorkspaceEditorQuickOpenClosed", Schema.Null)
export const WorkspaceEditorQuickOpenOpened = defineIntent("WorkspaceEditorQuickOpenOpened", Schema.Null)
export const WorkspaceEditorActiveTabPinToggled = defineIntent("WorkspaceEditorActiveTabPinToggled", Schema.Null)
export const WorkspaceEditorActiveTabClosed = defineIntent("WorkspaceEditorActiveTabClosed", Schema.Null)
export const WorkspaceEditorOtherTabsClosed = defineIntent("WorkspaceEditorOtherTabsClosed", Schema.Null)
export const WorkspaceEditorRightTabsClosed = defineIntent("WorkspaceEditorRightTabsClosed", Schema.Null)
export const WorkspaceEditorAllTabsClosed = defineIntent("WorkspaceEditorAllTabsClosed", Schema.Null)
export const WorkspaceEditorNextGroupFocused = defineIntent("WorkspaceEditorNextGroupFocused", Schema.Null)
export const WorkspaceEditorNavigationStepped = defineIntent("WorkspaceEditorNavigationStepped", Schema.Literals(["back", "forward"]))
export const WorkspaceEditorNavigationBackRequested = defineIntent("WorkspaceEditorNavigationBackRequested", Schema.Null)
export const WorkspaceEditorNavigationForwardRequested = defineIntent("WorkspaceEditorNavigationForwardRequested", Schema.Null)
export const WorkspaceEditorGroupFocused = defineIntent("WorkspaceEditorGroupFocused", IdeEditorGroupRefSchema)
export const WorkspaceEditorSettingChanged = defineIntent("WorkspaceEditorSettingChanged", IdeEditorSettingOverrideSchema)
export const WorkspaceEditorSettingReset = defineIntent("WorkspaceEditorSettingReset", WorkspaceEditorSettingResetPayloadSchema)
export const WorkspaceEditorSettingsImported = defineIntent("WorkspaceEditorSettingsImported", Schema.String.check(Schema.isMaxLength(20_000)))
export const WorkspaceEditorExternalChangeReceived = defineIntent("WorkspaceEditorExternalChangeReceived", DesktopWorkspaceChangeSchema)
export const WorkspaceEditorSaveAsStarted = defineIntent("WorkspaceEditorSaveAsStarted", Schema.Null)
export const WorkspaceEditorSaveAsChanged = defineIntent("WorkspaceEditorSaveAsChanged", Schema.String)
export const WorkspaceEditorSaveAsSubmitted = defineIntent("WorkspaceEditorSaveAsSubmitted", Schema.Null)
export const WorkspaceEditorSaveAsCancelled = defineIntent("WorkspaceEditorSaveAsCancelled", Schema.Null)
export const WorkspaceEditorRecoveryRequested = defineIntent("WorkspaceEditorRecoveryRequested", WorkspaceEditorRecoveryPayloadSchema)

export const workspaceEditorIntents = [
  WorkspaceEditorOpenRequested,
  WorkspaceEditorTabSelected,
  WorkspaceEditorTabModeChanged,
  WorkspaceEditorTabMoved,
  WorkspaceEditorTabsClosed,
  WorkspaceEditorClosedTabReopened,
  WorkspaceEditorTabCloseRequested,
  WorkspaceEditorTabCloseConfirmed,
  WorkspaceEditorTabCloseCancelled,
  WorkspaceEditorEventReceived,
  WorkspaceEditorMonacoEventReceived,
  WorkspaceEditorSaveRequested,
  WorkspaceEditorSaveAllRequested,
  WorkspaceEditorUndoRequested,
  WorkspaceEditorRedoRequested,
  WorkspaceEditorFindChanged,
  WorkspaceEditorFindNext,
  WorkspaceEditorFindPrevious,
  WorkspaceEditorConflictReload,
  WorkspaceEditorConflictKeepMine,
  WorkspaceEditorWordWrapToggled,
  WorkspaceEditorMinimapToggled,
  WorkspaceEditorSplitToggled,
  WorkspaceEditorVimToggled,
  WorkspaceEditorQuickOpenChanged,
  WorkspaceEditorQuickOpenClosed,
  WorkspaceEditorQuickOpenOpened,
  WorkspaceEditorActiveTabPinToggled,
  WorkspaceEditorActiveTabClosed,
  WorkspaceEditorOtherTabsClosed,
  WorkspaceEditorRightTabsClosed,
  WorkspaceEditorAllTabsClosed,
  WorkspaceEditorNextGroupFocused,
  WorkspaceEditorNavigationStepped,
  WorkspaceEditorNavigationBackRequested,
  WorkspaceEditorNavigationForwardRequested,
  WorkspaceEditorGroupFocused,
  WorkspaceEditorSettingChanged,
  WorkspaceEditorSettingReset,
  WorkspaceEditorSettingsImported,
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

export type WorkspaceEditorPreferenceHost = Readonly<{
  setVimEnabled: (enabled: boolean) => Promise<void>
}>

export const unavailableWorkspaceEditorPreferenceHost: WorkspaceEditorPreferenceHost = {
  setVimEnabled: async () => {},
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
  preferenceHost: WorkspaceEditorPreferenceHost = unavailableWorkspaceEditorPreferenceHost,
) => {
  const setEditor = (mutate: (editor: WorkspaceEditorState) => WorkspaceEditorState) =>
    Effect.gen(function* () {
      yield* SubscriptionRef.update(state, current => ({ ...current, workspaceEditor: mutate(current.workspaceEditor) }))
      if (onStateChange !== undefined) {
        onStateChange(yield* SubscriptionRef.get(state))
      }
    })

  const savePath = Effect.fn("WorkspaceEditor.savePath")(function* (pathRef: string) {
    const current = yield* SubscriptionRef.get(state)
    const tab = tabFor(current.workspaceEditor, pathRef)
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

  const saveActive = Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const tab = activeTab(current.workspaceEditor)
    if (tab !== null) yield* savePath(tab.pathRef)
  })

  const saveAll = Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    for (const tab of current.workspaceEditor.tabs) {
      if (workspaceEditorTabDirty(tab)) yield* savePath(tab.pathRef)
    }
  })

  const closePath = (pathRef: string, force: boolean) => setEditor(editor => {
    const tab = tabFor(editor, pathRef)
    if (tab === null) return editor
    if (!force) return closeEditorTabs(editor, [pathRef])
    const tabs = editor.tabs.filter(value => value.pathRef !== pathRef)
    return withSynchronizedWorkbench({
      ...editor,
      tabs,
      activePathRef: editor.activePathRef === pathRef ? tabs.at(-1)?.pathRef ?? null : editor.activePathRef,
      closeConfirmRef: editor.closeConfirmRef === pathRef ? null : editor.closeConfirmRef,
      closedPathRefs: [...editor.closedPathRefs, pathRef].slice(-20),
    })
  })

  const closeDocument = (event: Extract<IdeMonacoDocumentEvent, { readonly _tag: "Close" }>) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const tab = current.workspaceEditor.tabs.find(candidate =>
        candidate.documentRef === event.documentRef && candidate.generation === event.generation,
      )
      if (tab !== undefined) yield* closePath(tab.pathRef, event.force)
    })

  const saveDocument = (event: Extract<IdeMonacoDocumentEvent, { readonly _tag: "Save" }>) =>
    Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const tab = current.workspaceEditor.tabs.find(candidate =>
        candidate.documentRef === event.documentRef && candidate.generation === event.generation,
      )
      if (tab !== undefined) yield* savePath(tab.pathRef)
    })

  const refreshChangedDocuments = (change: DesktopWorkspaceChange) => Effect.gen(function* () {
    const current = yield* SubscriptionRef.get(state)
    const changedRefs = workspaceChangePathRefs(change)
    const targets = current.workspaceEditor.tabs.filter(tab =>
      tab.document !== null && (changedRefs === null || changedRefs.includes(tab.pathRef)),
    )
    const results = yield* Effect.promise(() => Promise.all(targets.map(async tab => {
      const raw = await bridge.openWorkspaceDocument({
        grantRef: tab.document!.grantRef,
        pathRef: tab.pathRef,
      }).catch(() => null)
      return [tab.pathRef, decodeWorkspaceDocumentResult(raw) ?? {
        state: "unavailable" as const,
        reason: "unavailable" as const,
        message: "The changed document response could not be read.",
      }] as const
    })))
    if (results.length > 0) yield* setEditor(editor => results.reduce(
      (next, [pathRef, result]) => withWorkspaceEditorExternalResult(next, pathRef, result), editor,
    ))
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
    WorkspaceEditorOpenRequested: ({ grantRef, pathRef, preview = false, source = "explorer", identity }: {
      grantRef: string
      pathRef: string
      preview?: boolean
      source?: "explorer" | "quick_open" | "workspace_search" | "recent_restore"
      identity?: IdePathIndexIdentity
    }) =>
      Effect.gen(function* () {
        const before = yield* SubscriptionRef.get(state)
        if (tabFor(before.workspaceEditor, pathRef) !== null) {
          yield* setEditor(editor => withWorkspaceEditorNavigation(
            withSynchronizedWorkbench({ ...editor, activePathRef: pathRef, closeConfirmRef: null }), pathRef, source, identity,
          ))
          return
        }
        yield* setEditor(editor => {
          const opening = withWorkspaceEditorOpening(editor, pathRef, grantRef)
          return preview ? withWorkspaceEditorTabMode(opening, pathRef, "preview") : opening
        })
        const raw = yield* Effect.promise(() => bridge.openWorkspaceDocument({ grantRef, pathRef }).catch(() => null))
        const result = decodeWorkspaceDocumentResult(raw) ?? {
          state: "unavailable" as const,
          reason: "unavailable" as const,
          message: "The document response could not be read.",
        }
        yield* setEditor(editor => withWorkspaceEditorNavigation(
          withWorkspaceEditorOpened(editor, pathRef, result), pathRef, source, identity,
        ))
      }),
    WorkspaceEditorTabSelected: (pathRef: string) =>
      setEditor(editor => tabFor(editor, pathRef) === null ? editor : withSynchronizedWorkbench({ ...editor, activePathRef: pathRef, closeConfirmRef: null })),
    WorkspaceEditorTabModeChanged: ({ pathRef, tabMode }: { pathRef: string; tabMode: "preview" | "pinned" }) =>
      setEditor(editor => withWorkspaceEditorTabMode(editor, pathRef, tabMode)),
    WorkspaceEditorTabMoved: ({ pathRef, delta }: { pathRef: string; delta: -1 | 1 }) =>
      setEditor(editor => withWorkspaceEditorTabMoved(editor, pathRef, delta)),
    WorkspaceEditorTabsClosed: (operation: "active" | "others" | "right" | "all") =>
      setEditor(editor => withWorkspaceEditorTabsClosed(editor, operation)),
    WorkspaceEditorClosedTabReopened: (grantRef: string) => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const pathRef = before.workspaceEditor.closedPathRefs.at(-1)
      if (pathRef === undefined || tabFor(before.workspaceEditor, pathRef) !== null) return
      yield* setEditor(editor => withWorkspaceEditorOpening({
        ...editor,
        closedPathRefs: editor.closedPathRefs.slice(0, -1),
      }, pathRef, grantRef))
      const raw = yield* Effect.promise(() => bridge.openWorkspaceDocument({ grantRef, pathRef }).catch(() => null))
      const result = decodeWorkspaceDocumentResult(raw) ?? {
        state: "unavailable" as const,
        reason: "unavailable" as const,
        message: "The reopened document response could not be read.",
      }
      yield* setEditor(editor => withWorkspaceEditorOpened(editor, pathRef, result))
    }),
    WorkspaceEditorTabCloseRequested: (pathRef: string) =>
      setEditor(editor => closeEditorTabs(editor, [pathRef])),
    WorkspaceEditorTabCloseConfirmed: (pathRef: string) =>
      setEditor(editor => {
        if (editor.closeConfirmRef !== pathRef) return editor
        const tabs = editor.tabs.filter(tab => tab.pathRef !== pathRef)
        return withSynchronizedWorkbench({
          ...editor,
          tabs,
          activePathRef: editor.activePathRef === pathRef ? tabs.at(-1)?.pathRef ?? null : editor.activePathRef,
          closeConfirmRef: null,
          closedPathRefs: [...editor.closedPathRefs, pathRef].slice(-20),
        })
      }),
    WorkspaceEditorTabCloseCancelled: () => setEditor(editor => ({ ...editor, closeConfirmRef: null })),
    WorkspaceEditorEventReceived: (event: CodeEditorEvent) => event.type === "save"
      ? Effect.gen(function* () {
          yield* setEditor(editor => withWorkspaceEditorEvent(editor, event))
          yield* saveActive
      })
      : setEditor(editor => withWorkspaceEditorEvent(editor, event)),
    WorkspaceEditorMonacoEventReceived: (event: IdeMonacoDocumentEvent) => event._tag === "Save"
      ? saveDocument(event)
      : event._tag === "Close"
        ? closeDocument(event)
        : setEditor(editor => withWorkspaceEditorMonacoEvent(editor, event)),
    WorkspaceEditorSaveRequested: () => saveActive,
    WorkspaceEditorSaveAllRequested: () => saveAll,
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
    WorkspaceEditorWordWrapToggled: () => setEditor(editor => withWorkspaceEditorSetting(editor, {
      id: "editor.wordWrap", scope: "workspace", value: { _tag: "Boolean", value: !editor.wordWrap },
    })),
    WorkspaceEditorMinimapToggled: () => setEditor(editor => withWorkspaceEditorSetting(editor, {
      id: "editor.minimap.enabled", scope: "workspace", value: { _tag: "Boolean", value: !editor.minimap },
    })),
    WorkspaceEditorSplitToggled: () => setEditor(editor => withSynchronizedWorkbench({ ...editor, split: !editor.split })),
    WorkspaceEditorVimToggled: () => Effect.gen(function* () {
      const before = yield* SubscriptionRef.get(state)
      const enabled = !before.workspaceEditor.vimEnabled
      yield* setEditor(editor => withWorkspaceEditorSetting(editor, {
        id: "editor.vim.enabled", scope: "user", value: { _tag: "Boolean", value: enabled },
      }))
      yield* Effect.promise(() => preferenceHost.setVimEnabled(enabled).catch(() => undefined))
    }),
    WorkspaceEditorQuickOpenChanged: ({ query, paths }: { query: string; paths: ReadonlyArray<string> }) =>
      setEditor(editor => withWorkspaceEditorQuickOpen(editor, query, paths)),
    WorkspaceEditorQuickOpenClosed: () => setEditor(editor => ({
      ...editor,
      workbench: { ...editor.workbench, quickOpen: { ...editor.workbench.quickOpen, phase: "closed" } },
    })),
    WorkspaceEditorQuickOpenOpened: () => setEditor(editor => ({
      ...editor,
      workbench: { ...editor.workbench, quickOpen: { ...editor.workbench.quickOpen, phase: editor.workbench.quickOpen.results.length === 0 ? "empty" : "ready" } },
    })),
    WorkspaceEditorActiveTabPinToggled: () => setEditor(editor => {
      const tab = activeTab(editor)
      return tab === null ? editor : withWorkspaceEditorTabMode(editor, tab.pathRef, tab.tabMode === "preview" ? "pinned" : "preview")
    }),
    WorkspaceEditorActiveTabClosed: () => setEditor(editor => withWorkspaceEditorTabsClosed(editor, "active")),
    WorkspaceEditorOtherTabsClosed: () => setEditor(editor => withWorkspaceEditorTabsClosed(editor, "others")),
    WorkspaceEditorRightTabsClosed: () => setEditor(editor => withWorkspaceEditorTabsClosed(editor, "right")),
    WorkspaceEditorAllTabsClosed: () => setEditor(editor => withWorkspaceEditorTabsClosed(editor, "all")),
    WorkspaceEditorNextGroupFocused: () => setEditor(editor => {
      const index = editor.workbench.groups.findIndex(group => group.groupRef === editor.workbench.focusedGroupRef)
      const next = editor.workbench.groups[(Math.max(0, index) + 1) % editor.workbench.groups.length]
      return next === undefined ? editor : { ...editor, workbench: { ...editor.workbench, focusedGroupRef: next.groupRef } }
    }),
    WorkspaceEditorNavigationStepped: (direction: "back" | "forward") => setEditor(editor => stepEditorNavigation(editor, direction)),
    WorkspaceEditorNavigationBackRequested: () => setEditor(editor => stepEditorNavigation(editor, "back")),
    WorkspaceEditorNavigationForwardRequested: () => setEditor(editor => stepEditorNavigation(editor, "forward")),
    WorkspaceEditorGroupFocused: (groupRef: IdeWorkbenchState["focusedGroupRef"]) =>
      setEditor(editor => editor.workbench.groups.some(group => group.groupRef === groupRef)
        ? { ...editor, workbench: { ...editor.workbench, focusedGroupRef: groupRef } }
        : editor),
    WorkspaceEditorSettingChanged: (override: IdeEditorSettingOverride) =>
      setEditor(editor => withWorkspaceEditorSetting(editor, override)),
    WorkspaceEditorSettingReset: ({ id, scope }: { id: IdeEditorSettingId; scope: "user" | "workspace" }) =>
      setEditor(editor => withWorkspaceEditorSettingsState(editor, resetIdeEditorSetting(editor.workbench.settings, id, scope))),
    WorkspaceEditorSettingsImported: (raw: string) =>
      setEditor(editor => withWorkspaceEditorSettingsState(editor, importIdeEditorSettings(raw))),
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
          ...emptyWorkspaceEditorState({ vimEnabled: before.workspaceEditor.vimEnabled }),
          wordWrap: before.workspaceEditor.wordWrap,
          minimap: before.workspaceEditor.minimap,
          split: snapshot.split,
          closedPathRefs: snapshot.closedPathRefs,
          workbench: snapshot.workbench,
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
        yield* setEditor(() => withSynchronizedWorkbench({ ...recovered, activePathRef }))
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
      Button({ key: "workspace-editor-split", label: "Split", variant: "ghost", selected: state.split, onPress: IntentRef("WorkspaceEditorSplitToggled"), a11y: { selected: state.split } }),
      Button({ key: "workspace-editor-vim", label: state.vimEnabled ? "Vim on" : "Vim off", variant: state.vimEnabled ? "primary" : "ghost", selected: state.vimEnabled, onPress: IntentRef("WorkspaceEditorVimToggled"), a11y: { selected: state.vimEnabled } }),
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
