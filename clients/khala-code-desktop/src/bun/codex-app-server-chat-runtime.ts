import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopChatTurnRequest,
  KhalaCodeDesktopChatTurnResponse,
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopUsage,
  KhalaCodeDesktopCodexThreadCompactRequest,
  KhalaCodeDesktopCodexThreadForkRequest,
  KhalaCodeDesktopCodexThreadIdRequest,
  KhalaCodeDesktopCodexThreadListRequest,
  KhalaCodeDesktopCodexThreadListResult,
  KhalaCodeDesktopCodexThreadMutationResult,
  KhalaCodeDesktopCodexThreadReadRequest,
  KhalaCodeDesktopCodexThreadRenameRequest,
  KhalaCodeDesktopCodexThreadResumeRequest,
  KhalaCodeDesktopCodexThreadResult,
  KhalaCodeDesktopCodexThreadStartRequest,
  KhalaCodeDesktopCodexTurnActionResult,
  KhalaCodeDesktopCodexTurnInterruptRequest,
  KhalaCodeDesktopCodexTurnSteerRequest,
} from "../shared/rpc.js"
import type {
  CodexAppServerHost,
  CodexAppServerNotification,
} from "./codex-app-server-client.js"
import type {
  KhalaCodeDesktopCodexMessageTokenAuditRecorder,
  KhalaCodeDesktopCodexMessageTokenAuditUsageEvent,
  KhalaCodeDesktopCodexTokenUsageCounts,
  KhalaCodeDesktopCodexTokenUsageReporter,
} from "./codex-token-usage-telemetry.js"
import {
  khalaCodeDesktopCodexMessageTokenAuditMessage,
  khalaCodeDesktopCodexTokenUsageEventRefs,
} from "./codex-token-usage-telemetry.js"
import { createCodexThreadItemEventProjector } from "./codex-thread-item-projector.js"
import { projectKhalaCodeDesktopCodexThreadList } from "../shared/codex-threads.js"

const CODEX_SESSION_STATE_SCHEMA = "khala-code-desktop.codex-sessions.v1"
const DEFAULT_TURN_TIMEOUT_MS = 30 * 60 * 1_000

type JsonObject = Readonly<Record<string, unknown>>

type StoredCodexSession = {
  readonly lastCodexTurnId?: string
  readonly threadId: string
  readonly updatedAt: string
}

type StoredCodexSessionState = {
  readonly schema: typeof CODEX_SESSION_STATE_SCHEMA
  readonly sessions: Record<string, StoredCodexSession>
}

type ActiveCodexTurn = {
  readonly codexThreadId: string
  readonly codexTurnId: string
  readonly desktopSessionId: string
  readonly desktopTurnId: string
}

export type CodexAppServerChatRuntime = Readonly<{
  compactThread: (
    request: KhalaCodeDesktopCodexThreadCompactRequest,
  ) => Promise<KhalaCodeDesktopCodexTurnActionResult>
  archiveThread: (
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  deleteThread: (
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  forkThread: (
    request: KhalaCodeDesktopCodexThreadForkRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  listThreads: (
    request?: KhalaCodeDesktopCodexThreadListRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadListResult>
  readThread: (
    request: KhalaCodeDesktopCodexThreadReadRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadResult>
  renameThread: (
    request: KhalaCodeDesktopCodexThreadRenameRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
  resumeThread: (
    request: KhalaCodeDesktopCodexThreadResumeRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadResult>
  startThread: (
    request?: KhalaCodeDesktopCodexThreadStartRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadResult>
  startTurn: (request: KhalaCodeDesktopChatTurnRequest & {
    readonly cwd?: string
  }) => Promise<KhalaCodeDesktopChatTurnResponse>
  interruptTurn: (
    request: KhalaCodeDesktopCodexTurnInterruptRequest,
  ) => Promise<KhalaCodeDesktopCodexTurnActionResult>
  steerTurn: (
    request: KhalaCodeDesktopCodexTurnSteerRequest,
  ) => Promise<KhalaCodeDesktopCodexTurnActionResult>
  threadIdForSession: (sessionId: string) => Promise<string | null>
  unarchiveThread: (
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ) => Promise<KhalaCodeDesktopCodexThreadMutationResult>
}>

export type CreateCodexAppServerChatRuntimeOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly host: CodexAppServerHost
  readonly onEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly statePath?: string
  readonly turnTimeoutMs?: number
  readonly messageTokenAuditRecorder?: KhalaCodeDesktopCodexMessageTokenAuditRecorder | null
  readonly tokenUsageReporter?: KhalaCodeDesktopCodexTokenUsageReporter | null
  readonly workingDirectory: string
}

const emptyState = (): StoredCodexSessionState => ({
  schema: CODEX_SESSION_STATE_SCHEMA,
  sessions: {},
})

const isoNow = (): string => new Date().toISOString()

const isObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringField = (value: unknown, field: string): string | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const objectField = (value: unknown, field: string): JsonObject | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return isObject(candidate) ? candidate : null
}

const arrayField = (value: unknown, field: string): readonly unknown[] | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return Array.isArray(candidate) ? candidate : null
}

const numericUsage = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0
    ? Math.trunc(value)
    : 0

const emptyCodexUsage = (): KhalaCodeDesktopCodexTokenUsageCounts => ({
  cachedInputTokens: 0,
  inputTokens: 0,
  outputTokens: 0,
  reasoningOutputTokens: 0,
  totalTokens: 0,
})

const addCodexUsage = (
  left: KhalaCodeDesktopCodexTokenUsageCounts,
  right: KhalaCodeDesktopCodexTokenUsageCounts,
): KhalaCodeDesktopCodexTokenUsageCounts => ({
  cachedInputTokens: left.cachedInputTokens + right.cachedInputTokens,
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  reasoningOutputTokens: left.reasoningOutputTokens + right.reasoningOutputTokens,
  totalTokens: left.totalTokens + right.totalTokens,
})

const subtractCodexUsage = (
  left: KhalaCodeDesktopCodexTokenUsageCounts,
  right: KhalaCodeDesktopCodexTokenUsageCounts,
): KhalaCodeDesktopCodexTokenUsageCounts => ({
  cachedInputTokens: Math.max(0, left.cachedInputTokens - right.cachedInputTokens),
  inputTokens: Math.max(0, left.inputTokens - right.inputTokens),
  outputTokens: Math.max(0, left.outputTokens - right.outputTokens),
  reasoningOutputTokens: Math.max(0, left.reasoningOutputTokens - right.reasoningOutputTokens),
  totalTokens: Math.max(0, left.totalTokens - right.totalTokens),
})

const codexUsageHasTokens = (
  usage: KhalaCodeDesktopCodexTokenUsageCounts,
): boolean =>
  usage.inputTokens > 0 ||
  usage.outputTokens > 0 ||
  usage.reasoningOutputTokens > 0 ||
  usage.totalTokens > 0

const codexUsageKey = (usage: KhalaCodeDesktopCodexTokenUsageCounts): string =>
  [
    usage.inputTokens,
    usage.cachedInputTokens,
    usage.outputTokens,
    usage.reasoningOutputTokens,
    usage.totalTokens,
  ].join(":")

const desktopUsageFromCodexUsage = (
  usage: KhalaCodeDesktopCodexTokenUsageCounts,
): KhalaCodeDesktopUsage => ({
  cachedInput: usage.cachedInputTokens,
  input: usage.inputTokens,
  output: usage.outputTokens,
  reasoningOutput: usage.reasoningOutputTokens,
})

const codexUsageFromObject = (value: unknown): KhalaCodeDesktopCodexTokenUsageCounts | null => {
  if (!isObject(value)) return null
  const promptDetails = objectField(value, "prompt_tokens_details") ?? {}
  const completionDetails = objectField(value, "completion_tokens_details") ?? {}
  const inputTokens =
    numericUsage(value.input_tokens) +
    numericUsage(value.inputTokens) +
    numericUsage(value.prompt_tokens) +
    numericUsage(value.promptTokens) +
    numericUsage(value.input)
  const outputTokens =
    numericUsage(value.output_tokens) +
    numericUsage(value.outputTokens) +
    numericUsage(value.completion_tokens) +
    numericUsage(value.completionTokens) +
    numericUsage(value.output)
  const reasoningOutputTokens =
    numericUsage(value.reasoning_output_tokens) +
    numericUsage(value.reasoningOutputTokens) +
    numericUsage(value.reasoning_output) +
    numericUsage(value.reasoningOutput) +
    numericUsage(completionDetails.reasoning_tokens) +
    numericUsage(completionDetails.reasoningTokens)
  const cachedInputTokens =
    numericUsage(value.cached_input_tokens) +
    numericUsage(value.cachedInputTokens) +
    numericUsage(value.cached_input) +
    numericUsage(value.cachedInput) +
    numericUsage(promptDetails.cached_tokens) +
    numericUsage(promptDetails.cachedTokens)
  const explicitTotal =
    numericUsage(value.total_tokens) +
    numericUsage(value.totalTokens)
  const usage = {
    cachedInputTokens,
    inputTokens,
    outputTokens,
    reasoningOutputTokens,
    totalTokens: explicitTotal > 0 ? explicitTotal : inputTokens + outputTokens,
  }
  return codexUsageHasTokens(usage) ? usage : null
}

const tokenUsageObject = (
  value: unknown,
  snakeField: string,
  camelField: string,
): KhalaCodeDesktopCodexTokenUsageCounts | null =>
  codexUsageFromObject(objectField(value, snakeField) ?? objectField(value, camelField))

const tokenUsageInfoFromNotification = (
  notification: CodexAppServerNotification,
): {
  readonly lastUsage: KhalaCodeDesktopCodexTokenUsageCounts | null
  readonly totalUsage: KhalaCodeDesktopCodexTokenUsageCounts | null
} | null => {
  if (notification.method !== "thread/tokenUsage/updated") return null
  const params = notification.params
  if (!isObject(params)) return null
  const info = objectField(params, "info") ?? params
  const lastUsage =
    tokenUsageObject(info, "last_token_usage", "lastTokenUsage") ??
    tokenUsageObject(info, "last_usage", "lastUsage") ??
    codexUsageFromObject(objectField(info, "usage") ?? objectField(params, "usage")) ??
    codexUsageFromObject(objectField(info, "tokenUsage") ?? objectField(params, "tokenUsage"))
  const totalUsage =
    tokenUsageObject(info, "total_token_usage", "totalTokenUsage") ??
    tokenUsageObject(info, "total_usage", "totalUsage")
  return lastUsage === null && totalUsage === null ? null : { lastUsage, totalUsage }
}

const isTokenUsageNotification = (notification: CodexAppServerNotification): boolean =>
  tokenUsageInfoFromNotification(notification) !== null

const messagesFromThread = (
  thread: JsonObject | null,
): readonly KhalaCodeDesktopMessage[] => {
  if (thread === null) return []
  const threadId = stringField(thread, "id")
  if (threadId === null) return []
  const messages: KhalaCodeDesktopMessage[] = []
  for (const turn of arrayField(thread, "turns") ?? []) {
    if (!isObject(turn)) continue
    const turnId = stringField(turn, "id")
    if (turnId === null) continue
    const projector = createCodexThreadItemEventProjector({
      desktopTurnId: `codex-history-${turnId}`,
      renderUserMessages: true,
    })
    for (const item of arrayField(turn, "items") ?? []) {
      if (!isObject(item)) continue
      projector.accept({
        method: "item/completed",
        params: { threadId, turnId, item },
        receivedAt: isoNow(),
      })
    }
    messages.push(...projector.messages())
  }
  return messages
}

const defaultStatePath = (env: Readonly<Record<string, string | undefined>>): string => {
  const explicit = env.KHALA_CODE_DESKTOP_CODEX_STATE_PATH?.trim()
  if (explicit !== undefined && explicit.length > 0) return explicit
  return join(homedir(), ".khala-code", "codex-sessions.json")
}

const parseState = (value: unknown): StoredCodexSessionState => {
  if (!isObject(value) || value.schema !== CODEX_SESSION_STATE_SCHEMA || !isObject(value.sessions)) {
    return emptyState()
  }
  const sessions: Record<string, StoredCodexSession> = {}
  for (const [sessionId, session] of Object.entries(value.sessions)) {
    if (!isObject(session)) continue
    const threadId = stringField(session, "threadId")
    const updatedAt = stringField(session, "updatedAt")
    if (threadId === null || updatedAt === null) continue
    const lastCodexTurnId = stringField(session, "lastCodexTurnId")
    sessions[sessionId] = {
      threadId,
      updatedAt,
      ...(lastCodexTurnId === null ? {} : { lastCodexTurnId }),
    }
  }
  return {
    schema: CODEX_SESSION_STATE_SCHEMA,
    sessions,
  }
}

const readState = async (statePath: string): Promise<StoredCodexSessionState> => {
  try {
    return parseState(JSON.parse(await readFile(statePath, "utf8")))
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return emptyState()
    throw error
  }
}

const writeState = async (
  statePath: string,
  state: StoredCodexSessionState,
): Promise<void> => {
  await mkdir(dirname(statePath), { recursive: true })
  await writeFile(statePath, `${JSON.stringify(state, null, 2)}\n`)
}

const persistSession = async (
  statePath: string,
  desktopSessionId: string,
  session: Omit<StoredCodexSession, "updatedAt">,
): Promise<void> => {
  const state = await readState(statePath)
  state.sessions[desktopSessionId] = {
    ...session,
    updatedAt: isoNow(),
  }
  await writeState(statePath, state)
}

const extractThreadResult = (
  response: unknown,
  desktopSessionId?: string,
): KhalaCodeDesktopCodexThreadResult => {
  const thread = objectField(response, "thread")
  const threadId = stringField(thread, "id")
  if (thread === null || threadId === null) {
    throw new Error("Codex app-server returned a thread response without thread.id")
  }
  const messages = messagesFromThread(thread)
  return {
    ok: true,
    thread,
    threadId,
    ...(desktopSessionId === undefined ? {} : { desktopSessionId }),
    ...(messages.length === 0 ? {} : { messages }),
    ...(stringField(response, "cwd") === null ? {} : { cwd: stringField(response, "cwd") ?? "" }),
    ...(stringField(response, "model") === null ? {} : { model: stringField(response, "model") ?? "" }),
    ...(stringField(response, "modelProvider") === null ? {} : { modelProvider: stringField(response, "modelProvider") ?? "" }),
  }
}

const extractTurnId = (response: unknown): string => {
  const turn = objectField(response, "turn")
  const turnId = stringField(turn, "id")
  if (turnId === null) {
    throw new Error("Codex app-server returned a turn response without turn.id")
  }
  return turnId
}

const turnIdFromNotification = (notification: CodexAppServerNotification): string | null => {
  const params = notification.params
  if (!isObject(params)) return null
  const directTurnId = stringField(params, "turnId")
  if (directTurnId !== null) return directTurnId
  return stringField(objectField(params, "turn"), "id")
}

const threadIdFromNotification = (notification: CodexAppServerNotification): string | null => {
  const params = notification.params
  if (!isObject(params)) return null
  return stringField(params, "threadId")
}

const completedTurnStatus = (turn: JsonObject | null): string =>
  stringField(turn, "status") ?? "completed"

const isNoRolloutError = (error: unknown): boolean => {
  const message = error instanceof Error ? error.message : String(error)
  return message.toLowerCase().includes("no rollout found for thread id")
}

const normalizedThreadId = (threadId: string | undefined): string | null => {
  const trimmed = threadId?.trim()
  return trimmed === undefined || trimmed.length === 0 ? null : trimmed
}

export function createCodexAppServerChatRuntime(
  options: CreateCodexAppServerChatRuntimeOptions,
): CodexAppServerChatRuntime {
  const env = options.env ?? process.env
  const host = options.host
  const statePath = options.statePath ?? defaultStatePath(env)
  const turnTimeoutMs = options.turnTimeoutMs ?? DEFAULT_TURN_TIMEOUT_MS
  const messageTokenAuditRecorder = options.messageTokenAuditRecorder ?? null
  const tokenUsageReporter = options.tokenUsageReporter ?? null
  const activeTurnsByDesktopId = new Map<string, ActiveCodexTurn>()
  const loadedThreadsById = new Map<string, KhalaCodeDesktopCodexThreadResult>()

  const markThreadLoaded = (
    result: KhalaCodeDesktopCodexThreadResult,
  ): KhalaCodeDesktopCodexThreadResult => {
    loadedThreadsById.set(result.threadId, result)
    return result
  }

  const loadedThreadResult = (
    threadId: string,
    desktopSessionId?: string,
  ): KhalaCodeDesktopCodexThreadResult => {
    const cached = loadedThreadsById.get(threadId)
    if (cached !== undefined) {
      return desktopSessionId === undefined ? cached : { ...cached, desktopSessionId }
    }
    return {
      ok: true,
      thread: { id: threadId },
      threadId,
      ...(desktopSessionId === undefined ? {} : { desktopSessionId }),
    }
  }

  const ensureStarted = async (): Promise<void> => {
    const start = await host.start()
    if (!start.ok) {
      throw new Error(start.error ?? start.status.lastError ?? "Codex app-server failed to start")
    }
  }

  const startThread = async (
    request: KhalaCodeDesktopCodexThreadStartRequest = {},
  ): Promise<KhalaCodeDesktopCodexThreadResult> => {
    await ensureStarted()
    const response = await host.request("thread/start", {
      cwd: request.cwd ?? options.workingDirectory,
    })
    const result = markThreadLoaded(extractThreadResult(response, request.sessionId))
    if (request.sessionId !== undefined) {
      await persistSession(statePath, request.sessionId, { threadId: result.threadId })
    }
    return result
  }

  const resumeThread = async (
    request: KhalaCodeDesktopCodexThreadResumeRequest,
  ): Promise<KhalaCodeDesktopCodexThreadResult> => {
    await ensureStarted()
    const response = await host.request("thread/resume", {
      threadId: request.threadId,
      cwd: request.cwd ?? options.workingDirectory,
    })
    const result = markThreadLoaded(extractThreadResult(response, request.sessionId))
    if (request.sessionId !== undefined) {
      await persistSession(statePath, request.sessionId, { threadId: result.threadId })
    }
    return result
  }

  const ensureThreadForSession = async (
    desktopSessionId: string,
    cwd?: string,
    requestedThreadId?: string,
  ): Promise<KhalaCodeDesktopCodexThreadResult> => {
    const explicitThreadId = normalizedThreadId(requestedThreadId)
    if (explicitThreadId !== null) {
      if (loadedThreadsById.has(explicitThreadId)) {
        await persistSession(statePath, desktopSessionId, { threadId: explicitThreadId })
        return loadedThreadResult(explicitThreadId, desktopSessionId)
      }
      try {
        return await resumeThread({
          cwd: cwd ?? options.workingDirectory,
          sessionId: desktopSessionId,
          threadId: explicitThreadId,
        })
      } catch (error) {
        if (!isNoRolloutError(error)) throw error
        return startThread({
          cwd: cwd ?? options.workingDirectory,
          sessionId: desktopSessionId,
        })
      }
    }

    const state = await readState(statePath)
    const stored = state.sessions[desktopSessionId]
    if (stored !== undefined) {
      if (loadedThreadsById.has(stored.threadId)) {
        return loadedThreadResult(stored.threadId, desktopSessionId)
      }
      try {
        return await resumeThread({
          cwd: cwd ?? options.workingDirectory,
          sessionId: desktopSessionId,
          threadId: stored.threadId,
        })
      } catch (error) {
        if (!isNoRolloutError(error)) throw error
        return startThread({
          cwd: cwd ?? options.workingDirectory,
          sessionId: desktopSessionId,
        })
      }
    }
    return startThread({
      cwd: cwd ?? options.workingDirectory,
      sessionId: desktopSessionId,
    })
  }

  const listThreads = async (
    request: KhalaCodeDesktopCodexThreadListRequest = {},
  ): Promise<KhalaCodeDesktopCodexThreadListResult> => {
    await ensureStarted()
    const activeThreadId = request.sessionId === undefined
      ? null
      : (await readState(statePath)).sessions[request.sessionId]?.threadId ?? null
    const response = await host.request("thread/list", {
      ...(request.archived === undefined ? {} : { archived: request.archived }),
      ...(request.cursor === undefined ? {} : { cursor: request.cursor }),
      ...(request.cwd === undefined ? {} : { cwd: request.cwd }),
      ...(request.limit === undefined ? {} : { limit: request.limit }),
      ...(request.searchTerm === undefined ? {} : { searchTerm: request.searchTerm }),
      ...(request.useStateDbOnly === undefined ? {} : { useStateDbOnly: request.useStateDbOnly }),
    })
    const projection = projectKhalaCodeDesktopCodexThreadList({
      activeThreadId,
      response,
      ...(request.archived === undefined ? {} : { archived: request.archived }),
      ...(request.searchTerm === undefined ? {} : { searchTerm: request.searchTerm }),
    })
    return {
      ok: true,
      data: arrayField(response, "data") ?? [],
      backwardsCursor: isObject(response) ? (response.backwardsCursor as string | null | undefined) ?? null : null,
      nextCursor: isObject(response) ? (response.nextCursor as string | null | undefined) ?? null : null,
      groups: projection.groups,
      threads: projection.threads,
    }
  }

  const readThread = async (
    request: KhalaCodeDesktopCodexThreadReadRequest,
  ): Promise<KhalaCodeDesktopCodexThreadResult> => {
    await ensureStarted()
    const response = await host.request("thread/read", {
      threadId: request.threadId,
      includeTurns: request.includeTurns === true,
    })
    return extractThreadResult(response)
  }

  const renameThread = async (
    request: KhalaCodeDesktopCodexThreadRenameRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => {
    await ensureStarted()
    try {
      const response = await host.request("thread/name/set", {
        threadId: request.threadId,
        name: request.name,
      })
      return { ok: true, action: "rename", threadId: request.threadId, response }
    } catch (error) {
      return {
        ok: false,
        action: "rename",
        threadId: request.threadId,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const archiveThread = async (
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => {
    await ensureStarted()
    try {
      const response = await host.request("thread/archive", { threadId: request.threadId })
      return { ok: true, action: "archive", threadId: request.threadId, response }
    } catch (error) {
      return {
        ok: false,
        action: "archive",
        threadId: request.threadId,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const deleteThread = async (
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => {
    await ensureStarted()
    try {
      const response = await host.request("thread/delete", { threadId: request.threadId })
      return { ok: true, action: "delete", threadId: request.threadId, response }
    } catch (error) {
      return {
        ok: false,
        action: "delete",
        threadId: request.threadId,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const unarchiveThread = async (
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => {
    await ensureStarted()
    try {
      const response = await host.request("thread/unarchive", { threadId: request.threadId })
      const result = extractThreadResult(response)
      return {
        ok: true,
        action: "unarchive",
        threadId: request.threadId,
        thread: result.thread,
        response,
      }
    } catch (error) {
      return {
        ok: false,
        action: "unarchive",
        threadId: request.threadId,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const forkThread = async (
    request: KhalaCodeDesktopCodexThreadForkRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => {
    await ensureStarted()
    try {
      const response = await host.request("thread/fork", {
        threadId: request.threadId,
        ...(request.lastTurnId === undefined ? {} : { lastTurnId: request.lastTurnId }),
        cwd: request.cwd ?? options.workingDirectory,
      })
      const result = markThreadLoaded(extractThreadResult(response, request.sessionId))
      if (request.sessionId !== undefined) {
        await persistSession(statePath, request.sessionId, { threadId: result.threadId })
      }
      return {
        ok: true,
        action: "fork",
        threadId: request.threadId,
        newThreadId: result.threadId,
        thread: result.thread,
        response,
        ...(result.messages === undefined ? {} : { messages: result.messages }),
      }
    } catch (error) {
      return {
        ok: false,
        action: "fork",
        threadId: request.threadId,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const steerTurn = async (
    request: KhalaCodeDesktopCodexTurnSteerRequest,
  ): Promise<KhalaCodeDesktopCodexTurnActionResult> => {
    const active = request.turnId === undefined
      ? [...activeTurnsByDesktopId.values()].find(turn => turn.desktopSessionId === request.sessionId)
      : activeTurnsByDesktopId.get(request.turnId)
    if (active === undefined) {
      return {
        ok: false,
        desktopSessionId: request.sessionId,
        ...(request.turnId === undefined ? {} : { desktopTurnId: request.turnId }),
        error: "No active Codex turn is registered for this desktop session.",
      }
    }
    await ensureStarted()
    const response = await host.request("turn/steer", {
      threadId: active.codexThreadId,
      clientUserMessageId: request.clientUserMessageId,
      input: [{ type: "text", text: request.text, textElements: [] }],
      expectedTurnId: active.codexTurnId,
    })
    return {
      ok: true,
      codexTurnId: stringField(response, "turnId") ?? active.codexTurnId,
      desktopSessionId: request.sessionId,
      desktopTurnId: active.desktopTurnId,
      response,
      threadId: active.codexThreadId,
    }
  }

  const interruptTurn = async (
    request: KhalaCodeDesktopCodexTurnInterruptRequest,
  ): Promise<KhalaCodeDesktopCodexTurnActionResult> => {
    const active = request.turnId === undefined
      ? [...activeTurnsByDesktopId.values()].find(turn => turn.desktopSessionId === request.sessionId)
      : activeTurnsByDesktopId.get(request.turnId)
    if (active === undefined) {
      return {
        ok: false,
        desktopSessionId: request.sessionId,
        ...(request.turnId === undefined ? {} : { desktopTurnId: request.turnId }),
        error: "No active Codex turn is registered for this desktop session.",
      }
    }
    await ensureStarted()
    try {
      const response = await host.request("turn/interrupt", {
        threadId: active.codexThreadId,
        turnId: active.codexTurnId,
      })
      return {
        ok: true,
        codexTurnId: active.codexTurnId,
        desktopSessionId: request.sessionId,
        desktopTurnId: active.desktopTurnId,
        response,
        threadId: active.codexThreadId,
      }
    } catch (error) {
      return {
        ok: false,
        codexTurnId: active.codexTurnId,
        desktopSessionId: request.sessionId,
        desktopTurnId: active.desktopTurnId,
        error: error instanceof Error ? error.message : String(error),
        threadId: active.codexThreadId,
      }
    }
  }

  const threadIdForSession = async (sessionId: string): Promise<string | null> =>
    (await readState(statePath)).sessions[sessionId]?.threadId ?? null

  const compactThread = async (
    request: KhalaCodeDesktopCodexThreadCompactRequest,
  ): Promise<KhalaCodeDesktopCodexTurnActionResult> => {
    await ensureStarted()
    const threadId =
      request.threadId ??
      (request.sessionId === undefined
        ? undefined
        : (await readState(statePath)).sessions[request.sessionId]?.threadId)
    if (threadId === undefined) {
      return {
        ok: false,
        desktopSessionId: request.sessionId ?? "unknown",
        error: "No Codex thread id was supplied or registered for this desktop session.",
      }
    }
    try {
      const response = await host.request("thread/compact/start", { threadId })
      return {
        ok: true,
        desktopSessionId: request.sessionId ?? "unknown",
        response,
        threadId,
      }
    } catch (error) {
      return {
        ok: false,
        desktopSessionId: request.sessionId ?? "unknown",
        error: error instanceof Error ? error.message : String(error),
        threadId,
      }
    }
  }

  const startTurn = async (
    request: KhalaCodeDesktopChatTurnRequest & { readonly cwd?: string },
  ): Promise<KhalaCodeDesktopChatTurnResponse> => {
    const desktopTurnId = request.turnId ?? `codex-turn-${Date.now().toString(36)}`
    const userMessage = [...request.messages].reverse().find(message => message.role === "user")
    if (userMessage === undefined || userMessage.body.trim().length === 0) {
      throw new Error("Codex turn requires a non-empty user message.")
    }

    let thread = await ensureThreadForSession(request.sessionId, request.cwd, request.threadId)
    options.onEvent?.({
      threadId: thread.threadId,
      turnId: desktopTurnId,
      type: "thread_ready",
    })
    const projector = createCodexThreadItemEventProjector({ desktopTurnId })
    let capturedUsage = emptyCodexUsage()
    let codexTurnId: string | null = null
    let turnStatus = "inProgress"
    let lastTotalUsage: KhalaCodeDesktopCodexTokenUsageCounts | null = null
    let lastTotalUsageKey: string | null = null
    let tokenUsageSequence = 0
    const submittedAt = isoNow()
    const tokenUsageAuditEvents: KhalaCodeDesktopCodexMessageTokenAuditUsageEvent[] = []
    const pendingTokenUsageReports: Promise<void>[] = []
    let done = false
    let finish!: () => void
    let fail!: (error: Error) => void
    const completed = new Promise<void>((resolve, reject) => {
      finish = resolve
      fail = reject
    })
    const timeout = setTimeout(() => {
      fail(new Error("Codex turn timed out before turn/completed arrived."))
    }, turnTimeoutMs)

    const shouldHandle = (notification: CodexAppServerNotification): boolean => {
      if (threadIdFromNotification(notification) !== thread.threadId) return false
      const notificationTurnId = turnIdFromNotification(notification)
      if (notificationTurnId === null) {
        return codexTurnId !== null && isTokenUsageNotification(notification)
      }
      if (codexTurnId === null) {
        codexTurnId = notificationTurnId
        activeTurnsByDesktopId.set(desktopTurnId, {
          codexThreadId: thread.threadId,
          codexTurnId,
          desktopSessionId: request.sessionId,
          desktopTurnId,
        })
      }
      return notificationTurnId === codexTurnId
    }

    const captureTokenUsage = (notification: CodexAppServerNotification): void => {
      const info = tokenUsageInfoFromNotification(notification)
      if (info === null) return

      let delta: KhalaCodeDesktopCodexTokenUsageCounts | null = info.lastUsage
      if (info.totalUsage !== null) {
        const totalKey = codexUsageKey(info.totalUsage)
        if (totalKey === lastTotalUsageKey) return
        if (delta === null && lastTotalUsage !== null) {
          delta = subtractCodexUsage(info.totalUsage, lastTotalUsage)
        }
        lastTotalUsage = info.totalUsage
        lastTotalUsageKey = totalKey
      }

      if (delta === null || !codexUsageHasTokens(delta)) return
      tokenUsageSequence += 1
      capturedUsage = addCodexUsage(capturedUsage, delta)
      if (codexTurnId === null) return
      const tokenUsageReport = {
        clientUserMessageId: userMessage.id,
        codexThreadId: thread.threadId,
        codexTurnId,
        desktopSessionId: request.sessionId,
        desktopTurnId,
        model: thread.model ?? "openagents/codex-direct-local",
        observedAt: notification.receivedAt,
        sequence: tokenUsageSequence,
        turnStatus,
        usage: delta,
      }
      const refs = khalaCodeDesktopCodexTokenUsageEventRefs(tokenUsageReport)
      tokenUsageAuditEvents.push({
        eventId: refs.eventId,
        idempotencyKey: refs.idempotencyKey,
        observedAt: notification.receivedAt,
        sequence: tokenUsageSequence,
        usage: delta,
      })
      if (tokenUsageReporter === null) return
      const report = tokenUsageReporter(tokenUsageReport).catch(() => undefined)
      pendingTokenUsageReports.push(report)
    }

    const unsubscribe = host.subscribe(notification => {
      if (!shouldHandle(notification)) return
      captureTokenUsage(notification)
      if (done) return
      for (const event of projector.accept(notification)) options.onEvent?.(event)
      if (notification.method === "turn/completed") {
        turnStatus = completedTurnStatus(objectField(notification.params, "turn"))
        done = true
        finish()
      }
    })

    try {
      await ensureStarted()
      const requestTurnStart = async (): Promise<unknown> =>
        host.request("turn/start", {
          threadId: thread.threadId,
          clientUserMessageId: userMessage.id,
          input: [{ type: "text", text: userMessage.body, textElements: [] }],
          cwd: request.cwd ?? options.workingDirectory,
          responsesapiClientMetadata: {
            khalaDesktopSessionId: request.sessionId,
            khalaDesktopTurnId: desktopTurnId,
          },
        })
      let turnStartResponse: unknown
      try {
        turnStartResponse = await requestTurnStart()
      } catch (error) {
        if (!isNoRolloutError(error)) throw error
        loadedThreadsById.delete(thread.threadId)
        try {
          thread = await resumeThread({
            cwd: request.cwd ?? options.workingDirectory,
            sessionId: request.sessionId,
            threadId: thread.threadId,
          })
        } catch (resumeError) {
          if (!isNoRolloutError(resumeError)) throw resumeError
          thread = await startThread({
            cwd: request.cwd ?? options.workingDirectory,
            sessionId: request.sessionId,
          })
        }
        turnStartResponse = await requestTurnStart()
      }
      codexTurnId = extractTurnId(turnStartResponse)
      activeTurnsByDesktopId.set(desktopTurnId, {
        codexThreadId: thread.threadId,
        codexTurnId,
        desktopSessionId: request.sessionId,
        desktopTurnId,
      })
      await persistSession(statePath, request.sessionId, {
        lastCodexTurnId: codexTurnId,
        threadId: thread.threadId,
      })
      await completed
    } finally {
      clearTimeout(timeout)
      unsubscribe()
      activeTurnsByDesktopId.delete(desktopTurnId)
      await Promise.allSettled(pendingTokenUsageReports)
      if (messageTokenAuditRecorder !== null) {
        const messages = projector.messages()
        await messageTokenAuditRecorder({
          clientUserMessage: khalaCodeDesktopCodexMessageTokenAuditMessage(
            userMessage,
            "khala_code_client",
          ),
          codexMessages: messages.map(message =>
            khalaCodeDesktopCodexMessageTokenAuditMessage(message, "codex_app_server")
          ),
          codexThreadId: thread.threadId,
          ...(codexTurnId === null ? {} : { codexTurnId }),
          completedAt: isoNow(),
          desktopSessionId: request.sessionId,
          desktopTurnId,
          model: thread.model ?? "openagents/codex-direct-local",
          reconciliation: {
            globalCountedTokens: capturedUsage.inputTokens + capturedUsage.outputTokens,
            globalCounterRoute: "/api/stats/token-usage/events",
            status: tokenUsageAuditEvents.length > 0
              ? "global_count_event_recorded"
              : "missing_token_usage_update",
            tokenAccountingRequired: true,
            tokenScope: "codex_turn_provider_reported",
            usageTruth: "exact",
          },
          submittedAt,
          turnStatus,
          usage: capturedUsage,
          usageEvents: tokenUsageAuditEvents,
        })
      }
    }

    const messages = projector.messages()
    return {
      backend: {
        kind: "codex_app_server",
        model: thread.model ?? "codex",
        runtimeMode: "codex_harness",
        threadId: thread.threadId,
        toolCatalogKind: "codex_app_server",
        ...(codexTurnId === null ? {} : { turnId: codexTurnId }),
        turnStatus,
      },
      messages: messages.length > 0
        ? messages
        : [{
          id: `${desktopTurnId}-codex-status`,
          role: "system",
          body: turnStatus === "interrupted"
            ? "Codex interrupted this turn."
            : `Codex completed the turn with status: ${turnStatus}.`,
        }],
      ok: turnStatus === "completed" || messages.length > 0,
      toolNames: [],
      ...(codexUsageHasTokens(capturedUsage) ? { usage: desktopUsageFromCodexUsage(capturedUsage) } : {}),
      usedTools: [],
    }
  }

  return {
    archiveThread,
    compactThread,
    deleteThread,
    forkThread,
    interruptTurn,
    listThreads,
    readThread,
    renameThread,
    resumeThread,
    startThread,
    startTurn,
    steerTurn,
    threadIdForSession,
    unarchiveThread,
  }
}
