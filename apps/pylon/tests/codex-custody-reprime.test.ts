import { describe, expect, test } from "bun:test"
import {
  CODEX_CUSTODY_REPRIME_PRE_EXPIRY_BUFFER_MS,
  reprimePylonCodexAccountAuthFromCustody,
} from "../src/codex-custody-reprime"
import {
  hashPylonAccountRef,
  type ResolvedPylonAccountSelection,
} from "../src/account-registry"

const now = new Date("2026-07-03T12:00:00.000Z")

const linkedAccount: ResolvedPylonAccountSelection = {
  provider: "codex",
  selector: "registry_ref",
  accountRef: "codex-work",
  accountRefHash: hashPylonAccountRef("codex", "codex-work"),
  home: "/tmp/pylon-codex-work",
  openAgentsProviderAccountRef: "provider_account.public.codex.work",
}

const authContentJson = (expires: number) =>
  JSON.stringify({
    openai: {
      type: "oauth",
      access: "access-secret",
      expires,
    },
  })

describe("codex custody re-prime", () => {
  test("fetches access-only auth material for linked Codex accounts", async () => {
    const requests: Array<{ url: string; authorization: string | null; body: unknown }> = []
    const expires = now.getTime() + CODEX_CUSTODY_REPRIME_PRE_EXPIRY_BUFFER_MS + 60_000
    const fetcher: typeof fetch = async (url, init) => {
      requests.push({
        url: String(url),
        authorization: new Headers(init?.headers).get("authorization"),
        body: JSON.parse(String(init?.body)),
      })
      return new Response(
        JSON.stringify({
          schema: "openagents.pylon.provider_account.codex_auth_material.v1",
          status: "issued",
          authMaterial: {
            authContentEnv: "OPENCODE_AUTH_CONTENT",
            authContentJson: authContentJson(expires),
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      )
    }

    const result = await reprimePylonCodexAccountAuthFromCustody({
      account: linkedAccount,
      agentToken: "oa_agent_test_token",
      baseUrl: "https://unit.openagents.test",
      env: { PATH: "/bin" },
      fetcher,
      now,
    })

    expect(result.status).toBe("reprimed")
    expect(requests).toEqual([
      {
        url: "https://unit.openagents.test/api/pylon/provider-accounts/chatgpt-codex/auth-material",
        authorization: "Bearer oa_agent_test_token",
        body: {
          accountRef: "codex-work",
          providerAccountRef: "provider_account.public.codex.work",
        },
      },
    ])
    const authContent = JSON.parse(result.env.OPENCODE_AUTH_CONTENT ?? "{}") as {
      openai: Record<string, unknown>
    }
    expect(authContent.openai.access).toBe("access-secret")
    expect(authContent.openai.expires).toBe(expires)
    expect(authContent.openai.refresh).toBeUndefined()
  })

  test("leaves unlinked accounts on their existing local auth path", async () => {
    const result = await reprimePylonCodexAccountAuthFromCustody({
      account: { ...linkedAccount, openAgentsProviderAccountRef: null },
      env: { PATH: "/bin" },
      now,
    })

    expect(result).toEqual({
      status: "not_applicable",
      env: { PATH: "/bin" },
      blockerRefs: [],
    })
  })

  test("blocks linked accounts when custody cannot be called", async () => {
    const result = await reprimePylonCodexAccountAuthFromCustody({
      account: linkedAccount,
      env: { PATH: "/bin" },
      now,
    })

    expect(result.status).toBe("blocked")
    expect(result.blockerRefs).toContain("blocker.pylon.codex_custody.agent_token_missing")
  })

  test("rejects auth material that expires inside the pre-expiry buffer", async () => {
    const fetcher: typeof fetch = async () =>
      new Response(
        JSON.stringify({
          authMaterial: {
            authContentEnv: "OPENCODE_AUTH_CONTENT",
            authContentJson: authContentJson(now.getTime() + 30_000),
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      )

    const result = await reprimePylonCodexAccountAuthFromCustody({
      account: linkedAccount,
      agentToken: "oa_agent_test_token",
      env: { PATH: "/bin" },
      fetcher,
      now,
    })

    expect(result.status).toBe("blocked")
    expect(result.blockerRefs).toContain("blocker.pylon.codex_custody.auth_material_expiring")
  })
})
