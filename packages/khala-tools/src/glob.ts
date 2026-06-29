import { readFile, realpath, readdir, stat } from "node:fs/promises"
import { basename, isAbsolute, relative, resolve } from "node:path"
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

export interface KhalaGlobToolOptions {
  readonly maxMatches?: number
}

export const globToolDefinition: KhalaToolDefinition = {
  authority: "search",
  availability: ["inspect", "coding", "owner_local_full"],
  description: "Find workspace paths by glob pattern with bounded, ignore-aware output.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      limit: {
        description: "Maximum number of paths to return.",
        minimum: 1,
        type: "integer",
      },
      path: {
        description: "Workspace-relative search root or approved absolute directory path.",
        type: "string",
      },
      pattern: {
        description: "Glob pattern, for example **/*.ts.",
        type: "string",
      },
    },
    required: ["pattern"],
    type: "object",
  },
  internalId: "khala.search.glob",
  label: "Find Paths",
  name: "glob",
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
  prompt: "Find paths by glob. Use grep for content search.",
  promptGuidelines: [
    "Use **/*.ext for recursive file discovery.",
    "Use path to scope large searches.",
    "Do not merge path glob and content grep into one query.",
  ],
  renderer: { kind: "path_glob", rendererRef: "khala.renderer.path_glob.v1" },
}

export function createGlobTool(options: KhalaGlobToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: globToolDefinition,
    execute: (input, context) => executeGlobTool(input, context, options),
  }
}

type GlobInput = Readonly<{
  limit?: number
  path: string
  pattern: string
}>

function executeGlobTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaGlobToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeGlobInput(input)
      if (credentialPathDenied(args.path)) {
        return khalaToolError("glob_blocked_credential_path", "glob blocked a credential-shaped search root")
      }
      const resolved = await resolveSearchRoot(args.path, context)
      if (resolved._tag === "denied") {
        return khalaToolDenied("glob_external_directory_denied", "glob outside the workspace was denied")
      }
      const info = await stat(resolved.realPath)
      if (!info.isDirectory()) {
        return khalaToolError("glob_not_directory", "glob path must be a directory")
      }

      const maxMatches = options.maxMatches ?? 200
      const requestedLimit = args.limit ?? maxMatches
      const cappedLimit = Math.min(requestedLimit, maxMatches)
      const allMatches = await findGlobMatches({
        pattern: args.pattern,
        rootDisplayPath: resolved.displayPath,
        searchRoot: resolved.realPath,
        workspaceRoot: resolved.workspaceRoot,
      })
      const matches = allMatches.slice(0, cappedLimit)
      const truncated = matches.length < allMatches.length || requestedLimit > cappedLimit

      return khalaToolOk({
        modelText: renderMatches(matches, truncated),
        publicSummary: `Glob ${args.pattern}: ${matches.length}/${allMatches.length} matches${truncated ? " (truncated)" : ""}.`,
        ui: {
          kind: "path_glob",
          matches,
          pattern: args.pattern,
          root: resolved.displayPath,
          totalMatches: allMatches.length,
          truncated,
        },
        publicSafety: "private",
      })
    } catch (error) {
      return khalaToolError("glob_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function findGlobMatches(input: {
  readonly pattern: string
  readonly rootDisplayPath: string
  readonly searchRoot: string
  readonly workspaceRoot: string
}): Promise<ReadonlyArray<string>> {
  const rg = await findGlobMatchesWithRipgrep(input)
  if (rg !== undefined) return rg
  return findGlobMatchesByWalking(input)
}

async function findGlobMatchesWithRipgrep(input: {
  readonly pattern: string
  readonly rootDisplayPath: string
  readonly searchRoot: string
  readonly workspaceRoot: string
}): Promise<ReadonlyArray<string> | undefined> {
  try {
    const proc = Bun.spawn(
      ["rg", "--files", "--hidden", "--glob", input.pattern, "--glob", "!.git/**"],
      {
        cwd: input.searchRoot,
        stderr: "pipe",
        stdout: "pipe",
      },
    )
    const [stdout, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      proc.exited,
    ])
    if (exitCode !== 0 && stdout.trim().length === 0) return []
    const ignore = await readGitignore(input.workspaceRoot)
    return stdout
      .split(/\r?\n/u)
      .map(line => line.trim())
      .filter(line => line.length > 0)
      .filter(path => !ignore(joinDisplayPath(input.rootDisplayPath, toPosix(path)), false))
      .map(path => joinDisplayPath(input.rootDisplayPath, toPosix(path)))
      .sort(comparePath)
  } catch {
    return undefined
  }
}

async function findGlobMatchesByWalking(input: {
  readonly pattern: string
  readonly rootDisplayPath: string
  readonly searchRoot: string
  readonly workspaceRoot: string
}): Promise<ReadonlyArray<string>> {
  const ignore = await readGitignore(input.workspaceRoot)
  const matches: string[] = []
  const pattern = globToRegExp(input.pattern)
  const patternHasSlash = input.pattern.includes("/")
  const walk = async (directory: string): Promise<void> => {
    const dirents = await readdir(directory, { withFileTypes: true })
    for (const dirent of dirents) {
      const absolute = resolve(directory, dirent.name)
      const searchRelative = toPosix(relative(input.searchRoot, absolute))
      if (searchRelative === ".git" || searchRelative.startsWith(".git/")) continue
      if (ignore(searchRelative, dirent.isDirectory())) continue
      const matchTarget = patternHasSlash ? searchRelative : basename(searchRelative)
      if (!dirent.isDirectory() && pattern.test(matchTarget)) {
        matches.push(joinDisplayPath(input.rootDisplayPath, searchRelative))
      }
      if (dirent.isDirectory()) {
        await walk(absolute)
      }
    }
  }
  await walk(input.searchRoot)
  return matches.sort(comparePath)
}

async function resolveSearchRoot(
  rawPath: string,
  context: KhalaToolExecuteContext,
): Promise<
  | Readonly<{
      _tag: "ok"
      displayPath: string
      realPath: string
      workspaceRoot: string
    }>
  | Readonly<{ _tag: "denied" }>
> {
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
  return {
    _tag: "ok",
    displayPath: inside ? toWorkspaceRelative(workspaceRoot, target) : rawPath,
    realPath: target,
    workspaceRoot,
  }
}

function decodeGlobInput(input: Readonly<Record<string, unknown>>): GlobInput {
  const pattern = typeof input.pattern === "string" ? input.pattern.trim() : ""
  if (pattern.length === 0) throw new Error("glob requires a non-empty pattern")
  const rawPath = input.path
  const path = typeof rawPath === "string" && rawPath.trim().length > 0 ? rawPath.trim() : "."
  const limit = optionalPositiveInteger(input.limit, "limit")
  return {
    path,
    pattern,
    ...(limit === undefined ? {} : { limit }),
  }
}

function optionalPositiveInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 1) {
    throw new Error(`glob ${field} must be a positive integer`)
  }
  return Number(value)
}

function renderMatches(matches: ReadonlyArray<string>, truncated: boolean): string {
  const body = matches.length === 0 ? "(no matches)" : matches.join("\n")
  return `${body}${truncated ? "\n[glob truncated; refine pattern/path or increase limit]" : ""}`
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
    toolName: "glob",
    workingDirectory: context.services.workspace.workingDirectory,
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

function toPosix(path: string): string {
  return path.split("\\").join("/")
}

function comparePath(a: string, b: string): number {
  return a.toLowerCase().localeCompare(b.toLowerCase()) || a.localeCompare(b)
}

const CREDENTIAL_BASENAMES = new Set([".env", ".secrets", ".npmrc", "auth.json", "provider-key.json"])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
