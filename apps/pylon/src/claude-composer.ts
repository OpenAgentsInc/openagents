import { createHash } from "node:crypto"
import { CLAUDE_AGENT_SDK_PACKAGE, probeClaudeAgentReadiness, type ClaudeAgentConfig } from "./claude-agent"

export interface ClaudeComposerCallbacks {
  onText?: (fullText: string) => void
  onEvent?: (summary: string, eventCount: number) => void
  onUsage?: (usage: { inputTokens: number; outputTokens: number; totalTokens: number; totalCostUsd: number }) => void
}

export interface ClaudeComposerOptions {
  cwd: string
  model?: string
  maxTurns?: number
  timeoutMs?: number
  config?: ClaudeAgentConfig
  importer?: (specifier: string) => Promise<unknown>
  env?: Record<string, string | undefined>
  platform?: string
  localClaudeSessionProbe?: () => Promise<boolean>
  resumeSessionId?: string | null
}

export interface ClaudeComposerResult {
  text: string
  eventCount: number
  turnCount: number
  commandCount: number
  editedFileCount: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  totalCostUsd: number
  sessionId: string | null
  sessionRef: string | null
}

type ClaudeSdkModule = {
  query: (options: { prompt: string; options?: Record<string, unknown> }) => AsyncIterable<unknown>
}

type ClaudeSdkMessage = {
  type?: string
  subtype?: string
  session_id?: unknown
  uuid?: unknown
  message?: {
    content?: unknown
  }
  result?: unknown
  is_error?: unknown
  errors?: unknown
  usage?: unknown
  total_cost_usd?: unknown
  num_turns?: unknown
  status?: unknown
  permissionMode?: unknown
}

const DEFAULT_ALLOWED_TOOLS = ["Read", "Edit", "Write", "Bash", "Glob", "Grep"]
const DEFAULT_MAX_TURNS = 16
const DEFAULT_TIMEOUT_MS = 300_000

export function claudeComposerLabel(model: string | null | undefined): string {
  return model ? `Claude (${model})` : "Claude"
}

function numberOrZero(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function contentBlocks(content: unknown): Record<string, unknown>[] {
  return Array.isArray(content) ? content.filter((block): block is Record<string, unknown> => block !== null && typeof block === "object") : []
}

function assistantText(message: ClaudeSdkMessage): string {
  const content = message.message?.content
  if (typeof content === "string") return content
  const text = contentBlocks(content)
    .filter((block) => block.type === "text" && typeof block.text === "string")
    .map((block) => block.text as string)
    .join("\n")
  return text.trim()
}

function toolUseNames(message: ClaudeSdkMessage): string[] {
  return contentBlocks(message.message?.content)
    .filter((block) => block.type === "tool_use" && typeof block.name === "string")
    .map((block) => block.name as string)
}

function usageTokens(usage: unknown) {
  const record = usage !== null && typeof usage === "object" ? (usage as Record<string, unknown>) : {}
  const inputTokens = numberOrZero(record.input_tokens)
  const outputTokens = numberOrZero(record.output_tokens)
  return { inputTokens, outputTokens, totalTokens: inputTokens + outputTokens }
}

export function summarizeClaudeComposerMessage(raw: unknown): string {
  const message = raw as ClaudeSdkMessage
  const type = typeof message.type === "string" ? message.type : "message"
  if (type === "system" && message.subtype === "init") return "session initialized"
  if (type === "system" && message.subtype === "status") {
    const status = typeof message.status === "string" ? message.status : "status"
    return `status: ${status}`
  }
  if (type === "assistant") {
    const tools = toolUseNames(message)
    if (tools.length > 0) return `assistant tool use: ${tools.slice(0, 3).join(", ")}`
    return assistantText(message).length > 0 ? "assistant message" : "assistant turn"
  }
  if (type === "user") return "tool result"
  if (type === "result") {
    const subtype = typeof message.subtype === "string" ? message.subtype : "result"
    const turns = typeof message.num_turns === "number" ? ` ${message.num_turns} turn(s)` : ""
    return `${subtype}${turns}`
  }
  if (type === "auth_status") return "auth status"
  if (type === "permission_denied") return "permission denied"
  if (type === "tool_progress") return "tool progress"
  return type
}

export async function runClaudeComposerStream(prompt: string, options: ClaudeComposerOptions, callbacks: ClaudeComposerCallbacks = {}): Promise<ClaudeComposerResult> {
  const config = options.config ?? {}
  const readiness = await probeClaudeAgentReadiness({
    config,
    env: options.env,
    importer: options.importer,
    localSessionProbe: options.localClaudeSessionProbe,
    platform: options.platform,
  })
  if (readiness.state !== "ready") {
    const blockers = readiness.blockerRefs.length > 0 ? ` (${readiness.blockerRefs.join(", ")})` : ""
    throw new Error(`Claude composer unavailable: ${readiness.state}${blockers}`)
  }

  const importer = options.importer ?? ((specifier: string) => import(specifier))
  const sdk = (await importer(CLAUDE_AGENT_SDK_PACKAGE)) as ClaudeSdkModule
  const abort = new AbortController()
  const timer = setTimeout(() => abort.abort(), options.timeoutMs ?? DEFAULT_TIMEOUT_MS)
  let textResult = ""
  let eventCount = 0
  let turnCount = 0
  let commandCount = 0
  let editedFileCount = 0
  let inputTokens = 0
  let outputTokens = 0
  let totalCostUsd = 0
  let sessionId = options.resumeSessionId ?? null

  try {
    const session = sdk.query({
      prompt,
      options: {
        cwd: options.cwd,
        allowedTools: DEFAULT_ALLOWED_TOOLS,
        maxTurns: options.maxTurns ?? config.maxTurns ?? DEFAULT_MAX_TURNS,
        settingSources: [],
        abortController: abort,
        permissionMode: "acceptEdits",
        ...((options.model ?? config.model) ? { model: options.model ?? config.model } : {}),
        ...(options.resumeSessionId ? { resume: options.resumeSessionId } : {}),
      },
    })
    for await (const raw of session) {
      eventCount += 1
      const message = raw as ClaudeSdkMessage
      callbacks.onEvent?.(summarizeClaudeComposerMessage(raw), eventCount)

      if (typeof message.session_id === "string") {
        sessionId = message.session_id
      }
      if (message.type === "assistant") {
        turnCount += 1
        const text = assistantText(message)
        if (text.length > 0) {
          textResult = textResult.length > 0 ? `${textResult}\n\n${text}` : text
          callbacks.onText?.(textResult)
        }
        for (const tool of toolUseNames(message)) {
          if (tool === "Bash") commandCount += 1
          if (tool === "Edit" || tool === "Write" || tool === "MultiEdit") editedFileCount += 1
        }
      }
      if (message.type === "result") {
        const usage = usageTokens(message.usage)
        inputTokens = usage.inputTokens
        outputTokens = usage.outputTokens
        totalCostUsd = numberOrZero(message.total_cost_usd)
        callbacks.onUsage?.({ ...usage, totalCostUsd })
        if (message.is_error === true) {
          const errors = Array.isArray(message.errors) ? message.errors.filter((entry) => typeof entry === "string") : []
          throw new Error(`Claude composer ${message.subtype ?? "error"}${errors.length > 0 ? `: ${errors[0]}` : ""}`)
        }
      }
    }
  } catch (error) {
    if (abort.signal.aborted) {
      throw new Error("Claude composer timed out")
    }
    throw error
  } finally {
    clearTimeout(timer)
  }

  return {
    text: textResult,
    eventCount,
    turnCount,
    commandCount,
    editedFileCount,
    inputTokens,
    outputTokens,
    totalTokens: inputTokens + outputTokens,
    totalCostUsd,
    sessionId,
    sessionRef: sessionId === null ? null : stableRef("session.pylon.claude_composer", sessionId),
  }
}
