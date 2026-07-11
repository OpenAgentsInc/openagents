import { createHash, randomUUID } from "node:crypto"
import { lstatSync, realpathSync, readdirSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from "node:fs"
import path from "node:path"
import { execFileSync } from "node:child_process"

import type {
  DesktopWorkspaceFile,
  DesktopWorkspaceGitChange,
  DesktopWorkspaceGitDiff,
  DesktopWorkspaceGitStatus,
  DesktopWorkspaceSaveResult,
  DesktopWorkspaceSnapshot,
} from "./workspace-contract.ts"

const maxEntries = 120
const maxBytes = 240_000
const maxGitEntries = 120
const maxGitDiffBytes = 120_000

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

const gitOutput = (root: string, args: ReadonlyArray<string>): string | null => {
  try {
    const canonicalRoot = realpathSync(root)
    return execFileSync("git", ["-C", canonicalRoot, ...args], {
      encoding: "utf8",
      timeout: 2_000,
      maxBuffer: maxGitDiffBytes + 1,
    })
  } catch {
    return null
  }
}

const safeRelativeGitPath = (value: string): string | null => {
  if (value === "" || value.includes("\0")) return null
  const normalized = path.posix.normalize(value.replaceAll("\\", "/"))
  return normalized === "." || normalized.startsWith("../") || path.posix.isAbsolute(normalized)
    ? null
    : normalized
}

const kindForPorcelain = (code: string): DesktopWorkspaceGitChange["kind"] | null => {
  if (code === "??") return "untracked"
  if (code.includes("R") || code.includes("C")) return "renamed"
  if (code.includes("D")) return "deleted"
  if (code.includes("A")) return "added"
  return code.includes("M") || code.includes("T") ? "modified" : null
}

/** Fixed `--porcelain=v1 -z` parser; no caller provides Git arguments. */
export const workspaceGitStatus = (root: string): DesktopWorkspaceGitStatus => {
  const raw = gitOutput(root, ["status", "--porcelain=v1", "-z", "--untracked-files=normal"])
  if (raw === null) return { state: "unavailable" }
  const parts = raw.split("\0")
  const changes: DesktopWorkspaceGitChange[] = []
  for (let index = 0; index < parts.length; index++) {
    const record = parts[index]!
    if (record === "") continue
    const code = record.slice(0, 2)
    const candidate = safeRelativeGitPath(record.slice(3))
    const kind = kindForPorcelain(code)
    // Renames/copies have a second NUL-delimited original name in -z mode.
    if (code.includes("R") || code.includes("C")) index++
    if (candidate === null || kind === null) return { state: "unavailable" }
    changes.push({ path: candidate, kind })
    if (changes.length >= maxGitEntries) return { state: "available", changes, truncated: true }
  }
  return { state: "available", changes, truncated: false }
}

const privateDiffOutput = (content: string): boolean =>
  /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----|(?:github_pat|gh[pousr]_|sk-|AKIA)[A-Za-z0-9_\-]{8,}|authorization\s*:\s*bearer\s+\S+|(?:password|secret|token)\s*[:=]\s*[^\s]+/iu.test(content)

/**
 * A bounded, selected-file unified diff. Binary/secret-shaped/excess output
 * is rejected as unavailable; raw command errors never cross the bridge.
 */
export const workspaceGitDiff = (root: string, requestedPath: string): DesktopWorkspaceGitDiff => {
  const resolved = workspaceFilePath(root, requestedPath)
  if (resolved === null) return { state: "unavailable", message: "This file is not available for review." }
  let relative: string
  try {
    relative = path.relative(realpathSync(root), resolved).split(path.sep).join("/")
  } catch {
    return { state: "unavailable", message: "This file is not available for review." }
  }
  if (safeRelativeGitPath(relative) === null) return { state: "unavailable", message: "This file is not available for review." }
  const numstat = gitOutput(root, ["diff", "--no-ext-diff", "--no-textconv", "--numstat", "--", relative])
  if (numstat === null || /^-\t-/m.test(numstat)) {
    return { state: "unavailable", message: "Binary or unavailable changes cannot be reviewed here." }
  }
  const content = gitOutput(root, ["diff", "--no-ext-diff", "--no-textconv", "--unified=3", "--", relative])
  if (content === null || Buffer.byteLength(content, "utf8") > maxGitDiffBytes || privateDiffOutput(content)) {
    return { state: "unavailable", message: "This diff is unavailable for the bounded review surface." }
  }
  return { state: "available", path: relative, content, truncated: false }
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

export type DesktopWorkspaceService = Readonly<{
  summary: () => DesktopWorkspaceSnapshot
  read: (requestedPath: string) => DesktopWorkspaceFile | null
  save: (input: Readonly<{ path: string; content: string; expectedRevision: string }>) => DesktopWorkspaceSaveResult
  gitStatus: () => DesktopWorkspaceGitStatus
  gitDiff: (requestedPath: string) => DesktopWorkspaceGitDiff
  dispose: () => void
}>

/**
 * Creates one explicitly selected WorkContext service. The process composition
 * root may replace this value after another directory-picker decision, but no
 * ambient cwd or process environment selects the root.
 */
export const openWorkspaceService = (selectedRoot: string): DesktopWorkspaceService => {
  const root = path.resolve(selectedRoot)
  let disposed = false
  return {
    summary: () => {
      if (disposed) throw new Error("workspace_disposed")
      return inspectWorkspace(root)
    },
    read: requestedPath => disposed ? null : readWorkspaceFile(root, requestedPath),
    save: input => disposed
      ? { state: "unavailable", message: "The selected workspace has been disposed." }
      : saveWorkspaceFile(root, input),
    gitStatus: () => disposed ? { state: "unavailable" } : workspaceGitStatus(root),
    gitDiff: requestedPath => disposed
      ? { state: "unavailable", message: "The selected workspace has been disposed." }
      : workspaceGitDiff(root, requestedPath),
    dispose: () => { disposed = true },
  }
}
