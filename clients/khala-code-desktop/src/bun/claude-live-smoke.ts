import { randomUUID } from "node:crypto"
import { readFile, mkdtemp, rm } from "node:fs/promises"
import { homedir, tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { Effect } from "effect"

import type { KhalaCodeDesktopClaudeHarnessStatus } from "./claude-harness-status.js"
import { inspectClaudeHarnessStatus } from "./claude-harness-status.js"
import {
  createClaudeApprovalService,
  type ClaudeApprovalDecision,
  type ClaudeApprovalRequest,
  type ClaudeApprovalService,
} from "./claude-approvals.js"
import {
  createClaudeAppSdkChatRuntime,
  type CreateClaudeAppSdkChatRuntimeOptions,
} from "./claude-app-sdk-chat-runtime.js"
import {
  createKhalaCodeDesktopClaudeTokenUsageReporter,
  khalaCodeDesktopClaudeTokenUsageEvent,
  type KhalaCodeDesktopClaudeTokenUsageReport,
} from "./claude-token-usage-telemetry.js"
import { createClaudeSessionStore } from "./claude-session-store.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"

export const KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS = "claude_agent_sdk_live_smoke"

export type KhalaCodeClaudeLiveSmokeResult = Readonly<{
  approvalCount?: number
  approvalToolNames?: readonly string[]
  eventCount?: number
  harness: typeof KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS
  ok: boolean
  reason?: string
  required: boolean
  runtimeMode?: string
  skipped: boolean
  status: "failed" | "ok" | "skipped"
  threadId?: string
  tokenUsage?: {
    readonly eventId: string
    readonly ledgerPath: string
    readonly provider: string
    readonly totalTokens: number
    readonly usageTruth: string
  }
  tokenUsageDiagnostics?: readonly string[]
  turnId?: string
  turnStatus?: string
  usedTools?: readonly string[]
}>

export type RunKhalaCodeClaudeLiveSmokeInput = Readonly<{
  approvalDecision?: ClaudeApprovalDecision
  env?: NodeJS.ProcessEnv | Readonly<Record<string, string | undefined>>
  inspectHarness?: () => Promise<KhalaCodeDesktopClaudeHarnessStatus>
  prompt?: string
  query?: CreateClaudeAppSdkChatRuntimeOptions["query"]
  repoRoot?: string
  requireLive?: boolean
  tokenUsageLedgerPath?: string
  workingDirectory?: string
}>

const defaultPrompt = [
  "Khala Claude live smoke:",
  "Use one read-only local tool to inspect the current working directory name.",
  "Then answer with one short sentence that includes the word khala-claude-smoke.",
].join(" ")

const notRequested = (): KhalaCodeClaudeLiveSmokeResult => ({
  harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
  ok: true,
  reason:
    "Live Claude smoke not requested. Set KHALA_CODE_DESKTOP_CLAUDE_LIVE_SMOKE=1 or pass --require-live.",
  required: false,
  skipped: true,
  status: "skipped",
})

const resolveSourceRepositoryRoot = (): string =>
  resolve(import.meta.dir, "../../../..")

const totalTokensFromEvent = (event: Record<string, unknown>): number => {
  const tokenCounts = event.tokenCounts
  if (typeof tokenCounts !== "object" || tokenCounts === null || Array.isArray(tokenCounts)) return 0
  const totalTokens = (tokenCounts as { readonly totalTokens?: unknown }).totalTokens
  return typeof totalTokens === "number" && Number.isFinite(totalTokens) ? totalTokens : 0
}

const createTrackingApprovalService = (
  decision: ClaudeApprovalDecision,
  approvals: ClaudeApprovalRequest[],
): ClaudeApprovalService => {
  const service = createClaudeApprovalService()
  return {
    async canUseTool(toolName, input, options) {
      const pending = service.canUseTool(toolName, input, options)
      const item = await service.take()
      approvals.push(item.request)
      await service.respond(item.request.id, decision)
      return pending
    },
    pending: service.pending,
    respond: service.respond,
    take: service.take,
  }
}

const readLedgerEvents = async (path: string): Promise<readonly Record<string, unknown>[]> => {
  try {
    const text = await readFile(path, "utf8")
    return text.split(/\r?\n/u).flatMap(line => {
      if (line.trim().length === 0) return []
      try {
        const row = JSON.parse(line)
        if (typeof row !== "object" || row === null || Array.isArray(row)) return []
        const event = (row as { readonly event?: unknown }).event
        return typeof event === "object" && event !== null && !Array.isArray(event)
          ? [event as Record<string, unknown>]
          : []
      } catch {
        return []
      }
    })
  } catch {
    return []
  }
}

export async function runKhalaCodeClaudeLiveSmoke(
  input: RunKhalaCodeClaudeLiveSmokeInput = {},
): Promise<KhalaCodeClaudeLiveSmokeResult> {
  const env = input.env ?? khalaCodeConfigFromRuntimeEnv().env
  const requireLive = input.requireLive === true ||
    env.KHALA_CODE_DESKTOP_CLAUDE_LIVE_SMOKE === "1"
  if (!requireLive) return notRequested()

  const harness = await (input.inspectHarness ?? (() => inspectClaudeHarnessStatus({ env: { ...env } })))()
  if (!harness.available) {
    return {
      harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
      ok: false,
      reason: `Explicit live Claude smoke requested, but Claude is unavailable: ${harness.reason}`,
      required: true,
      skipped: false,
      status: "failed",
    }
  }

  const tempRoot = input.workingDirectory ?? await mkdtemp(join(tmpdir(), "khala-code-claude-live-"))
  const ownsTempRoot = input.workingDirectory === undefined
  const ledgerPath = input.tokenUsageLedgerPath ?? join(homedir(), ".khala-code", "claude-token-usage-events.jsonl")
  const approvals: ClaudeApprovalRequest[] = []
  const events: unknown[] = []
  const reports: KhalaCodeDesktopClaudeTokenUsageReport[] = []
  const reporter = createKhalaCodeDesktopClaudeTokenUsageReporter({
    env: {
      ...env,
      KHALA_CODE_DESKTOP_RUNTIME: "claude_runtime",
    },
    localLedgerPath: ledgerPath,
  })
  const approvalDecision = input.approvalDecision ?? {
    behavior: "allow" as const,
    decisionClassification: "khala_code_claude_live_smoke",
  }

  try {
    const runtime = createClaudeAppSdkChatRuntime({
      approvalService: createTrackingApprovalService(approvalDecision, approvals),
      env: {
        ...env,
        KHALA_CODE_DESKTOP_RUNTIME: "claude_runtime",
      },
      onEvent: event => events.push(event),
      ...(input.query === undefined ? {} : { query: input.query }),
      repoRoot: input.repoRoot ?? resolveSourceRepositoryRoot(),
      sessionStore: createClaudeSessionStore({
        path: join(tempRoot, "claude-sessions.json"),
      }),
      tokenUsageReporter: report => {
        reports.push(report)
        return reporter(report).pipe(Effect.asVoid)
      },
      workingDirectory: tempRoot,
    })
    const sessionId = randomUUID()
    const turnId = "khala-code-claude-live-smoke-turn"
    await runtime.startThread({ cwd: tempRoot, sessionId })
    const response = await runtime.startTurn({
      cwd: tempRoot,
      messages: [{
        body: input.prompt ?? defaultPrompt,
        id: "khala-code-claude-live-smoke-user",
        role: "user",
      }],
      sessionId,
      turnId,
    })
    const settings = await runtime.claudeSettingsRead()
    const expectedEvent = reports[0] === undefined ? null : khalaCodeDesktopClaudeTokenUsageEvent(reports[0])
    const ledgerEvents = await readLedgerEvents(ledgerPath)
    const ledgerEvent = expectedEvent === null
      ? undefined
      : ledgerEvents.find(event => event.eventId === expectedEvent.eventId)
    const tokenUsageDiagnostics = [
      ...(settings.errors ?? []),
      ...(expectedEvent === null ? ["missing_claude_token_usage_report"] : []),
      ...(ledgerEvent === undefined ? ["missing_claude_token_usage_ledger_event"] : []),
    ]
    const runtimeOk = response.backend.runtimeMode === "claude_runtime"
    const approvalOk = approvals.length > 0
    const tokenOk = expectedEvent !== null &&
      ledgerEvent !== undefined &&
      expectedEvent.usageTruth === "exact" &&
      totalTokensFromEvent(expectedEvent) > 0 &&
      tokenUsageDiagnostics.length === 0
    const ok = response.ok && runtimeOk && approvalOk && tokenOk

    return {
      approvalCount: approvals.length,
      approvalToolNames: approvals.map(approval => approval.toolName),
      eventCount: events.length,
      harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
      ok,
      ...(ok ? {} : {
        reason: [
          response.ok ? null : "Claude runtime turn did not complete successfully.",
          runtimeOk ? null : `Expected claude_runtime badge, got ${response.backend.runtimeMode ?? "missing"}.`,
          approvalOk ? null : "Claude canUseTool approval callback was not exercised.",
          tokenOk ? null : "Claude exact token usage report was not evidenced in the local ledger.",
        ].filter((item): item is string => item !== null).join(" "),
      }),
      required: true,
      ...(response.backend.runtimeMode === undefined ? {} : { runtimeMode: response.backend.runtimeMode }),
      skipped: false,
      status: ok ? "ok" : "failed",
      ...(response.backend.threadId === undefined ? {} : { threadId: response.backend.threadId }),
      ...(expectedEvent === null ? {} : {
        tokenUsage: {
          eventId: String(expectedEvent.eventId),
          ledgerPath,
          provider: String(expectedEvent.provider),
          totalTokens: totalTokensFromEvent(expectedEvent),
          usageTruth: String(expectedEvent.usageTruth),
        },
      }),
      tokenUsageDiagnostics,
      ...(response.backend.turnId === undefined ? {} : { turnId: response.backend.turnId }),
      ...(response.backend.turnStatus === undefined ? {} : { turnStatus: response.backend.turnStatus }),
      ...(response.usedTools === undefined ? {} : { usedTools: response.usedTools }),
    }
  } catch (error) {
    return {
      approvalCount: approvals.length,
      approvalToolNames: approvals.map(approval => approval.toolName),
      eventCount: events.length,
      harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
      ok: false,
      reason: error instanceof Error ? error.message : String(error),
      required: true,
      skipped: false,
      status: "failed",
    }
  } finally {
    if (ownsTempRoot) await rm(tempRoot, { force: true, recursive: true })
  }
}
