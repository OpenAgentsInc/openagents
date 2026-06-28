import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"

import {
  classifyCodexAuthProbeOutput,
  parsePylonAccountsConnectArgs,
  pylonCodexAuthCliOutcome,
  runPylonAccountsConnect,
  type PylonCodexAuthValidity,
  type PylonCodexAuthValidityProbe,
} from "./account-connect.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

async function withHome<T>(fn: (home: string) => Promise<T>): Promise<T> {
  const home = await mkdtemp(join(tmpdir(), "pylon-account-connect-probe-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function seedExistingAuth(home: string, accountRef: string): Promise<string> {
  const accountHome = join(home, "accounts", "codex", accountRef)
  await mkdir(accountHome, { recursive: true })
  await writeFile(join(accountHome, "auth.json"), JSON.stringify({ tokens: { access_token: "x" } }))
  return accountHome
}

const stubProbe = (result: PylonCodexAuthValidity): PylonCodexAuthValidityProbe => {
  return async () => result
}

describe("Codex reconnect credential-validity probe (revoked auth.json regression)", () => {
  test("(a) valid existing auth reuses the credential and never runs device-login", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await seedExistingAuth(home, "codex-a")
      const probeCalls: string[] = []
      const loginCalls: string[] = []

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--json"]),
        {
          codexAuthValidityProbe: async input => {
            probeCalls.push(input.home)
            return { valid: true }
          },
          runCodexDeviceLogin: async () => {
            loginCalls.push("called")
            throw new Error("device login must not run for a valid existing credential")
          },
        },
      )

      expect(probeCalls).toHaveLength(1)
      expect(loginCalls).toHaveLength(0)
      expect(projection.deviceLogin.status).toBe("skipped_existing_auth")
      expect(projection.deviceLogin.reason).toBeUndefined()
      expect(projection.blockerRefs).toEqual([])
      assertPublicProjectionSafe(projection)
    })
  })

  test("(b) revoked existing auth auto-runs device-login to recover and reports recovery", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const accountHome = await seedExistingAuth(home, "codex-a")
      const loginCalls: string[] = []

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--json"]),
        {
          codexAuthValidityProbe: stubProbe({ valid: false, reason: "credentials_revoked" }),
          runCodexDeviceLogin: async input => {
            loginCalls.push(input.home)
            // Recovery writes a fresh auth.json into the SAME isolated home.
            await writeFile(join(input.home, "auth.json"), JSON.stringify({ tokens: { access_token: "fresh" } }))
            return { exitCode: 0 }
          },
        },
      )

      expect(loginCalls).toEqual([accountHome])
      expect(projection.deviceLogin.status).toBe("completed_recovered_invalid_auth")
      expect(projection.deviceLogin.reason).toBe("credentials_revoked")
      expect(projection.blockerRefs).toEqual([])
      assertPublicProjectionSafe(projection)
    })
  })

  test("(b2) revoked existing auth that cannot be recovered is blocked, never a success", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await seedExistingAuth(home, "codex-a")

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--json"]),
        {
          codexAuthValidityProbe: stubProbe({ valid: false, reason: "credentials_revoked" }),
          // Non-interactive: device-login fails and does not refresh auth.json.
          runCodexDeviceLogin: async () => ({ exitCode: 1 }),
        },
      )

      expect(projection.deviceLogin.status).toBe("blocked_invalid_auth")
      expect(projection.deviceLogin.reason).toBe("credentials_revoked")
      expect(projection.blockerRefs).toContain(
        "blocker.pylon.accounts_connect.codex_credentials_invalid_unrecovered",
      )
      assertPublicProjectionSafe(projection)
    })
  })

  test("usage-limited existing auth is reused (not an auth failure) with the reason surfaced", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await seedExistingAuth(home, "codex-a")
      const loginCalls: string[] = []

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--json"]),
        {
          codexAuthValidityProbe: stubProbe({ valid: false, reason: "usage_limited" }),
          runCodexDeviceLogin: async () => {
            loginCalls.push("called")
            throw new Error("usage limits must not trigger a re-login")
          },
        },
      )

      expect(loginCalls).toHaveLength(0)
      expect(projection.deviceLogin.status).toBe("skipped_existing_auth")
      expect(projection.deviceLogin.reason).toBe("usage_limited")
      assertPublicProjectionSafe(projection)
    })
  })

  test("(c) --force-device-login always logs in and never runs the probe", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await seedExistingAuth(home, "codex-a")
      const probeCalls: string[] = []
      const loginCalls: string[] = []

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--force-device-login", "--json"]),
        {
          codexAuthValidityProbe: async input => {
            probeCalls.push(input.home)
            return { valid: true }
          },
          runCodexDeviceLogin: async input => {
            loginCalls.push(input.home)
            await writeFile(join(input.home, "auth.json"), JSON.stringify({ tokens: { access_token: "forced" } }))
            return { exitCode: 0 }
          },
        },
      )

      expect(probeCalls).toHaveLength(0)
      expect(loginCalls).toHaveLength(1)
      expect(projection.deviceLogin.status).toBe("completed")
      assertPublicProjectionSafe(projection)
    })
  })

  test("(d) --skip-device-login never logs in and never runs the probe", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await seedExistingAuth(home, "codex-a")
      const probeCalls: string[] = []
      const loginCalls: string[] = []

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--skip-device-login", "--json"]),
        {
          codexAuthValidityProbe: async input => {
            probeCalls.push(input.home)
            return { valid: false, reason: "credentials_revoked" }
          },
          runCodexDeviceLogin: async () => {
            loginCalls.push("called")
            throw new Error("device login must be skipped")
          },
        },
      )

      expect(probeCalls).toHaveLength(0)
      expect(loginCalls).toHaveLength(0)
      expect(projection.deviceLogin.status).toBe("skipped_by_flag")
      assertPublicProjectionSafe(projection)
    })
  })

  test("with no probe injected, behavior is the legacy reuse of existing auth", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      await seedExistingAuth(home, "codex-a")

      const projection = await runPylonAccountsConnect(
        summary,
        parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--json"]),
        {
          runCodexDeviceLogin: async () => {
            throw new Error("device login should not run when auth.json already exists and no probe is injected")
          },
        },
      )

      expect(projection.deviceLogin.status).toBe("skipped_existing_auth")
    })
  })
})

describe("(e) CLI message outcome never reports bare success for revoked creds", () => {
  test("blocked unrecovered credentials map to a non-ok CLI outcome", () => {
    const outcome = pylonCodexAuthCliOutcome("blocked_invalid_auth", "credentials_revoked")
    expect(outcome.ok).toBe(false)
    expect(outcome.kind).toBe("blocked")
    expect(outcome.reason).toBe("credentials_revoked")
  })

  test("recovered credentials report a re-auth (not a plain link)", () => {
    const outcome = pylonCodexAuthCliOutcome("completed_recovered_invalid_auth", "auth_error")
    expect(outcome).toEqual({ ok: true, kind: "reauthed", reason: "auth_error" })
  })

  test("valid reuse and fresh completion report a successful link", () => {
    expect(pylonCodexAuthCliOutcome("skipped_existing_auth")).toEqual({ ok: true, kind: "linked" })
    expect(pylonCodexAuthCliOutcome("completed")).toEqual({ ok: true, kind: "linked" })
    expect(pylonCodexAuthCliOutcome("skipped_by_flag")).toEqual({ ok: true, kind: "linked" })
  })
})

describe("classifyCodexAuthProbeOutput", () => {
  test("classifies a revoked refresh token as credentials_revoked", () => {
    expect(
      classifyCodexAuthProbeOutput({
        exitCode: 1,
        stdout: "",
        stderr: "Your access token could not be refreshed because your refresh token was revoked.",
      }),
    ).toEqual({ valid: false, reason: "credentials_revoked" })
  })

  test("classifies a usage limit as usage_limited", () => {
    expect(
      classifyCodexAuthProbeOutput({ exitCode: 1, stdout: "", stderr: "You have hit your usage limit." }),
    ).toEqual({ valid: false, reason: "usage_limited" })
  })

  test("classifies a 401/unauthorized as auth_error", () => {
    expect(
      classifyCodexAuthProbeOutput({ exitCode: 1, stdout: "", stderr: "http 401 Unauthorized" }),
    ).toEqual({ valid: false, reason: "auth_error" })
  })

  test("treats a clean exit as valid", () => {
    expect(classifyCodexAuthProbeOutput({ exitCode: 0, stdout: "ok", stderr: "" })).toEqual({ valid: true })
  })

  test("is fail-safe (valid + inconclusive) on an unrecognized failure", () => {
    expect(
      classifyCodexAuthProbeOutput({ exitCode: 7, stdout: "", stderr: "some unrelated transient glitch" }),
    ).toEqual({ valid: true, reason: "probe_inconclusive" })
  })
})

// Guards against ever silently reusing a present-but-dead auth.json: a seeded
// revoked credential must change state (recover or block), never bare-skip.
test("seeded existing auth still exists after a recovery attempt", async () => {
  await withHome(async home => {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const accountHome = await seedExistingAuth(home, "codex-a")
    await runPylonAccountsConnect(
      summary,
      parsePylonAccountsConnectArgs(["codex", "--account", "codex-a", "--json"]),
      {
        codexAuthValidityProbe: stubProbe({ valid: false, reason: "auth_error" }),
        runCodexDeviceLogin: async input => {
          await writeFile(join(input.home, "auth.json"), JSON.stringify({ tokens: { access_token: "fresh" } }))
          return { exitCode: 0 }
        },
      },
    )
    expect(existsSync(join(accountHome, "auth.json"))).toBe(true)
  })
})
