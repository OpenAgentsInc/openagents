import { createHash } from "node:crypto"
import { CLAUDE_AGENT_SDK_PACKAGE, probeClaudeAgentReadiness, type ClaudeAgentConfig } from "./claude-agent.js"
import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import type { BootstrapSummary } from "./bootstrap.js"
import {
  providerRateLimitSnapshotsFromEvent,
  recordPylonAccountUsageObservation,
} from "./account-usage.js"

export type ClaudeComposerExecutionMode = "local_bounded" | "local_supervised_danger"
export type ClaudeComposerPermissionMode = "acceptEdits" | "bypassPermissions"
export const CLAUDE_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF =
  "blocker.claude.local_supervised_danger_public_path"
export const CLAUDE_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF =
  "blocker.claude.local_supervised_danger_requires_opt_in"

export interface ClaudeComposerCallbacks {
  onText?: (fullText: string) => void
  onEvent?: (summary: string, eventCount: number) => void
  onUsage?: (usage: { inputTokens: number; outputTokens: number; totalTokens: number; totalCostUsd: number }) => void
}

export interface ClaudeComposerOptions {
  cwd: string
  account?: ResolvedPylonAccountSelection | null
  accountHome?: string
  accountRef?: string
  model?: string
  maxTurns?: number
  timeoutMs?: number
  abortSignal?: AbortSignal
  usageStateSummary?: Pick<BootstrapSummary, "paths">
  executionMode?: ClaudeComposerExecutionMode
  permissionMode?: ClaudeComposerPermissionMode
  config?: ClaudeAgentConfig
  importer?: (specifier: string) => Promise<unknown>
  env?: Record<string, string | undefined>
  platform?: string
  localClaudeSessionProbe?: () => Promise<boolean>
  resumeSessionId?: string | null
}

/**
 * The Claude equivalent of the Codex composer danger mapping (#4845). The
 * Claude Agent SDK's unrestricted control is the permission system, not an
 * OS sandbox: local_supervised_danger maps to permissionMode
 * "bypassPermissions" with no tool allowlist, the same owner-watching
 * semantics as Codex danger-full-access + approvalPolicy "never".
 */
export function permissionModeForClaudeComposerExecutionMode(
  mode: ClaudeComposerExecutionMode,
): ClaudeComposerPermissionMode {
  return mode === "local_supervised_danger" ? "bypassPermissions" : "acceptEdits"
}

export function rejectClaudeLocalDangerForPublicPath(
  args: ReadonlyArray<string>,
  routeName: string,
): void {
  if (!args.includes("--claude-danger")) return
  throw new Error(
    `${routeName} rejects local_supervised_danger (${CLAUDE_LOCAL_DANGER_PUBLIC_PATH_BLOCKER_REF})`,
  )
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

export function claudeComposerLabel(
  model: string | null | undefined,
  executionMode: ClaudeComposerExecutionMode = "local_bounded",
): string {
  const base = executionMode === "local_supervised_danger" ? "Claude DANGER" : "Claude"
  return model ? `${base} (${model})` : base
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
  const account =
    options.account ??
    (options.accountHome
      ? {
          provider: "claude_agent" as const,
          selector: "direct_home" as const,
          accountRef: options.accountRef ?? null,
          accountRefHash: hashPylonAccountRef("claude_agent", options.accountRef ?? options.accountHome),
          home: options.accountHome,
        }
      : null)
  const env = pylonAccountEnvironment(
    options.env ?? (Bun.env as Record<string, string | undefined>),
    account,
  )
  const executionMode = options.executionMode ?? "local_bounded"
  const permissionMode = options.permissionMode ?? permissionModeForClaudeComposerExecutionMode(executionMode)
  if (permissionMode === "bypassPermissions" && executionMode !== "local_supervised_danger") {
    throw new Error(
      `bypassPermissions requires local_supervised_danger (${CLAUDE_LOCAL_DANGER_REQUIRES_OPT_IN_BLOCKER_REF})`,
    )
  }
  const readiness = await probeClaudeAgentReadiness({
    config,
    env,
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
  const abortFromCaller = () => abort.abort()
  if (options.abortSignal?.aborted) abort.abort()
  else options.abortSignal?.addEventListener("abort", abortFromCaller, { once: true })
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
    // Deliberate settingSources decision (#4845): the bounded lane keeps the
    // executor-style isolation it shipped with; the supervised danger lane
    // loads the repo's own project instruction layers (CLAUDE.md / .claude
    // settings) because the owner is operating their own checkout and wants
    // their instruction stack active.
    const session = sdk.query({
      prompt,
      options: {
        cwd: options.cwd,
        env,
        maxTurns: options.maxTurns ?? config.maxTurns ?? DEFAULT_MAX_TURNS,
        abortController: abort,
        permissionMode,
        ...(executionMode === "local_supervised_danger"
          ? { settingSources: ["project"] }
          : { allowedTools: DEFAULT_ALLOWED_TOOLS, settingSources: [] }),
        ...((options.model ?? config.model) ? { model: options.model ?? config.model } : {}),
        ...(options.resumeSessionId ? { resume: options.resumeSessionId } : {}),
      },
    })
    for await (const raw of session) {
      eventCount += 1
      const message = raw as ClaudeSdkMessage
      const providerSnapshots = providerRateLimitSnapshotsFromEvent("claude_agent", raw)
      if (options.usageStateSummary && providerSnapshots.length > 0) {
        await recordPylonAccountUsageObservation(options.usageStateSummary, {
          provider: "claude_agent",
          account,
          providerSnapshots,
        })
      }
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
        if (options.usageStateSummary) {
          await recordPylonAccountUsageObservation(options.usageStateSummary, {
            provider: "claude_agent",
            account,
            localSessionUsage: {
              provider: "claude_agent",
              sessionRef: sessionId === null ? null : stableRef("session.pylon.claude_composer", sessionId),
              inputTokens,
              outputTokens,
              totalTokens: inputTokens + outputTokens,
              totalCostUsd,
            },
          })
        }
        if (message.is_error === true) {
          const errors = Array.isArray(message.errors) ? message.errors.filter((entry) => typeof entry === "string") : []
          throw new Error(`Claude composer ${message.subtype ?? "error"}${errors.length > 0 ? `: ${errors[0]}` : ""}`)
        }
      }
    }
  } catch (error) {
    if (abort.signal.aborted) {
      throw new Error(options.abortSignal?.aborted ? "Claude composer cancelled" : "Claude composer timed out")
    }
    throw error
  } finally {
    clearTimeout(timer)
    options.abortSignal?.removeEventListener("abort", abortFromCaller)
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
