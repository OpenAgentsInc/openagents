import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  fetchOperatorDashboard,
  normalizeOperatorDashboard,
} from "../src/shared/operator-dashboard.js"

const fleetStatus = {
  generatedAt: "2026-06-28T23:30:00.000Z",
  pace: {
    todayTokens: 4242,
  },
  fleet: {
    activeAssignmentCount: 1,
    busySlots: 1,
    inFlightAssignments: [
      {
        assignmentRef: "assignment_public_1",
        accountRef: "codex-3",
        elapsedMs: 125_000,
        jobKind: "codex_agent_task",
        pylonRef: "pylon_operator_1",
        provider: "codex",
        state: "running",
        totalTokens: 1337,
        updatedAt: "2026-06-28T23:29:00.000Z",
      },
    ],
    queuedSlots: 0,
    readySlots: 2,
    spread: [
      {
        busySlots: 1,
        codexCapable: true,
        heartbeatFresh: true,
        latestHeartbeatAt: "2026-06-28T23:29:50.000Z",
        pylonRef: "pylon_operator_1",
        queuedSlots: 0,
        readySlots: 2,
        status: "online",
      },
    ],
  },
}

describe("khala desktop operator dashboard", () => {
  test("normalizes fleet, account, rate-limit, and session projections", () => {
    const dashboard = normalizeOperatorDashboard({
      accountsStatus: {
        accounts: [
          {
            accountRefHash: "codex-3",
            cooldownExpiresAt: "2026-06-29T01:00:00.000Z",
            email: "operator@example.test",
            isRateLimited: true,
            provider: "codex",
            windows: [{ label: "weekly", percentUsed: 91 }],
          },
          {
            accountRefHash: "claude-main",
            isRateLimited: false,
            provider: "claude",
          },
        ],
      },
      baseUrl: "https://example.test",
      fleetStatus,
    })

    expect(dashboard.pylons).toHaveLength(1)
    expect(dashboard.accounts).toMatchObject([
      {
        accountRef: "codex-3",
        provider: "codex",
        readiness: "usage_limited",
        resetAt: "2026-06-29T01:00:00.000Z",
        usedPercent: 91,
      },
      {
        accountRef: "claude-main",
        provider: "claude",
        readiness: "ready",
      },
    ])
    expect(dashboard.sessions[0]).toMatchObject({
      accountRef: "codex-3",
      assignmentRef: "assignment_public_1",
      provider: "codex",
      tokenCount: 1337,
    })
    expect(dashboard.totals).toMatchObject({
      activeAssignments: 1,
      busySlots: 1,
      readyAccounts: 1,
      readySlots: 2,
      tokensToday: 4242,
    })
  })

  test("reports a clear owner-token blocker before polling", async () => {
    const result = await Effect.runPromise(fetchOperatorDashboard({ token: "" }))
    expect(result).toMatchObject({
      ok: false,
      error: "Set OPENAGENTS_AGENT_TOKEN to load the owner fleet dashboard.",
    })
  })

  test("polls both real operator endpoints with bearer auth", async () => {
    const seen: string[] = []
    const fetchStub = (async (input: string | URL | Request) => {
      const url = String(input)
      seen.push(url)
      const body = url.endsWith("/api/operator/fleet/status")
        ? fleetStatus
        : { accounts: [] }
      return new Response(JSON.stringify(body), {
        headers: { "content-type": "application/json" },
        status: 200,
      })
    }) as typeof fetch

    const result = await Effect.runPromise(
      fetchOperatorDashboard({
        baseUrl: "https://example.test/",
        fetch: fetchStub,
        token: "owner-token",
      }),
    )

    expect(result.ok).toBe(true)
    expect(seen).toEqual([
      "https://example.test/api/operator/fleet/status",
      "https://example.test/api/operator/accounts/status",
    ])
  })
})
