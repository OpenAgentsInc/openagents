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

type ClaudeQuery = AsyncIterable<unknown> & {
  readonly close?: () => Promise<void> | void
  readonly interrupt?: () => Promise<void> | void
}

type ClaudeQueryFn = (input: {
  readonly prompt: string
  readonly options: Record<string, unknown>
}) => ClaudeQuery

type ClaudeSdkModule = {
  readonly query: ClaudeQueryFn
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
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly importer?: (specifier: string) => Promise<unknown>
  readonly onEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly query?: ClaudeQueryFn
  readonly sessionStore?: ClaudeSessionStore
  readonly workingDirectory: string
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
): CodexAppServerChatRuntime {
  const sessionStore = options.sessionStore ?? createClaudeSessionStore({
    ...(options.env === undefined ? {} : { env: options.env }),
  })
  const activeTurns = new Map<string, ActiveClaudeTurn>()

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

    const acquireQuery = Effect.tryPromise({
      try: async () => query({
        prompt,
        options: {
          abortController: controller,
          cwd: request.cwd ?? options.workingDirectory,
          includePartialMessages: true,
          permissionMode: options.env?.KHALA_CODE_DESKTOP_CLAUDE_PERMISSION_MODE ?? "acceptEdits",
          ...(shouldResume ? { resume: threadId } : { sessionId: freshSessionId }),
          env: {
            ...options.env,
            ...(options.env?.CLAUDE_CONFIG_DIR === undefined ? {} : { CLAUDE_CONFIG_DIR: options.env.CLAUDE_CONFIG_DIR }),
          },
        },
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
    unarchiveThread: request => mutationUnsupported("unarchive", request),
  }
}
