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
import type { FableLocalEvent, FableLocalRateLimitWindow } from "./fable-local-contract.ts"
import { makeCodexTurnState } from "./codex-turn-state.ts"
import {
  WORKBENCH_OUTPUT_TAIL_LIMIT,
  workbenchFileChangeItemFromDiff,
  workbenchItemFromThreadItem,
  type WorkbenchCommandItem,
  type WorkbenchFileChangeItem,
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
  emit: (event: FableLocalEvent) => void
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
): Extract<FableLocalEvent, { kind: "meter_updated" }> | null => {
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
): Extract<FableLocalEvent, { kind: "meter_updated" }> | null => {
  const rateLimits = record(params.rateLimits)
  if (rateLimits === null) return null
  const windows: Array<FableLocalRateLimitWindow> = []
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
  let releaseCompatibility: (() => void) | null = null
  let releaseReverseHandler: (() => void) | null = null
  let threadId: string | null = input.resumeThreadId
  let turnId: string | null = null
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

  const handleNotification = (message: CodexAppServerMessage): void => {
    turnState.apply({ generation: lease?.state().generation ?? 0, message })
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
    if (threadId !== null && notifiedThread !== null && notifiedThread !== threadId) return
    const notifiedTurn = string(params.turnId) ?? string(record(params.turn)?.id)
    if (turnId !== null && notifiedTurn !== null && notifiedTurn !== turnId) return

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
    // T11 #8868: previously entirely unconsumed. Account-scoped (no
    // threadId/turnId params), so it is not gated by the thread/turn checks
    // above — every rolling update for this account-level connection applies.
    if (message.method === "account/rateLimits/updated") {
      const meter = meterFromRateLimitsNotification(params)
      if (meter !== null) input.emit(meter)
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
        const parts = Array.isArray(item.summary) ? item.summary.filter(value => typeof value === "string") : []
        const summary = parts.join("\n").slice(0, 400)
        if (summary !== "") input.emit({ kind: "reasoning", text: summary })
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
    const visibleCompatibility = new Set<string>()
    const reportCompatibility = (method: string, reason: string): void => {
      const key = `${method}\u0000${reason}`
      if (visibleCompatibility.has(key)) return
      visibleCompatibility.add(key)
      input.emit({
        kind: "lane_notice",
        text: `Codex compatibility notice: ${method.slice(0, 160)} (${reason}). The unrecognized provider event was retained privately.`,
      })
    }
    for (const receipt of lease.compatibilityReceipts()) reportCompatibility(receipt.method, receipt.reason)
    releaseCompatibility = lease.subscribeCompatibility(receipt => reportCompatibility(receipt.method, receipt.reason))
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
    releaseCompatibility?.()
    releaseReverseHandler?.()
    releaseState?.()
    client?.close()
    if (ownsSupervisor) supervisor.close()
  }
}
