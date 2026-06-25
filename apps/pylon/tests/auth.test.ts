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

describe("pylon auth", () => {
  test("connects OpenAgents and Codex with only device prompts surfaced", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const prompts: Array<{ userCode: string; verificationUrl: string }> = []
      const calls: Array<{ url: string; method: string | undefined; authorization: string | null }> = []
      const fetcher: typeof fetch = async (input, init) => {
        const url = String(input)
        calls.push({
          url,
          method: init?.method,
          authorization: new Headers(init?.headers).get("authorization"),
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
        if (url === "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/start") {
          return jsonResponse({
            attempt: { id: "provider_attempt_1", status: "pending" },
            expiresAt: "2026-06-25T12:10:00.000Z",
            intervalSeconds: 1,
            providerAccountRef: "provider_account_codex",
            pylonLink: { owner: "openauth", status: "linked" },
            userCode: "CODE-X123",
            verificationUrl: "https://auth.openai.com/device",
          }, 201)
        }
        if (url === "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/provider_attempt_1") {
          return jsonResponse({
            account: {
              status: "connected",
              providerAccountRef: "provider_account_codex",
            },
            attempt: { id: "provider_attempt_1", status: "connected" },
            pylonLink: { owner: "openauth", status: "linked" },
          })
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
            await writeFile(join(input.home, "auth.json"), JSON.stringify({ ok: true }))
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
        {
          kind: "codex_provider",
          userCode: "CODE-X123",
          verificationUrl: "https://auth.openai.com/device",
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
        },
      })
      expect(JSON.stringify(projection)).not.toContain("oa_agent_fixture_123")
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)

      expect(await readFile(join(home, "auth", "openagents-agent-token"), "utf8")).toBe("oa_agent_fixture_123\n")
      expect((await stat(join(home, "auth", "openagents-agent-token"))).mode & 0o077).toBe(0)
      expect(calls.map(call => call.authorization).filter(Boolean)).toEqual([
        "Bearer oa_agent_fixture_123",
        "Bearer oa_agent_fixture_123",
        "Bearer oa_agent_fixture_123",
        "Bearer oa_agent_fixture_123",
      ])
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

      const fetcher: typeof fetch = async (input) => {
        const url = String(input)
        if (url === "https://openagents.example/api/pylon/auth/openagents/device/start") {
          return jsonResponse({
            schema: "openagents.pylon.auth.openagents.v1",
            status: "linked",
            linkedAgent: { tokenPrefix: "oa_agent_fixture" },
          })
        }
        if (url === "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/start") {
          return jsonResponse({
            attempt: { id: "provider_attempt_2", status: "pending" },
            expiresAt: "2026-06-25T12:10:00.000Z",
            intervalSeconds: 1,
            providerAccountRef: "provider_account_codex_2",
            pylonLink: { owner: "openauth", status: "linked" },
            userCode: "CODE-X222",
            verificationUrl: "https://auth.openai.com/device",
          }, 201)
        }
        if (url === "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/provider_attempt_2") {
          return jsonResponse({
            account: {
              status: "connected",
              providerAccountRef: "provider_account_codex_2",
            },
            attempt: { id: "provider_attempt_2", status: "connected" },
            pylonLink: { owner: "openauth", status: "linked" },
          })
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
            await writeFile(join(input.home, "auth.json"), JSON.stringify({ ok: true }))
            return { exitCode: 0 }
          },
          sleep: async () => undefined,
        },
      )

      expect(projection.accountRef).toBe("codex-2")
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
