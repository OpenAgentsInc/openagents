import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"
import { afterEach, describe, expect, test } from "bun:test"

import {
  createCodexAppServerChatRuntime,
} from "../src/bun/codex-app-server-chat-runtime"
import {
  createCodexAppServerHost,
  type CodexAppServerNotification,
} from "../src/bun/codex-app-server-client"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc"

const fixtureAppServerPath = fileURLToPath(
  new URL("../src/bun/fixture-codex-app-server.ts", import.meta.url),
)

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempRoot(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-fixture-app-server-"))
  tempDirs.push(root)
  return root
}

async function waitFor(
  predicate: () => boolean,
  label: string,
): Promise<void> {
  for (let index = 0; index < 200; index += 1) {
    if (predicate()) return
    await Bun.sleep(5)
  }
  throw new Error(`Timed out waiting for ${label}`)
}

describe("fixture Codex app-server process", () => {
  test("spawns over stdio and streams approvals plus terminal output through the real runtime", async () => {
    const root = await tempRoot()
    const notifications: CodexAppServerNotification[] = []
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const host = createCodexAppServerHost({
      codexArgs: [fixtureAppServerPath, "--stdio"],
      codexCommand: process.execPath,
      env: {
        CODEX_HOME: join(root, "codex-home"),
      } as NodeJS.ProcessEnv,
      initializeTimeoutMs: 1_000,
      requestTimeoutMs: 1_000,
    })
    host.subscribe(notification => {
      notifications.push(notification)
      if (
        notification.method === "item/commandExecution/requestApproval" &&
        notification.id !== undefined
      ) {
        queueMicrotask(() => {
          host.respondToServerRequest(notification.id!, { decision: "accept" })
        })
      }
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      onEvent: event => events.push(event),
      statePath: join(root, "codex-sessions.json"),
      turnTimeoutMs: 2_000,
      workingDirectory: root,
    })

    try {
      const response = await runtime.startTurn({
        messages: [{ body: "exercise the fixture app-server", id: "user-fixture", role: "user" }],
        sessionId: "desktop-session-fixture",
        turnId: "desktop-turn-fixture",
      })

      await waitFor(
        () => notifications.some(notification => notification.method === "turn/completed"),
        "fixture turn completion",
      )

      expect(response).toMatchObject({
        backend: {
          kind: "codex_app_server",
          model: "gpt-5.1-codex-fixture",
          threadId: "fixture-thread-1",
          turnId: "fixture-turn-1",
          turnStatus: "completed",
        },
        ok: true,
        usage: {
          cachedInput: 2,
          input: 11,
          output: 7,
          reasoningOutput: 3,
        },
      })
      expect(notifications.map(notification => notification.method)).toEqual([
        "turn/started",
        "item/commandExecution/requestApproval",
        "serverRequest/resolved",
        "item/started",
        "item/commandExecution/outputDelta",
        "item/completed",
        "item/started",
        "item/agentMessage/delta",
        "item/completed",
        "thread/tokenUsage/updated",
        "turn/completed",
      ])
      expect(events.map(event => event.type)).toContain("message_delta")
      expect(response.messages.some(message =>
        message.codexItem?.itemType === "approval" &&
        message.codexItem.status === "completed" &&
        message.body.includes("Approval resolved.")
      )).toBe(true)
      expect(response.messages.some(message =>
        message.codexItem?.itemType === "commandExecution" &&
        message.body.includes("fixture terminal: booted")
      )).toBe(true)
      expect(response.messages.some(message =>
        message.role === "assistant" &&
        message.body === "Fixture app-server completed deterministically."
      )).toBe(true)

      const read = await runtime.readThread({
        includeTurns: true,
        threadId: "fixture-thread-1",
      })
      expect(read.messages?.map(message => message.id)).toContain("item-command-fixture-turn-1")
      expect(read.messages?.map(message => message.id)).toContain("item-agent-fixture-turn-1")
    } finally {
      host.dispose()
    }
  })

  test("can be selected through the host fixture environment switch", async () => {
    const root = await tempRoot()
    const host = createCodexAppServerHost({
      env: {
        CODEX_HOME: join(root, "codex-home"),
        KHALA_CODE_BUN_BINARY: process.execPath,
        KHALA_CODE_CODEX_APP_SERVER_FIXTURE: "1",
        KHALA_CODE_CODEX_APP_SERVER_FIXTURE_PATH: fixtureAppServerPath,
      } as NodeJS.ProcessEnv,
      initializeTimeoutMs: 1_000,
      requestTimeoutMs: 1_000,
    })

    try {
      const start = await host.start()

      expect(start.ok).toBe(true)
      expect(start.status.initializeResult).toMatchObject({
        fixtureScript: "default-live-shaped-turn",
        serverInfo: {
          name: "khala-code-fixture-codex-app-server",
        },
      })
    } finally {
      host.dispose()
    }
  })
})
