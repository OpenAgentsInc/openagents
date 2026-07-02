import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
  runKhalaCodeClaudeLiveSmoke,
} from "../src/bun/claude-live-smoke"
import type { KhalaCodeDesktopClaudeHarnessStatus } from "../src/bun/claude-harness-status"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempDir(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "khala-code-claude-live-smoke-test-"))
  tempDirs.push(dir)
  return dir
}

async function* messages(items: readonly unknown[]): AsyncGenerator<unknown> {
  for (const item of items) yield item
}

const readyHarnessStatus = (): KhalaCodeDesktopClaudeHarnessStatus => ({
  available: true,
  blockerRefs: [],
  capability: "claude_harness",
  credentialSourceRef: "credential.source.claude_agent.local_claude_session",
  observedAt: "2026-07-02T12:00:00.000Z",
  reason: "ready",
  status: "ready",
})

const unavailableHarnessStatus = (): KhalaCodeDesktopClaudeHarnessStatus => ({
  available: false,
  blockerRefs: ["blocker.claude_agent.credentials_missing"],
  capability: "claude_harness",
  credentialSourceRef: null,
  observedAt: "2026-07-02T12:00:00.000Z",
  reason: "Claude credentials are missing.",
  status: "credentials_missing",
})

describe("Khala Code Claude live smoke", () => {
  test("skips clearly unless the live Claude smoke is explicitly requested", async () => {
    let inspected = false
    const result = await runKhalaCodeClaudeLiveSmoke({
      inspectHarness: async () => {
        inspected = true
        return readyHarnessStatus()
      },
      requireLive: false,
    })

    expect(inspected).toBe(false)
    expect(result).toMatchObject({
      harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
      ok: true,
      required: false,
      skipped: true,
      status: "skipped",
    })
    expect(result.reason).toContain("KHALA_CODE_DESKTOP_CLAUDE_LIVE_SMOKE=1")
  })

  test("fails loudly when required but Claude is unavailable", async () => {
    const result = await runKhalaCodeClaudeLiveSmoke({
      inspectHarness: async () => unavailableHarnessStatus(),
      requireLive: true,
    })

    expect(result).toMatchObject({
      harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
      ok: false,
      required: true,
      skipped: false,
      status: "failed",
    })
    expect(result.reason).toContain("Explicit live Claude smoke requested")
    expect(result.reason).toContain("Claude credentials are missing")
  })

  test("runs a Claude runtime turn with approval callback and exact token ledger evidence", async () => {
    const root = await tempDir()
    const result = await runKhalaCodeClaudeLiveSmoke({
      env: {
        KHALA_CODE_TOKEN_USAGE_REMOTE_DISABLED: "1",
      },
      inspectHarness: async () => readyHarnessStatus(),
      query: input => ({
        async *[Symbol.asyncIterator]() {
          const canUseTool = input.options.canUseTool as (
            toolName: string,
            input: Record<string, unknown>,
            options: Record<string, unknown>,
          ) => Promise<unknown>
          await canUseTool("Read", { file_path: "README.md" }, {
            description: "Read a public repository file for the live smoke.",
            title: "Read README.md",
          })
          yield {
            message: {
              content: [
                { id: "tool-use-1", input: { file_path: "README.md" }, name: "Read", type: "tool_use" },
                { text: "khala-claude-smoke complete", type: "text" },
              ],
            },
            session_id: "claude-live-smoke-session",
            type: "assistant",
            uuid: "assistant-live-smoke",
          }
          yield {
            message: {
              content: [{ content: "ok", tool_use_id: "tool-use-1", type: "tool_result" }],
            },
            session_id: "claude-live-smoke-session",
            type: "user",
            uuid: "tool-result-live-smoke",
          }
          yield {
            model: "claude-sonnet-4",
            session_id: "claude-live-smoke-session",
            subtype: "success",
            type: "result",
            usage: {
              cache_read_input_tokens: 2,
              input_tokens: 11,
              output_tokens: 7,
              reasoning_output_tokens: 3,
            },
          }
        },
      }),
      requireLive: true,
      tokenUsageLedgerPath: join(root, "claude-token-usage-events.jsonl"),
      workingDirectory: root,
    })

    expect(result).toMatchObject({
      approvalCount: 1,
      approvalToolNames: ["Read"],
      harness: KHALA_CODE_CLAUDE_LIVE_SMOKE_HARNESS,
      ok: true,
      required: true,
      runtimeMode: "claude_runtime",
      skipped: false,
      status: "ok",
      threadId: "claude-live-smoke-session",
      tokenUsage: {
        provider: "pylon-claude-direct-local",
        totalTokens: 23,
        usageTruth: "exact",
      },
      tokenUsageDiagnostics: [],
      turnId: "khala-code-claude-live-smoke-turn",
      turnStatus: "completed",
      usedTools: ["Read"],
    })
    expect(result.tokenUsage?.eventId).toContain("token_usage_event.khala_code_claude_direct_local")
  })

  test("fails when the Claude turn never exercises canUseTool approval", async () => {
    const root = await tempDir()
    const result = await runKhalaCodeClaudeLiveSmoke({
      env: {
        KHALA_CODE_TOKEN_USAGE_REMOTE_DISABLED: "1",
      },
      inspectHarness: async () => readyHarnessStatus(),
      query: () => messages([{
        session_id: "claude-live-smoke-no-approval",
        subtype: "success",
        type: "result",
        usage: {
          input_tokens: 1,
          output_tokens: 1,
        },
      }]),
      requireLive: true,
      tokenUsageLedgerPath: join(root, "claude-token-usage-events.jsonl"),
      workingDirectory: root,
    })

    expect(result).toMatchObject({
      approvalCount: 0,
      ok: false,
      status: "failed",
    })
    expect(result.reason).toContain("approval callback was not exercised")
  })
})
