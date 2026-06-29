import { createHash } from "node:crypto"
import { readFile, realpath, stat, writeFile } from "node:fs/promises"
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

export interface KhalaEditToolOptions {
  readonly maxDiffLines?: number
  readonly maxFileBytes?: number
}

export const editToolDefinition: KhalaToolDefinition = {
  authority: "edit",
  availability: ["coding", "owner_local_full"],
  description: "Apply exact text replacements to one workspace file with a diff preview and write approval.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      dry_run: {
        description: "Preview the edit without writing bytes or requesting write approval.",
        type: "boolean",
      },
      edits: {
        description: "One or more exact old_text/new_text replacements.",
        items: {
          additionalProperties: false,
          properties: {
            new_text: { type: "string" },
            old_text: { type: "string" },
            replace_all: { type: "boolean" },
          },
          required: ["old_text", "new_text"],
          type: "object",
        },
        minItems: 1,
        type: "array",
      },
      expected_sha256: {
        description: "Optional SHA-256 of the current file bytes for stale-content protection.",
        type: "string",
      },
      new_text: {
        description: "Convenience single-edit replacement text.",
        type: "string",
      },
      old_text: {
        description: "Convenience single-edit exact text to replace.",
        type: "string",
      },
      path: {
        description: "Workspace-relative file path or approved absolute path.",
        type: "string",
      },
      replace_all: {
        description: "Convenience single-edit replace-all flag.",
        type: "boolean",
      },
    },
    required: ["path"],
    type: "object",
  },
  internalId: "khala.file.edit",
  label: "Edit File",
  name: "edit",
  outputSchema: {
    additionalProperties: false,
    properties: {
      diff: { type: "string" },
      firstChangedLine: { type: "integer" },
      path: { type: "string" },
      replacementCount: { type: "integer" },
    },
    required: ["path", "diff", "firstChangedLine", "replacementCount"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Replace exact text in a file. Use write or apply_patch for whole-file or multi-file changes.",
  promptGuidelines: [
    "Provide enough old_text to match exactly once unless replace_all is intentional.",
    "Do not use edit to replace an entire file.",
    "Use expected_sha256 when editing from an older file snapshot.",
  ],
  renderer: { kind: "file_edit", rendererRef: "khala.renderer.file_edit.v1" },
}

export function createEditTool(options: KhalaEditToolOptions = {}): RegisteredKhalaTool {
  const queues = new Map<string, Promise<void>>()
  return {
    definition: editToolDefinition,
    execute: (input, context) => executeEditTool(input, context, options, queues),
  }
}

type EditInput = Readonly<{
  dryRun: boolean
  edits: ReadonlyArray<ExactEdit>
  expectedSha256?: string
  path: string
}>

type ExactEdit = Readonly<{
  newText: string
  oldText: string
  replaceAll: boolean
}>

type ResolvedEditablePath = Readonly<{
  _tag: "ok"
  displayPath: string
  realPath: string
}>

type EditPlan = Readonly<{
  afterHash: string
  beforeHash: string
  diff: string
  diffTruncated: boolean
  firstChangedLine: number
  normalizedAfter: string
  replacementCount: number
}>

function executeEditTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaEditToolOptions,
  queues: Map<string, Promise<void>>,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeEditInput(input)
      if (credentialPathDenied(args.path)) {
        return khalaToolError("edit_blocked_credential_path", "edit blocked a credential-shaped path")
      }
      const resolved = await resolveEditablePath(args.path, context)
      if (resolved._tag === "denied") {
        return khalaToolDenied("edit_external_file_denied", "edit outside the workspace was denied")
      }
      return await withPathQueue(queues, resolved.realPath, () => editResolvedFile(args, resolved, context, options))
    } catch (error) {
      return khalaToolError("edit_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function editResolvedFile(
  args: EditInput,
  resolved: ResolvedEditablePath,
  context: KhalaToolExecuteContext,
  options: KhalaEditToolOptions,
): Promise<KhalaToolResult> {
  const beforeBytes = await readEditableBytes(resolved.realPath, options)
  const beforeHash = sha256Hex(beforeBytes)
  if (args.expectedSha256 !== undefined && args.expectedSha256 !== beforeHash) {
    return khalaToolError("edit_stale_file", "current file SHA-256 does not match expected_sha256")
  }
  const before = decodeTextBytes(beforeBytes)
  const plan = planExactEdit(args, resolved.displayPath, before, beforeHash, options)
  if (args.dryRun) {
    return editSuccessResult(args, resolved, plan, true)
  }

  const decision = await Effect.runPromise(
    context.services.permission.decide(editPermission(args.path, resolved.displayPath, context)),
  )
  if (decision === "deny") {
    return khalaToolDenied("edit_write_denied", "edit write approval was denied")
  }

  const latestBytes = await readEditableBytes(resolved.realPath, options)
  if (sha256Hex(latestBytes) !== beforeHash) {
    return khalaToolError("edit_stale_file", "file changed after edit preview; retry with fresh content")
  }
  await writeFile(resolved.realPath, encodeTextBytes(plan.normalizedAfter, before))
  return editSuccessResult(args, resolved, plan, false)
}

function planExactEdit(
  args: EditInput,
  displayPath: string,
  before: DecodedText,
  beforeHash: string,
  options: KhalaEditToolOptions,
): EditPlan {
  let content = before.normalizedText
  let firstChangedLine = Number.POSITIVE_INFINITY
  let replacementCount = 0
  for (const edit of args.edits) {
    const oldText = normalizeLineEndings(edit.oldText)
    const newText = normalizeLineEndings(edit.newText)
    if (oldText.length === 0) throw new Error("edit old_text must be non-empty")
    if (oldText === content) {
      throw new Error("edit refuses whole-file replacement; use write or apply_patch")
    }
    const positions = occurrencePositions(content, oldText)
    if (positions.length === 0) throw new Error("edit old_text did not match")
    if (positions.length > 1 && !edit.replaceAll) {
      throw new Error("edit old_text matched more than once; pass replace_all to replace every occurrence")
    }
    firstChangedLine = Math.min(firstChangedLine, lineNumberAt(content, positions[0] ?? 0))
    replacementCount += edit.replaceAll ? positions.length : 1
    content = edit.replaceAll
      ? content.split(oldText).join(newText)
      : replaceFirst(content, oldText, newText)
  }
  if (replacementCount === 0 || content === before.normalizedText) {
    throw new Error("edit produced no content change")
  }
  const diff = unifiedDiff(displayPath, before.normalizedText, content, options.maxDiffLines ?? 200)
  return {
    afterHash: sha256Hex(encodeTextBytes(content, before)),
    beforeHash,
    diff: diff.text,
    diffTruncated: diff.truncated,
    firstChangedLine: Number.isFinite(firstChangedLine) ? firstChangedLine : 1,
    normalizedAfter: content,
    replacementCount,
  }
}

function editSuccessResult(
  args: EditInput,
  resolved: ResolvedEditablePath,
  plan: EditPlan,
  dryRun: boolean,
): KhalaToolResult {
  const modelText = [
    `${dryRun ? "Previewed" : "Edited"} ${resolved.displayPath}`,
    `First changed line: ${plan.firstChangedLine}`,
    `Replacements: ${plan.replacementCount}`,
    "```diff",
    plan.diff,
    "```",
  ].join("\n")
  return khalaToolOk({
    modelText,
    publicSummary:
      `${dryRun ? "Previewed" : "Edited"} ${resolved.displayPath}: ${plan.replacementCount} replacements` +
      ` at line ${plan.firstChangedLine}${plan.diffTruncated ? " (diff truncated)" : ""}.`,
    ui: {
      afterSha256: plan.afterHash,
      approval: {
        action: "edit",
        resource: resolved.displayPath,
      },
      beforeSha256: plan.beforeHash,
      diff: plan.diff,
      diffTruncated: plan.diffTruncated,
      dryRun,
      firstChangedLine: plan.firstChangedLine,
      kind: "file_edit",
      path: resolved.displayPath,
      replacementCount: plan.replacementCount,
      requestedPath: args.path,
    },
    publicSafety: "private",
  })
}

async function readEditableBytes(realPath: string, options: KhalaEditToolOptions): Promise<Buffer> {
  const info = await stat(realPath)
  if (!info.isFile()) throw new Error("edit path must be a regular file")
  if (info.size > (options.maxFileBytes ?? 2 * 1024 * 1024)) {
    throw new Error("edit file exceeds maximum editable size")
  }
  const bytes = await readFile(realPath)
  if (bytes.includes(0)) throw new Error("edit only supports text files")
  return bytes
}

async function resolveEditablePath(
  rawPath: string,
  context: KhalaToolExecuteContext,
): Promise<ResolvedEditablePath | Readonly<{ _tag: "denied" }>> {
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

function decodeEditInput(input: Readonly<Record<string, unknown>>): EditInput {
  const path = typeof input.path === "string" ? input.path.trim() : ""
  if (path.length === 0) throw new Error("edit requires a non-empty path")
  const edits = decodeEdits(input)
  const dryRun = optionalBoolean(input.dry_run, "dry_run") ?? false
  const expectedSha256 = typeof input.expected_sha256 === "string" && input.expected_sha256.trim().length > 0
    ? input.expected_sha256.trim()
    : undefined
  return {
    dryRun,
    edits,
    ...(expectedSha256 === undefined ? {} : { expectedSha256 }),
    path,
  }
}

function decodeEdits(input: Readonly<Record<string, unknown>>): ReadonlyArray<ExactEdit> {
  if (Array.isArray(input.edits)) {
    if (input.edits.length === 0) throw new Error("edit edits must not be empty")
    return input.edits.map((value, index) => {
      if (!isRecord(value)) throw new Error(`edit edits[${index}] must be an object`)
      return decodeOneEdit(value, `edits[${index}]`)
    })
  }
  if (typeof input.old_text === "string" && typeof input.new_text === "string") {
    return [decodeOneEdit(input, "edit")]
  }
  throw new Error("edit requires edits or old_text/new_text")
}

function decodeOneEdit(input: Readonly<Record<string, unknown>>, label: string): ExactEdit {
  const oldText = input.old_text
  const newText = input.new_text
  if (typeof oldText !== "string") throw new Error(`${label}.old_text must be a string`)
  if (typeof newText !== "string") throw new Error(`${label}.new_text must be a string`)
  return {
    newText,
    oldText,
    replaceAll: optionalBoolean(input.replace_all, `${label}.replace_all`) ?? false,
  }
}

function optionalBoolean(value: unknown, field: string): boolean | undefined {
  if (value === undefined) return undefined
  if (typeof value !== "boolean") throw new Error(`edit ${field} must be a boolean`)
  return value
}

function normalizeLineEndings(value: string): string {
  return value.replace(/\r\n/gu, "\n").replace(/\r/gu, "\n")
}

type DecodedText = Readonly<{
  bom: boolean
  lineEnding: "\n" | "\r\n"
  normalizedText: string
}>

function decodeTextBytes(bytes: Buffer): DecodedText {
  const bom = bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf
  const body = bom ? bytes.subarray(3) : bytes
  const text = body.toString("utf8")
  return {
    bom,
    lineEnding: text.includes("\r\n") ? "\r\n" : "\n",
    normalizedText: normalizeLineEndings(text),
  }
}

function encodeTextBytes(normalizedText: string, original: DecodedText): Buffer {
  const text = original.lineEnding === "\r\n" ? normalizedText.replace(/\n/gu, "\r\n") : normalizedText
  const bytes = Buffer.from(text, "utf8")
  return original.bom ? Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), bytes]) : bytes
}

function occurrencePositions(content: string, needle: string): ReadonlyArray<number> {
  const positions: number[] = []
  let start = 0
  while (start <= content.length) {
    const index = content.indexOf(needle, start)
    if (index === -1) break
    positions.push(index)
    start = index + Math.max(needle.length, 1)
  }
  return positions
}

function replaceFirst(content: string, oldText: string, newText: string): string {
  const index = content.indexOf(oldText)
  if (index === -1) return content
  return `${content.slice(0, index)}${newText}${content.slice(index + oldText.length)}`
}

function lineNumberAt(content: string, index: number): number {
  let line = 1
  for (let position = 0; position < index; position += 1) {
    if (content[position] === "\n") line += 1
  }
  return line
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

function editPermission(
  rawPath: string,
  displayPath: string,
  context: KhalaToolExecuteContext,
): KhalaPermissionRequest {
  return {
    action: "edit",
    authorityMode: "local",
    publicSafety: "private",
    resources: [displayPath, rawPath],
    saveScope: "once",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "edit",
    workingDirectory: context.services.workspace.workingDirectory,
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
    toolName: "edit",
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

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

const CREDENTIAL_BASENAMES = new Set([".env", ".secrets", ".npmrc", "auth.json", "provider-key.json"])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
