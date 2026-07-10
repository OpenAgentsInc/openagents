import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopWorkspaceSummaryChannel = "openagents-desktop/workspace-summary" as const
export const DesktopWorkspaceChooseChannel = "openagents-desktop/workspace-choose" as const
export const DesktopWorkspaceFilesChannel = "openagents-desktop/workspace-files" as const
export const DesktopWorkspaceReadChannel = "openagents-desktop/workspace-read" as const
export const DesktopWorkspaceSaveChannel = "openagents-desktop/workspace-save" as const
export const DesktopWorkspaceGitStatusChannel = "openagents-desktop/workspace-git-status" as const
export const DesktopWorkspaceGitDiffChannel = "openagents-desktop/workspace-git-diff" as const

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
