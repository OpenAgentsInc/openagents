import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "bun:test"

import {
  createCodexAppServerChatRuntime,
} from "../src/bun/codex-app-server-chat-runtime"
import type {
  CodexAppServerHost,
  CodexAppServerNotification,
  CodexAppServerNotificationHandler,
} from "../src/bun/codex-app-server-client"
import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopCodexAppServerControlResult,
  KhalaCodeDesktopCodexAppServerStatus,
} from "../src/shared/rpc"

type RequestRecord = {
  readonly method: string
  readonly params: unknown
}

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

const runningStatus = (): KhalaCodeDesktopCodexAppServerStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  adapterVersion: "test-adapter",
  codexCommand: "codex",
  codexHome: "/tmp/codex-home",
  diagnostics: [],
  initialized: true,
  initializeResult: {},
  lastError: null,
  pendingRequestCount: 0,
  pid: 123,
  state: "running",
  transport: "stdio",
})

const startResult = (): KhalaCodeDesktopCodexAppServerControlResult => ({
  ok: true,
  action: "start",
  changed: false,
  status: runningStatus(),
})

function emit(
  subscribers: Set<CodexAppServerNotificationHandler>,
  notification: Omit<CodexAppServerNotification, "receivedAt">,
): void {
  const fullNotification = {
    ...notification,
    receivedAt: "2026-07-01T16:00:00.000Z",
  }
  for (const subscriber of subscribers) subscriber(fullNotification)
}

function createFakeHost(input: {
  readonly onRequest: (
    method: string,
    params: unknown,
    subscribers: Set<CodexAppServerNotificationHandler>,
  ) => unknown
  readonly records: RequestRecord[]
}): CodexAppServerHost {
  const subscribers = new Set<CodexAppServerNotificationHandler>()
  return {
    dispose: () => undefined,
    request: async <Result>(method: string, params?: unknown): Promise<Result> => {
      input.records.push({ method, params })
      return input.onRequest(method, params, subscribers) as Result
    },
    respondToServerRequest: () => undefined,
    restart: async () => ({ ...startResult(), action: "restart" }),
    start: async () => startResult(),
    status: () => runningStatus(),
    stop: async () => ({ ...startResult(), action: "stop" }),
    subscribe: handler => {
      subscribers.add(handler)
      return () => {
        subscribers.delete(handler)
      }
    },
  }
}

async function stateFixture(): Promise<{
  readonly root: string
  readonly statePath: string
}> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-codex-chat-"))
  tempDirs.push(root)
  return {
    root,
    statePath: join(root, "codex-sessions.json"),
  }
}

describe("Codex app-server chat runtime", () => {
  test("starts a Codex thread and streams a turn into desktop chat events", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const host = createFakeHost({
      records,
      onRequest: (method, _params, subscribers) => {
        if (method === "thread/start") {
          return {
            thread: { id: "thread-codex-1", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "turn/start") {
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/started",
              params: {
                threadId: "thread-codex-1",
                turn: { id: "turn-codex-1", status: "inProgress" },
              },
            })
            emit(subscribers, {
              method: "item/started",
              params: {
                threadId: "thread-codex-1",
                turnId: "turn-codex-1",
                item: { type: "agentMessage", id: "item-agent-1", text: "" },
              },
            })
            emit(subscribers, {
              method: "item/agentMessage/delta",
              params: {
                threadId: "thread-codex-1",
                turnId: "turn-codex-1",
                itemId: "item-agent-1",
                delta: "Hello",
              },
            })
            emit(subscribers, {
              method: "item/completed",
              params: {
                threadId: "thread-codex-1",
                turnId: "turn-codex-1",
                item: { type: "agentMessage", id: "item-agent-1", text: "Hello from Codex" },
              },
            })
            emit(subscribers, {
              method: "turn/completed",
              params: {
                threadId: "thread-codex-1",
                turn: { id: "turn-codex-1", status: "completed" },
              },
            })
          })
          return { turn: { id: "turn-codex-1", status: "inProgress" } }
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      onEvent: event => events.push(event),
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })

    const response = await runtime.startTurn({
      messages: [{ id: "user-1", role: "user", body: "Say hello" }],
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })

    expect(records.map(record => record.method)).toEqual(["thread/start", "turn/start"])
    expect(records[1]?.params).toMatchObject({
      threadId: "thread-codex-1",
      clientUserMessageId: "user-1",
      input: [{ type: "text", text: "Say hello", textElements: [] }],
    })
    expect(response).toMatchObject({
      backend: {
        kind: "codex_app_server",
        model: "gpt-5.1-codex",
        threadId: "thread-codex-1",
        turnId: "turn-codex-1",
        turnStatus: "completed",
      },
      messages: [{ id: "item-agent-1", role: "assistant", body: "Hello from Codex" }],
      ok: true,
    })
    expect(events.map(event => event.type)).toEqual([
      "message_start",
      "message_delta",
      "message_replace",
      "message_done",
    ])
    expect(await readFile(fixture.statePath, "utf8")).toContain("thread-codex-1")
  })

  test("starts turns on freshly created Codex threads without resuming an unmaterialized rollout", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    const host = createFakeHost({
      records,
      onRequest: (method, params, subscribers) => {
        if (method === "thread/start") {
          return {
            thread: { id: "thread-new-empty", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "thread/resume") {
          throw new Error("no rollout found for thread id thread-new-empty")
        }
        if (method === "turn/start") {
          expect(params).toMatchObject({ threadId: "thread-new-empty" })
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/completed",
              params: {
                threadId: "thread-new-empty",
                turn: { id: "turn-new-empty", status: "completed" },
              },
            })
          })
          return { turn: { id: "turn-new-empty", status: "inProgress" } }
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })

    const thread = await runtime.startThread({ sessionId: "desktop-session-new" })
    expect(thread.threadId).toBe("thread-new-empty")
    records.splice(0)

    const response = await runtime.startTurn({
      messages: [{ id: "user-new", role: "user", body: "Hello" }],
      sessionId: "desktop-session-new",
      threadId: thread.threadId,
      turnId: "desktop-turn-new",
    })

    expect(records.map(record => record.method)).toEqual(["turn/start"])
    expect(response.backend).toMatchObject({
      threadId: "thread-new-empty",
      turnId: "turn-new-empty",
      turnStatus: "completed",
    })
    expect(await readFile(fixture.statePath, "utf8")).toContain("thread-new-empty")
  })

  test("replaces stale unmaterialized thread ids with a fresh Codex thread for the turn", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    const host = createFakeHost({
      records,
      onRequest: (method, params, subscribers) => {
        if (method === "thread/resume") {
          expect(params).toMatchObject({ threadId: "thread-stale-empty" })
          throw new Error("thread/resume failed: no rollout found for thread id thread-stale-empty")
        }
        if (method === "thread/start") {
          return {
            thread: { id: "thread-replacement", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "turn/start") {
          expect(params).toMatchObject({ threadId: "thread-replacement" })
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/completed",
              params: {
                threadId: "thread-replacement",
                turn: { id: "turn-replacement", status: "completed" },
              },
            })
          })
          return { turn: { id: "turn-replacement", status: "inProgress" } }
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })

    const response = await runtime.startTurn({
      messages: [{ id: "user-stale", role: "user", body: "Hello after restart" }],
      sessionId: "desktop-session-stale",
      threadId: "thread-stale-empty",
      turnId: "desktop-turn-stale",
    })

    expect(records.map(record => record.method)).toEqual([
      "thread/resume",
      "thread/start",
      "turn/start",
    ])
    expect(response.backend).toMatchObject({
      threadId: "thread-replacement",
      turnId: "turn-replacement",
      turnStatus: "completed",
    })
    expect(await readFile(fixture.statePath, "utf8")).toContain("thread-replacement")
  })

  test("recovers when a loaded Codex thread loses its rollout before turn start", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    let replacementStarted = false
    const host = createFakeHost({
      records,
      onRequest: (method, params, subscribers) => {
        if (method === "thread/start") {
          if (!replacementStarted) {
            replacementStarted = true
            return {
              thread: { id: "thread-lost-rollout", status: "running" },
              model: "gpt-5.1-codex",
              modelProvider: "openai",
              cwd: fixture.root,
            }
          }
          return {
            thread: { id: "thread-recovered", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "thread/resume") {
          expect(params).toMatchObject({ threadId: "thread-lost-rollout" })
          throw new Error("thread/resume failed: no rollout found for thread id thread-lost-rollout")
        }
        if (method === "turn/start") {
          if ((params as { threadId?: string }).threadId === "thread-lost-rollout") {
            throw new Error("turn/start failed: no rollout found for thread id thread-lost-rollout")
          }
          expect(params).toMatchObject({ threadId: "thread-recovered" })
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/completed",
              params: {
                threadId: "thread-recovered",
                turn: { id: "turn-recovered", status: "completed" },
              },
            })
          })
          return { turn: { id: "turn-recovered", status: "inProgress" } }
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })

    const thread = await runtime.startThread({ sessionId: "desktop-session-recover" })
    expect(thread.threadId).toBe("thread-lost-rollout")
    records.splice(0)

    const response = await runtime.startTurn({
      messages: [{ id: "user-recover", role: "user", body: "Recover" }],
      sessionId: "desktop-session-recover",
      threadId: "thread-lost-rollout",
      turnId: "desktop-turn-recover",
    })

    expect(records.map(record => record.method)).toEqual([
      "turn/start",
      "thread/resume",
      "thread/start",
      "turn/start",
    ])
    expect(response.backend).toMatchObject({
      threadId: "thread-recovered",
      turnId: "turn-recovered",
      turnStatus: "completed",
    })
    expect(await readFile(fixture.statePath, "utf8")).toContain("thread-recovered")
  })

  test("resumes a persisted Codex thread for the same desktop session", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    let threadStarted = false
    const host = createFakeHost({
      records,
      onRequest: (method, _params, subscribers) => {
        if (method === "thread/start") {
          threadStarted = true
          return {
            thread: { id: "thread-persisted", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "thread/resume") {
          return {
            thread: { id: "thread-persisted", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "turn/start") {
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/completed",
              params: {
                threadId: "thread-persisted",
                turn: { id: "turn-codex-2", status: "completed" },
              },
            })
          })
          return { turn: { id: "turn-codex-2", status: "inProgress" } }
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const firstRuntime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })
    await firstRuntime.startTurn({
      messages: [{ id: "user-1", role: "user", body: "First" }],
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })
    expect(threadStarted).toBe(true)

    records.splice(0)
    const secondRuntime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })
    await secondRuntime.startTurn({
      messages: [{ id: "user-2", role: "user", body: "Second" }],
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-2",
    })

    expect(records.map(record => record.method)).toEqual(["thread/resume", "turn/start"])
  })

  test("lists, reads, renames, forks, archives, unarchives, and deletes Codex threads", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    const threadWithTurns = {
      id: "thread-history",
      sessionId: "thread-history",
      name: "History",
      preview: "Hello",
      cwd: fixture.root,
      modelProvider: "openai",
      source: "appServer",
      forkedFromId: null,
      parentThreadId: null,
      createdAt: 1,
      updatedAt: 2,
      recencyAt: 2,
      status: { type: "idle" },
      gitInfo: null,
      turns: [
        {
          id: "turn-history",
          status: "completed",
          items: [
            {
              id: "item-user-history",
              type: "userMessage",
              content: [{ type: "text", text: "Hello", textElements: [] }],
            },
            {
              id: "item-agent-history",
              type: "agentMessage",
              text: "Hi from stored Codex history",
            },
          ],
        },
      ],
    }
    const host = createFakeHost({
      records,
      onRequest: (method, params) => {
        if (method === "thread/list") {
          return {
            data: [threadWithTurns],
            nextCursor: null,
            backwardsCursor: null,
          }
        }
        if (method === "thread/read" || method === "thread/resume") {
          return {
            thread: threadWithTurns,
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "thread/name/set") return {}
        if (method === "thread/archive") return {}
        if (method === "thread/delete") return {}
        if (method === "thread/unarchive") return { thread: threadWithTurns }
        if (method === "thread/fork") {
          return {
            thread: {
              ...threadWithTurns,
              id: "thread-forked",
              sessionId: "thread-forked",
              forkedFromId: (params as { threadId?: string }).threadId ?? null,
            },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      workingDirectory: fixture.root,
    })

    const list = await runtime.listThreads({
      archived: false,
      limit: 20,
      searchTerm: "Hello",
      sessionId: "desktop-session-history",
      useStateDbOnly: true,
    })
    expect(list.threads).toEqual([expect.objectContaining({
      id: "thread-history",
      title: "History",
      projectLabel: fixture.root.split("/").at(-1),
      status: "idle",
    })])
    expect(list.groups?.[0]?.threadIds).toEqual(["thread-history"])

    const read = await runtime.readThread({
      threadId: "thread-history",
      includeTurns: true,
    })
    expect(read.messages?.map(message => [message.id, message.role, message.body])).toEqual([
      ["item-user-history", "user", "Hello"],
      ["item-agent-history", "assistant", "Hi from stored Codex history"],
    ])

    const resume = await runtime.resumeThread({
      sessionId: "desktop-session-history",
      threadId: "thread-history",
    })
    expect(resume.messages?.map(message => message.id)).toEqual([
      "item-user-history",
      "item-agent-history",
    ])
    expect(await runtime.threadIdForSession("desktop-session-history")).toBe("thread-history")

    await expect(runtime.renameThread({
      threadId: "thread-history",
      name: "Renamed",
    })).resolves.toMatchObject({ ok: true, action: "rename" })
    const fork = await runtime.forkThread({
      sessionId: "desktop-session-history",
      threadId: "thread-history",
    })
    expect(fork).toMatchObject({
      ok: true,
      action: "fork",
      threadId: "thread-history",
      newThreadId: "thread-forked",
    })
    expect(await runtime.threadIdForSession("desktop-session-history")).toBe("thread-forked")
    await expect(runtime.archiveThread({ threadId: "thread-history" }))
      .resolves.toMatchObject({ ok: true, action: "archive" })
    await expect(runtime.unarchiveThread({ threadId: "thread-history" }))
      .resolves.toMatchObject({ ok: true, action: "unarchive" })
    await expect(runtime.deleteThread({ threadId: "thread-history" }))
      .resolves.toMatchObject({ ok: true, action: "delete" })

    expect(records.map(record => record.method)).toEqual([
      "thread/list",
      "thread/read",
      "thread/resume",
      "thread/name/set",
      "thread/fork",
      "thread/archive",
      "thread/unarchive",
      "thread/delete",
    ])
    expect(records[0]?.params).toMatchObject({
      archived: false,
      limit: 20,
      searchTerm: "Hello",
      useStateDbOnly: true,
    })
    expect(records[1]?.params).toEqual({
      threadId: "thread-history",
      includeTurns: true,
    })
    expect(records[3]?.params).toEqual({
      threadId: "thread-history",
      name: "Renamed",
    })
  })

  test("interrupts the active Codex turn by desktop turn id", async () => {
    const fixture = await stateFixture()
    const records: RequestRecord[] = []
    const host = createFakeHost({
      records,
      onRequest: (method, _params, subscribers) => {
        if (method === "thread/start") {
          return {
            thread: { id: "thread-codex-1", status: "running" },
            model: "gpt-5.1-codex",
            modelProvider: "openai",
            cwd: fixture.root,
          }
        }
        if (method === "turn/start") {
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/started",
              params: {
                threadId: "thread-codex-1",
                turn: { id: "turn-codex-1", status: "inProgress" },
              },
            })
          })
          return { turn: { id: "turn-codex-1", status: "inProgress" } }
        }
        if (method === "turn/interrupt") {
          queueMicrotask(() => {
            emit(subscribers, {
              method: "turn/completed",
              params: {
                threadId: "thread-codex-1",
                turn: { id: "turn-codex-1", status: "interrupted" },
              },
            })
          })
          return {}
        }
        throw new Error(`unexpected request ${method}`)
      },
    })
    const runtime = createCodexAppServerChatRuntime({
      host,
      statePath: fixture.statePath,
      turnTimeoutMs: 1_000,
      workingDirectory: fixture.root,
    })
    const pendingTurn = runtime.startTurn({
      messages: [{ id: "user-1", role: "user", body: "Run for a while" }],
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })
    for (let index = 0; index < 50 && !records.some(record => record.method === "turn/start"); index += 1) {
      await Bun.sleep(1)
    }
    await Promise.resolve()

    await expect(runtime.interruptTurn({
      sessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })).resolves.toMatchObject({
      ok: true,
      codexTurnId: "turn-codex-1",
      threadId: "thread-codex-1",
    })
    await expect(pendingTurn).resolves.toMatchObject({
      backend: {
        turnStatus: "interrupted",
      },
    })
    expect(records.map(record => record.method)).toEqual([
      "thread/start",
      "turn/start",
      "turn/interrupt",
    ])
  })
})
