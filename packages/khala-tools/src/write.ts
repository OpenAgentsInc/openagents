import { createHash } from "node:crypto"
import { mkdir, readFile, realpath, stat, writeFile } from "node:fs/promises"
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

export interface KhalaWriteToolOptions {
  readonly maxDiffLines?: number
  readonly maxFileBytes?: number
}

export const writeToolDefinition: KhalaToolDefinition = {
  authority: "write",
  availability: ["coding", "owner_local_full"],
  description: "Create or intentionally overwrite one UTF-8 text file with stale-version guards.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      content: {
        description: "Full UTF-8 text content to write.",
        type: "string",
      },
      expected_content: {
        description: "Expected current text content for guarded overwrites.",
        type: "string",
      },
      expected_sha256: {
        description: "Expected SHA-256 of current file bytes for guarded overwrites.",
        type: "string",
      },
      path: {
        description: "Workspace-relative file path or approved absolute path.",
        type: "string",
      },
    },
    required: ["path", "content"],
    type: "object",
  },
  internalId: "khala.file.write",
  label: "Write File",
  name: "write",
  outputSchema: {
    additionalProperties: false,
    properties: {
      bytesWritten: { type: "integer" },
      diff: { type: "string" },
      existed: { type: "boolean" },
      path: { type: "string" },
    },
    required: ["path", "bytesWritten", "existed", "diff"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Create or rewrite a whole file. Use edit for localized exact replacements.",
  promptGuidelines: [
    "Use write mainly for new files or deliberate full rewrites.",
    "Pass expected_sha256 or expected_content when overwriting an existing file.",
    "Do not expose private file contents in public summaries.",
  ],
  renderer: { kind: "file_write", rendererRef: "khala.renderer.file_write.v1" },
}

export function createWriteTool(options: KhalaWriteToolOptions = {}): RegisteredKhalaTool {
  const queues = new Map<string, Promise<void>>()
  return {
    definition: writeToolDefinition,
    execute: (input, context) => executeWriteTool(input, context, options, queues),
  }
}

type WriteInput = Readonly<{
  content: string
  expectedContent?: string
  expectedSha256?: string
  path: string
}>

type ResolvedWritablePath = Readonly<{
  _tag: "ok"
  displayPath: string
  existed: boolean
  parentRealPath: string
  realPath: string
}>

type WritePlan = Readonly<{
  afterHash: string
  beforeHash?: string
  bytes: Buffer
  bytesWritten: number
  diff: string
  diffTruncated: boolean
  existed: boolean
  firstChangedLine: number
}>

function executeWriteTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaWriteToolOptions,
  queues: Map<string, Promise<void>>,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeWriteInput(input)
      if (credentialPathDenied(args.path)) {
        return khalaToolError("write_blocked_credential_path", "write blocked a credential-shaped path")
      }
      const resolved = await resolveWritablePath(args.path, context)
      if (resolved._tag === "denied") {
        return khalaToolDenied("write_external_file_denied", "write outside the workspace was denied")
      }
      return await withPathQueue(queues, resolved.realPath, () => writeResolvedFile(args, resolved, context, options))
    } catch (error) {
      return khalaToolError("write_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function writeResolvedFile(
  args: WriteInput,
  resolved: ResolvedWritablePath,
  context: KhalaToolExecuteContext,
  options: KhalaWriteToolOptions,
): Promise<KhalaToolResult> {
  const before = resolved.existed ? await readExistingText(resolved.realPath, options) : undefined
  if (before !== undefined) {
    if (args.expectedSha256 === undefined && args.expectedContent === undefined) {
      return khalaToolError(
        "write_expected_version_required",
        "overwriting an existing file requires expected_sha256 or expected_content",
      )
    }
    if (args.expectedSha256 !== undefined && args.expectedSha256 !== before.hash) {
      return khalaToolError("write_stale_file", "current file SHA-256 does not match expected_sha256")
    }
    if (args.expectedContent !== undefined && args.expectedContent !== before.text) {
      return khalaToolError("write_stale_file", "current file content does not match expected_content")
    }
  }
  const plan = planWrite(args, resolved, before, options)
  const decision = await Effect.runPromise(
    context.services.permission.decide(writePermission(args.path, resolved.displayPath, context)),
  )
  if (decision === "deny") {
    return khalaToolDenied("write_denied", "write approval was denied")
  }

  const latestExists = await fileExists(resolved.realPath)
  if (!resolved.existed && latestExists) {
    return khalaToolError("write_stale_file", "file appeared after write preview; retry with expected version")
  }
  if (resolved.existed) {
    const latest = await readExistingText(resolved.realPath, options)
    if (latest.hash !== before?.hash) {
      return khalaToolError("write_stale_file", "file changed after write preview; retry with fresh content")
    }
  }
  await mkdir(resolved.parentRealPath, { recursive: true })
  await writeFile(resolved.realPath, plan.bytes)
  return writeSuccessResult(args, resolved, plan)
}

function planWrite(
  args: WriteInput,
  resolved: ResolvedWritablePath,
  before: ExistingText | undefined,
  options: KhalaWriteToolOptions,
): WritePlan {
  const bytes = encodeContent(args.content, before?.bom ?? false)
  if (bytes.byteLength > (options.maxFileBytes ?? 2 * 1024 * 1024)) {
    throw new Error("write content exceeds maximum writable size")
  }
  const beforeText = before?.text ?? ""
  const diff = unifiedDiff(resolved.displayPath, normalizeLineEndings(beforeText), normalizeLineEndings(args.content), options.maxDiffLines ?? 200)
  return {
    afterHash: sha256Hex(bytes),
    ...(before === undefined ? {} : { beforeHash: before.hash }),
    bytes,
    bytesWritten: bytes.byteLength,
    diff: diff.text,
    diffTruncated: diff.truncated,
    existed: before !== undefined,
    firstChangedLine: firstChangedLine(normalizeLineEndings(beforeText), normalizeLineEndings(args.content)),
  }
}

function writeSuccessResult(
  args: WriteInput,
  resolved: ResolvedWritablePath,
  plan: WritePlan,
): KhalaToolResult {
  const modelText = [
    `${plan.existed ? "Overwrote" : "Wrote"} ${resolved.displayPath}`,
    `Bytes written: ${plan.bytesWritten}`,
    `First changed line: ${plan.firstChangedLine}`,
    "```diff",
    plan.diff,
    "```",
  ].join("\n")
  return khalaToolOk({
    modelText,
    publicSummary:
      `${plan.existed ? "Overwrote" : "Wrote"} ${resolved.displayPath}: ${plan.bytesWritten} bytes` +
      `${plan.existed ? ", 1 diff receipt" : ""}${plan.diffTruncated ? " (diff truncated)" : ""}.`,
    ui: {
      afterSha256: plan.afterHash,
      approval: {
        action: "write",
        resource: resolved.displayPath,
      },
      beforeSha256: plan.beforeHash,
      bytesWritten: plan.bytesWritten,
      diff: plan.diff,
      diffTruncated: plan.diffTruncated,
      existed: plan.existed,
      firstChangedLine: plan.firstChangedLine,
      kind: "file_write",
      path: resolved.displayPath,
      requestedPath: args.path,
    },
    publicSafety: "private",
  })
}

type ExistingText = Readonly<{
  bom: boolean
  hash: string
  text: string
}>

async function readExistingText(realPath: string, options: KhalaWriteToolOptions): Promise<ExistingText> {
  const info = await stat(realPath)
  if (!info.isFile()) throw new Error("write path must be a regular file")
  if (info.size > (options.maxFileBytes ?? 2 * 1024 * 1024)) {
    throw new Error("write file exceeds maximum writable size")
  }
  const bytes = await readFile(realPath)
  if (bytes.includes(0)) throw new Error("write only supports text files")
  const bom = hasUtf8Bom(bytes)
  return {
    bom,
    hash: sha256Hex(bytes),
    text: (bom ? bytes.subarray(3) : bytes).toString("utf8"),
  }
}

async function resolveWritablePath(
  rawPath: string,
  context: KhalaToolExecuteContext,
): Promise<ResolvedWritablePath | Readonly<{ _tag: "denied" }>> {
  const workspaceRoot = await realpath(context.services.workspace.workingDirectory)
  const candidate = isAbsolute(rawPath) ? resolve(rawPath) : resolve(workspaceRoot, rawPath)
  const existingTarget = await realpath(candidate).then(
    value => value,
    () => undefined,
  )
  const existed = existingTarget !== undefined
  const parentCandidate = dirname(candidate)
  const target = existingTarget ?? await resolveFutureTarget(parentCandidate, candidate)
  const inside = pathIsInside(workspaceRoot, target)
  if (!inside) {
    const decision = await Effect.runPromise(
      context.services.permission.decide(externalDirectoryPermission(rawPath, context)),
    )
    if (decision === "deny") return { _tag: "denied" }
  }
  if (existed) {
    const info = await stat(target)
    if (!info.isFile()) throw new Error("write path must be a regular file")
  }
  return {
    _tag: "ok",
    displayPath: inside ? toWorkspaceRelative(workspaceRoot, target) : rawPath,
    existed,
    parentRealPath: dirname(target),
    realPath: target,
  }
}

async function resolveFutureTarget(parentCandidate: string, candidate: string): Promise<string> {
  const nearest = await nearestExistingParent(parentCandidate)
  const nearestReal = await realpath(nearest)
  return resolve(nearestReal, relative(nearest, candidate))
}

async function nearestExistingParent(path: string): Promise<string> {
  let current = path
  while (true) {
    try {
      const info = await stat(current)
      if (!info.isDirectory()) throw new Error("write parent path is not a directory")
      return current
    } catch (error) {
      if (error instanceof Error && "code" in error && error.code !== "ENOENT") throw error
      const next = dirname(current)
      if (next === current) throw new Error("write could not resolve an existing parent directory")
      current = next
    }
  }
}

function decodeWriteInput(input: Readonly<Record<string, unknown>>): WriteInput {
  const path = typeof input.path === "string" ? input.path.trim() : ""
  if (path.length === 0) throw new Error("write requires a non-empty path")
  if (typeof input.content !== "string") throw new Error("write content must be a string")
  const expectedSha256 = typeof input.expected_sha256 === "string" && input.expected_sha256.trim().length > 0
    ? input.expected_sha256.trim()
    : undefined
  const expectedContent = typeof input.expected_content === "string" ? input.expected_content : undefined
  return {
    content: input.content,
    ...(expectedContent === undefined ? {} : { expectedContent }),
    ...(expectedSha256 === undefined ? {} : { expectedSha256 }),
    path,
  }
}

function encodeContent(content: string, preserveBom: boolean): Buffer {
  const text = content.startsWith("\ufeff") ? content.slice(1) : content
  const bytes = Buffer.from(text, "utf8")
  return preserveBom ? Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), bytes]) : bytes
}

function hasUtf8Bom(bytes: Buffer): boolean {
  return bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf
}

function normalizeLineEndings(value: string): string {
  return value.replace(/\r\n/gu, "\n").replace(/\r/gu, "\n")
}

function firstChangedLine(before: string, after: string): number {
  const beforeLines = before.split("\n")
  const afterLines = after.split("\n")
  const count = Math.max(beforeLines.length, afterLines.length)
  for (let index = 0; index < count; index += 1) {
    if (beforeLines[index] !== afterLines[index]) return index + 1
  }
  return 1
}

function unifiedDiff(
  path: string,
  before: string,
  after: string,
  maxLines: number,
): Readonly<{ text: string; truncated: boolean }> {
  const beforeLines = before.split("\n")
  const afterLines = after.split("\n")
  let prefix = 0
  while (prefix < beforeLines.length && prefix < afterLines.length && beforeLines[prefix] === afterLines[prefix]) {
    prefix += 1
  }
  let suffix = 0
  while (
    suffix < beforeLines.length - prefix &&
    suffix < afterLines.length - prefix &&
    beforeLines[beforeLines.length - suffix - 1] === afterLines[afterLines.length - suffix - 1]
  ) {
    suffix += 1
  }
  const context = 3
  const oldStart = Math.max(0, prefix - context)
  const newStart = Math.max(0, prefix - context)
  const oldEnd = Math.min(beforeLines.length, beforeLines.length - suffix + context)
  const newEnd = Math.min(afterLines.length, afterLines.length - suffix + context)
  const removed = beforeLines.slice(oldStart, oldEnd)
  const added = afterLines.slice(newStart, newEnd)
  const lines = [
    `--- a/${path}`,
    `+++ b/${path}`,
    `@@ -${oldStart + 1},${removed.length} +${newStart + 1},${added.length} @@`,
  ]
  for (let index = 0; index < Math.max(removed.length, added.length); index += 1) {
    const oldLine = removed[index]
    const newLine = added[index]
    if (oldLine !== undefined && newLine !== undefined && oldLine === newLine) {
      lines.push(` ${oldLine}`)
      continue
    }
    if (oldLine !== undefined) lines.push(`-${oldLine}`)
    if (newLine !== undefined) lines.push(`+${newLine}`)
  }
  const truncated = lines.length > maxLines
  return {
    text: `${lines.slice(0, maxLines).join("\n")}${truncated ? "\n[diff truncated]" : ""}`,
    truncated,
  }
}

async function fileExists(path: string): Promise<boolean> {
  try {
    await stat(path)
    return true
  } catch {
    return false
  }
}

async function withPathQueue<T>(
  queues: Map<string, Promise<void>>,
  path: string,
  task: () => Promise<T>,
): Promise<T> {
  const previous = queues.get(path) ?? Promise.resolve()
  const current = previous.catch(() => undefined).then(task)
  const release = current.then(() => undefined, () => undefined)
  queues.set(path, release)
  try {
    return await current
  } finally {
    if (queues.get(path) === release) queues.delete(path)
  }
}

function writePermission(
  rawPath: string,
  displayPath: string,
  context: KhalaToolExecuteContext,
): KhalaPermissionRequest {
  return {
    action: "write",
    authorityMode: "local",
    publicSafety: "private",
    resources: [displayPath, rawPath],
    saveScope: "session",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "write",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function externalDirectoryPermission(rawPath: string, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "external_directory",
    authorityMode: "local",
    publicSafety: "private",
    resources: [rawPath],
    saveScope: "session",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "write",
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

function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex")
}

const CREDENTIAL_BASENAMES = new Set([".env", ".secrets", ".npmrc", "auth.json", "provider-key.json"])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
