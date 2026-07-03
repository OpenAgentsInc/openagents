import { describe, expect, test } from "bun:test"

import {
  parseKhalaCodeHeadlessArgs,
  runKhalaCodeDesktopHeadlessJsonl,
  type KhalaCodeDesktopHeadlessCodexRuntime,
} from "../src/bun/headless"
import type { KhalaCodeDesktopChatTurnEvent } from "../src/shared/rpc"

const writable = (): { readonly sink: { write: (chunk: string) => boolean }; readonly text: () => string } => {
  let value = ""
  return {
    sink: {
      write: (chunk: string) => {
        value += chunk
        return true
      },
    },
    text: () => value,
  }
}

const jsonl = (text: string): readonly Record<string, unknown>[] =>
  text.trim().split(/\n/u).filter(Boolean).map(line => JSON.parse(line) as Record<string, unknown>)

describe("Khala Code desktop headless JSONL mode", () => {
  test("parses preset flags without adding them to the prompt", () => {
    expect(parseKhalaCodeHeadlessArgs([
      "--json",
      "--preset",
      "architect-coder-judge",
      "fix",
      "the",
      "bug",
    ])).toEqual({
      preset: "architect-coder-judge",
      promptArgv: ["fix", "the", "bug"],
    })
    expect(parseKhalaCodeHeadlessArgs([
      "--preset=architect-coder-judge",
      "ship",
    ])).toEqual({
      preset: "architect-coder-judge",
      promptArgv: ["ship"],
    })
    expect(() => parseKhalaCodeHeadlessArgs(["--preset", "--json"]))
      .toThrow("khala code --preset requires a preset name.")
  })

  test("streams a Codex app-server thread and writes one final stdout object", async () => {
    let onEvent: ((event: KhalaCodeDesktopChatTurnEvent) => void) | null = null
    const stdout = writable()
    const stderr = writable()
    const runtime: KhalaCodeDesktopHeadlessCodexRuntime = {
      startThread: async (request = {}) => {
        expect(request).toMatchObject({
          cwd: process.cwd(),
          sessionId: "headless-session-1",
        })
        return {
          ok: true,
          ...(request.sessionId === undefined ? {} : { desktopSessionId: request.sessionId }),
          thread: { id: "thread-codex-1" },
          threadId: "thread-codex-1",
        }
      },
      startTurn: async request => {
        expect(request).toMatchObject({
          cwd: process.cwd(),
          sessionId: "headless-session-1",
          turnId: "desktop-turn-1",
        })
        expect(request.messages[0]).toMatchObject({
          body: "hello Codex",
          id: "headless-user-1",
          role: "user",
        })
        onEvent?.({
          message: { body: "", id: "item-codex-assistant-1", role: "assistant" },
          turnId: "desktop-turn-1",
          type: "message_start",
        })
        onEvent?.({
          delta: "Hello from Codex",
          messageId: "item-codex-assistant-1",
          turnId: "desktop-turn-1",
          type: "message_delta",
        })
        onEvent?.({
          messageId: "item-codex-assistant-1",
          turnId: "desktop-turn-1",
          type: "message_done",
        })
        return {
          backend: {
            kind: "codex_app_server",
            model: "gpt-5.1-codex",
            runtimeMode: "codex_harness",
            threadId: "thread-codex-1",
            toolCatalogKind: "codex_app_server",
            turnId: "turn-codex-1",
            turnStatus: "completed",
          },
          messages: [{ body: "Hello from Codex", id: "item-codex-assistant-1", role: "assistant" }],
          ok: true,
          toolNames: [],
          usedTools: [],
        }
      },
      interruptTurn: async () => {
        throw new Error("interrupt should not be called")
      },
    }

    const result = await runKhalaCodeDesktopHeadlessJsonl({
      createCodexChatRuntime: input => {
        onEvent = input.onEvent
        return runtime
      },
      env: {},
      prompt: "hello Codex",
      sessionId: "headless-session-1",
      stderr: stderr.sink,
      stdout: stdout.sink,
      turnId: "desktop-turn-1",
      workingDirectory: process.cwd(),
    })

    const progress = jsonl(stderr.text())
    const final = JSON.parse(stdout.text()) as Record<string, unknown>

    expect(result.finalMessage).toBe("Hello from Codex")
    expect(progress.map(event => event.type)).toEqual([
      "thread.started",
      "turn.started",
      "item.started",
      "item.delta",
      "item.completed",
      "turn.completed",
    ])
    expect(progress[0]).toMatchObject({
      session_id: "headless-session-1",
      thread_id: "thread-codex-1",
    })
    expect(progress.at(-1)).toMatchObject({
      codex_turn_id: "turn-codex-1",
      ok: true,
      status: "completed",
      thread_id: "thread-codex-1",
      turn_id: "desktop-turn-1",
    })
    expect(final).toMatchObject({
      codexTurnId: "turn-codex-1",
      finalMessage: "Hello from Codex",
      ok: true,
      sessionId: "headless-session-1",
      threadId: "thread-codex-1",
      turnId: "desktop-turn-1",
    })
  })

  test("writes a structured Codex unavailable error when app-server setup fails", async () => {
    const stdout = writable()
    const stderr = writable()

    await expect(runKhalaCodeDesktopHeadlessJsonl({
      createCodexChatRuntime: () => ({
        startThread: async () => {
          throw new Error("Codex auth.json is missing")
        },
        startTurn: async () => {
          throw new Error("turn should not start")
        },
        interruptTurn: async () => ({ ok: false, desktopSessionId: "headless-session-error" }),
      }),
      env: {},
      prompt: "hello",
      sessionId: "headless-session-error",
      stderr: stderr.sink,
      stdout: stdout.sink,
      turnId: "desktop-turn-error",
    })).rejects.toThrow("Codex auth.json is missing")

    expect(jsonl(stderr.text())).toEqual([{
      error: "Codex auth.json is missing",
      status: "codex_app_server_unavailable",
      turn_id: "desktop-turn-error",
      type: "turn.failed",
      usage: {
        cached_input: 0,
        input: 0,
        output: 0,
        reasoning_output: 0,
      },
    }])
    expect(JSON.parse(stdout.text())).toMatchObject({
      backend: {
        kind: "codex_app_server",
        runtimeMode: "codex_harness",
        toolCatalogKind: "codex_app_server",
      },
      error: "Codex auth.json is missing",
      ok: false,
      status: "codex_app_server_unavailable",
    })
  })

  test("can interrupt an active Codex app-server turn and exit cleanly", async () => {
    const stdout = writable()
    const stderr = writable()
    let interruptRequest: unknown = null
    let resolveInterrupted!: () => void
    const interrupted = new Promise<void>(resolve => {
      resolveInterrupted = resolve
    })

    await runKhalaCodeDesktopHeadlessJsonl({
      createCodexChatRuntime: () => ({
        startThread: async () => ({
          ok: true,
          thread: { id: "thread-interrupt" },
          threadId: "thread-interrupt",
        }),
        startTurn: async () => {
          await interrupted
          return {
            backend: {
              kind: "codex_app_server",
              model: "gpt-5.1-codex",
              runtimeMode: "codex_harness",
              threadId: "thread-interrupt",
              toolCatalogKind: "codex_app_server",
              turnId: "turn-interrupt",
              turnStatus: "interrupted",
            },
            messages: [{
              body: "Codex interrupted this turn.",
              id: "interrupt-status",
              role: "system",
            }],
            ok: true,
            toolNames: [],
            usedTools: [],
          }
        },
        interruptTurn: async request => {
          interruptRequest = request
          resolveInterrupted()
          return {
            ok: true,
            desktopSessionId: request.sessionId,
            ...(request.turnId === undefined ? {} : { desktopTurnId: request.turnId }),
            threadId: "thread-interrupt",
          }
        },
      }),
      env: {},
      interruptAfterMs: 0,
      prompt: "long running",
      sessionId: "headless-session-interrupt",
      stderr: stderr.sink,
      stdout: stdout.sink,
      turnId: "desktop-turn-interrupt",
    })

    expect(interruptRequest).toEqual({
      sessionId: "headless-session-interrupt",
      turnId: "desktop-turn-interrupt",
    })
    expect(jsonl(stderr.text()).at(-1)).toMatchObject({
      codex_turn_id: "turn-interrupt",
      ok: true,
      status: "interrupted",
      thread_id: "thread-interrupt",
      type: "turn.completed",
    })
    expect(JSON.parse(stdout.text())).toMatchObject({
      finalMessage: "",
      ok: true,
      threadId: "thread-interrupt",
    })
  })

  test("keeps the headless entrypoint on the Codex app-server runtime", async () => {
    const [headlessSource, indexSource] = await Promise.all([
      Bun.file(new URL("../src/bun/headless.ts", import.meta.url)).text(),
      Bun.file(new URL("../src/bun/index.ts", import.meta.url)).text(),
    ])

    expect(headlessSource).not.toContain("runKhalaCodeDesktopChatTurn")
    expect(indexSource).toContain("createCodexAppServerChatRuntime")
    expect(indexSource).toContain("KHALA_CODE_HEADLESS_INTERRUPT_AFTER_MS")
  })
})
