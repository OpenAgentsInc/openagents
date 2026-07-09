import { describe, expect, test } from "bun:test"
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
