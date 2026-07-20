import type { CodexChildUsage } from "./codex-child-contract.ts"
import {
  registerProductSpecSkill,
  type CodexAppServerClient,
  type CodexAppServerMessage,
  type CodexAppServerRequest,
  type CodexAppServerSpawn,
} from "./codex-app-server-client.ts"
import {
  createCodexAppServerSupervisor,
  type CodexAppServerLease,
  type CodexAppServerSupervisor,
} from "./codex-app-server-supervisor.ts"
import { CLAUDE_LOCAL_SUMMARY_LIMIT, type ClaudeLocalEvent, type ClaudeLocalRateLimitWindow } from "./claude-local-contract.ts"
import { makeCodexTurnState } from "./codex-turn-state.ts"
import {
  WORKBENCH_OUTPUT_TAIL_LIMIT,
  WORKBENCH_REASONING_SUMMARY_LIMIT,
  workbenchFileChangeItemFromDiff,
  workbenchItemFromThreadItem,
  workbenchNoticeItem,
  type WorkbenchCommandItem,
  type WorkbenchFileChangeItem,
  type WorkbenchReasoningItem,
} from "./workbench-item-contract.ts"

export type CodexAppServerTurnOutcome = Readonly<{
  outcome: "success" | "reconnect_required" | "incompatible_workflow" | "failed" | "timeout" | "interrupted"
  text: string
  usage: CodexChildUsage | null
  threadId: string | null
  detail: string
  preContent: boolean
  policyDenied: boolean
  quotaExhausted: boolean
  rateLimited: boolean
}>

export type CodexAppServerTurnControl = {
  interrupted: boolean
  interrupt: (() => void) | null
  /** Inject text into this exact active provider turn. */
  steer: ((message: string, expectedTurnId?: string, clientUserMessageId?: string) => Promise<boolean>) | null
}

type ProductSpecSkill = Readonly<{ skillRoot: string; skillPath: string }>

export type RunCodexAppServerTurnInput = Readonly<{
  binary: string
  env: NodeJS.ProcessEnv
  workspace: string
  runtimeCwd?: string
  hostTarget?: string
  supervisor?: CodexAppServerSupervisor
  threadRef: string
  turnRef: string
  /** Stable user intent identity; survives queue promotion retries/restarts. */
  clientUserMessageId?: string
  accountRef: string
  prompt: string
  imagePaths: ReadonlyArray<string>
  resumeThreadId: string | null
  model: string
  reasoningEffort: string
  /** Generated protocol extensions: apps/plugins/mentions/remote images/extra skills. */
  additionalInput?: ReadonlyArray<Readonly<Record<string, unknown>>>
  /** Reconciled extension identities; admission is main-owned and must fail before turn/start. */
  extensionSelection?: Readonly<{ skillIds?: ReadonlyArray<string>; appIds?: ReadonlyArray<string>; pluginIds?: ReadonlyArray<string> }>
  admitExtensions?: (selection: Readonly<{ skillIds?: ReadonlyArray<string>; appIds?: ReadonlyArray<string>; pluginIds?: ReadonlyArray<string> }>) => void | Promise<void>
  /** Full current thread/start option surface; canonical identity/cwd fields below win. */
  threadStartOptions?: Readonly<Record<string, unknown>>
  /** Full current turn/start controls (schema, tier, personality, collaboration, roots, context, metadata). */
  turnStartOptions?: Readonly<Record<string, unknown>>
  productSpecSkill: ProductSpecSkill
  productSpecDynamicTools?: ReadonlyArray<Readonly<Record<string, unknown>>>
  onProductSpecToolCall?: (request: CodexAppServerRequest) => Promise<unknown | null>
  ephemeral?: boolean
  sandbox?: "read-only" | "danger-full-access"
  includeProductSpecSkill?: boolean
  approvalPolicy?: "never" | "on-request"
  control: CodexAppServerTurnControl
  emit: (event: ClaudeLocalEvent) => void
  spawnImpl?: CodexAppServerSpawn
  requestTimeoutMs?: number
  turnTimeoutMs?: number
  onServerRequest?: (request: CodexAppServerRequest) => Promise<unknown>
  onProviderSession?: (threadId: string) => void
  onProviderTurn?: (turnId: string) => void
  turnReceiptPath?: string
}>

const record = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null

const string = (value: unknown): string | null => typeof value === "string" ? value : null

const number = (value: unknown): number | undefined => typeof value === "number" ? value : undefined

/**
 * T9 #8866: `item/autoApprovalReview/started|completed` (the Guardian
 * background auto-reviewer) were previously fully unconsumed. Honest
 * classification, NOT a "notice" masquerading as an approval: this is an
 * automated risk assessment of an action (`decisionSource: "agent"`, never a
 * human choice), not the interactive tool_approval/plan_review decision the
 * user makes on `DesktopQuestionCard`. Reusing the `approval` WorkbenchItem
 * kind here would conflate the two — an automated reviewer's verdict is not
 * the same thing as a user's approve/deny, and `DesktopApprovalCard`'s
 * onDecision/approved/denied model implies an actionable user decision that
 * does not exist for this event. It renders instead as a bounded, read-only
 * `lane_notice` system line (the same treatment already given to compat/
 * rotation notices in this file), never a card offering a decision no one
 * can make.
 */
const guardianReviewActionLabel = (action: Record<string, unknown> | null): string => {
  const type = string(action?.type)
  if (type === "command") return `command: ${string(action?.command)?.slice(0, 200) ?? "unknown"}`
  if (type === "execve") return `exec: ${string(action?.program)?.slice(0, 200) ?? "unknown"}`
  if (type === "applyPatch") return "file changes"
  if (type === "networkAccess") return `network: ${string(action?.target)?.slice(0, 200) ?? "unknown"}`
  if (type === "mcpToolCall") return `tool: ${string(action?.toolName)?.slice(0, 200) ?? "unknown"}`
  if (type === "requestPermissions") return "permission request"
  return "review"
}

const guardianReviewStatusLabel = (status: string | null): string => {
  switch (status) {
    case "approved": return "approved"
    case "denied": return "denied"
    case "timedOut": return "timed out"
    case "aborted": return "aborted"
    default: return "in progress"
  }
}

const usageFromNotification = (params: Record<string, unknown>): CodexChildUsage | null => {
  const tokenUsage = record(params.tokenUsage)
  const last = record(tokenUsage?.last)
  if (last === null) return null
  const inputTokens = typeof last.inputTokens === "number" ? last.inputTokens : 0
  const cachedInputTokens = typeof last.cachedInputTokens === "number" ? last.cachedInputTokens : 0
  const outputTokens = typeof last.outputTokens === "number" ? last.outputTokens : 0
  const reasoningOutputTokens = typeof last.reasoningOutputTokens === "number" ? last.reasoningOutputTokens : 0
  return {
    inputTokens,
    cachedInputTokens,
    outputTokens,
    reasoningOutputTokens,
    totalTokens: typeof last.totalTokens === "number"
      ? last.totalTokens
      : inputTokens + outputTokens + reasoningOutputTokens,
  }
}

/**
 * T11 #8868: the live-meter projection of `thread/tokenUsage/updated`,
 * additive to `usageFromNotification` above (which still defaults absent
 * fields to `0` for internal turn-outcome accounting). This projection never
 * defaults — an absent wire field stays absent so the ContextMeter renders
 * an honest "—" instead of a fabricated zero.
 */
const meterFromTokenUsageNotification = (
  params: Record<string, unknown>,
): Extract<ClaudeLocalEvent, { kind: "meter_updated" }> | null => {
  const tokenUsage = record(params.tokenUsage)
  const last = record(tokenUsage?.last)
  if (last === null) return null
  const inputTokens = number(last.inputTokens)
  const cachedInputTokens = number(last.cachedInputTokens)
  const outputTokens = number(last.outputTokens)
  const reasoningOutputTokens = number(last.reasoningOutputTokens)
  const totalTokens = number(last.totalTokens)
  if (
    inputTokens === undefined && cachedInputTokens === undefined && outputTokens === undefined &&
    reasoningOutputTokens === undefined && totalTokens === undefined
  ) return null
  return {
    kind: "meter_updated",
    ...(inputTokens === undefined ? {} : { inputTokens }),
    ...(cachedInputTokens === undefined ? {} : { cachedInputTokens }),
    ...(outputTokens === undefined ? {} : { outputTokens }),
    ...(reasoningOutputTokens === undefined ? {} : { reasoningTokens: reasoningOutputTokens }),
    ...(totalTokens === undefined ? {} : { totalTokens }),
  }
}

/**
 * T11 #8868: project `account/rateLimits/updated` (`AccountRateLimitsUpdatedNotification`
 * — previously entirely unconsumed) into the same `meter_updated` event's
 * `rateLimits` field. Codex reports at most a `primary` and `secondary`
 * rolling window; a window the server omitted from this sparse update is
 * simply absent here, never synthesized.
 */
const meterFromRateLimitsNotification = (
  params: Record<string, unknown>,
): Extract<ClaudeLocalEvent, { kind: "meter_updated" }> | null => {
  const rateLimits = record(params.rateLimits)
  if (rateLimits === null) return null
  const windows: Array<ClaudeLocalRateLimitWindow> = []
  for (const label of ["primary", "secondary"] as const) {
    const window = record(rateLimits[label])
    const usedPercent = number(window?.usedPercent)
    if (window === null || usedPercent === undefined) continue
    const resetsAt = number(window.resetsAt)
    const windowDurationMins = number(window.windowDurationMins)
    windows.push({
      label,
      usedPercent,
      ...(resetsAt === undefined ? {} : { resetsAt }),
      ...(windowDurationMins === undefined ? {} : { windowDurationMins }),
    })
  }
  if (windows.length === 0) return null
  return { kind: "meter_updated", rateLimits: windows }
}

const toolFacts = (item: Record<string, unknown>): Readonly<{
  name: string
  summary: string
  ok: boolean
}> | null => {
  switch (item.type) {
    case "commandExecution":
      return {
        name: "Bash",
        summary: JSON.stringify({ command: string(item.command) ?? "" }).slice(0, 400),
        ok: item.status === "completed" && (item.exitCode === null || item.exitCode === 0),
      }
    case "fileChange":
      return {
        name: "FileChange",
        summary: `${Array.isArray(item.changes) ? item.changes.length : 0} file change(s)`,
        ok: item.status === "completed",
      }
    case "mcpToolCall":
      return {
        name: `${string(item.server) ?? "mcp"}.${string(item.tool) ?? "tool"}`.slice(0, 120),
        summary: "",
        ok: item.status === "completed",
      }
    case "dynamicToolCall":
      return {
        name: string(item.tool)?.slice(0, 120) ?? "tool",
        summary: "",
        ok: item.success === true,
      }
    case "webSearch":
      return { name: "WebSearch", summary: "", ok: true }
    case "collabAgentToolCall":
      return {
        name: "Agent",
        summary: (string(item.prompt) ?? string(item.tool) ?? "Delegate agent").slice(0, 400),
        ok: item.status === "completed",
      }
    // Long-tail honest rows (#8869, T12 epic #8857 wave 2): these ThreadItem
    // variants carry no status/exitCode of their own (they are lifecycle
    // facts, not action outcomes), so `ok` is always true — nothing here can
    // "fail", it either happened or it did not arrive. Previously each of
    // these hit the `default: return null` below and the item was dropped
    // WHOLE (never emitted as tool_use/tool_result at all).
    case "hookPrompt": {
      const fragments = Array.isArray(item.fragments) ? item.fragments : []
      const text = fragments
        .map(fragment => string(record(fragment)?.text))
        .filter((value): value is string => value !== null)
        .join(" ")
      return { name: "HookPrompt", summary: text.slice(0, 400), ok: true }
    }
    case "sleep": {
      const durationMs = typeof item.durationMs === "number" ? item.durationMs : 0
      return { name: "Sleep", summary: `${durationMs}ms`, ok: true }
    }
    case "enteredReviewMode":
    case "exitedReviewMode":
      return { name: "ReviewMode", summary: (string(item.review) ?? "").slice(0, 400), ok: true }
    case "contextCompaction":
      return { name: "ContextCompaction", summary: "", ok: true }
    default:
      return null
  }
}

const turnError = (turn: Record<string, unknown>): string => {
  const error = record(turn.error)
  return string(error?.message) ?? "Codex app-server turn failed"
}

const classifyFailure = (
  detail: string,
  text: string,
  usage: CodexChildUsage | null,
  threadId: string | null,
): CodexAppServerTurnOutcome => {
  const lower = detail.toLowerCase()
  const reconnect = lower.includes("unauthorized") || lower.includes("authentication") ||
    lower.includes("login") || lower.includes("credential") || lower.includes("401")
  const policyDenied = lower.includes("denied by policy") || lower.includes("policy denied") ||
    lower.includes("policy violation") || lower.includes("approval policy")
  const quotaExhausted = lower.includes("usage limit") || lower.includes("quota") || lower.includes("purchase more credits")
  const rateLimited = !quotaExhausted && (lower.includes("rate limit") || lower.includes("429") || lower.includes("too many requests"))
  const incompatibleWorkflow = lower.includes("productspec-work") || lower.includes("dynamictools") || lower.includes("dynamic tools")
  return {
    outcome: reconnect ? "reconnect_required" : incompatibleWorkflow ? "incompatible_workflow" : "failed",
    text,
    usage,
    threadId,
    detail,
    preContent: text.trim() === "" && (usage === null || usage.totalTokens === 0),
    policyDenied,
    quotaExhausted,
    rateLimited,
  }
}

/** One app-server process per active turn; persisted Codex thread ids provide restart continuity. */
export const runCodexAppServerTurn = async (
  input: RunCodexAppServerTurnInput,
): Promise<CodexAppServerTurnOutcome> => {
  const sandbox = input.sandbox ?? "danger-full-access"
  const approvalPolicy = input.approvalPolicy ?? "on-request"
  let client: CodexAppServerClient | null = null
  let lease: CodexAppServerLease | null = null
  const supervisor = input.supervisor ?? createCodexAppServerSupervisor()
  const ownsSupervisor = input.supervisor === undefined
  let releaseState: (() => void) | null = null
  let releaseNotification: (() => void) | null = null
  let releaseReverseHandler: (() => void) | null = null
  let threadId: string | null = input.resumeThreadId
  let turnId: string | null = null
  // A shared supervised app-server can publish the tail of an older turn
  // immediately after this listener is installed. Until BOTH provider
  // identities are bound, no turn-scoped notification has authority over the
  // new local turn. Keep only a bounded quarantine and replay it through the
  // exact identity fence once turn/start returns.
  const preBindNotifications: Array<CodexAppServerMessage> = []
  const PRE_BIND_NOTIFICATION_LIMIT = 256
  let text = ""
  let usage: CodexChildUsage | null = null
  const children = new Map<string, {
    parentThreadId: string
    prompt: string
    response: string
    usage: CodexChildUsage | null
    startedAt: number
    terminal: boolean
  }>()
  const pendingTools = new Set<string>()
  const commandStreams = new Map<string, {
    item: WorkbenchCommandItem | null
    outputTail: string
    outputCapReached: boolean
    receivedCharacters: number
  }>()
  let turnDiff: WorkbenchFileChangeItem | null = null
  let turnDiffRef: string | null = null
  // Streaming reasoning (#8863, T6): accumulates `item/reasoning/textDelta` +
  // `item/reasoning/summaryTextDelta` chunks per itemId into a growing ghost
  // text, keyed by the wire item id (not toolName-FIFO — reasoning always
  // carries a stable id). Cleared on `item/completed`.
  const reasoningStreams = new Map<string, { summary: string }>()
  let settle: ((outcome: CodexAppServerTurnOutcome) => void) | null = null
  let settled = false
  let turnTimer: ReturnType<typeof setTimeout> | null = null
  const completion = new Promise<CodexAppServerTurnOutcome>(resolve => { settle = resolve })
  const turnState = makeCodexTurnState({ ...(input.turnReceiptPath === undefined ? {} : { receiptPath: input.turnReceiptPath }) })

  const finish = (outcome: CodexAppServerTurnOutcome): void => {
    if (settled) return
    settled = true
    if (turnTimer !== null) clearTimeout(turnTimer)
    input.control.interrupt = null
    input.control.steer = null
    settle?.(outcome)
  }

  const registerChild = (childRef: string, parentThreadId: string, prompt: string): void => {
    if (children.has(childRef)) return
    children.set(childRef, { parentThreadId, prompt, response: "", usage: null, startedAt: Date.now(), terminal: false })
    const parentChildRef = threadId !== null && parentThreadId !== threadId ? parentThreadId : undefined
    input.emit({
      kind: "child_started",
      childRef: childRef.slice(0, 120),
      ...(parentChildRef === undefined ? {} : { parentChildRef: parentChildRef.slice(0, 120) }),
      accountRef: input.accountRef,
      summary: (prompt.trim() || "Codex child agent").slice(0, 400),
      ...(prompt.trim() === "" ? {} : { prompt: prompt.slice(0, 32_000) }),
    })
  }

  const childParent = (child: { parentThreadId: string }): Readonly<{ parentChildRef?: string }> =>
    threadId !== null && child.parentThreadId !== threadId
      ? { parentChildRef: child.parentThreadId.slice(0, 120) }
      : {}

  const settleChild = (childRef: string, status: string, message: string): void => {
    const child = children.get(childRef)
    if (child === undefined || child.terminal) return
    child.terminal = true
    if (status === "completed") {
      input.emit({
        kind: "child_completed",
        childRef: childRef.slice(0, 120),
        ...childParent(child),
        accountRef: input.accountRef,
        summary: (message || child.response.trim() || "Codex child completed").slice(0, 400),
        ...((child.response.trim() || message) === "" ? {} : { response: (child.response.trim() || message).slice(0, 32_000) }),
        usage: child.usage === null ? null : {
          inputTokens: child.usage.inputTokens,
          cachedInputTokens: child.usage.cachedInputTokens,
          outputTokens: child.usage.outputTokens,
          reasoningTokens: child.usage.reasoningOutputTokens,
          totalTokens: child.usage.totalTokens,
        },
        durationMs: Date.now() - child.startedAt,
      })
    } else {
      input.emit({
        kind: "child_failed",
        childRef: childRef.slice(0, 120),
        ...childParent(child),
        accountRef: input.accountRef || null,
        reason: "child_failed",
        detail: (message || `Codex child ${status}`).slice(0, 400),
      })
    }
  }

  const reconcileAgentStates = (item: Record<string, unknown>, parentThreadId: string): void => {
    const states = record(item.agentsStates)
    if (states === null) return
    for (const [childRef, raw] of Object.entries(states)) {
      const state = record(raw)
      const status = string(state?.status)
      const message = string(state?.message) ?? ""
      registerChild(childRef, parentThreadId, string(item.prompt) ?? message)
      if (status === "completed") settleChild(childRef, status, message)
      else if (status === "errored" || status === "interrupted" || status === "shutdown" || status === "notFound") {
        settleChild(childRef, status, message)
      }
    }
  }

  const handleNotification = (
    message: CodexAppServerMessage,
    source: "live" | "quarantined" = "live",
  ): void => {
    const params = record(message.params)
    if (params === null) return
    if (message.method === "thread/started") {
      const startedThread = record(params.thread)
      const childRef = string(startedThread?.id)
      const parentThreadId = string(startedThread?.parentThreadId)
      if (childRef !== null && parentThreadId !== null &&
        (parentThreadId === threadId || children.has(parentThreadId))) {
        registerChild(childRef, parentThreadId, string(startedThread?.preview) ?? "")
      }
      return
    }
    if (threadId === null || turnId === null) {
      if (preBindNotifications.length === PRE_BIND_NOTIFICATION_LIMIT) {
        preBindNotifications.shift()
      }
      preBindNotifications.push(message)
      return
    }
    turnState.apply({ generation: lease?.state().generation ?? 0, message })
    // Account rate limits are intentionally connection-scoped and carry no
    // thread/turn identity. They remain ordered with the quarantined stream,
    // but never contribute transcript content.
    if (message.method === "account/rateLimits/updated") {
      const meter = meterFromRateLimitsNotification(params)
      if (meter !== null) input.emit(meter)
      return
    }
    const notifiedThread = string(params.threadId)
    const child = notifiedThread === null ? undefined : children.get(notifiedThread)
    if (child !== undefined) {
      if (message.method === "item/agentMessage/delta") {
        const delta = string(params.delta)
        if (delta !== null && delta !== "") {
          child.response += delta
          input.emit({
            kind: "child_activity",
            childRef: notifiedThread!.slice(0, 120),
            ...childParent(child),
            activity: "item",
            accountRef: input.accountRef,
            summary: delta.slice(0, 400),
          })
        }
        return
      }
      if (message.method === "thread/tokenUsage/updated") {
        child.usage = usageFromNotification(params) ?? child.usage
        return
      }
      if (message.method === "item/started" || message.method === "item/completed") {
        const item = record(params.item)
        if (item?.type === "collabAgentToolCall") {
          const receivers = Array.isArray(item.receiverThreadIds)
            ? item.receiverThreadIds.filter(value => typeof value === "string") as string[]
            : []
          for (const receiver of receivers) registerChild(receiver, notifiedThread!, string(item.prompt) ?? "")
          reconcileAgentStates(item, notifiedThread!)
        }
        const facts = item === null ? null : toolFacts(item)
        const summary = facts?.summary || facts?.name || string(item?.type) || "child activity"
        input.emit({
          kind: "child_activity",
          childRef: notifiedThread!.slice(0, 120),
          ...childParent(child),
          activity: "item",
          accountRef: input.accountRef,
          summary: summary.slice(0, 400),
        })
        return
      }
      if (message.method === "turn/completed") {
        const turn = record(params.turn)
        const status = string(turn?.status)
        if (status === "completed") {
          settleChild(notifiedThread!, status, "")
        } else {
          settleChild(notifiedThread!, status ?? "failed", turn === null ? "Codex child failed" : turnError(turn))
        }
        return
      }
      return
    }
    const notifiedTurn = string(params.turnId) ?? string(record(params.turn)?.id)
    // A notification received before binding must present provider identity:
    // missing both is not "probably current" on a reused connection. Some
    // live Codex compatibility notifications legitimately omit both ids, so
    // preserve them only when they arrived after this turn was already bound.
    // Every identity that is present must match the immutable admission.
    const unaffiliatedCompatibilityNotice = message.method === "warning" ||
      message.method === "configWarning" || message.method === "deprecationNotice"
    if (source === "quarantined" && notifiedThread === null && notifiedTurn === null &&
      !unaffiliatedCompatibilityNotice) return
    if (notifiedThread !== null && notifiedThread !== threadId) return
    if (notifiedTurn !== null && notifiedTurn !== turnId) return

    if (message.method === "item/commandExecution/outputDelta") {
      const itemRef = string(params.itemId)
      const delta = string(params.delta)
      if (itemRef === null || delta === null || delta === "") return
      const previous = commandStreams.get(itemRef) ?? {
        item: null,
        outputTail: "",
        outputCapReached: false,
        receivedCharacters: 0,
      }
      const boundedDelta = delta.slice(-WORKBENCH_OUTPUT_TAIL_LIMIT)
      const combined = `${previous.outputTail}${boundedDelta}`
      const outputTail = combined.slice(-WORKBENCH_OUTPUT_TAIL_LIMIT)
      const outputCapReached = previous.outputCapReached ||
        delta.length > WORKBENCH_OUTPUT_TAIL_LIMIT || combined.length > WORKBENCH_OUTPUT_TAIL_LIMIT
      const receivedCharacters = previous.receivedCharacters + delta.length
      const item = previous.item === null ? null : {
        ...previous.item,
        outputTail,
        ...(outputCapReached ? { outputCapReached: true } : {}),
      }
      commandStreams.set(itemRef, { item, outputTail, outputCapReached, receivedCharacters })
      if (item !== null) {
        input.emit({
          kind: "tool_progress",
          toolName: "Bash",
          itemRef: itemRef.slice(0, 120),
          summary: `${receivedCharacters} output character${receivedCharacters === 1 ? "" : "s"}`,
          item,
        })
      }
      return
    }

    if (message.method === "item/fileChange/patchUpdated") {
      const itemRef = string(params.itemId)
      if (itemRef === null) return
      const item = workbenchItemFromThreadItem({
        type: "fileChange",
        status: "inProgress",
        changes: params.changes,
      }, "codex")
      if (item?.kind !== "fileChange") return
      input.emit({
        kind: "tool_progress",
        toolName: "FileChange",
        itemRef: itemRef.slice(0, 120),
        summary: `${item.changes.length} file change(s)`,
        item,
      })
      return
    }

    if (message.method === "turn/diff/updated") {
      const diff = string(params.diff)
      if (diff === null) return
      turnDiff = workbenchFileChangeItemFromDiff(diff, "codex", "in_progress", "turn")
      turnDiffRef = `turn-diff:${notifiedTurn ?? turnId ?? input.turnRef}`.slice(0, 120)
      input.emit({
        kind: "tool_progress",
        toolName: "FileChange",
        itemRef: turnDiffRef,
        summary: `${turnDiff.changes.length} file change(s) in turn`,
        item: turnDiff,
      })
      return
    }

    // Streaming reasoning (#8863, T6): the raw reasoning content stream and
    // the user-facing reasoning-summary stream both contribute ghost text to
    // the SAME growing card — the disclosure only cares about "here is what
    // the model is thinking right now", not which of the two wire streams a
    // chunk came from. First chunk for an itemId opens the card (`tool_use`);
    // later chunks update it in place (`tool_progress`), reusing the exact
    // started/progress note-pairing infrastructure `tool-cards.ts` already
    // gives every FIFO/itemRef-keyed card (toolName "Reasoning").
    if (message.method === "item/reasoning/textDelta" || message.method === "item/reasoning/summaryTextDelta") {
      const itemRef = string(params.itemId)
      const delta = string(params.delta)
      if (itemRef === null || delta === null || delta === "") return
      const wasStreaming = reasoningStreams.has(itemRef)
      const previous = reasoningStreams.get(itemRef) ?? { summary: "" }
      const summary = `${previous.summary}${delta}`.slice(-WORKBENCH_REASONING_SUMMARY_LIMIT)
      reasoningStreams.set(itemRef, { summary })
      const reasoningItem: WorkbenchReasoningItem = { kind: "reasoning", source: "codex", summary, status: "in_progress" }
      input.emit(wasStreaming
        ? { kind: "tool_progress", toolName: "Reasoning", itemRef: itemRef.slice(0, 120), summary: "", item: reasoningItem }
        : { kind: "tool_use", toolName: "Reasoning", itemRef: itemRef.slice(0, 120), summary: "", item: reasoningItem })
      return
    }
    // A new reasoning-summary paragraph is starting (`summaryPartAdded`
    // carries no text itself) — insert a paragraph break so the streamed
    // ghost text reads as separate summary parts rather than one run-on line.
    if (message.method === "item/reasoning/summaryPartAdded") {
      const itemRef = string(params.itemId)
      if (itemRef === null) return
      const previous = reasoningStreams.get(itemRef)
      if (previous === undefined || previous.summary === "" || previous.summary.endsWith("\n\n")) return
      const summary = `${previous.summary}\n\n`.slice(-WORKBENCH_REASONING_SUMMARY_LIMIT)
      reasoningStreams.set(itemRef, { summary })
      input.emit({
        kind: "tool_progress",
        toolName: "Reasoning",
        itemRef: itemRef.slice(0, 120),
        summary: "",
        item: { kind: "reasoning", source: "codex", summary, status: "in_progress" },
      })
      return
    }

    if (message.method === "item/agentMessage/delta") {
      const delta = string(params.delta)
      if (delta !== null && delta !== "") {
        text += delta
        input.emit({ kind: "text_delta", text: delta.slice(0, 2_000) })
      }
      return
    }
    if (message.method === "thread/tokenUsage/updated") {
      usage = usageFromNotification(params) ?? usage
      // T11 #8868: additive live-meter event alongside the accounting use
      // above — never replaces it, never invents a value it lacks.
      const meter = meterFromTokenUsageNotification(params)
      if (meter !== null) input.emit(meter)
      return
    }
    if (message.method === "item/autoApprovalReview/started" || message.method === "item/autoApprovalReview/completed") {
      const review = record(params.review)
      const status = string(review?.status)
      const action = record(params.action)
      const actionLabel = guardianReviewActionLabel(action)
      const rationale = string(review?.rationale)
      const text = message.method === "item/autoApprovalReview/started"
        ? `Guardian review started: ${actionLabel}`
        : `Guardian review ${guardianReviewStatusLabel(status)}: ${actionLabel}${rationale === null ? "" : ` — ${rationale}`}`
      input.emit({ kind: "lane_notice", text: text.slice(0, CLAUDE_LOCAL_SUMMARY_LIMIT) })
      return
    }
    if (message.method === "turn/plan/updated") {
      const entries = Array.isArray(params.plan) ? params.plan.slice(0, 64).flatMap(value => {
        const item = record(value)
        const step = string(item?.step)
        const status = string(item?.status)
        if (step === null || (status !== "pending" && status !== "inProgress" && status !== "completed")) return []
        const projectedStatus: "pending" | "in_progress" | "completed" =
          status === "inProgress" ? "in_progress" : status
        return [{
          step: step.slice(0, 400),
          status: projectedStatus,
        }]
      }) : []
      if (entries.length > 0) input.emit({ kind: "plan_updated", entries })
      return
    }
    if (message.method === "item/started" || message.method === "item/completed") {
      const item = record(params.item)
      if (item === null) return
      if (item.type === "collabAgentToolCall") {
        const receivers = Array.isArray(item.receiverThreadIds)
          ? item.receiverThreadIds.filter(value => typeof value === "string") as string[]
          : []
        for (const receiver of receivers) registerChild(receiver, threadId ?? "", string(item.prompt) ?? "")
        reconcileAgentStates(item, threadId ?? "")
      } else if (item.type === "subAgentActivity") {
        const childRef = string(item.agentThreadId)
        if (childRef !== null) {
          registerChild(childRef, threadId ?? "", string(item.agentPath) ?? "Codex child agent")
          if (item.kind === "interrupted") settleChild(childRef, "interrupted", "Codex child interrupted")
        }
      }
      const id = string(item.id) ?? `${item.type ?? "item"}`
      if (item.type === "agentMessage" && message.method === "item/completed" && text === "") {
        const full = string(item.text)
        if (full !== null && full !== "") {
          text = full
          input.emit({ kind: "text_delta", text: full.slice(0, 2_000) })
        }
        return
      }
      if (item.type === "reasoning" && message.method === "item/completed") {
        const streamed = reasoningStreams.get(id)
        reasoningStreams.delete(id)
        const parts = Array.isArray(item.summary) ? item.summary.filter(value => typeof value === "string") : []
        const joined = parts.join("\n").trim()
        // Prefer the authoritative completed summary; fall back to whatever
        // ghost text already streamed if the completed payload came back
        // empty (still honest — that text was already shown live).
        const finalSummary = joined !== "" ? joined : (streamed?.summary.trim() ?? "")
        // Honest absence: redacted/never-summarized reasoning emits nothing,
        // live or historical, matching `workbenchItemFromThreadItem`.
        if (finalSummary === "") return
        const completedItem: WorkbenchReasoningItem = {
          kind: "reasoning",
          source: "codex",
          summary: finalSummary.slice(0, WORKBENCH_REASONING_SUMMARY_LIMIT),
          status: "completed",
        }
        // A completed item with no visible streaming (fast/no-delta turns):
        // emit the started+completed pair together so the FIFO card pairing
        // in tool-cards.ts stays balanced, matching every other tool item.
        if (streamed === undefined) {
          input.emit({ kind: "tool_use", toolName: "Reasoning", itemRef: id.slice(0, 120), summary: "", item: completedItem })
        }
        input.emit({ kind: "tool_result", toolName: "Reasoning", itemRef: id.slice(0, 120), ok: true, summary: "", item: completedItem })
        return
      }
      // T8 (#8865): the `plan` ThreadItem (`{id, text, type: "plan"}`,
      // collaboration-mode plan write-ups) has no `toolFacts()` case and was
      // silently dropped. It rides the SAME per-turn stable-key plan note as
      // `turn/plan/updated` (`plan_updated` -> `${turnRef}-plan`, latest
      // wins, never remounts) so both plan representations render through one
      // DesktopPlanCard.
      if (item.type === "plan" && message.method === "item/completed") {
        const planText = string(item.text)
        if (planText !== null && planText.trim() !== "") {
          input.emit({ kind: "plan_updated", entries: [], prose: planText.slice(0, 4_000) })
        }
        return
      }
      const facts = toolFacts(item)
      if (facts === null) return
      // Typed payload (#8859): the structured fields toolFacts() flattens
      // (command cwd/exit/duration/output tail, per-file diffs, MCP args/
      // results, web queries) ride the same events additively. The string
      // summary stays populated for pre-wave-2 renderers and older notes.
      let typedItem = workbenchItemFromThreadItem(item, "codex")
      if (typedItem?.kind === "command") {
        const streamed = commandStreams.get(id)
        if (message.method === "item/started") {
          typedItem = {
            ...typedItem,
            ...(typedItem.outputTail === undefined && streamed?.outputTail !== undefined && streamed.outputTail !== ""
              ? { outputTail: streamed.outputTail }
              : {}),
            ...(typedItem.outputCapReached === true || streamed?.outputCapReached === true
              ? { outputCapReached: true }
              : {}),
          }
          commandStreams.set(id, {
            item: typedItem,
            outputTail: typedItem.outputTail ?? streamed?.outputTail ?? "",
            outputCapReached: typedItem.outputCapReached === true || streamed?.outputCapReached === true,
            receivedCharacters: streamed?.receivedCharacters ?? typedItem.outputTail?.length ?? 0,
          })
        } else if (streamed !== undefined) {
          typedItem = {
            ...typedItem,
            ...(typedItem.outputTail === undefined && streamed.outputTail !== ""
              ? { outputTail: streamed.outputTail }
              : {}),
            ...(typedItem.outputCapReached === true || streamed.outputCapReached
              ? { outputCapReached: true }
              : {}),
          }
          commandStreams.delete(id)
        }
      }
      const typed = typedItem === null ? {} : { item: typedItem }
      if (message.method === "item/started") {
        pendingTools.add(id)
        input.emit({ kind: "tool_use", toolName: facts.name, itemRef: id.slice(0, 120), summary: facts.summary, ...typed })
      } else {
        if (!pendingTools.has(id)) {
          input.emit({ kind: "tool_use", toolName: facts.name, itemRef: id.slice(0, 120), summary: facts.summary, ...typed })
        }
        pendingTools.delete(id)
        const output = item.type === "commandExecution" && typeof item.aggregatedOutput === "string"
          ? item.aggregatedOutput.slice(0, 400)
          : facts.summary
        input.emit({ kind: "tool_result", toolName: facts.name, itemRef: id.slice(0, 120), ok: facts.ok, summary: output, ...typed })
      }
      return
    }
    // Notice-class notifications (#8869, T12 epic #8857 wave 2): previously
    // ignored entirely (61 of 69 notifications per the audit) — each now
    // becomes a typed "notice" (or, for the deprecated compaction notice, a
    // "compaction") WorkbenchItem so the timeline shows a quiet, honest row
    // instead of silently swallowing the signal. Severity stays muted per the
    // design spec (info/warning only; these are advisories, not failures).
    if (message.method === "thread/compacted") {
      // Deprecated in favor of the `contextCompaction` ThreadItem above; kept
      // for older app-servers that only ever sent the notification form.
      input.emit({
        kind: "tool_result",
        toolName: "ContextCompaction",
        ok: true,
        summary: "",
        item: { kind: "compaction", source: "codex" },
      })
      return
    }
    if (message.method === "model/rerouted") {
      const fromModel = string(params.fromModel) ?? "unknown"
      const toModel = string(params.toModel) ?? "unknown"
      const noticeText = `MODEL REROUTED · ${fromModel} -> ${toModel}`
      input.emit({
        kind: "tool_result",
        toolName: "ModelRerouted",
        ok: true,
        summary: noticeText.slice(0, 400),
        item: workbenchNoticeItem("codex", "warning", noticeText),
      })
      return
    }
    if (message.method === "warning") {
      const noticeText = string(params.message) ?? "Codex reported a warning"
      input.emit({
        kind: "tool_result",
        toolName: "Warning",
        ok: true,
        summary: noticeText.slice(0, 400),
        item: workbenchNoticeItem("codex", "warning", noticeText),
      })
      return
    }
    if (message.method === "configWarning") {
      const summary = string(params.summary) ?? "Config warning"
      const details = string(params.details)
      const noticeText = details === null ? summary : `${summary}: ${details}`
      input.emit({
        kind: "tool_result",
        toolName: "ConfigWarning",
        ok: true,
        summary: noticeText.slice(0, 400),
        item: workbenchNoticeItem("codex", "warning", noticeText),
      })
      return
    }
    if (message.method === "deprecationNotice") {
      const summary = string(params.summary) ?? "Deprecation notice"
      const details = string(params.details)
      const noticeText = details === null ? summary : `${summary}: ${details}`
      input.emit({
        kind: "tool_result",
        toolName: "DeprecationNotice",
        ok: true,
        summary: noticeText.slice(0, 400),
        item: workbenchNoticeItem("codex", "info", noticeText),
      })
      return
    }
    if (message.method === "guardianWarning") {
      const noticeText = string(params.message) ?? "Guardian warning"
      input.emit({
        kind: "tool_result",
        toolName: "GuardianWarning",
        ok: true,
        summary: noticeText.slice(0, 400),
        item: workbenchNoticeItem("codex", "warning", noticeText),
      })
      return
    }
    if (message.method === "error") {
      const error = record(params.error)
      if (params.willRetry !== true) finish(classifyFailure(
        string(error?.message) ?? "Codex app-server error",
        text,
        usage,
        threadId,
      ))
      return
    }
    if (message.method === "turn/completed") {
      const turn = record(params.turn)
      if (turn === null) return
      const status = string(turn.status)
      if (turnDiff !== null && turnDiffRef !== null) {
        const completedDiff: WorkbenchFileChangeItem = {
          ...turnDiff,
          status: status === "completed" ? "completed" : "failed",
        }
        input.emit({
          kind: "tool_result",
          toolName: "FileChange",
          itemRef: turnDiffRef,
          ok: status === "completed",
          summary: `${completedDiff.changes.length} file change(s) in turn`,
          item: completedDiff,
        })
      }
      if (status === "completed" && text.trim() !== "") {
        finish({ outcome: "success", text, usage, threadId, detail: "", preContent: false, policyDenied: false, quotaExhausted: false, rateLimited: false })
      } else if (status === "interrupted" || input.control.interrupted) {
        finish({ outcome: "interrupted", text, usage, threadId, detail: "turn interrupted", preContent: text === "", policyDenied: false, quotaExhausted: false, rateLimited: false })
      } else {
        finish(classifyFailure(
          status === "completed" ? "the turn produced no agent message text" : turnError(turn),
          text,
          usage,
          threadId,
        ))
      }
    }
  }

  try {
    const target = {
      binary: input.binary,
      env: input.env,
      cwd: input.runtimeCwd ?? input.workspace,
      accountRef: input.accountRef,
      hostTarget: input.hostTarget ?? "local-desktop",
      ...(input.spawnImpl === undefined ? {} : { spawnImpl: input.spawnImpl }),
      ...(input.requestTimeoutMs === undefined ? {} : { requestTimeoutMs: input.requestTimeoutMs }),
    }
    lease = await supervisor.acquire(target)
    const activeLease = lease
    // Compatibility receipts are private connection diagnostics and release
    // gates. They are never conversation content and must not become chat
    // notices merely because the installed Codex added telemetry fields.
    client = {
      initialize: async () => undefined,
      request: (method, params, options) => activeLease.request(method, params, options),
      notify: (method, params) => activeLease.notify(method, params),
      onNotification: listener => activeLease.subscribe(notification => listener(notification.message)),
      isClosed: () => activeLease.state().status === "closed",
      close: () => activeLease.release(),
    }
    if (input.extensionSelection !== undefined) {
      if (input.admitExtensions === undefined) throw new Error("Codex extension authority is unavailable")
      await input.admitExtensions(input.extensionSelection)
    }
    releaseState = supervisor.subscribeState((identity, state) => {
      if (identity.binary !== activeLease.identity.binary ||
        identity.binarySha256 !== activeLease.identity.binarySha256 ||
        identity.codexHome !== activeLease.identity.codexHome ||
        identity.accountRef !== activeLease.identity.accountRef ||
        identity.hostTarget !== activeLease.identity.hostTarget ||
        state.status === "ready") return
      if (state.status === "degraded" || state.status === "repairing") {
        finish({
          outcome: "reconnect_required",
          text,
          usage,
          threadId,
          detail: state.status === "degraded" ? state.reason : "Codex app-server connection is repairing",
          preContent: text === "",
          policyDenied: false,
          quotaExhausted: false,
          rateLimited: false,
        })
      }
    })
    releaseReverseHandler = lease.registerReverseHandler(async (request: CodexAppServerRequest) => {
      if (request.method === "item/tool/call" && input.onProductSpecToolCall !== undefined) {
        const result = await input.onProductSpecToolCall(request)
        if (result !== null) return result
      }
      if (input.onServerRequest !== undefined) return input.onServerRequest(request)
      throw new Error(`Unsupported Codex server request: ${request.method}`)
    })
    releaseNotification = client.onNotification(handleNotification)
    if (input.includeProductSpecSkill !== false) {
      await registerProductSpecSkill({
        client,
        cwd: input.workspace,
        skillRoot: input.productSpecSkill.skillRoot,
        skillPath: input.productSpecSkill.skillPath,
      })
    } else {
      await client.initialize()
    }
    const threadResponse = record(await client.request(
      input.resumeThreadId === null ? "thread/start" : "thread/resume",
      input.resumeThreadId === null
        ? {
            ...(input.threadStartOptions ?? {}),
            model: input.model,
            cwd: input.workspace,
            approvalPolicy,
            sandbox,
            ephemeral: input.ephemeral ?? false,
            threadSource: "appServer",
            ...(input.productSpecDynamicTools === undefined ? {} : { dynamicTools: input.productSpecDynamicTools }),
          }
        : {
            threadId: input.resumeThreadId,
            model: input.model,
            cwd: input.workspace,
            approvalPolicy,
            sandbox,
          },
    ))
    const thread = record(threadResponse?.thread)
    threadId = string(thread?.id)
    if (threadId === null) throw new Error("Codex app-server omitted thread identity")
    input.onProviderSession?.(threadId)
    if (input.ephemeral !== true) lease.registerVisibleThread(threadId)

    const userInput: Array<Record<string, unknown>> = [
      { type: "text", text: input.prompt, text_elements: [] },
      ...input.imagePaths.map(path => ({ type: "localImage", path })),
      ...(input.includeProductSpecSkill === false ? [] : [{ type: "skill", name: "productspec-work", path: input.productSpecSkill.skillPath }]),
      ...(input.additionalInput ?? []),
    ]
    const turnResponse = record(await client.request("turn/start", {
      ...(input.turnStartOptions ?? {}),
      threadId,
      clientUserMessageId: input.clientUserMessageId ?? input.turnRef,
      input: userInput,
      cwd: input.workspace,
      model: input.model,
      effort: input.reasoningEffort,
      approvalPolicy,
      sandboxPolicy: sandbox === "read-only"
        ? { type: "readOnly", networkAccess: true }
        : { type: "dangerFullAccess" },
    }))
    turnId = string(record(turnResponse?.turn)?.id)
    if (turnId === null) throw new Error("Codex app-server omitted turn identity")
    turnState.bindStartedTurn(threadId, turnId)
    input.onProviderTurn?.(turnId)
    const quarantined = preBindNotifications.splice(0)
    for (const notification of quarantined) handleNotification(notification, "quarantined")
    input.control.interrupt = () => {
      if (client !== null && threadId !== null && turnId !== null) {
        if (!turnState.admitInterrupt(threadId, turnId)) return
        // ACK means interruption was admitted only. Completion still waits for
        // turn/completed or later durable reconciliation.
        void client.request("turn/interrupt", { threadId, turnId }).catch(() => undefined)
      }
    }
    input.control.steer = async (message, expectedTurnId, clientUserMessageId) => {
      if (client === null || threadId === null || turnId === null || message.trim() === "") return false
      if (expectedTurnId !== undefined && expectedTurnId !== turnId) return false
      const authorization = turnState.authorizeSteer(threadId, turnId, clientUserMessageId)
      if (!authorization.accepted) {
        turnState.settleSteer(threadId, turnId, authorization.clientUserMessageId, false)
        return false
      }
      try {
        await client.request("turn/steer", {
          threadId,
          clientUserMessageId: authorization.clientUserMessageId,
          input: [{ type: "text", text: message, text_elements: [] }],
          expectedTurnId: turnId,
        })
        turnState.settleSteer(threadId, turnId, authorization.clientUserMessageId, true)
        return true
      } catch {
        turnState.settleSteer(threadId, turnId, authorization.clientUserMessageId, false)
        return false
      }
    }
    if (input.control.interrupted) input.control.interrupt()

    if (input.turnTimeoutMs !== undefined) {
      turnTimer = setTimeout(() => {
        input.control.interrupt?.()
        finish({
          outcome: "timeout",
          text,
          usage,
          threadId,
          detail: `test deadline reached (${Math.round(input.turnTimeoutMs! / 1_000)}s)`,
          preContent: text === "",
          policyDenied: false,
          quotaExhausted: false,
          rateLimited: false,
        })
      }, input.turnTimeoutMs)
    }
    const outcome = await completion
    return outcome
  } catch (error) {
    return classifyFailure(error instanceof Error ? error.message : "Codex app-server failed", text, usage, threadId)
  } finally {
    input.control.interrupt = null
    input.control.steer = null
    releaseNotification?.()
    releaseReverseHandler?.()
    releaseState?.()
    client?.close()
    if (ownsSupervisor) supervisor.close()
  }
}
