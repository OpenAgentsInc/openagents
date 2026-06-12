import { describe, expect, test } from "bun:test"

import { claudeComposerLabel, runClaudeComposerStream, summarizeClaudeComposerMessage } from "../src/claude-composer"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"

async function* fakeClaudeMessages() {
  yield {
    type: "system",
    subtype: "init",
    session_id: "session-test-claude",
    model: "claude-fable-5",
    permissionMode: "acceptEdits",
  }
  yield {
    type: "assistant",
    session_id: "session-test-claude",
    message: {
      content: [
        { type: "text", text: "I will inspect and patch it." },
        { type: "tool_use", name: "Bash", input: { command: "bun test" } },
        {
          type: "tool_use",
          name: "Edit",
          input: { file_path: "src/index.ts" },
        },
      ],
    },
  }
  yield {
    type: "assistant",
    session_id: "session-test-claude",
    message: {
      content: [{ type: "text", text: "Patched the Claude composer." }],
    },
  }
  yield {
    type: "result",
    subtype: "success",
    is_error: false,
    num_turns: 2,
    result: "done",
    session_id: "session-test-claude",
    total_cost_usd: 0.01,
    usage: { input_tokens: 20, output_tokens: 8 },
  }
}

describe("Claude composer SDK stream", () => {
  test("labels Claude with the selected model when one is configured", () => {
    expect(claudeComposerLabel("claude-fable-5")).toBe("Claude (claude-fable-5)")
    expect(claudeComposerLabel(null)).toBe("Claude")
  })

  test("runs the Claude Agent SDK in the selected cwd and resumes sessions", async () => {
    let queryOptions: Record<string, unknown> | null = null
    let promptSeen = ""
    const summaries: string[] = []
    const textUpdates: string[] = []
    const usageUpdates: number[] = []
    const importer = async (specifier: string) => {
      if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        query: (input: { prompt: string; options?: Record<string, unknown> }) => {
          promptSeen = input.prompt
          queryOptions = input.options ?? null
          return fakeClaudeMessages()
        },
      }
    }

    const result = await runClaudeComposerStream(
      "fix the composer",
      {
        config: { model: "claude-fable-5", maxTurns: 7 },
        cwd: "/tmp/current-repo",
        env: { ANTHROPIC_API_KEY: "test-key-shape" },
        importer,
        platform: "darwin",
        resumeSessionId: "session-existing",
      },
      {
        onEvent: (summary) => summaries.push(summary),
        onText: (text) => textUpdates.push(text),
        onUsage: (usage) => usageUpdates.push(usage.totalTokens),
      },
    )

    expect(promptSeen).toBe("fix the composer")
    expect(queryOptions).toMatchObject({
      allowedTools: ["Read", "Edit", "Write", "Bash", "Glob", "Grep"],
      cwd: "/tmp/current-repo",
      maxTurns: 7,
      model: "claude-fable-5",
      permissionMode: "acceptEdits",
      resume: "session-existing",
      settingSources: [],
    })
    expect(queryOptions?.abortController).toBeInstanceOf(AbortController)
    expect(summaries).toContain("session initialized")
    expect(summaries).toContain("assistant tool use: Bash, Edit")
    expect(summaries).toContain("success 2 turn(s)")
    expect(textUpdates).toEqual(["I will inspect and patch it.", "I will inspect and patch it.\n\nPatched the Claude composer."])
    expect(usageUpdates).toEqual([28])
    expect(result).toMatchObject({
      commandCount: 1,
      editedFileCount: 1,
      eventCount: 4,
      inputTokens: 20,
      outputTokens: 8,
      sessionId: "session-test-claude",
      text: "I will inspect and patch it.\n\nPatched the Claude composer.",
      totalCostUsd: 0.01,
      totalTokens: 28,
      turnCount: 2,
    })
    expect(result.sessionRef).toMatch(/^session\.pylon\.claude_composer\./)
    expect(JSON.stringify(result)).not.toContain("test-key-shape")
  })

  test("reports readiness blockers before starting the SDK stream", async () => {
    let queryStarted = false
    const importer = async (specifier: string) => {
      if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error(`unexpected import: ${specifier}`)
      return {
        query: () => {
          queryStarted = true
          return fakeClaudeMessages()
        },
      }
    }

    await expect(
      runClaudeComposerStream("fix", {
        cwd: "/tmp/current-repo",
        env: {},
        importer,
        localClaudeSessionProbe: async () => false,
        platform: "darwin",
      }),
    ).rejects.toThrow("Claude composer unavailable: credentials_missing")
    expect(queryStarted).toBe(false)
  })

  test("summarizes messages without dumping raw tool payloads", () => {
    expect(
      summarizeClaudeComposerMessage({
        type: "assistant",
        message: {
          content: [
            {
              type: "tool_use",
              name: "Bash",
              input: {
                command: "cat /Users/operator/.claude/.credentials.json",
              },
            },
          ],
        },
      }),
    ).toBe("assistant tool use: Bash")
    expect(summarizeClaudeComposerMessage({ type: "permission_denied" })).toBe("permission denied")
  })
})
