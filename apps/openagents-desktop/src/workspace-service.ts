import { createHash, randomUUID } from "node:crypto"
import { lstatSync, realpathSync, readdirSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from "node:fs"
import path from "node:path"
import { execFileSync } from "node:child_process"

import type {
  DesktopWorkspaceFile,
  DesktopWorkspaceSaveResult,
  DesktopWorkspaceSnapshot,
} from "./workspace-contract.ts"

const maxEntries = 120
const maxBytes = 240_000

const inside = (root: string, candidate: string): boolean => {
  const relative = path.relative(root, candidate)
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
}

const revisionFor = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex")

/**
 * Resolve an existing regular file through the chosen root. `realpath` is
 * deliberately checked after lexical containment: a symlinked parent or file
 * may not cross the user-selected root, even when the requested spelling did.
 */
const workspaceFilePath = (root: string, requestedPath: string): string | null => {
  try {
    const lexicalRoot = path.resolve(root)
    const requested = path.resolve(requestedPath)
    if (!inside(lexicalRoot, requested)) return null
    const canonicalRoot = realpathSync(lexicalRoot)
    const stats = lstatSync(requested)
    if (!stats.isFile() || stats.isSymbolicLink()) return null
    const canonicalFile = realpathSync(requested)
    return inside(canonicalRoot, canonicalFile) ? canonicalFile : null
  } catch {
    return null
  }
}

const projectWorkspaceFile = (resolved: string): DesktopWorkspaceFile | null => {
  try {
    const bytes = readFileSync(resolved)
    // Text-only surface: binary data is unavailable rather than coerced into
    // a renderer string or accidentally written back as lossy UTF-8.
    if (bytes.includes(0)) return null
    return {
      path: resolved,
      content: bytes.subarray(0, maxBytes).toString("utf8"),
      truncated: bytes.length > maxBytes,
      revision: revisionFor(bytes),
    }
  } catch {
    return null
  }
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
  const resolved = workspaceFilePath(root, requestedPath)
  return resolved === null ? null : projectWorkspaceFile(resolved)
}

/**
 * Save one existing, bounded text file atomically. The renderer must echo the
 * host-provided revision; a changed file returns the current projection so it
 * can explicitly reload rather than overwriting concurrent work.
 */
export const saveWorkspaceFile = (
  root: string,
  input: Readonly<{ path: string; content: string; expectedRevision: string }>,
): DesktopWorkspaceSaveResult => {
  const resolved = workspaceFilePath(root, input.path)
  if (resolved === null) {
    return { state: "unavailable", message: "This file is not available in the selected workspace." }
  }
  const current = projectWorkspaceFile(resolved)
  if (current === null || current.truncated) {
    return { state: "unavailable", message: "Only bounded text files can be saved." }
  }
  if (current.revision !== input.expectedRevision) return { state: "conflict", file: current }

  const next = Buffer.from(input.content, "utf8")
  if (next.length > maxBytes) {
    return { state: "unavailable", message: "The edited file exceeds the workspace size limit." }
  }

  const temporary = path.join(path.dirname(resolved), `.${path.basename(resolved)}.${randomUUID()}.tmp`)
  try {
    // `wx` avoids clobbering a pre-existing temporary name; rename is atomic
    // within the selected file's directory.
    writeFileSync(temporary, next, { encoding: "utf8", flag: "wx", mode: statSync(resolved).mode })
    renameSync(temporary, resolved)
    const saved = projectWorkspaceFile(resolved)
    return saved === null
      ? { state: "unavailable", message: "The saved file could not be read back." }
      : { state: "saved", file: saved }
  } catch {
    try { unlinkSync(temporary) } catch { /* best-effort cleanup only */ }
    return { state: "unavailable", message: "The file could not be saved." }
  }
}
