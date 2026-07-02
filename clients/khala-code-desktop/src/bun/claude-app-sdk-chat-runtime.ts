import { Data, Effect, Stream } from "effect"

import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopChatTurnRequest,
  KhalaCodeDesktopChatTurnResponse,
  KhalaCodeDesktopCodexThreadCompactRequest,
  KhalaCodeDesktopCodexThreadForkRequest,
  KhalaCodeDesktopCodexThreadIdRequest,
  KhalaCodeDesktopCodexThreadListRequest,
  KhalaCodeDesktopCodexThreadListResult,
  KhalaCodeDesktopCodexThreadMutationResult,
  KhalaCodeDesktopCodexThreadReadRequest,
  KhalaCodeDesktopCodexThreadRenameRequest,
  KhalaCodeDesktopCodexThreadResult,
  KhalaCodeDesktopCodexThreadResumeRequest,
  KhalaCodeDesktopCodexThreadStartRequest,
  KhalaCodeDesktopCodexTurnActionResult,
  KhalaCodeDesktopCodexTurnInterruptRequest,
  KhalaCodeDesktopCodexTurnSteerRequest,
} from "../shared/rpc.js"
import type { CodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import {
  createClaudeSessionStore,
  type ClaudeSessionStore,
} from "./claude-session-store.js"
import { createClaudeThreadItemProjector } from "./claude-thread-item-projector.js"
import {
  createClaudeApprovalService,
  type ClaudeApprovalService,
} from "./claude-approvals.js"
import {
  createKhalaCodeDesktopClaudeTokenUsageReporter,
  type KhalaCodeDesktopClaudeTokenUsageReporter,
} from "./claude-token-usage-telemetry.js"
import { withClaudeFleetMcpBridgeOptions } from "./claude-fleet-mcp-bridge.js"
import {
  projectKhalaCodeDesktopClaudeSettings,
  type KhalaCodeDesktopClaudeSettingsProjection,
} from "../shared/claude-settings.js"

type ClaudeQuery = AsyncIterable<unknown> & {
  readonly accountInfo?: () => Promise<unknown>
  readonly close?: () => Promise<void> | void
  readonly initializationResult?: () => Promise<unknown>
  readonly interrupt?: () => Promise<void> | void
  readonly supportedModels?: () => Promise<unknown>
}

type ClaudeQueryFn = (input: {
  readonly prompt: string
  readonly options: Record<string, unknown>
}) => ClaudeQuery

type ClaudeSdkModule = {
  readonly accountInfo?: () => Promise<unknown>
  readonly query: ClaudeQueryFn
  readonly supportedModels?: () => Promise<unknown>
}

class ClaudeSdkRuntimeError extends Data.TaggedError("ClaudeSdkRuntimeError")<{
  readonly cause: unknown
  readonly message: string
}> {}

const claudeSdkRuntimeError = (error: unknown): ClaudeSdkRuntimeError =>
  new ClaudeSdkRuntimeError({
    cause: error,
    message: error instanceof Error ? error.message : String(error),
  })

export type CreateClaudeAppSdkChatRuntimeOptions = {
  readonly approvalService?: ClaudeApprovalService
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly importer?: (specifier: string) => Promise<unknown>
  readonly onEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly query?: ClaudeQueryFn
  readonly repoRoot?: string
  readonly sessionStore?: ClaudeSessionStore
  readonly tokenUsageReporter?: KhalaCodeDesktopClaudeTokenUsageReporter
  readonly workingDirectory: string
}

export type ClaudeAppSdkChatRuntime = CodexAppServerChatRuntime & {
  readonly claudeSettingsRead: () => Promise<KhalaCodeDesktopClaudeSettingsProjection>
}

type ActiveClaudeTurn = {
  readonly controller: AbortController
  readonly query: ClaudeQuery
  readonly sessionId: string
}

const unsupported = async (): Promise<never> => {
  throw new Error("Claude app SDK chat runtime does not support this thread operation until a later parity phase.")
}

const textFromRequest = (request: KhalaCodeDesktopChatTurnRequest): string =>
  request.messages.map(message => message.body).filter(Boolean).join("\n\n").trim()

const sdkSessionId = (value: unknown): string | null => {
  if (typeof value !== "object" || value === null) return null
  const candidate = (value as { readonly session_id?: unknown }).session_id
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const numberField = (value: Record<string, unknown>, field: string): number | undefined => {
  const candidate = value[field]
  return typeof candidate === "number" && Number.isFinite(candidate) ? candidate : undefined
}

const resultModel = (value: unknown): string | null => {
  if (!isRecord(value)) return null
  const model = typeof value.model === "string" && value.model.length > 0 ? value.model : null
  if (model !== null) return model
  const modelUsage = isRecord(value.modelUsage) ? value.modelUsage : null
  const [firstModel] = modelUsage === null ? [] : Object.keys(modelUsage)
  return firstModel ?? null
}

const resultTotalCostUsd = (value: unknown): number | undefined => {
  if (!isRecord(value)) return undefined
  return numberField(value, "total_cost_usd") ?? numberField(value, "totalCostUsd")
}

const loadQuery = async (
  options: CreateClaudeAppSdkChatRuntimeOptions,
): Promise<ClaudeQueryFn> => {
  if (options.query !== undefined) return options.query
  const importer = options.importer ?? ((specifier: string) => import(specifier))
  const mod = await importer("@anthropic-ai/claude-agent-sdk") as Partial<ClaudeSdkModule>
  if (typeof mod.query !== "function") {
    throw new Error("Claude Agent SDK did not expose query().")
  }
  return mod.query
}

const runScoped = async <A>(effect: Effect.Effect<A, unknown, never>): Promise<A> =>
  Effect.runPromise(effect)

export function createClaudeAppSdkChatRuntime(
  options: CreateClaudeAppSdkChatRuntimeOptions,
): ClaudeAppSdkChatRuntime {
  const sessionStore = options.sessionStore ?? createClaudeSessionStore({
    ...(options.env === undefined ? {} : { env: options.env }),
  })
  const approvalService = options.approvalService ?? createClaudeApprovalService()
  const tokenUsageReporter = options.tokenUsageReporter ??
    createKhalaCodeDesktopClaudeTokenUsageReporter({
      ...(options.env === undefined ? {} : { env: options.env }),
    })
  const activeTurns = new Map<string, ActiveClaudeTurn>()
  let lastAccountInfo: unknown
  let lastInitializationResult: unknown
  let lastSupportedModels: unknown

  const startThread = async (
    request: KhalaCodeDesktopCodexThreadStartRequest = {},
  ): Promise<KhalaCodeDesktopCodexThreadResult> => {
    const desktopSessionId = request.sessionId ?? `claude-desktop-${Date.now().toString(36)}`
    const stored = await sessionStore.put(desktopSessionId, {
      sessionId: desktopSessionId,
    })
    return {
      ok: true,
      desktopSessionId,
      thread: { id: stored.sessionId, title: "Claude session" },
      threadId: stored.sessionId,
    }
  }

  const resumeThread = async (
    request: KhalaCodeDesktopCodexThreadResumeRequest,
  ): Promise<KhalaCodeDesktopCodexThreadResult> => {
    const desktopSessionId = request.sessionId ?? request.threadId
    await sessionStore.put(desktopSessionId, { sessionId: request.threadId })
    return {
      ok: true,
      desktopSessionId,
      thread: { id: request.threadId, title: "Claude session" },
      threadId: request.threadId,
    }
  }

  const threadIdForSession = async (desktopSessionId: string): Promise<string | null> =>
    (await sessionStore.get(desktopSessionId))?.sessionId ?? null

  const startTurn = async (
    request: KhalaCodeDesktopChatTurnRequest & { readonly cwd?: string },
  ): Promise<KhalaCodeDesktopChatTurnResponse> => {
    const desktopTurnId = request.turnId ?? `claude-turn-${Date.now().toString(36)}`
    const prompt = textFromRequest(request)
    if (prompt.length === 0) {
      throw new Error("Claude app SDK chat runtime requires a non-empty prompt.")
    }
    const stored = request.startNewThread === true ? null : await sessionStore.get(request.sessionId)
    const threadId = request.threadId ?? stored?.sessionId
    const freshSessionId = request.threadId ?? stored?.sessionId ?? request.sessionId
    const shouldResume = threadId !== undefined && stored?.lastTurnId !== undefined
    const controller = new AbortController()
    const query = await loadQuery(options)
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: request.sessionId,
      turnId: desktopTurnId,
    })
    let resolvedThreadId = threadId ?? null

    let resultMessage: unknown
    const baseQueryOptions = {
      abortController: controller,
      canUseTool: approvalService.canUseTool,
      cwd: request.cwd ?? options.workingDirectory,
      includePartialMessages: true,
      permissionMode: options.env?.KHALA_CODE_DESKTOP_CLAUDE_PERMISSION_MODE ?? "acceptEdits",
      ...(shouldResume ? { resume: threadId } : { sessionId: freshSessionId }),
      env: {
        ...options.env,
        ...(options.env?.CLAUDE_CONFIG_DIR === undefined ? {} : { CLAUDE_CONFIG_DIR: options.env.CLAUDE_CONFIG_DIR }),
      },
    }
    const queryOptions = withClaudeFleetMcpBridgeOptions({
      env: options.env ?? {},
      options: baseQueryOptions,
      repoRoot: options.repoRoot ?? options.workingDirectory,
    })
    const acquireQuery = Effect.tryPromise({
      try: async () => query({
        prompt,
        options: queryOptions,
      }),
      catch: claudeSdkRuntimeError,
    })

    await runScoped(
      Effect.acquireRelease(
        acquireQuery.pipe(
          Effect.tap(handle => Effect.sync(() => {
            activeTurns.set(desktopTurnId, {
              controller,
              query: handle,
              sessionId: request.sessionId,
            })
          })),
          Effect.tap(handle => Effect.promise(async () => {
            const [initializationResult, supportedModels, accountInfo] = await Promise.all([
              handle.initializationResult?.().catch(error => ({ error: error instanceof Error ? error.message : String(error) })),
              handle.supportedModels?.().catch(error => ({ error: error instanceof Error ? error.message : String(error) })),
              handle.accountInfo?.().catch(error => ({ error: error instanceof Error ? error.message : String(error) })),
            ])
            if (initializationResult !== undefined) lastInitializationResult = initializationResult
            if (supportedModels !== undefined) lastSupportedModels = supportedModels
            if (accountInfo !== undefined) lastAccountInfo = accountInfo
          })),
        ),
        handle => Effect.promise(async () => {
          try {
            await handle.close?.()
          } finally {
            controller.abort()
            activeTurns.delete(desktopTurnId)
          }
        }),
      ).pipe(
        Effect.flatMap(handle =>
          Stream.fromAsyncIterable(handle, claudeSdkRuntimeError).pipe(
            Stream.runForEach(message =>
              Effect.sync(() => {
                const sessionId = sdkSessionId(message)
                if (sessionId !== null) resolvedThreadId = sessionId
                if (isRecord(message) && message.type === "result") resultMessage = message
                if (isRecord(message) && message.type === "system" && message.subtype === "init") {
                  lastInitializationResult = message
                }
                const projected = projector.project(message)
                for (const event of projected.events) options.onEvent?.(event)
              }),
            ),
          ),
        ),
        Effect.scoped,
      ),
    )
    const finalThreadId = resolvedThreadId ?? threadId ?? request.sessionId
    await sessionStore.put(request.sessionId, {
      sessionId: finalThreadId,
      lastTurnId: desktopTurnId,
    })
    const usage = projector.usage()
    if (usage !== undefined) {
      const totalCostUsd = resultTotalCostUsd(resultMessage)
      await Effect.runPromise(tokenUsageReporter({
        claudeSessionId: finalThreadId,
        desktopSessionId: request.sessionId,
        desktopTurnId,
        model: resultModel(resultMessage) ?? "openagents/claude-direct-local",
        observedAt: new Date().toISOString(),
        sequence: 1,
        ...(totalCostUsd === undefined ? {} : { totalCostUsd }),
        turnStatus: projector.status(),
        usage,
      }).pipe(Effect.catchCause(() => Effect.void)))
    }
    const messages = projector.messages()
    return {
      backend: {
        kind: "claude_app_sdk",
        model: "claude-app-sdk",
        runtimeMode: "claude_runtime",
        threadId: finalThreadId,
        toolCatalogKind: "codex_harness_supplemental",
        turnId: desktopTurnId,
        turnStatus: projector.status(),
      },
      messages: messages.length === 0
        ? [{
          body: `Claude completed the turn with status: ${projector.status()}.`,
          id: `${desktopTurnId}-claude-status`,
          role: "system",
        }]
        : messages,
      ok: projector.status() !== "failed",
      toolNames: projector.toolNames(),
      ...(projector.usage() === undefined ? {} : { usage: projector.usage() }),
      usedTools: projector.toolNames(),
    }
  }

  const interruptTurn = async (
    request: KhalaCodeDesktopCodexTurnInterruptRequest,
  ): Promise<KhalaCodeDesktopCodexTurnActionResult> => {
    const entries = [...activeTurns.entries()]
    const match = request.turnId === undefined
      ? entries.find(([, turn]) => turn.sessionId === request.sessionId)
      : entries.find(([turnId, turn]) => turnId === request.turnId && turn.sessionId === request.sessionId)
    if (match === undefined) {
      return { ok: false, desktopSessionId: request.sessionId, desktopTurnId: request.turnId }
    }
    if (typeof match[1].query.interrupt !== "function") {
      match[1].controller.abort()
      await match[1].query.close?.()
      return {
        ok: false,
        desktopSessionId: request.sessionId,
        desktopTurnId: match[0],
        error: "Claude Agent SDK query does not expose interrupt().",
        threadId: await threadIdForSession(request.sessionId) ?? undefined,
      }
    }
    await match[1].query.interrupt()
    return {
      ok: true,
      desktopSessionId: request.sessionId,
      desktopTurnId: match[0],
      threadId: await threadIdForSession(request.sessionId) ?? undefined,
    }
  }

  const mutationUnsupported = async <A extends KhalaCodeDesktopCodexThreadMutationResult["action"]>(
    action: A,
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => ({
    action,
    ok: false,
    threadId: request.threadId,
  })
  const actionUnsupported = async (
    request: KhalaCodeDesktopCodexThreadCompactRequest | KhalaCodeDesktopCodexTurnSteerRequest,
  ): Promise<KhalaCodeDesktopCodexTurnActionResult> => ({
    ok: false,
    desktopSessionId: request.sessionId ?? "",
    ...(!("threadId" in request) || request.threadId === undefined ? {} : { threadId: request.threadId }),
  })
  const claudeSettingsRead = async (): Promise<KhalaCodeDesktopClaudeSettingsProjection> =>
    projectKhalaCodeDesktopClaudeSettings({
      accountInfo: lastAccountInfo,
      initializationResult: lastInitializationResult,
      permissionMode: options.env?.KHALA_CODE_DESKTOP_CLAUDE_PERMISSION_MODE ?? "acceptEdits",
      supportedModels: lastSupportedModels,
    })

  return {
    archiveThread: request => mutationUnsupported("archive", request),
    compactThread: actionUnsupported,
    deleteThread: request => mutationUnsupported("delete", request),
    forkThread: async (request: KhalaCodeDesktopCodexThreadForkRequest) => mutationUnsupported("fork", request),
    interruptTurn,
    listThreads: async (_request?: KhalaCodeDesktopCodexThreadListRequest): Promise<KhalaCodeDesktopCodexThreadListResult> => unsupported(),
    readThread: async (_request: KhalaCodeDesktopCodexThreadReadRequest) => unsupported(),
    renameThread: async (request: KhalaCodeDesktopCodexThreadRenameRequest) => mutationUnsupported("rename", request),
    resumeThread,
    startThread,
    startTurn,
    steerTurn: actionUnsupported,
    threadIdForSession,
    claudeSettingsRead,
    unarchiveThread: request => mutationUnsupported("unarchive", request),
  }
}
