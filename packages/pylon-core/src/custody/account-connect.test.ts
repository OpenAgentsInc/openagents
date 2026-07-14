import { existsSync } from "node:fs"
import { describe, expect, test } from "vite-plus/test"
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  parsePylonAccountsConnectArgs,
  runPylonAccountsConnect,
  writeClaudeOauthTokenFile,
} from "./account-connect.js"
import { hashPylonAccountRef, PYLON_CLAUDE_OAUTH_TOKEN_FILE } from "./account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "../shared/bootstrap.js"
import { assertPublicProjectionSafe } from "../shared/state.js"

async function withHome<T>(fn: (home: string) => Promise<T>): Promise<T> {
  const home = await mkdtemp(join(tmpdir(), "pylon-core-account-connect-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("pylon-core accounts connect claude", () => {
  test("normalizes claude alias and rejects device-login flags", () => {
    expect(parsePylonAccountsConnectArgs(["claude", "--account", "a", "--token", "t", "--json"])).toMatchObject({
      provider: "claude_agent",
      setupToken: "t",
    })
    expect(() =>
      parsePylonAccountsConnectArgs(["claude", "--account", "a", "--force-device-login", "--json"]),
    ).toThrow(/does not use device-login/)
    expect(() =>
      parsePylonAccountsConnectArgs(["claude", "--account", "a", "--skip-device-login", "--json"]),
    ).toThrow(/does not use device-login/)
  })

  test("writes claude-oauth-token with private mode and public-safe projection", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const secret = "sk-ant-oat-pylon-core-secret"
      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs([
          "claude",
          "--account",
          "core-claude",
          "--token",
          secret,
          "--json",
        ]),
        { env: { PYLON_HOME: home } },
      )

      expect(projection).toMatchObject({
        schema: "pylon.accounts.connect.v1",
        provider: "claude_agent",
        accountRef: "core-claude",
        accountRefHash: hashPylonAccountRef("claude_agent", "core-claude"),
        codexCredentialStore: "not_applicable",
        registry: { status: "created" },
        deviceLogin: { status: "completed", reason: "setup_token" },
      })
      assertPublicProjectionSafe(projection)
      const serialized = JSON.stringify(projection)
      expect(serialized).not.toContain(secret)
      expect(serialized).not.toContain("sk-ant-oat")
      expect(serialized).not.toContain(home)

      const tokenPath = join(home, "accounts", "claude_agent", "core-claude", PYLON_CLAUDE_OAUTH_TOKEN_FILE)
      expect(await readFile(tokenPath, "utf8")).toBe(`${secret}\n`)
      expect((await stat(tokenPath)).mode & 0o077).toBe(0)

      // Helper write path also keeps private perms.
      const otherHome = join(home, "manual-claude")
      await writeClaudeOauthTokenFile(otherHome, "sk-ant-oat-manual")
      expect(await readFile(join(otherHome, PYLON_CLAUDE_OAUTH_TOKEN_FILE), "utf8")).toBe("sk-ant-oat-manual\n")
      expect((await stat(join(otherHome, PYLON_CLAUDE_OAUTH_TOKEN_FILE))).mode & 0o077).toBe(0)

      // Existing file can be reused without supplying a token again.
      await mkdir(join(home, "accounts", "claude_agent", "reuse"), { recursive: true })
      await writeFile(
        join(home, "accounts", "claude_agent", "reuse", PYLON_CLAUDE_OAUTH_TOKEN_FILE),
        "sk-ant-oat-reuse\n",
        { mode: 0o600 },
      )
      const reused = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["claude", "--account", "reuse", "--json"]),
        { env: { PYLON_HOME: home } },
      )
      expect(reused.deviceLogin.status).toBe("skipped_existing_auth")
      expect(JSON.stringify(reused)).not.toContain("sk-ant-oat-reuse")
    })
  })
})

describe("pylon-core accounts connect grok", () => {
  test("accepts only named isolated Grok device-login custody", () => {
    expect(
      parsePylonAccountsConnectArgs([
        "grok",
        "--account",
        "grok-owner",
        "--json",
      ]),
    ).toMatchObject({
      provider: "grok",
      accountRef: "grok-owner",
      home: null,
    })
    expect(() =>
      parsePylonAccountsConnectArgs([
        "grok",
        "--account",
        "grok-owner",
        "--grok-home",
        "~/.grok",
      ]),
    ).toThrow(/always uses the isolated/)
    expect(() =>
      parsePylonAccountsConnectArgs([
        "grok",
        "--account",
        "grok-owner",
        "--openagents-link",
      ]),
    ).toThrow(/does not support the Codex/)
    expect(() =>
      parsePylonAccountsConnectArgs([
        "grok",
        "--account",
        "grok-owner",
        "--token",
        "not-valid-for-grok",
      ]),
    ).toThrow(/only valid.*claude/)
  })

  test("runs login and readiness only inside the derived GROK_HOME", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const defaultHome = join(home, "owner-default-grok-must-not-be-used")
      const expectedHome = join(home, "accounts", "grok", "grok-owner")
      const originalProcessHome = process.env.GROK_HOME
      let probes = 0
      let logins = 0
      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs([
          "grok",
          "--account",
          "grok-owner",
          "--json",
        ]),
        {
          env: {
            HOME: home,
            GROK_HOME: defaultHome,
            XAI_API_KEY: "shared-key-must-not-alias-accounts",
            GROK_CODE_XAI_API_KEY: "shared-code-key-must-not-alias-accounts",
            GROK_AUTH_PROVIDER_COMMAND: "shared-auth-must-not-alias-accounts",
            GROK_AUTH_PROVIDER_LABEL: "shared-auth-label-must-not-alias-accounts",
            GROK_AUTH: "shared-auth-token-must-not-alias-accounts",
            GROK_AUTH_PATH: "/shared/auth/must-not-alias-accounts",
            GROK_LOCAL_AUTH: "1",
          },
          grokReadinessProbe: async input => {
            probes += 1
            expect(input.home).toBe(expectedHome)
            expect(input.env.GROK_HOME).toBe(expectedHome)
            expect(input.env.HOME).toBe(home)
            expect(input.env.XAI_API_KEY).toBeUndefined()
            expect(input.env.GROK_CODE_XAI_API_KEY).toBeUndefined()
            expect(input.env.GROK_AUTH_PROVIDER_COMMAND).toBeUndefined()
            expect(input.env.GROK_AUTH_PROVIDER_LABEL).toBeUndefined()
            expect(input.env.GROK_AUTH).toBeUndefined()
            expect(input.env.GROK_AUTH_PATH).toBeUndefined()
            expect(input.env.GROK_LOCAL_AUTH).toBeUndefined()
            expect(input.timeoutMs).toBe(10_000)
            return probes === 1
              ? { ready: false, plane: "cli_session", failureClass: "auth_required" }
              : { ready: true, plane: "cli_session" }
          },
          runGrokDeviceLogin: async input => {
            logins += 1
            expect(input.home).toBe(expectedHome)
            expect(input.env.GROK_HOME).toBe(expectedHome)
            expect(input.env.XAI_API_KEY).toBeUndefined()
            expect(input.env.GROK_CODE_XAI_API_KEY).toBeUndefined()
            expect(input.env.GROK_AUTH_PROVIDER_COMMAND).toBeUndefined()
            expect(input.env.GROK_AUTH_PROVIDER_LABEL).toBeUndefined()
            expect(input.env.GROK_AUTH).toBeUndefined()
            expect(input.env.GROK_AUTH_PATH).toBeUndefined()
            expect(input.env.GROK_LOCAL_AUTH).toBeUndefined()
            return { exitCode: 0 }
          },
        },
      )

      expect({ probes, logins }).toEqual({ probes: 2, logins: 1 })
      expect(process.env.GROK_HOME).toBe(originalProcessHome)
      expect(projection).toMatchObject({
        schema: "pylon.accounts.connect.v1",
        provider: "grok",
        accountRef: "grok-owner",
        accountRefHash: hashPylonAccountRef("grok", "grok-owner"),
        codexCredentialStore: "not_applicable",
        registry: { status: "created" },
        deviceLogin: { status: "completed" },
      })
      const serialized = JSON.stringify(projection)
      expect(serialized).not.toContain(home)
      expect(serialized).not.toContain("shared-key")
      assertPublicProjectionSafe(projection)

      const config = JSON.parse(await readFile(summary.paths.config, "utf8")) as {
        dev?: { accounts?: Array<{ ref: string; provider: string; home: string }> }
      }
      expect(config.dev?.accounts).toEqual([
        { ref: "grok-owner", provider: "grok", home: expectedHome },
      ])
    })
  })

  test("reuses ready isolated custody and fails closed when post-login readiness is absent", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const args = parsePylonAccountsConnectArgs([
        "grok",
        "--account",
        "grok-ready",
        "--json",
      ])
      const reused = await runPylonAccountsConnect(summary, args, {
        grokReadinessProbe: async () => ({
          ready: true,
          plane: "cli_session",
        }),
        runGrokDeviceLogin: async () => {
          throw new Error("ready Grok custody must not re-login")
        },
      })
      expect(reused.deviceLogin).toEqual({
        status: "skipped_existing_auth",
        reason: "existing_grok_cli_session",
      })

      const blockedSummary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { PYLON_HOME: join(home, "blocked") },
      )
      await expect(
        runPylonAccountsConnect(blockedSummary, args, {
          grokReadinessProbe: async () => ({
            ready: false,
            plane: "cli_session",
            failureClass: "auth_required",
          }),
          runGrokDeviceLogin: async () => ({ exitCode: 0 }),
        }),
      ).rejects.toThrow(/isolated account readiness was not confirmed/)
      expect(await existsSync(blockedSummary.paths.config)).toBe(false)
    })
  })
})
