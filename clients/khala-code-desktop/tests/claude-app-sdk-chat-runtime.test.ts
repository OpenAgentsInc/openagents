import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createClaudeAppSdkChatRuntime } from "../src/bun/claude-app-sdk-chat-runtime"
import { CLAUDE_APP_SDK_GAP_MATRIX } from "../src/bun/claude-app-sdk-gap-matrix"
import { createClaudeSessionStore } from "../src/bun/claude-session-store"
import { createClaudeThreadItemProjector } from "../src/bun/claude-thread-item-projector"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempPath(name: string): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "khala-code-claude-"))
  tempDirs.push(dir)
  return join(dir, name)
}

async function* messages(items: readonly unknown[]): AsyncGenerator<unknown> {
  for (const item of items) yield item
}

describe("Claude Agent SDK chat runtime", () => {
  test("projects SDK text reasoning tools and usage into neutral turn events", () => {
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: "desktop-session-1",
      turnId: "desktop-turn-1",
    })

    const first = projector.project({
      type: "assistant",
      uuid: "assistant-message-1",
      session_id: "claude-session-1",
      message: {
        content: [
          { type: "thinking", text: "checking constraints" },
          { type: "text", text: "Hello from Claude" },
          { type: "tool_use", id: "tool-use-1", name: "Bash", input: { command: "pwd" } },
        ],
      },
    })
    const second = projector.project({
      type: "user",
      uuid: "tool-result-message-1",
      session_id: "claude-session-1",
      message: {
        content: [{ type: "tool_result", tool_use_id: "tool-use-1", content: "ok" }],
      },
    })
    const done = projector.project({
      type: "result",
      subtype: "success",
      uuid: "result-1",
      session_id: "claude-session-1",
      usage: {
        input_tokens: 7,
        cache_read_input_tokens: 2,
        output_tokens: 11,
        reasoning_output_tokens: 3,
      },
    })

    expect(first.events.map(event => event.type)).toEqual([
      "message_start",
      "message_delta",
      "message_done",
      "message_start",
      "message_delta",
      "message_done",
      "tool_event",
    ])
    expect(second.events[0]).toMatchObject({
      event: { invocationId: "tool-use-1", kind: "tool_completed" },
      type: "tool_event",
    })
    expect(projector.messages().map(message => message.body)).toEqual([
      "checking constraints",
      "Hello from Claude",
    ])
    expect(done.usage).toEqual({
      cachedInput: 2,
      input: 7,
      output: 11,
      reasoningOutput: 3,
    })
    expect(projector.toolNames()).toEqual(["Bash"])
  })

  test("skips unmodeled SDK messages and renders stream_event deltas", () => {
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: "desktop-session-stream",
      turnId: "desktop-turn-stream",
    })

    expect(projector.project({ type: "rate_limit_event", uuid: "rate-limit-1" }).events).toEqual([])
    projector.project({
      event: {
        content_block: { type: "text" },
        index: 0,
        type: "content_block_start",
      },
      session_id: "claude-session-stream",
      type: "stream_event",
      uuid: "stream-start",
    })
    const delta = projector.project({
      event: {
        delta: { text: "partial text", type: "text_delta" },
        index: 0,
        type: "content_block_delta",
      },
      session_id: "claude-session-stream",
      type: "stream_event",
      uuid: "stream-delta",
    })
    const done = projector.project({
      event: {
        index: 0,
        type: "content_block_stop",
      },
      session_id: "claude-session-stream",
      type: "stream_event",
      uuid: "stream-stop",
    })

    expect(delta.events).toContainEqual({
      delta: "partial text",
      messageId: "desktop-turn-stream-claude-stream-0",
      turnId: "desktop-turn-stream",
      type: "message_delta",
    })
    expect(done.events).toContainEqual({
      messageId: "desktop-turn-stream-claude-stream-0",
      turnId: "desktop-turn-stream",
      type: "message_done",
    })
    expect(projector.messages()).toMatchObject([{ body: "partial text" }])
  })

  test("captures SDK modelUsage from interrupted results", () => {
    const projector = createClaudeThreadItemProjector({
      desktopSessionId: "desktop-session-usage",
      turnId: "desktop-turn-usage",
    })

    const result = projector.project({
      modelUsage: {
        "claude-opus-4": {
          cache_read_input_tokens: 5,
          input_tokens: 10,
          output_tokens: 7,
          reasoning_output_tokens: 2,
        },
      },
      session_id: "claude-session-usage",
      subtype: "interrupted",
      type: "result",
      uuid: "result-usage",
    })

    expect(result.status).toBe("interrupted")
    expect(result.usage).toEqual({
      cachedInput: 5,
      input: 10,
      output: 7,
      reasoningOutput: 2,
    })
  })

  test("persists desktop session mapping in the v1 Claude session store", async () => {
    const statePath = await tempPath("claude-sessions.json")
    const store = createClaudeSessionStore({
      now: () => new Date("2026-07-01T12:00:00.000Z"),
      path: statePath,
    })

    await store.put("desktop-session-1", {
      sessionId: "claude-session-1",
      lastTurnId: "turn-1",
    })

    expect(await store.get("desktop-session-1")).toEqual({
      sessionId: "claude-session-1",
      lastTurnId: "turn-1",
      updatedAt: "2026-07-01T12:00:00.000Z",
    })
    expect(JSON.parse(await readFile(statePath, "utf8"))).toMatchObject({
      schema: "khala-code-desktop.claude-sessions.v1",
      sessions: {
        "desktop-session-1": { sessionId: "claude-session-1" },
      },
    })
  })

  test("starts fresh sessions with sessionId and resumes later turns with resume alone", async () => {
    const statePath = await tempPath("claude-runtime-state.json")
    const events: KhalaCodeDesktopChatTurnEvent[] = []
    const queryCalls: unknown[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      onEvent: event => events.push(event),
      query: input => {
        queryCalls.push(input)
        return messages([
          {
            type: "assistant",
            uuid: "assistant-1",
            session_id: "claude-session-1",
            message: { content: [{ type: "text", text: "First turn" }] },
          },
          { type: "result", subtype: "success", session_id: "claude-session-1" },
        ])
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const first = await runtime.startTurn({
      messages: [{ body: "hello", id: "user-1", role: "user" }],
      sessionId: "desktop-session-1",
      turnId: "turn-1",
    })
    const second = await runtime.startTurn({
      messages: [{ body: "again", id: "user-2", role: "user" }],
      sessionId: "desktop-session-1",
      turnId: "turn-2",
    })

    expect(first.backend).toMatchObject({
      kind: "claude_app_sdk",
      runtimeMode: "claude_runtime",
      threadId: "claude-session-1",
    })
    expect(second.backend.threadId).toBe("claude-session-1")
    expect((queryCalls[1] as { options: Record<string, unknown> }).options).toMatchObject({
      resume: "claude-session-1",
    })
    expect((queryCalls[1] as { options: Record<string, unknown> }).options).not.toHaveProperty("sessionId")
    expect(events.map(event => event.type)).toContain("message_delta")
  })

  test("startThread then startTurn sends only the fresh sessionId", async () => {
    const statePath = await tempPath("claude-start-thread-state.json")
    const queryCalls: unknown[] = []
    const runtime = createClaudeAppSdkChatRuntime({
      query: input => {
        queryCalls.push(input)
        return messages([
          { type: "result", subtype: "success", session_id: "desktop-session-started" },
        ])
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    await runtime.startThread({ sessionId: "desktop-session-started" })
    await runtime.startTurn({
      messages: [{ body: "hello", id: "user-started", role: "user" }],
      sessionId: "desktop-session-started",
      turnId: "turn-started",
    })

    expect((queryCalls[0] as { options: Record<string, unknown> }).options).toMatchObject({
      sessionId: "desktop-session-started",
    })
    expect((queryCalls[0] as { options: Record<string, unknown> }).options).not.toHaveProperty("resume")
  })

  test("interrupts the active SDK query handle", async () => {
    const statePath = await tempPath("claude-interrupt-state.json")
    let interrupted = false
    let release!: () => void
    const released = new Promise<void>(resolve => {
      release = resolve
    })
    let queryStarted!: () => void
    const started = new Promise<void>(resolve => {
      queryStarted = resolve
    })
    const runtime = createClaudeAppSdkChatRuntime({
      query: () => {
        queryStarted()
        return {
          async *[Symbol.asyncIterator]() {
            await released
            yield { type: "result", subtype: "success", session_id: "claude-session-interrupt" }
          },
          interrupt: async () => {
            interrupted = true
            release()
          },
          close: async () => undefined,
        }
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const turn = runtime.startTurn({
      messages: [{ body: "wait", id: "user-1", role: "user" }],
      sessionId: "desktop-session-interrupt",
      turnId: "turn-interrupt",
    })
    await started
    await new Promise(resolve => setTimeout(resolve, 0))
    const interruptedResult = await runtime.interruptTurn({
      sessionId: "desktop-session-interrupt",
      turnId: "turn-interrupt",
    })
    await turn

    expect(interrupted).toBe(true)
    expect(interruptedResult).toMatchObject({
      ok: true,
      desktopTurnId: "turn-interrupt",
    })
  })

  test("does not report interrupt success when the SDK handle has no interrupt method", async () => {
    const statePath = await tempPath("claude-no-interrupt-state.json")
    let release!: () => void
    const released = new Promise<void>(resolve => {
      release = resolve
    })
    let queryStarted!: () => void
    const started = new Promise<void>(resolve => {
      queryStarted = resolve
    })
    const runtime = createClaudeAppSdkChatRuntime({
      query: () => {
        queryStarted()
        return {
          async *[Symbol.asyncIterator]() {
            await released
            yield { type: "result", subtype: "success", session_id: "claude-session-no-interrupt" }
          },
          close: async () => {
            release()
          },
        }
      },
      sessionStore: createClaudeSessionStore({ path: statePath }),
      workingDirectory: "/repo",
    })

    const turn = runtime.startTurn({
      messages: [{ body: "wait", id: "user-no-interrupt", role: "user" }],
      sessionId: "desktop-session-no-interrupt",
      turnId: "turn-no-interrupt",
    })
    await started
    await new Promise(resolve => setTimeout(resolve, 0))
    const interruptedResult = await runtime.interruptTurn({
      sessionId: "desktop-session-no-interrupt",
      turnId: "turn-no-interrupt",
    })
    await turn

    expect(interruptedResult).toMatchObject({
      ok: false,
      desktopTurnId: "turn-no-interrupt",
      error: "Claude Agent SDK query does not expose interrupt().",
    })
  })

  test("imports the Claude SDK gap matrix into the checked runtime surface", () => {
    expect(CLAUDE_APP_SDK_GAP_MATRIX.map(row => row.id)).toEqual([
      "claude.phase1.chat_stream",
      "claude.phase1.interrupt",
      "claude.phase1.session_resume",
      "claude.phase2.approvals",
      "claude.phase2.telemetry_ingest",
      "claude.phase3.sidebar_catalog",
    ])
    expect(CLAUDE_APP_SDK_GAP_MATRIX.filter(row => row.status === "covered").map(row => row.phase))
      .toEqual(["phase_1", "phase_1", "phase_1"])
  })
})
