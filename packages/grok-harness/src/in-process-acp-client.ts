import type { AcpSessionUpdate } from "./event-projector.ts"
import { createMockAcpStdioPair, type MockAcpServerOptions } from "./mock-acp-server.ts"
import type { GrokAcpClient } from "./acp-client.ts"

/**
 * In-process ACP client for fixture tests (no subprocess, no real grok).
 */
export function createInProcessMockAcpClient(
  options: MockAcpServerOptions = {},
): GrokAcpClient {
  const pair = createMockAcpStdioPair(options)
  let nextId = 1
  let updateHandler: ((update: AcpSessionUpdate, sessionId?: string) => void) | null =
    null
  let dead = false

  function pump() {
    for (const line of pair.drainServerLines()) {
      let message: {
        id?: number
        method?: string
        params?: Record<string, unknown>
        result?: Record<string, unknown>
        error?: { message?: string }
      }
      try {
        message = JSON.parse(line) as typeof message
      } catch {
        continue
      }
      if (message.method === "session/update") {
        const params = message.params ?? {}
        const update = (params.update ?? params) as AcpSessionUpdate
        const sessionId =
          typeof params.sessionId === "string" ? params.sessionId : undefined
        updateHandler?.(update, sessionId)
      }
      // request() handles responses via sequential pump after write
      void message
    }
  }

  return {
    pid: undefined,
    onSessionUpdate(handler) {
      updateHandler = handler
    },
    request(method, params = {}, _timeoutMs = 60_000) {
      if (dead) return Promise.reject(new Error("mock acp client disposed"))
      const id = nextId++
      pair.pushClientLine(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }),
      )

      // Collect responses; session/update may interleave before result
      let result: Record<string, unknown> | null = null
      let error: Error | null = null

      for (const line of pair.drainServerLines()) {
        let message: {
          id?: number
          method?: string
          params?: Record<string, unknown>
          result?: Record<string, unknown>
          error?: { message?: string }
        }
        try {
          message = JSON.parse(line) as typeof message
        } catch {
          continue
        }
        if (message.method === "session/update") {
          const p = message.params ?? {}
          const update = (p.update ?? p) as AcpSessionUpdate
          const sessionId =
            typeof p.sessionId === "string" ? p.sessionId : undefined
          updateHandler?.(update, sessionId)
          continue
        }
        if (message.id === id) {
          if (message.error) {
            error = new Error(message.error.message ?? "error")
          } else {
            result = message.result ?? {}
          }
        }
      }

      if (error) return Promise.reject(error)
      if (result) return Promise.resolve(result)
      pump()
      return Promise.reject(new Error(`no response for ${method}`))
    },
    kill() {
      dead = true
    },
  }
}
