import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopWorkspaceSummaryChannel = "openagents-desktop/workspace-summary" as const
export const DesktopWorkspaceChooseChannel = "openagents-desktop/workspace-choose" as const
export const DesktopWorkspaceFilesChannel = "openagents-desktop/workspace-files" as const
export const DesktopWorkspaceReadChannel = "openagents-desktop/workspace-read" as const
export const DesktopWorkspaceSaveChannel = "openagents-desktop/workspace-save" as const
export const DesktopWorkspaceGitStatusChannel = "openagents-desktop/workspace-git-status" as const
export const DesktopWorkspaceGitDiffChannel = "openagents-desktop/workspace-git-diff" as const
export const DesktopWorkspaceTreeChannel = "openagents-desktop/workspace-tree" as const
export const DesktopWorkspaceRefreshChannel = "openagents-desktop/workspace-refresh" as const
export const DesktopWorkspaceWatchChannel = "openagents-desktop/workspace-watch" as const
export const DesktopWorkspaceChangeChannel = "openagents-desktop/workspace-change" as const

export const DesktopWorkspacePathRefSchema = Schema.String.pipe(
  Schema.check(Schema.isMaxLength(1_024)),
)

export const DesktopWorkspaceTreeRequestSchema = Schema.Struct({
  directoryRef: DesktopWorkspacePathRefSchema,
  offset: Schema.optional(Schema.Number),
  limit: Schema.optional(Schema.Number),
})

export const DesktopWorkspaceSearchRequestSchema = Schema.Struct({
  query: Schema.String,
  mode: Schema.Literals(["path", "content"]),
  offset: Schema.optional(Schema.Number),
  limit: Schema.optional(Schema.Number),
})

export const DesktopWorkspaceWatchRequestSchema = Schema.Struct({
  active: Schema.Boolean,
})

export const DesktopWorkspaceCacheFactSchema = Schema.Struct({
  key: Schema.String,
  epoch: Schema.Number,
  freshness: Schema.Literal("current"),
})

export const DesktopWorkspaceTreeEntrySchema = Schema.Struct({
  name: Schema.String,
  pathRef: DesktopWorkspacePathRefSchema,
  kind: Schema.Literals(["file", "directory"]),
  expandable: Schema.Boolean,
  sizeBytes: Schema.NullOr(Schema.Number),
  revisionRef: Schema.String,
})

export const DesktopWorkspaceTreePageSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    grantRef: Schema.String,
    directoryRef: DesktopWorkspacePathRefSchema,
    entries: Schema.Array(DesktopWorkspaceTreeEntrySchema),
    nextOffset: Schema.NullOr(Schema.Number),
    cache: DesktopWorkspaceCacheFactSchema,
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    message: Schema.String,
  }),
])

export const DesktopWorkspaceChangeSchema = Schema.Struct({
  kind: Schema.Literals(["changed", "overflow", "refresh"]),
  pathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  epoch: Schema.Number,
})

export const DesktopWorkspaceFileRequestSchema = Schema.Struct({ path: Schema.String })
export const DesktopWorkspaceSaveRequestSchema = Schema.Struct({
  path: Schema.String,
  content: Schema.String,
  expectedRevision: Schema.String,
})
export const DesktopWorkspaceGitDiffRequestSchema = Schema.Struct({ path: Schema.String })

export type DesktopWorkspaceEntry = Readonly<{
  name: string
  path: string
  kind: "file" | "directory"
}>

export type DesktopWorkspaceSnapshot = Readonly<{
  root: string
  label: string
  entries: ReadonlyArray<DesktopWorkspaceEntry>
  git: "clean" | "changed" | "unavailable"
}>

export type DesktopWorkspaceCacheFact = Readonly<{
  key: string
  epoch: number
  freshness: "current"
}>

export type DesktopWorkspaceTreeEntry = Readonly<{
  name: string
  pathRef: string
  kind: "file" | "directory"
  expandable: boolean
  sizeBytes: number | null
  revisionRef: string
}>

export type DesktopWorkspaceTreePage =
  | Readonly<{
      state: "available"
      grantRef: string
      directoryRef: string
      entries: ReadonlyArray<DesktopWorkspaceTreeEntry>
      nextOffset: number | null
      cache: DesktopWorkspaceCacheFact
    }>
  | Readonly<{ state: "unavailable"; message: string }>

export type DesktopWorkspaceSearchMatch = Readonly<{
  pathRef: string
  kind: "path" | "content"
  line: number | null
  preview: string | null
}>

export type DesktopWorkspaceSearchPage =
  | Readonly<{
      state: "available"
      grantRef: string
      query: string
      mode: "path" | "content"
      matches: ReadonlyArray<DesktopWorkspaceSearchMatch>
      nextOffset: number | null
      truncated: boolean
      cache: DesktopWorkspaceCacheFact
    }>
  | Readonly<{ state: "unavailable"; message: string }>

export type DesktopWorkspaceChange = Readonly<{
  kind: "changed" | "overflow" | "refresh"
  pathRef: string | null
  epoch: number
}>

export type DesktopWorkspaceFile = Readonly<{
  path: string
  content: string
  truncated: boolean
  /** SHA-256 of the complete confirmed file bytes; required for safe save. */
  revision: string
}>

/** A write is never silently retried after a concurrent file change. */
export type DesktopWorkspaceSaveResult =
  | Readonly<{ state: "saved"; file: DesktopWorkspaceFile }>
  | Readonly<{ state: "conflict"; file: DesktopWorkspaceFile }>
  | Readonly<{ state: "unavailable"; message: string }>

export type DesktopWorkspaceGitChange = Readonly<{
  path: string
  kind: "added" | "modified" | "deleted" | "renamed" | "untracked"
}>

export type DesktopWorkspaceGitStatus =
  | Readonly<{ state: "available"; changes: ReadonlyArray<DesktopWorkspaceGitChange>; truncated: boolean }>
  | Readonly<{ state: "unavailable" }>

export type DesktopWorkspaceGitDiff =
  | Readonly<{ state: "available"; path: string; content: string; truncated: boolean }>
  | Readonly<{ state: "unavailable"; message: string }>

export const decodeWorkspaceFileRequest = (value: unknown): { path: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceFileRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceSaveRequest = (
  value: unknown,
): { path: string; content: string; expectedRevision: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSaveRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceGitDiffRequest = (
  value: unknown,
): { path: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceGitDiffRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceTreeRequest = (
  value: unknown,
): { directoryRef: string; offset?: number; limit?: number } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceTreeRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceSearchRequest = (
  value: unknown,
): { query: string; mode: "path" | "content"; offset?: number; limit?: number } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceWatchRequest = (
  value: unknown,
): { active: boolean } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceWatchRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceTreePage = (
  value: unknown,
): DesktopWorkspaceTreePage | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceTreePageSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceChange = (
  value: unknown,
): DesktopWorkspaceChange | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceChangeSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
