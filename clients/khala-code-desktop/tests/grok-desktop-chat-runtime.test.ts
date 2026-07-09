import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createInProcessMockAcpClient, createGrokAcpChatRuntime, createGrokSessionStore } from "@openagentsinc/grok-harness"

import { createGrokDesktopChatRuntime } from "../src/bun/grok-desktop-chat-runtime.js"

describe("GrokDesktopChatRuntime (mock ACP)", () => {
  test("startThread + startTurn returns assistant message via desktop shape", async () => {
    const dir = await mkdtemp(join(tmpdir(), "grok-desk-"))
    try {
      const acpRuntime = await createGrokAcpChatRuntime({
        clientFactory: () =>
          createInProcessMockAcpClient({
            replyText: "desktop-fixture-ok",
            chunkSize: 6,
          }),
        sessionStore: createGrokSessionStore({ path: join(dir, "s.json") }),
      })

      const runtime = await createGrokDesktopChatRuntime({
        workingDirectory: dir,
        acpRuntime,
        sessionStore: createGrokSessionStore({ path: join(dir, "s.json") }),
      })

      const thread = await runtime.startThread({ sessionId: "desk-1", cwd: dir })
      expect(thread.ok).toBe(true)
      expect(thread.threadId.length).toBeGreaterThan(0)

      const events: string[] = []
      const withEvents = await createGrokDesktopChatRuntime({
        workingDirectory: dir,
        acpRuntime,
        sessionStore: createGrokSessionStore({ path: join(dir, "s.json") }),
        onEvent: (e) => events.push(e.type),
      })

      const turn = await withEvents.startTurn({
        sessionId: "desk-1",
        messages: [{ id: "u1", role: "user", body: "hi" }],
        cwd: dir,
      })

      expect(turn.ok).toBe(true)
      expect(turn.backend.kind).toBe("grok_acp")
      expect(turn.backend.runtimeMode).toBe("grok_runtime")
      expect(turn.messages[0]?.body).toBe("desktop-fixture-ok")
      expect(events).toContain("message_delta")
    } finally {
      await rm(dir, { recursive: true, force: true })
    }
  })
})
