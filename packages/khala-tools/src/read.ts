import { readFile, realpath, stat } from "node:fs/promises"
import { basename, extname, isAbsolute, relative, resolve } from "node:path"
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

export interface KhalaReadToolOptions {
  readonly maxBytes?: number
  readonly maxLines?: number
}

export const readToolDefinition: KhalaToolDefinition = {
  authority: "read",
  availability: ["inspect", "coding", "owner_local_full"],
  description: "Read a text file from the active workspace with line-numbered, bounded output.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      limit: {
        description: "Maximum number of lines to return.",
        minimum: 1,
        type: "integer",
      },
      offset: {
        description: "1-indexed line offset.",
        minimum: 1,
        type: "integer",
      },
      path: {
        description: "Workspace-relative path or approved absolute path.",
        type: "string",
      },
    },
    required: ["path"],
    type: "object",
  },
  internalId: "khala.file.read",
  label: "Read File",
  name: "read",
  outputSchema: {
    additionalProperties: false,
    properties: {
      lineEnd: { type: "integer" },
      lineStart: { type: "integer" },
      path: { type: "string" },
      text: { type: "string" },
      truncated: { type: "boolean" },
    },
    required: ["path", "text", "lineStart", "lineEnd", "truncated"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Read one text file. Use ls for directories and view_image for images.",
  promptGuidelines: [
    "Pass a workspace-relative path whenever possible.",
    "Use offset and limit for large files.",
    "Do not use read for directories, device files, or credential paths.",
  ],
  renderer: { kind: "file_read", rendererRef: "khala.renderer.file_read.v1" },
}

export function createReadTool(options: KhalaReadToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: readToolDefinition,
    execute: (input, context) => executeReadTool(input, context, options),
  }
}

type ReadInput = Readonly<{
  limit?: number
  offset?: number
  path: string
}>

function executeReadTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaReadToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(
    async () => {
      try {
        const args = decodeReadInput(input)
        if (credentialPathDenied(args.path)) {
          return khalaToolError("read_blocked_credential_path", "read blocked a credential-shaped path")
        }
        const resolved = await resolveReadablePath(args.path, context)
        if (resolved._tag === "denied") {
          return khalaToolDenied("read_external_directory_denied", "read outside the workspace was denied")
        }

        const info = await stat(resolved.realPath)
        if (!info.isFile()) {
          return khalaToolError("read_blocked_file_type", "read only supports regular files")
        }
        if (imageExtension(args.path)) {
          return khalaToolOk({
            modelText: `Image-like file detected at ${resolved.displayPath}. Use view_image for visual inspection.`,
            publicSummary: `Image-like file detected at ${resolved.displayPath}; text bytes were not returned.`,
            ui: {
              displayPath: resolved.displayPath,
              kind: "image_hint",
            },
          })
        }

        const bytes = await readFile(resolved.realPath)
        if (bytes.includes(0)) {
          return khalaToolError("read_blocked_binary", "read only supports text files")
        }
        const text = bytes.toString("utf8")
        const bounded = boundText(text, args, options)
        return khalaToolOk({
          modelText: bounded.modelText,
          publicSummary: `Read ${resolved.displayPath} lines ${bounded.lineStart}-${bounded.lineEnd}${bounded.truncated ? " (truncated)" : ""}.`,
          ui: {
            byteLength: bytes.byteLength,
            displayPath: resolved.displayPath,
            kind: "text",
            lineEnd: bounded.lineEnd,
            lineStart: bounded.lineStart,
            totalLines: bounded.totalLines,
            truncated: bounded.truncated,
          },
          publicSafety: "private",
        })
      } catch (error) {
        return khalaToolError("read_failed", error instanceof Error ? error.message : String(error))
      }
    },
  )
}

async function resolveReadablePath(
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

function decodeReadInput(input: Readonly<Record<string, unknown>>): ReadInput {
  const path = typeof input.path === "string" ? input.path.trim() : ""
  if (path.length === 0) throw new Error("read requires a non-empty path")
  const offset = optionalPositiveInteger(input.offset, "offset")
  const limit = optionalPositiveInteger(input.limit, "limit")
  return {
    path,
    ...(offset === undefined ? {} : { offset }),
    ...(limit === undefined ? {} : { limit }),
  }
}

function optionalPositiveInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 1) {
    throw new Error(`read ${field} must be a positive integer`)
  }
  return Number(value)
}

function boundText(
  text: string,
  args: ReadInput,
  options: KhalaReadToolOptions,
): Readonly<{
  lineEnd: number
  lineStart: number
  modelText: string
  totalLines: number
  truncated: boolean
}> {
  const maxBytes = options.maxBytes ?? 50 * 1024
  const maxLines = options.maxLines ?? 2_000
  const lines = text.split(/\r?\n/u)
  const lineStart = args.offset ?? 1
  const requestedLimit = args.limit ?? maxLines
  const cappedLimit = Math.min(requestedLimit, maxLines)
  const selected = lines.slice(lineStart - 1, lineStart - 1 + cappedLimit)
  const numbered: string[] = []
  let bytes = 0
  let byteTruncated = false
  for (const [index, line] of selected.entries()) {
    const rendered = `${lineStart + index}: ${line}`
    const nextBytes = Buffer.byteLength(`${rendered}\n`, "utf8")
    if (bytes + nextBytes > maxBytes) {
      byteTruncated = true
      break
    }
    bytes += nextBytes
    numbered.push(rendered)
  }
  const lineEnd = numbered.length === 0 ? lineStart - 1 : lineStart + numbered.length - 1
  const lineTruncated = lineStart - 1 + selected.length < lines.length || selected.length > numbered.length
  const truncated = byteTruncated || lineTruncated || requestedLimit > cappedLimit
  const hint = truncated ? `\n[read truncated; continue with offset ${lineEnd + 1}]` : ""
  return {
    lineEnd,
    lineStart,
    modelText: `${numbered.join("\n")}${hint}`,
    totalLines: lines.length,
    truncated,
  }
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
    toolName: "read",
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

const IMAGE_EXTENSIONS = new Set([".gif", ".jpeg", ".jpg", ".png", ".webp"])

function imageExtension(path: string): boolean {
  return IMAGE_EXTENSIONS.has(extname(path).toLowerCase())
}

const CREDENTIAL_BASENAMES = new Set([
  ".env",
  ".npmrc",
  "auth.json",
  "id_ed25519",
  "id_rsa",
  "provider-key.json",
])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
