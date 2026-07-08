import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createGrokAcpChatRuntime } from "./chat-runtime.ts"
import { createInProcessMockAcpClient } from "./in-process-acp-client.ts"
import { createGrokSessionStore } from "./session-store.ts"
import type { NeutralChatTurnEvent } from "./types.ts"

describe("GrokAcpChatRuntime (mock ACP fixture)", () => {
  test("startThread + startTurn emits neutral events and full text", async () => {
    const dir = await mkdtemp(join(tmpdir(), "grok-sess-"))
    try {
      const runtime = await createGrokAcpChatRuntime({
        clientFactory: () =>
          createInProcessMockAcpClient({
            replyText: "fixture-hello",
            chunkSize: 4,
          }),
        sessionStore: createGrokSessionStore({
          path: join(dir, "sessions.json"),
        }),
      })

      const thread = await runtime.startThread({ cwd: dir })
      expect(thread.grokSessionId.length).toBeGreaterThan(0)

      const events: NeutralChatTurnEvent[] = []
      const turn = await runtime.startTurn({
        threadId: thread.threadId,
        desktopSessionId: thread.desktopSessionId,
        grokSessionId: thread.grokSessionId,
        prompt: "Say hi",
        onEvent: (e) => events.push(e),
      })

      expect(turn.text).toBe("fixture-hello")
      expect(turn.stopReason).toBe("end_turn")
      expect(events.some((e) => e.type === "thread_ready")).toBe(true)
      expect(events.some((e) => e.type === "message_start")).toBe(true)
      expect(events.some((e) => e.type === "message_delta")).toBe(true)
      expect(events.some((e) => e.type === "message_done")).toBe(true)

      runtime.dispose()
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
})
