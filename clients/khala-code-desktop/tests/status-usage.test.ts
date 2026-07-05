import { describe, expect, test } from "bun:test"

import type {
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopRuntimeStatus,
  KhalaCodeDesktopThreadTokenSummary,
} from "../src/shared/rpc"
import {
  classifyKhalaCodeProviderError,
  projectKhalaCodeStatusUsage,
  projectKhalaCodeTimelineMetrics,
  projectKhalaCodeUsageBreakdown,
} from "../src/shared/status-usage"

const tokenSummary = (
  patch: Partial<KhalaCodeDesktopThreadTokenSummary> = {},
): KhalaCodeDesktopThreadTokenSummary => ({
  auditRows: 0,
  codexStateDbPath: "",
  codexStateTokens: 0,
  leaderboardLabel: "OpenAgents Stats",
  leaderboardSyncedTokens: 0,
  localLedgerPath: "",
  localMessageAuditLedgerPath: "",
  missingUsageTurns: 0,
  ok: true,
  pendingSyncTokens: 0,
  remoteConfigured: false,
  remoteDisabled: false,
  roleEconomics: [],
  threadId: "thread-1",
  totalTokens: 0,
  updatedAt: null,
  usageEventRows: 0,
  ...patch,
})

describe("Khala Code status and usage projection", () => {
  test("classifies provider auth, model, quota, server, and generic errors", () => {
    expect(classifyKhalaCodeProviderError("OpenAI API key missing")).toMatchObject({
      kind: "provider_auth",
      settingsEntryPoint: "provider",
      retryable: true,
    })
    expect(classifyKhalaCodeProviderError("model_not_found: gpt-missing")).toMatchObject({
      kind: "model_unavailable",
      settingsEntryPoint: "models",
    })
    expect(classifyKhalaCodeProviderError("429 rate limit exceeded")).toMatchObject({
      kind: "quota_or_rate_limit",
      settingsEntryPoint: "usage",
    })
    expect(classifyKhalaCodeProviderError("app-server connection refused")).toMatchObject({
      kind: "local_server_unavailable",
      settingsEntryPoint: "server",
    })
    expect(classifyKhalaCodeProviderError("unknown exploded")).toMatchObject({
      kind: "generic_failure",
      retryable: false,
    })
  })

  test("projects timeline metrics and usage breakdown without raw payloads", () => {
    const messages: KhalaCodeDesktopMessage[] = Array.from({ length: 260 }, (_, index) => ({
      id: `message-${index}`,
      role: index % 2 === 0 ? "user" : "assistant",
      body: `message ${index}`,
      ...(index % 10 === 0
        ? {
            codexItem: {
              itemId: `tool-${index}`,
              itemType: "mcpToolCall",
              status: "completed",
              title: "tool",
            },
          }
        : {}),
    }))
    const timeline = projectKhalaCodeTimelineMetrics(messages)
    expect(timeline).toMatchObject({
      messageCount: 260,
      toolCallCount: 26,
      estimatedVirtualizationUseful: true,
    })
    expect(timeline.anchorIds).toHaveLength(12)

    const usage = projectKhalaCodeUsageBreakdown(tokenSummary({
      missingUsageTurns: 1,
      totalTokens: 42,
    }))
    expect(usage).toMatchObject({
      status: "needs_attention",
      totalTokens: 42,
    })
  })

  test("projects runtime rows and sanitized errors together", () => {
    const runtimeStatuses: KhalaCodeDesktopRuntimeStatus[] = [{
      app: "Khala Code Desktop",
      available: true,
      capability: "pylon",
      observedAt: "2026-07-05T00:00:00.000Z",
      ok: true,
      reason: "ready",
      status: "ready",
    }]
    const projection = projectKhalaCodeStatusUsage({
      bootDegradedStates: [{
        detail: "session catalog degraded",
        method: "sessionCatalog",
        recoverable: true,
        state: "degraded",
      }],
      messages: [],
      runtimeStatuses,
      threadTokenSummary: tokenSummary({ totalTokens: 100 }),
      turnErrors: ["Bearer sk-1234567890abcdef123456 quota exceeded"],
    })

    expect(projection.runtime).toMatchObject({
      readyCount: 1,
      degradedCount: 1,
    })
    expect(projection.errors[0]).toMatchObject({
      kind: "provider_auth",
    })
    expect(JSON.stringify(projection)).not.toContain("sk-1234567890abcdef123456")
  })
})
