import { readFile, realpath, readdir, stat } from "node:fs/promises"
import { basename, dirname, isAbsolute, relative, resolve } from "node:path"
import { Effect } from "effect"
import {
  khalaToolDenied,
  khalaToolError,
  khalaToolOk,
  type KhalaPermissionRequest,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export interface KhalaGrepToolOptions {
  readonly maxContextLines?: number
  readonly maxFileBytes?: number
  readonly maxLineLength?: number
  readonly maxMatches?: number
  readonly maxModelBytes?: number
  readonly maxRipgrepOutputBytes?: number
  readonly maxSearchBytes?: number
}

export const grepToolDefinition: KhalaToolDefinition = {
  authority: "search",
  availability: ["inspect", "coding", "owner_local_full"],
  description: "Search workspace text content with bounded, structured match output.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      context: {
        description: "Number of context lines before and after each match.",
        minimum: 0,
        type: "integer",
      },
      glob: {
        description: "Optional file glob filter relative to the search root.",
        type: "string",
      },
      ignore_case: {
        description: "Search case-insensitively.",
        type: "boolean",
      },
      limit: {
        description: "Maximum number of matching lines to return.",
        minimum: 1,
        type: "integer",
      },
      literal: {
        description: "Treat pattern as a literal string instead of a regular expression.",
        type: "boolean",
      },
      path: {
        description: "Workspace-relative search root, file path, or approved absolute path.",
        type: "string",
      },
      pattern: {
        description: "Regular expression or literal content to search for.",
        type: "string",
      },
    },
    required: ["pattern"],
    type: "object",
  },
  internalId: "khala.search.grep",
  label: "Search Text",
  name: "grep",
  outputSchema: {
    additionalProperties: false,
    properties: {
      matches: { type: "array" },
      pattern: { type: "string" },
      totalMatches: { type: "integer" },
      truncated: { type: "boolean" },
    },
    required: ["pattern", "matches", "totalMatches", "truncated"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Search text content. Use glob for path discovery and read for full files.",
  promptGuidelines: [
    "Use literal for exact strings that contain regex punctuation.",
    "Use glob and path to scope broad searches.",
    "Use context sparingly because it increases model-visible output.",
  ],
  renderer: { kind: "content_search", rendererRef: "khala.renderer.content_search.v1" },
}

export function createGrepTool(options: KhalaGrepToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: grepToolDefinition,
    execute: (input, context) => executeGrepTool(input, context, options),
  }
}

type GrepInput = Readonly<{
  context: number
  glob?: string
  ignoreCase: boolean
  limit?: number
  literal: boolean
  path: string
  pattern: string
}>

type ResolvedSearchTarget = Readonly<{
  _tag: "ok"
  displayPath: string
  kind: "directory" | "file"
  realPath: string
  workspaceRoot: string
}>

type RawGrepMatch = Readonly<{
  column: number
  displayPath: string
  line: number
  lineText: string
  matchText: string
  realPath: string
}>

type GrepContextLine = Readonly<{
  line: number
  text: string
  truncated: boolean
}>

type GrepUiMatch = Readonly<{
  column: number
  contextAfter: ReadonlyArray<GrepContextLine>
  contextBefore: ReadonlyArray<GrepContextLine>
  file: string
  line: number
  match: string
  text: string
  truncated: boolean
}>

type GrepSearchResult = Readonly<{
  matches: ReadonlyArray<RawGrepMatch>
  truncated: boolean
}>

function executeGrepTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaGrepToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeGrepInput(input)
      if (credentialPathDenied(args.path)) {
        return khalaToolError("grep_blocked_credential_path", "grep blocked a credential-shaped search root")
      }
      const resolved = await resolveSearchTarget(args.path, context)
      if (resolved._tag === "denied") {
        return khalaToolDenied("grep_external_directory_denied", "grep outside the workspace was denied")
      }

      const search = await findGrepMatches(args, resolved, options)
      const maxMatches = options.maxMatches ?? 100
      const requestedLimit = args.limit ?? maxMatches
      const cappedLimit = Math.min(requestedLimit, maxMatches)
      const rawMatches = search.matches.slice(0, cappedLimit)
      const uiMatches = await attachContext(rawMatches, args.context, options)
      const resultTruncated = search.truncated || rawMatches.length < search.matches.length || requestedLimit > cappedLimit
      const rendered = renderMatches(uiMatches, resultTruncated, options)
      const truncated = resultTruncated || rendered.truncated
      const first = uiMatches[0]
      const firstSummary = first === undefined ? "" : ` First: ${first.file}:${first.line}: ${first.text}`

      return khalaToolOk({
        modelText: rendered.text,
        publicSummary:
          `Grep ${args.pattern}: ${rawMatches.length}/${search.matches.length} matches${truncated ? " (truncated)" : ""}.` +
          firstSummary,
        ui: {
          glob: args.glob,
          ignoreCase: args.ignoreCase,
          kind: "content_search",
          literal: args.literal,
          matches: uiMatches,
          pattern: args.pattern,
          root: resolved.displayPath,
          totalMatches: search.matches.length,
          truncated,
        },
        publicSafety: "private",
      })
    } catch (error) {
      return khalaToolError("grep_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function findGrepMatches(
  args: GrepInput,
  target: ResolvedSearchTarget,
  options: KhalaGrepToolOptions,
): Promise<GrepSearchResult> {
  const rg = await findGrepMatchesWithRipgrep(args, target, options)
  if (rg !== undefined) return rg
  return findGrepMatchesByWalking(args, target, options)
}

async function findGrepMatchesWithRipgrep(
  args: GrepInput,
  target: ResolvedSearchTarget,
  options: KhalaGrepToolOptions,
): Promise<GrepSearchResult | undefined> {
  try {
    const maxFileBytes = options.maxFileBytes ?? 1024 * 1024
    const cwd = target.kind === "file" ? dirname(target.realPath) : target.realPath
    const rootDisplayPath = target.kind === "file" ? displayDirname(target.displayPath) : target.displayPath
    const targetArgs = target.kind === "file" ? [basename(target.realPath)] : []
    const proc = Bun.spawn(
      [
        "rg",
        "--json",
        "--hidden",
        "--line-number",
        "--column",
        "--with-filename",
        "--max-filesize",
        String(maxFileBytes),
        ...(args.literal ? ["--fixed-strings"] : []),
        ...(args.ignoreCase ? ["--ignore-case"] : []),
        ...(args.glob === undefined ? [] : ["--glob", args.glob]),
        ...CREDENTIAL_RIPGREP_EXCLUDES.flatMap(glob => ["--glob", glob]),
        "--",
        args.pattern,
        ...targetArgs,
      ],
      {
        cwd,
        stderr: "pipe",
        stdout: "pipe",
      },
    )
    const stdout = await readStreamTextBounded(
      proc.stdout,
      options.maxRipgrepOutputBytes ?? 1024 * 1024,
      () => proc.kill(),
    )
    await readStreamTextBounded(proc.stderr, 8 * 1024, () => undefined)
    const exitCode = await proc.exited.catch(() => 2)
    if (exitCode !== 0 && stdout.text.trim().length === 0) {
      return exitCode === 1 ? { matches: [], truncated: stdout.truncated } : undefined
    }
    const ignore = await readGitignore(target.workspaceRoot)
    const matches = parseRipgrepJson({
      cwd,
      ignore,
      rootDisplayPath,
      stdout: stdout.text,
    })
    return {
      matches: [...matches].sort(compareMatch),
      truncated: stdout.truncated,
    }
  } catch {
    return undefined
  }
}

async function findGrepMatchesByWalking(
  args: GrepInput,
  target: ResolvedSearchTarget,
  options: KhalaGrepToolOptions,
): Promise<GrepSearchResult> {
  const matcher = makeLineMatcher(args)
  const candidates = await findCandidateFiles(args, target, options)
  const maxMatches = options.maxMatches ?? 100
  const requestedLimit = args.limit ?? maxMatches
  const cappedLimit = Math.min(requestedLimit, maxMatches)
  const outputLimit = cappedLimit + 1
  const matches: RawGrepMatch[] = []
  let searchedBytes = 0
  let truncated = false
  for (const candidate of candidates) {
    if (matches.length >= outputLimit) break
    const bytes = await readFile(candidate.realPath)
    searchedBytes += bytes.byteLength
    if (searchedBytes > (options.maxSearchBytes ?? 5 * 1024 * 1024)) {
      truncated = true
      break
    }
    if (bytes.includes(0)) continue
    const lines = bytes.toString("utf8").split(/\r?\n/u)
    for (const [index, line] of lines.entries()) {
      const match = matcher(line)
      if (match === undefined) continue
      matches.push({
        column: match.column,
        displayPath: candidate.displayPath,
        line: index + 1,
        lineText: line,
        matchText: match.matchText,
        realPath: candidate.realPath,
      })
      if (matches.length >= outputLimit) {
        truncated = true
        break
      }
    }
  }
  return {
    matches: matches.slice(0, cappedLimit).sort(compareMatch),
    truncated,
  }
}

async function findCandidateFiles(
  args: GrepInput,
  target: ResolvedSearchTarget,
  options: KhalaGrepToolOptions,
): Promise<ReadonlyArray<Readonly<{ displayPath: string; realPath: string }>>> {
  const ignore = await readGitignore(target.workspaceRoot)
  const maxFileBytes = options.maxFileBytes ?? 1024 * 1024
  if (target.kind === "file") {
    const info = await stat(target.realPath)
    if (!info.isFile() || info.size > maxFileBytes || credentialPathDenied(target.displayPath)) return []
    if (args.glob !== undefined && !pathMatchesGlob(args.glob, basename(target.displayPath))) return []
    return [{ displayPath: target.displayPath, realPath: target.realPath }]
  }

  const candidates: Array<Readonly<{ displayPath: string; realPath: string }>> = []
  const walk = async (directory: string): Promise<void> => {
    const dirents = await readdir(directory, { withFileTypes: true })
    for (const dirent of dirents) {
      const absolute = resolve(directory, dirent.name)
      const searchRelative = toPosix(relative(target.realPath, absolute))
      if (searchRelative === ".git" || searchRelative.startsWith(".git/")) continue
      const displayPath = joinDisplayPath(target.displayPath, searchRelative)
      if (credentialPathDenied(displayPath)) continue
      if (ignore(displayPath, dirent.isDirectory())) continue
      if (dirent.isDirectory()) {
        await walk(absolute)
        continue
      }
      if (!dirent.isFile()) continue
      if (args.glob !== undefined && !pathMatchesGlob(args.glob, searchRelative)) continue
      const info = await stat(absolute)
      if (!info.isFile() || info.size > maxFileBytes) continue
      candidates.push({ displayPath, realPath: absolute })
    }
  }
  await walk(target.realPath)
  return candidates.sort((a, b) => comparePath(a.displayPath, b.displayPath))
}

async function attachContext(
  matches: ReadonlyArray<RawGrepMatch>,
  requestedContext: number,
  options: KhalaGrepToolOptions,
): Promise<ReadonlyArray<GrepUiMatch>> {
  const context = Math.min(requestedContext, options.maxContextLines ?? 5)
  const maxLineLength = options.maxLineLength ?? 240
  const linesByFile = new Map<string, ReadonlyArray<string>>()
  const uiMatches: GrepUiMatch[] = []
  for (const match of matches) {
    let lines = linesByFile.get(match.realPath)
    if (lines === undefined) {
      const bytes = await readFile(match.realPath)
      lines = bytes.toString("utf8").split(/\r?\n/u)
      linesByFile.set(match.realPath, lines)
    }
    const before = contextLines(lines, Math.max(0, match.line - context - 1), match.line - 1, maxLineLength)
    const after = contextLines(lines, match.line, Math.min(lines.length, match.line + context), maxLineLength)
    const line = truncateSnippet(match.lineText, maxLineLength)
    const matched = truncateSnippet(match.matchText, maxLineLength)
    uiMatches.push({
      column: match.column,
      contextAfter: after,
      contextBefore: before,
      file: match.displayPath,
      line: match.line,
      match: matched.text,
      text: line.text,
      truncated: line.truncated || matched.truncated || before.some(item => item.truncated) || after.some(item => item.truncated),
    })
  }
  return uiMatches
}

function contextLines(
  lines: ReadonlyArray<string>,
  start: number,
  end: number,
  maxLineLength: number,
): ReadonlyArray<GrepContextLine> {
  const output: GrepContextLine[] = []
  for (let index = start; index < end; index += 1) {
    const line = lines[index]
    if (line === undefined) continue
    const snippet = truncateSnippet(line, maxLineLength)
    output.push({
      line: index + 1,
      text: snippet.text,
      truncated: snippet.truncated,
    })
  }
  return output
}

function renderMatches(
  matches: ReadonlyArray<GrepUiMatch>,
  truncated: boolean,
  options: KhalaGrepToolOptions,
): Readonly<{ text: string; truncated: boolean }> {
  if (matches.length === 0) {
    return { text: truncated ? "(no matches)\n[grep truncated; refine pattern/path/glob]" : "(no matches)", truncated }
  }
  const maxBytes = options.maxModelBytes ?? 64 * 1024
  const lines: string[] = []
  let bytes = 0
  let outputTruncated = false
  const append = (line: string): void => {
    if (outputTruncated) return
    const nextBytes = Buffer.byteLength(`${line}\n`, "utf8")
    if (bytes + nextBytes > maxBytes) {
      outputTruncated = true
      return
    }
    bytes += nextBytes
    lines.push(line)
  }
  for (const match of matches) {
    for (const context of match.contextBefore) append(`${match.file}:${context.line}- ${context.text}`)
    append(`${match.file}:${match.line}:${match.column}: ${match.text}`)
    for (const context of match.contextAfter) append(`${match.file}:${context.line}- ${context.text}`)
  }
  const finalTruncated = truncated || outputTruncated
  if (finalTruncated) append("[grep truncated; refine pattern/path/glob or increase limit]")
  return {
    text: lines.join("\n"),
    truncated: finalTruncated,
  }
}

function parseRipgrepJson(input: {
  readonly cwd: string
  readonly ignore: (path: string, directory: boolean) => boolean
  readonly rootDisplayPath: string
  readonly stdout: string
}): ReadonlyArray<RawGrepMatch> {
  const matches: RawGrepMatch[] = []
  for (const line of input.stdout.split(/\r?\n/u)) {
    if (line.trim().length === 0) continue
    const event = parseJsonRecord(line)
    if (event?.type !== "match" || !isRecord(event.data)) continue
    const path = textValue(event.data.path)
    const lineText = textValue(event.data.lines)
    const lineNumber = typeof event.data.line_number === "number" ? event.data.line_number : undefined
    if (path === undefined || lineText === undefined || lineNumber === undefined) continue
    const displayPath = joinDisplayPath(input.rootDisplayPath, toPosix(path))
    if (credentialPathDenied(displayPath) || input.ignore(displayPath, false)) continue
    const submatch = firstSubmatch(event.data.submatches)
    matches.push({
      column: submatch?.column ?? 1,
      displayPath,
      line: lineNumber,
      lineText: trimLineEnding(lineText),
      matchText: submatch?.text ?? trimLineEnding(lineText),
      realPath: resolve(input.cwd, path),
    })
  }
  return matches
}

function firstSubmatch(value: unknown): Readonly<{ column: number; text: string }> | undefined {
  if (!Array.isArray(value)) return undefined
  const submatch = value.find(isRecord)
  if (submatch === undefined) return undefined
  const matchText = textValue(submatch.match)
  const start = typeof submatch.start === "number" ? submatch.start : undefined
  if (matchText === undefined || start === undefined) return undefined
  return { column: start + 1, text: matchText }
}

async function resolveSearchTarget(
  rawPath: string,
  context: KhalaToolExecuteContext,
): Promise<ResolvedSearchTarget | Readonly<{ _tag: "denied" }>> {
  const workspaceRoot = await realpath(context.services.workspace.workingDirectory)
  const candidate = isAbsolute(rawPath) ? rawPath : resolve(workspaceRoot, rawPath)
  const target = await realpath(candidate)
  const inside = pathIsInside(workspaceRoot, target)
  if (!inside) {
    const decision = await Effect.runPromise(
      context.services.permission.decide(externalDirectoryPermission(rawPath, context)),
    )
    if (decision === "deny") return { _tag: "denied" }
  }
  const info = await stat(target)
  if (!info.isDirectory() && !info.isFile()) {
    throw new Error("grep path must be a file or directory")
  }
  return {
    _tag: "ok",
    displayPath: inside ? toWorkspaceRelative(workspaceRoot, target) : rawPath,
    kind: info.isDirectory() ? "directory" : "file",
    realPath: target,
    workspaceRoot,
  }
}

function decodeGrepInput(input: Readonly<Record<string, unknown>>): GrepInput {
  const pattern = typeof input.pattern === "string" ? input.pattern : ""
  if (pattern.length === 0) throw new Error("grep requires a non-empty pattern")
  const rawPath = input.path
  const path = typeof rawPath === "string" && rawPath.trim().length > 0 ? rawPath.trim() : "."
  const rawGlob = input.glob
  const glob = typeof rawGlob === "string" && rawGlob.trim().length > 0 ? rawGlob.trim() : undefined
  const ignoreCase = optionalBoolean(input.ignore_case, "ignore_case")
  const literal = optionalBoolean(input.literal, "literal")
  const context = optionalNonnegativeInteger(input.context, "context") ?? 0
  const limit = optionalPositiveInteger(input.limit, "limit")
  return {
    context,
    ...(glob === undefined ? {} : { glob }),
    ignoreCase: ignoreCase ?? false,
    ...(limit === undefined ? {} : { limit }),
    literal: literal ?? false,
    path,
    pattern,
  }
}

function optionalBoolean(value: unknown, field: string): boolean | undefined {
  if (value === undefined) return undefined
  if (typeof value !== "boolean") throw new Error(`grep ${field} must be a boolean`)
  return value
}

function optionalPositiveInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 1) {
    throw new Error(`grep ${field} must be a positive integer`)
  }
  return Number(value)
}

function optionalNonnegativeInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 0) {
    throw new Error(`grep ${field} must be a nonnegative integer`)
  }
  return Number(value)
}

function makeLineMatcher(args: GrepInput): (line: string) => Readonly<{ column: number; matchText: string }> | undefined {
  if (args.literal) {
    const needle = args.ignoreCase ? args.pattern.toLocaleLowerCase() : args.pattern
    return line => {
      const haystack = args.ignoreCase ? line.toLocaleLowerCase() : line
      const index = haystack.indexOf(needle)
      if (index === -1) return undefined
      return { column: index + 1, matchText: line.slice(index, index + args.pattern.length) }
    }
  }
  const regex = new RegExp(args.pattern, args.ignoreCase ? "iu" : "u")
  return line => {
    const match = regex.exec(line)
    if (match === null) return undefined
    return { column: match.index + 1, matchText: match[0] }
  }
}

async function readStreamTextBounded(
  stream: ReadableStream<Uint8Array>,
  maxBytes: number,
  onTruncate: () => void,
): Promise<Readonly<{ text: string; truncated: boolean }>> {
  const reader = stream.getReader()
  const chunks: Buffer[] = []
  let bytes = 0
  let truncated = false
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const chunk = read.value
    const remaining = maxBytes - bytes
    if (remaining <= 0 || chunk.byteLength > remaining) {
      if (remaining > 0) chunks.push(Buffer.from(chunk.slice(0, remaining)))
      truncated = true
      onTruncate()
      break
    }
    bytes += chunk.byteLength
    chunks.push(Buffer.from(chunk))
  }
  return {
    text: Buffer.concat(chunks).toString("utf8"),
    truncated,
  }
}

async function readGitignore(workspaceRoot: string): Promise<(path: string, directory: boolean) => boolean> {
  try {
    const raw = await readFile(resolve(workspaceRoot, ".gitignore"), "utf8")
    const patterns = raw
      .split(/\r?\n/u)
      .map(line => line.trim())
      .filter(line => line.length > 0 && !line.startsWith("#") && !line.startsWith("!"))
    return (path, directory) => patterns.some(pattern => gitignorePatternMatches(pattern, path, directory))
  } catch {
    return () => false
  }
}

function gitignorePatternMatches(pattern: string, path: string, directory: boolean): boolean {
  if (pattern.endsWith("/")) {
    const prefix = pattern.slice(0, -1)
    return directory && (path === prefix || path.startsWith(`${prefix}/`)) || path.startsWith(`${prefix}/`)
  }
  const target = pattern.includes("/") ? path : basename(path)
  return globToRegExp(pattern).test(target)
}

function pathMatchesGlob(pattern: string, path: string): boolean {
  const target = pattern.includes("/") ? path : basename(path)
  return globToRegExp(pattern).test(target)
}

function globToRegExp(pattern: string): RegExp {
  let source = "^"
  for (let index = 0; index < pattern.length; index += 1) {
    const char = pattern[index]
    const next = pattern[index + 1]
    if (char === "*" && next === "*") {
      source += ".*"
      index += 1
    } else if (char === "*") {
      source += "[^/]*"
    } else if (char === "?") {
      source += "[^/]"
    } else {
      source += escapeRegExp(char ?? "")
    }
  }
  return new RegExp(`${source}$`, "u")
}

function escapeRegExp(value: string): string {
  return value.replace(/[|\\{}()[\]^$+*?.]/gu, "\\$&")
}

function externalDirectoryPermission(rawPath: string, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "external_directory",
    authorityMode: "local",
    publicSafety: "private",
    resources: [rawPath],
    saveScope: "once",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "grep",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function pathIsInside(root: string, target: string): boolean {
  const rel = relative(root, target)
  return rel === "" || !rel.startsWith("..") && !isAbsolute(rel)
}

function toWorkspaceRelative(root: string, target: string): string {
  const rel = relative(root, target)
  return rel === "" ? "." : toPosix(rel)
}

function joinDisplayPath(rootDisplayPath: string, child: string): string {
  if (rootDisplayPath === ".") return child
  if (child.length === 0) return rootDisplayPath
  return `${rootDisplayPath}/${child}`
}

function displayDirname(path: string): string {
  const index = path.lastIndexOf("/")
  if (index === -1) return "."
  if (index === 0) return "/"
  return path.slice(0, index)
}

function toPosix(path: string): string {
  return path.split("\\").join("/")
}

function trimLineEnding(value: string): string {
  return value.replace(/\r?\n$/u, "")
}

function truncateSnippet(value: string, maxLength: number): Readonly<{ text: string; truncated: boolean }> {
  if (value.length <= maxLength) return { text: value, truncated: false }
  return { text: `${value.slice(0, Math.max(0, maxLength - 15))}[truncated]`, truncated: true }
}

function compareMatch(a: RawGrepMatch, b: RawGrepMatch): number {
  return comparePath(a.displayPath, b.displayPath) || a.line - b.line || a.column - b.column
}

function comparePath(a: string, b: string): number {
  return a.toLowerCase().localeCompare(b.toLowerCase()) || a.localeCompare(b)
}

function parseJsonRecord(value: string): Readonly<Record<string, unknown>> | undefined {
  try {
    const parsed = JSON.parse(value)
    return isRecord(parsed) ? parsed : undefined
  } catch {
    return undefined
  }
}

function textValue(value: unknown): string | undefined {
  if (!isRecord(value)) return undefined
  return typeof value.text === "string" ? value.text : undefined
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

const CREDENTIAL_BASENAMES = new Set([".env", ".secrets", ".npmrc", "auth.json", "provider-key.json"])

const CREDENTIAL_RIPGREP_EXCLUDES = [
  "!.env",
  "!**/.env",
  "!.npmrc",
  "!**/.npmrc",
  "!auth.json",
  "!**/auth.json",
  "!provider-key.json",
  "!**/provider-key.json",
  "!.secrets/**",
  "!**/.secrets/**",
] as const

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
