/**
 * MH-3: Desktop ChatRuntime adapter for Grok ACP.
 *
 * Implements CodexAppServerChatRuntime enough for submitChatMessage /
 * codexTurnStart / interrupt / startThread. Other lifecycle methods throw
 * explicit unsupported errors until parity expands.
 */

import { randomUUID } from "node:crypto"

import {
  createGrokAcpChatRuntime,
  createGrokSessionStore,
  type CreateGrokAcpChatRuntimeOptions,
  type GrokAcpChatRuntime,
  type GrokSessionStore,
  type NeutralChatTurnEvent,
} from "@openagentsinc/grok-harness"

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
  KhalaCodeDesktopSlashCommandDispatchRequest,
  KhalaCodeDesktopSlashCommandDispatchResult,
  KhalaCodeDesktopSlashCommandListRequest,
  KhalaCodeDesktopSlashCommandListResponse,
} from "../shared/rpc.js"
import type { CodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import type { KhalaCodeDesktopSlashCommandWithAvailability } from "../shared/codex-slash-commands.js"

const unsupported = (method: string): never => {
  throw new Error(
    `Grok harness does not support ${method} yet (MH-3 chat turns only).`,
  )
}

const textFromRequest = (request: KhalaCodeDesktopChatTurnRequest): string =>
  [...request.messages].reverse().find((message) => message.role === "user")?.body
    .trim() ?? ""

const toDesktopEvent = (event: NeutralChatTurnEvent): KhalaCodeDesktopChatTurnEvent => {
  switch (event.type) {
    case "thread_ready":
      return event
    case "message_start":
      return {
        type: "message_start",
        turnId: event.turnId,
        message: {
          id: event.message.id,
          role: "assistant",
          body: event.message.content,
        },
      }
    case "message_delta":
      return event
    case "message_replace":
      return {
        type: "message_replace",
        turnId: event.turnId,
        message: {
          id: event.message.id,
          role: "assistant",
          body: event.message.content,
        },
      }
    case "message_done":
      return event
    case "tool_event":
      return {
        type: "tool_event",
        turnId: event.turnId,
        event: {
          eventId: `grok-tool-${event.turnId}`,
          kind: "tool_requested",
          payload: event.event,
          sessionId: event.turnId,
        },
      }
  }
}

export type GrokDesktopChatRuntime = CodexAppServerChatRuntime & {
  readonly harnessKind: "grok"
  readonly slashCommandDispatch: (
    request: KhalaCodeDesktopSlashCommandDispatchRequest,
  ) => Promise<KhalaCodeDesktopSlashCommandDispatchResult>
  readonly slashCommandList: (
    request?: KhalaCodeDesktopSlashCommandListRequest,
  ) => Promise<KhalaCodeDesktopSlashCommandListResponse>
}

export type CreateGrokDesktopChatRuntimeOptions = {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly workingDirectory?: string
  readonly onEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly acp?: CreateGrokAcpChatRuntimeOptions["acp"]
  readonly sessionStore?: GrokSessionStore
  readonly acpRuntime?: GrokAcpChatRuntime
  readonly model?: string
}

export async function createGrokDesktopChatRuntime(
  options: CreateGrokDesktopChatRuntimeOptions = {},
): Promise<GrokDesktopChatRuntime> {
  const sessionStore =
    options.sessionStore ??
    createGrokSessionStore({
      ...(options.env === undefined ? {} : { env: options.env }),
    })
  const model = options.model ?? "grok-4.5"

  const acpRuntime =
    options.acpRuntime ??
    (await createGrokAcpChatRuntime({
      ...(options.acp === undefined ? {} : { acp: options.acp }),
      sessionStore,
    }))

  const threads = new Map<
    string,
    { readonly threadId: string; readonly grokSessionId: string }
  >()

  const mutation = async (
    action: KhalaCodeDesktopCodexThreadMutationResult["action"],
    request: KhalaCodeDesktopCodexThreadIdRequest,
  ): Promise<KhalaCodeDesktopCodexThreadMutationResult> => ({
    action,
    ok: false,
    threadId: request.threadId,
    error: `Grok harness does not support ${action} yet`,
  })

  const runtime: GrokDesktopChatRuntime = {
    harnessKind: "grok",

    async startThread(
      request: KhalaCodeDesktopCodexThreadStartRequest = {},
    ): Promise<KhalaCodeDesktopCodexThreadResult> {
      const desktopSessionId =
        request.sessionId?.trim() || `grok-desktop-${randomUUID()}`
      const started = await acpRuntime.startThread({
        desktopSessionId,
        cwd: request.cwd ?? options.workingDirectory ?? process.cwd(),
      })
      threads.set(desktopSessionId, {
        threadId: started.threadId,
        grokSessionId: started.grokSessionId,
      })
      return {
        ok: true,
        desktopSessionId,
        threadId: started.threadId,
        thread: { id: started.threadId, title: "Grok session" },
        ...(request.cwd === undefined ? {} : { cwd: request.cwd }),
        model,
      }
    },

    async startTurn(
      request: KhalaCodeDesktopChatTurnRequest & { readonly cwd?: string },
    ): Promise<KhalaCodeDesktopChatTurnResponse> {
      const desktopSessionId = request.sessionId
      let mapping = threads.get(desktopSessionId)
      if (mapping === undefined || request.startNewThread === true) {
        const started = await acpRuntime.startThread({
          desktopSessionId,
          cwd: request.cwd ?? options.workingDirectory ?? process.cwd(),
        })
        mapping = {
          threadId: started.threadId,
          grokSessionId: started.grokSessionId,
        }
        threads.set(desktopSessionId, mapping)
      }

      const prompt = textFromRequest(request)
      if (prompt.length === 0) {
        throw new Error("Grok chat runtime requires a non-empty user message.")
      }

      const turnId = request.turnId ?? `grok-turn-${randomUUID()}`
      options.onEvent?.({
        type: "thread_ready",
        threadId: mapping.threadId,
        turnId,
      })

      const turn = await acpRuntime.startTurn({
        threadId: mapping.threadId,
        desktopSessionId,
        grokSessionId: mapping.grokSessionId,
        prompt,
        onEvent: (event) => {
          options.onEvent?.(toDesktopEvent(event))
        },
      })

      return {
        ok: true,
        backend: {
          kind: "grok_acp",
          model,
          runtimeMode: "grok_runtime",
          threadId: mapping.threadId,
          turnId: turn.turnId,
          turnStatus: "completed",
          toolCatalogKind: "codex_harness_supplemental",
        },
        messages: [
          {
            id: `msg_${turn.turnId}`,
            role: "assistant",
            body: turn.text,
          },
        ],
        toolNames: [],
        usedTools: [],
      }
    },

    async interruptTurn(
      request: KhalaCodeDesktopCodexTurnInterruptRequest,
    ): Promise<KhalaCodeDesktopCodexTurnActionResult> {
      await acpRuntime.interruptTurn()
      return {
        ok: true,
        desktopSessionId: request.sessionId,
        ...(request.turnId === undefined ? {} : { desktopTurnId: request.turnId }),
      }
    },

    async threadIdForSession(desktopSessionId: string): Promise<string | null> {
      const mapped = threads.get(desktopSessionId)
      if (mapped) return mapped.threadId
      const stored = await sessionStore.get(desktopSessionId)
      return stored?.grokSessionId ?? null
    },

    async listThreads(
      request: KhalaCodeDesktopCodexThreadListRequest = {},
    ): Promise<KhalaCodeDesktopCodexThreadListResult> {
      const entries = await sessionStore.list()
      const search = request.searchTerm?.trim().toLowerCase()
      const threadsList = entries
        .filter((entry) => {
          if (search === undefined || search.length === 0) return true
          return (
            entry.grokSessionId.toLowerCase().includes(search) ||
            entry.desktopSessionId.toLowerCase().includes(search)
          )
        })
        .map((entry) => ({
          id: entry.grokSessionId,
          sessionId: entry.desktopSessionId,
          title: `Grok ${entry.grokSessionId.slice(0, 8)}`,
          preview: "",
          cwd: null,
          projectLabel: "Grok",
          status: "ready",
          statusLabel: "Grok session",
          modelProvider: "grok",
          source: "grok_acp",
          forkedFromId: null,
          parentThreadId: null,
          createdAt: null,
          updatedAt: Date.parse(entry.updatedAt) || null,
          recencyAt: Date.parse(entry.updatedAt) || null,
          badges: ["Grok"],
          resumable: false,
        }))
        .slice(0, request.limit ?? undefined)
      return {
        ok: true,
        data: entries,
        groups: [
          {
            key: "grok",
            label: "Grok",
            threadIds: threadsList.map((t) => t.id),
          },
        ],
        threads: threadsList,
      }
    },

    async readThread(
      request: KhalaCodeDesktopCodexThreadReadRequest,
    ): Promise<KhalaCodeDesktopCodexThreadResult> {
      return {
        ok: true,
        threadId: request.threadId,
        desktopSessionId: request.threadId,
        thread: { id: request.threadId, title: "Grok session" },
        messages: [],
        model,
      }
    },

    async resumeThread(
      request: KhalaCodeDesktopCodexThreadResumeRequest,
    ): Promise<KhalaCodeDesktopCodexThreadResult> {
      return runtime.readThread({
        threadId: request.threadId,
        includeTurns: true,
      })
    },

    compactThread: async (_r: KhalaCodeDesktopCodexThreadCompactRequest) =>
      unsupported("compactThread"),
    archiveThread: (r) => mutation("archive", r),
    deleteThread: (r) => mutation("delete", r),
    unarchiveThread: (r) => mutation("unarchive", r),
    forkThread: async (_r: KhalaCodeDesktopCodexThreadForkRequest) =>
      unsupported("forkThread"),
    renameThread: async (_r: KhalaCodeDesktopCodexThreadRenameRequest) =>
      unsupported("renameThread"),
    steerTurn: async (_r: KhalaCodeDesktopCodexTurnSteerRequest) =>
      unsupported("steerTurn"),

    async slashCommandDispatch(
      request: KhalaCodeDesktopSlashCommandDispatchRequest,
    ): Promise<KhalaCodeDesktopSlashCommandDispatchResult> {
      return {
        ok: false,
        status: "blocked",
        message: `Grok harness does not support slash command: ${request.raw}`,
      }
    },

    async slashCommandList(
      _request?: KhalaCodeDesktopSlashCommandListRequest,
    ): Promise<KhalaCodeDesktopSlashCommandListResponse> {
      return {
        ok: true,
        commands: [] as KhalaCodeDesktopSlashCommandWithAvailability[],
      }
    },
  }

  return runtime
}
