import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, writeFile, mkdir, stat } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  CodexCliMissingError,
  codexAccountHome,
  codexConfigWithFileCredentialStore,
  connectFleetAccount,
  decodeCodexIdTokenEmail,
  linkFleetToKhala,
  listFleetAccounts,
  nextCodexAccountRef,
  parseCodexAccounts,
  pylonConfigPath,
  resolvePylonHome,
  upsertCodexAccount,
} from "./fleet.js"
import { readStoredAgentToken } from "./token-store.js"

function idTokenFor(email: string): string {
  const header = Buffer.from(JSON.stringify({ alg: "none" })).toString("base64url")
  const payload = Buffer.from(JSON.stringify({ email })).toString("base64url")
  return `${header}.${payload}.`
}

const noSleep = async (): Promise<void> => {}

const PENDING_LINK_BODY = {
  schema: "openagents.pylon.auth.openagents.v1",
  status: "pending",
  attemptId: "pylon_openauth_fleet_link",
  expiresAt: "2026-06-27T04:12:19.889Z",
  intervalSeconds: 2,
  userCode: "LINK-CODE",
  verificationUrl:
    "https://example.test/api/pylon/auth/openagents/device/verify?attempt=pylon_openauth_fleet_link&code=LINK-CODE",
  linkedAgent: { tokenPrefix: "oa_agent_link" },
}

describe("fleet ref assignment", () => {
  test("assigns codex, then codex-2, codex-3", () => {
    expect(nextCodexAccountRef([])).toBe("codex")
    expect(nextCodexAccountRef(["codex"])).toBe("codex-2")
    expect(nextCodexAccountRef(["codex", "codex-2"])).toBe("codex-3")
    expect(nextCodexAccountRef(["codex", "codex-3"])).toBe("codex-2")
  })
})

describe("config parsing + upsert (Pylon-compatible shape)", () => {
  test("parses only codex accounts with refs", () => {
    const config = {
      dev: {
        accounts: [
          { ref: "codex", provider: "codex", home: "/h/codex" },
          { ref: "claude", provider: "claude_agent", home: "/h/claude" },
          { provider: "codex" },
        ],
      },
    }
    expect(parseCodexAccounts(config)).toEqual([{ ref: "codex", home: "/h/codex" }])
  })

  test("upsert adds a new account and is idempotent", () => {
    const first = upsertCodexAccount({}, { ref: "codex", home: "/h/codex" })
    expect(first.changed).toBe(true)
    expect(first.config).toEqual({ dev: { accounts: [{ ref: "codex", provider: "codex", home: "/h/codex" }] } })
    const second = upsertCodexAccount(first.config, { ref: "codex", home: "/h/codex" })
    expect(second.changed).toBe(false)
    const moved = upsertCodexAccount(first.config, { ref: "codex", home: "/h/new" })
    expect(moved.changed).toBe(true)
    expect(parseCodexAccounts(moved.config)).toEqual([{ ref: "codex", home: "/h/new" }])
  })
})

describe("codex id_token email decode", () => {
  test("decodes a top-level email claim", () => {
    expect(decodeCodexIdTokenEmail(idTokenFor("alice@example.com"))).toBe("alice@example.com")
  })
  test("returns null on garbage", () => {
    expect(decodeCodexIdTokenEmail("not-a-jwt")).toBeNull()
  })
})

describe("file credential store config", () => {
  test("appends the file store line when missing", () => {
    expect(codexConfigWithFileCredentialStore("")).toBe('cli_auth_credentials_store = "file"\n')
    expect(codexConfigWithFileCredentialStore('model = "x"')).toBe(
      'model = "x"\ncli_auth_credentials_store = "file"\n',
    )
  })
  test("replaces an existing store line", () => {
    expect(codexConfigWithFileCredentialStore('cli_auth_credentials_store = "keychain"\n')).toBe(
      'cli_auth_credentials_store = "file"',
    )
  })
})

describe("pylon home resolution", () => {
  test("PYLON_HOME wins", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-home-"))
    expect(resolvePylonHome({ PYLON_HOME: "/explicit/home" }, base)).toBe("/explicit/home")
  })
  test("defaults to ~/.openagents/pylon on a fresh machine", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-home-"))
    expect(resolvePylonHome({}, base)).toBe(join(base, ".openagents", "pylon"))
  })
  test("prefers ~/.pylon when it already holds a config", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-home-"))
    await mkdir(join(base, ".pylon"), { recursive: true })
    await writeFile(join(base, ".pylon", "config.json"), "{}\n")
    expect(resolvePylonHome({}, base)).toBe(join(base, ".pylon"))
  })
})

describe("connect + status (with injected device login)", () => {
  test("connects, registers in Pylon config, never touches ~/.codex, and status lists it", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-"))
    const pylonHome = join(base, ".openagents", "pylon")
    const env = { PYLON_HOME: pylonHome }

    let loginHome: string | undefined
    const result = await connectFleetAccount({
      env,
      runDeviceLogin: async input => {
        loginHome = input.home
        // Simulate codex writing auth.json to the isolated account home.
        await mkdir(input.home, { recursive: true })
        await writeFile(
          join(input.home, "auth.json"),
          JSON.stringify({ tokens: { id_token: idTokenFor("fleet@example.com") } }),
        )
        return { exitCode: 0 }
      },
    })

    expect(result.accountRef).toBe("codex")
    expect(result.email).toBe("fleet@example.com")
    expect(result.status).toBe("connected")
    // Isolated per-account home; NEVER ~/.codex.
    expect(loginHome).toBe(codexAccountHome(pylonHome, "codex"))
    expect(loginHome).not.toContain(".codex/auth")

    // Registered into the Pylon config in the Pylon-compatible shape.
    const config = JSON.parse(await readFile(pylonConfigPath(pylonHome), "utf8"))
    expect(parseCodexAccounts(config)).toEqual([
      { ref: "codex", home: codexAccountHome(pylonHome, "codex") },
    ])

    // A second connect auto-assigns codex-2.
    const second = await connectFleetAccount({
      env,
      runDeviceLogin: async input => {
        await mkdir(input.home, { recursive: true })
        await writeFile(
          join(input.home, "auth.json"),
          JSON.stringify({ tokens: { id_token: idTokenFor("second@example.com") } }),
        )
        return { exitCode: 0 }
      },
    })
    expect(second.accountRef).toBe("codex-2")

    const status = await listFleetAccounts({ env })
    expect(status.readyCount).toBe(2)
    expect(status.accounts.map(a => a.accountRef)).toEqual(["codex", "codex-2"])
    expect(status.accounts.map(a => a.readiness)).toEqual(["ready", "ready"])

    // The isolated home got the file credential store config.
    const codexToml = await readFile(join(codexAccountHome(pylonHome, "codex"), "config.toml"), "utf8")
    expect(codexToml).toContain('cli_auth_credentials_store = "file"')
  }, 20000)

  test("surfaces a friendly error when the codex CLI is missing (exit 127)", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-"))
    const env = { PYLON_HOME: join(base, ".openagents", "pylon") }
    await expect(
      connectFleetAccount({ env, runDeviceLogin: async () => ({ exitCode: 127 }) }),
    ).rejects.toBeInstanceOf(CodexCliMissingError)
  })

  test("status reports credentials-missing for a registered home with no auth.json", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-"))
    const pylonHome = join(base, ".openagents", "pylon")
    const home = codexAccountHome(pylonHome, "codex")
    await mkdir(home, { recursive: true })
    const config = { dev: { accounts: [{ ref: "codex", provider: "codex", home }] } }
    await mkdir(pylonHome, { recursive: true })
    await writeFile(pylonConfigPath(pylonHome), JSON.stringify(config))
    const status = await listFleetAccounts({ env: { PYLON_HOME: pylonHome } })
    expect(status.accounts).toEqual([
      { accountRef: "codex", home, email: null, readiness: "credentials_missing", lastLinkedAt: null },
    ])
    void stat // keep import used across runtimes
  })
})

describe("fleet OpenAgents link", () => {
  test("links with the existing device-auth flow and stores the owner-linked token", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-link-"))
    const env = {
      KHALA_TOKEN_PATH: join(base, "khala-token"),
      PYLON_HOME: join(base, ".openagents", "pylon"),
    }
    const prompts: Array<{ expiresAt: string | undefined; userCode: string; verificationUrl: string }> = []
    const urls: Array<string> = []
    const authHeaders: Array<string | null> = []

    const fakeFetch = (async (url: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
      const target = String(url)
      authHeaders.push((init?.headers as Record<string, string> | undefined)?.authorization ?? null)
      urls.push(target)
      if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
        return Response.json(PENDING_LINK_BODY, { status: 201 })
      }
      if (target.includes("/api/pylon/auth/openagents/device/pylon_openauth_fleet_link")) {
        return Response.json(
          {
            schema: "openagents.pylon.auth.openagents.v1",
            status: "linked",
            linkedAgent: { tokenPrefix: "oa_agent_link" },
          },
          { status: 200 },
        )
      }
      if (target.endsWith("/api/agents/me")) {
        return Response.json(
          { authenticated: true, agent: { user: { displayName: "Fleet Owner", primaryEmail: "owner@example.com" } } },
          { status: 200 },
        )
      }
      throw new Error(`unexpected fetch: ${target}`)
    }) as unknown as typeof fetch

    const result = await linkFleetToKhala({
      baseUrl: "https://example.test",
      env,
      explicitToken: "oa_agent_link_token",
      fetch: fakeFetch,
      onPrompt: prompt => prompts.push(prompt),
      openBrowser: () => {},
      sleep: noSleep,
    })

    expect(prompts).toEqual([
      {
        expiresAt: PENDING_LINK_BODY.expiresAt,
        userCode: "LINK-CODE",
        verificationUrl: PENDING_LINK_BODY.verificationUrl,
      },
    ])
    expect(urls).toContain("https://example.test/api/pylon/auth/openagents/device/start")
    expect(authHeaders).toContain("Bearer oa_agent_link_token")
    expect(result).toEqual({
      alreadyLinked: false,
      displayName: "Fleet Owner",
      email: "owner@example.com",
      pylonHome: env.PYLON_HOME,
      tokenPrefix: "oa_agent_link",
    })
    expect(await readStoredAgentToken(env)).toBe("oa_agent_link_token")
  })

  test("reports an already-linked fleet without prompting", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-link-"))
    const env = {
      KHALA_TOKEN_PATH: join(base, "khala-token"),
      PYLON_HOME: join(base, ".openagents", "pylon"),
    }
    const prompts: Array<unknown> = []

    const fakeFetch = (async (url: Parameters<typeof fetch>[0]) => {
      const target = String(url)
      if (target.endsWith("/api/pylon/auth/openagents/device/start")) {
        return Response.json(
          {
            schema: "openagents.pylon.auth.openagents.v1",
            status: "linked",
            linkedAgent: { tokenPrefix: "oa_agent_owner" },
          },
          { status: 200 },
        )
      }
      if (target.endsWith("/api/agents/me")) {
        return Response.json(
          { authenticated: true, agent: { user: { displayName: "Fleet Owner" } } },
          { status: 200 },
        )
      }
      throw new Error(`unexpected fetch: ${target}`)
    }) as unknown as typeof fetch

    const result = await linkFleetToKhala({
      baseUrl: "https://example.test",
      env,
      explicitToken: "oa_agent_owner_token",
      fetch: fakeFetch,
      onPrompt: prompt => prompts.push(prompt),
      openBrowser: () => {},
      sleep: noSleep,
    })

    expect(prompts).toHaveLength(0)
    expect(result.alreadyLinked).toBe(true)
    expect(result.displayName).toBe("Fleet Owner")
    expect(result.tokenPrefix).toBe("oa_agent_owner")
  })
})
