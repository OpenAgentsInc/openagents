/**
 * CUT-11 (#8691): typed Codex app-server source tests. A scripted in-memory
 * app-server (JSONL JSON-RPC over a PassThrough stdout, exactly the wire
 * protocol) proves the runner's handshake, the adapted `CodexRawEvent`
 * stream INCLUDING the `subAgentActivity` / receiver-bearing
 * `collabAgentToolCall` child records the exec encoder drops, exec-shaped
 * terminal usage, resume, fail-closed server-request refusal, and the typed
 * pre-frame failure the composed default runner uses for its single SDK
 * fallback. The final test folds the adapted stream through the REAL
 * one-conversation-service translation to prove Codex `agent.child.*`
 * events emerge end-to-end.
 */
import { describe, expect, test } from "vite-plus/test"
import { PassThrough } from "node:stream"

import {
  CodexAppServerPreFrameError,
  isCodexAppServerPreFrameFailure,
  makeCodexAppServerRunner,
  type CodexAppServerProcess,
  type CodexAppServerSpawn,
} from "./codex-app-server-source.js"
import {
  codexRawEventToRuntimeEvents,
  type CodexRawEvent,
} from "./runtime-intent-enforcement.js"

type JsonRecord = Record<string, unknown>

type FakeAppServer = Readonly<{
  spawn: CodexAppServerSpawn
  written: Array<JsonRecord>
  spawned: Array<{ executable: string; args: ReadonlyArray<string>; cwd: string }>
  wasKilled: () => boolean
}>

/**
 * Scripted app-server: answers the handshake requests and, after answering
 * `turn/start`, emits the given notification script one tick later (so the
 * runner's own `thread.started` frame is first, as in production).
 */
const makeFakeAppServer = (script: {
  notifications: ReadonlyArray<JsonRecord>
  failInitialize?: boolean
  serverRequestAfterTurnStart?: boolean
}): FakeAppServer => {
  const written: Array<JsonRecord> = []
  const spawned: Array<{ executable: string; args: ReadonlyArray<string>; cwd: string }> = []
  let killed = false
  const spawn: CodexAppServerSpawn = (input) => {
    spawned.push({ args: input.args, cwd: input.cwd, executable: input.executable })
    const stdout = new PassThrough()
    const send = (message: JsonRecord): void => {
      stdout.write(`${JSON.stringify(message)}\n`)
    }
    const handle = (message: JsonRecord): void => {
      const id = message.id
      const method = message.method
      if (method === "initialize" && typeof id === "number") {
        if (script.failInitialize === true) {
          send({ error: { code: -32000, message: "initialize refused by fixture" }, id })
          return
        }
        send({ id, result: { userAgent: "codex-fixture" } })
        return
      }
      if (method === "thread/start" && typeof id === "number") {
        send({ id, result: { thread: { id: "thread-as-1" } } })
        return
      }
      if (method === "thread/resume" && typeof id === "number") {
        const params = message.params as JsonRecord
        send({ id, result: { thread: { id: params.threadId } } })
        return
      }
      if (method === "turn/start" && typeof id === "number") {
        send({ id, result: { turn: { id: "turn-as-1", items: [], status: "inProgress" } } })
        setTimeout(() => {
          if (script.serverRequestAfterTurnStart === true) {
            send({ id: 9_001, method: "item/commandExecution/approval", params: {} })
          }
          for (const notification of script.notifications) send(notification)
        }, 1)
        return
      }
    }
    const child: CodexAppServerProcess = {
      kill: () => {
        killed = true
      },
      on: () => undefined,
      stdin: {
        write: (chunk: string) => {
          for (const line of chunk.split("\n")) {
            if (line.trim() === "") continue
            const message = JSON.parse(line) as JsonRecord
            written.push(message)
            handle(message)
          }
          return true
        },
      },
      stdout,
    }
    return child
  }
  return { spawn, spawned, wasKilled: () => killed, written }
}

const runnerInput = (overrides: Partial<Parameters<ReturnType<typeof makeCodexAppServerRunner>>[0]> = {}) => ({
  cwd: "/workspace/thread-1",
  env: { PATH: "/usr/bin" } as Record<string, string | undefined>,
  instructions: "run the fixture task",
  networkAccessEnabled: true,
  signal: new AbortController().signal,
  ...overrides,
})

const collectEvents = async (events: AsyncIterable<CodexRawEvent>): Promise<Array<CodexRawEvent>> => {
  const collected: Array<CodexRawEvent> = []
  for await (const event of events) collected.push(event)
  return collected
}

const CHILD_NOTIFICATIONS: ReadonlyArray<JsonRecord> = [
  { method: "turn/started", params: { threadId: "thread-as-1", turn: { id: "turn-as-1", items: [], status: "inProgress" } } },
  {
    method: "item/completed",
    params: {
      completedAtMs: 1,
      item: { agentPath: "root/child", agentThreadId: "child-thread-1", id: "sa1", kind: "started", type: "subAgentActivity" },
      threadId: "thread-as-1",
      turnId: "turn-as-1",
    },
  },
  {
    method: "item/completed",
    params: {
      completedAtMs: 2,
      item: {
        agentsStates: { "child-thread-1": { message: null, status: "completed" } },
        id: "ct1",
        receiverThreadIds: ["child-thread-1"],
        senderThreadId: "thread-as-1",
        status: "completed",
        tool: "wait",
        type: "collabAgentToolCall",
      },
      threadId: "thread-as-1",
      turnId: "turn-as-1",
    },
  },
  {
    method: "item/completed",
    params: {
      completedAtMs: 3,
      item: { id: "m1", text: "final answer", type: "agentMessage" },
      threadId: "thread-as-1",
      turnId: "turn-as-1",
    },
  },
  {
    method: "thread/tokenUsage/updated",
    params: {
      threadId: "thread-as-1",
      tokenUsage: {
        last: { cachedInputTokens: 7, inputTokens: 100, outputTokens: 25, reasoningOutputTokens: 5, totalTokens: 125 },
        modelContextWindow: null,
        total: { cachedInputTokens: 7, inputTokens: 100, outputTokens: 25, reasoningOutputTokens: 5, totalTokens: 125 },
      },
      turnId: "turn-as-1",
    },
  },
  {
    method: "turn/completed",
    params: {
      threadId: "thread-as-1",
      turn: { completedAt: 3, error: null, id: "turn-as-1", items: [], status: "completed" },
    },
  },
]

describe("makeCodexAppServerRunner", () => {
  test("adapts the typed app-server stream, preserving the child records the exec encoder drops", async () => {
    const fake = makeFakeAppServer({ notifications: CHILD_NOTIFICATIONS })
    const runner = makeCodexAppServerRunner({ spawnImpl: fake.spawn })
    const { events } = await runner(runnerInput({ imagePaths: ["/workspace/thread-1/pixel.png"] }))
    const collected = await collectEvents(events)

    expect(collected.map((event) => event.type)).toEqual([
      "thread.started",
      "turn.started",
      "item.completed",
      "item.completed",
      "item.completed",
      "turn.completed",
    ])
    expect(collected[0]!.thread_id).toBe("thread-as-1")
    const items = collected.filter((event) => event.type === "item.completed")
      .map((event) => (event.item as JsonRecord).type)
    expect(items).toEqual(["subAgentActivity", "collabAgentToolCall", "agentMessage"])
    // Exec-shaped terminal usage: reasoning split out of output.
    expect(collected[collected.length - 1]!.usage).toEqual({
      cached_input_tokens: 7,
      input_tokens: 100,
      output_tokens: 20,
      reasoning_output_tokens: 5,
    })
    // Handshake wire order: initialize -> initialized -> thread/start -> turn/start.
    expect(fake.written.map((message) => message.method)).toEqual([
      "initialize",
      "initialized",
      "thread/start",
      "turn/start",
    ])
    const turnStart = fake.written[3]!.params as JsonRecord
    expect(turnStart.approvalPolicy).toBe("never")
    expect(turnStart.sandboxPolicy).toEqual({ type: "dangerFullAccess" })
    expect(turnStart.input).toEqual([
      { text: "run the fixture task", type: "text" },
      { path: "/workspace/thread-1/pixel.png", type: "localImage" },
    ])
    expect(fake.spawned[0]!.args).toEqual(["app-server"])
    expect(fake.wasKilled()).toBe(true)
  })

  test("the adapted stream folds through the REAL translation into Codex agent.child events (end-to-end convergence)", async () => {
    const fake = makeFakeAppServer({ notifications: CHILD_NOTIFICATIONS })
    const runner = makeCodexAppServerRunner({ spawnImpl: fake.spawn })
    const { events } = await runner(runnerInput())
    let sequence = 0
    const ctx = {
      allocateSequence: () => {
        sequence += 1
        return sequence
      },
      childThreads: new Map(),
      nowIso: () => "2026-07-12T00:00:00.000Z",
      source: { adapterKind: "codex" as const, lane: "codex_app_server" as const, surface: "server" as const },
      threadId: "thread-1",
      turnId: "turn-1",
      turnStarted: { value: false },
    }
    const translated = []
    for await (const raw of events) {
      translated.push(...codexRawEventToRuntimeEvents(raw, ctx))
    }
    expect(translated.map((event) => event.kind)).toEqual([
      "turn.started",
      "agent.child.started",
      "agent.child.finished",
      "text.delta",
      "text.completed",
      "usage.recorded",
      "turn.finished",
    ])
  })

  test("resume drives thread/resume with the prior thread id", async () => {
    const fake = makeFakeAppServer({
      notifications: [
        { method: "turn/started", params: {} },
        { method: "turn/completed", params: { threadId: "thread-resumed", turn: { id: "t", items: [], status: "completed" } } },
      ],
    })
    const runner = makeCodexAppServerRunner({ spawnImpl: fake.spawn })
    const { events } = await runner(runnerInput({ resumeThreadId: "thread-resumed" }))
    const collected = await collectEvents(events)
    expect(collected[0]).toEqual({ thread_id: "thread-resumed", type: "thread.started" })
    expect(fake.written.map((message) => message.method)).toContain("thread/resume")
    expect(fake.written.map((message) => message.method)).not.toContain("thread/start")
  })

  test("a failed turn surfaces turn.failed with the typed error detail", async () => {
    const fake = makeFakeAppServer({
      notifications: [
        { method: "turn/started", params: {} },
        {
          method: "turn/completed",
          params: {
            threadId: "thread-as-1",
            turn: { error: { message: "usage limit reached" }, id: "t", items: [], status: "failed" },
          },
        },
      ],
    })
    const runner = makeCodexAppServerRunner({ spawnImpl: fake.spawn })
    const { events } = await runner(runnerInput())
    const collected = await collectEvents(events)
    const terminal = collected[collected.length - 1]!
    expect(terminal.type).toBe("turn.failed")
    expect(terminal.error).toBe("usage limit reached")
  })

  test("an unexpected server request is refused fail-closed with a JSON-RPC error", async () => {
    const fake = makeFakeAppServer({
      notifications: [
        { method: "turn/started", params: {} },
        { method: "turn/completed", params: { threadId: "thread-as-1", turn: { id: "t", items: [], status: "completed" } } },
      ],
      serverRequestAfterTurnStart: true,
    })
    const runner = makeCodexAppServerRunner({ spawnImpl: fake.spawn })
    const { events } = await runner(runnerInput())
    await collectEvents(events)
    const refusal = fake.written.find((message) => message.id === 9_001)
    expect(refusal).toBeDefined()
    expect((refusal!.error as JsonRecord).code).toBe(-32601)
  })

  test("a typed initialize failure is a pre-frame error (single-fallback contract)", async () => {
    const fake = makeFakeAppServer({ failInitialize: true, notifications: [] })
    const runner = makeCodexAppServerRunner({ spawnImpl: fake.spawn })
    let caught: unknown = null
    try {
      await runner(runnerInput())
    } catch (error) {
      caught = error
    }
    expect(caught).toBeInstanceOf(CodexAppServerPreFrameError)
    expect(isCodexAppServerPreFrameFailure(caught)).toBe(true)
    expect(fake.wasKilled()).toBe(true)
  })

  test("a spawn throw is a pre-frame error", async () => {
    const runner = makeCodexAppServerRunner({
      spawnImpl: () => {
        throw new Error("ENOENT codex")
      },
    })
    let caught: unknown = null
    try {
      await runner(runnerInput())
    } catch (error) {
      caught = error
    }
    expect(isCodexAppServerPreFrameFailure(caught)).toBe(true)
  })
})
