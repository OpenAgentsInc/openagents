import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { runKhalaCodeClaudeLiveSmoke } from "../src/bun/claude-live-smoke"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempLedger(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "khala-code-claude-live-smoke-"))
  tempDirs.push(dir)
  return join(dir, "claude-token-usage-events.jsonl")
}

describe("Khala Code Claude live smoke harness", () => {
  test("exercises canUseTool and proves exact token reporting through the desktop runtime", async () => {
    const ledgerPath = await tempLedger()
    const summary = await runKhalaCodeClaudeLiveSmoke({
      env: {
        KHALA_CODE_TOKEN_USAGE_REMOTE_DISABLED: "1",
      },
      localLedgerPath: ledgerPath,
      query: input => ({
        async *[Symbol.asyncIterator]() {
          const canUseTool = input.options.canUseTool as (
            toolName: string,
            toolInput: Record<string, unknown>,
            options: Record<string, unknown>,
          ) => Promise<unknown>
          yield {
            message: {
              content: [{ text: "Checking the working directory.", type: "text" }],
            },
            session_id: "claude-live-smoke-session",
            type: "assistant",
            uuid: "claude-live-smoke-assistant",
          }
          await canUseTool("Bash", { command: "pwd" }, {
            signal: new AbortController().signal,
            suggestions: [{ rule: "allow", toolName: "Bash" }],
            title: "Claude wants to run pwd",
          })
          yield {
            message: {
              content: [{ content: "/repo", tool_use_id: "tool-use-pwd", type: "tool_result" }],
            },
            session_id: "claude-live-smoke-session",
            type: "user",
            uuid: "claude-live-smoke-tool-result",
          }
          yield {
            model: "claude-sonnet-4",
            session_id: "claude-live-smoke-session",
            subtype: "success",
            type: "result",
            usage: {
              cache_read_input_tokens: 2,
              input_tokens: 7,
              output_tokens: 11,
              reasoning_output_tokens: 3,
            },
            uuid: "claude-live-smoke-result",
          }
        },
      }),
      readiness: {
        available: true,
        blockerRefs: [],
        capability: "claude_harness",
        credentialSourceRef: "credential.source.test",
        observedAt: "2026-07-02T00:00:00.000Z",
        reason: "ready",
        status: "ready",
      },
      timeoutMs: 5_000,
      workingDirectory: "/repo",
    })

    expect(summary).toMatchObject({
      approvalRequestCount: 1,
      exactTokenRows: 1,
      ok: true,
      responseRuntimeMode: "claude_runtime",
      readiness: { status: "ready" },
      runtimeBadgeMode: "claude_runtime",
      totalTokens: 23,
      usageTruth: "exact",
    })
    expect(summary.approvedToolNames).toEqual(["Bash"])
    expect(summary.toolNames).toEqual([])
    expect(summary.ledgerPath).toBe(ledgerPath)
  })
})
