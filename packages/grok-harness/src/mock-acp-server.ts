/**
 * Minimal in-process ACP JSON-RPC server for fixture tests.
 * Speaks the same surface as `grok agent stdio` for initialize /
 * authenticate / session/new / session/prompt + session/update chunks.
 */

import { randomUUID } from "node:crypto"

type JsonRpcRequest = {
  readonly jsonrpc?: string
  readonly id?: number | string
  readonly method?: string
  readonly params?: Record<string, unknown>
}

export type MockAcpServerOptions = {
  readonly replyText?: string
  readonly chunkSize?: number
  readonly authMethods?: readonly { readonly id: string }[]
  readonly onPrompt?: (text: string) => string
}

export function createMockAcpStdioPair(options: MockAcpServerOptions = {}) {
  const replyText = options.replyText ?? "hello from mock grok"
  const chunkSize = options.chunkSize ?? 8
  const authMethods = options.authMethods ?? [
    { id: "cached_token" },
    { id: "xai.api_key" },
  ]

  let nextSession = 0
  const sessions = new Map<string, { cwd: string }>()

  const clientToServer: string[] = []
  const serverToClient: string[] = []

  function writeResponse(id: number | string | undefined, result: unknown) {
    serverToClient.push(JSON.stringify({ jsonrpc: "2.0", id, result }))
  }

  function writeError(id: number | string | undefined, message: string) {
    serverToClient.push(
      JSON.stringify({
        jsonrpc: "2.0",
        id,
        error: { code: -32000, message },
      }),
    )
  }

  function writeNotification(method: string, params: unknown) {
    serverToClient.push(JSON.stringify({ jsonrpc: "2.0", method, params }))
  }

  function handleLine(line: string) {
    let msg: JsonRpcRequest
    try {
      msg = JSON.parse(line) as JsonRpcRequest
    } catch {
      return
    }
    const method = msg.method
    const id = msg.id
    const params = msg.params ?? {}

    if (method === "initialize") {
      writeResponse(id, {
        protocolVersion: 1,
        serverInfo: { name: "mock-grok-acp", version: "0.0.0" },
        authMethods,
        agentCapabilities: {},
      })
      return
    }

    if (method === "authenticate") {
      writeResponse(id, {})
      return
    }

    if (method === "session/new") {
      const sessionId = `mock-session-${++nextSession}-${randomUUID().slice(0, 8)}`
      sessions.set(sessionId, { cwd: String(params.cwd ?? process.cwd()) })
      writeResponse(id, { sessionId })
      return
    }

    if (method === "session/prompt") {
      const sessionId = String(params.sessionId ?? "")
      if (!sessions.has(sessionId)) {
        writeError(id, `unknown session ${sessionId}`)
        return
      }
      const promptBlocks = params.prompt as
        | ReadonlyArray<{ type?: string; text?: string }>
        | undefined
      const userText =
        promptBlocks
          ?.map((b) => b.text ?? "")
          .join("")
          .trim() || ""
      const text = options.onPrompt?.(userText) ?? replyText

      for (let i = 0; i < text.length; i += chunkSize) {
        const delta = text.slice(i, i + chunkSize)
        writeNotification("session/update", {
          sessionId,
          update: {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: delta },
          },
        })
      }

      writeResponse(id, {
        stopReason: "end_turn",
        sessionId,
      })
      return
    }

    writeError(id, `unsupported method ${method}`)
  }

  return {
    /** Push a client → server JSON-RPC line (no trailing newline required). */
    pushClientLine(line: string) {
      clientToServer.push(line)
      handleLine(line.trim())
    },
    /** Drain server → client lines produced so far. */
    drainServerLines(): string[] {
      return serverToClient.splice(0, serverToClient.length)
    },
    sessions,
  }
}

/**
 * Spawnable mock as a real subprocess for integration-shaped tests.
 * Usage: `bun packages/grok-harness/scripts/mock-acp-stdio.ts`
 */
export async function runMockAcpStdioMain(): Promise<void> {
  const pair = createMockAcpStdioPair({
    replyText: process.env.MOCK_GROK_REPLY ?? "mock-acp-ok",
  })

  const decoder = new TextDecoder()
  let buffer = ""

  // Read stdin line-by-line
  for await (const chunk of Bun.stdin.stream()) {
    buffer += decoder.decode(chunk)
    let idx: number
    while ((idx = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, idx)
      buffer = buffer.slice(idx + 1)
      if (line.trim().length === 0) continue
      pair.pushClientLine(line)
      for (const out of pair.drainServerLines()) {
        process.stdout.write(`${out}\n`)
      }
    }
  }
}
