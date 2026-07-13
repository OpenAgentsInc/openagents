import path from "node:path"

export const desktopRuntimeWorkspaceRoot = (input: Readonly<{
  fixtureMode: boolean
  userDataPath: string
  selectedWorkspaceRoot: string | null
  launchFallbackRoot: string
}>): string => input.fixtureMode
  ? path.join(input.userDataPath, "fable-local", "fixture-workspace")
  : input.selectedWorkspaceRoot ?? input.launchFallbackRoot
