import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, test } from "bun:test"

import {
  createKhalaCodeDesktopCodexTokenUsageReporter,
  khalaCodeDesktopCodexTokenUsageEvent,
  khalaCodeDesktopTokenUsageTelemetryStatus,
} from "../src/bun/codex-token-usage-telemetry"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempLedgerPath(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-token-usage-"))
  tempDirs.push(root)
  return join(root, "token-usage-events.jsonl")
}

const sampleReport = () => ({
  codexThreadId: "thread-direct-local",
  codexTurnId: "turn-direct-local",
  desktopSessionId: "desktop-session-direct-local",
  desktopTurnId: "desktop-turn-direct-local",
  model: "gpt-5.5",
  observedAt: "2026-07-01T16:30:00.000Z",
  sequence: 1,
  turnStatus: "inProgress",
  usage: {
    cachedInputTokens: 456,
    inputTokens: 1234,
    outputTokens: 56,
    reasoningOutputTokens: 7,
    totalTokens: 1290,
  },
})

describe("Codex token usage telemetry", () => {
  test("builds a canonical direct-local token usage event without private material", () => {
    const event = khalaCodeDesktopCodexTokenUsageEvent(sampleReport())

    expect(event).toMatchObject({
      backendProfile: "codex-app-server",
      demand: {
        demandChannel: "direct_local",
        demandClient: "khala_code_desktop",
        demandKind: "own_capacity",
        demandSource: "direct_local_codex",
      },
      model: "gpt-5.5",
      producerSystem: "pylon",
      provider: "pylon-codex-direct-local",
      sourceRoute: "pylon_codex_direct_local",
      tokenCounts: {
        cacheReadTokens: 456,
        inputTokens: 1234,
        outputTokens: 56,
        reasoningTokens: 7,
        totalTokens: 1290,
      },
      usageTruth: "exact",
    })
    expect(JSON.stringify(event)).not.toContain("/Users/")
    expect(JSON.stringify(event)).not.toContain("Count this")
  })

  test("stores a local JSONL row and posts to OpenAgents when configured", async () => {
    const ledgerPath = await tempLedgerPath()
    const posts: Array<{ readonly body: unknown, readonly url: string }> = []
    const reporter = createKhalaCodeDesktopCodexTokenUsageReporter({
      env: {
        KHALA_CODE_TOKEN_USAGE_BASE_URL: "https://openagents.example",
        KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN: "test-token",
      },
      fetch: async (url, init) => {
        posts.push({
          body: JSON.parse(String(init?.body)),
          url: String(url),
        })
        return new Response(JSON.stringify({ inserted: true }), { status: 201 })
      },
      localLedgerPath: ledgerPath,
    })

    await reporter(sampleReport())

    const lines = (await readFile(ledgerPath, "utf8")).trim().split("\n")
    expect(lines).toHaveLength(1)
    expect(JSON.parse(lines[0] ?? "{}")).toMatchObject({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      event: {
        sourceRoute: "pylon_codex_direct_local",
        tokenCounts: {
          totalTokens: 1290,
        },
      },
    })
    expect(posts).toHaveLength(1)
    expect(posts[0]).toMatchObject({
      url: "https://openagents.example/api/stats/token-usage/events",
      body: {
        provider: "pylon-codex-direct-local",
        sourceRoute: "pylon_codex_direct_local",
      },
    })
  })

  test("reports local accounting as ready even when remote mirroring is not configured", () => {
    expect(khalaCodeDesktopTokenUsageTelemetryStatus({})).toMatchObject({
      remoteConfigured: false,
      remoteDisabled: false,
    })
  })
})
