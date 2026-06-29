import { realpath, readdir, stat } from "node:fs/promises"
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

export interface KhalaLsToolOptions {
  readonly maxEntries?: number
}

export const lsToolDefinition: KhalaToolDefinition = {
  authority: "read",
  availability: ["inspect", "coding", "owner_local_full"],
  description: "List one directory page from the active workspace with bounded, structured entries.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      limit: {
        description: "Maximum number of entries to return.",
        minimum: 1,
        type: "integer",
      },
      path: {
        description: "Workspace-relative directory path or approved absolute directory path.",
        type: "string",
      },
    },
    type: "object",
  },
  internalId: "khala.file.list",
  label: "List Directory",
  name: "ls",
  outputSchema: {
    additionalProperties: false,
    properties: {
      entries: { type: "array" },
      path: { type: "string" },
      totalEntries: { type: "integer" },
      truncated: { type: "boolean" },
    },
    required: ["path", "entries", "totalEntries", "truncated"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "List one directory. Use read for files.",
  promptGuidelines: [
    "Pass a workspace-relative path whenever possible.",
    "Use limit for large directories.",
    "Do not spend shell authority on routine directory listings.",
  ],
  renderer: { kind: "directory_list", rendererRef: "khala.renderer.directory_list.v1" },
}

export function createLsTool(options: KhalaLsToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: lsToolDefinition,
    execute: (input, context) => executeLsTool(input, context, options),
  }
}

type LsInput = Readonly<{
  limit?: number
  path: string
}>

type LsEntry = Readonly<{
  kind: "directory" | "file" | "symlink" | "other"
  name: string
}>

function executeLsTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaLsToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeLsInput(input)
      if (credentialPathDenied(args.path)) {
        return khalaToolError("ls_blocked_credential_path", "ls blocked a credential-shaped directory path")
      }
      const resolved = await resolveDirectoryPath(args.path, context)
      if (resolved._tag === "denied") {
        return khalaToolDenied("ls_external_directory_denied", "listing outside the workspace was denied")
      }
      const info = await stat(resolved.realPath)
      if (!info.isDirectory()) {
        return khalaToolError("ls_not_directory", "ls only lists directories")
      }

      const dirents = await readdir(resolved.realPath, { withFileTypes: true })
      const sorted = dirents
        .map(dirent => ({
          kind: dirent.isDirectory()
            ? "directory"
            : dirent.isFile()
              ? "file"
              : dirent.isSymbolicLink()
                ? "symlink"
                : "other",
          name: `${dirent.name}${dirent.isDirectory() ? "/" : ""}`,
        }) satisfies LsEntry)
        .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()) || a.name.localeCompare(b.name))

      const maxEntries = options.maxEntries ?? 200
      const requestedLimit = args.limit ?? maxEntries
      const cappedLimit = Math.min(requestedLimit, maxEntries)
      const entries = sorted.slice(0, cappedLimit)
      const truncated = entries.length < sorted.length || requestedLimit > cappedLimit
      const modelText = renderEntries(resolved.displayPath, entries, truncated)
      return khalaToolOk({
        modelText,
        publicSummary: `Listed ${resolved.displayPath}: ${entries.length}/${sorted.length} entries${truncated ? " (truncated)" : ""}.`,
        ui: {
          displayPath: resolved.displayPath,
          entries,
          kind: "directory_list",
          totalEntries: sorted.length,
          truncated,
        },
        publicSafety: "private",
      })
    } catch (error) {
      return khalaToolError("ls_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function resolveDirectoryPath(
  rawPath: string,
  context: KhalaToolExecuteContext,
): Promise<
  | Readonly<{
      _tag: "ok"
      displayPath: string
      realPath: string
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
  }
}

function decodeLsInput(input: Readonly<Record<string, unknown>>): LsInput {
  const rawPath = input.path
  const path = typeof rawPath === "string" && rawPath.trim().length > 0 ? rawPath.trim() : "."
  const limit = optionalPositiveInteger(input.limit, "limit")
  return {
    path,
    ...(limit === undefined ? {} : { limit }),
  }
}

function optionalPositiveInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 1) {
    throw new Error(`ls ${field} must be a positive integer`)
  }
  return Number(value)
}

function renderEntries(path: string, entries: ReadonlyArray<LsEntry>, truncated: boolean): string {
  const body = entries.length === 0 ? "(empty)" : entries.map(entry => entry.name).join("\n")
  return `${path}:\n${body}${truncated ? "\n[ls truncated; refine path or increase limit]" : ""}`
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
    toolName: "ls",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function pathIsInside(root: string, target: string): boolean {
  const rel = relative(root, target)
  return rel === "" || !rel.startsWith("..") && !isAbsolute(rel)
}

function toWorkspaceRelative(root: string, target: string): string {
  const rel = relative(root, target)
  return rel === "" ? "." : rel.split("\\").join("/")
}

const CREDENTIAL_BASENAMES = new Set([".env", ".secrets", ".npmrc", "auth.json", "provider-key.json"])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
