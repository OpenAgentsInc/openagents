import { Exit, Schema } from "@effect-native/core/effect"

export const DesktopWorkspaceSummaryChannel = "openagents-desktop/workspace-summary" as const
export const DesktopWorkspaceChooseChannel = "openagents-desktop/workspace-choose" as const
export const DesktopWorkspaceFilesChannel = "openagents-desktop/workspace-files" as const
export const DesktopWorkspaceReadChannel = "openagents-desktop/workspace-read" as const

export const DesktopWorkspaceFileRequestSchema = Schema.Struct({ path: Schema.String })

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
}>

export const decodeWorkspaceFileRequest = (value: unknown): { path: string } | null => {
  const result = Schema.decodeUnknownExit(DesktopWorkspaceFileRequestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}
