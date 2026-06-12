import { describe, expect, test } from "bun:test"

import {
  runCodexComposerStream,
  summarizeCodexThreadEvent,
} from "../src/codex-composer"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"

async function* fakeCodexEvents() {
  yield { type: "thread.started", thread_id: "thread.test.codex" }
  yield { type: "turn.started" }
  yield {
    type: "item.completed",
    item: {
      id: "cmd-1",
      type: "command_execution",
      command: "bun test",
      aggregated_output: "ok",
      exit_code: 0,
      status: "completed",
    },
  }
  yield {
    type: "item.completed",
    item: {
      id: "patch-1",
      type: "file_change",
      status: "completed",
      changes: [
        { kind: "update", path: "src/index.ts" },
        { kind: "add", path: "tests/index.test.ts" },
      ],
    },
  }
  yield {
    type: "item.completed",
    item: {
      id: "msg-1",
      type: "agent_message",
      text: "Patched the composer.",
    },
  }
  yield {
    type: "turn.completed",
    usage: {
      input_tokens: 10,
      cached_input_tokens: 0,
      output_tokens: 5,
      reasoning_output_tokens: 2,
    },
  }
}

describe("Codex composer SDK stream", () => {
  test("runs the TypeScript SDK in the selected cwd and surfaces typed events", async () => {
    let threadOptions: Record<string, unknown> | null = null
    let promptSeen = ""
    let signalSeen = false
    const eventSummaries: string[] = []
    const textUpdates: string[] = []
    const usageUpdates: number[] = []
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          startThread(options: Record<string, unknown>) {
            threadOptions = options
            return {
              runStreamed: async (prompt: string, turnOptions: Record<string, unknown>) => {
                promptSeen = prompt
                signalSeen = turnOptions.signal instanceof AbortSignal
                return { events: fakeCodexEvents() }
              },
            }
          }
        },
      }
    }

    const result = await runCodexComposerStream(
      "fix the composer",
      {
        approvalPolicy: "never",
        codexCliLoginPresent: false,
        cwd: "/tmp/current-repo",
        env: { CODEX_API_KEY: "test-key-shape" },
        importer,
        model: "gpt-5-codex",
        networkAccessEnabled: false,
        platform: "darwin",
        sandboxMode: "workspace-write",
      },
      {
        onEvent: (summary) => eventSummaries.push(summary),
        onText: (text) => textUpdates.push(text),
        onUsage: (usage) => usageUpdates.push(usage.totalTokens),
      },
    )

    expect(promptSeen).toBe("fix the composer")
    expect(signalSeen).toBe(true)
    expect(threadOptions).toMatchObject({
      approvalPolicy: "never",
      model: "gpt-5-codex",
      networkAccessEnabled: false,
      sandboxMode: "workspace-write",
      skipGitRepoCheck: true,
      workingDirectory: "/tmp/current-repo",
    })
    expect(textUpdates).toEqual(["Patched the composer."])
    expect(usageUpdates).toEqual([15])
    expect(eventSummaries).toContain("completed: bun test exit 0")
    expect(eventSummaries).toContain("completed: update src/index.ts, add tests/index.test.ts")
    expect(result).toMatchObject({
      commandCount: 1,
      editedFileCount: 2,
      eventCount: 6,
      inputTokens: 10,
      outputTokens: 5,
      text: "Patched the composer.",
      threadId: "thread.test.codex",
      totalTokens: 15,
    })
  })

  test("reports readiness blockers before starting the SDK thread", async () => {
    const importer = async (specifier: string) => {
      if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        Codex: class {
          startThread() {
            throw new Error("must not start")
          }
        },
      }
    }

    await expect(
      runCodexComposerStream("fix", {
        codexCliLoginPresent: false,
        cwd: "/tmp/current-repo",
        env: {},
        importer,
        platform: "darwin",
      }),
    ).rejects.toThrow("Codex composer unavailable: credentials_missing")
  })

  test("summarizes todo and error stream events without raw payload dumps", () => {
    expect(
      summarizeCodexThreadEvent({
        type: "item.updated",
        item: {
          type: "todo_list",
          items: [
            { text: "inspect", completed: true },
            { text: "patch", completed: false },
          ],
        },
      }),
    ).toBe("todo list 1/2")
    expect(summarizeCodexThreadEvent({ type: "error", message: "bad stream" })).toBe("error: bad stream")
  })
})
