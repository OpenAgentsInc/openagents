import { randomUUID } from "node:crypto"

import {
  createGrokAcpClient,
  initializeAndAuth,
  type CreateGrokAcpClientOptions,
  type GrokAcpClient,
} from "./acp-client.ts"
import { createGrokAcpEventProjector } from "./event-projector.ts"
import {
  createGrokSessionStore,
  type GrokSessionStore,
} from "./session-store.ts"
import type { NeutralChatTurnEvent } from "./types.ts"

export type GrokAcpChatRuntime = {
  readonly startThread: (input?: {
    readonly desktopSessionId?: string
    readonly cwd?: string
  }) => Promise<{
    readonly threadId: string
    readonly desktopSessionId: string
    readonly grokSessionId: string
  }>
  readonly startTurn: (input: {
    readonly threadId: string
    readonly desktopSessionId: string
    readonly grokSessionId: string
    readonly prompt: string
    readonly onEvent?: (event: NeutralChatTurnEvent) => void
  }) => Promise<{
    readonly turnId: string
    readonly text: string
    readonly stopReason: string
  }>
  readonly interruptTurn: () => Promise<void>
  readonly dispose: () => void
}

export type CreateGrokAcpChatRuntimeOptions = {
  readonly acp?: CreateGrokAcpClientOptions
  readonly sessionStore?: GrokSessionStore
  /** Inject a pre-built client (tests). */
  readonly clientFactory?: () => GrokAcpClient
}

export async function createGrokAcpChatRuntime(
  options: CreateGrokAcpChatRuntimeOptions = {},
): Promise<GrokAcpChatRuntime> {
  const client = options.clientFactory
    ? options.clientFactory()
    : createGrokAcpClient(options.acp)
  const store = options.sessionStore ?? createGrokSessionStore()

  await initializeAndAuth(client)

  let activeClient = client
  let interrupted = false

  return {
    async startThread(input = {}) {
      const desktopSessionId = input.desktopSessionId ?? randomUUID()
      const result = await activeClient.request("session/new", {
        cwd: input.cwd ?? process.cwd(),
        mcpServers: [],
      })
      const grokSessionId = String(result.sessionId ?? "")
      if (!grokSessionId) throw new Error("session/new missing sessionId")

      await store.put({
        desktopSessionId,
        grokSessionId,
        updatedAt: new Date().toISOString(),
        capabilities: { resume: false, fork: false },
      })

      return {
        threadId: grokSessionId,
        desktopSessionId,
        grokSessionId,
      }
    },

    async startTurn(input) {
      interrupted = false
      const turnId = randomUUID()
      const projector = createGrokAcpEventProjector({
        threadId: input.threadId,
        turnId,
      })

      input.onEvent?.({
        type: "thread_ready",
        threadId: input.threadId,
        turnId,
      })

      activeClient.onSessionUpdate((update) => {
        if (interrupted) return
        for (const event of projector.onUpdate(update)) {
          input.onEvent?.(event)
        }
      })

      const promptResult = await activeClient.request(
        "session/prompt",
        {
          sessionId: input.grokSessionId,
          prompt: [{ type: "text", text: input.prompt }],
        },
        120_000,
      )

      for (const event of projector.finish()) {
        input.onEvent?.(event)
      }

      await store.put({
        desktopSessionId: input.desktopSessionId,
        grokSessionId: input.grokSessionId,
        lastTurnId: turnId,
        updatedAt: new Date().toISOString(),
        capabilities: { resume: false, fork: false },
      })

      return {
        turnId,
        text: projector.text(),
        stopReason: String(promptResult.stopReason ?? "end_turn"),
      }
    },

    async interruptTurn() {
      interrupted = true
      // ACP cancel is not uniformly available; kill is the hard interrupt.
      activeClient.kill()
      activeClient = options.clientFactory
        ? options.clientFactory()
        : createGrokAcpClient(options.acp)
      await initializeAndAuth(activeClient)
    },

    dispose() {
      activeClient.kill()
    },
  }
}
