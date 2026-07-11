/**
 * EP250 regressions (owner hit both live on camera):
 *
 * 1. `pylon auth codex` completed the device login, wrote valid credentials
 *    into the isolated account home, and registered the account in
 *    config.json — then the post-auth OpenAgents provider-account import
 *    POST failed (server down) and the CLI reported a bare
 *    `Pylon auth failed` exit 1 for a flow that substantively succeeded.
 *
 * 2. Owner directive: the OpenAgents server link is opt-in / disabled by
 *    default. The DEFAULT `auth codex` flow is LOCAL-ONLY — isolated device
 *    login + local config registration with ZERO network calls to
 *    openagents.com. The server link runs only behind `--openagents-link`,
 *    and when that opt-in flow's import fails after a successful local
 *    connect, the result is an honest `connected_local_only` projection —
 *    never a throw.
 */
import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { parsePylonAuthArgs, runPylonAuthCodex } from "./auth.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

async function withHome<T>(fn: (home: string) => Promise<T>): Promise<T> {
  const home = await mkdtemp(join(tmpdir(), "pylon-auth-local-only-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

const AGENT_TOKEN = "oa_agent_test_0123456789abcdef"

const deviceLoginWritingAuth = async (input: { home: string }): Promise<{ exitCode: number }> => {
  await writeFile(
    join(input.home, "auth.json"),
    JSON.stringify({
      tokens: { access_token: "at_test", refresh_token: "rt_test", account_id: "acct_test" },
      last_refresh: new Date().toISOString(),
    }),
  )
  return { exitCode: 0 }
}

describe("pylon auth codex — LOCAL-ONLY default (owner directive)", () => {
  test("default connect makes ZERO openagents.com calls and reports connected", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const networkCalls: string[] = []
      const fetcher = (async (url: string | URL | Request): Promise<Response> => {
        const target = typeof url === "string" ? url : url instanceof URL ? url.href : url.url
        networkCalls.push(target)
        throw new Error("the local-only default must not touch the network")
      }) as unknown as typeof fetch

      const projection = await runPylonAuthCodex(
        summary,
        parsePylonAuthArgs(["codex", "--account", "codex-a", "--json"]),
        { env: {}, fetcher, runCodexDeviceLogin: deviceLoginWritingAuth },
      )

      expect(networkCalls).toEqual([])
      expect(projection.status).toBe("connected")
      expect(projection.accountRef).toBe("codex-a")
      expect(projection.openAgents).toBeUndefined()
      expect(projection.openAgentsProviderAccount.accountStatus).toBe("not_attempted_local_only")
      expect(projection.blockerRefs).toEqual([])
      assertPublicProjectionSafe(projection)

      // Local receipts: valid auth.json in the isolated home, account
      // registered in config.json.
      expect(existsSync(join(home, "accounts", "codex", "codex-a", "auth.json"))).toBe(true)
      const config = JSON.parse(await readFile(join(home, "config.json"), "utf8")) as {
        dev?: { accounts?: Array<{ provider?: string; ref?: string }> }
      }
      expect(
        config.dev?.accounts?.some(account => account.provider === "codex" && account.ref === "codex-a"),
      ).toBe(true)

      // No token material leaks into the public projection.
      const serialized = JSON.stringify(projection)
      expect(serialized).not.toContain("at_test")
      expect(serialized).not.toContain("rt_test")
    })
  })

  test("--openagents-link is rejected for non-codex targets", () => {
    expect(() => parsePylonAuthArgs(["claude", "--openagents-link"])).toThrow(
      "--openagents-link is only valid for pylon auth codex",
    )
  })
})

describe("pylon auth codex --openagents-link — connected_local_only (post-auth import failure)", () => {
  test("network-failed import returns connected_local_only; credentials and registration survive", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const fetcher = (async (url: string | URL | Request): Promise<Response> => {
        const target = typeof url === "string" ? url : url instanceof URL ? url.href : url.url
        if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
          return Response.json({ status: "linked", linkedAgent: { tokenPrefix: "oa_agent_te" } })
        }
        // The exact owner-observed class: fetch to openagents.com fails hard.
        throw new Error("Unable to connect. Is the computer able to access the url?")
      }) as unknown as typeof fetch

      const projection = await runPylonAuthCodex(
        summary,
        parsePylonAuthArgs([
          "codex", "--account", "codex-a", "--openagents-link", "--agent-token", AGENT_TOKEN, "--json",
        ]),
        { env: {}, fetcher, runCodexDeviceLogin: deviceLoginWritingAuth },
      )

      expect(projection.status).toBe("connected_local_only")
      expect(projection.accountRef).toBe("codex-a")
      expect(projection.localCodex.deviceLoginStatus).toBe("completed")
      expect(projection.openAgentsProviderAccount.accountStatus).toBe("import_failed")
      expect(projection.blockerRefs).toEqual([
        "blocker.pylon.auth.codex.openagents_provider_import_failed",
      ])
      assertPublicProjectionSafe(projection)

      // The local receipts the owner proved by hand survive the failed import.
      expect(existsSync(join(home, "accounts", "codex", "codex-a", "auth.json"))).toBe(true)
      const config = JSON.parse(await readFile(join(home, "config.json"), "utf8")) as {
        dev?: { accounts?: Array<{ provider?: string; ref?: string }> }
      }
      expect(
        config.dev?.accounts?.some(account => account.provider === "codex" && account.ref === "codex-a"),
      ).toBe(true)
    })
  })

  test("HTTP 5xx import failure also degrades to connected_local_only, never a throw", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const fetcher = (async (url: string | URL | Request): Promise<Response> => {
        const target = typeof url === "string" ? url : url instanceof URL ? url.href : url.url
        if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
          return Response.json({ status: "linked", linkedAgent: { tokenPrefix: "oa_agent_te" } })
        }
        return Response.json({ message: "service unavailable" }, { status: 503 })
      }) as unknown as typeof fetch

      const projection = await runPylonAuthCodex(
        summary,
        parsePylonAuthArgs([
          "codex", "--account", "codex-b", "--openagents-link", "--agent-token", AGENT_TOKEN, "--json",
        ]),
        { env: {}, fetcher, runCodexDeviceLogin: deviceLoginWritingAuth },
      )

      expect(projection.status).toBe("connected_local_only")
      expect(projection.blockerRefs).toEqual([
        "blocker.pylon.auth.codex.openagents_provider_import_failed",
      ])
    })
  })

  test("healthy opt-in import still reports plain connected (no regression on the happy path)", async () => {
    await withHome(async home => {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const fetcher = (async (url: string | URL | Request): Promise<Response> => {
        const target = typeof url === "string" ? url : url instanceof URL ? url.href : url.url
        if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
          return Response.json({ status: "linked", linkedAgent: { tokenPrefix: "oa_agent_te" } })
        }
        if (target.endsWith("/api/pylon/provider-accounts/chatgpt-codex/local-auth/import")) {
          return Response.json({
            account: { providerAccountRef: "pa_test", status: "active" },
            attempt: { id: "attempt_test", status: "confirmed" },
            pylonLink: { owner: "openauth", status: "linked" },
          })
        }
        return Response.json({ message: `unexpected call: ${target}` }, { status: 500 })
      }) as unknown as typeof fetch

      const projection = await runPylonAuthCodex(
        summary,
        parsePylonAuthArgs([
          "codex", "--account", "codex-c", "--openagents-link", "--agent-token", AGENT_TOKEN, "--json",
        ]),
        { env: {}, fetcher, runCodexDeviceLogin: deviceLoginWritingAuth },
      )

      expect(projection.status).toBe("connected")
      expect(projection.openAgents?.status).toBe("linked")
      expect(projection.openAgentsProviderAccount.providerAccountRef).toBe("pa_test")
      expect(projection.blockerRefs).toEqual([])
    })
  })
})
