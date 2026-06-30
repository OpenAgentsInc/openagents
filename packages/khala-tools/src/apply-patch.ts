import { createHash } from "node:crypto"
import { access, constants, mkdir, readFile, realpath, rmdir, stat, unlink, writeFile } from "node:fs/promises"
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

export interface KhalaApplyPatchToolOptions {
  readonly failAfterOperations?: number
  readonly maxDiffLines?: number
  readonly maxFileBytes?: number
}

export const applyPatchToolDefinition: KhalaToolDefinition = {
  authority: "patch",
  availability: ["coding", "owner_local_full"],
  description: "Apply a constrained multi-file patch grammar after validation and scoped approval.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      patch: {
        description: "Patch text beginning with *** Begin Patch and ending with *** End Patch.",
        type: "string",
      },
    },
    required: ["patch"],
    type: "object",
  },
  internalId: "khala.file.apply_patch",
  label: "Apply Patch",
  name: "apply_patch",
  outputSchema: {
    additionalProperties: false,
    properties: {
      affectedPaths: { type: "array" },
      appliedOperations: { type: "integer" },
      atomic: { type: "boolean" },
      diff: { type: "string" },
    },
    required: ["affectedPaths", "appliedOperations", "atomic", "diff"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Apply a multi-file patch. Use edit for one exact replacement and write for full-file rewrites.",
  promptGuidelines: [
    "Use the constrained Begin Patch / End Patch grammar.",
    "Keep hunks specific enough to match exactly once.",
    "Treat V1 results as non-atomic and inspect partial-failure receipts.",
  ],
  renderer: { kind: "patch_receipt", rendererRef: "khala.renderer.patch_receipt.v1" },
}

export function createApplyPatchTool(options: KhalaApplyPatchToolOptions = {}): RegisteredKhalaTool {
  const queues = new Map<string, Promise<void>>()
  return {
    definition: applyPatchToolDefinition,
    execute: (input, context) => executeApplyPatchTool(input, context, options, queues),
  }
}

type ParsedPatchOperation =
  | Readonly<{ kind: "add"; path: string; lines: ReadonlyArray<string> }>
  | Readonly<{ kind: "delete"; path: string }>
  | Readonly<{ hunks: ReadonlyArray<PatchHunk>; kind: "update"; path: string }>

type PatchHunk = Readonly<{
  lines: ReadonlyArray<Readonly<{ kind: "add" | "context" | "remove"; text: string }>>
}>

type ResolvedPatchPath = Readonly<{
  displayPath: string
  existed: boolean
  parentRealPath: string
  realPath: string
  workspaceRoot: string
}>

type PlannedPatchOperation =
  | Readonly<{
      afterHash: string
      afterText: string
      beforeHash?: string
      beforeText: string
      diff: string
      displayPath: string
      kind: "add"
      parentRealPath: string
      realPath: string
      workspaceRoot: string
    }>
  | Readonly<{
      beforeHash: string
      beforeText: string
      diff: string
      displayPath: string
      kind: "delete"
      realPath: string
      workspaceRoot: string
    }>
  | Readonly<{
      afterHash: string
      afterText: string
      beforeHash: string
      beforeText: string
      bom: boolean
      diff: string
      displayPath: string
      kind: "update"
      lineEnding: "\n" | "\r\n"
      realPath: string
      workspaceRoot: string
    }>

type PatchPlan = Readonly<{
  affectedPaths: ReadonlyArray<string>
  diff: string
  operations: ReadonlyArray<PlannedPatchOperation>
}>

type StagedPatchOperation = Readonly<{
  afterBytes?: Buffer
  beforeBytes?: Buffer
  displayPath: string
  existed: boolean
  kind: PlannedPatchOperation["kind"]
  parentRealPath: string
  realPath: string
  workspaceRoot: string
}>

function executeApplyPatchTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaApplyPatchToolOptions,
  queues: Map<string, Promise<void>>,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const patch = decodePatchInput(input)
      const parsed = parsePatch(patch)
      const plan = await planPatch(parsed, context, options)
      return await withPathQueues(queues, plan.operations.map(operation => operation.realPath), () =>
        applyPlannedPatch(plan, context, options),
      )
    } catch (error) {
      return khalaToolError("apply_patch_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function applyPlannedPatch(
  plan: PatchPlan,
  context: KhalaToolExecuteContext,
  options: KhalaApplyPatchToolOptions,
): Promise<KhalaToolResult> {
  const decision = await Effect.runPromise(
    context.services.permission.decide(patchPermission(plan.affectedPaths, context)),
  )
  if (decision === "deny") {
    return khalaToolDenied("apply_patch_denied", "patch approval was denied")
  }

  let appliedOperations = 0
  try {
    for (const operation of plan.operations) {
      if (options.failAfterOperations !== undefined && appliedOperations >= options.failAfterOperations) {
        throw new AtomicPatchApplyError("injected atomic patch failure", appliedOperations)
      }
      await revalidateOperation(operation, options)
      appliedOperations += 1
    }
    const staged = await stagePatchOperations(plan.operations, options)
    await commitStagedPatch(staged, options)
  } catch (error) {
    const applied = error instanceof AtomicPatchApplyError ? error.appliedOperations : appliedOperations
    return patchFailureResult(plan, applied, error instanceof Error ? error.message : String(error))
  }

  return patchSuccessResult(plan, appliedOperations)
}

async function planPatch(
  operations: ReadonlyArray<ParsedPatchOperation>,
  context: KhalaToolExecuteContext,
  options: KhalaApplyPatchToolOptions,
): Promise<PatchPlan> {
  const resolved = new Map<string, ResolvedPatchPath>()
  const planned: PlannedPatchOperation[] = []
  for (const operation of operations) {
    if (resolved.has(operation.path)) {
      throw new Error(`apply_patch touches ${operation.path} more than once; split into one operation per file`)
    }
    const path = await resolvePatchPath(operation.path, context)
    resolved.set(operation.path, path)
    planned.push(await planOperation(operation, path, options))
  }
  const affectedPaths = planned.map(operation => operation.displayPath).sort(comparePath)
  const diff = boundedDiff(planned.map(operation => operation.diff).join("\n"), options.maxDiffLines ?? 400)
  return {
    affectedPaths,
    diff: diff.text,
    operations: planned,
  }
}

async function planOperation(
  operation: ParsedPatchOperation,
  path: ResolvedPatchPath,
  options: KhalaApplyPatchToolOptions,
): Promise<PlannedPatchOperation> {
  if (operation.kind === "add") {
    if (path.existed) throw new Error(`apply_patch add target already exists: ${operation.path}`)
    const afterText = `${operation.lines.join("\n")}\n`
    const afterBytes = Buffer.from(afterText, "utf8")
    ensureSize(afterBytes.byteLength, options)
    return {
      afterHash: sha256Hex(afterBytes),
      afterText,
      beforeText: "",
      diff: unifiedDiff(path.displayPath, "", afterText, options.maxDiffLines ?? 200),
      displayPath: path.displayPath,
      kind: "add",
      parentRealPath: path.parentRealPath,
      realPath: path.realPath,
      workspaceRoot: path.workspaceRoot,
    }
  }

  if (!path.existed) throw new Error(`apply_patch target does not exist: ${operation.path}`)
  const before = await readPatchText(path.realPath, options)
  if (operation.kind === "delete") {
    return {
      beforeHash: before.hash,
      beforeText: before.text,
      diff: unifiedDiff(path.displayPath, normalizeLineEndings(before.text), "", options.maxDiffLines ?? 200),
      displayPath: path.displayPath,
      kind: "delete",
      realPath: path.realPath,
      workspaceRoot: path.workspaceRoot,
    }
  }

  const updated = applyHunks(before.normalizedText, operation.hunks)
  const afterBytes = encodeText(updated.text, before)
  ensureSize(afterBytes.byteLength, options)
  return {
    afterHash: sha256Hex(afterBytes),
    afterText: updated.text,
    beforeHash: before.hash,
    beforeText: before.normalizedText,
    bom: before.bom,
    diff: unifiedDiff(path.displayPath, before.normalizedText, updated.text, options.maxDiffLines ?? 200),
    displayPath: path.displayPath,
    kind: "update",
    lineEnding: before.lineEnding,
    realPath: path.realPath,
    workspaceRoot: path.workspaceRoot,
  }
}

async function revalidateOperation(operation: PlannedPatchOperation, options: KhalaApplyPatchToolOptions): Promise<void> {
  if (operation.kind === "add") {
    await assertWritableRoot(operation.parentRealPath, operation.displayPath)
    if (await fileExists(operation.realPath)) {
      throw new AtomicPatchApplyError(`add target appeared before apply: ${operation.displayPath}`, 0)
    }
    return
  }
  await assertWritableRoot(dirname(operation.realPath), operation.displayPath)
  await assertSafeWritableFile(operation.realPath, operation.displayPath)
  const current = await readPatchText(operation.realPath, options)
  if (current.hash !== operation.beforeHash) {
    throw new Error(`stale content before applying ${operation.displayPath}`)
  }
}

async function stagePatchOperations(
  operations: ReadonlyArray<PlannedPatchOperation>,
  options: KhalaApplyPatchToolOptions,
): Promise<ReadonlyArray<StagedPatchOperation>> {
  const staged: StagedPatchOperation[] = []
  for (const operation of operations) {
    await revalidateOperation(operation, options)
    if (operation.kind === "add") {
      staged.push({
        afterBytes: Buffer.from(operation.afterText, "utf8"),
        displayPath: operation.displayPath,
        existed: false,
        kind: operation.kind,
        parentRealPath: operation.parentRealPath,
        realPath: operation.realPath,
        workspaceRoot: operation.workspaceRoot,
      })
      continue
    }
    const beforeBytes = await readFile(operation.realPath)
    staged.push({
      ...(operation.kind === "delete" ? {} : { afterBytes: encodeText(operation.afterText, operation) }),
      beforeBytes,
      displayPath: operation.displayPath,
      existed: true,
      kind: operation.kind,
      parentRealPath: dirname(operation.realPath),
      realPath: operation.realPath,
      workspaceRoot: operation.workspaceRoot,
    })
  }
  return staged
}

async function commitStagedPatch(
  operations: ReadonlyArray<StagedPatchOperation>,
  options: KhalaApplyPatchToolOptions,
): Promise<void> {
  const committed: StagedPatchOperation[] = []
  try {
    for (const operation of operations) {
      if (options.failAfterOperations !== undefined && committed.length >= options.failAfterOperations) {
        throw new AtomicPatchApplyError("injected atomic patch failure", committed.length)
      }
      await applyStagedOperation(operation)
      committed.push(operation)
    }
  } catch (error) {
    await rollbackStagedPatch(committed)
    if (error instanceof AtomicPatchApplyError) throw error
    throw new AtomicPatchApplyError(error instanceof Error ? error.message : String(error), committed.length)
  }
}

async function applyStagedOperation(operation: StagedPatchOperation): Promise<void> {
  if (operation.kind === "add") {
    await mkdir(operation.parentRealPath, { recursive: true })
    await writeFile(operation.realPath, operation.afterBytes ?? Buffer.alloc(0))
    return
  }
  if (operation.kind === "delete") {
    await unlink(operation.realPath)
    return
  }
  await writeFile(operation.realPath, operation.afterBytes ?? Buffer.alloc(0))
}

async function rollbackStagedPatch(committed: ReadonlyArray<StagedPatchOperation>): Promise<void> {
  const failures: string[] = []
  for (const operation of [...committed].reverse()) {
    try {
      if (operation.existed) {
        await writeFile(operation.realPath, operation.beforeBytes ?? Buffer.alloc(0))
      } else if (await fileExists(operation.realPath)) {
        await unlink(operation.realPath)
        await pruneEmptyParents(operation.parentRealPath, operation.workspaceRoot)
      }
    } catch (error) {
      failures.push(`${operation.displayPath}: ${error instanceof Error ? error.message : String(error)}`)
    }
  }
  if (failures.length > 0) {
    throw new Error(`patch rollback failed: ${failures.join("; ")}`)
  }
}

async function pruneEmptyParents(start: string, workspaceRoot: string): Promise<void> {
  let current = start
  while (pathIsInside(workspaceRoot, current) && current !== workspaceRoot) {
    try {
      await rmdir(current)
    } catch {
      return
    }
    current = dirname(current)
  }
}

function patchSuccessResult(plan: PatchPlan, appliedOperations: number): KhalaToolResult {
  const modelText = renderPatchModelText("Applied", plan, appliedOperations, false)
  return khalaToolOk({
    modelText,
    publicSummary:
      `Applied atomic patch: ${appliedOperations}/${plan.operations.length} operations, ` +
      `${plan.affectedPaths.length} paths, 1 private diff receipt.`,
    ui: patchUi(plan, appliedOperations, false, null),
    publicSafety: "private",
  })
}

function patchFailureResult(plan: PatchPlan, appliedOperations: number, reason: string): KhalaToolResult {
  const failed = khalaToolError("apply_patch_partial_failure", reason)
  return {
    ...failed,
    modelOutput: {
      text: renderPatchModelText("Patch failed", plan, appliedOperations, true),
    },
    publicSafety: "private",
    publicSummary:
      `Atomic patch failed before commit; ${appliedOperations}/${plan.operations.length} operations validated, ` +
      `${plan.affectedPaths.length} paths, 1 private diff receipt.`,
    status: "failed",
    ui: patchUi(plan, appliedOperations, true, reason),
  }
}

function renderPatchModelText(
  label: string,
  plan: PatchPlan,
  appliedOperations: number,
  partialFailure: boolean,
): string {
  return [
    `${label} atomic patch`,
    `Applied operations: ${appliedOperations}/${plan.operations.length}`,
    `Rolled back: ${partialFailure ? "yes" : "no"}`,
    "```diff",
    plan.diff,
    "```",
  ].join("\n")
}

function patchUi(
  plan: PatchPlan,
  appliedOperations: number,
  partialFailure: boolean,
  failureReason: string | null,
): unknown {
  return {
    affectedPaths: plan.affectedPaths,
    appliedOperations,
    atomic: true,
    diff: {
      format: "unified",
      kind: "unified_diff",
      publicSafety: "private",
      rendererRef: "khala.renderer.diff.v1",
      text: plan.diff,
    },
    events: [
      { kind: "diff_chunk", payload: { bytes: Buffer.byteLength(plan.diff, "utf8"), private: true } },
      { kind: partialFailure ? "tool_failed" : "tool_completed", payload: { appliedOperations } },
    ],
    failureReason,
    kind: "patch_receipt",
    operationCount: plan.operations.length,
    partialFailure,
    publicSafety: "private",
  }
}

function parsePatch(raw: string): ReadonlyArray<ParsedPatchOperation> {
  const lines = normalizeLineEndings(raw).split("\n")
  if (lines[0] !== "*** Begin Patch") throw new Error("patch must start with *** Begin Patch")
  const endIndex = lines.findIndex((line, index) => index > 0 && line === "*** End Patch")
  if (endIndex === -1) throw new Error("patch must end with *** End Patch")
  if (lines.slice(endIndex + 1).some(line => line.trim().length > 0)) {
    throw new Error("unexpected content after *** End Patch")
  }
  const operations: ParsedPatchOperation[] = []
  let index = 1
  while (index < endIndex) {
    const line = lines[index]
    if (line?.startsWith("*** Add File: ") === true) {
      const path = parsePatchPath(line.slice("*** Add File: ".length))
      index += 1
      const added: string[] = []
      while (index < endIndex && !isOperationHeader(lines[index] ?? "")) {
        const body = lines[index] ?? ""
        if (!body.startsWith("+")) throw new Error(`add file lines must start with + for ${path}`)
        added.push(body.slice(1))
        index += 1
      }
      if (added.length === 0) throw new Error(`add file requires at least one line: ${path}`)
      operations.push({ kind: "add", lines: added, path })
      continue
    }
    if (line?.startsWith("*** Delete File: ") === true) {
      operations.push({ kind: "delete", path: parsePatchPath(line.slice("*** Delete File: ".length)) })
      index += 1
      continue
    }
    if (line?.startsWith("*** Update File: ") === true) {
      const path = parsePatchPath(line.slice("*** Update File: ".length))
      index += 1
      const hunks: PatchHunk[] = []
      while (index < endIndex && !isOperationHeader(lines[index] ?? "")) {
        if ((lines[index] ?? "") !== "@@" && (lines[index] ?? "").startsWith("@@ ") === false) {
          throw new Error(`update hunk for ${path} must start with @@`)
        }
        index += 1
        const hunkLines: Array<Readonly<{ kind: "add" | "context" | "remove"; text: string }>> = []
        while (index < endIndex && !isOperationHeader(lines[index] ?? "") && !((lines[index] ?? "").startsWith("@@"))) {
          const body = lines[index] ?? ""
          const prefix = body[0]
          if (prefix !== " " && prefix !== "-" && prefix !== "+") {
            throw new Error(`update hunk lines must start with space, -, or + for ${path}`)
          }
          hunkLines.push({
            kind: prefix === " " ? "context" : prefix === "-" ? "remove" : "add",
            text: body.slice(1),
          })
          index += 1
        }
        if (!hunkLines.some(item => item.kind === "add" || item.kind === "remove")) {
          throw new Error(`update hunk must add or remove at least one line for ${path}`)
        }
        hunks.push({ lines: hunkLines })
      }
      if (hunks.length === 0) throw new Error(`update file requires at least one hunk: ${path}`)
      operations.push({ hunks, kind: "update", path })
      continue
    }
    throw new Error(`unexpected patch line: ${line ?? ""}`)
  }
  if (operations.length === 0) throw new Error("patch must contain at least one operation")
  return operations
}

function applyHunks(content: string, hunks: ReadonlyArray<PatchHunk>): Readonly<{ text: string }> {
  let current = content
  for (const hunk of hunks) {
    const oldBlock = hunk.lines
      .filter(line => line.kind !== "add")
      .map(line => line.text)
      .join("\n")
    const newBlock = hunk.lines
      .filter(line => line.kind !== "remove")
      .map(line => line.text)
      .join("\n")
    if (oldBlock.length === 0) throw new Error("update hunk needs context or removed lines")
    const positions = occurrencePositions(current, oldBlock)
    if (positions.length === 0) throw new Error("patch hunk did not match current file content")
    if (positions.length > 1) throw new Error("patch hunk matched more than once")
    current = `${current.slice(0, positions[0])}${newBlock}${current.slice((positions[0] ?? 0) + oldBlock.length)}`
  }
  if (current === content) throw new Error("patch update produced no content change")
  return { text: current }
}

async function resolvePatchPath(rawPath: string, context: KhalaToolExecuteContext): Promise<ResolvedPatchPath> {
  validateRelativePatchPath(rawPath)
  const workspaceRoot = await realpath(context.services.workspace.workingDirectory)
  const candidate = resolve(workspaceRoot, rawPath)
  if (!pathIsInside(workspaceRoot, candidate)) throw new Error(`patch path escapes workspace: ${rawPath}`)
  const existingTarget = await realpath(candidate).then(
    value => value,
    () => undefined,
  )
  const existed = existingTarget !== undefined
  const target = existingTarget ?? await resolveFutureTarget(dirname(candidate), candidate)
  if (!pathIsInside(workspaceRoot, target)) throw new Error(`patch path resolves outside workspace: ${rawPath}`)
  if (existed) {
    const info = await stat(target)
    if (!info.isFile()) throw new Error(`patch target is not a regular file: ${rawPath}`)
  }
  return {
    displayPath: toWorkspaceRelative(workspaceRoot, target),
    existed,
    parentRealPath: dirname(target),
    realPath: target,
    workspaceRoot,
  }
}

async function assertSafeWritableFile(realPath: string, displayPath: string): Promise<void> {
  const info = await stat(realPath)
  if (!info.isFile()) throw new Error(`patch target is not a regular file: ${displayPath}`)
  if (info.nlink > 1) throw new Error(`patch target has hard links: ${displayPath}`)
  try {
    await access(realPath, constants.W_OK)
  } catch {
    throw new Error(`patch target is not writable: ${displayPath}`)
  }
  await assertWritableRoot(dirname(realPath), displayPath)
}

async function assertWritableRoot(realPath: string, displayPath: string): Promise<void> {
  try {
    const writableRoot = await nearestExistingParent(realPath)
    await access(writableRoot, constants.W_OK)
  } catch {
    throw new Error(`patch target is not writable: ${displayPath}`)
  }
}

function validateRelativePatchPath(path: string): void {
  if (path.length === 0) throw new Error("patch path must not be empty")
  if (isAbsolute(path)) throw new Error("patch paths must be workspace-relative")
  const normalized = path.split("\\").join("/")
  if (normalized.split("/").some(part => part === ".." || part.length === 0)) {
    throw new Error(`invalid patch path: ${path}`)
  }
  if (credentialPathDenied(normalized)) throw new Error(`patch path is credential-shaped: ${path}`)
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
      if (!info.isDirectory()) throw new Error("patch parent path is not a directory")
      return current
    } catch (error) {
      if (error instanceof Error && "code" in error && error.code !== "ENOENT") throw error
      const next = dirname(current)
      if (next === current) throw new Error("patch could not resolve an existing parent directory")
      current = next
    }
  }
}

type PatchText = Readonly<{
  bom: boolean
  hash: string
  lineEnding: "\n" | "\r\n"
  normalizedText: string
  text: string
}>

async function readPatchText(realPath: string, options: KhalaApplyPatchToolOptions): Promise<PatchText> {
  const info = await stat(realPath)
  if (!info.isFile()) throw new Error("patch target must be a file")
  if (info.size > (options.maxFileBytes ?? 2 * 1024 * 1024)) throw new Error("patch target exceeds maximum size")
  const bytes = await readFile(realPath)
  if (bytes.includes(0)) throw new Error("patch only supports text files")
  const bom = hasUtf8Bom(bytes)
  const text = (bom ? bytes.subarray(3) : bytes).toString("utf8")
  return {
    bom,
    hash: sha256Hex(bytes),
    lineEnding: text.includes("\r\n") ? "\r\n" : "\n",
    normalizedText: normalizeLineEndings(text),
    text,
  }
}

function encodeText(normalizedText: string, original: Readonly<{ bom: boolean; lineEnding: "\n" | "\r\n" }>): Buffer {
  const text = original.lineEnding === "\r\n" ? normalizedText.replace(/\n/gu, "\r\n") : normalizedText
  const bytes = Buffer.from(text, "utf8")
  return original.bom ? Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), bytes]) : bytes
}

function parsePatchPath(value: string): string {
  const path = value.trim()
  validateRelativePatchPath(path)
  return path
}

function decodePatchInput(input: Readonly<Record<string, unknown>>): string {
  if (typeof input.patch !== "string" || input.patch.trim().length === 0) {
    throw new Error("apply_patch requires patch text")
  }
  return input.patch
}

function isOperationHeader(line: string): boolean {
  return line.startsWith("*** Add File: ") || line.startsWith("*** Update File: ") || line.startsWith("*** Delete File: ")
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

function unifiedDiff(path: string, before: string, after: string, maxLines: number): string {
  return boundedDiff(createUnifiedDiff(path, before, after), maxLines).text
}

function createUnifiedDiff(path: string, before: string, after: string): string {
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
  return lines.join("\n")
}

function boundedDiff(diff: string, maxLines: number): Readonly<{ text: string; truncated: boolean }> {
  const lines = diff.split("\n")
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

async function withPathQueues<T>(
  queues: Map<string, Promise<void>>,
  paths: ReadonlyArray<string>,
  task: () => Promise<T>,
): Promise<T> {
  const unique = [...new Set(paths)].sort(comparePath)
  const acquire = (index: number): Promise<T> => {
    if (index >= unique.length) return task()
    return withPathQueue(queues, unique[index] ?? "", () => acquire(index + 1))
  }
  return acquire(0)
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

function patchPermission(resources: ReadonlyArray<string>, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "patch",
    authorityMode: "local",
    publicSafety: "private",
    resources: [...resources],
    saveScope: "session",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "apply_patch",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function normalizeLineEndings(value: string): string {
  return value.replace(/\r\n/gu, "\n").replace(/\r/gu, "\n")
}

function hasUtf8Bom(bytes: Buffer): boolean {
  return bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf
}

function ensureSize(size: number, options: KhalaApplyPatchToolOptions): void {
  if (size > (options.maxFileBytes ?? 2 * 1024 * 1024)) throw new Error("patch output exceeds maximum file size")
}

function pathIsInside(root: string, target: string): boolean {
  const rel = relative(root, target)
  return rel === "" || !rel.startsWith("..") && !isAbsolute(rel)
}

function toWorkspaceRelative(root: string, target: string): string {
  const rel = relative(root, target)
  return rel === "" ? "." : rel.split("\\").join("/")
}

function comparePath(a: string, b: string): number {
  return a.toLowerCase().localeCompare(b.toLowerCase()) || a.localeCompare(b)
}

function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex")
}

class AtomicPatchApplyError extends Error {
  constructor(message: string, readonly appliedOperations: number) {
    super(message)
  }
}

const CREDENTIAL_BASENAMES = new Set([".env", ".secrets", ".npmrc", "auth.json", "provider-key.json"])

function credentialPathDenied(path: string): boolean {
  const normalized = path.split("\\").join("/")
  return normalized.includes("/.secrets/") ||
    normalized.startsWith(".secrets/") ||
    CREDENTIAL_BASENAMES.has(basename(normalized))
}
