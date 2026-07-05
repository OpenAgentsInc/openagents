import { Schema as S } from "effect"

export const KHALA_CODE_EDITOR_DEFAULT_MAX_FILE_BYTES = 1_000_000

export const KhalaCodeEditorProviderKind = S.Literals([
  "local_workspace",
])
export type KhalaCodeEditorProviderKind =
  typeof KhalaCodeEditorProviderKind.Type

export const KhalaCodeEditorNodeKind = S.Literals([
  "directory",
  "file",
  "symlink",
])
export type KhalaCodeEditorNodeKind =
  typeof KhalaCodeEditorNodeKind.Type

export const KhalaCodeEditorErrorCode = S.Literals([
  "binary_file",
  "file_too_large",
  "not_directory",
  "not_file",
  "not_found",
  "outside_workspace",
  "provider_unavailable",
  "unknown",
])
export type KhalaCodeEditorErrorCode =
  typeof KhalaCodeEditorErrorCode.Type

export const KhalaCodeEditorError = S.Struct({
  code: KhalaCodeEditorErrorCode,
  message: S.String,
  path: S.optional(S.String),
  providerId: S.optional(S.String),
})
export type KhalaCodeEditorError =
  typeof KhalaCodeEditorError.Type

export const KhalaCodeEditorProvider = S.Struct({
  capabilities: S.Struct({
    read: S.Boolean,
    watch: S.Boolean,
    write: S.Boolean,
  }),
  kind: KhalaCodeEditorProviderKind,
  label: S.String,
  providerId: S.String,
  rootPath: S.String,
  status: S.Literals(["available", "unavailable"]),
})
export type KhalaCodeEditorProvider =
  typeof KhalaCodeEditorProvider.Type

export const KhalaCodeEditorWorkspaceRoot = S.Struct({
  label: S.String,
  path: S.String,
  providerId: S.String,
  readonly: S.Boolean,
})
export type KhalaCodeEditorWorkspaceRoot =
  typeof KhalaCodeEditorWorkspaceRoot.Type

export const KhalaCodeEditorTreeNode = S.Struct({
  childrenLoaded: S.Boolean,
  depth: S.Number,
  kind: KhalaCodeEditorNodeKind,
  mtime: S.NullOr(S.Number),
  name: S.String,
  parentPath: S.NullOr(S.String),
  path: S.String,
  providerId: S.String,
  readonly: S.Boolean,
  rootPath: S.String,
  sizeBytes: S.NullOr(S.Number),
  symlink: S.Boolean,
})
export type KhalaCodeEditorTreeNode =
  typeof KhalaCodeEditorTreeNode.Type

export const KhalaCodeEditorProviderListResult = S.Union([
  S.Struct({
    ok: S.Literal(true),
    providers: S.Array(KhalaCodeEditorProvider),
  }),
  S.Struct({
    error: KhalaCodeEditorError,
    ok: S.Literal(false),
  }),
])
export type KhalaCodeEditorProviderListResult =
  typeof KhalaCodeEditorProviderListResult.Type

export const KhalaCodeEditorWorkspaceReadResult = S.Union([
  S.Struct({
    ok: S.Literal(true),
    roots: S.Array(KhalaCodeEditorWorkspaceRoot),
  }),
  S.Struct({
    error: KhalaCodeEditorError,
    ok: S.Literal(false),
  }),
])
export type KhalaCodeEditorWorkspaceReadResult =
  typeof KhalaCodeEditorWorkspaceReadResult.Type

export const KhalaCodeEditorDirectoryReadRequest = S.Struct({
  path: S.optional(S.String),
  providerId: S.optional(S.String),
})
export type KhalaCodeEditorDirectoryReadRequest =
  typeof KhalaCodeEditorDirectoryReadRequest.Type

export const KhalaCodeEditorDirectoryReadResult = S.Union([
  S.Struct({
    entries: S.Array(KhalaCodeEditorTreeNode),
    node: KhalaCodeEditorTreeNode,
    ok: S.Literal(true),
    providerId: S.String,
    rootPath: S.String,
    truncated: S.Boolean,
  }),
  S.Struct({
    error: KhalaCodeEditorError,
    ok: S.Literal(false),
  }),
])
export type KhalaCodeEditorDirectoryReadResult =
  typeof KhalaCodeEditorDirectoryReadResult.Type

export const KhalaCodeEditorFileReadRequest = S.Struct({
  maxBytes: S.optional(S.Number),
  path: S.String,
  providerId: S.optional(S.String),
})
export type KhalaCodeEditorFileReadRequest =
  typeof KhalaCodeEditorFileReadRequest.Type

export const KhalaCodeEditorFileReadResult = S.Union([
  S.Struct({
    content: S.String,
    encoding: S.Literal("utf8"),
    mtime: S.NullOr(S.Number),
    ok: S.Literal(true),
    path: S.String,
    providerId: S.String,
    rootPath: S.String,
    sizeBytes: S.Number,
  }),
  S.Struct({
    error: KhalaCodeEditorError,
    ok: S.Literal(false),
  }),
])
export type KhalaCodeEditorFileReadResult =
  typeof KhalaCodeEditorFileReadResult.Type
