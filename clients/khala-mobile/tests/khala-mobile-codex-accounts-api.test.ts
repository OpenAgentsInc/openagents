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
})
