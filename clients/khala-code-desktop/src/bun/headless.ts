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
import { runKhalaCodeDesktopChatTurn } from "./khala-chat-runtime.js"

export type KhalaCodeDesktopHeadlessRunInput = {
  readonly env: Readonly<Record<string, string | undefined>>
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

  emitJsonl(khalaCodeHeadlessThreadStarted({ sessionId }))
  emitJsonl(khalaCodeHeadlessTurnStarted(turnId))

  try {
    const response = await runKhalaCodeDesktopChatTurn({
      env: input.env,
      onEvent,
      request: {
        messages: [{ body: input.prompt, id: "headless-user-1", role: "user" }],
        sessionId,
        turnId,
      },
      ...(input.workingDirectory === undefined ? {} : { workingDirectory: input.workingDirectory }),
    })
    const finalMessage = lastAssistantMessage(response.messages)?.body ?? ""
    emitJsonl(khalaCodeHeadlessTurnCompleted({
      finalMessage,
      ok: response.ok,
      turnId,
      ...(response.usage === undefined ? {} : { usage: response.usage }),
    }))
    stdout.write(`${JSON.stringify({
      backend: response.backend,
      finalMessage,
      ok: response.ok,
      sessionId,
      toolNames: response.toolNames,
      turnId,
      usage: response.usage,
      usedTools: response.usedTools,
    })}\n`)
    return { finalMessage, response, sessionId, turnId }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    emitJsonl(khalaCodeHeadlessTurnFailed({ error: message, turnId }))
    stdout.write(`${JSON.stringify({
      error: message,
      finalMessage: "",
      ok: false,
      sessionId,
      turnId,
    })}\n`)
    throw error
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
