import path from "node:path"

export const desktopRuntimeWorkspaceRoot = (input: Readonly<{
  fixtureMode: boolean
  userDataPath: string
  selectedWorkspaceRoot: string | null
  launchFallbackRoot: string
}>): string => input.fixtureMode
  ? path.join(input.userDataPath, "claude-local", "fixture-workspace")
  : input.selectedWorkspaceRoot ?? input.launchFallbackRoot
