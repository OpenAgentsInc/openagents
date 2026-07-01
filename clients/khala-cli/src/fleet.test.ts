import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, writeFile, mkdir, stat } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  ClaudeCliMissingError,
  CodexCliMissingError,
  claudeAccountHome,
  codexAccountHome,
  codexConfigWithFileCredentialStore,
  connectFleetAccount,
  decodeCodexIdTokenEmail,
  fetchOperatorFleetStatus,
  formatOperatorFleetDashboard,
  linkFleetPylon,
  listFleetAccounts,
  nextFleetAccountRef,
  nextCodexAccountRef,
  parseCodexAccounts,
  parseFleetAccounts,
  pylonConfigPath,
  resolvePylonHome,
  upsertCodexAccount,
  upsertFleetAccount,
} from "./fleet.js"

function idTokenFor(email: string): string {
  const header = Buffer.from(JSON.stringify({ alg: "none" })).toString("base64url")
  const payload = Buffer.from(JSON.stringify({ email })).toString("base64url")
  return `${header}.${payload}.`
}

describe("fleet ref assignment", () => {
  test("assigns codex, then codex-2, codex-3", () => {
    expect(nextCodexAccountRef([])).toBe("codex")
    expect(nextCodexAccountRef(["codex"])).toBe("codex-2")
    expect(nextCodexAccountRef(["codex", "codex-2"])).toBe("codex-3")
    expect(nextCodexAccountRef(["codex", "codex-3"])).toBe("codex-2")
  })

  test("assigns claude, then claude-2, claude-3", () => {
    expect(nextFleetAccountRef("claude", [])).toBe("claude")
    expect(nextFleetAccountRef("claude", ["claude"])).toBe("claude-2")
    expect(nextFleetAccountRef("claude", ["claude", "claude-2"])).toBe("claude-3")
  })
})

describe("fleet Pylon link", () => {
  test("links the local Pylon to the signed-in Khala token without printing or reading Codex credentials", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-link-"))
    const pylonHome = join(base, ".openagents", "pylon")
    const tokenPath = join(base, "khala-token")
    const env = { PYLON_HOME: pylonHome, KHALA_TOKEN_PATH: tokenPath }
    await mkdir(pylonHome, { recursive: true })
    await writeFile(tokenPath, "oa_agent_owner_link\n")

    const codexHome = codexAccountHome(pylonHome, "codex")
    await mkdir(codexHome, { recursive: true })
    await writeFile(join(codexHome, "auth.json"), JSON.stringify({ tokens: { id_token: idTokenFor("fleet@example.com") } }))
    await writeFile(
      pylonConfigPath(pylonHome),
      JSON.stringify({ dev: { accounts: [{ ref: "codex", provider: "codex", home: codexHome }] } }),
    )

    const requests: Array<{ url: string; init: RequestInit; body: Record<string, unknown> }> = []
    const result = await linkFleetPylon({
      env,
      baseUrl: "https://openagents.test",
      fetch: (async (url: Parameters<typeof fetch>[0], init: Parameters<typeof fetch>[1]) => {
        const requestInit = init ?? {}
        requests.push({
          url: String(url),
          init: requestInit,
          body: JSON.parse(String(requestInit.body)) as Record<string, unknown>,
        })
        return new Response(JSON.stringify({ registrationRef: "registration.pylon.linked" }), { status: 201 })
      }) as unknown as typeof fetch,
    })

    expect(result.linked).toBe(true)
    expect(result.registrationRef).toBe("registration.pylon.linked")
    expect(result.pylonRef).toStartWith("pylon.")
    expect(result.publicKey).toMatch(/^[0-9a-f]{64}$/)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.url).toBe("https://openagents.test/api/pylons/register")
    expect((requests[0]?.init.headers as Record<string, string>).authorization).toBe("Bearer oa_agent_owner_link")
    expect(requests[0]?.body).toMatchObject({
      schema: "openagents.pylon.register.v0.3",
      pylonRef: result.pylonRef,
      providerNostrPubkey: result.publicKey,
      statusRefs: ["status.public.khala_fleet_linked"],
    })
    expect(requests[0]?.body.capabilityRefs).toContain("capability.pylon.local_codex")
    expect(JSON.stringify(requests[0]?.body)).not.toContain("fleet@example.com")
    expect(JSON.stringify(requests[0]?.body)).not.toContain(codexHome)
  })

  test("requires khala login before linking", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-link-"))
    await expect(
      linkFleetPylon({
        env: { PYLON_HOME: join(base, ".openagents", "pylon"), KHALA_TOKEN_PATH: join(base, "missing-token") },
        fetch: (async () => new Response("{}", { status: 201 })) as unknown as typeof fetch,
      }),
    ).rejects.toThrow("khala fleet link requires a signed-in Khala account")
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
    expect(parseFleetAccounts(config)).toEqual([
      { ref: "codex", provider: "codex", harness: "codex", home: "/h/codex" },
      { ref: "claude", provider: "claude_agent", harness: "claude", home: "/h/claude" },
    ])
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

  test("upsert adds a claude_agent account and is idempotent", () => {
    const first = upsertFleetAccount({}, { provider: "claude_agent", ref: "claude", home: "/h/.claude-claude" })
    expect(first.changed).toBe(true)
    expect(first.config).toEqual({
      dev: { accounts: [{ ref: "claude", provider: "claude_agent", home: "/h/.claude-claude" }] },
    })
    const second = upsertFleetAccount(first.config, { provider: "claude_agent", ref: "claude", home: "/h/.claude-claude" })
    expect(second.changed).toBe(false)
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
    expect(status.accounts.map(a => a.harness)).toEqual(["codex", "codex"])
    expect(status.accounts.map(a => a.readiness)).toEqual(["ready", "ready"])

    // The isolated home got the file credential store config.
    const codexToml = await readFile(join(codexAccountHome(pylonHome, "codex"), "config.toml"), "utf8")
    expect(codexToml).toContain('cli_auth_credentials_store = "file"')
  }, 20000)

  test("connects a Claude account with setup-token in an isolated CLAUDE_CONFIG_DIR home", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-claude-"))
    const pylonHome = join(base, ".openagents", "pylon")
    const env = { PYLON_HOME: pylonHome }

    let setupHome: string | undefined
    let setupEnv: Record<string, string | undefined> | undefined
    const result = await connectFleetAccount({
      env,
      harness: "claude",
      runClaudeSetupToken: async input => {
        setupHome = input.home
        setupEnv = input.env
        return { exitCode: 0, stdout: "sk-ant-oat-test-token\n" }
      },
    })

    expect(result.accountRef).toBe("claude")
    expect(result.status).toBe("connected")
    expect(setupHome).toBe(claudeAccountHome(pylonHome, "claude"))
    expect(setupHome).toContain(".claude-claude")
    expect(setupHome).not.toBe(join(base, ".claude"))
    expect(setupEnv).toEqual(env)

    const token = await readFile(join(claudeAccountHome(pylonHome, "claude"), "claude-oauth-token"), "utf8")
    expect(token).toBe("sk-ant-oat-test-token\n")

    const config = JSON.parse(await readFile(pylonConfigPath(pylonHome), "utf8"))
    expect(parseFleetAccounts(config)).toEqual([
      {
        ref: "claude",
        provider: "claude_agent",
        harness: "claude",
        home: claudeAccountHome(pylonHome, "claude"),
      },
    ])

    const second = await connectFleetAccount({
      env,
      harness: "claude",
      runClaudeSetupToken: async () => ({ exitCode: 0, stdout: "sk-ant-oat-second\n" }),
    })
    expect(second.accountRef).toBe("claude-2")

    const status = await listFleetAccounts({ env })
    expect(status.readyCount).toBe(2)
    expect(status.accounts.map(account => ({
      harness: account.harness,
      readiness: account.readiness,
      ref: account.accountRef,
    }))).toEqual([
      { ref: "claude", harness: "claude", readiness: "ready" },
      { ref: "claude-2", harness: "claude", readiness: "ready" },
    ])
  })

  test("surfaces a friendly error when the codex CLI is missing (exit 127)", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-"))
    const env = { PYLON_HOME: join(base, ".openagents", "pylon") }
    await expect(
      connectFleetAccount({ env, runDeviceLogin: async () => ({ exitCode: 127 }) }),
    ).rejects.toBeInstanceOf(CodexCliMissingError)
  })

  test("surfaces a friendly error when the claude CLI is missing (exit 127)", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-claude-"))
    const env = { PYLON_HOME: join(base, ".openagents", "pylon") }
    await expect(
      connectFleetAccount({ env, harness: "claude", runClaudeSetupToken: async () => ({ exitCode: 127 }) }),
    ).rejects.toBeInstanceOf(ClaudeCliMissingError)
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
      {
        accountRef: "codex",
        harness: "codex",
        provider: "codex",
        home,
        email: null,
        readiness: "credentials_missing",
        lastLinkedAt: null,
      },
    ])
    void stat // keep import used across runtimes
  })

  test("status reports credentials-missing for a registered Claude home with no setup token", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-claude-"))
    const pylonHome = join(base, ".openagents", "pylon")
    const home = claudeAccountHome(pylonHome, "claude")
    await mkdir(home, { recursive: true })
    const config = { dev: { accounts: [{ ref: "claude", provider: "claude_agent", home }] } }
    await mkdir(pylonHome, { recursive: true })
    await writeFile(pylonConfigPath(pylonHome), JSON.stringify(config))
    const status = await listFleetAccounts({ env: { PYLON_HOME: pylonHome } })
    expect(status.accounts).toEqual([
      {
        accountRef: "claude",
        harness: "claude",
        provider: "claude_agent",
        home,
        email: null,
        readiness: "credentials_missing",
        lastLinkedAt: null,
      },
    ])
  })
})

describe("operator fleet live status", () => {
  test("fetches the owner operator fleet status endpoint with bearer auth", async () => {
    const seenAuth: Array<string | null> = []
    const snapshot = await fetchOperatorFleetStatus({
      baseUrl: "https://example.test/",
      token: "oa_agent_live_test",
      fetch: async (input, init) => {
        seenAuth.push(init?.headers instanceof Headers
          ? init.headers.get("authorization")
          : (init?.headers as Record<string, string> | undefined)?.authorization ?? null)
        expect(String(input)).toBe("https://example.test/api/operator/fleet/state")
        return Response.json({ pace: { burnRate: 12 }, fleet: { ready: 2 } })
      },
    })

    expect(seenAuth).toEqual(["Bearer oa_agent_live_test"])
    expect(snapshot.payload).toEqual({ pace: { burnRate: 12 }, fleet: { ready: 2 } })
  })

  test("renders the five operator dashboard blocks", () => {
    const rendered = formatOperatorFleetDashboard({
      baseUrl: "https://example.test",
      fetchedAt: "2026-06-27T00:00:00.000Z",
      payload: {
        pace: { burnRate: "1.2k tokens/min", paceToFloor: "above floor" },
        fleet: { concurrency: 4, inFlightIssues: ["#6429"] },
        watchdog: { state: "healthy", leases: 1 },
        glmFleetStatus: { status: "ready", readyReplicas: 8, totalReplicas: 8 },
        brain: { state: "running", recentDecisions: ["dispatch issue #6429"] },
      },
    })

    expect(rendered).toContain("Khala fleet live dashboard")
    expect(rendered).toContain("[Pace]")
    expect(rendered).toContain("burnRate: 1.2k tokens/min")
    expect(rendered).toContain("[Fleet]")
    expect(rendered).toContain("[Watchdog]")
    expect(rendered).toContain("[GLM]")
    expect(rendered).toContain("[Brain]")
  })
})

describe("claude setup-token output guard", () => {
  test("fails loudly when no token-shaped line is present instead of storing junk", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-claude-junk-"))
    const pylonHome = join(base, ".openagents", "pylon")
    await expect(
      connectFleetAccount({
        env: { PYLON_HOME: pylonHome },
        harness: "claude",
        runClaudeSetupToken: async () => ({ exitCode: 0, stdout: "Login successful\n" }),
      }),
    ).rejects.toThrow(/claude-oauth-token was not written/)
  })
})
