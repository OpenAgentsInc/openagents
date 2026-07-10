import { readdirSync, readFileSync, statSync } from "node:fs"
import path from "node:path"
import { execFileSync } from "node:child_process"

import type { DesktopWorkspaceFile, DesktopWorkspaceSnapshot } from "./workspace-contract.ts"

const maxEntries = 120
const maxBytes = 240_000

const inside = (root: string, candidate: string): boolean => {
  const relative = path.relative(root, candidate)
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
}

const gitState = (root: string): DesktopWorkspaceSnapshot["git"] => {
  try {
    return execFileSync("git", ["-C", root, "status", "--porcelain"], { encoding: "utf8", timeout: 2_000 }).trim() === "" ? "clean" : "changed"
  } catch { return "unavailable" }
}

export const inspectWorkspace = (root: string): DesktopWorkspaceSnapshot => {
  const entries = readdirSync(root, { withFileTypes: true })
    .filter((entry) => !entry.name.startsWith("."))
    .sort((left, right) => Number(right.isDirectory()) - Number(left.isDirectory()) || left.name.localeCompare(right.name))
    .slice(0, maxEntries)
    .map((entry) => ({ name: entry.name, path: path.join(root, entry.name), kind: entry.isDirectory() ? "directory" as const : "file" as const }))
  return { root, label: path.basename(root) || root, entries, git: gitState(root) }
}

export const readWorkspaceFile = (root: string, requestedPath: string): DesktopWorkspaceFile | null => {
  const resolved = path.resolve(requestedPath)
  if (!inside(root, resolved)) return null
  try {
    const stats = statSync(resolved)
    if (!stats.isFile()) return null
    const bytes = readFileSync(resolved)
    const content = bytes.subarray(0, maxBytes).toString("utf8")
    return { path: resolved, content, truncated: bytes.length > maxBytes }
  } catch { return null }
}
