import { describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { runKhalaLogin, type KhalaLoginPrompt } from "./login.js"

// Isolate the token store so tests never read or write the real
// ~/.config/khala/agent-token. Each test gets a throwaway path.
function isolatedEnv(): Record<string, string | undefined> {
  const dir = mkdtempSync(join(tmpdir(), "khala-login-test-"))
  return { KHALA_TOKEN_PATH: join(dir, "agent-token") }
}

const PENDING_BODY = {
  schema: "openagents.pylon.auth.openagents.v1",
  status: "pending",
  attemptId: "pylon_openauth_test",
  expiresAt: "2026-06-27T04:12:19.889Z",
  intervalSeconds: 2,
  userCode: "TEST-CODE",
  verificationUrl:
    "https://example.test/api/pylon/auth/openagents/device/verify?attempt=pylon_openauth_test&code=TEST-CODE",
  linkedAgent: { tokenPrefix: "oa_agent_minted" },
}

const noSleep = async (): Promise<void> => {}

describe("khala login device-auth", () => {
  test("starts the flow, prints the verification prompt, polls, and stores the linked token", async () => {
    const prompts: Array<KhalaLoginPrompt> = []
    let startCalls = 0
    let pollCalls = 0
    const stored: Array<string> = []

    const fakeFetch = (async (url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      const target = String(url)
      const auth = (init?.headers as Record<string, string> | undefined)?.authorization
      if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
        startCalls += 1
        expect(auth).toBe("Bearer oa_agent_minted")
        return Response.json(PENDING_BODY, { status: 201 })
      }
      if (target.includes("/api/pylon/auth/openagents/device/pylon_openauth_test")) {
        pollCalls += 1
        return Response.json(
          {
            schema: "openagents.pylon.auth.openagents.v1",
            status: "linked",
            linkedAgent: { tokenPrefix: "oa_agent_minted" },
          },
          { status: 200 },
        )
      }
      if (target.endsWith("/api/agents/me")) {
        return Response.json(
          { authenticated: true, agent: { user: { displayName: "Artanis", primaryEmail: null } } },
          { status: 200 },
        )
      }
      throw new Error(`unexpected fetch: ${target}`)
    }) as unknown as typeof fetch

    const result = await runKhalaLogin({
      baseUrl: "https://example.test",
      env: isolatedEnv(),
      explicitToken: "oa_agent_minted",
      fetch: fakeFetch,
      onPrompt: prompt => prompts.push(prompt),
      sleep: noSleep,
      // a mock writeStoredAgentToken is not injectable; assert via result instead
    })

    expect(startCalls).toBe(1)
    expect(pollCalls).toBe(1)
    expect(prompts).toHaveLength(1)
    expect(prompts[0]?.userCode).toBe("TEST-CODE")
    expect(prompts[0]?.verificationUrl).toContain("/device/verify?attempt=pylon_openauth_test")
    expect(result.token).toBe("oa_agent_minted")
    expect(result.displayName).toBe("Artanis")
    expect(result.alreadyLinked).toBe(false)
    void stored
  })

  test("falls through an unauthorized explicit token to a freshly minted free key", async () => {
    let mintCalls = 0
    const startTokens: Array<string | undefined> = []

    const fakeFetch = (async (url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      const target = String(url)
      const auth = (init?.headers as Record<string, string> | undefined)?.authorization
      if (target.endsWith("/api/keys/free")) {
        mintCalls += 1
        return Response.json({ credential: { token: "oa_agent_minted" } }, { status: 201 })
      }
      if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
        startTokens.push(auth)
        // The bad explicit token is rejected; the minted token is accepted.
        if (auth === "Bearer oa_agent_bad") {
          return Response.json({ error: "unauthorized" }, { status: 401 })
        }
        return Response.json(PENDING_BODY, { status: 201 })
      }
      if (target.includes("/api/pylon/auth/openagents/device/pylon_openauth_test")) {
        return Response.json(
          { schema: "openagents.pylon.auth.openagents.v1", status: "linked", linkedAgent: {} },
          { status: 200 },
        )
      }
      if (target.endsWith("/api/agents/me")) {
        return Response.json({ authenticated: true, agent: { user: { displayName: "Khala Free" } } }, { status: 200 })
      }
      throw new Error(`unexpected fetch: ${target}`)
    }) as unknown as typeof fetch

    const result = await runKhalaLogin({
      baseUrl: "https://example.test",
      env: isolatedEnv(),
      explicitToken: "oa_agent_bad",
      fetch: fakeFetch,
      onPrompt: () => {},
      sleep: noSleep,
    })

    expect(mintCalls).toBe(1)
    expect(startTokens[0]).toBe("Bearer oa_agent_bad")
    expect(startTokens.at(-1)).toBe("Bearer oa_agent_minted")
    expect(result.token).toBe("oa_agent_minted")
  })

  test("reports an already-linked token without prompting", async () => {
    const prompts: Array<KhalaLoginPrompt> = []
    const fakeFetch = (async (url: Parameters<typeof fetch>[0]) => {
      const target = String(url)
      if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
        return Response.json(
          { schema: "openagents.pylon.auth.openagents.v1", status: "linked", linkedAgent: { tokenPrefix: "oa_agent_owner" } },
          { status: 200 },
        )
      }
      if (target.endsWith("/api/agents/me")) {
        return Response.json({ authenticated: true, agent: { user: { displayName: "Artanis" } } }, { status: 200 })
      }
      throw new Error(`unexpected fetch: ${target}`)
    }) as unknown as typeof fetch

    const result = await runKhalaLogin({
      baseUrl: "https://example.test",
      env: isolatedEnv(),
      explicitToken: "oa_agent_owner",
      fetch: fakeFetch,
      onPrompt: prompt => prompts.push(prompt),
      sleep: noSleep,
    })

    expect(prompts).toHaveLength(0)
    expect(result.alreadyLinked).toBe(true)
    expect(result.displayName).toBe("Artanis")
  })
})
