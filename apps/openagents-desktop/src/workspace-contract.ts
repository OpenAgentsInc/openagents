import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopWorkspaceSummaryChannel = "openagents-desktop/workspace-summary" as const
export const DesktopWorkspaceChooseChannel = "openagents-desktop/workspace-choose" as const
export const DesktopWorkspaceFilesChannel = "openagents-desktop/workspace-files" as const
export const DesktopWorkspaceReadChannel = "openagents-desktop/workspace-read" as const
export const DesktopWorkspaceSaveChannel = "openagents-desktop/workspace-save" as const
export const DesktopWorkspaceGitStatusChannel = "openagents-desktop/workspace-git-status" as const
export const DesktopWorkspaceGitDiffChannel = "openagents-desktop/workspace-git-diff" as const
export const DesktopWorkspaceTreeChannel = "openagents-desktop/workspace-tree" as const
export const DesktopWorkspaceSearchChannel = "openagents-desktop/workspace-search" as const
export const DesktopWorkspaceSearchCancelChannel = "openagents-desktop/workspace-search-cancel" as const
export const DesktopWorkspaceCreateChannel = "openagents-desktop/workspace-create" as const
export const DesktopWorkspaceRenameChannel = "openagents-desktop/workspace-rename" as const
export const DesktopWorkspaceDeleteChannel = "openagents-desktop/workspace-delete" as const
export const DesktopWorkspaceRevealChannel = "openagents-desktop/workspace-reveal" as const
export const DesktopWorkspaceDocumentOpenChannel = "openagents-desktop/workspace-document-open" as const
export const DesktopWorkspaceDocumentSaveChannel = "openagents-desktop/workspace-document-save" as const
export const DesktopWorkspaceDocumentSaveAsChannel = "openagents-desktop/workspace-document-save-as" as const
export const DesktopWorkspaceRefreshChannel = "openagents-desktop/workspace-refresh" as const
export const DesktopWorkspaceWatchChannel = "openagents-desktop/workspace-watch" as const
export const DesktopWorkspaceChangeChannel = "openagents-desktop/workspace-change" as const

export const DesktopWorkspacePathRefSchema = Schema.String.pipe(
  Schema.check(
    Schema.isMaxLength(1_024),
    Schema.isPattern(/^(?!\/)(?![A-Za-z]:[\\/])(?!.*(?:^|\/)\.\.(?:\/|$))(?!.*\\)[^\0\r\n]*$/u),
  ),
)

export const DesktopWorkspaceTreeRequestSchema = Schema.Struct({
  directoryRef: DesktopWorkspacePathRefSchema,
  offset: Schema.optional(Schema.Number),
  limit: Schema.optional(Schema.Number),
})

export const DesktopWorkspaceSearchRequestSchema = Schema.Struct({
  query: Schema.String.check(Schema.isMaxLength(200)),
  mode: Schema.Literals(["path", "content"]),
  offset: Schema.optional(Schema.Number.check(
    Schema.isInt(),
    Schema.isBetween({ minimum: 0, maximum: 100 }),
  )),
  limit: Schema.optional(Schema.Number.check(
    Schema.isInt(),
    Schema.isBetween({ minimum: 1, maximum: 100 }),
  )),
})

export const DesktopWorkspaceSearchRequestRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(160),
)

export const DesktopWorkspaceSearchBridgeRequestSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
  query: Schema.String.check(Schema.isMaxLength(200)),
  mode: Schema.Literals(["path", "content"]),
  offset: Schema.optional(Schema.Number.check(
    Schema.isInt(),
    Schema.isBetween({ minimum: 0, maximum: 100 }),
  )),
  limit: Schema.optional(Schema.Number.check(
    Schema.isInt(),
    Schema.isBetween({ minimum: 1, maximum: 100 }),
  )),
})

export const DesktopWorkspaceSearchCancelRequestSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
})

export const DesktopWorkspaceWatchRequestSchema = Schema.Struct({
  active: Schema.Boolean,
})

export const DesktopWorkspaceCreateRequestSchema = Schema.Struct({
  parentRef: DesktopWorkspacePathRefSchema,
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  kind: Schema.Literals(["file", "directory"]),
})

export const DesktopWorkspaceRenameRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
})

export const DesktopWorkspaceDeleteRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
})

export const DesktopWorkspaceRevealRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
})

export const DesktopWorkspaceDocumentUnavailableReasonSchema = Schema.Literals([
  "invalid_ref",
  "unavailable",
  "missing",
  "directory",
  "binary",
  "too_large",
  "unsupported_encoding",
  "permission_denied",
  "grant_revoked",
])

export const DesktopWorkspaceDocumentRequestSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  pathRef: DesktopWorkspacePathRefSchema,
})

export const DesktopWorkspaceDocumentSaveRequestSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  pathRef: DesktopWorkspacePathRefSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
})

export const DesktopWorkspaceDocumentSaveAsRequestSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  pathRef: DesktopWorkspacePathRefSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
})

export const DesktopWorkspaceDocumentSchema = Schema.Struct({
  grantRef: Schema.String,
  pathRef: DesktopWorkspacePathRefSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
  revisionRef: Schema.String,
  languageMode: Schema.Literals([
    "typescript", "javascript", "json", "markdown", "rust", "python",
    "shell", "toml", "yaml", "css", "html", "plaintext",
  ]),
  encoding: Schema.Literals(["utf-8", "utf-8-bom"]),
  lineEnding: Schema.Literals(["lf", "crlf", "mixed", "none"]),
  sizeBytes: Schema.Number,
})

export const DesktopWorkspaceDocumentResultSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("available"), document: DesktopWorkspaceDocumentSchema }),
  Schema.Struct({ state: Schema.Literal("saved"), document: DesktopWorkspaceDocumentSchema }),
  Schema.Struct({ state: Schema.Literal("conflict"), current: DesktopWorkspaceDocumentSchema }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    reason: DesktopWorkspaceDocumentUnavailableReasonSchema,
    message: Schema.String.check(Schema.isMaxLength(400)),
  }),
])

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

export const DesktopWorkspaceOperationResultSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("created"), entry: DesktopWorkspaceTreeEntrySchema }),
  Schema.Struct({ state: Schema.Literal("renamed"), entry: DesktopWorkspaceTreeEntrySchema }),
  Schema.Struct({ state: Schema.Literal("deleted"), pathRef: DesktopWorkspacePathRefSchema }),
  Schema.Struct({ state: Schema.Literal("revealed"), pathRef: DesktopWorkspacePathRefSchema }),
  Schema.Struct({ state: Schema.Literal("conflict"), message: Schema.String.check(Schema.isMaxLength(400)) }),
  Schema.Struct({ state: Schema.Literal("permission_denied"), message: Schema.String.check(Schema.isMaxLength(400)) }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String.check(Schema.isMaxLength(400)) }),
])

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

export const DesktopWorkspaceSearchMatchSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  kind: Schema.Literals(["path", "content"]),
  line: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))),
  preview: Schema.NullOr(Schema.String.check(Schema.isMaxLength(240))),
})

export const DesktopWorkspaceSearchPageSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    grantRef: Schema.String,
    query: Schema.String.check(Schema.isMaxLength(200)),
    mode: Schema.Literals(["path", "content"]),
    matches: Schema.Array(DesktopWorkspaceSearchMatchSchema).check(Schema.isMaxLength(100)),
    nextOffset: Schema.NullOr(Schema.Number.check(
      Schema.isInt(),
      Schema.isBetween({ minimum: 1, maximum: 100 }),
    )),
    truncated: Schema.Boolean,
    cache: DesktopWorkspaceCacheFactSchema,
  }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    message: Schema.String,
  }),
])

export const DesktopWorkspaceSearchResponseSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
  page: DesktopWorkspaceSearchPageSchema,
})

export const DesktopWorkspaceSearchCancelResultSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
  cancelled: Schema.Boolean,
})

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

export type DesktopWorkspaceSearchBridgeRequest = Readonly<{
  requestRef: string
  query: string
  mode: "path" | "content"
  offset?: number
  limit?: number
}>

export type DesktopWorkspaceSearchResponse = Readonly<{
  requestRef: string
  page: DesktopWorkspaceSearchPage
}>

export type DesktopWorkspaceSearchCancelResult = Readonly<{
  requestRef: string
  cancelled: boolean
}>

export type DesktopWorkspaceOperationResult =
  | Readonly<{ state: "created"; entry: DesktopWorkspaceTreeEntry }>
  | Readonly<{ state: "renamed"; entry: DesktopWorkspaceTreeEntry }>
  | Readonly<{ state: "deleted"; pathRef: string }>
  | Readonly<{ state: "revealed"; pathRef: string }>
  | Readonly<{ state: "conflict"; message: string }>
  | Readonly<{ state: "permission_denied"; message: string }>
  | Readonly<{ state: "unavailable"; message: string }>

export type DesktopWorkspaceDocumentUnavailableReason =
  | "invalid_ref"
  | "unavailable"
  | "missing"
  | "directory"
  | "binary"
  | "too_large"
  | "unsupported_encoding"
  | "permission_denied"
  | "grant_revoked"

export type DesktopWorkspaceDocument = Readonly<{
  grantRef: string
  pathRef: string
  content: string
  revisionRef: string
  languageMode: "typescript" | "javascript" | "json" | "markdown" | "rust" | "python" | "shell" | "toml" | "yaml" | "css" | "html" | "plaintext"
  encoding: "utf-8" | "utf-8-bom"
  lineEnding: "lf" | "crlf" | "mixed" | "none"
  sizeBytes: number
}>

export type DesktopWorkspaceDocumentResult =
  | Readonly<{ state: "available"; document: DesktopWorkspaceDocument }>
  | Readonly<{ state: "saved"; document: DesktopWorkspaceDocument }>
  | Readonly<{ state: "conflict"; current: DesktopWorkspaceDocument }>
  | Readonly<{
      state: "unavailable"
      reason: DesktopWorkspaceDocumentUnavailableReason
      message: string
    }>

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

const validWorkspaceSearchRequestRef = (value: string): boolean =>
  /^workspace\.search\.request\.[A-Za-z0-9._-]{1,120}$/u.test(value)

export const decodeWorkspaceSearchBridgeRequest = (
  value: unknown,
): DesktopWorkspaceSearchBridgeRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchBridgeRequestSchema)(value)
  return Exit.isSuccess(result) && validWorkspaceSearchRequestRef(result.value.requestRef)
    ? result.value
    : null
}

export const decodeWorkspaceSearchCancelRequest = (
  value: unknown,
): { requestRef: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchCancelRequestSchema)(value)
  return Exit.isSuccess(result) && validWorkspaceSearchRequestRef(result.value.requestRef)
    ? result.value
    : null
}

export const decodeWorkspaceWatchRequest = (
  value: unknown,
): { active: boolean } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceWatchRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceCreateRequest = (
  value: unknown,
): { parentRef: string; name: string; kind: "file" | "directory" } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceCreateRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceRenameRequest = (
  value: unknown,
): { pathRef: string; name: string; expectedRevisionRef: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceRenameRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDeleteRequest = (
  value: unknown,
): { pathRef: string; expectedRevisionRef: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDeleteRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceRevealRequest = (
  value: unknown,
): { pathRef: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceRevealRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentRequest = (
  value: unknown,
): { grantRef: string; pathRef: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDocumentRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentSaveRequest = (
  value: unknown,
): { grantRef: string; pathRef: string; content: string; expectedRevisionRef: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDocumentSaveRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentSaveAsRequest = (
  value: unknown,
): { grantRef: string; pathRef: string; content: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDocumentSaveAsRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceTreePage = (
  value: unknown,
): DesktopWorkspaceTreePage | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceTreePageSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceSearchPage = (
  value: unknown,
): DesktopWorkspaceSearchPage | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchPageSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceSearchResponse = (
  value: unknown,
): DesktopWorkspaceSearchResponse | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchResponseSchema)(value)
  return Exit.isSuccess(result) && validWorkspaceSearchRequestRef(result.value.requestRef)
    ? result.value
    : null
}

export const decodeWorkspaceSearchCancelResult = (
  value: unknown,
): DesktopWorkspaceSearchCancelResult | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchCancelResultSchema)(value)
  return Exit.isSuccess(result) && validWorkspaceSearchRequestRef(result.value.requestRef)
    ? result.value
    : null
}

export const decodeWorkspaceChange = (
  value: unknown,
): DesktopWorkspaceChange | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceChangeSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceOperationResult = (
  value: unknown,
): DesktopWorkspaceOperationResult | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceOperationResultSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentResult = (
  value: unknown,
): DesktopWorkspaceDocumentResult | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDocumentResultSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
