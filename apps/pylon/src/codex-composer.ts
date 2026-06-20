import { createHash } from "node:crypto"
import {
  CODEX_AGENT_SDK_PACKAGE,
  probeCodexAgentReadiness,
  type CodexAgentConfig,
  type CodexAgentSandboxMode,
} from "./codex-agent"
import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "./account-registry"
import type { BootstrapSummary } from "./bootstrap"
import {
  providerRateLimitSnapshotsFromEvent,
  recordPylonAccountUsageObservation,
} from "./account-usage"

export type CodexComposerSandboxMode = CodexAgentSandboxMode | "danger-full-access"
export type CodexComposerExecutionMode = "local_bounded" | "local_supervised_danger"
export const CODEX_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF =
  "blocker.codex.local_supervised_danger_public_path"
export const CODEX_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF =
  "blocker.codex.local_supervised_danger_requires_opt_in"

export interface CodexComposerCallbacks {
  onText?: (fullText: string) => void
  onEvent?: (summary: string, eventCount: number) => void
  onThreadId?: (threadId: string, externalSessionRef: string) => void
  onUsage?: (usage: { inputTokens: number; outputTokens: number; totalTokens: number }) => void
}

export interface CodexComposerOptions {
  cwd: string
  account?: ResolvedPylonAccountSelection | null
  accountHome?: string
  accountRef?: string
  model?: string
  sandboxMode?: CodexComposerSandboxMode
  executionMode?: CodexComposerExecutionMode
  approvalPolicy?: "never" | "on-request" | "on-failure" | "untrusted"
  networkAccessEnabled?: boolean
  timeoutMs?: number
  abortSignal?: AbortSignal
  usageStateSummary?: Pick<BootstrapSummary, "paths">
  config?: CodexAgentConfig
  importer?: (specifier: string) => Promise<unknown>
  env?: Record<string, string | undefined>
  platform?: string
  codexCliLoginPresent?: boolean
}

export function sandboxModeForCodexComposerExecutionMode(
  mode: CodexComposerExecutionMode,
  boundedMode: CodexAgentSandboxMode | undefined,
): CodexComposerSandboxMode {
  return mode === "local_supervised_danger" ? "danger-full-access" : boundedMode ?? "workspace-write"
}

export function rejectCodexLocalDangerForPublicPath(
  args: ReadonlyArray<string>,
  routeName: string,
): void {
  if (!args.includes("--codex-danger")) return
  throw new Error(`${routeName} rejects local_supervised_danger (${CODEX_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF})`)
}

export interface CodexComposerResult {
  text: string
  eventCount: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  commandCount: number
  editedFileCount: number
  threadId: string | null
}

type CodexSdkModule = {
  Codex: new (options?: { env?: Record<string, string | undefined> }) => {
    startThread: (options: Record<string, unknown>) => {
      runStreamed: (
        prompt: string,
        turnOptions?: Record<string, unknown>,
      ) => Promise<{ events: AsyncIterable<unknown> }>
    }
  }
}

type CodexThreadEvent = {
  type?: string
  thread_id?: unknown
  payload?: {
    type?: unknown
    role?: unknown
    message?: unknown
    info?: unknown
    summary?: unknown
    content?: unknown
    last_agent_message?: unknown
  }
  usage?: {
    input_tokens?: unknown
    output_tokens?: unknown
    reasoning_output_tokens?: unknown
  }
  error?: { message?: unknown }
  message?: unknown
  item?: {
    type?: string
    text?: unknown
    command?: unknown
    aggregated_output?: unknown
    exit_code?: unknown
    status?: unknown
    changes?: unknown
    server?: unknown
    tool?: unknown
    query?: unknown
    message?: unknown
    items?: unknown
  }
}

function numberOrZero(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function summarizeChanges(changes: unknown): string {
  if (!Array.isArray(changes) || changes.length === 0) return "no file paths reported"
  return changes
    .slice(0, 3)
    .map((change) => {
      if (change === null || typeof change !== "object") return "unknown"
      const record = change as Record<string, unknown>
      const kind = typeof record.kind === "string" ? record.kind : "change"
      const path = typeof record.path === "string" ? record.path : "unknown"
      return `${kind} ${path}`
    })
    .join(", ")
}

function completedTodos(items: unknown): string {
  if (!Array.isArray(items)) return "todo list updated"
  const total = items.length
  const done = items.filter((item) => item !== null && typeof item === "object" && (item as { completed?: unknown }).completed === true).length
  return `todo list ${done}/${total}`
}

function textFromContent(content: unknown, blockType: string): string {
  if (typeof content === "string") return content
  if (!Array.isArray(content)) return ""
  const block = content.find((entry: any) => entry?.type === blockType && typeof entry.text === "string")
  return block?.text ?? ""
}

function reasoningSummaryText(summary: unknown): string {
  if (typeof summary === "string") return summary.replace(/\s+/g, " ").trim()
  if (!Array.isArray(summary)) return ""
  return summary
    .map((entry) => {
      if (typeof entry === "string") return entry
      if (entry !== null && typeof entry === "object" && typeof (entry as { text?: unknown }).text === "string") {
        return (entry as { text: string }).text
      }
      return ""
    })
    .join(" ")
    .replace(/\s+/g, " ")
    .trim()
}

function tokenUsageMessage(info: unknown): string | null {
  if (info === null || typeof info !== "object") return null
  const record = info as Record<string, unknown>
  const usage =
    record.last_token_usage !== null && typeof record.last_token_usage === "object"
      ? record.last_token_usage as Record<string, unknown>
      : record.total_token_usage !== null && typeof record.total_token_usage === "object"
        ? record.total_token_usage as Record<string, unknown>
        : record
  const output = numberOrZero(usage.output_tokens)
  const reasoning = numberOrZero(usage.reasoning_output_tokens)
  return output === 0 && reasoning === 0
    ? null
    : `thinking tokens: ${reasoning}; output tokens: ${output}`
}

export function summarizeCodexThreadEvent(raw: unknown): string {
  const event = raw as CodexThreadEvent
  const type = typeof event.type === "string" ? event.type : "event"
  if (type === "thread.started") return "thread started"
  if (type === "turn.started") return "turn started"
  if (type === "turn.completed") return "turn completed"
  if (type === "turn.failed") {
    const message = typeof event.error?.message === "string" ? event.error.message : "turn failed"
    return `turn failed: ${message}`
  }
  if (type === "error") {
    const message = typeof event.message === "string" ? event.message : "stream error"
    return `error: ${message}`
  }

  const payload = event.payload && typeof event.payload === "object" ? event.payload : null
  const payloadType = typeof payload?.type === "string" ? payload.type : ""
  if (type === "event_msg") {
    if (payloadType === "task_started") return "task started"
    if (payloadType === "task_complete") {
      const text = typeof payload?.last_agent_message === "string"
        ? payload.last_agent_message.replace(/\s+/g, " ").trim()
        : ""
      return text.length > 0 ? `agent: ${text.slice(0, 200)}` : "task complete"
    }
    if (payloadType === "agent_message") {
      const text = typeof payload?.message === "string"
        ? payload.message.replace(/\s+/g, " ").trim()
        : ""
      return text.length > 0 ? `agent: ${text.slice(0, 200)}` : "agent message"
    }
    if (payloadType === "token_count") return tokenUsageMessage(payload?.info) ?? "token count"
  }
  if (type === "response_item" && payload !== null) {
    if (payloadType === "reasoning") {
      const text = reasoningSummaryText(payload.summary)
      return text.length > 0 ? `thinking: ${text.slice(0, 160)}` : "thinking…"
    }
    if (payloadType === "message" && payload.role === "assistant") {
      const text = textFromContent(payload.content, "output_text").replace(/\s+/g, " ").trim()
      return text.length > 0 ? `agent: ${text.slice(0, 200)}` : "agent message"
    }
  }

  const item = event.item
  if (!item || typeof item.type !== "string") return type
  // Surface the actual text so a remote viewer (the phone timeline) sees WHAT
  // the agent is doing, not just an opaque "agent message" label. Bounded +
  // whitespace-collapsed; the proof-serialization scanner still redacts secrets.
  const itemText = (() => {
    const t = (item as Record<string, unknown>).text
    if (typeof t === "string") return t.replace(/\s+/g, " ").trim()
    const contentText = textFromContent((item as Record<string, unknown>).content, "output_text")
    return contentText.replace(/\s+/g, " ").trim()
  })()
  if (item.type === "agent_message") return itemText ? `agent: ${itemText.slice(0, 200)}` : "agent message"
  if (item.type === "reasoning") {
    const text = itemText || reasoningSummaryText((item as Record<string, unknown>).summary)
    return text ? `thinking: ${text.slice(0, 160)}` : "thinking…"
  }
  if (item.type === "command_execution") {
    const command = typeof item.command === "string" ? item.command : "command"
    const status = typeof item.status === "string" ? item.status : "running"
    const exit = typeof item.exit_code === "number" ? ` exit ${item.exit_code}` : ""
    return `${status}: ${command}${exit}`
  }
  if (item.type === "file_change") {
    const status = typeof item.status === "string" ? item.status : "file change"
    return `${status}: ${summarizeChanges(item.changes)}`
  }
  if (item.type === "mcp_tool_call") {
    const server = typeof item.server === "string" ? item.server : "mcp"
    const tool = typeof item.tool === "string" ? item.tool : "tool"
    const status = typeof item.status === "string" ? item.status : "running"
    return `${status}: ${server}.${tool}`
  }
  if (item.type === "web_search") {
    const query = typeof item.query === "string" ? item.query : "search"
    return `web search: ${query}`
  }
  if (item.type === "todo_list") return completedTodos(item.items)
  if (item.type === "error") {
    const message = typeof item.message === "string" ? item.message : "item error"
    return `error: ${message}`
  }
  return `${type}: ${item.type}`
}

export async function runCodexComposerStream(
  prompt: string,
  options: CodexComposerOptions,
  callbacks: CodexComposerCallbacks = {},
): Promise<CodexComposerResult> {
  const config = options.config ?? {}
  const account =
    options.account ??
    (options.accountHome
      ? {
          provider: "codex" as const,
          selector: "direct_home" as const,
          accountRef: options.accountRef ?? null,
          accountRefHash: hashPylonAccountRef("codex", options.accountRef ?? options.accountHome),
          home: options.accountHome,
        }
      : null)
  const env = pylonAccountEnvironment(
    options.env ?? (Bun.env as Record<string, string | undefined>),
    account,
  )
  const executionMode = options.executionMode ?? "local_bounded"
  const sandboxMode = options.sandboxMode ?? sandboxModeForCodexComposerExecutionMode(executionMode, config.sandboxMode)
  if (sandboxMode === "danger-full-access" && executionMode !== "local_supervised_danger") {
    throw new Error(
      `danger-full-access requires local_supervised_danger (${CODEX_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF})`,
    )
  }
  const readiness = await probeCodexAgentReadiness({
    config,
    env,
    importer: options.importer,
    platform: options.platform,
    codexCliLoginPresent: options.codexCliLoginPresent,
  })
  if (readiness.state !== "ready") {
    const blockers = readiness.blockerRefs.length > 0 ? ` (${readiness.blockerRefs.join(", ")})` : ""
    throw new Error(`Codex composer unavailable: ${readiness.state}${blockers}`)
  }

  const importer = options.importer ?? ((specifier: string) => import(specifier))
  const sdk = (await importer(CODEX_AGENT_SDK_PACKAGE)) as CodexSdkModule
  const abort = new AbortController()
  const abortFromCaller = () => abort.abort()
  if (options.abortSignal?.aborted) abort.abort()
  else options.abortSignal?.addEventListener("abort", abortFromCaller, { once: true })
  const timer = options.timeoutMs === undefined
    ? null
    : setTimeout(() => abort.abort(), options.timeoutMs)
  let textResult = ""
  let eventCount = 0
  let inputTokens = 0
  let outputTokens = 0
  let commandCount = 0
  let editedFileCount = 0
  let threadId: string | null = null

  try {
    const codex = new sdk.Codex({ env })
    const thread = codex.startThread({
      workingDirectory: options.cwd,
      sandboxMode,
      approvalPolicy: options.approvalPolicy ?? "never",
      skipGitRepoCheck: true,
      networkAccessEnabled: options.networkAccessEnabled ?? false,
      ...(options.model ?? config.model ? { model: options.model ?? config.model } : {}),
    })
    const { events } = await thread.runStreamed(prompt, { signal: abort.signal })
    for await (const raw of events) {
      eventCount += 1
      const event = raw as CodexThreadEvent
      const providerSnapshots = providerRateLimitSnapshotsFromEvent("codex", raw)
      if (options.usageStateSummary && providerSnapshots.length > 0) {
        await recordPylonAccountUsageObservation(options.usageStateSummary, {
          provider: "codex",
          account,
          providerSnapshots,
        })
      }
      callbacks.onEvent?.(summarizeCodexThreadEvent(raw), eventCount)
      if (event.type === "thread.started" && typeof event.thread_id === "string") {
        threadId = event.thread_id
        callbacks.onThreadId?.(threadId, stableRef("session.pylon.codex_composer", threadId))
      }
      if (event.type === "turn.completed") {
        inputTokens = numberOrZero(event.usage?.input_tokens)
        outputTokens = numberOrZero(event.usage?.output_tokens)
        callbacks.onUsage?.({ inputTokens, outputTokens, totalTokens: inputTokens + outputTokens })
        if (options.usageStateSummary) {
          await recordPylonAccountUsageObservation(options.usageStateSummary, {
            provider: "codex",
            account,
            localSessionUsage: {
              provider: "codex",
              sessionRef: threadId === null ? null : stableRef("session.pylon.codex_composer", threadId),
              inputTokens,
              outputTokens,
              totalTokens: inputTokens + outputTokens,
            },
          })
        }
      }
      if (event.type === "item.completed" && event.item?.type === "agent_message" && typeof event.item.text === "string") {
        textResult = event.item.text
        callbacks.onText?.(textResult)
      }
      if (event.type === "item.completed" && event.item?.type === "command_execution") {
        commandCount += 1
      }
      if (event.type === "item.completed" && event.item?.type === "file_change") {
        editedFileCount += Array.isArray(event.item.changes) ? event.item.changes.length : 0
      }
      if (event.type === "turn.failed") {
        const message = typeof event.error?.message === "string" ? event.error.message : "turn failed"
        throw new Error(`Codex turn failed: ${message}`)
      }
      if (event.type === "error") {
        const message = typeof event.message === "string" ? event.message : "stream error"
        throw new Error(`Codex stream error: ${message}`)
      }
    }
  } catch (error) {
    if (abort.signal.aborted) {
      throw new Error(options.abortSignal?.aborted ? "Codex composer cancelled" : "Codex composer timed out")
    }
    throw error
  } finally {
    if (timer !== null) clearTimeout(timer)
    options.abortSignal?.removeEventListener("abort", abortFromCaller)
  }

  return {
    text: textResult.trim(),
    eventCount,
    inputTokens,
    outputTokens,
    totalTokens: inputTokens + outputTokens,
    commandCount,
    editedFileCount,
    threadId,
  }
}
