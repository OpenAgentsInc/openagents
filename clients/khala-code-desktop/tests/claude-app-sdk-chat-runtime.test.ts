import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createClaudeAppSdkChatRuntime } from "../src/bun/claude-app-sdk-chat-runtime"
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
      event: { invocationId: "tool-use-1", kind: "tool.completed" },
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

  test("streams a mocked SDK query and resumes by persisted Claude session id", async () => {
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
      sessionId: "claude-session-1",
    })
    expect(events.map(event => event.type)).toContain("message_delta")
  })

  test("interrupts the active SDK query handle", async () => {
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

})
