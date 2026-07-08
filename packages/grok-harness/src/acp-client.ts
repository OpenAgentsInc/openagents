import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { createInterface, type Interface } from "node:readline"

import type { AcpSessionUpdate } from "./event-projector.ts"

type Pending = {
  resolve: (value: Record<string, unknown>) => void
  reject: (error: Error) => void
  timer: ReturnType<typeof setTimeout>
}

export type GrokAcpClient = {
  readonly request: (
    method: string,
    params?: Record<string, unknown>,
    timeoutMs?: number,
  ) => Promise<Record<string, unknown>>
  readonly onSessionUpdate: (handler: (update: AcpSessionUpdate, sessionId?: string) => void) => void
  readonly kill: () => void
  readonly pid: number | undefined
}

export type CreateGrokAcpClientOptions = {
  /** Command argv. Default: real grok agent stdio. Tests pass mock. */
  readonly command?: readonly string[]
  readonly env?: NodeJS.ProcessEnv
  readonly cwd?: string
}

export function createGrokAcpClient(
  options: CreateGrokAcpClientOptions = {},
): GrokAcpClient {
  const command = options.command ?? ["grok", "agent", "stdio"]
  const [bin, ...args] = command
  if (!bin) throw new Error("createGrokAcpClient: empty command")

  const proc: ChildProcessWithoutNullStreams = spawn(bin, args, {
    stdio: ["pipe", "pipe", "pipe"],
    env: options.env ?? process.env,
    cwd: options.cwd,
  })

  const pending = new Map<number, Pending>()
  let nextId = 1
  let updateHandler: ((update: AcpSessionUpdate, sessionId?: string) => void) | null =
    null

  const rl: Interface = createInterface({ input: proc.stdout })

  proc.stderr.on("data", () => {
    // Intentionally quiet in fixtures; live callers can attach debug.
  })

  rl.on("line", (line) => {
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
      return
    }

    if (message.method === "session/update") {
      const params = message.params ?? {}
      const update = (params.update ?? params) as AcpSessionUpdate
      const sessionId =
        typeof params.sessionId === "string" ? params.sessionId : undefined
      updateHandler?.(update, sessionId)
      return
    }

    if (message.id === undefined) return
    const p = pending.get(message.id)
    if (!p) return
    pending.delete(message.id)
    clearTimeout(p.timer)
    if (message.error) {
      p.reject(new Error(message.error.message ?? JSON.stringify(message.error)))
    } else {
      p.resolve(message.result ?? {})
    }
  })

  proc.on("exit", (code) => {
    for (const [id, p] of pending) {
      pending.delete(id)
      clearTimeout(p.timer)
      p.reject(new Error(`grok acp process exited (${code})`))
    }
  })

  return {
    pid: proc.pid,
    onSessionUpdate(handler) {
      updateHandler = handler
    },
    request(method, params = {}, timeoutMs = 60_000) {
      const id = nextId++
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          pending.delete(id)
          reject(new Error(`${method} timed out after ${timeoutMs}ms`))
        }, timeoutMs)
        pending.set(id, { resolve, reject, timer })
        proc.stdin.write(
          `${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`,
        )
      })
    },
    kill() {
      rl.close()
      proc.kill()
    },
  }
}

export async function initializeAndAuth(
  client: GrokAcpClient,
  options: { readonly preferApiKey?: boolean } = {},
): Promise<{ readonly methodId: string }> {
  const init = await client.request("initialize", {
    protocolVersion: 1,
    clientCapabilities: {
      fs: { readTextFile: true, writeTextFile: true },
      terminal: true,
    },
  })

  const methods = new Set(
    ((init.authMethods as { id?: string }[] | undefined) ?? [])
      .map((m) => m.id)
      .filter((id): id is string => typeof id === "string"),
  )

  const hasApiKey = Boolean(process.env.XAI_API_KEY?.trim())
  let methodId: string | null = null
  if (options.preferApiKey && hasApiKey && methods.has("xai.api_key")) {
    methodId = "xai.api_key"
  } else if (hasApiKey && methods.has("xai.api_key") && !methods.has("cached_token")) {
    methodId = "xai.api_key"
  } else if (methods.has("cached_token")) {
    methodId = "cached_token"
  } else if (hasApiKey && methods.has("xai.api_key")) {
    methodId = "xai.api_key"
  }

  if (!methodId) {
    throw new Error(
      "Grok ACP auth unavailable: run `grok login` or set XAI_API_KEY",
    )
  }

  await client.request("authenticate", {
    methodId,
    _meta: { headless: true },
  })

  return { methodId }
}
