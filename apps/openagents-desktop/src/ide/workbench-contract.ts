import { Schema } from "effect"

import { DesktopWorkspacePathRefSchema } from "../workspace-contract.ts"
import {
  IdeEditorGroupRefSchema,
  IdeProjectRefSchema,
  IdeRootRefSchema,
  IdeWorktreeRefSchema,
} from "./project-contract.ts"
import {
  IdeDocumentGeneration,
  IdeDocumentRef,
  IdeEditorSelectionSchema,
} from "./monaco-document-contract.ts"

const boundedCount = (maximum: number) =>
  Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum }))

export const IdeWorkbenchSchemaVersion = Schema.Literal("openagents.desktop.ide-workbench.v1")

export const IdeNavigationEntryRefSchema = Schema.String.check(
  Schema.isPattern(/^ide\.navigation\.[a-z0-9._-]{1,120}$/),
).pipe(Schema.brand("IdeNavigationEntryRef"))
export type IdeNavigationEntryRef = typeof IdeNavigationEntryRefSchema.Type

export const IdeNavigationSourceSchema = Schema.Literals([
  "explorer",
  "quick_open",
  "workspace_search",
  "go_to_line",
  "quick_symbol",
  "problems",
  "outline",
  "breadcrumb",
  "git_review",
  "recent_restore",
  "agent_backlink",
])
export type IdeNavigationSource = typeof IdeNavigationSourceSchema.Type

export const IdeNavigationEntrySchema = Schema.Struct({
  entryRef: IdeNavigationEntryRefSchema,
  source: IdeNavigationSourceSchema,
  projectRef: IdeProjectRefSchema,
  rootRef: IdeRootRefSchema,
  worktreeRef: IdeWorktreeRefSchema,
  documentRef: IdeDocumentRef,
  generation: IdeDocumentGeneration,
  pathRef: DesktopWorkspacePathRefSchema,
  selection: IdeEditorSelectionSchema,
  state: Schema.Literals(["ready", "stale", "unavailable"]),
  reason: Schema.NullOr(Schema.String.check(Schema.isMaxLength(300))),
})
export type IdeNavigationEntry = typeof IdeNavigationEntrySchema.Type

export const IdeNavigationHistorySchema = Schema.Struct({
  entries: Schema.Array(IdeNavigationEntrySchema).check(Schema.isMaxLength(100)),
  cursor: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: -1, maximum: 99 })),
})
export type IdeNavigationHistory = typeof IdeNavigationHistorySchema.Type

export const emptyIdeNavigationHistory = (): IdeNavigationHistory => ({ entries: [], cursor: -1 })

export const pushIdeNavigation = (
  history: IdeNavigationHistory,
  entry: IdeNavigationEntry,
): IdeNavigationHistory => {
  const current = history.entries[history.cursor]
  if (current?.documentRef === entry.documentRef && current.generation === entry.generation && current.source === entry.source &&
    current.selection.start === entry.selection.start && current.selection.end === entry.selection.end) {
    return history
  }
  const entries = [...history.entries.slice(0, history.cursor + 1), entry].slice(-100)
  return IdeNavigationHistorySchema.make({ entries, cursor: entries.length - 1 })
}

export const stepIdeNavigation = (
  history: IdeNavigationHistory,
  direction: "back" | "forward",
): Readonly<{ history: IdeNavigationHistory; entry: IdeNavigationEntry | null }> => {
  const cursor = history.cursor + (direction === "back" ? -1 : 1)
  const entry = history.entries[cursor] ?? null
  return entry === null ? { history, entry: null } : {
    history: IdeNavigationHistorySchema.make({ ...history, cursor }),
    entry,
  }
}

export const markIdeNavigationUnavailable = (
  history: IdeNavigationHistory,
  entryRef: IdeNavigationEntryRef,
  reason: string,
): IdeNavigationHistory => IdeNavigationHistorySchema.make({
  ...history,
  entries: history.entries.map(entry => entry.entryRef === entryRef
    ? { ...entry, state: "unavailable" as const, reason: reason.slice(0, 300) }
    : entry),
})

export const IdeQuickOpenResultSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  score: boundedCount(100_000),
  highlights: Schema.Array(Schema.Struct({ start: boundedCount(1_024), end: boundedCount(1_024) })).check(Schema.isMaxLength(32)),
})
export type IdeQuickOpenResult = typeof IdeQuickOpenResultSchema.Type

export const IdeQuickOpenStateSchema = Schema.Struct({
  phase: Schema.Literals(["closed", "ready", "empty", "unavailable"]),
  query: Schema.String.check(Schema.isMaxLength(200)),
  results: Schema.Array(IdeQuickOpenResultSchema).check(Schema.isMaxLength(100)),
  activeIndex: boundedCount(99),
  reason: Schema.NullOr(Schema.String.check(Schema.isMaxLength(300))),
})
export type IdeQuickOpenState = typeof IdeQuickOpenStateSchema.Type

export const emptyIdeQuickOpenState = (): IdeQuickOpenState => ({
  phase: "closed",
  query: "",
  results: [],
  activeIndex: 0,
  reason: null,
})

const subsequence = (query: string, value: string): Readonly<{ score: number; highlights: ReadonlyArray<{ start: number; end: number }> }> | null => {
  const needle = query.toLocaleLowerCase()
  const haystack = value.toLocaleLowerCase()
  if (needle === "") return { score: 0, highlights: [] }
  let cursor = 0
  let previous = -2
  let score = 0
  const highlights: Array<{ start: number; end: number }> = []
  for (const character of needle) {
    const found = haystack.indexOf(character, cursor)
    if (found < 0) return null
    const boundary = found === 0 || "/._-".includes(value[found - 1] ?? "")
    score += boundary ? 500 : found === previous + 1 ? 250 : Math.max(1, 100 - found)
    const prior = highlights.at(-1)
    if (prior !== undefined && prior.end === found) prior.end = found + 1
    else highlights.push({ start: found, end: found + 1 })
    cursor = found + 1
    previous = found
  }
  return { score, highlights }
}

export const rankIdeQuickOpen = (
  query: string,
  paths: ReadonlyArray<string>,
  limit = 50,
): IdeQuickOpenState => {
  const boundedQuery = query.slice(0, 200)
  const results = paths.flatMap(pathRef => {
    const ranked = subsequence(boundedQuery, pathRef)
    if (ranked === null) return []
    const decoded = Schema.decodeUnknownExit(DesktopWorkspacePathRefSchema)(pathRef)
    return decoded._tag === "Failure" ? [] : [IdeQuickOpenResultSchema.make({
      pathRef: decoded.value,
      score: Math.min(100_000, ranked.score),
      highlights: ranked.highlights,
    })]
  }).sort((left, right) => right.score - left.score || left.pathRef.localeCompare(right.pathRef))
    .slice(0, Math.max(1, Math.min(100, Math.trunc(limit))))
  return IdeQuickOpenStateSchema.make({
    phase: results.length === 0 ? "empty" : "ready",
    query: boundedQuery,
    results,
    activeIndex: 0,
    reason: null,
  })
}

export const IdeOutlineStateSchema = Schema.TaggedUnion({
  Loading: {},
  Partial: { message: Schema.String.check(Schema.isMaxLength(300)) },
  Degraded: { message: Schema.String.check(Schema.isMaxLength(300)) },
  Unavailable: { reason: Schema.Literal("language_service_not_admitted"), message: Schema.String.check(Schema.isMaxLength(300)) },
  Empty: {},
  Ready: { symbols: Schema.Array(Schema.Struct({
    symbolRef: Schema.String.check(Schema.isMaxLength(192)),
    label: Schema.String.check(Schema.isMaxLength(200)),
    selection: IdeEditorSelectionSchema,
  })).check(Schema.isMaxLength(2_000)) },
})
export type IdeOutlineState = typeof IdeOutlineStateSchema.Type

export const preLanguageIdeOutline = (): IdeOutlineState => IdeOutlineStateSchema.cases.Unavailable.make({
  reason: "language_service_not_admitted",
  message: "Outline is waiting for the project language service in IDE-06.",
})

export const IdeBreadcrumbSchema = Schema.Struct({
  kind: Schema.Literals(["project", "root", "folder", "file", "symbol_unavailable"]),
  label: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(200)),
  pathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
})
export type IdeBreadcrumb = typeof IdeBreadcrumbSchema.Type

export const breadcrumbsForPath = (pathRef: string): ReadonlyArray<IdeBreadcrumb> => {
  const parts = pathRef.split("/").filter(Boolean)
  const paths = parts.map((_, index) => parts.slice(0, index + 1).join("/"))
  return [
    IdeBreadcrumbSchema.make({ kind: "project", label: "Project", pathRef: null }),
    ...paths.map((part, index) => IdeBreadcrumbSchema.make({
      kind: index === paths.length - 1 ? "file" : "folder",
      label: parts[index]!,
      pathRef: DesktopWorkspacePathRefSchema.make(part),
    })),
    IdeBreadcrumbSchema.make({ kind: "symbol_unavailable", label: "Symbols unavailable", pathRef: null }),
  ]
}

export const IdeEditorGroupSchema = Schema.Struct({
  groupRef: IdeEditorGroupRefSchema,
  documentRefs: Schema.Array(IdeDocumentRef).check(Schema.isMaxLength(12)),
  activeDocumentRef: Schema.NullOr(IdeDocumentRef),
  direction: Schema.Literals(["primary", "right", "down"]),
  viewStates: Schema.Array(Schema.Struct({
    documentRef: IdeDocumentRef,
    selection: IdeEditorSelectionSchema,
    scrollTop: boundedCount(10_000_000),
    foldedLineStarts: Schema.Array(boundedCount(10_000_000)).check(Schema.isMaxLength(2_000)),
  })).check(Schema.isMaxLength(12)),
})
export type IdeEditorGroup = typeof IdeEditorGroupSchema.Type

export const IdeEditorSettingIdSchema = Schema.Literals([
  "editor.vim.enabled",
  "editor.minimap.enabled",
  "editor.wordWrap",
  "editor.lineNumbers",
  "editor.bracketMatching",
  "editor.indentationGuides",
  "editor.multiCursor",
  "editor.accessibilitySupport",
  "editor.tabSize",
  "editor.insertSpaces",
  "editor.fontSize",
  "editor.lineHeight",
  "editor.renderWhitespace",
  "editor.rulers",
  "editor.stickyScroll",
  "workbench.theme",
])
export type IdeEditorSettingId = typeof IdeEditorSettingIdSchema.Type

export const IdeEditorSettingValueSchema = Schema.Union([
  Schema.Struct({ _tag: Schema.Literal("Boolean"), value: Schema.Boolean }),
  Schema.Struct({ _tag: Schema.Literal("Integer"), value: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 0, maximum: 200 })) }),
  Schema.Struct({ _tag: Schema.Literal("String"), value: Schema.String.check(Schema.isMaxLength(120)) }),
  Schema.Struct({ _tag: Schema.Literal("Integers"), value: Schema.Array(Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 400 }))).check(Schema.isMaxLength(8)) }),
])
export type IdeEditorSettingValue = typeof IdeEditorSettingValueSchema.Type

export const IdeEditorSettingOverrideSchema = Schema.Struct({
  id: IdeEditorSettingIdSchema,
  scope: Schema.Literals(["user", "workspace"]),
  value: IdeEditorSettingValueSchema,
})
export type IdeEditorSettingOverride = typeof IdeEditorSettingOverrideSchema.Type

const defaults: ReadonlyArray<Readonly<{ id: IdeEditorSettingId; value: IdeEditorSettingValue }>> = [
  { id: "editor.vim.enabled", value: { _tag: "Boolean", value: false } },
  { id: "editor.minimap.enabled", value: { _tag: "Boolean", value: false } },
  { id: "editor.wordWrap", value: { _tag: "Boolean", value: false } },
  { id: "editor.lineNumbers", value: { _tag: "Boolean", value: true } },
  { id: "editor.bracketMatching", value: { _tag: "Boolean", value: true } },
  { id: "editor.indentationGuides", value: { _tag: "Boolean", value: true } },
  { id: "editor.multiCursor", value: { _tag: "Boolean", value: true } },
  { id: "editor.accessibilitySupport", value: { _tag: "Boolean", value: true } },
  { id: "editor.tabSize", value: { _tag: "Integer", value: 2 } },
  { id: "editor.insertSpaces", value: { _tag: "Boolean", value: true } },
  { id: "editor.fontSize", value: { _tag: "Integer", value: 12 } },
  { id: "editor.lineHeight", value: { _tag: "Integer", value: 18 } },
  { id: "editor.renderWhitespace", value: { _tag: "String", value: "selection" } },
  { id: "editor.rulers", value: { _tag: "Integers", value: [80, 120] } },
  { id: "editor.stickyScroll", value: { _tag: "Boolean", value: true } },
  { id: "workbench.theme", value: { _tag: "String", value: "tokyo-night" } },
]

export const IdeEditorSettingsStateSchema = Schema.Struct({
  overrides: Schema.Array(IdeEditorSettingOverrideSchema).check(Schema.isMaxLength(32)),
  errors: Schema.Array(Schema.String.check(Schema.isMaxLength(300))).check(Schema.isMaxLength(32)),
})
export type IdeEditorSettingsState = typeof IdeEditorSettingsStateSchema.Type

export const IdeMonacoEditorOptionsSchema = Schema.Struct({
  lineNumbers: Schema.Boolean,
  bracketMatching: Schema.Boolean,
  indentationGuides: Schema.Boolean,
  multiCursor: Schema.Boolean,
  accessibilitySupport: Schema.Boolean,
  tabSize: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 16 })),
  insertSpaces: Schema.Boolean,
  fontSize: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 8, maximum: 40 })),
  lineHeight: Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 12, maximum: 64 })),
  renderWhitespace: Schema.Literals(["none", "boundary", "selection", "trailing", "all"]),
  rulers: Schema.Array(Schema.Number.check(Schema.isInt(), Schema.isBetween({ minimum: 1, maximum: 400 }))).check(Schema.isMaxLength(8)),
  stickyScroll: Schema.Boolean,
}).annotate({ identifier: "IdeMonacoEditorOptions" })
export type IdeMonacoEditorOptions = typeof IdeMonacoEditorOptionsSchema.Type

export const emptyIdeEditorSettings = (): IdeEditorSettingsState => ({ overrides: [], errors: [] })

export const resolveIdeEditorSetting = (
  state: IdeEditorSettingsState,
  id: IdeEditorSettingId,
): Readonly<{ value: IdeEditorSettingValue; source: "default" | "user" | "workspace" }> => {
  const workspace = state.overrides.findLast(entry => entry.id === id && entry.scope === "workspace")
  if (workspace !== undefined) return { value: workspace.value, source: "workspace" }
  const user = state.overrides.findLast(entry => entry.id === id && entry.scope === "user")
  if (user !== undefined) return { value: user.value, source: "user" }
  return { value: defaults.find(entry => entry.id === id)!.value, source: "default" }
}

export const resolveIdeMonacoEditorOptions = (state: IdeEditorSettingsState): IdeMonacoEditorOptions => {
  const bool = (id: IdeEditorSettingId): boolean => {
    const value = resolveIdeEditorSetting(state, id).value
    return value._tag === "Boolean" && value.value
  }
  const integer = (id: IdeEditorSettingId, fallback: number): number => {
    const value = resolveIdeEditorSetting(state, id).value
    return value._tag === "Integer" ? value.value : fallback
  }
  const string = (id: IdeEditorSettingId, fallback: IdeMonacoEditorOptions["renderWhitespace"]): IdeMonacoEditorOptions["renderWhitespace"] => {
    const value = resolveIdeEditorSetting(state, id).value
    return value._tag === "String" && ["none", "boundary", "selection", "trailing", "all"].includes(value.value)
      ? value.value as IdeMonacoEditorOptions["renderWhitespace"] : fallback
  }
  const rulers = resolveIdeEditorSetting(state, "editor.rulers").value
  return IdeMonacoEditorOptionsSchema.make({
    lineNumbers: bool("editor.lineNumbers"),
    bracketMatching: bool("editor.bracketMatching"),
    indentationGuides: bool("editor.indentationGuides"),
    multiCursor: bool("editor.multiCursor"),
    accessibilitySupport: bool("editor.accessibilitySupport"),
    tabSize: Math.max(1, Math.min(16, integer("editor.tabSize", 2))),
    insertSpaces: bool("editor.insertSpaces"),
    fontSize: Math.max(8, Math.min(40, integer("editor.fontSize", 12))),
    lineHeight: Math.max(12, Math.min(64, integer("editor.lineHeight", 18))),
    renderWhitespace: string("editor.renderWhitespace", "selection"),
    rulers: rulers._tag === "Integers" ? rulers.value : [80, 120],
    stickyScroll: bool("editor.stickyScroll"),
  })
}

export const setIdeEditorSetting = (
  state: IdeEditorSettingsState,
  override: IdeEditorSettingOverride,
): IdeEditorSettingsState => IdeEditorSettingsStateSchema.make({
  overrides: [...state.overrides.filter(entry => entry.id !== override.id || entry.scope !== override.scope), override],
  errors: [],
})

export const resetIdeEditorSetting = (
  state: IdeEditorSettingsState,
  id: IdeEditorSettingId,
  scope: "user" | "workspace",
): IdeEditorSettingsState => IdeEditorSettingsStateSchema.make({
  ...state,
  overrides: state.overrides.filter(entry => entry.id !== id || entry.scope !== scope),
})

export const exportIdeEditorSettings = (state: IdeEditorSettingsState): string => JSON.stringify({
  schema: "openagents.desktop.ide-editor-settings.export.v1",
  overrides: state.overrides,
}, null, 2)

const IdeEditorSettingsExportSchema = Schema.Struct({
  schema: Schema.Literal("openagents.desktop.ide-editor-settings.export.v1"),
  overrides: Schema.Array(IdeEditorSettingOverrideSchema).check(Schema.isMaxLength(32)),
})

export const importIdeEditorSettings = (raw: string): IdeEditorSettingsState => {
  try {
    const decoded = Schema.decodeUnknownExit(IdeEditorSettingsExportSchema)(JSON.parse(raw))
    return decoded._tag === "Success"
      ? IdeEditorSettingsStateSchema.make({ overrides: decoded.value.overrides, errors: [] })
      : { overrides: [], errors: ["Settings import did not match the allowlisted schema."] }
  } catch {
    return { overrides: [], errors: ["Settings import was not valid JSON."] }
  }
}

export const IdeWorkbenchStateSchema = Schema.Struct({
  schemaVersion: IdeWorkbenchSchemaVersion,
  navigation: IdeNavigationHistorySchema,
  quickOpen: IdeQuickOpenStateSchema,
  outline: IdeOutlineStateSchema,
  breadcrumbs: Schema.Array(IdeBreadcrumbSchema).check(Schema.isMaxLength(64)),
  groups: Schema.Array(IdeEditorGroupSchema).check(Schema.isMinLength(1), Schema.isMaxLength(2)),
  focusedGroupRef: IdeEditorGroupRefSchema,
  settings: IdeEditorSettingsStateSchema,
})
export type IdeWorkbenchState = typeof IdeWorkbenchStateSchema.Type

export const emptyIdeWorkbenchState = (): IdeWorkbenchState => {
  const groupRef = IdeEditorGroupRefSchema.make("ide.editor-group.primary")
  return IdeWorkbenchStateSchema.make({
    schemaVersion: "openagents.desktop.ide-workbench.v1",
    navigation: emptyIdeNavigationHistory(),
    quickOpen: emptyIdeQuickOpenState(),
    outline: preLanguageIdeOutline(),
    breadcrumbs: [],
    groups: [{ groupRef, documentRefs: [], activeDocumentRef: null, direction: "primary", viewStates: [] }],
    focusedGroupRef: groupRef,
    settings: emptyIdeEditorSettings(),
  })
}
