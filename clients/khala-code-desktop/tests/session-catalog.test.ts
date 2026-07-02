import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import type { ClaudeAppSdkChatRuntime } from "../src/bun/claude-app-sdk-chat-runtime"
import type { CodexAppServerChatRuntime } from "../src/bun/codex-app-server-chat-runtime"
import { readKhalaCodeDesktopSessionCatalog } from "../src/bun/session-catalog"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

const tempRoot = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), "khala-code-session-catalog-"))
  tempDirs.push(root)
  return root
}

describe("Khala Code cross-harness session catalog", () => {
  test("merges Codex and Claude local stores with runtime-reported exact totals", async () => {
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    const claudeStatePath = join(root, "claude-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {
        "desktop-codex": {
          threadId: "codex-thread-1",
          lastCodexTurnId: "codex-turn-1",
          updatedAt: "2026-07-01T10:00:00.000Z",
        },
      },
    }))
    await writeFile(claudeStatePath, JSON.stringify({
      schema: "khala-code-desktop.claude-sessions.v1",
      sessions: {
        "desktop-claude": {
          sessionId: "claude-session-1",
          lastTurnId: "claude-turn-1",
          updatedAt: "2026-07-01T10:05:00.000Z",
        },
      },
    }))

    const codexRuntime = {
      listThreads: async () => ({
        ok: true as const,
        data: [{
          id: "codex-thread-1",
          sessionId: "codex-session-1",
          title: "Codex repair",
          preview: "Fixed the fixture",
          cwd: "/repo",
          updatedAt: 1782910000000,
          totalTokens: 120,
          inputTokens: 70,
          outputTokens: 50,
        }],
        threads: [],
      }),
    } as Partial<CodexAppServerChatRuntime> as CodexAppServerChatRuntime
    const claudeRuntime = {
      listThreads: async () => ({
        ok: true as const,
        data: [{
          id: "claude-session-1",
          title: "Claude plan",
          preview: "Plan and review",
          cwd: "/repo",
          updatedAt: 1782910100000,
          total_tokens: 80,
        }],
        threads: [],
      }),
    } as Partial<ClaudeAppSdkChatRuntime> as ClaudeAppSdkChatRuntime

    const catalog = await readKhalaCodeDesktopSessionCatalog({}, {
      claudeRuntime,
      codexRuntime,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: claudeStatePath,
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })

    expect(catalog.schemaVersion).toBe("khala-code-desktop.session-catalog.v1")
    expect(catalog.diagnostics).toEqual([])
    expect(catalog.entries.map(entry => [entry.harnessKind, entry.threadRef, entry.desktopSessionRef])).toEqual([
      ["claude", "claude-session-1", "desktop-claude"],
      ["codex", "codex-thread-1", "desktop-codex"],
    ])
    expect(catalog.entries.find(entry => entry.harnessKind === "codex")?.exactTotals).toMatchObject({
      totalTokens: 120,
      inputTokens: 70,
      outputTokens: 50,
      source: "codex_app_server_thread_list",
    })
    expect(catalog.entries.find(entry => entry.harnessKind === "claude")?.exactTotals).toMatchObject({
      totalTokens: 80,
      source: "claude_sdk_list_sessions",
    })
  })
})
