import type { CodexChildUsage } from "./codex-child-contract.ts"
import {
  openCodexAppServerClient,
  registerProductSpecSkill,
  type CodexAppServerClient,
  type CodexAppServerMessage,
  type CodexAppServerRequest,
  type CodexAppServerSpawn,
} from "./codex-app-server-client.ts"
import type { FableLocalEvent } from "./fable-local-contract.ts"

export type CodexAppServerTurnOutcome = Readonly<{
  outcome: "success" | "reconnect_required" | "incompatible_workflow" | "failed" | "timeout" | "interrupted"
  text: string
  usage: CodexChildUsage | null
  threadId: string | null
  detail: string
  preContent: boolean
  rateLimited: boolean
}>

export type CodexAppServerTurnControl = {
  interrupted: boolean
  interrupt: (() => void) | null
  /** Inject text into this exact active provider turn. */
  steer: ((message: string) => Promise<boolean>) | null
}

type ProductSpecSkill = Readonly<{ skillRoot: string; skillPath: string }>

export type RunCodexAppServerTurnInput = Readonly<{
  binary: string
  env: NodeJS.ProcessEnv
  workspace: string
  threadRef: string
  turnRef: string
  accountRef: string
  prompt: string
  imagePaths: ReadonlyArray<string>
  resumeThreadId: string | null
  model: string
  reasoningEffort: string
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
}>

const record = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null

const string = (value: unknown): string | null => typeof value === "string" ? value : null

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
  const rateLimited = lower.includes("rate limit") || lower.includes("usage limit") || lower.includes("429")
  const incompatibleWorkflow = lower.includes("productspec-work") || lower.includes("dynamictools") || lower.includes("dynamic tools")
  return {
    outcome: reconnect ? "reconnect_required" : incompatibleWorkflow ? "incompatible_workflow" : "failed",
    text,
    usage,
    threadId,
    detail,
    preContent: text.trim() === "" && (usage === null || usage.totalTokens === 0),
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
  let settle: ((outcome: CodexAppServerTurnOutcome) => void) | null = null
  let settled = false
  let turnTimer: ReturnType<typeof setTimeout> | null = null
  const completion = new Promise<CodexAppServerTurnOutcome>(resolve => { settle = resolve })

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
      const facts = toolFacts(item)
      if (facts === null) return
      if (message.method === "item/started") {
        pendingTools.add(id)
        input.emit({ kind: "tool_use", toolName: facts.name, summary: facts.summary })
      } else {
        if (!pendingTools.has(id)) input.emit({ kind: "tool_use", toolName: facts.name, summary: facts.summary })
        pendingTools.delete(id)
        const output = item.type === "commandExecution" && typeof item.aggregatedOutput === "string"
          ? item.aggregatedOutput.slice(0, 400)
          : facts.summary
        input.emit({ kind: "tool_result", toolName: facts.name, ok: facts.ok, summary: output })
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
      if (status === "completed" && text.trim() !== "") {
        finish({ outcome: "success", text, usage, threadId, detail: "", preContent: false, rateLimited: false })
      } else if (status === "interrupted" || input.control.interrupted) {
        finish({ outcome: "interrupted", text, usage, threadId, detail: "turn interrupted", preContent: text === "", rateLimited: false })
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
    client = openCodexAppServerClient({
      binary: input.binary,
      env: input.env,
      cwd: input.workspace,
      ...(input.spawnImpl === undefined ? {} : { spawnImpl: input.spawnImpl }),
      ...(input.requestTimeoutMs === undefined ? {} : { requestTimeoutMs: input.requestTimeoutMs }),
      ...((input.onProductSpecToolCall === undefined && input.onServerRequest === undefined) ? {} : {
        onServerRequest: async (request: CodexAppServerRequest) => {
          if (request.method === "item/tool/call" && input.onProductSpecToolCall !== undefined) {
            const result = await input.onProductSpecToolCall(request)
            if (result !== null) return result
          }
          if (input.onServerRequest !== undefined) return input.onServerRequest(request)
          throw new Error(`Unsupported Codex server request: ${request.method}`)
        },
      }),
    })
    const release = client.onNotification(handleNotification)
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

    const userInput: Array<Record<string, unknown>> = [
      { type: "text", text: input.prompt, text_elements: [] },
      ...input.imagePaths.map(path => ({ type: "localImage", path })),
      ...(input.includeProductSpecSkill === false ? [] : [{ type: "skill", name: "productspec-work", path: input.productSpecSkill.skillPath }]),
    ]
    const turnResponse = record(await client.request("turn/start", {
      threadId,
      clientUserMessageId: input.turnRef,
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
    input.control.interrupt = () => {
      if (client !== null && threadId !== null && turnId !== null) {
        void client.request("turn/interrupt", { threadId, turnId }).catch(() => undefined)
      }
    }
    input.control.steer = async message => {
      if (client === null || threadId === null || turnId === null || message.trim() === "") return false
      try {
        await client.request("turn/steer", {
          threadId,
          clientUserMessageId: null,
          input: [{ type: "text", text: message, text_elements: [] }],
          expectedTurnId: turnId,
        })
        return true
      } catch {
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
          rateLimited: false,
        })
      }, input.turnTimeoutMs)
    }
    const outcome = await completion
    release()
    return outcome
  } catch (error) {
    return classifyFailure(error instanceof Error ? error.message : "Codex app-server failed", text, usage, threadId)
  } finally {
    input.control.interrupt = null
    input.control.steer = null
    client?.close()
  }
}
