import { readFile } from "node:fs/promises"
import { join } from "node:path"
import { homedir } from "node:os"

import { createClaudeAppSdkChatRuntime, type CreateClaudeAppSdkChatRuntimeOptions } from "./claude-app-sdk-chat-runtime.js"
import { createClaudeApprovalService, type ClaudeApprovalService } from "./claude-approvals.js"
import { createClaudeSessionStore } from "./claude-session-store.js"
import { inspectClaudeHarnessStatus, type KhalaCodeDesktopClaudeHarnessStatus } from "./claude-harness-status.js"
import { createKhalaCodeDesktopClaudeTokenUsageReporter } from "./claude-token-usage-telemetry.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"
import type { KhalaCodeDesktopChatTurnEvent } from "../shared/rpc.js"

export const CLAUDE_LIVE_SMOKE_HARNESS = "claude_runtime_live_smoke"
export const CLAUDE_LIVE_SMOKE_DEFAULT_TIMEOUT_MS = 240_000
export const CLAUDE_LIVE_SMOKE_DEFAULT_PROMPT = [
  "Khala Code Desktop Claude live smoke.",
  "Use the Bash tool exactly once to run `pwd`, then reply with one short sentence.",
  "Do not edit files, create files, delete files, commit, push, or open pull requests.",
].join("\n")

type ClaudeQueryFn = NonNullable<CreateClaudeAppSdkChatRuntimeOptions["query"]>

export type KhalaCodeClaudeLiveSmokeOptions = {
  readonly approvalService?: ClaudeApprovalService | undefined
  readonly env?: Readonly<Record<string, string | undefined>> | undefined
  readonly localLedgerPath?: string | undefined
  readonly now?: (() => Date) | undefined
  readonly onEvent?: ((event: KhalaCodeDesktopChatTurnEvent) => void) | undefined
  readonly prompt?: string | undefined
  readonly query?: ClaudeQueryFn | undefined
  readonly readiness?: KhalaCodeDesktopClaudeHarnessStatus | undefined
  readonly sleep?: ((ms: number) => Promise<void>) | undefined
  readonly timeoutMs?: number | undefined
  readonly workingDirectory?: string | undefined
}

export type KhalaCodeClaudeLiveSmokeSummary = {
  readonly approvalRequestCount: number
  readonly approvedToolNames: readonly string[]
  readonly eventCount: number
  readonly exactTokenRows: number
  readonly failures: readonly string[]
  readonly finishedAt: string
  readonly harness: typeof CLAUDE_LIVE_SMOKE_HARNESS
  readonly ledgerPath: string
  readonly ok: boolean
  readonly readiness: KhalaCodeDesktopClaudeHarnessStatus
  readonly responseRuntimeMode: string | null
  readonly runtimeBadgeMode: "claude_runtime" | null
  readonly startedAt: string
  readonly threadId: string | null
  readonly toolNames: readonly string[]
  readonly totalTokens: number
  readonly turnId: string
  readonly usageTruth: "exact" | "missing"
}

export async function runKhalaCodeClaudeLiveSmoke(
  options: KhalaCodeClaudeLiveSmokeOptions = {},
): Promise<KhalaCodeClaudeLiveSmokeSummary> {
  const env = {
    ...(options.env ?? khalaCodeConfigFromRuntimeEnv().env),
    KHALA_CODE_DESKTOP_RUNTIME: "claude_runtime",
  }
  const sleep = options.sleep ?? ((ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms)))
  const timeoutMs = options.timeoutMs ?? CLAUDE_LIVE_SMOKE_DEFAULT_TIMEOUT_MS
  const now = options.now ?? (() => new Date())
  const startedAt = now().toISOString()
  const readiness = options.readiness ?? await inspectClaudeHarnessStatus({ env, now })
  const approvalService = options.approvalService ?? createClaudeApprovalService()
  const localLedgerPath = options.localLedgerPath ?? defaultClaudeTokenLedgerPath()
  const runId = now().getTime().toString(36)
  const turnId = `claude-live-smoke-${runId}`
  const sessionId = `khala-code-claude-live-smoke-${runId}`
  const events: KhalaCodeDesktopChatTurnEvent[] = []
  const approvedToolNames: string[] = []
  const beforeRows = await exactClaudeTokenRows(localLedgerPath, turnId)
  let turnSettled = false

  const runtime = createClaudeAppSdkChatRuntime({
    approvalService,
    env,
    onEvent: event => {
      events.push(event)
      options.onEvent?.(event)
    },
    ...(options.query === undefined ? {} : { query: options.query }),
    sessionStore: createClaudeSessionStore({ env, path: sessionStorePath(localLedgerPath, turnId) }),
    tokenUsageReporter: createKhalaCodeDesktopClaudeTokenUsageReporter({
      env,
      localLedgerPath,
    }),
    workingDirectory: options.workingDirectory ?? process.cwd(),
  })

  const approvalPump = (async (): Promise<void> => {
    const seen = new Set<string>()
    while (!turnSettled) {
      for (const request of approvalService.pending()) {
        if (seen.has(request.id)) continue
        seen.add(request.id)
        approvedToolNames.push(request.toolName)
        await approvalService.respond(request.id, {
          behavior: "allow",
          decisionClassification: "live_smoke_allow_once",
          updatedPermissions: request.options.suggestions ?? [],
        })
      }
      await sleep(50)
    }
  })()

  let responseRuntimeMode: string | null = null
  let runtimeBadgeMode: "claude_runtime" | null = null
  let threadId: string | null = null
  let toolNames: readonly string[] = []
  let totalTokens = 0
  const failures: string[] = readiness.available
    ? []
    : [`Claude harness readiness is ${readiness.status}: ${readiness.reason}`]

  try {
    if (!readiness.available) throw new Error(`Claude harness is not ready (${readiness.status})`)
    const response = await withTimeout(timeoutMs, runtime.startTurn({
      messages: [{ body: options.prompt ?? CLAUDE_LIVE_SMOKE_DEFAULT_PROMPT, id: `${turnId}-user`, role: "user" }],
      sessionId,
      startNewThread: true,
      turnId,
    }))
    responseRuntimeMode = response.backend.runtimeMode ?? null
    runtimeBadgeMode = response.backend.runtimeMode === "claude_runtime" ? "claude_runtime" : null
    threadId = response.backend.threadId ?? null
    toolNames = response.toolNames
    totalTokens = usageTotal(response.usage)
    if (!response.ok) failures.push("Claude runtime returned a failed turn response")
  } catch (error) {
    failures.push(error instanceof Error ? error.message : String(error))
  } finally {
    turnSettled = true
    await approvalPump
  }

  const afterRows = await exactClaudeTokenRows(localLedgerPath, turnId)
  const exactTokenRows = Math.max(0, afterRows.length - beforeRows.length)
  if (responseRuntimeMode !== "claude_runtime") {
    failures.push(`expected response runtimeMode claude_runtime, got ${responseRuntimeMode ?? "missing"}`)
  }
  if (runtimeBadgeMode !== "claude_runtime") {
    failures.push("expected runtime badge mode claude_runtime")
  }
  if (approvedToolNames.length < 1) {
    failures.push("expected canUseTool approval to be exercised at least once")
  }
  if (exactTokenRows < 1) {
    failures.push("expected at least one exact Claude token usage row")
  }
  if (totalTokens <= 0) {
    failures.push("expected positive exact token usage on the Claude turn")
  }

  return {
    approvalRequestCount: approvedToolNames.length,
    approvedToolNames,
    eventCount: events.length,
    exactTokenRows,
    failures,
    finishedAt: now().toISOString(),
    harness: CLAUDE_LIVE_SMOKE_HARNESS,
    ledgerPath: localLedgerPath,
    ok: failures.length === 0,
    readiness,
    responseRuntimeMode,
    runtimeBadgeMode,
    startedAt,
    threadId,
    toolNames,
    totalTokens,
    turnId,
    usageTruth: exactTokenRows > 0 ? "exact" : "missing",
  }
}

async function withTimeout<T>(
  timeoutMs: number,
  promise: Promise<T>,
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined
  const timeout = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(new Error(`Claude live smoke timed out after ${timeoutMs}ms`))
    }, timeoutMs)
  })
  try {
    return await Promise.race([promise, timeout])
  } finally {
    if (timeoutId !== undefined) clearTimeout(timeoutId)
  }
}

function defaultClaudeTokenLedgerPath(): string {
  return join(homedir(), ".khala-code", "claude-token-usage-events.jsonl")
}

function sessionStorePath(localLedgerPath: string, turnId: string): string {
  return join(localLedgerPath, "..", `claude-live-smoke-session-${turnId}.json`)
}

function usageTotal(usage: { cachedInput: number; input: number; output: number; reasoningOutput: number } | undefined): number {
  if (usage === undefined) return 0
  return usage.cachedInput + usage.input + usage.output + usage.reasoningOutput
}

async function exactClaudeTokenRows(
  path: string,
  turnId: string,
): Promise<readonly Record<string, unknown>[]> {
  try {
    const text = await readFile(path, "utf8")
    return text.split(/\r?\n/u).flatMap(line => {
      if (line.trim().length === 0) return []
      try {
        const row = JSON.parse(line) as Record<string, unknown>
        const event = row.event
        if (typeof event !== "object" || event === null || Array.isArray(event)) return []
        const record = event as Record<string, unknown>
        const safeMetadata = record.safeMetadata
        if (record.usageTruth !== "exact") return []
        if (typeof safeMetadata !== "object" || safeMetadata === null || Array.isArray(safeMetadata)) return []
        return (safeMetadata as Record<string, unknown>).desktopTurnId === turnId ? [record] : []
      } catch {
        return []
      }
    })
  } catch {
    return []
  }
}
