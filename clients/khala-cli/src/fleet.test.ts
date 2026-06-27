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
  listFleetAccounts,
  nextCodexAccountRef,
  planFleetRun,
  parseCodexAccounts,
  pylonConfigPath,
  resolvePylonHome,
  runFleetSupervisor,
  upsertCodexAccount,
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
})

describe("fleet run planning and dispatch", () => {
  async function writeReadyFleet(pylonHome: string, refs: ReadonlyArray<string>): Promise<void> {
    const accounts = []
    for (const ref of refs) {
      const home = codexAccountHome(pylonHome, ref)
      await mkdir(home, { recursive: true })
      await writeFile(
        join(home, "auth.json"),
        JSON.stringify({ tokens: { id_token: idTokenFor(`${ref}@example.com`) } }),
      )
      accounts.push({ ref, provider: "codex", home })
    }
    await mkdir(pylonHome, { recursive: true })
    await writeFile(pylonConfigPath(pylonHome), JSON.stringify({ dev: { accounts } }))
  }

  test("plans tenant fleet work from ready accounts and auto-resolved pylon ref", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-run-plan-"))
    const pylonHome = join(base, ".openagents", "pylon")
    await writeReadyFleet(pylonHome, ["codex", "codex-2"])
    const commands: string[] = []

    const plan = await planFleetRun({
      commit: "0123456789abcdef0123456789abcdef01234567",
      env: { PYLON_HOME: pylonHome },
      issues: [6384, 6385],
      maxSlots: 8,
      perAccount: 2,
      repo: "ExampleCo/example",
      runner: async input => {
        commands.push(input.command.join(" "))
        return { exitCode: 0, stdout: JSON.stringify({ pylonRef: "pylon.tenant.local" }), stderr: "" }
      },
      verify: "bun test",
    })

    expect(plan.pylonRef).toBe("pylon.tenant.local")
    expect(plan.desiredSlots).toBe(4)
    expect(plan.readyAccounts.map(account => account.accountRef)).toEqual(["codex", "codex-2"])
    expect(commands).toEqual(["pylon provider go-online --json"])
  })

  test("dry-run returns a plan without dispatching assignments", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-run-dry-"))
    const pylonHome = join(base, ".openagents", "pylon")
    await writeReadyFleet(pylonHome, ["codex"])
    const commands: string[] = []

    const result = await runFleetSupervisor({
      commit: "0123456789abcdef0123456789abcdef01234567",
      dryRun: true,
      env: { PYLON_HOME: pylonHome },
      issues: [6384],
      repo: "ExampleCo/example",
      runner: async input => {
        commands.push(input.command.join(" "))
        return { exitCode: 0, stdout: JSON.stringify({ pylonRef: "pylon.tenant.local" }), stderr: "" }
      },
      verify: "bun test",
    })

    expect(result.dryRun).toBe(true)
    expect(result.dispatched).toEqual([])
    expect(commands).toEqual(["pylon provider go-online --json"])
  })

  test("one refill cycle publishes capacity and round-robins account refs", async () => {
    const base = await mkdtemp(join(tmpdir(), "khala-fleet-run-dispatch-"))
    const pylonHome = join(base, ".openagents", "pylon")
    await writeReadyFleet(pylonHome, ["codex", "codex-2"])
    const commands: string[] = []

    const result = await runFleetSupervisor({
      commit: "0123456789abcdef0123456789abcdef01234567",
      env: { PYLON_HOME: pylonHome },
      issues: [6384, 6385],
      maxSlots: 2,
      perAccount: 2,
      pylonRef: "pylon.tenant.local",
      repo: "ExampleCo/example",
      runner: async input => {
        commands.push(input.command.join(" "))
        if (input.command.includes("heartbeat")) {
          expect(input.env.OPENAGENTS_PYLON_CODEX_CONCURRENCY).toBe("2")
          return { exitCode: 0, stdout: JSON.stringify({ ok: true }), stderr: "" }
        }
        const issue = input.command[input.command.indexOf("--prompt") + 1]?.match(/#(\d+)/)?.[1] ?? "unknown"
        return {
          exitCode: 0,
          stdout: JSON.stringify({
            assignmentRef: `assignment.public.${issue}`,
            durableRequestId: `chatcmpl_${issue}`,
          }),
          stderr: "",
        }
      },
      verify: "bun test",
    })

    expect(result.dispatched.map(entry => [entry.issue, entry.accountRef, entry.assignmentRef])).toEqual([
      [6384, "codex", "assignment.public.6384"],
      [6385, "codex-2", "assignment.public.6385"],
    ])
    expect(commands[0]).toBe("pylon presence heartbeat --json")
    expect(commands[1]).toContain("--account-ref codex")
    expect(commands[2]).toContain("--account-ref codex-2")
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
