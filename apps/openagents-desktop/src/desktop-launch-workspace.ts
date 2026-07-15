import path from "node:path"

export const desktopLaunchWorkspaceRoot = (input: Readonly<{
  explicitRoot: string | undefined
  processWorkingDirectory: string
  homeRoot: string
  isDirectory: (candidate: string) => boolean
}>): string => {
  const requested = input.explicitRoot?.trim()
  const candidate = path.resolve(requested === undefined || requested.length === 0
    ? input.processWorkingDirectory
    : requested)
  if (input.isDirectory(candidate)) return candidate

  const processRoot = path.resolve(input.processWorkingDirectory)
  if (input.isDirectory(processRoot)) return processRoot
  return path.resolve(input.homeRoot)
}
