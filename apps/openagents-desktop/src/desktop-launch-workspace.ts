import path from "node:path"

/**
 * The filesystem root (`/` on POSIX, a drive root on Windows) is a directory,
 * but it is never a valid workspace: a Finder/Dock launch reports
 * `process.cwd() === "/"`, and returning it opens the app against the entire
 * filesystem and provokes a storm of macOS permission prompts. Treat any
 * candidate that resolves to a filesystem root as unusable and fall back to the
 * user's home directory.
 */
const isFilesystemRoot = (candidate: string): boolean => path.parse(candidate).root === candidate

const usableWorkspace = (
  candidate: string,
  isDirectory: (candidate: string) => boolean,
): boolean => !isFilesystemRoot(candidate) && isDirectory(candidate)

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
  if (usableWorkspace(candidate, input.isDirectory)) return candidate

  const processRoot = path.resolve(input.processWorkingDirectory)
  if (usableWorkspace(processRoot, input.isDirectory)) return processRoot
  return path.resolve(input.homeRoot)
}
