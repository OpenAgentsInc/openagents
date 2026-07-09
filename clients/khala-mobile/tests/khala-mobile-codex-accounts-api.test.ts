import { describe, expect, test } from "bun:test"

import {
  disconnectKhalaMobileCodexAccount,
  fetchKhalaMobileCodexAccounts,
  pollKhalaMobileCodexDeviceLogin,
  startKhalaMobileCodexDeviceLogin,
} from "../src/sync/khala-mobile-codex-accounts-api"

const account = {
  health: "healthy",
  lastStatusAt: "2026-07-08T12:00:00.000Z",
  providerAccountRef: "provider-account_1",
  publicStatus: "connected",
  quotaState: "available",
  status: "connected",
}

const attempt = {
  expiresAt: "2026-07-08T12:15:00.000Z",
  id: "provider_attempt_1",
  providerAccountRef: "provider-account_1",
  status: "pending",
  userCode: "ABCD-EFGH",
  verificationUrl: "https://auth.openai.com/codex/device",
}

const response = (body: unknown, status = 200) => ({
  json: async () => body,
  ok: status >= 200 && status < 300,
  status,
})

describe("contract khala_mobile.cx2.codex_accounts_api.v1", () => {
  test("lists accounts and maps account_exhausted/account_rate_limited failure classes", async () => {
    const result = await fetchKhalaMobileCodexAccounts("https://openagents.test", "token", async () =>
      response({
        accounts: [
          { ...account, failureClass: "account_exhausted", quotaState: "exhausted" },
          { ...account, failureClass: "account_rate_limited", providerAccountRef: "provider-account_2", quotaState: "rate_limited" },
        ],
        attempts: [attempt],
      }),
    )

    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.accounts.map(value => value.readiness)).toEqual([
      "account_exhausted",
      "account_rate_limited",
    ])
    expect(result.value.attempts[0]?.userCode).toBe("ABCD-EFGH")
  })

  test("starts, polls, and disconnects through mobile bearer routes without token material", async () => {
    const requests: Array<{ body?: string; method: string; url: string }> = []
    const fetchImpl = async (url: string, init: { body?: string; headers: Record<string, string>; method: string }) => {
      requests.push({ body: init.body, method: init.method, url })
      if (url.endsWith("/device-login/start")) {
        return response(
          {
            account,
            attempt,
            expiresAt: attempt.expiresAt,
            intervalSeconds: 5,
            providerAccountRef: "provider-account_1",
            userCode: "ABCD-EFGH",
            verificationUrl: "https://auth.openai.com/codex/device",
          },
          201,
        )
      }
      if (url.includes("/device-login/provider_attempt_1")) return response({ account, attempt: { ...attempt, status: "connected" } })
      return response({ account: { ...account, publicStatus: "disconnected", status: "disconnected" } })
    }

    const started = await startKhalaMobileCodexDeviceLogin("https://openagents.test", "token", fetchImpl)
    const polled = await pollKhalaMobileCodexDeviceLogin(
      "https://openagents.test",
      "token",
      "provider_attempt_1",
      fetchImpl,
    )
    const disconnected = await disconnectKhalaMobileCodexAccount(
      "https://openagents.test",
      "token",
      "provider-account_1",
      fetchImpl,
    )

    expect(started.ok && started.value.userCode).toBe("ABCD-EFGH")
    expect(polled.ok && polled.value.attempt.status).toBe("connected")
    expect(disconnected.ok && disconnected.value.status).toBe("disconnected")
    expect(requests.map(request => `${request.method} ${request.url}`)).toEqual([
      "POST https://openagents.test/api/mobile/codex-accounts/device-login/start",
      "GET https://openagents.test/api/mobile/codex-accounts/device-login/provider_attempt_1",
      "POST https://openagents.test/api/mobile/codex-accounts/provider-account_1/disconnect",
    ])
    expect(requests.some(request => request.body?.includes("auth") || request.body?.includes("token"))).toBe(false)
  })

  // Oracle for khala_mobile.settings.disconnect_removes_account_and_hides_stale.v1
  test("list projection hides stale/dead accounts and never renders them as connected (#8546)", async () => {
    const result = await fetchKhalaMobileCodexAccounts("https://openagents.test", "token", async () =>
      response({
        accounts: [
          { ...account, providerAccountRef: "provider-account_live" },
          // A disconnected row that a legacy server still returns (deleted_at
          // not yet set) must not appear as a connected account.
          {
            ...account,
            providerAccountRef: "provider-account_disconnected",
            publicStatus: "disconnected",
            status: "disconnected",
          },
          // A pending login whose device codes expired arrives as publicStatus
          // 'expired' and must be dropped.
          {
            ...account,
            providerAccountRef: "provider-account_expired",
            publicStatus: "expired",
            status: "connected",
          },
          {
            ...account,
            providerAccountRef: "provider-account_denied",
            publicStatus: "denied",
            status: "denied",
          },
        ],
        attempts: [
          { ...attempt, id: "attempt_live", providerAccountRef: "provider-account_live" },
          { ...attempt, id: "attempt_expired", providerAccountRef: "provider-account_expired", status: "expired" },
        ],
      }),
    )

    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.accounts.map(value => value.providerAccountRef)).toEqual([
      "provider-account_live",
    ])
    expect(result.value.attempts.map(value => value.providerAccountRef)).toEqual([
      "provider-account_live",
    ])
  })

  test("disconnect round-trip: after disconnect the account is gone from a refetch (#8546)", async () => {
    let disconnected = false
    const fetchImpl = async (url: string, init: { body?: string; headers: Record<string, string>; method: string }) => {
      if (url.endsWith("/provider-account_1/disconnect")) {
        disconnected = true
        return response({ account: { ...account, publicStatus: "disconnected", status: "disconnected" } })
      }
      // The list endpoint reflects server truth: once disconnected (soft-
      // deleted + projection-filtered) it is no longer returned at all.
      return response({ accounts: disconnected ? [] : [account], attempts: [] })
    }

    const before = await fetchKhalaMobileCodexAccounts("https://openagents.test", "token", fetchImpl)
    expect(before.ok && before.value.accounts.length).toBe(1)

    const result = await disconnectKhalaMobileCodexAccount(
      "https://openagents.test",
      "token",
      "provider-account_1",
      fetchImpl,
    )
    expect(result.ok).toBe(true)

    const after = await fetchKhalaMobileCodexAccounts("https://openagents.test", "token", fetchImpl)
    expect(after.ok && after.value.accounts).toEqual([])
  })
})
