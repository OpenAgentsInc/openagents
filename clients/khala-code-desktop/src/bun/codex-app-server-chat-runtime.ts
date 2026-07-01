import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopChatTurnRequest,
  KhalaCodeDesktopChatTurnResponse,
  KhalaCodeDesktopMessage,
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
    const projector = createCodexThreadItemEventProjector({ desktopTurnId })
    let codexTurnId: string | null = null
    let turnStatus = "completed"
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
      if (notificationTurnId === null) return false
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

    const unsubscribe = host.subscribe(notification => {
      if (!shouldHandle(notification) || done) return
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
