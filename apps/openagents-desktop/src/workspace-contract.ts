import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopWorkspaceSummaryChannel = "openagents-desktop/workspace-summary" as const
export const DesktopWorkspaceWorkingDirectoryChannel = "openagents-desktop/workspace-working-directory" as const
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
export const DesktopWorkspaceMoveChannel = "openagents-desktop/workspace-move" as const
export const DesktopWorkspaceCopyChannel = "openagents-desktop/workspace-copy" as const
export const DesktopWorkspaceDuplicateChannel = "openagents-desktop/workspace-duplicate" as const
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
).annotate({ identifier: "DesktopWorkspacePathRef" })

/** Narrow main-owned projection used by the empty conversation welcome. */
export const DesktopWorkspaceWorkingDirectorySchema = Schema.NullOr(
  Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(4_096)),
).annotate({ identifier: "DesktopWorkspaceWorkingDirectory" })

export const DesktopWorkspaceTreeRequestSchema = Schema.Struct({
  directoryRef: DesktopWorkspacePathRefSchema,
  offset: Schema.optional(Schema.Number),
  limit: Schema.optional(Schema.Number),
}).annotate({ identifier: "DesktopWorkspaceTreeRequest" })

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
}).annotate({ identifier: "DesktopWorkspaceSearchRequest" })

export const DesktopWorkspaceSearchRequestRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(160),
).annotate({ identifier: "DesktopWorkspaceSearchRequestRef" })

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
}).annotate({ identifier: "DesktopWorkspaceSearchBridgeRequest" })

export const DesktopWorkspaceSearchCancelRequestSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
}).annotate({ identifier: "DesktopWorkspaceSearchCancelRequest" })

export const DesktopWorkspaceWatchRequestSchema = Schema.Struct({
  active: Schema.Boolean,
}).annotate({ identifier: "DesktopWorkspaceWatchRequest" })

export const DesktopWorkspaceCreateRequestSchema = Schema.Struct({
  parentRef: DesktopWorkspacePathRefSchema,
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  kind: Schema.Literals(["file", "directory"]),
}).annotate({ identifier: "DesktopWorkspaceCreateRequest" })

export const DesktopWorkspaceRenameRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  name: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
}).annotate({ identifier: "DesktopWorkspaceRenameRequest" })

export const DesktopWorkspaceMoveRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  destinationParentRef: DesktopWorkspacePathRefSchema,
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
}).annotate({ identifier: "DesktopWorkspaceMoveRequest" })

export const DesktopWorkspaceCopyRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  destinationParentRef: DesktopWorkspacePathRefSchema,
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
}).annotate({ identifier: "DesktopWorkspaceCopyRequest" })

export const DesktopWorkspaceDuplicateRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
}).annotate({ identifier: "DesktopWorkspaceDuplicateRequest" })

export const DesktopWorkspaceDeleteRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
}).annotate({ identifier: "DesktopWorkspaceDeleteRequest" })

export const DesktopWorkspaceRevealRequestSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
}).annotate({ identifier: "DesktopWorkspaceRevealRequest" })

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
]).annotate({ identifier: "DesktopWorkspaceDocumentUnavailableReason" })

export const DesktopWorkspaceDocumentRequestSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  pathRef: DesktopWorkspacePathRefSchema,
}).annotate({ identifier: "DesktopWorkspaceDocumentRequest" })

export const DesktopWorkspaceDocumentSaveRequestSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  pathRef: DesktopWorkspacePathRefSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
  expectedRevisionRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
}).annotate({ identifier: "DesktopWorkspaceDocumentSaveRequest" })

export const DesktopWorkspaceDocumentSaveAsRequestSchema = Schema.Struct({
  grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(160)),
  pathRef: DesktopWorkspacePathRefSchema,
  content: Schema.String.check(Schema.isMaxLength(1_000_000)),
}).annotate({ identifier: "DesktopWorkspaceDocumentSaveAsRequest" })

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
}).annotate({ identifier: "DesktopWorkspaceDocument" })

export const DesktopWorkspaceDocumentResultSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("available"), document: DesktopWorkspaceDocumentSchema }),
  Schema.Struct({ state: Schema.Literal("saved"), document: DesktopWorkspaceDocumentSchema }),
  Schema.Struct({ state: Schema.Literal("conflict"), current: DesktopWorkspaceDocumentSchema }),
  Schema.Struct({
    state: Schema.Literal("unavailable"),
    reason: DesktopWorkspaceDocumentUnavailableReasonSchema,
    message: Schema.String.check(Schema.isMaxLength(400)),
  }),
]).annotate({ identifier: "DesktopWorkspaceDocumentResult" })

export const DesktopWorkspaceCacheFactSchema = Schema.Struct({
  key: Schema.String,
  epoch: Schema.Number,
  freshness: Schema.Literal("current"),
}).annotate({ identifier: "DesktopWorkspaceCacheFact" })

export const DesktopWorkspaceTreeEntrySchema = Schema.Struct({
  name: Schema.String,
  pathRef: DesktopWorkspacePathRefSchema,
  kind: Schema.Literals(["file", "directory"]),
  expandable: Schema.Boolean,
  sizeBytes: Schema.NullOr(Schema.Number),
  revisionRef: Schema.String,
}).annotate({ identifier: "DesktopWorkspaceTreeEntry" })

export const DesktopWorkspaceOperationResultSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("created"), entry: DesktopWorkspaceTreeEntrySchema }),
  Schema.Struct({ state: Schema.Literal("renamed"), entry: DesktopWorkspaceTreeEntrySchema }),
  Schema.Struct({ state: Schema.Literal("deleted"), pathRef: DesktopWorkspacePathRefSchema }),
  Schema.Struct({ state: Schema.Literal("revealed"), pathRef: DesktopWorkspacePathRefSchema }),
  Schema.Struct({ state: Schema.Literal("conflict"), message: Schema.String.check(Schema.isMaxLength(400)) }),
  Schema.Struct({ state: Schema.Literal("permission_denied"), message: Schema.String.check(Schema.isMaxLength(400)) }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String.check(Schema.isMaxLength(400)) }),
]).annotate({ identifier: "DesktopWorkspaceOperationResult" })

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
]).annotate({ identifier: "DesktopWorkspaceTreePage" })

export const DesktopWorkspaceSearchMatchSchema = Schema.Struct({
  pathRef: DesktopWorkspacePathRefSchema,
  kind: Schema.Literals(["path", "content"]),
  line: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))),
  preview: Schema.NullOr(Schema.String.check(Schema.isMaxLength(240))),
}).annotate({ identifier: "DesktopWorkspaceSearchMatch" })

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
]).annotate({ identifier: "DesktopWorkspaceSearchPage" })

export const DesktopWorkspaceSearchResponseSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
  page: DesktopWorkspaceSearchPageSchema,
}).annotate({ identifier: "DesktopWorkspaceSearchResponse" })

export const DesktopWorkspaceSearchCancelResultSchema = Schema.Struct({
  requestRef: DesktopWorkspaceSearchRequestRefSchema,
  cancelled: Schema.Boolean,
}).annotate({ identifier: "DesktopWorkspaceSearchCancelResult" })

export const DesktopWorkspaceChangeSchema = Schema.Struct({
  kind: Schema.Literals(["changed", "overflow", "refresh"]),
  pathRef: Schema.NullOr(DesktopWorkspacePathRefSchema),
  pathRefs: Schema.optional(
    Schema.Array(DesktopWorkspacePathRefSchema).check(Schema.isMaxLength(256)),
  ),
  epoch: Schema.Number,
}).annotate({ identifier: "DesktopWorkspaceChange" })

export const DesktopWorkspaceFileRequestSchema = Schema.Struct({ path: Schema.String }).annotate({
  identifier: "DesktopWorkspaceFileRequest",
})
export const DesktopWorkspaceSaveRequestSchema = Schema.Struct({
  path: Schema.String,
  content: Schema.String,
  expectedRevision: Schema.String,
}).annotate({ identifier: "DesktopWorkspaceSaveRequest" })
export const DesktopWorkspaceGitDiffRequestSchema = Schema.Struct({ path: Schema.String }).annotate({
  identifier: "DesktopWorkspaceGitDiffRequest",
})

export const DesktopWorkspaceEntrySchema = Schema.Struct({
  name: Schema.String,
  path: Schema.String,
  kind: Schema.Literals(["file", "directory"]),
}).annotate({ identifier: "DesktopWorkspaceEntry" })

export const DesktopWorkspaceSnapshotSchema = Schema.Struct({
  root: Schema.String,
  label: Schema.String,
  entries: Schema.Array(DesktopWorkspaceEntrySchema),
  git: Schema.Literals(["clean", "changed", "unavailable"]),
}).annotate({ identifier: "DesktopWorkspaceSnapshot" })

export const DesktopWorkspaceFileSchema = Schema.Struct({
  path: Schema.String,
  content: Schema.String,
  truncated: Schema.Boolean,
  /** SHA-256 of the complete confirmed file bytes; required for safe save. */
  revision: Schema.String,
}).annotate({ identifier: "DesktopWorkspaceFile" })

/** A write is never silently retried after a concurrent file change. */
export const DesktopWorkspaceSaveResultSchema = Schema.Union([
  Schema.Struct({ state: Schema.Literal("saved"), file: DesktopWorkspaceFileSchema }),
  Schema.Struct({ state: Schema.Literal("conflict"), file: DesktopWorkspaceFileSchema }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String }),
]).annotate({ identifier: "DesktopWorkspaceSaveResult" })

export const DesktopWorkspaceGitChangeSchema = Schema.Struct({
  path: Schema.String,
  kind: Schema.Literals(["added", "modified", "deleted", "renamed", "untracked"]),
}).annotate({ identifier: "DesktopWorkspaceGitChange" })

export const DesktopWorkspaceGitStatusSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    changes: Schema.Array(DesktopWorkspaceGitChangeSchema),
    truncated: Schema.Boolean,
  }),
  Schema.Struct({ state: Schema.Literal("unavailable") }),
]).annotate({ identifier: "DesktopWorkspaceGitStatus" })

export const DesktopWorkspaceGitDiffSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("available"),
    path: Schema.String,
    content: Schema.String,
    truncated: Schema.Boolean,
  }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: Schema.String }),
]).annotate({ identifier: "DesktopWorkspaceGitDiff" })

export type DesktopWorkspaceEntry = typeof DesktopWorkspaceEntrySchema.Type
export type DesktopWorkspaceSnapshot = typeof DesktopWorkspaceSnapshotSchema.Type
export type DesktopWorkspaceCacheFact = typeof DesktopWorkspaceCacheFactSchema.Type
export type DesktopWorkspaceTreeEntry = typeof DesktopWorkspaceTreeEntrySchema.Type
export type DesktopWorkspaceTreePage = typeof DesktopWorkspaceTreePageSchema.Type
export type DesktopWorkspaceSearchMatch = typeof DesktopWorkspaceSearchMatchSchema.Type
export type DesktopWorkspaceSearchPage = typeof DesktopWorkspaceSearchPageSchema.Type
export type DesktopWorkspaceSearchBridgeRequest = typeof DesktopWorkspaceSearchBridgeRequestSchema.Type
export type DesktopWorkspaceSearchResponse = typeof DesktopWorkspaceSearchResponseSchema.Type
export type DesktopWorkspaceSearchCancelResult = typeof DesktopWorkspaceSearchCancelResultSchema.Type
export type DesktopWorkspaceOperationResult = typeof DesktopWorkspaceOperationResultSchema.Type
export type DesktopWorkspaceDocumentUnavailableReason = typeof DesktopWorkspaceDocumentUnavailableReasonSchema.Type
export type DesktopWorkspaceDocument = typeof DesktopWorkspaceDocumentSchema.Type
export type DesktopWorkspaceDocumentResult = typeof DesktopWorkspaceDocumentResultSchema.Type
export type DesktopWorkspaceChange = typeof DesktopWorkspaceChangeSchema.Type

/** Exact affected refs for a coalesced change; null means full invalidation. */
export const workspaceChangePathRefs = (
  change: DesktopWorkspaceChange,
): ReadonlyArray<string> | null => change.kind !== "changed"
  ? null
  : change.pathRefs ?? (change.pathRef === null ? null : [change.pathRef])

export type DesktopWorkspaceFile = typeof DesktopWorkspaceFileSchema.Type
export type DesktopWorkspaceSaveResult = typeof DesktopWorkspaceSaveResultSchema.Type
export type DesktopWorkspaceGitChange = typeof DesktopWorkspaceGitChangeSchema.Type
export type DesktopWorkspaceGitStatus = typeof DesktopWorkspaceGitStatusSchema.Type
export type DesktopWorkspaceGitDiff = typeof DesktopWorkspaceGitDiffSchema.Type
export type DesktopWorkspaceWorkingDirectory = typeof DesktopWorkspaceWorkingDirectorySchema.Type
export type DesktopWorkspaceTreeRequest = typeof DesktopWorkspaceTreeRequestSchema.Type
export type DesktopWorkspaceSearchRequest = typeof DesktopWorkspaceSearchRequestSchema.Type
export type DesktopWorkspaceSearchCancelRequest = typeof DesktopWorkspaceSearchCancelRequestSchema.Type
export type DesktopWorkspaceWatchRequest = typeof DesktopWorkspaceWatchRequestSchema.Type
export type DesktopWorkspaceCreateRequest = typeof DesktopWorkspaceCreateRequestSchema.Type
export type DesktopWorkspaceRenameRequest = typeof DesktopWorkspaceRenameRequestSchema.Type
export type DesktopWorkspaceMoveRequest = typeof DesktopWorkspaceMoveRequestSchema.Type
export type DesktopWorkspaceCopyRequest = typeof DesktopWorkspaceCopyRequestSchema.Type
export type DesktopWorkspaceDuplicateRequest = typeof DesktopWorkspaceDuplicateRequestSchema.Type
export type DesktopWorkspaceDeleteRequest = typeof DesktopWorkspaceDeleteRequestSchema.Type
export type DesktopWorkspaceRevealRequest = typeof DesktopWorkspaceRevealRequestSchema.Type
export type DesktopWorkspaceDocumentRequest = typeof DesktopWorkspaceDocumentRequestSchema.Type
export type DesktopWorkspaceDocumentSaveRequest = typeof DesktopWorkspaceDocumentSaveRequestSchema.Type
export type DesktopWorkspaceDocumentSaveAsRequest = typeof DesktopWorkspaceDocumentSaveAsRequestSchema.Type
export type DesktopWorkspaceFileRequest = typeof DesktopWorkspaceFileRequestSchema.Type
export type DesktopWorkspaceSaveRequest = typeof DesktopWorkspaceSaveRequestSchema.Type
export type DesktopWorkspaceGitDiffRequest = typeof DesktopWorkspaceGitDiffRequestSchema.Type

export const decodeWorkspaceFileRequest = (value: unknown): DesktopWorkspaceFileRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceFileRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceSaveRequest = (
  value: unknown,
): DesktopWorkspaceSaveRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSaveRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceGitDiffRequest = (
  value: unknown,
): DesktopWorkspaceGitDiffRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceGitDiffRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceTreeRequest = (
  value: unknown,
): DesktopWorkspaceTreeRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceTreeRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceSearchRequest = (
  value: unknown,
): DesktopWorkspaceSearchRequest | null => {
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
): DesktopWorkspaceSearchCancelRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceSearchCancelRequestSchema)(value)
  return Exit.isSuccess(result) && validWorkspaceSearchRequestRef(result.value.requestRef)
    ? result.value
    : null
}

export const decodeWorkspaceWatchRequest = (
  value: unknown,
): DesktopWorkspaceWatchRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceWatchRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceCreateRequest = (
  value: unknown,
): DesktopWorkspaceCreateRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceCreateRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceRenameRequest = (
  value: unknown,
): DesktopWorkspaceRenameRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceRenameRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceMoveRequest = (
  value: unknown,
): DesktopWorkspaceMoveRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceMoveRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceCopyRequest = (
  value: unknown,
): DesktopWorkspaceCopyRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceCopyRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDuplicateRequest = (
  value: unknown,
): DesktopWorkspaceDuplicateRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDuplicateRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDeleteRequest = (
  value: unknown,
): DesktopWorkspaceDeleteRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDeleteRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceRevealRequest = (
  value: unknown,
): DesktopWorkspaceRevealRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceRevealRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentRequest = (
  value: unknown,
): DesktopWorkspaceDocumentRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDocumentRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentSaveRequest = (
  value: unknown,
): DesktopWorkspaceDocumentSaveRequest | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceDocumentSaveRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

export const decodeWorkspaceDocumentSaveAsRequest = (
  value: unknown,
): DesktopWorkspaceDocumentSaveAsRequest | null => {
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

export const decodeWorkspaceWorkingDirectory = (value: unknown): DesktopWorkspaceWorkingDirectory => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceWorkingDirectorySchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
