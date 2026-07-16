/**
 * Harness-neutral typed workbench item model (#8859, epic #8857 wave 0).
 *
 * `WorkbenchItem` is the ONE typed presentation contract between every
 * provider harness (Codex app-server, Codex exec, the Claude/Fable SDK lane)
 * and the timeline renderer. It replaces the string-flattening loss point in
 * `toolFacts()` (codex-app-server-turn.ts) without breaking the existing
 * string contract: typed items ride ADDITIVELY on the same tool trace events
 * and notes (`DesktopToolTrace.item`), so old notes and old emitters keep
 * decoding while wave-2 component lanes render the typed fields.
 *
 * Every string is bounded at the schema (the emitters bound before emitting)
 * and payloads are source-tagged so no renderer branch needs Codex-only
 * assumptions. Opaque-blob suppression stays a RENDER concern
 * (tool-cards.ts `isOpaqueBlobValue`); this contract never carries unbounded
 * payloads across the Electron boundary.
 */
import { Exit, Schema } from "@effect-native/core/effect"

/** Which harness observed the item. */
export const WorkbenchItemSourceSchema = Schema.Literals(["codex", "claude", "local"])
export type WorkbenchItemSource = typeof WorkbenchItemSourceSchema.Type

/**
 * Normalized lifecycle status across wire vocabularies
 * (`inProgress`/`in_progress`, `completed`, `failed`, `declined`).
 */
export const WorkbenchItemStatusSchema = Schema.Literals([
  "in_progress",
  "completed",
  "failed",
  "declined",
])
export type WorkbenchItemStatus = typeof WorkbenchItemStatusSchema.Type

// Bounds. Diff/text ceilings match the history contract's 20k text bound;
// tails and snippets stay small enough for note payloads crossing IPC.
export const WORKBENCH_COMMAND_LIMIT = 4_000
export const WORKBENCH_PATH_LIMIT = 1_024
export const WORKBENCH_OUTPUT_TAIL_LIMIT = 4_000
export const WORKBENCH_DIFF_LIMIT = 20_000
export const WORKBENCH_CHANGE_LIMIT = 64
export const WORKBENCH_ARG_LIMIT = 24
export const WORKBENCH_ARG_KEY_LIMIT = 80
export const WORKBENCH_ARG_VALUE_LIMIT = 400
export const WORKBENCH_RESULT_SNIPPET_LIMIT = 2_000
export const WORKBENCH_TEXT_LIMIT = 32_000

const BoundedString = (limit: number) => Schema.String.check(Schema.isMaxLength(limit))
const Count = Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))

/** One bounded key/value argument pair (tool call arguments, k/v projected). */
export const WorkbenchArgEntrySchema = Schema.Struct({
  key: BoundedString(WORKBENCH_ARG_KEY_LIMIT),
  value: BoundedString(WORKBENCH_ARG_VALUE_LIMIT),
})
export type WorkbenchArgEntry = typeof WorkbenchArgEntrySchema.Type

/** commandExecution → command,cwd,status,exitCode,durationMs,outputTail,source. */
export const WorkbenchCommandItemSchema = Schema.Struct({
  kind: Schema.Literal("command"),
  source: WorkbenchItemSourceSchema,
  command: BoundedString(WORKBENCH_COMMAND_LIMIT),
  cwd: Schema.optional(BoundedString(WORKBENCH_PATH_LIMIT)),
  status: WorkbenchItemStatusSchema,
  exitCode: Schema.optional(Schema.NullOr(Schema.Number.check(Schema.isInt()))),
  durationMs: Schema.optional(Schema.Number),
  /** Bounded TAIL of aggregated stdout+stderr (the end carries the verdict). */
  outputTail: Schema.optional(BoundedString(WORKBENCH_OUTPUT_TAIL_LIMIT)),
  /** True when earlier output was discarded to preserve the bounded tail. */
  outputCapReached: Schema.optional(Schema.Boolean),
  /** Wire `source` field: who initiated the command (agent vs user shell). */
  commandSource: Schema.optional(
    Schema.Literals(["agent", "userShell", "unifiedExecStartup", "unifiedExecInteraction"]),
  ),
})
export type WorkbenchCommandItem = typeof WorkbenchCommandItemSchema.Type

export const WorkbenchFileChangeEntrySchema = Schema.Struct({
  path: BoundedString(WORKBENCH_PATH_LIMIT),
  kind: Schema.Literals(["add", "delete", "update"]),
  /** Added/removed line counts derived from the unified diff, when present. */
  adds: Schema.optional(Count),
  dels: Schema.optional(Count),
  diff: Schema.optional(BoundedString(WORKBENCH_DIFF_LIMIT)),
  /** True when this file's unified diff was truncated at the schema bound. */
  diffCapReached: Schema.optional(Schema.Boolean),
})
export type WorkbenchFileChangeEntry = typeof WorkbenchFileChangeEntrySchema.Type

/** fileChange → changes[{path,kind,adds,dels,diff}],status. */
export const WorkbenchFileChangeItemSchema = Schema.Struct({
  kind: Schema.Literal("fileChange"),
  source: WorkbenchItemSourceSchema,
  status: WorkbenchItemStatusSchema,
  /** Item patch versus the aggregate diff for the active turn. */
  scope: Schema.optional(Schema.Literals(["item", "turn"])),
  changes: Schema.Array(WorkbenchFileChangeEntrySchema).check(
    Schema.isMaxLength(WORKBENCH_CHANGE_LIMIT),
  ),
})
export type WorkbenchFileChangeItem = typeof WorkbenchFileChangeItemSchema.Type

/** mcpToolCall / dynamicToolCall / webSearch / image items, one typed card. */
export const WorkbenchToolCallItemSchema = Schema.Struct({
  kind: Schema.Literal("toolCall"),
  source: WorkbenchItemSourceSchema,
  callKind: Schema.Literals(["mcp", "dynamic", "web", "image"]),
  tool: BoundedString(120),
  server: Schema.optional(BoundedString(120)),
  namespace: Schema.optional(BoundedString(120)),
  args: Schema.Array(WorkbenchArgEntrySchema).check(Schema.isMaxLength(WORKBENCH_ARG_LIMIT)),
  resultSnippet: Schema.optional(BoundedString(WORKBENCH_RESULT_SNIPPET_LIMIT)),
  errorMessage: Schema.optional(BoundedString(400)),
  durationMs: Schema.optional(Schema.Number),
  status: WorkbenchItemStatusSchema,
  /** web: the search query. */
  query: Schema.optional(BoundedString(400)),
  /** web: how many structured results came back. */
  resultCount: Schema.optional(Count),
  /** image: the viewed/saved image path (already host-redacted). */
  path: Schema.optional(BoundedString(WORKBENCH_PATH_LIMIT)),
})
export type WorkbenchToolCallItem = typeof WorkbenchToolCallItemSchema.Type

export const WorkbenchMessageItemSchema = Schema.Struct({
  kind: Schema.Literal("message"),
  source: WorkbenchItemSourceSchema,
  role: Schema.Literals(["user", "assistant", "system"]),
  text: BoundedString(WORKBENCH_TEXT_LIMIT),
  phase: Schema.optional(BoundedString(40)),
  citation: Schema.optional(BoundedString(400)),
})

export const WorkbenchReasoningItemSchema = Schema.Struct({
  kind: Schema.Literal("reasoning"),
  source: WorkbenchItemSourceSchema,
  summary: BoundedString(4_000),
})

export const WorkbenchAgentItemSchema = Schema.Struct({
  kind: Schema.Literal("agent"),
  source: WorkbenchItemSourceSchema,
  tool: Schema.optional(BoundedString(40)),
  prompt: Schema.optional(BoundedString(2_000)),
  status: WorkbenchItemStatusSchema,
  childRefs: Schema.optional(Schema.Array(BoundedString(120)).check(Schema.isMaxLength(16))),
})

export const WorkbenchPlanItemSchema = Schema.Struct({
  kind: Schema.Literal("plan"),
  source: WorkbenchItemSourceSchema,
  entries: Schema.Array(Schema.Struct({
    step: BoundedString(400),
    status: Schema.Literals(["pending", "in_progress", "completed"]),
  })).check(Schema.isMaxLength(64)),
  /**
   * Free-form plan narrative (T8 #8865 unification). The `plan` ThreadItem
   * wire variant (`{id, text, type: "plan"}`, collaboration-mode write-ups)
   * carries prose instead of a structured step list; `turn/plan/updated` and
   * history `todo_list` rows carry structured `entries` instead. A plan item
   * may carry either, or both (prose narrative + a live step checklist).
   */
  prose: Schema.optional(BoundedString(4_000)),
})
export type WorkbenchPlanItem = typeof WorkbenchPlanItemSchema.Type

export const WorkbenchApprovalItemSchema = Schema.Struct({
  kind: Schema.Literal("approval"),
  source: WorkbenchItemSourceSchema,
  status: WorkbenchItemStatusSchema,
  decision: Schema.optional(BoundedString(40)),
  detail: Schema.optional(BoundedString(400)),
})

export const WorkbenchMeterItemSchema = Schema.Struct({
  kind: Schema.Literal("meter"),
  source: WorkbenchItemSourceSchema,
  inputTokens: Schema.optional(Count),
  cachedInputTokens: Schema.optional(Count),
  outputTokens: Schema.optional(Count),
  reasoningTokens: Schema.optional(Count),
  totalTokens: Schema.optional(Count),
})

export const WorkbenchNoticeItemSchema = Schema.Struct({
  kind: Schema.Literal("notice"),
  source: WorkbenchItemSourceSchema,
  severity: Schema.optional(Schema.Literals(["info", "warning", "error"])),
  text: BoundedString(400),
})

export const WorkbenchCompactionItemSchema = Schema.Struct({
  kind: Schema.Literal("compaction"),
  source: WorkbenchItemSourceSchema,
})

export const WorkbenchSleepItemSchema = Schema.Struct({
  kind: Schema.Literal("sleep"),
  source: WorkbenchItemSourceSchema,
  durationMs: Schema.Number,
})

export const WorkbenchReviewItemSchema = Schema.Struct({
  kind: Schema.Literal("review"),
  source: WorkbenchItemSourceSchema,
  phase: Schema.Literals(["entered", "exited"]),
  review: BoundedString(2_000),
})

export const WorkbenchHookItemSchema = Schema.Struct({
  kind: Schema.Literal("hook"),
  source: WorkbenchItemSourceSchema,
  text: BoundedString(2_000),
})

/**
 * The harness-neutral item union. Wave-0 (#8859) live-emits the command,
 * fileChange, and toolCall variants; the remaining variants define the
 * contract wave-2 component lanes render and later emitters fill.
 */
export const WorkbenchItemSchema = Schema.Union([
  WorkbenchMessageItemSchema,
  WorkbenchReasoningItemSchema,
  WorkbenchCommandItemSchema,
  WorkbenchFileChangeItemSchema,
  WorkbenchToolCallItemSchema,
  WorkbenchAgentItemSchema,
  WorkbenchPlanItemSchema,
  WorkbenchApprovalItemSchema,
  WorkbenchMeterItemSchema,
  WorkbenchNoticeItemSchema,
  WorkbenchCompactionItemSchema,
  WorkbenchSleepItemSchema,
  WorkbenchReviewItemSchema,
  WorkbenchHookItemSchema,
])
export type WorkbenchItem = typeof WorkbenchItemSchema.Type

export const decodeWorkbenchItem = (value: unknown): WorkbenchItem | null => {
  const result = Schema.decodeUnknownExit(WorkbenchItemSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

// ---------------------------------------------------------------------------
// Tolerant projection from provider wire items.
//
// The Codex app-server speaks camelCase JSON-RPC (`aggregatedOutput`,
// `exitCode`); rollout JSONL and the `codex exec --json` stream speak
// snake_case (`aggregated_output`, `exit_code`). One tolerant reader covers
// both so live turns and history rebuild the SAME model, and an older or
// newer app-server that drops/renames a field degrades to fewer optional
// fields instead of a decode failure (the compatibility posture of
// `@openagentsinc/codex-app-server-protocol`). Fixtures for the camelCase
// side are validated against the generated current-source wire documents
// (the protocol package's Ajv notification decoder) in the unit tests so
// this reader provably tracks the wire contract.
// ---------------------------------------------------------------------------

/** Host redaction hook (e.g. `redactChildText`); identity when a lane has no redactor. */
export type WorkbenchRedactor = (value: string) => string
const identity: WorkbenchRedactor = value => value

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
const asString = (value: unknown): string | null => typeof value === "string" ? value : null
const asNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const head = (value: string, limit: number): string => value.slice(0, limit)
const tail = (value: string, limit: number): string => value.slice(-limit)

const normalizeWorkbenchStatus = (
  value: unknown,
  fallback: WorkbenchItemStatus,
): WorkbenchItemStatus => {
  const raw = asString(value)?.toLowerCase() ?? ""
  if (raw === "inprogress" || raw === "in_progress" || raw === "started" || raw === "running") {
    return "in_progress"
  }
  if (raw === "completed" || raw === "complete" || raw === "success") return "completed"
  if (raw === "failed" || raw === "errored" || raw === "error") return "failed"
  if (raw === "declined") return "declined"
  return fallback
}

/** Bounded k/v projection of a JSON arguments payload (objects only). */
export const workbenchArgEntries = (
  value: unknown,
  redact: WorkbenchRedactor = identity,
): ReadonlyArray<WorkbenchArgEntry> => {
  const record = asRecord(value)
  if (record === null) return []
  const entries: Array<WorkbenchArgEntry> = []
  for (const [key, raw] of Object.entries(record)) {
    if (entries.length >= WORKBENCH_ARG_LIMIT) break
    const rendered = typeof raw === "string"
      ? raw
      : typeof raw === "number" || typeof raw === "boolean"
        ? String(raw)
        : raw === null || raw === undefined
          ? ""
          : (() => { try { return JSON.stringify(raw) ?? "" } catch { return "" } })()
    entries.push({
      key: head(key, WORKBENCH_ARG_KEY_LIMIT),
      value: head(redact(rendered), WORKBENCH_ARG_VALUE_LIMIT),
    })
  }
  return entries
}

export const WORKBENCH_PLAN_ENTRY_LIMIT = 64
export const WORKBENCH_PLAN_STEP_LIMIT = 400
export const WORKBENCH_PLAN_PROSE_LIMIT = 4_000

export type WorkbenchPlanEntryInput = Readonly<{
  step: string
  status: "pending" | "in_progress" | "completed"
}>

/**
 * Builds a bounded, redacted `WorkbenchPlanItem` from typed entries and/or
 * free-form prose (T8 #8865 plan unification). This is the ONE constructor
 * every plan source uses — the `turn/plan/updated` notification (structured
 * entries), the `plan` ThreadItem (prose only, see `workbenchItemFromThreadItem`
 * below), and history `plan`/`todo_list` rows (`codex-history.ts`, structured
 * entries with a prose fallback) — so all three project into the identical
 * shape `dispatchWorkbenchItem`'s "plan" branch renders through one
 * `DesktopPlanCard`.
 */
export const workbenchPlanItemFromEntries = (
  input: Readonly<{
    source: WorkbenchItemSource
    entries: ReadonlyArray<WorkbenchPlanEntryInput>
    prose?: string
  }>,
  redact: WorkbenchRedactor = identity,
): WorkbenchPlanItem => {
  const prose = input.prose === undefined ? "" : redact(input.prose).trim()
  return {
    kind: "plan",
    source: input.source,
    entries: input.entries.slice(0, WORKBENCH_PLAN_ENTRY_LIMIT).map(entry => ({
      step: head(redact(entry.step), WORKBENCH_PLAN_STEP_LIMIT),
      status: entry.status,
    })),
    ...(prose === "" ? {} : { prose: head(prose, WORKBENCH_PLAN_PROSE_LIMIT) }),
  }
}

const diffLineCounts = (diff: string): Readonly<{ adds: number; dels: number }> => {
  let adds = 0
  let dels = 0
  for (const line of diff.split("\n")) {
    if (line.startsWith("+") && !line.startsWith("+++")) adds++
    else if (line.startsWith("-") && !line.startsWith("---")) dels++
  }
  return { adds, dels }
}

const changeKind = (value: unknown): "add" | "delete" | "update" | null => {
  const raw = asString(value) ?? asString(asRecord(value)?.type)
  return raw === "add" || raw === "delete" || raw === "update" ? raw : null
}

const boundedFileChange = (
  path: string,
  kind: "add" | "delete" | "update",
  diff: string,
  redact: WorkbenchRedactor,
): WorkbenchFileChangeEntry => {
  const redactedDiff = redact(diff)
  const boundedDiff = head(redactedDiff, WORKBENCH_DIFF_LIMIT)
  const counts = diffLineCounts(boundedDiff)
  return {
    path: head(redact(path), WORKBENCH_PATH_LIMIT),
    kind,
    adds: counts.adds,
    dels: counts.dels,
    diff: boundedDiff,
    ...(redactedDiff.length > WORKBENCH_DIFF_LIMIT ? { diffCapReached: true } : {}),
  }
}

/**
 * Parse a bounded unified/apply_patch document into the same file-change
 * contract used by current app-server items and retained rollout history.
 */
export const workbenchFileChangeItemFromDiff = (
  diff: string,
  source: WorkbenchItemSource,
  status: WorkbenchItemStatus = "in_progress",
  scope: "item" | "turn" = "turn",
  redact: WorkbenchRedactor = identity,
): WorkbenchFileChangeItem => {
  const changes: Array<WorkbenchFileChangeEntry> = []
  const gitStarts = [...diff.matchAll(/^diff --git a\/(.+?) b\/(.+)$/gmu)]
  for (let index = 0; index < Math.min(gitStarts.length, WORKBENCH_CHANGE_LIMIT); index++) {
    const match = gitStarts[index]!
    const next = gitStarts[index + 1]
    const section = diff.slice(match.index, next?.index ?? diff.length)
    const oldPath = match[1] ?? ""
    const newPath = match[2] ?? oldPath
    const kind = /^new file mode\b/mu.test(section) || /^--- \/dev\/null$/mu.test(section)
      ? "add"
      : /^deleted file mode\b/mu.test(section) || /^\+\+\+ \/dev\/null$/mu.test(section)
        ? "delete"
        : "update"
    changes.push(boundedFileChange(kind === "delete" ? oldPath : newPath, kind, section, redact))
  }

  if (changes.length === 0) {
    const patchPattern = /^\*\*\* (Add|Delete|Update) File: (.+)$/gmu
    const patchStarts = [...diff.matchAll(patchPattern)]
    for (let index = 0; index < Math.min(patchStarts.length, WORKBENCH_CHANGE_LIMIT); index++) {
      const match = patchStarts[index]!
      const next = patchStarts[index + 1]
      const section = diff.slice(match.index, next?.index ?? diff.length).replace(/\n\*\*\* End Patch\s*$/u, "")
      const kind = match[1] === "Add" ? "add" : match[1] === "Delete" ? "delete" : "update"
      changes.push(boundedFileChange(match[2] ?? "Unknown file", kind, section, redact))
    }
  }

  if (changes.length === 0 && diff !== "") {
    changes.push(boundedFileChange("Turn diff", "update", diff, redact))
  }
  return { kind: "fileChange", source, status, scope, changes }
}

/** Bounded text extraction from an MCP result payload ({content:[{text}...]}). */
const mcpResultSnippet = (value: unknown, redact: WorkbenchRedactor): string | null => {
  if (value === null || value === undefined) return null
  const record = asRecord(value)
  const content = record === null ? null : record.content
  if (Array.isArray(content)) {
    const text = content
      .map(part => asString(asRecord(part)?.text) ?? "")
      .filter(part => part !== "")
      .join("\n")
    if (text !== "") return head(redact(text), WORKBENCH_RESULT_SNIPPET_LIMIT)
  }
  if (typeof value === "string") return head(redact(value), WORKBENCH_RESULT_SNIPPET_LIMIT)
  try {
    const rendered = JSON.stringify(value)
    return rendered === undefined ? null : head(redact(rendered), WORKBENCH_RESULT_SNIPPET_LIMIT)
  } catch {
    return null
  }
}

/**
 * Projects one provider thread/rollout item record into a typed
 * `WorkbenchItem`, or null when the record is not a tool-class item this
 * wave emits typed payloads for. Tolerant by construction: absent or
 * drifted fields become absent optional fields, never a throw.
 */
export const workbenchItemFromThreadItem = (
  item: Record<string, unknown>,
  source: WorkbenchItemSource,
  redact: WorkbenchRedactor = identity,
): WorkbenchItem | null => {
  const type = asString(item.type) ?? ""
  if (type === "commandExecution" || type === "command_execution") {
    const cwd = asString(item.cwd)
    const exitCode = asNumber(item.exitCode ?? item.exit_code)
    const durationMs = asNumber(item.durationMs ?? item.duration_ms)
    const output = asString(item.aggregatedOutput ?? item.aggregated_output)
    const commandSource = asString(item.source)
    return {
      kind: "command",
      source,
      command: head(redact(asString(item.command) ?? ""), WORKBENCH_COMMAND_LIMIT),
      ...(cwd === null ? {} : { cwd: head(redact(cwd), WORKBENCH_PATH_LIMIT) }),
      status: normalizeWorkbenchStatus(item.status, "in_progress"),
      ...(exitCode === null && (item.exitCode === undefined && item.exit_code === undefined)
        ? {}
        : { exitCode: Number.isInteger(exitCode) ? exitCode : null }),
      ...(durationMs === null ? {} : { durationMs }),
      ...(output === null ? {} : { outputTail: tail(redact(output), WORKBENCH_OUTPUT_TAIL_LIMIT) }),
      ...(output !== null && output.length > WORKBENCH_OUTPUT_TAIL_LIMIT
        ? { outputCapReached: true }
        : {}),
      ...(commandSource === "agent" || commandSource === "userShell" ||
          commandSource === "unifiedExecStartup" || commandSource === "unifiedExecInteraction"
        ? { commandSource }
        : {}),
    }
  }
  if (type === "fileChange" || type === "file_change") {
    const rawChanges = Array.isArray(item.changes) ? item.changes : []
    const changes: Array<WorkbenchFileChangeEntry> = []
    for (const raw of rawChanges.slice(0, WORKBENCH_CHANGE_LIMIT)) {
      const change = asRecord(raw)
      if (change === null) continue
      const path = asString(change.path)
      if (path === null) continue
      const kind = changeKind(change.kind) ?? "update"
      const diff = asString(change.diff)
      if (diff === null) changes.push({ path: head(redact(path), WORKBENCH_PATH_LIMIT), kind })
      else changes.push(boundedFileChange(path, kind, diff, redact))
    }
    return {
      kind: "fileChange",
      source,
      status: normalizeWorkbenchStatus(item.status, "in_progress"),
      scope: "item",
      changes,
    }
  }
  if (type === "apply_patch" || type === "applyPatch") {
    const rawPatch = item.patch ?? item.input ?? item.arguments ?? item.content
    const patchRecord = asRecord(rawPatch) ?? (typeof rawPatch === "string"
      ? (() => { try { return asRecord(JSON.parse(rawPatch)) } catch { return null } })()
      : null)
    const patch = asString(patchRecord?.patch ?? patchRecord?.input ?? rawPatch) ?? ""
    return workbenchFileChangeItemFromDiff(
      patch,
      source,
      normalizeWorkbenchStatus(item.status, "completed"),
      "item",
      redact,
    )
  }
  if (type === "mcpToolCall" || type === "mcp_tool_call") {
    const server = asString(item.server ?? item.server_name)
    const durationMs = asNumber(item.durationMs ?? item.duration_ms)
    const snippet = mcpResultSnippet(item.result, redact)
    const errorMessage = asString(asRecord(item.error)?.message)
    return {
      kind: "toolCall",
      source,
      callKind: "mcp",
      tool: head(redact(asString(item.tool ?? item.tool_name ?? item.name) ?? "tool"), 120),
      ...(server === null ? {} : { server: head(redact(server), 120) }),
      args: workbenchArgEntries(item.arguments ?? item.args, redact),
      ...(snippet === null ? {} : { resultSnippet: snippet }),
      ...(errorMessage === null ? {} : { errorMessage: head(redact(errorMessage), 400) }),
      ...(durationMs === null ? {} : { durationMs }),
      status: normalizeWorkbenchStatus(item.status, "in_progress"),
    }
  }
  if (type === "dynamicToolCall" || type === "dynamic_tool_call" || type === "custom_tool_call") {
    const namespace = asString(item.namespace)
    const durationMs = asNumber(item.durationMs ?? item.duration_ms)
    const status = item.success === true
      ? "completed" as const
      : item.success === false
        ? "failed" as const
        : normalizeWorkbenchStatus(item.status, "in_progress")
    return {
      kind: "toolCall",
      source,
      callKind: "dynamic",
      tool: head(redact(asString(item.tool ?? item.name) ?? "tool"), 120),
      ...(namespace === null ? {} : { namespace: head(redact(namespace), 120) }),
      args: workbenchArgEntries(item.arguments ?? item.args ?? item.input, redact),
      ...(durationMs === null ? {} : { durationMs }),
      status,
    }
  }
  if (type === "webSearch" || type === "web_search") {
    const query = asString(item.query)
    const resultCount = Array.isArray(item.results) ? item.results.length : null
    return {
      kind: "toolCall",
      source,
      callKind: "web",
      tool: "webSearch",
      args: [],
      ...(query === null ? {} : { query: head(redact(query), 400) }),
      ...(resultCount === null ? {} : { resultCount }),
      status: normalizeWorkbenchStatus(item.status, "completed"),
    }
  }
  if (type === "imageGeneration" || type === "image_generation" ||
    type === "imageView" || type === "image_view") {
    const path = asString(item.savedPath ?? item.saved_path ?? item.path)
    const revised = asString(item.revisedPrompt ?? item.revised_prompt)
    return {
      kind: "toolCall",
      source,
      callKind: "image",
      tool: type === "imageView" || type === "image_view" ? "imageView" : "imageGeneration",
      args: [],
      ...(revised === null
        ? {}
        : { resultSnippet: head(redact(revised), WORKBENCH_RESULT_SNIPPET_LIMIT) }),
      ...(path === null ? {} : { path: head(redact(path), WORKBENCH_PATH_LIMIT) }),
      status: normalizeWorkbenchStatus(item.status, "completed"),
    }
  }
  // T8 (#8865): the `plan` ThreadItem variant (`{id, text, type: "plan"}`,
  // collaboration-mode plan write-ups) previously fell through to `null` here
  // (dropped entirely by the live turn — `toolFacts()` has no "plan" case).
  // It carries prose, never structured entries; `turn/plan/updated` and
  // history `todo_list` rows carry the structured entries side of the same
  // canonical plan model (see `workbenchPlanItemFromEntries`).
  if (type === "plan") {
    const text = asString(item.text)
    if (text === null || text.trim() === "") return null
    return workbenchPlanItemFromEntries({ source, entries: [], prose: text }, redact)
  }
  return null
}

/**
 * Typed toolCall projection for the Claude/Fable SDK lane's `tool_use`
 * content blocks (source-tagged "claude"). `mcp__server__tool` names project
 * as MCP calls with the server segment; every other SDK tool is a dynamic
 * call whose args come from the block input.
 */
export const workbenchToolCallFromSdkUse = (
  input: Readonly<{
    toolName: string
    input: unknown
    status: WorkbenchItemStatus
    resultSnippet?: string
    errorMessage?: string
  }>,
  redact: WorkbenchRedactor = identity,
): WorkbenchToolCallItem => {
  const segments = input.toolName.startsWith("mcp__")
    ? input.toolName.split("__").filter(part => part !== "")
    : null
  const server = segments !== null && segments.length >= 3 ? segments[1] ?? null : null
  const tool = segments !== null && segments.length >= 3
    ? segments.slice(2).join("__")
    : input.toolName
  return {
    kind: "toolCall",
    source: "claude",
    callKind: server === null ? "dynamic" : "mcp",
    tool: head(redact(tool), 120),
    ...(server === null ? {} : { server: head(redact(server), 120) }),
    args: workbenchArgEntries(input.input, redact),
    ...(input.resultSnippet === undefined
      ? {}
      : { resultSnippet: head(redact(input.resultSnippet), WORKBENCH_RESULT_SNIPPET_LIMIT) }),
    ...(input.errorMessage === undefined
      ? {}
      : { errorMessage: head(redact(input.errorMessage), 400) }),
    status: input.status,
  }
}

/**
 * Cheap scalar signature for memo/equality checks in the renderer. Long
 * fields contribute their lengths (content changes flip the signature)
 * without stringifying multi-kilobyte diffs on every comparison.
 */
export const workbenchItemSignature = (item: WorkbenchItem | undefined): string => {
  if (item === undefined) return ""
  switch (item.kind) {
    case "command":
      return [
        "command", item.source, item.status, item.command.length, item.cwd ?? "",
        item.exitCode ?? "x", item.durationMs ?? "x", item.outputTail ?? "",
        item.outputCapReached === true ? "capped" : "complete",
        item.commandSource ?? "",
      ].join("|")
    case "fileChange":
      return [
        "fileChange", item.source, item.status, item.scope ?? "item", item.changes.length,
        ...item.changes.map(change =>
          `${change.path}:${change.kind}:${change.adds ?? "x"}:${change.dels ?? "x"}:${workbenchTextSignature(change.diff)}:${change.diffCapReached === true ? "capped" : "complete"}`),
      ].join("|")
    case "toolCall":
      return [
        "toolCall", item.source, item.callKind, item.status, item.tool, item.server ?? "",
        item.namespace ?? "", item.args.length, item.resultSnippet?.length ?? 0,
        item.errorMessage ?? "", item.durationMs ?? "x", item.query ?? "",
        item.resultCount ?? "x", item.path ?? "",
      ].join("|")
    case "message":
      return ["message", item.source, item.role, item.text.length, item.phase ?? ""].join("|")
    case "reasoning":
      return ["reasoning", item.source, item.summary.length].join("|")
    case "agent":
      return ["agent", item.source, item.status, item.tool ?? "", item.prompt?.length ?? 0].join("|")
    case "plan":
      return [
        "plan", item.source, item.prose?.length ?? 0,
        ...item.entries.map(entry => `${entry.status}:${entry.step.length}`),
      ].join("|")
    case "approval":
      return ["approval", item.source, item.status, item.decision ?? "", item.detail ?? ""].join("|")
    case "meter":
      return ["meter", item.source, item.totalTokens ?? "x", item.inputTokens ?? "x", item.outputTokens ?? "x"].join("|")
    case "notice":
      return ["notice", item.source, item.severity ?? "", item.text].join("|")
    case "compaction":
      return ["compaction", item.source].join("|")
    case "sleep":
      return ["sleep", item.source, item.durationMs].join("|")
    case "review":
      return ["review", item.source, item.phase, item.review.length].join("|")
    case "hook":
      return ["hook", item.source, item.text.length].join("|")
  }
}

const workbenchTextSignature = (value: string | undefined): string => {
  if (value === undefined) return "0:0"
  let hash = 2_166_136_261
  for (let index = 0; index < value.length; index++) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 16_777_619)
  }
  return `${value.length}:${hash >>> 0}`
}
