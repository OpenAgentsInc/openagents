import path from "node:path"

export type FileAccessMode = "read" | "write"

export type FileAccessReason =
  | "inside_workspace"
  | "outside_workspace"
  | "write_outside_workspace"

export type FileAccessDecision = Readonly<{
  allowed: boolean
  reason: FileAccessReason
}>

export function isPathInWorkspace(
  workspaceRoot: string,
  candidatePath: string,
): boolean {
  const root = normalizeWorkspaceRoot(workspaceRoot)
  const candidate = normalizeCandidatePath(root, candidatePath)

  return candidate === root || candidate.startsWith(`${root}/`)
}

export function classifyFileAccess(
  workspaceRoot: string,
  candidatePath: string,
  mode: FileAccessMode,
): FileAccessDecision {
  if (isPathInWorkspace(workspaceRoot, candidatePath)) {
    return {
      allowed: true,
      reason: "inside_workspace",
    }
  }

  return {
    allowed: false,
    reason:
      mode === "write" ? "write_outside_workspace" : "outside_workspace",
  }
}

function normalizeWorkspaceRoot(workspaceRoot: string): string {
  return path.posix.resolve("/", workspaceRoot)
}

function normalizeCandidatePath(
  normalizedWorkspaceRoot: string,
  candidatePath: string,
): string {
  if (path.posix.isAbsolute(candidatePath)) {
    return path.posix.normalize(candidatePath)
  }

  return path.posix.resolve(normalizedWorkspaceRoot, candidatePath)
}
