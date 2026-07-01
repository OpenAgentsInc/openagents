import { appendFile, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { Database } from "bun:sqlite"
import { afterEach, describe, expect, test } from "bun:test"

import {
  createKhalaCodeDesktopCodexMessageTokenAuditRecorder,
  createKhalaCodeDesktopCodexTokenUsageReporter,
  khalaCodeDesktopCodexMessageTokenAuditMessage,
  khalaCodeDesktopCodexTokenUsageEvent,
  khalaCodeDesktopCodexTokenUsageEventRefs,
  khalaCodeDesktopTokenUsageTelemetryStatus,
  readKhalaCodeDesktopThreadTokenSummary,
  startKhalaCodeDesktopTokenUsageBackgroundSync,
  syncKhalaCodeDesktopPendingTokenUsageReports,
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

async function tempLedgerRoot(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-token-usage-"))
  tempDirs.push(root)
  return root
}

const sampleReport = () => ({
  clientUserMessageId: "user-direct-local",
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
      privacy: {
        leaderboardEligible: true,
        privacyOptOut: false,
      },
      safeMetadata: {
        clientUserMessageId: "user-direct-local",
        codexThreadId: "thread-direct-local",
        codexTurnId: "turn-direct-local",
        desktopTurnId: "desktop-turn-direct-local",
      },
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
    const successLines = (await readFile(
      join(dirname(ledgerPath), "token-usage-report-successes.jsonl"),
      "utf8",
    )).trim().split("\n")
    expect(successLines).toHaveLength(1)
  })

  test("loads the owner Stats token from the local secret file when env is not exported", async () => {
    const root = await tempLedgerRoot()
    const secretPath = join(root, "vortex-admin.env")
    await writeFile(secretPath, "OPENAGENTS_ADMIN_API_TOKEN=test-token-from-file\n", "utf8")

    expect(khalaCodeDesktopTokenUsageTelemetryStatus({
      KHALA_CODE_TOKEN_USAGE_SECRET_PATH: secretPath,
    })).toMatchObject({
      remoteConfigured: true,
      remoteDisabled: false,
    })
  })

  test("reports missing remote mirroring when no token source is available", () => {
    expect(khalaCodeDesktopTokenUsageTelemetryStatus({
      KHALA_CODE_TOKEN_USAGE_SECRET_DISABLED: "1",
    })).toMatchObject({
      localMessageAuditLedgerPath: expect.stringContaining("message-token-audit.jsonl"),
      remoteConfigured: false,
      remoteDisabled: false,
    })
  })

  test("does not treat a Probe Omega bearer as a Stats token usage producer token", () => {
    expect(khalaCodeDesktopTokenUsageTelemetryStatus({
      KHALA_CODE_TOKEN_USAGE_SECRET_DISABLED: "1",
      PROBE_OMEGA_BEARER_TOKEN: "agent-token-that-cannot-post-token-usage",
    })).toMatchObject({
      remoteConfigured: false,
      remoteDisabled: false,
    })
  })

  test("retries failed local token usage events when Stats mirroring is configured", async () => {
    const root = await tempLedgerRoot()
    const localLedgerPath = join(root, "token-usage-events.jsonl")
    const failurePath = join(root, "token-usage-report-failures.jsonl")
    const oldEvent = khalaCodeDesktopCodexTokenUsageEvent({
      ...sampleReport(),
      codexThreadId: "thread-previous-failure",
      codexTurnId: "turn-previous-failure",
      desktopTurnId: "desktop-turn-previous-failure",
    })
    await appendFile(localLedgerPath, `${JSON.stringify({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: "2026-07-01T16:29:00.000Z",
      event: oldEvent,
    })}\n`, "utf8")
    await appendFile(failurePath, `${JSON.stringify({
      eventId: oldEvent.eventId,
      idempotencyKey: oldEvent.idempotencyKey,
      observedAt: "2026-07-01T16:29:00.000Z",
      status: 401,
    })}\n`, "utf8")

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
      localLedgerPath,
    })

    await reporter(sampleReport())

    expect(posts.map(post => (post.body as { eventId?: string }).eventId)).toEqual([
      String(oldEvent.eventId),
      String(khalaCodeDesktopCodexTokenUsageEvent(sampleReport()).eventId),
    ])
    expect(await readFile(failurePath, "utf8")).toBe("")
  })

  test("syncs local token usage events that were recorded before remote mirroring was configured", async () => {
    const root = await tempLedgerRoot()
    const localLedgerPath = join(root, "token-usage-events.jsonl")
    const localOnlyEvent = khalaCodeDesktopCodexTokenUsageEvent({
      ...sampleReport(),
      codexThreadId: "thread-local-only",
      codexTurnId: "turn-local-only",
      desktopTurnId: "desktop-turn-local-only",
    })
    const oldLocalOnlyEvent = {
      ...localOnlyEvent,
      privacy: { leaderboardEligible: false, privacyOptOut: true },
    }
    await appendFile(localLedgerPath, `${JSON.stringify({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: "2026-07-01T16:29:00.000Z",
      event: oldLocalOnlyEvent,
    })}\n`, "utf8")

    const posts: Array<{ readonly body: unknown, readonly url: string }> = []
    const result = await syncKhalaCodeDesktopPendingTokenUsageReports({
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
      localLedgerPath,
    })

    expect(result).toMatchObject({
      attempted: 1,
      failed: 0,
      ok: true,
      remoteConfigured: true,
      synced: 1,
    })
    expect(posts.map(post => (post.body as { eventId?: string }).eventId)).toEqual([
      String(localOnlyEvent.eventId),
    ])
    expect(posts[0]?.body).toMatchObject({
      privacy: { leaderboardEligible: true, privacyOptOut: false },
    })
    const successRows = (await readFile(
      join(root, "token-usage-report-successes.jsonl"),
      "utf8",
    )).trim().split("\n")
    expect(successRows).toHaveLength(1)
  })

  test("background sync replays pending Khala usage rows on startup and interval ticks", async () => {
    const root = await tempLedgerRoot()
    const localLedgerPath = join(root, "token-usage-events.jsonl")
    const firstEvent = khalaCodeDesktopCodexTokenUsageEvent({
      ...sampleReport(),
      codexThreadId: "thread-background-sync",
      codexTurnId: "turn-background-sync-1",
      desktopTurnId: "desktop-turn-background-sync-1",
    })
    const secondEvent = khalaCodeDesktopCodexTokenUsageEvent({
      ...sampleReport(),
      codexThreadId: "thread-background-sync",
      codexTurnId: "turn-background-sync-2",
      desktopTurnId: "desktop-turn-background-sync-2",
    })
    await appendFile(localLedgerPath, `${JSON.stringify({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: "2026-07-01T16:29:00.000Z",
      event: firstEvent,
    })}\n`, "utf8")

    const posts: Array<{ readonly body: unknown, readonly url: string }> = []
    const resultResolvers: Array<(result: unknown) => void> = []
    const nextResult = () => new Promise(resolve => resultResolvers.push(resolve))
    const intervalRef: { callback?: () => void } = {}
    let intervalMs = 0
    let cleared = false
    const firstResult = nextResult()
    const backgroundSync = startKhalaCodeDesktopTokenUsageBackgroundSync({
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
      intervalMs: 1_000,
      localLedgerPath,
      onResult: result => resultResolvers.shift()?.(result),
      setInterval: (callback, milliseconds) => {
        intervalRef.callback = callback
        intervalMs = milliseconds
        return "timer"
      },
      clearInterval: timer => {
        cleared = timer === "timer"
      },
    })

    await firstResult
    expect(intervalMs).toBe(1_000)
    expect(posts.map(post => (post.body as { eventId?: string }).eventId)).toEqual([
      String(firstEvent.eventId),
    ])

    await appendFile(localLedgerPath, `${JSON.stringify({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: "2026-07-01T16:30:00.000Z",
      event: secondEvent,
    })}\n`, "utf8")
    const secondResult = nextResult()
    if (intervalRef.callback === undefined) {
      throw new Error("background sync did not register an interval callback")
    }
    intervalRef.callback()
    await secondResult

    expect(posts.map(post => (post.body as { eventId?: string }).eventId)).toEqual([
      String(firstEvent.eventId),
      String(secondEvent.eventId),
    ])
    backgroundSync.dispose()
    expect(cleared).toBe(true)
  })

  test("stores local message provenance with exact turn token refs for reconciliation", async () => {
    const ledgerPath = await tempLedgerPath()
    const recorder = createKhalaCodeDesktopCodexMessageTokenAuditRecorder({
      env: {},
      localMessageAuditLedgerPath: ledgerPath,
    })
    const report = sampleReport()
    const refs = khalaCodeDesktopCodexTokenUsageEventRefs(report)

    await recorder({
      clientUserMessage: khalaCodeDesktopCodexMessageTokenAuditMessage({
        body: "Count this exact Khala client message",
        id: "user-direct-local",
        role: "user",
      }, "khala_code_client"),
      codexMessages: [
        khalaCodeDesktopCodexMessageTokenAuditMessage({
          body: "Codex answered this exact message",
          id: "assistant-direct-local",
          role: "assistant",
        }, "codex_app_server"),
      ],
      codexThreadId: report.codexThreadId,
      codexTurnId: report.codexTurnId,
      completedAt: "2026-07-01T16:30:04.000Z",
      desktopSessionId: report.desktopSessionId,
      desktopTurnId: report.desktopTurnId,
      model: report.model,
      reconciliation: {
        globalCountedTokens: 1290,
        globalCounterRoute: "/api/stats/token-usage/events",
        status: "global_count_event_recorded",
        tokenAccountingRequired: true,
        tokenScope: "codex_turn_provider_reported",
        usageTruth: "exact",
      },
      submittedAt: "2026-07-01T16:30:00.000Z",
      turnStatus: "completed",
      usage: report.usage,
      usageEvents: [{
        eventId: refs.eventId,
        idempotencyKey: refs.idempotencyKey,
        observedAt: report.observedAt,
        sequence: report.sequence,
        usage: report.usage,
      }],
    })

    const lines = (await readFile(ledgerPath, "utf8")).trim().split("\n")
    expect(lines).toHaveLength(1)
    const row = JSON.parse(lines[0] ?? "{}")
    expect(row).toMatchObject({
      schemaVersion: "khala-code-desktop.codex-message-token-audit.local.v1",
      record: {
        clientUserMessage: {
          body: "Count this exact Khala client message",
          id: "user-direct-local",
          source: "khala_code_client",
        },
        codexMessages: [{
          body: "Codex answered this exact message",
          id: "assistant-direct-local",
          source: "codex_app_server",
        }],
        reconciliation: {
          status: "global_count_event_recorded",
          tokenScope: "codex_turn_provider_reported",
        },
        usageEvents: [{
          eventId: refs.eventId,
          idempotencyKey: refs.idempotencyKey,
        }],
      },
    })
    expect(row.record.clientUserMessage.bodySha256).toHaveLength(64)
  })

  test("summarizes active-thread tokens from audited turns and live usage events", async () => {
    const root = await tempLedgerRoot()
    const localLedgerPath = join(root, "token-usage-events.jsonl")
    const localMessageAuditLedgerPath = join(root, "message-token-audit.jsonl")
    const env = {
      KHALA_CODE_MESSAGE_TOKEN_AUDIT_LOCAL_LEDGER_PATH: localMessageAuditLedgerPath,
      KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN: "test-token",
      KHALA_CODE_TOKEN_USAGE_LOCAL_LEDGER_PATH: localLedgerPath,
    }
    const recorder = createKhalaCodeDesktopCodexMessageTokenAuditRecorder({
      env,
      localMessageAuditLedgerPath,
    })
    const report = {
      ...sampleReport(),
      codexThreadId: "thread-token-meter",
      codexTurnId: "turn-audited",
      usage: {
        cachedInputTokens: 0,
        inputTokens: 100,
        outputTokens: 25,
        reasoningOutputTokens: 0,
        totalTokens: 125,
      },
    }

    await recorder({
      clientUserMessage: khalaCodeDesktopCodexMessageTokenAuditMessage({
        body: "Measure this thread",
        id: "user-token-meter",
        role: "user",
      }, "khala_code_client"),
      codexMessages: [
        khalaCodeDesktopCodexMessageTokenAuditMessage({
          body: "Measured.",
          id: "assistant-token-meter",
          role: "assistant",
        }, "codex_app_server"),
      ],
      codexThreadId: report.codexThreadId,
      codexTurnId: report.codexTurnId,
      completedAt: "2026-07-01T16:30:04.000Z",
      desktopSessionId: report.desktopSessionId,
      desktopTurnId: report.desktopTurnId,
      model: report.model,
      reconciliation: {
        aggregateBackfillEventId: "token_usage_event.khala_code_direct_local.backfill",
        aggregateBackfillIdempotencyKey: "khala-code-desktop:backfill:thread-token-meter",
        globalCountedTokens: 125,
        globalCounterRoute: "/api/stats/token-usage/events",
        status: "global_count_backfilled_aggregate",
        tokenAccountingRequired: true,
        tokenScope: "codex_turn_provider_reported",
        usageTruth: "exact",
      },
      submittedAt: "2026-07-01T16:30:00.000Z",
      turnStatus: "completed",
      usage: report.usage,
      usageEvents: [],
    })
    await appendFile(localLedgerPath, `${JSON.stringify({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: "2026-07-01T16:31:00.000Z",
      event: {
        eventId: "token_usage_event.live.ok",
        idempotencyKey: "khala-code-desktop:live:ok",
        observedAt: "2026-07-01T16:31:00.000Z",
        safeMetadata: { codexThreadId: report.codexThreadId },
        tokenCounts: {
          inputTokens: 7,
          outputTokens: 3,
          totalTokens: 10,
        },
      },
    })}\n`, "utf8")
    await appendFile(localLedgerPath, `${JSON.stringify({
      schemaVersion: "khala-code-desktop.codex-token-usage.local.v1",
      recordedAt: "2026-07-01T16:31:02.000Z",
      event: {
        eventId: "token_usage_event.live.failed",
        idempotencyKey: "khala-code-desktop:live:failed",
        observedAt: "2026-07-01T16:31:02.000Z",
        safeMetadata: { codexThreadId: report.codexThreadId },
        tokenCounts: {
          inputTokens: 4,
          outputTokens: 1,
          totalTokens: 5,
        },
      },
    })}\n`, "utf8")
    await appendFile(join(root, "token-usage-report-failures.jsonl"), `${JSON.stringify({
      eventId: "token_usage_event.live.failed",
      idempotencyKey: "khala-code-desktop:live:failed",
    })}\n`, "utf8")
    await appendFile(join(root, "token-usage-report-successes.jsonl"), `${JSON.stringify({
      eventId: "token_usage_event.live.ok",
      idempotencyKey: "khala-code-desktop:live:ok",
    })}\n`, "utf8")

    const summary = await readKhalaCodeDesktopThreadTokenSummary({
      env,
      threadId: report.codexThreadId,
    })

    expect(summary).toMatchObject({
      auditRows: 1,
      codexStateTokens: 0,
      leaderboardSyncedTokens: 135,
      missingUsageTurns: 0,
      pendingSyncTokens: 5,
      remoteConfigured: true,
      threadId: report.codexThreadId,
      totalTokens: 140,
      usageEventRows: 2,
    })
  })

  test("does not count Codex-only state tokens as Khala local or pending usage", async () => {
    const root = await tempLedgerRoot()
    const codexStateDbPath = join(root, "state_5.sqlite")
    const db = new Database(codexStateDbPath)
    db.exec(`
      create table threads (
        id text primary key,
        tokens_used integer not null default 0,
        updated_at integer,
        updated_at_ms integer
      );
      insert into threads (id, tokens_used, updated_at, updated_at_ms)
      values ('thread-visible-before-audit', 2439775, 1782926387, 1782926387386);
    `)
    db.close()

    const summary = await readKhalaCodeDesktopThreadTokenSummary({
      env: {
        KHALA_CODE_CODEX_STATE_DB_PATH: codexStateDbPath,
        KHALA_CODE_MESSAGE_TOKEN_AUDIT_LOCAL_LEDGER_PATH: join(root, "message-token-audit.jsonl"),
        KHALA_CODE_TOKEN_USAGE_SECRET_DISABLED: "1",
        KHALA_CODE_TOKEN_USAGE_LOCAL_LEDGER_PATH: join(root, "token-usage-events.jsonl"),
      },
      threadId: "thread-visible-before-audit",
    })

    expect(summary).toMatchObject({
      auditRows: 0,
      codexStateDbPath,
      codexStateTokens: 2_439_775,
      leaderboardSyncedTokens: 0,
      pendingSyncTokens: 0,
      threadId: "thread-visible-before-audit",
      totalTokens: 0,
      usageEventRows: 0,
    })
    expect(summary.updatedAt).toBe(null)
  })
})
