import { describe, expect, test } from "vite-plus/test"

import {
  disconnectKhalaMobileClaudeAccount,
  fetchKhalaMobileClaudeAccounts,
  importKhalaMobileClaudeLocalAuth,
} from "../src/sync/khala-mobile-claude-accounts-api"

const account = {
  health: "healthy",
  lastStatusAt: "2026-07-08T12:00:00.000Z",
  providerAccountRef: "provider-account_claude_1",
  publicStatus: "connected",
  quotaState: "available",
  status: "connected",
}

const response = (body: unknown, status = 200) => ({
  json: async () => body,
  ok: status >= 200 && status < 300,
  status,
})

describe("contract khala_mobile.cx5.claude_accounts_api.v1", () => {
  test("lists accounts and maps readiness without requiring device-login attempts", async () => {
    const result = await fetchKhalaMobileClaudeAccounts("https://openagents.test", "token", async () =>
      response({
        accounts: [
          { ...account, failureClass: "account_exhausted", quotaState: "exhausted" },
          {
            ...account,
            failureClass: "account_rate_limited",
            providerAccountRef: "provider-account_claude_2",
            quotaState: "rate_limited",
          },
        ],
        attempts: [],
      }),
    )

    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.accounts.map(value => value.readiness)).toEqual([
      "account_exhausted",
      "account_rate_limited",
    ])
  })

  test("imports and disconnects through mobile bearer routes; response never echoes the token", async () => {
    const secret = "sk-ant-oat-claude-secret-for-import"
    const requests: Array<{ body?: string; method: string; url: string }> = []
    const fetchImpl = async (
      url: string,
      init: { body?: string; headers: Record<string, string>; method: string },
    ) => {
      requests.push({ body: init.body, method: init.method, url })
      if (url.endsWith("/local-auth/import")) {
        return response(
          {
            account,
            attempt: {
              id: "provider_attempt_claude_1",
              providerAccountRef: "provider-account_claude_1",
              status: "connected",
            },
            providerAccountRef: "provider-account_claude_1",
          },
          201,
        )
      }
      return response({ account: { ...account, publicStatus: "disconnected", status: "disconnected" } })
    }

    const imported = await importKhalaMobileClaudeLocalAuth(
      "https://openagents.test",
      "token",
      { accountLabel: "Personal Claude", authContentValue: secret },
      fetchImpl,
    )
    const disconnected = await disconnectKhalaMobileClaudeAccount(
      "https://openagents.test",
      "token",
      "provider-account_claude_1",
      fetchImpl,
    )

    expect(imported.ok && imported.value.providerAccountRef).toBe("provider-account_claude_1")
    expect(imported.ok && imported.value.account.status).toBe("connected")
    expect(disconnected.ok && disconnected.value.status).toBe("disconnected")
    expect(requests.map(request => `${request.method} ${request.url}`)).toEqual([
      "POST https://openagents.test/api/mobile/claude-accounts/local-auth/import",
      "POST https://openagents.test/api/mobile/claude-accounts/provider-account_claude_1/disconnect",
    ])
    // Request body carries the token once for import; client never treats response as containing it.
    expect(requests[0]?.body).toContain(secret)
    expect(JSON.stringify(imported)).not.toContain(secret)
    expect(JSON.stringify(disconnected)).not.toContain(secret)
  })

  test("list projection hides stale/dead Claude accounts", async () => {
    const result = await fetchKhalaMobileClaudeAccounts("https://openagents.test", "token", async () =>
      response({
        accounts: [
          { ...account, providerAccountRef: "provider-account_live" },
          {
            ...account,
            providerAccountRef: "provider-account_disconnected",
            publicStatus: "disconnected",
            status: "disconnected",
          },
          {
            ...account,
            providerAccountRef: "provider-account_expired",
            publicStatus: "expired",
            status: "connected",
          },
        ],
        attempts: [],
      }),
    )

    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.accounts.map(value => value.providerAccountRef)).toEqual([
      "provider-account_live",
    ])
  })

  test("disconnect round-trip: after disconnect the account is gone from a refetch", async () => {
    let disconnected = false
    const fetchImpl = async (url: string) => {
      if (url.endsWith("/provider-account_claude_1/disconnect")) {
        disconnected = true
        return response({ account: { ...account, publicStatus: "disconnected", status: "disconnected" } })
      }
      return response({ accounts: disconnected ? [] : [account], attempts: [] })
    }

    const before = await fetchKhalaMobileClaudeAccounts("https://openagents.test", "token", fetchImpl)
    expect(before.ok && before.value.accounts.length).toBe(1)

    const result = await disconnectKhalaMobileClaudeAccount(
      "https://openagents.test",
      "token",
      "provider-account_claude_1",
      fetchImpl,
    )
    expect(result.ok).toBe(true)

    const after = await fetchKhalaMobileClaudeAccounts("https://openagents.test", "token", fetchImpl)
    expect(after.ok && after.value.accounts).toEqual([])
  })
})
