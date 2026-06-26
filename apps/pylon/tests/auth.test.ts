import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  parsePylonAuthArgs,
  runPylonAuthCodex,
  runPylonAuthOpenAgents,
} from "../src/auth"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { assertPublicProjectionSafe } from "../src/state"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-auth-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  })
}

const localCodexAuthFixture = (): Record<string, unknown> => ({
  auth_mode: "chatgpt",
  last_refresh: "2026-06-25T12:00:00.000Z",
  tokens: {
    access_token: "access-secret",
    refresh_token: "refresh-secret",
    account_id: "codex-account-fixture",
    id_token: "id-secret",
  },
})

describe("pylon auth", () => {
  test("connects OpenAgents and Codex with only required device prompts surfaced", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const prompts: Array<{ kind: string; userCode: string; verificationUrl: string }> = []
      const calls: Array<{
        authorization: string | null
        body: string | undefined
        method: string | undefined
        url: string
      }> = []
      const fetcher: typeof fetch = async (input, init) => {
        const url = String(input)
        calls.push({
          url,
          method: init?.method,
          authorization: new Headers(init?.headers).get("authorization"),
          body: typeof init?.body === "string" ? init.body : undefined,
        })
        if (url === "https://openagents.example/api/agents/register") {
          return jsonResponse({
            credential: { token: "oa_agent_fixture_123" },
          }, 201)
        }
        if (url === "https://openagents.example/api/pylon/auth/openagents/device/start") {
          return jsonResponse({
            schema: "openagents.pylon.auth.openagents.v1",
            status: "pending",
            attemptId: "pylon_openauth_attempt_1",
            expiresAt: "2026-06-25T12:05:00.000Z",
            intervalSeconds: 1,
            linkedAgent: { tokenPrefix: "oa_agent_fixture" },
            userCode: "PYLO-NOPE",
            verificationUrl: "https://openagents.example/api/pylon/auth/openagents/device/verify?attempt=pylon_openauth_attempt_1&code=PYLO-NOPE",
          }, 201)
        }
        if (url === "https://openagents.example/api/pylon/auth/openagents/device/pylon_openauth_attempt_1") {
          return jsonResponse({
            schema: "openagents.pylon.auth.openagents.v1",
            status: "linked",
            linkedAgent: { tokenPrefix: "oa_agent_fixture" },
          })
        }
        if (url === "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/local-auth/import") {
          const body = JSON.parse(String(init?.body)) as Record<string, unknown>
          const auth = body.auth as Record<string, unknown>
          expect(body).toMatchObject({
            accountLabel: "codex",
            createNew: true,
          })
          expect(auth).toMatchObject({
            type: "oauth",
            access: "access-secret",
            refresh: "refresh-secret",
            accountId: "codex-account-fixture",
            idToken: "id-secret",
          })
          return jsonResponse({
            account: {
              status: "connected",
              providerAccountRef: "provider_account_codex",
            },
            attempt: { id: "provider_attempt_import_1", status: "connected" },
            pylonLink: { owner: "openauth", status: "linked" },
          }, 201)
        }
        throw new Error(`unexpected fetch ${url}`)
      }

      const projection = await runPylonAuthCodex(
        summary,
        parsePylonAuthArgs(["codex", "--base-url", "https://openagents.example"]),
        {
          env: {},
          fetcher,
          onDevicePrompt: prompt => prompts.push(prompt),
          runCodexDeviceLogin: async input => {
            await mkdir(input.home, { recursive: true })
            await writeFile(join(input.home, "auth.json"), JSON.stringify(localCodexAuthFixture()))
            return { exitCode: 0 }
          },
          sleep: async () => undefined,
        },
      )

      expect(prompts).toEqual([
        {
          kind: "openagents",
          userCode: "PYLO-NOPE",
          verificationUrl: "https://openagents.example/api/pylon/auth/openagents/device/verify?attempt=pylon_openauth_attempt_1&code=PYLO-NOPE",
        },
      ])
      expect(projection).toMatchObject({
        schema: "pylon.auth.codex.v1",
        status: "connected",
        accountRef: "codex",
        localCodex: { deviceLoginStatus: "completed" },
        openAgentsProviderAccount: {
          accountStatus: "connected",
          attemptStatus: "connected",
          providerAccountRef: "provider_account_codex",
          source: "pylon_local_codex_auth",
        },
      })
      expect(JSON.stringify(projection)).not.toContain("oa_agent_fixture_123")
      expect(JSON.stringify(projection)).not.toContain("access-secret")
      expect(JSON.stringify(projection)).not.toContain("refresh-secret")
      expect(JSON.stringify(projection)).not.toContain("id-secret")
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)

      expect(await readFile(join(home, "auth", "openagents-agent-token"), "utf8")).toBe("oa_agent_fixture_123\n")
      expect((await stat(join(home, "auth", "openagents-agent-token"))).mode & 0o077).toBe(0)
      expect(calls.map(call => call.authorization).filter(Boolean)).toEqual([
        "Bearer oa_agent_fixture_123",
        "Bearer oa_agent_fixture_123",
        "Bearer oa_agent_fixture_123",
      ])
      expect(calls.map(call => call.url)).not.toContain(
        "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/start",
      )
      expect(JSON.parse(await readFile(summary.paths.config, "utf8"))).toMatchObject({
        dev: {
          accounts: [
            {
              ref: "codex",
              openAgentsProviderAccountRef: "provider_account_codex",
            },
          ],
        },
      })
    })
  })

  test("uses the next default Codex account ref for repeated auth", async () => {
    await withHome(async home => {
      await mkdir(home, { recursive: true })
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      await writeFile(
        summary.paths.config,
        JSON.stringify({
          dev: {
            accounts: [{ ref: "codex", provider: "codex", home: join(home, "accounts", "codex", "codex") }],
          },
        }),
      )
      await mkdir(join(home, "auth"), { recursive: true })
      await writeFile(join(home, "auth", "openagents-agent-token"), "oa_agent_fixture_456\n")

      const fetcher: typeof fetch = async (input, init) => {
        const url = String(input)
        if (url === "https://openagents.example/api/pylon/auth/openagents/device/start") {
          return jsonResponse({
            schema: "openagents.pylon.auth.openagents.v1",
            status: "linked",
            linkedAgent: { tokenPrefix: "oa_agent_fixture" },
          })
        }
        if (url === "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/local-auth/import") {
          const body = JSON.parse(String(init?.body)) as Record<string, unknown>
          expect(body).toMatchObject({
            accountLabel: "codex-2",
            createNew: true,
          })
          return jsonResponse({
            account: {
              status: "connected",
              providerAccountRef: "provider_account_codex_2",
            },
            attempt: { id: "provider_attempt_import_2", status: "connected" },
            pylonLink: { owner: "openauth", status: "linked" },
          }, 201)
        }
        throw new Error(`unexpected fetch ${url}`)
      }

      const projection = await runPylonAuthCodex(
        summary,
        parsePylonAuthArgs(["codex", "--base-url", "https://openagents.example"]),
        {
          env: {},
          fetcher,
          runCodexDeviceLogin: async input => {
            await mkdir(input.home, { recursive: true })
            await writeFile(join(input.home, "auth.json"), JSON.stringify(localCodexAuthFixture()))
            return { exitCode: 0 }
          },
          sleep: async () => undefined,
        },
      )

      expect(projection.accountRef).toBe("codex-2")
      expect(projection.openAgentsProviderAccount.providerAccountRef).toBe("provider_account_codex_2")
      assertPublicProjectionSafe(projection)
    })
  })

  test("can run only the OpenAgents link and reject malformed auth args", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const prompts: unknown[] = []
      const result = await runPylonAuthOpenAgents(
        summary,
        parsePylonAuthArgs(["openagents", "--base-url", "https://openagents.example"]),
        {
          env: { OPENAGENTS_AGENT_TOKEN: "oa_agent_fixture_789" },
          fetcher: async input => {
            expect(String(input)).toBe("https://openagents.example/api/pylon/auth/openagents/device/start")
            return jsonResponse({
              schema: "openagents.pylon.auth.openagents.v1",
              status: "linked",
              linkedAgent: { tokenPrefix: "oa_agent_fixture" },
            })
          },
          onDevicePrompt: prompt => prompts.push(prompt),
        },
      )

      expect(prompts).toEqual([])
      expect(result.projection.deviceLogin.status).toBe("already_linked")
      expect(JSON.stringify(result.projection)).not.toContain("oa_agent_fixture_789")
      assertPublicProjectionSafe(result.projection)
      expect(() => parsePylonAuthArgs(["claude"])).toThrow(/pylon auth openagents\|codex/)
      expect(() => parsePylonAuthArgs(["openagents", "--account", "codex-a"])).toThrow(/does not take --account/)
      expect(() => parsePylonAuthArgs(["codex", "--account", "../bad"])).toThrow(/letters, numbers/)
    })
  })
})
