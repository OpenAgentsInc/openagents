import {
  khalaCodeHeadlessThreadStarted,
  khalaCodeHeadlessTurnCompleted,
  khalaCodeHeadlessTurnFailed,
  khalaCodeHeadlessTurnStarted,
  projectKhalaCodeDesktopEventToThreadEvents,
  stringifyKhalaCodeHeadlessThreadEvent,
} from "../shared/headless-events.js"
import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopChatTurnResponse,
} from "../shared/rpc.js"
import type { CodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"

export type KhalaCodeDesktopHeadlessChatRuntime = Pick<
  CodexAppServerChatRuntime,
  "interruptTurn" | "startThread" | "startTurn"
>

export type KhalaCodeDesktopHeadlessCodexRuntime = KhalaCodeDesktopHeadlessChatRuntime

export type KhalaCodeDesktopHeadlessRunInput = {
  readonly createCodexChatRuntime: (input: {
    readonly onEvent: (event: KhalaCodeDesktopChatTurnEvent) => void
  }) => KhalaCodeDesktopHeadlessChatRuntime
  readonly env: Readonly<Record<string, string | undefined>>
  readonly interruptAfterMs?: number
  readonly prompt: string
  readonly sessionId?: string
  readonly stderr?: Pick<typeof process.stderr, "write">
  readonly stdout?: Pick<typeof process.stdout, "write">
  readonly turnId?: string
  readonly workingDirectory?: string
}

export type KhalaCodeDesktopHeadlessRunResult = {
  readonly finalMessage: string
  readonly response: KhalaCodeDesktopChatTurnResponse
  readonly sessionId: string
  readonly turnId: string
}

export async function runKhalaCodeDesktopHeadlessJsonl(
  input: KhalaCodeDesktopHeadlessRunInput,
): Promise<KhalaCodeDesktopHeadlessRunResult> {
  const sessionId = input.sessionId ?? `khala-code-headless-${Date.now().toString(36)}`
  const turnId = input.turnId ?? `turn-${Date.now().toString(36)}`
  const stderr = input.stderr ?? process.stderr
  const stdout = input.stdout ?? process.stdout
  const emitJsonl = (event: Parameters<typeof stringifyKhalaCodeHeadlessThreadEvent>[0]): void => {
    stderr.write(`${stringifyKhalaCodeHeadlessThreadEvent(event)}\n`)
  }
  const onEvent = (event: KhalaCodeDesktopChatTurnEvent): void => {
    for (const threadEvent of projectKhalaCodeDesktopEventToThreadEvents(event)) {
      emitJsonl(threadEvent)
    }
  }
  const codexChatRuntime = input.createCodexChatRuntime({ onEvent })

  let threadId: string | undefined
  let interruptTimer: ReturnType<typeof setTimeout> | undefined

  try {
    const thread = await codexChatRuntime.startThread({
      sessionId,
      ...(input.workingDirectory === undefined ? {} : { cwd: input.workingDirectory }),
    })
    threadId = thread.threadId
    emitJsonl(khalaCodeHeadlessThreadStarted({ sessionId, threadId }))
    emitJsonl(khalaCodeHeadlessTurnStarted(turnId, { threadId }))

    if (input.interruptAfterMs !== undefined && input.interruptAfterMs >= 0) {
      interruptTimer = setTimeout(() => {
        void codexChatRuntime.interruptTurn({ sessionId, turnId }).catch(() => undefined)
      }, input.interruptAfterMs)
    }

    const response = await codexChatRuntime.startTurn({
      messages: [{ body: input.prompt, id: "headless-user-1", role: "user" }],
      sessionId,
      ...(threadId === undefined ? {} : { threadId }),
      turnId,
      ...(input.workingDirectory === undefined ? {} : { cwd: input.workingDirectory }),
    })
    const finalMessage = lastAssistantMessage(response.messages)?.body ?? ""
    const responseThreadId = response.backend.threadId ?? threadId
    emitJsonl(khalaCodeHeadlessTurnCompleted({
      ...(response.backend.turnId === undefined ? {} : { codexTurnId: response.backend.turnId }),
      finalMessage,
      ok: response.ok,
      ...(response.backend.turnStatus === undefined ? {} : { status: response.backend.turnStatus }),
      ...(responseThreadId === undefined ? {} : { threadId: responseThreadId }),
      turnId,
      ...(response.usage === undefined ? {} : { usage: response.usage }),
    }))
    stdout.write(`${JSON.stringify({
      backend: response.backend,
      codexTurnId: response.backend.turnId,
      finalMessage,
      ok: response.ok,
      sessionId,
      threadId: response.backend.threadId ?? threadId,
      toolNames: response.toolNames,
      turnId,
      usage: response.usage,
      usedTools: response.usedTools,
    })}\n`)
    return { finalMessage, response, sessionId, turnId }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    emitJsonl(khalaCodeHeadlessTurnFailed({
      error: message,
      status: "codex_app_server_unavailable",
      ...(threadId === undefined ? {} : { threadId }),
      turnId,
    }))
    stdout.write(`${JSON.stringify({
      backend: {
        kind: "codex_app_server",
        runtimeMode: "codex_harness",
        toolCatalogKind: "codex_app_server",
      },
      error: message,
      finalMessage: "",
      ok: false,
      sessionId,
      status: "codex_app_server_unavailable",
      ...(threadId === undefined ? {} : { threadId }),
      turnId,
    })}\n`)
    throw error
  } finally {
    if (interruptTimer !== undefined) clearTimeout(interruptTimer)
  }
}

function lastAssistantMessage(
  messages: readonly { readonly body: string; readonly role: string }[],
): { readonly body: string; readonly role: string } | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (message?.role === "assistant") return message
  }
  return undefined
}

export async function readKhalaCodeHeadlessPrompt(argv: readonly string[]): Promise<string> {
  const prompt = argv.filter(arg => arg !== "--json").join(" ").trim()
  if (prompt.length > 0) return prompt
  if (process.stdin.isTTY) return ""
  const chunks: Buffer[] = []
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(String(chunk)))
  }
  return Buffer.concat(chunks).toString("utf8").trim()
}
