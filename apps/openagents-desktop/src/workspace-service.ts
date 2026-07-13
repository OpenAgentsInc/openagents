import { createHash, randomUUID } from "node:crypto"
import { lstatSync, mkdirSync, realpathSync, readdirSync, readFileSync, renameSync, rmdirSync, statSync, unlinkSync, watch, writeFileSync } from "node:fs"
import path from "node:path"
import { execFileSync, spawnSync } from "node:child_process"

import type {
  DesktopWorkspaceDocument,
  DesktopWorkspaceDocumentResult,
  DesktopWorkspaceFile,
  DesktopWorkspaceGitChange,
  DesktopWorkspaceGitDiff,
  DesktopWorkspaceGitStatus,
  DesktopWorkspaceSaveResult,
  DesktopWorkspaceSearchPage,
  DesktopWorkspaceSnapshot,
  DesktopWorkspaceTreePage,
  DesktopWorkspaceChange,
  DesktopWorkspaceOperationResult,
} from "./workspace-contract.ts"
import {
  makeWorkspaceSearchHost,
  type WorkspaceSearchHost,
  type WorkspaceSearchTask,
} from "./workspace-search-host.ts"
import { desktopWorkerUrl } from "./desktop-worker-location.ts"
import { workspaceGitEnvironment } from "./git-process-environment.ts"

const maxEntries = 120
const maxBytes = 240_000
const maxGitEntries = 120
const maxGitDiffBytes = 120_000
const maxTreePageEntries = 200
const maxSearchResults = 100
const maxSearchVisitedEntries = 20_000
const maxSearchFileBytes = 1_000_000
const maxSearchPreviewCharacters = 240
const maxDocumentBytes = 1_000_000

const inside = (root: string, candidate: string): boolean => {
  const relative = path.relative(root, candidate)
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
}

const permissionError = (error: unknown): boolean => {
  const code = (error as NodeJS.ErrnoException | null)?.code
  return code === "EACCES" || code === "EPERM" || code === "EROFS"
}

const revisionFor = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex")

const boundedInteger = (
  value: number | undefined,
  fallback: number,
  maximum: number,
): number => value === undefined || !Number.isFinite(value)
  ? fallback
  : Math.max(0, Math.min(maximum, Math.trunc(value)))

const workspacePathRef = (value: string): string | null => {
  if (/[\0\r\n]/u.test(value) || path.isAbsolute(value) || /^[A-Za-z]:[\\/]/u.test(value)) {
    return null
  }
  const normalized = path.posix.normalize(value.replaceAll("\\", "/"))
  if (normalized === ".") return ""
  return normalized === ".." || normalized.startsWith("../") || normalized.length > 1_024
    ? null
    : normalized
}

const secretShapedName = (name: string): boolean =>
  /^\.env(?:\.|$)|(?:^|[-_.])(?:id_rsa|id_ed25519|credentials|secrets?|tokens?|keychain)(?:[-_.]|$)|\.(?:key|p12|pem|pfx)$/iu.test(name)

const defaultIgnoredName = (name: string): boolean =>
  name.startsWith(".") || name === "node_modules"

const canonicalDirectoryForRef = (
  root: string,
  directoryRef: string,
  surfacePermission = false,
): string | null => {
  const relative = workspacePathRef(directoryRef)
  if (relative === null) return null
  try {
    const lexicalRoot = path.resolve(root)
    const requested = path.resolve(lexicalRoot, relative)
    if (!inside(lexicalRoot, requested)) return null
    const stats = lstatSync(requested)
    if (!stats.isDirectory() || stats.isSymbolicLink()) return null
    const canonicalRoot = realpathSync(lexicalRoot)
    const canonicalDirectory = realpathSync(requested)
    return inside(canonicalRoot, canonicalDirectory) ? canonicalDirectory : null
  } catch (error) {
    if (surfacePermission && permissionError(error)) throw error
    return null
  }
}

const gitIgnoredPathRefs = (root: string, refs: ReadonlyArray<string>): Set<string> => {
  const repositoryProbe = spawnSync("git", ["rev-parse", "--is-inside-work-tree"], {
    cwd: root,
    env: workspaceGitEnvironment(),
    stdio: "ignore",
    timeout: 10_000,
  })
  if (repositoryProbe.status !== 0) return new Set()
  const ignored = new Set<string>()
  const classify = (candidates: ReadonlyArray<string>): void => {
    if (candidates.length === 0) return
    const result = spawnSync("git", ["check-ignore", "--no-index", "--", ...candidates], {
      cwd: root,
      env: workspaceGitEnvironment(),
      stdio: "ignore",
      timeout: 10_000,
    })
    if (result.status === 1) return
    if (result.status !== 0) {
      for (const candidate of candidates) ignored.add(candidate)
      return
    }
    if (candidates.length === 1) {
      ignored.add(candidates[0]!)
      return
    }
    const midpoint = Math.ceil(candidates.length / 2)
    classify(candidates.slice(0, midpoint))
    classify(candidates.slice(midpoint))
  }
  classify(refs)
  return ignored
}

const safeDirectoryEntries = (
  root: string,
  directory: string,
): ReadonlyArray<Readonly<{
  absolutePath: string
  name: string
  pathRef: string
  kind: "file" | "directory"
  sizeBytes: number | null
  revisionRef: string
}>> => {
  const candidates = readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    if (defaultIgnoredName(entry.name) || secretShapedName(entry.name)) return []
    const absolutePath = path.join(directory, entry.name)
    try {
      const stats = lstatSync(absolutePath)
      if (stats.isSymbolicLink() || (!stats.isFile() && !stats.isDirectory())) return []
      const pathRef = path.relative(realpathSync(root), absolutePath).split(path.sep).join("/")
      if (workspacePathRef(pathRef) === null) return []
      const kind = stats.isDirectory() ? "directory" as const : "file" as const
      const revisionRef = `workspace.entry.${createHash("sha256")
        .update(`${pathRef}\0${stats.size}\0${stats.mtimeMs}\0${stats.mode}`)
        .digest("hex")}`
      return [{
        absolutePath,
        kind,
        name: entry.name,
        pathRef,
        revisionRef,
        sizeBytes: kind === "file" ? stats.size : null,
      }]
    } catch {
      return []
    }
  })
  const ignored = gitIgnoredPathRefs(root, candidates.map(value => value.pathRef))
  return candidates
    .filter(value => !ignored.has(value.pathRef))
    .sort((left, right) =>
      Number(right.kind === "directory") - Number(left.kind === "directory") ||
      left.name.localeCompare(right.name))
}

type SafeWorkspaceEntry = ReturnType<typeof safeDirectoryEntries>[number]

const safeWorkspaceEntry = (root: string, requestedRef: string): SafeWorkspaceEntry | null => {
  const pathRef = workspacePathRef(requestedRef)
  if (pathRef === null || pathRef === "") return null
  const parentRef = path.posix.dirname(pathRef) === "." ? "" : path.posix.dirname(pathRef)
  const parent = canonicalDirectoryForRef(root, parentRef, true)
  if (parent === null) return null
  try {
    return safeDirectoryEntries(root, parent).find(entry => entry.pathRef === pathRef) ?? null
  } catch (error) {
    if (permissionError(error)) throw error
    return null
  }
}

const publicWorkspaceEntry = (entry: SafeWorkspaceEntry) => ({
  name: entry.name,
  pathRef: entry.pathRef,
  kind: entry.kind,
  expandable: entry.kind === "directory",
  sizeBytes: entry.sizeBytes,
  revisionRef: entry.revisionRef,
})

const workspaceEntryName = (value: string): string | null => {
  if (value !== value.trim() || value.length === 0 || value.length > 120 ||
      value === "." || value === ".." || value.includes("/") || value.includes("\\") ||
      /[\0\r\n]/u.test(value) || defaultIgnoredName(value) || secretShapedName(value)) return null
  return value
}

const missingPath = (value: string): boolean => {
  try {
    lstatSync(value)
    return false
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return true
    throw error
  }
}

const mutationFailure = (
  error: unknown,
  conflictMessage: string,
  unavailableMessage: string,
): DesktopWorkspaceOperationResult => {
  const code = (error as NodeJS.ErrnoException | null)?.code
  if (permissionError(error)) {
    return { state: "permission_denied", message: "The selected workspace no longer permits that change." }
  }
  if (code === "EEXIST" || code === "ENOTEMPTY") return { state: "conflict", message: conflictMessage }
  return { state: "unavailable", message: unavailableMessage }
}

export type WorkspaceMutationIo = Readonly<{
  createFile: (absolutePath: string) => void
  createDirectory: (absolutePath: string) => void
  rename: (source: string, target: string) => void
  deleteFile: (absolutePath: string) => void
  deleteDirectory: (absolutePath: string) => void
}>

const defaultWorkspaceMutationIo: WorkspaceMutationIo = {
  createFile: absolutePath => writeFileSync(absolutePath, "", { encoding: "utf8", flag: "wx" }),
  createDirectory: absolutePath => mkdirSync(absolutePath),
  rename: (source, target) => renameSync(source, target),
  deleteFile: absolutePath => unlinkSync(absolutePath),
  deleteDirectory: absolutePath => rmdirSync(absolutePath),
}

export const createWorkspaceEntry = (
  root: string,
  input: Readonly<{ parentRef: string; name: string; kind: "file" | "directory" }>,
  io: WorkspaceMutationIo = defaultWorkspaceMutationIo,
): DesktopWorkspaceOperationResult => {
  const parentRef = workspacePathRef(input.parentRef)
  const name = workspaceEntryName(input.name)
  let parent: string | null
  try {
    parent = parentRef === null ? null : canonicalDirectoryForRef(root, parentRef, true)
  } catch (error) {
    return mutationFailure(error, "That workspace location is not available.", "That workspace location is not available.")
  }
  if (parentRef === null || name === null || parent === null) {
    return { state: "unavailable", message: "That workspace location is not available." }
  }
  const pathRef = parentRef === "" ? name : `${parentRef}/${name}`
  if (gitIgnoredPathRefs(root, [pathRef]).has(pathRef)) {
    return { state: "unavailable", message: "Ignored workspace entries cannot be created from this surface." }
  }
  const absolutePath = path.join(parent, name)
  try {
    if (!missingPath(absolutePath)) return { state: "conflict", message: "An entry with that name already exists." }
    if (input.kind === "directory") io.createDirectory(absolutePath)
    else io.createFile(absolutePath)
    const entry = safeWorkspaceEntry(root, pathRef)
    return entry === null
      ? { state: "unavailable", message: "The created entry could not be projected safely." }
      : { state: "created", entry: publicWorkspaceEntry(entry) }
  } catch (error) {
    return mutationFailure(error, "An entry with that name already exists.", "The workspace entry could not be created.")
  }
}

export const renameWorkspaceEntry = (
  root: string,
  input: Readonly<{ pathRef: string; name: string; expectedRevisionRef: string }>,
  io: WorkspaceMutationIo = defaultWorkspaceMutationIo,
): DesktopWorkspaceOperationResult => {
  let source: SafeWorkspaceEntry | null
  try {
    source = safeWorkspaceEntry(root, input.pathRef)
  } catch (error) {
    return mutationFailure(error, "That workspace entry is not available.", "That workspace entry is not available.")
  }
  const name = workspaceEntryName(input.name)
  if (source === null || name === null) return { state: "unavailable", message: "That workspace entry is not available." }
  if (source.revisionRef !== input.expectedRevisionRef) {
    return { state: "conflict", message: "That workspace entry changed before it could be renamed." }
  }
  const parentRef = path.posix.dirname(source.pathRef) === "." ? "" : path.posix.dirname(source.pathRef)
  const targetRef = parentRef === "" ? name : `${parentRef}/${name}`
  if (targetRef === source.pathRef) return { state: "conflict", message: "That workspace entry already uses this name." }
  if (gitIgnoredPathRefs(root, [targetRef]).has(targetRef)) {
    return { state: "unavailable", message: "Ignored workspace entries cannot be created from this surface." }
  }
  const target = path.join(path.dirname(source.absolutePath), name)
  try {
    if (!missingPath(target)) return { state: "conflict", message: "An entry with that name already exists." }
    io.rename(source.absolutePath, target)
    const entry = safeWorkspaceEntry(root, targetRef)
    return entry === null
      ? { state: "unavailable", message: "The renamed entry could not be projected safely." }
      : { state: "renamed", entry: publicWorkspaceEntry(entry) }
  } catch (error) {
    return mutationFailure(error, "An entry with that name already exists.", "The workspace entry could not be renamed.")
  }
}

export const deleteWorkspaceEntry = (
  root: string,
  input: Readonly<{ pathRef: string; expectedRevisionRef: string }>,
  io: WorkspaceMutationIo = defaultWorkspaceMutationIo,
): DesktopWorkspaceOperationResult => {
  let entry: SafeWorkspaceEntry | null
  try {
    entry = safeWorkspaceEntry(root, input.pathRef)
  } catch (error) {
    return mutationFailure(error, "That workspace entry is not available.", "That workspace entry is not available.")
  }
  if (entry === null) return { state: "unavailable", message: "That workspace entry is not available." }
  if (entry.revisionRef !== input.expectedRevisionRef) {
    return { state: "conflict", message: "That workspace entry changed before it could be deleted." }
  }
  try {
    if (entry.kind === "directory") io.deleteDirectory(entry.absolutePath)
    else io.deleteFile(entry.absolutePath)
    return { state: "deleted", pathRef: entry.pathRef }
  } catch (error) {
    return mutationFailure(error, "Only empty directories can be deleted.", "The workspace entry could not be deleted.")
  }
}

export const revealWorkspaceEntry = async (
  root: string,
  input: Readonly<{ pathRef: string }>,
  reveal: ((absolutePath: string) => Promise<boolean> | boolean) | undefined,
): Promise<DesktopWorkspaceOperationResult> => {
  let entry: SafeWorkspaceEntry | null
  try {
    entry = safeWorkspaceEntry(root, input.pathRef)
  } catch (error) {
    return mutationFailure(error, "That workspace entry cannot be revealed.", "That workspace entry cannot be revealed.")
  }
  if (entry === null || reveal === undefined) return { state: "unavailable", message: "That workspace entry cannot be revealed." }
  try {
    return await reveal(entry.absolutePath)
      ? { state: "revealed", pathRef: entry.pathRef }
      : { state: "unavailable", message: "The workspace entry could not be revealed." }
  } catch (error) {
    return mutationFailure(error, "The workspace entry could not be revealed.", "The workspace entry could not be revealed.")
  }
}

const cacheFact = (key: string, epoch: number) => ({
  key,
  epoch,
  freshness: "current" as const,
})

export const workspaceTreePage = (input: Readonly<{
  root: string
  grantRef: string
  directoryRef: string
  epoch?: number
  offset?: number
  limit?: number
}>): DesktopWorkspaceTreePage => {
  const directoryRef = workspacePathRef(input.directoryRef)
  const directory = directoryRef === null
    ? null
    : canonicalDirectoryForRef(input.root, directoryRef)
  if (directory === null || directoryRef === null) {
    return { state: "unavailable", message: "This directory is not available in the selected workspace." }
  }
  try {
    const entries = safeDirectoryEntries(input.root, directory)
    const offset = boundedInteger(input.offset, 0, entries.length)
    const limit = Math.max(1, boundedInteger(input.limit, 80, maxTreePageEntries))
    const page = entries.slice(offset, offset + limit)
    const epoch = Math.max(0, Math.trunc(input.epoch ?? 0))
    return {
      state: "available",
      grantRef: input.grantRef,
      directoryRef,
      entries: page.map(entry => ({
        name: entry.name,
        pathRef: entry.pathRef,
        kind: entry.kind,
        expandable: entry.kind === "directory",
        sizeBytes: entry.sizeBytes,
        revisionRef: entry.revisionRef,
      })),
      nextOffset: offset + page.length < entries.length ? offset + page.length : null,
      cache: cacheFact(`workspace.tree.${input.grantRef}.${revisionFor(Buffer.from(directoryRef))}`, epoch),
    }
  } catch {
    return { state: "unavailable", message: "This directory could not be read." }
  }
}

const privateSearchContent = (content: string): boolean =>
  /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----|(?:github_pat|gh[pousr]_|sk-|AKIA)[A-Za-z0-9_\-]{8,}|authorization\s*:\s*bearer\s+\S+|(?:password|secret|token)\s*[:=]\s*[^\s]+/iu.test(content)

const gitContentSearchCandidates = (
  root: string,
): Readonly<{ pathRefs: ReadonlyArray<string>; truncated: boolean }> | null => {
  const result = spawnSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    {
      cwd: root,
      env: workspaceGitEnvironment(),
      encoding: "buffer",
      maxBuffer: 8_000_000,
      timeout: 10_000,
    },
  )
  if (result.status !== 0 || !Buffer.isBuffer(result.stdout)) return null
  const safe = result.stdout.toString("utf8").split("\0").flatMap(value => {
    const pathRef = workspacePathRef(value)
    if (pathRef === null || pathRef === "") return []
    const components = pathRef.split("/")
    return components.some(component => defaultIgnoredName(component) || secretShapedName(component))
      ? []
      : [pathRef]
  }).sort((left, right) => left.localeCompare(right))
  return {
    pathRefs: safe.slice(0, maxSearchVisitedEntries),
    truncated: safe.length > maxSearchVisitedEntries,
  }
}

export const searchWorkspace = (input: Readonly<{
  root: string
  grantRef: string
  query: string
  mode: "path" | "content"
  epoch?: number
  offset?: number
  limit?: number
}>): DesktopWorkspaceSearchPage => {
  const query = input.query.trim().slice(0, 200)
  if (query.length === 0) return { state: "unavailable", message: "Enter a search query." }
  const root = canonicalDirectoryForRef(input.root, "")
  if (root === null) return { state: "unavailable", message: "The selected workspace is unavailable." }
  const offset = boundedInteger(input.offset, 0, maxSearchResults)
  const limit = Math.max(1, boundedInteger(input.limit, 40, maxSearchResults))
  const wanted = Math.min(maxSearchResults, offset + limit + 1)
  const lowered = query.toLocaleLowerCase()
  const matches: Array<{
    pathRef: string
    kind: "path" | "content"
    line: number | null
    preview: string | null
  }> = []
  const gitCandidates = input.mode === "content" ? gitContentSearchCandidates(root) : null
  const directories = gitCandidates === null ? [root] : []
  let visited = 0
  let truncated = gitCandidates?.truncated ?? false

  if (gitCandidates !== null) {
    for (const pathRef of gitCandidates.pathRefs) {
      visited += 1
      const absolutePath = path.join(root, ...pathRef.split("/"))
      try {
        const stats = lstatSync(absolutePath)
        if (!stats.isFile() || stats.isSymbolicLink() || stats.size > maxSearchFileBytes) continue
        const bytes = readFileSync(absolutePath)
        if (bytes.includes(0)) continue
        const content = bytes.toString("utf8")
        if (privateSearchContent(content)) continue
        const lines = content.split(/\r?\n/u)
        const lineIndex = lines.findIndex(line => line.toLocaleLowerCase().includes(lowered))
        if (lineIndex >= 0) {
          matches.push({
            pathRef,
            kind: "content",
            line: lineIndex + 1,
            preview: lines[lineIndex]!.trim().slice(0, maxSearchPreviewCharacters),
          })
        }
      } catch { /* unreadable or concurrently removed entries are omitted */ }
      if (matches.length >= wanted) {
        truncated = true
        break
      }
    }
  }

  while (directories.length > 0 && visited < maxSearchVisitedEntries && matches.length < wanted) {
    const directory = directories.shift()!
    let entries: ReturnType<typeof safeDirectoryEntries>
    try { entries = safeDirectoryEntries(root, directory) } catch { continue }
    for (const entry of entries) {
      visited += 1
      if (visited > maxSearchVisitedEntries) { truncated = true; break }
      if (entry.kind === "directory") {
        directories.push(entry.absolutePath)
        if (input.mode === "path" && entry.pathRef.toLocaleLowerCase().includes(lowered)) {
          matches.push({ pathRef: entry.pathRef, kind: "path", line: null, preview: null })
        }
        continue
      }
      if (input.mode === "path") {
        if (entry.pathRef.toLocaleLowerCase().includes(lowered)) {
          matches.push({ pathRef: entry.pathRef, kind: "path", line: null, preview: null })
        }
        continue
      }
      if ((entry.sizeBytes ?? maxSearchFileBytes + 1) > maxSearchFileBytes) continue
      try {
        const bytes = readFileSync(entry.absolutePath)
        if (bytes.includes(0)) continue
        const content = bytes.toString("utf8")
        if (privateSearchContent(content)) continue
        const lines = content.split(/\r?\n/u)
        const lineIndex = lines.findIndex(line => line.toLocaleLowerCase().includes(lowered))
        if (lineIndex >= 0) {
          matches.push({
            pathRef: entry.pathRef,
            kind: "content",
            line: lineIndex + 1,
            preview: lines[lineIndex]!.trim().slice(0, maxSearchPreviewCharacters),
          })
        }
      } catch { /* unreadable entries are omitted */ }
      if (matches.length >= wanted) break
    }
  }
  if (directories.length > 0 || visited >= maxSearchVisitedEntries || matches.length >= wanted) {
    truncated = true
  }
  const page = matches.slice(offset, offset + limit)
  const hasMore = matches.length > offset + page.length || truncated
  const epoch = Math.max(0, Math.trunc(input.epoch ?? 0))
  const queryKey = revisionFor(Buffer.from(`${input.mode}\0${query}`))
  return {
    state: "available",
    grantRef: input.grantRef,
    query,
    mode: input.mode,
    matches: page,
    nextOffset: hasMore && page.length > 0 ? offset + page.length : null,
    truncated,
    cache: cacheFact(`workspace.search.${input.grantRef}.${queryKey}`, epoch),
  }
}

export type WorkspaceDocumentIo = Readonly<{
  read: (absolutePath: string) => Buffer
  replace: (absolutePath: string, bytes: Buffer) => void
  create: (absolutePath: string, bytes: Buffer) => void
}>

const defaultWorkspaceDocumentIo: WorkspaceDocumentIo = {
  read: absolutePath => readFileSync(absolutePath),
  replace: (absolutePath, bytes) => {
    const temporary = path.join(
      path.dirname(absolutePath),
      `.${path.basename(absolutePath)}.${randomUUID()}.tmp`,
    )
    try {
      writeFileSync(temporary, bytes, {
        flag: "wx",
        mode: statSync(absolutePath).mode,
      })
      renameSync(temporary, absolutePath)
    } catch (error) {
      try { unlinkSync(temporary) } catch { /* best-effort cleanup only */ }
      throw error
    }
  },
  create: (absolutePath, bytes) => writeFileSync(absolutePath, bytes, { flag: "wx", mode: 0o666 }),
}

const documentUnavailable = (
  reason: Extract<DesktopWorkspaceDocumentResult, { state: "unavailable" }>["reason"],
  message: string,
): DesktopWorkspaceDocumentResult => ({ state: "unavailable", reason, message })

const documentLanguageMode = (pathRef: string): DesktopWorkspaceDocument["languageMode"] => {
  const extension = path.posix.extname(pathRef).toLocaleLowerCase()
  if (extension === ".ts" || extension === ".tsx" || extension === ".mts" || extension === ".cts") return "typescript"
  if (extension === ".js" || extension === ".jsx" || extension === ".mjs" || extension === ".cjs") return "javascript"
  if (extension === ".json" || extension === ".jsonl") return "json"
  if (extension === ".md" || extension === ".mdx") return "markdown"
  if (extension === ".rs") return "rust"
  if (extension === ".py") return "python"
  if (extension === ".sh" || extension === ".bash" || extension === ".zsh") return "shell"
  if (extension === ".toml") return "toml"
  if (extension === ".yaml" || extension === ".yml") return "yaml"
  if (extension === ".css") return "css"
  if (extension === ".html" || extension === ".htm") return "html"
  return "plaintext"
}

const documentLineEnding = (content: string): DesktopWorkspaceDocument["lineEnding"] => {
  const hasCrlf = content.includes("\r\n")
  const hasLf = /(^|[^\r])\n/u.test(content)
  if (hasCrlf && hasLf) return "mixed"
  if (hasCrlf) return "crlf"
  if (hasLf) return "lf"
  return "none"
}

const documentEntry = (
  root: string,
  requestedRef: string,
): SafeWorkspaceEntry | DesktopWorkspaceDocumentResult => {
  const pathRef = workspacePathRef(requestedRef)
  if (pathRef === null || pathRef === "") {
    return documentUnavailable("invalid_ref", "That document reference is invalid.")
  }
  const name = path.posix.basename(pathRef)
  if (defaultIgnoredName(name) || secretShapedName(name) ||
      gitIgnoredPathRefs(root, [pathRef]).has(pathRef)) {
    return documentUnavailable("unavailable", "That document is not available from this workspace surface.")
  }
  const parentRef = path.posix.dirname(pathRef) === "." ? "" : path.posix.dirname(pathRef)
  let parent: string | null
  try {
    parent = canonicalDirectoryForRef(root, parentRef, true)
  } catch (error) {
    return permissionError(error)
      ? documentUnavailable("permission_denied", "The selected workspace no longer permits reading that document.")
      : documentUnavailable("unavailable", "That document is not available from this workspace surface.")
  }
  if (parent === null) return documentUnavailable("missing", "That document no longer exists in the selected workspace.")
  const absolutePath = path.join(parent, name)
  try {
    const stats = lstatSync(absolutePath)
    if (stats.isDirectory()) return documentUnavailable("directory", "Folders cannot be opened as documents.")
    if (!stats.isFile() || stats.isSymbolicLink()) {
      return documentUnavailable("unavailable", "That document is not available from this workspace surface.")
    }
    const canonicalRoot = realpathSync(root)
    const canonicalFile = realpathSync(absolutePath)
    if (!inside(canonicalRoot, canonicalFile)) {
      return documentUnavailable("unavailable", "That document is not available from this workspace surface.")
    }
    return {
      absolutePath: canonicalFile,
      name,
      pathRef,
      kind: "file",
      sizeBytes: stats.size,
      revisionRef: `workspace.entry.${createHash("sha256")
        .update(`${pathRef}\0${stats.size}\0${stats.mtimeMs}\0${stats.mode}`)
        .digest("hex")}`,
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException | null)?.code === "ENOENT") {
      return documentUnavailable("missing", "That document no longer exists in the selected workspace.")
    }
    return permissionError(error)
      ? documentUnavailable("permission_denied", "The selected workspace no longer permits reading that document.")
      : documentUnavailable("unavailable", "That document is not available from this workspace surface.")
  }
}

const projectWorkspaceDocument = (
  root: string,
  grantRef: string,
  pathRef: string,
  io: WorkspaceDocumentIo,
): DesktopWorkspaceDocumentResult => {
  const entry = documentEntry(root, pathRef)
  if ("state" in entry) return entry
  let bytes: Buffer
  try {
    bytes = io.read(entry.absolutePath)
  } catch (error) {
    return permissionError(error)
      ? documentUnavailable("permission_denied", "The selected workspace no longer permits reading that document.")
      : documentUnavailable("unavailable", "That document could not be read.")
  }
  if (bytes.length > maxDocumentBytes) {
    return documentUnavailable("too_large", "That document exceeds the 1 MB editor limit.")
  }
  if (bytes.includes(0)) return documentUnavailable("binary", "Binary files cannot be opened in the text editor.")
  const hasUtf8Bom = bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf
  const textBytes = hasUtf8Bom ? bytes.subarray(3) : bytes
  let content: string
  try {
    content = new TextDecoder("utf-8", { fatal: true }).decode(textBytes)
  } catch {
    return documentUnavailable("unsupported_encoding", "Only UTF-8 documents can be opened in the editor.")
  }
  return {
    state: "available",
    document: {
      grantRef,
      pathRef: entry.pathRef,
      content,
      revisionRef: `workspace.document.${revisionFor(bytes)}`,
      languageMode: documentLanguageMode(entry.pathRef),
      encoding: hasUtf8Bom ? "utf-8-bom" : "utf-8",
      lineEnding: documentLineEnding(content),
      sizeBytes: bytes.length,
    },
  }
}

export const openWorkspaceDocument = (
  root: string,
  currentGrantRef: string,
  input: Readonly<{ grantRef: string; pathRef: string }>,
  io: WorkspaceDocumentIo = defaultWorkspaceDocumentIo,
): DesktopWorkspaceDocumentResult => input.grantRef !== currentGrantRef
  ? documentUnavailable("grant_revoked", "The workspace grant changed before that document could be opened.")
  : projectWorkspaceDocument(root, currentGrantRef, input.pathRef, io)

export const saveWorkspaceDocument = (
  root: string,
  currentGrantRef: string,
  input: Readonly<{
    grantRef: string
    pathRef: string
    content: string
    expectedRevisionRef: string
  }>,
  io: WorkspaceDocumentIo = defaultWorkspaceDocumentIo,
): DesktopWorkspaceDocumentResult => {
  if (input.grantRef !== currentGrantRef) {
    return documentUnavailable("grant_revoked", "The workspace grant changed before that document could be saved.")
  }
  const current = projectWorkspaceDocument(root, currentGrantRef, input.pathRef, io)
  if (current.state !== "available") return current
  if (current.document.revisionRef !== input.expectedRevisionRef) {
    return { state: "conflict", current: current.document }
  }
  const contentBytes = Buffer.from(input.content, "utf8")
  const bytes = current.document.encoding === "utf-8-bom"
    ? Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), contentBytes])
    : contentBytes
  if (bytes.length > maxDocumentBytes) {
    return documentUnavailable("too_large", "The edited document exceeds the 1 MB editor limit.")
  }
  if (bytes.includes(0)) {
    return documentUnavailable("binary", "Binary content cannot be saved through the text editor.")
  }
  const entry = documentEntry(root, input.pathRef)
  if ("state" in entry) return entry
  try {
    io.replace(entry.absolutePath, bytes)
  } catch (error) {
    return permissionError(error)
      ? documentUnavailable("permission_denied", "The selected workspace no longer permits saving that document.")
      : documentUnavailable("unavailable", "That document could not be saved.")
  }
  const saved = projectWorkspaceDocument(root, currentGrantRef, input.pathRef, io)
  return saved.state === "available"
    ? { state: "saved", document: saved.document }
    : saved
}

export const saveWorkspaceDocumentAs = (
  root: string,
  currentGrantRef: string,
  input: Readonly<{
    grantRef: string
    pathRef: string
    content: string
  }>,
  io: WorkspaceDocumentIo = defaultWorkspaceDocumentIo,
): DesktopWorkspaceDocumentResult => {
  if (input.grantRef !== currentGrantRef) {
    return documentUnavailable("grant_revoked", "The workspace grant changed before that document could be saved.")
  }
  const pathRef = workspacePathRef(input.pathRef)
  if (pathRef === null || pathRef === "") {
    return documentUnavailable("invalid_ref", "That Save As document reference is invalid.")
  }
  const name = path.posix.basename(pathRef)
  if (workspaceEntryName(name) === null || gitIgnoredPathRefs(root, [pathRef]).has(pathRef)) {
    return documentUnavailable("unavailable", "That Save As target is not available from this workspace surface.")
  }
  const parentRef = path.posix.dirname(pathRef) === "." ? "" : path.posix.dirname(pathRef)
  let parent: string | null
  try {
    parent = canonicalDirectoryForRef(root, parentRef, true)
  } catch (error) {
    return permissionError(error)
      ? documentUnavailable("permission_denied", "The selected workspace no longer permits saving that document.")
      : documentUnavailable("unavailable", "That Save As target is not available from this workspace surface.")
  }
  if (parent === null) {
    return documentUnavailable("missing", "The Save As destination folder does not exist.")
  }
  const bytes = Buffer.from(input.content, "utf8")
  if (bytes.length > maxDocumentBytes) {
    return documentUnavailable("too_large", "The Save As document exceeds the 1 MB editor limit.")
  }
  if (bytes.includes(0)) {
    return documentUnavailable("binary", "Binary content cannot be saved through the text editor.")
  }
  const absolutePath = path.join(parent, name)
  if (!missingPath(absolutePath)) {
    const current = projectWorkspaceDocument(root, currentGrantRef, pathRef, io)
    return current.state === "available"
      ? { state: "conflict", current: current.document }
      : documentUnavailable("unavailable", "Save As never overwrites an existing workspace entry.")
  }
  try {
    io.create(absolutePath, bytes)
  } catch (error) {
    if ((error as NodeJS.ErrnoException | null)?.code === "EEXIST") {
      const current = projectWorkspaceDocument(root, currentGrantRef, pathRef, io)
      return current.state === "available"
        ? { state: "conflict", current: current.document }
        : documentUnavailable("unavailable", "Save As never overwrites an existing workspace entry.")
    }
    return permissionError(error)
      ? documentUnavailable("permission_denied", "The selected workspace no longer permits saving that document.")
      : documentUnavailable("unavailable", "That document could not be created.")
  }
  const saved = projectWorkspaceDocument(root, currentGrantRef, pathRef, io)
  return saved.state === "available"
    ? { state: "saved", document: saved.document }
    : saved
}

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
    return execFileSync("git", ["-C", root, "status", "--porcelain"], {
      encoding: "utf8",
      env: workspaceGitEnvironment(),
      timeout: 2_000,
    }).trim() === "" ? "clean" : "changed"
  } catch { return "unavailable" }
}

const gitOutput = (root: string, args: ReadonlyArray<string>): string | null => {
  try {
    const canonicalRoot = realpathSync(root)
    return execFileSync("git", ["-C", canonicalRoot, ...args], {
      encoding: "utf8",
      env: workspaceGitEnvironment(),
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
  grantRef: string
  summary: () => DesktopWorkspaceSnapshot
  tree: (input: Readonly<{ directoryRef: string; offset?: number; limit?: number }>) => DesktopWorkspaceTreePage
  search: (input: Readonly<{ query: string; mode: "path" | "content"; offset?: number; limit?: number }>) => WorkspaceSearchTask
  createEntry: (input: Readonly<{ parentRef: string; name: string; kind: "file" | "directory" }>) => DesktopWorkspaceOperationResult
  renameEntry: (input: Readonly<{ pathRef: string; name: string; expectedRevisionRef: string }>) => DesktopWorkspaceOperationResult
  deleteEntry: (input: Readonly<{ pathRef: string; expectedRevisionRef: string }>) => DesktopWorkspaceOperationResult
  revealEntry: (input: Readonly<{ pathRef: string }>) => Promise<DesktopWorkspaceOperationResult>
  openDocument: (input: Readonly<{ grantRef: string; pathRef: string }>) => DesktopWorkspaceDocumentResult
  saveDocument: (input: Readonly<{ grantRef: string; pathRef: string; content: string; expectedRevisionRef: string }>) => DesktopWorkspaceDocumentResult
  saveDocumentAs: (input: Readonly<{ grantRef: string; pathRef: string; content: string }>) => DesktopWorkspaceDocumentResult
  refresh: () => void
  subscribe: (listener: (change: DesktopWorkspaceChange) => void) => Readonly<{ close: () => void }>
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
export const openWorkspaceService = (
  selectedRoot: string,
  options: Readonly<{
    grantRef?: string
    watchFactory?: (
      root: string,
      onChange: (pathRef: string | null) => void,
    ) => Readonly<{ close: () => void }>
    searchHostFactory?: (root: string, grantRef: string) => WorkspaceSearchHost
    mutationIo?: WorkspaceMutationIo
    documentIo?: WorkspaceDocumentIo
    reveal?: (absolutePath: string) => Promise<boolean> | boolean
  }> = {},
): DesktopWorkspaceService => {
  const root = path.resolve(selectedRoot)
  const grantRef = options.grantRef ?? `workspace.grant.${randomUUID()}`
  const searchHost = options.searchHostFactory?.(root, grantRef) ?? makeWorkspaceSearchHost(
    root,
    grantRef,
    desktopWorkerUrl(import.meta.url, "workspace-search-worker.js"),
  )
  let disposed = false
  let epoch = 0
  let watcher: Readonly<{ close: () => void }> | null = null
  const listeners = new Set<(change: DesktopWorkspaceChange) => void>()
  const treeCache = new Map<string, DesktopWorkspaceTreePage>()
  const searchCache = new Map<string, DesktopWorkspaceSearchPage>()
  const notify = (kind: DesktopWorkspaceChange["kind"], changedPathRef: string | null) => {
    epoch += 1
    searchHost.cancelAll()
    treeCache.clear()
    searchCache.clear()
    const change = { kind, pathRef: changedPathRef, epoch }
    for (const listener of listeners) listener(change)
  }
  const defaultWatchFactory = (
    watchedRoot: string,
    onChange: (pathRef: string | null) => void,
  ): Readonly<{ close: () => void }> => {
    const native = watch(watchedRoot, { recursive: true }, (_event, filename) => {
      const pathRef = filename === null ? null : workspacePathRef(filename.toString())
      onChange(pathRef)
    })
    native.on("error", () => onChange(null))
    return { close: () => native.close() }
  }
  const ensureWatcher = () => {
    if (watcher !== null || disposed || listeners.size === 0) return
    try {
      watcher = (options.watchFactory ?? defaultWatchFactory)(root, pathRef => {
        if (disposed) return
        notify(pathRef === null ? "overflow" : "changed", pathRef)
      })
    } catch {
      notify("overflow", null)
    }
  }
  const closeWatcher = () => {
    const active = watcher
    watcher = null
    active?.close()
  }
  return {
    grantRef,
    summary: () => {
      if (disposed) throw new Error("workspace_disposed")
      return inspectWorkspace(root)
    },
    tree: request => {
      if (disposed) return { state: "unavailable", message: "The selected workspace has been disposed." }
      const directoryRef = workspacePathRef(request.directoryRef)
      if (directoryRef === null) return { state: "unavailable", message: "This directory is not available in the selected workspace." }
      const offset = boundedInteger(request.offset, 0, Number.MAX_SAFE_INTEGER)
      const limit = Math.max(1, boundedInteger(request.limit, 80, maxTreePageEntries))
      const key = `${directoryRef}\0${offset}\0${limit}\0${epoch}`
      const cached = treeCache.get(key)
      if (cached !== undefined) return cached
      const page = workspaceTreePage({ root, grantRef, directoryRef, offset, limit, epoch })
      treeCache.set(key, page)
      return page
    },
    search: request => {
      if (disposed) return searchHost.start({ query: "", mode: request.mode, epoch })
      const query = request.query.trim().slice(0, 200)
      const offset = boundedInteger(request.offset, 0, maxSearchResults)
      const limit = Math.max(1, boundedInteger(request.limit, 40, maxSearchResults))
      const key = `${request.mode}\0${revisionFor(Buffer.from(query))}\0${offset}\0${limit}\0${epoch}`
      const cached = searchCache.get(key)
      if (cached !== undefined) {
        return {
          taskRef: `workspace.search.cache.${revisionFor(Buffer.from(key))}`,
          result: Promise.resolve(cached),
          cancel: () => undefined,
        }
      }
      const startedEpoch = epoch
      const task = searchHost.start({ query, mode: request.mode, offset, limit, epoch: startedEpoch })
      return {
        ...task,
        result: task.result.then(page => {
          if (!disposed && epoch === startedEpoch && page.state === "available") searchCache.set(key, page)
          return page
        }),
      }
    },
    createEntry: request => {
      if (disposed) return { state: "unavailable", message: "The selected workspace has been disposed." }
      const result = createWorkspaceEntry(root, request, options.mutationIo)
      if (result.state === "created") notify("changed", result.entry.pathRef)
      return result
    },
    renameEntry: request => {
      if (disposed) return { state: "unavailable", message: "The selected workspace has been disposed." }
      const result = renameWorkspaceEntry(root, request, options.mutationIo)
      if (result.state === "renamed") notify("changed", result.entry.pathRef)
      return result
    },
    deleteEntry: request => {
      if (disposed) return { state: "unavailable", message: "The selected workspace has been disposed." }
      const result = deleteWorkspaceEntry(root, request, options.mutationIo)
      if (result.state === "deleted") notify("changed", result.pathRef)
      return result
    },
    revealEntry: request => disposed
      ? Promise.resolve({ state: "unavailable", message: "The selected workspace has been disposed." })
      : revealWorkspaceEntry(root, request, options.reveal),
    openDocument: request => disposed
      ? documentUnavailable("grant_revoked", "The selected workspace has been disposed.")
      : openWorkspaceDocument(root, grantRef, request, options.documentIo),
    saveDocument: request => {
      if (disposed) return documentUnavailable("grant_revoked", "The selected workspace has been disposed.")
      const result = saveWorkspaceDocument(root, grantRef, request, options.documentIo)
      if (result.state === "saved") notify("changed", result.document.pathRef)
      return result
    },
    saveDocumentAs: request => {
      if (disposed) return documentUnavailable("grant_revoked", "The selected workspace has been disposed.")
      const result = saveWorkspaceDocumentAs(root, grantRef, request, options.documentIo)
      if (result.state === "saved") notify("changed", result.document.pathRef)
      return result
    },
    refresh: () => {
      if (!disposed) notify("refresh", null)
    },
    subscribe: listener => {
      if (disposed) return { close: () => undefined }
      listeners.add(listener)
      ensureWatcher()
      let closed = false
      return {
        close: () => {
          if (closed) return
          closed = true
          listeners.delete(listener)
          if (listeners.size === 0) closeWatcher()
        },
      }
    },
    read: requestedPath => disposed ? null : readWorkspaceFile(root, requestedPath),
    save: input => disposed
      ? { state: "unavailable", message: "The selected workspace has been disposed." }
      : (() => {
          const result = saveWorkspaceFile(root, input)
          if (result.state === "saved") {
            const relative = path.relative(root, result.file.path).split(path.sep).join("/")
            notify("changed", workspacePathRef(relative))
          }
          return result
        })(),
    gitStatus: () => disposed ? { state: "unavailable" } : workspaceGitStatus(root),
    gitDiff: requestedPath => disposed
      ? { state: "unavailable", message: "The selected workspace has been disposed." }
      : workspaceGitDiff(root, requestedPath),
    dispose: () => {
      if (disposed) return
      disposed = true
      closeWatcher()
      listeners.clear()
      treeCache.clear()
      searchCache.clear()
      searchHost.dispose()
    },
  }
}
