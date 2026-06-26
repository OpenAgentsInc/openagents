import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  parsePylonAccountsConnectArgs,
  runPylonAccountsConnect,
} from "../src/account-connect"
import { hashPylonAccountRef } from "../src/account-registry"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { assertPublicProjectionSafe, ensurePylonLocalState, loadOrCreatePresenceState } from "../src/state"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-connect-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("pylon accounts connect", () => {
  test("registers an isolated Codex account home after device auth", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const calls: string[] = []
      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs([
          "codex",
          "--account",
          "codex-a",
          "--json",
        ]),
        {
          runCodexDeviceLogin: async input => {
            calls.push(input.home)
            await writeFile(join(input.home, "auth.json"), JSON.stringify({ authenticated: true }))
            return { exitCode: 0 }
          },
        },
      )

      expect(calls).toHaveLength(1)
      expect(projection).toMatchObject({
        schema: "pylon.accounts.connect.v1",
        provider: "codex",
        accountRef: "codex-a",
        accountRefHash: hashPylonAccountRef("codex", "codex-a"),
        codexCredentialStore: "file",
        registry: { status: "created" },
        deviceLogin: { status: "completed" },
        openAgentsDeviceLogin: { status: "not_requested" },
      })
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)

      const config = JSON.parse(await readFile(summary.paths.config, "utf8")) as {
        dev?: { accounts?: Array<{ ref: string; provider: string; home: string }> }
      }
      expect(config.dev?.accounts).toEqual([
        {
          ref: "codex-a",
          provider: "codex",
          home: join(home, "accounts", "codex", "codex-a"),
        },
      ])
      expect(await readFile(join(home, "accounts", "codex", "codex-a", "config.toml"), "utf8")).toContain(
        'cli_auth_credentials_store = "file"',
      )
    })
  })

  test("is idempotent when the account home is already authenticated", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const args = parsePylonAccountsConnectArgs([
        "codex",
        "--account",
        "codex-a",
        "--json",
      ])
      await runPylonAccountsConnect(summary, args, {
        runCodexDeviceLogin: async input => {
          await writeFile(join(input.home, "auth.json"), JSON.stringify({ ok: true }))
          return { exitCode: 0 }
        },
      })

      const second = await runPylonAccountsConnect(summary, args, {
        runCodexDeviceLogin: async () => {
          throw new Error("device login should not run when auth.json already exists")
        },
      })

      expect(second.registry.status).toBe("unchanged")
      expect(second.deviceLogin.status).toBe("skipped_existing_auth")
      const config = JSON.parse(await readFile(summary.paths.config, "utf8")) as {
        dev?: { accounts?: Array<unknown> }
      }
      expect(config.dev?.accounts).toHaveLength(1)
      assertPublicProjectionSafe(second)
    })
  })

  test("reused Codex auth still records linked OpenAgents presence", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const codexHome = join(home, "accounts", "codex", "codex-a")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "auth.json"), JSON.stringify({ authenticated: true }))

      const state = await ensurePylonLocalState(summary)
      const before = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(before.linked).toBe(false)
      expect(before.linkRef).toBeNull()

      const args = parsePylonAccountsConnectArgs([
        "codex",
        "--account",
        "codex-a",
        "--openagents-link",
        "--base-url",
        "https://openagents.example/",
        "--agent-token",
        "oa_agent_secret_test",
        "--json",
      ])
      const first = await runPylonAccountsConnect(summary, args, {
        fetcher: async () =>
          new Response(
            JSON.stringify({
              attempt: { id: "provider_attempt_reused", status: "pending" },
              expiresAt: "2026-06-25T12:10:00.000Z",
              intervalSeconds: 5,
              providerAccountRef: "provider_account_codex_reused",
              pylonLink: { owner: "openauth", status: "linked" },
              userCode: "ABCD-EFGH",
              verificationUrl: "https://auth.openai.com/device",
            }),
            { status: 201, headers: { "content-type": "application/json" } },
          ),
        runCodexDeviceLogin: async () => {
          throw new Error("device login should not run when auth.json already exists")
        },
      })

      expect(first.deviceLogin.status).toBe("skipped_existing_auth")
      expect(first.openAgentsDeviceLogin).toMatchObject({
        status: "started",
        providerAccountRef: "provider_account_codex_reused",
        pylonLink: { owner: "openauth", status: "linked" },
      })
      const linked = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(linked.linked).toBe(true)
      expect(linked.linkRef?.startsWith("link.account.")).toBe(true)
      assertPublicProjectionSafe({ linkRef: linked.linkRef })
      expect(linked.linkRef).not.toContain("provider_account_codex_reused")

      const second = await runPylonAccountsConnect(summary, args, {
        fetcher: async () =>
          new Response(
            JSON.stringify({
              attempt: { id: "provider_attempt_reused_2", status: "pending" },
              expiresAt: "2026-06-25T12:10:00.000Z",
              intervalSeconds: 5,
              providerAccountRef: "provider_account_codex_reused",
              pylonLink: { owner: "openauth", status: "linked" },
              userCode: "WXYZ-1234",
              verificationUrl: "https://auth.openai.com/device",
            }),
            { status: 201, headers: { "content-type": "application/json" } },
          ),
      })
      const linkedAgain = await loadOrCreatePresenceState(state.paths, state.identity)
      expect(second.deviceLogin.status).toBe("skipped_existing_auth")
      expect(linkedAgain.linkRef).toBe(linked.linkRef)
      assertPublicProjectionSafe(first)
      assertPublicProjectionSafe(second)
      expect(JSON.stringify(first)).not.toContain(home)
      expect(JSON.stringify(first)).not.toContain("oa_agent_secret_test")
    })
  })

  test("updates existing credential-store config and can register without running login", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const codexHome = join(home, "manual-codex")
      await mkdir(codexHome, { recursive: true })
      await writeFile(join(codexHome, "config.toml"), 'model = "gpt-5"\ncli_auth_credentials_store = "keyring"\n')

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs([
          "codex",
          "--account",
          "manual",
          "--home",
          codexHome,
          "--skip-device-login",
          "--json",
        ]),
        {
          runCodexDeviceLogin: async () => {
            throw new Error("device login should be skipped")
          },
        },
      )

      expect(projection.deviceLogin.status).toBe("skipped_by_flag")
      expect(await readFile(join(codexHome, "config.toml"), "utf8")).toContain(
        'cli_auth_credentials_store = "file"',
      )
      expect(existsSync(join(codexHome, "auth.json"))).toBe(false)
      expect(JSON.stringify(projection)).not.toContain(codexHome)
      assertPublicProjectionSafe(projection)
    })
  })

  test("can start the linked OpenAgents Codex provider-account device flow", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const seen: Array<{ url: string; authorization: string | null; body: Record<string, unknown> }> = []
      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs([
          "codex",
          "--account",
          "codex-a",
          "--skip-device-login",
          "--openagents-link",
          "--base-url",
          "https://openagents.example/",
          "--agent-token",
          "oa_agent_secret_test",
          "--json",
        ]),
        {
          fetcher: async (input, init) => {
            seen.push({
              url: String(input),
              authorization: new Headers(init?.headers).get("authorization"),
              body: JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>,
            })
            return new Response(
              JSON.stringify({
                attempt: { id: "provider_attempt_1", status: "pending" },
                expiresAt: "2026-06-25T12:10:00.000Z",
                intervalSeconds: 5,
                providerAccountRef: "provider_account_codex_a",
                pylonLink: { owner: "openauth", status: "linked" },
                userCode: "ABCD-EFGH",
                verificationUrl: "https://auth.openai.com/device",
              }),
              { status: 201, headers: { "content-type": "application/json" } },
            )
          },
        },
      )

      expect(seen).toEqual([
        {
          url: "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/start",
          authorization: "Bearer oa_agent_secret_test",
          body: {
            accountLabel: "codex-a",
            createNew: true,
          },
        },
      ])
      expect(projection.openAgentsDeviceLogin).toEqual({
        status: "started",
        attemptId: "provider_attempt_1",
        expiresAt: "2026-06-25T12:10:00.000Z",
        intervalSeconds: 5,
        providerAccountRef: "provider_account_codex_a",
        pylonLink: { owner: "openauth", status: "linked" },
        userCode: "ABCD-EFGH",
        verificationUrl: "https://auth.openai.com/device",
      })
      expect(JSON.stringify(projection)).not.toContain("oa_agent_secret_test")
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)
    })
  })

  test("can poll the linked OpenAgents Codex provider-account device flow", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const seen: Array<{ url: string; authorization: string | null; method: string | undefined }> = []
      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs([
          "codex",
          "--account",
          "codex-a",
          "--skip-device-login",
          "--openagents-attempt-id",
          "provider_attempt_1",
          "--base-url",
          "https://openagents.example/",
          "--agent-token",
          "oa_agent_secret_test",
          "--json",
        ]),
        {
          fetcher: async (input, init) => {
            seen.push({
              url: String(input),
              authorization: new Headers(init?.headers).get("authorization"),
              method: init?.method,
            })
            return new Response(
              JSON.stringify({
                account: {
                  status: "connected",
                  providerAccountRef: "provider_account_codex_a",
                },
                attempt: { id: "provider_attempt_1", status: "connected" },
                pylonLink: { owner: "openauth", status: "linked" },
              }),
              { status: 200, headers: { "content-type": "application/json" } },
            )
          },
        },
      )

      expect(seen).toEqual([
        {
          url: "https://openagents.example/api/pylon/provider-accounts/chatgpt-codex/device-login/provider_attempt_1",
          authorization: "Bearer oa_agent_secret_test",
          method: "GET",
        },
      ])
      expect(projection.openAgentsDeviceLogin).toEqual({
        status: "polled",
        attemptId: "provider_attempt_1",
        attemptStatus: "connected",
        accountStatus: "connected",
        providerAccountRef: "provider_account_codex_a",
        pylonLink: { owner: "openauth", status: "linked" },
      })
      expect(JSON.stringify(projection)).not.toContain("oa_agent_secret_test")
      expect(JSON.stringify(projection)).not.toContain(home)
      assertPublicProjectionSafe(projection)
    })
  })

  test("parses connect options conservatively", () => {
    expect(
      parsePylonAccountsConnectArgs([
        "codex",
        "--account",
        "work_1",
        "--openagents-link",
        "--openagents-attempt-id",
        "provider_attempt_1",
        "--provider-account-ref",
        "provider_account_work",
        "--force-device-login",
        "--json",
      ]),
    ).toMatchObject({
      provider: "codex",
      accountRef: "work_1",
      openAgentsLink: true,
      openAgentsAttemptId: "provider_attempt_1",
      providerAccountRef: "provider_account_work",
      forceDeviceLogin: true,
      json: true,
    })
    expect(() => parsePylonAccountsConnectArgs(["claude", "--account", "a", "--json"])).toThrow(/connect codex/)
    expect(() => parsePylonAccountsConnectArgs(["codex", "--account", "../bad", "--json"])).toThrow(
      /requires --account/,
    )
    expect(() =>
      parsePylonAccountsConnectArgs([
        "codex",
        "--account",
        "a",
        "--force-device-login",
        "--skip-device-login",
        "--json",
      ]),
    ).toThrow(/Use either --force-device-login or --skip-device-login/)
  })
})
