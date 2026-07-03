import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import type { ClaudeAppSdkChatRuntime } from "../src/bun/claude-app-sdk-chat-runtime"
import type { CodexAppServerChatRuntime } from "../src/bun/codex-app-server-chat-runtime"
import { readKhalaCodeDesktopSessionCatalog } from "../src/bun/session-catalog"
import { sessionCatalogEntryToThreadSummary } from "../src/shared/session-catalog"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

const tempRoot = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), "khala-code-session-catalog-"))
  tempDirs.push(root)
  return root
}

const delay = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

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
    expect(catalog.scope).toBe("app")
    expect(catalog.diagnostics).toEqual([])
    expect(catalog.entries.map(entry => [entry.harnessKind, entry.threadRef, entry.desktopSessionRef])).toEqual([
      ["claude", "claude-session-1", "desktop-claude"],
      ["codex", "codex-thread-1", "desktop-codex"],
    ])
    expect(catalog.entries.map(entry => entry.updatedAt)).toEqual([
      1782910100,
      1782910000,
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

  test("labels Codex-shaped legacy records from the Claude store as Codex", async () => {
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    const claudeStatePath = join(root, "claude-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {},
    }))
    await writeFile(claudeStatePath, JSON.stringify({
      schema: "khala-code-desktop.claude-sessions.v1",
      sessions: {
        "legacy-codex-desktop": {
          threadId: "legacy-codex-thread",
          lastCodexTurnId: "legacy-codex-turn",
          updatedAt: "2026-07-01T11:00:00.000Z",
        },
        "legacy-thread-only-desktop": {
          threadId: "legacy-thread-only",
          updatedAt: "2026-07-01T10:30:00.000Z",
        },
        "real-claude-desktop": {
          sessionId: "real-claude-session",
          lastTurnId: "real-claude-turn",
          updatedAt: "2026-07-01T10:00:00.000Z",
        },
      },
    }))

    const catalog = await readKhalaCodeDesktopSessionCatalog({}, {
      codexRuntime: null,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: claudeStatePath,
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })

    expect(catalog.entries.map(entry => [entry.harnessKind, entry.threadRef, entry.desktopSessionRef])).toEqual([
      ["codex", "legacy-codex-thread", "legacy-codex-desktop"],
      ["codex", "legacy-thread-only", "legacy-thread-only-desktop"],
      ["claude", "real-claude-session", "real-claude-desktop"],
    ])
    expect(catalog.entries.map(entry => entry.statusLabel)).toEqual([
      "Codex session",
      "Codex session",
      "Claude session",
    ])
    expect(catalog.entries.map(entry => entry.updatedAt)).toEqual([
      Date.parse("2026-07-01T11:00:00.000Z") / 1000,
      Date.parse("2026-07-01T10:30:00.000Z") / 1000,
      Date.parse("2026-07-01T10:00:00.000Z") / 1000,
    ])
  })

  test("uses Codex thread ids for sidebar resume even when session ids are UUIDs", async () => {
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {
        "desktop-codex": {
          threadId: "id-recent-chat-row",
          updatedAt: "2026-07-01T10:00:00.000Z",
        },
      },
    }))
    const codexRuntime = {
      listThreads: async () => ({
        ok: true as const,
        data: [{
          id: "id-recent-chat-row",
          sessionId: "018f1d59-1a9f-7c40-b4d1-7b0706c531ad",
          title: "Most recent chat",
          preview: "Resume me",
          createdAt: "2026-07-01T10:00:00.000Z",
          updatedAt: 1782910000000,
        }],
        threads: [],
      }),
    } as Partial<CodexAppServerChatRuntime> as CodexAppServerChatRuntime

    const catalog = await readKhalaCodeDesktopSessionCatalog({}, {
      codexRuntime,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: join(await tempRoot(), "missing-claude.json"),
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })

    expect(catalog.entries).toHaveLength(1)
    expect(catalog.entries[0]).toMatchObject({
      harnessKind: "codex",
      sessionRef: "018f1d59-1a9f-7c40-b4d1-7b0706c531ad",
      threadRef: "id-recent-chat-row",
      createdAt: Date.parse("2026-07-01T10:00:00.000Z") / 1000,
      updatedAt: 1782910000,
      recencyAt: 1782910000,
    })
    expect(sessionCatalogEntryToThreadSummary(catalog.entries[0]!).id)
      .toBe("id-recent-chat-row")
  })

  test("does not surface Codex missing-rollout diagnostics as session previews", async () => {
    const codexRuntime = {
      listThreads: async () => ({
        ok: true as const,
        data: [
          {
            id: "thread-stale-rollout",
            title: "Khala Claude live smoke",
            preview: "no rollout found for thread id thread-stale-rollout",
            updatedAt: "2026-07-01T10:00:00.000Z",
          },
          {
            id: "thread-stale-title",
            title: "thread not found",
            preview: "thread not found",
            updatedAt: "2026-07-01T09:00:00.000Z",
          },
        ],
        threads: [],
      }),
    } as Partial<CodexAppServerChatRuntime> as CodexAppServerChatRuntime

    const catalog = await readKhalaCodeDesktopSessionCatalog({ scope: "all_home" }, {
      codexRuntime,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: join(await tempRoot(), "missing-claude.json"),
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: join(await tempRoot(), "missing-codex.json"),
      },
    })

    expect(catalog.entries.map(entry => ({
      title: entry.title,
      preview: entry.preview,
    }))).toEqual([
      {
        title: "Khala Claude live smoke",
        preview: "",
      },
      {
        title: "Codex session",
        preview: "",
      },
    ])
    expect(catalog.entries.map(sessionCatalogEntryToThreadSummary).map(thread => thread.preview))
      .toEqual(["", ""])
    expect(JSON.stringify(catalog)).not.toContain("no rollout found")
    expect(JSON.stringify(catalog)).not.toContain("thread not found")
  })

  test("defaults to app-owned history and hides unrelated home runtime sessions", async () => {
    // Oracle for khala_code.history.app_sessions_default.v1
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {
        "desktop-codex": {
          threadId: "app-thread",
          updatedAt: "2026-07-01T10:00:00.000Z",
        },
      },
    }))
    const codexRuntime = {
      listThreads: async () => ({
        ok: true as const,
        data: [
          {
            id: "app-thread",
            sessionId: "app-session",
            title: "Desktop chat",
            preview: "created here",
            updatedAt: "2026-07-01T11:00:00.000Z",
          },
          {
            id: "headless-thread",
            sessionId: "headless-session",
            title: "You are the Orrery memory-distiller",
            preview: "headless automation",
            updatedAt: "2026-07-01T12:00:00.000Z",
          },
        ],
        threads: [],
      }),
    } as Partial<CodexAppServerChatRuntime> as CodexAppServerChatRuntime

    const catalog = await readKhalaCodeDesktopSessionCatalog({}, {
      codexRuntime,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: join(root, "missing-claude.json"),
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })

    expect(catalog.scope).toBe("app")
    expect(catalog.entries.map(entry => entry.threadRef)).toEqual(["app-thread"])
    expect(JSON.stringify(catalog)).not.toContain("Orrery")
    expect(JSON.stringify(catalog)).not.toContain("headless-thread")
  })

  test("can explicitly include all home runtime sessions", async () => {
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {
        "desktop-codex": {
          threadId: "app-thread",
          updatedAt: "2026-07-01T10:00:00.000Z",
        },
      },
    }))
    const codexRuntime = {
      listThreads: async () => ({
        ok: true as const,
        data: [
          { id: "app-thread", title: "Desktop chat", updatedAt: "2026-07-01T11:00:00.000Z" },
          { id: "headless-thread", title: "Headless task", updatedAt: "2026-07-01T12:00:00.000Z" },
        ],
        threads: [],
      }),
    } as Partial<CodexAppServerChatRuntime> as CodexAppServerChatRuntime

    const catalog = await readKhalaCodeDesktopSessionCatalog({ scope: "all_home" }, {
      codexRuntime,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: join(root, "missing-claude.json"),
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })

    expect(catalog.scope).toBe("all_home")
    expect(catalog.entries.map(entry => entry.threadRef)).toEqual([
      "headless-thread",
      "app-thread",
    ])
  })

  test("keeps legacy non-UUID Codex thread ids when no UUID session ref exists", async () => {
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {
        "desktop-codex": {
          threadId: "thread-history",
          updatedAt: "2026-07-01T10:00:00.000Z",
        },
      },
    }))

    const nextCatalog = await readKhalaCodeDesktopSessionCatalog({}, {
      codexRuntime: null,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: join(root, "missing-claude.json"),
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })

    expect(sessionCatalogEntryToThreadSummary(nextCatalog.entries[0]!).id).toBe("thread-history")
  })

  test("queries Codex and Claude thread sources concurrently", async () => {
    const root = await tempRoot()
    const codexStatePath = join(root, "codex-sessions.json")
    const claudeStatePath = join(root, "claude-sessions.json")
    await writeFile(codexStatePath, JSON.stringify({
      schema: "khala-code-desktop.codex-sessions.v1",
      sessions: {},
    }))
    await writeFile(claudeStatePath, JSON.stringify({
      schema: "khala-code-desktop.claude-sessions.v1",
      sessions: {},
    }))

    const codexStartedAt: number[] = []
    const claudeStartedAt: number[] = []
    const codexRuntime = {
      listThreads: async () => {
        codexStartedAt.push(performance.now())
        await delay(40)
        return { ok: true as const, data: [], threads: [] }
      },
    } as Partial<CodexAppServerChatRuntime> as CodexAppServerChatRuntime
    const claudeRuntime = {
      listThreads: async () => {
        claudeStartedAt.push(performance.now())
        await delay(40)
        return { ok: true as const, data: [], threads: [] }
      },
    } as Partial<ClaudeAppSdkChatRuntime> as ClaudeAppSdkChatRuntime

    const started = performance.now()
    const catalog = await readKhalaCodeDesktopSessionCatalog({}, {
      claudeRuntime,
      codexRuntime,
      env: {
        KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH: claudeStatePath,
        KHALA_CODE_DESKTOP_CODEX_STATE_PATH: codexStatePath,
      },
    })
    const elapsed = performance.now() - started

    expect(catalog.ok).toBe(true)
    expect(codexStartedAt).toHaveLength(1)
    expect(claudeStartedAt).toHaveLength(1)
    expect(Math.abs(codexStartedAt[0]! - claudeStartedAt[0]!)).toBeLessThan(20)
    expect(elapsed).toBeLessThan(75)
  })
})
