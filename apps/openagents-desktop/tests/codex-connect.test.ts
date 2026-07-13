/**
 * Main-process codex-connect unit tests (#8574, #8640 unblock): the
 * device-auth stdout parser against the exact pylon CLI output format, the
 * accounts-list JSON projection, the bridge contract schemas (main side and
 * renderer side must agree), and the service lifecycle with fake children.
 */
import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { homedir, tmpdir } from "node:os"
import path from "node:path"

import {
  decodeCodexAccountsResult,
  decodeCodexConnectStatus,
} from "../src/codex-connect-contract.ts"
import {
  createDeviceAuthStdoutParser,
  fixtureAccountsListStdout,
  fixtureDeviceAuthStdout,
  makeCodexConnectService,
  makeFixtureSpawnPylon,
  makeInstalledCodexCustody,
  parseAccountsListJson,
  publicSafeFailureDetail,
} from "../src/codex-connect.ts"
import {
  decodeAccountsView,
  decodeConnectStatusView,
} from "../src/renderer/settings.ts"
import type { CodexConnectServiceDependencies } from "../src/codex-connect.ts"

/**
 * Exact device-auth output format of `pylon auth codex` (non-JSON mode):
 * apps/pylon/src/index.ts prints `${verificationUrl}\n${userCode}\n` when the
 * device prompt fires, then a `✓ Linked/Re-authenticated ... (ref)` success
 * line. Failures print a `⚠ ...` line on stderr and exit non-zero.
 */
const deviceAuthSuccessFixture =
  "https://auth.openai.com/codex/device\n" +
  "8260-DUG55\n" +
  "✓ Linked Codex account: owner@example.com (codex-4)\n"

type FakeChild = NonNullable<ReturnType<NonNullable<CodexConnectServiceDependencies["spawnPylon"]>>>

const makeFakeChild = (): FakeChild & {
  killCount: number
  emitStdout: (chunk: string) => void
  emitStderr: (chunk: string) => void
  emitClose: (code: number | null) => void
} => {
  const listeners = new Map<string, Array<(...values: unknown[]) => void>>()
  const stdoutHandlers: Array<(chunk: string) => void> = []
  const stderrHandlers: Array<(chunk: string) => void> = []
  const child: FakeChild & {
    killCount: number
    emitStdout: (chunk: string) => void
    emitStderr: (chunk: string) => void
    emitClose: (code: number | null) => void
  } = {
    stdout: {
      on: (event: string, listener: (chunk: string) => void) => {
        if (event === "data") stdoutHandlers.push(listener)
      },
    } as unknown as NodeJS.ReadableStream,
    stderr: {
      on: (event: string, listener: (chunk: string) => void) => {
        if (event === "data") stderrHandlers.push(listener)
      },
    } as unknown as NodeJS.ReadableStream,
    on: (event: "close" | "error", listener: (...args: unknown[]) => void) => {
      const existing = listeners.get(event) ?? []
      listeners.set(event, [...existing, listener])
      return child
    },
    kill: () => {
      child.killCount++
      child.killed = true
      return true
    },
    killCount: 0,
    killed: false,
    exitCode: null,
    emitStdout: (chunk: string) => {
      for (const handler of stdoutHandlers) handler(chunk)
    },
    emitStderr: (chunk: string) => {
      for (const handler of stderrHandlers) handler(chunk)
    },
    emitClose: (code: number | null) => {
      child.exitCode = code
      for (const listener of listeners.get("close") ?? []) listener(code)
    },
  }
  return child
}

describe("createDeviceAuthStdoutParser (pylon auth codex output)", () => {
  test("parses the URL + user-code pair then the connected ref; the email never leaks", () => {
    const parser = createDeviceAuthStdoutParser()
    const events = parser.feed(deviceAuthSuccessFixture)
    expect(events).toEqual([
      {
        kind: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "8260-DUG55",
      },
      { kind: "connected", ref: "codex-4" },
    ])
    expect(JSON.stringify(events)).not.toContain("owner@example.com")
  })

  test("handles chunk boundaries mid-line (streamed stdio)", () => {
    const parser = createDeviceAuthStdoutParser()
    const chunks = ["https://auth.openai.com/co", "dex/device\n8260-", "DUG55\n"]
    const events = chunks.flatMap((chunk) => parser.feed(chunk))
    expect(events).toEqual([
      {
        kind: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "8260-DUG55",
      },
    ])
  })

  test("re-authenticated wording also yields connected", () => {
    const parser = createDeviceAuthStdoutParser()
    const events = parser.feed("✓ Re-authenticated Codex account: o@x.com (codex-2)\n")
    expect(events).toEqual([{ kind: "connected", ref: "codex-2" }])
  })

  test("invalid-credentials warning yields a typed failure without raw text", () => {
    const parser = createDeviceAuthStdoutParser()
    const events = parser.feed(
      "⚠ Codex account codex-2 has invalid credentials; automatic re-login did not complete.\n",
    )
    expect(events).toEqual([
      { kind: "failed", reason: "credentials_invalid_relogin_incomplete" },
    ])
  })

  test("ANSI-wrapped output still parses (defense-in-depth)", () => {
    const parser = createDeviceAuthStdoutParser()
    const esc = String.fromCharCode(27)
    const events = parser.feed(
      `https://auth.openai.com/codex/device\n${esc}[94m8260-DUG55${esc}[0m\n`,
    )
    expect(events).toEqual([
      {
        kind: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "8260-DUG55",
      },
    ])
  })

  test("end() flushes an unterminated trailing line", () => {
    const parser = createDeviceAuthStdoutParser()
    parser.feed("https://auth.openai.com/codex/device\n8260-DUG55\n")
    const events = parser.end()
    expect(events).toEqual([])
    const parser2 = createDeviceAuthStdoutParser()
    parser2.feed("✓ Linked Codex account: o@x.com (codex-9)")
    expect(parser2.end()).toEqual([{ kind: "connected", ref: "codex-9" }])
  })
})

describe("parseAccountsListJson (pylon codex accounts list --json)", () => {
  test("projects codex refs + readiness states only", () => {
    const result = parseAccountsListJson(fixtureAccountsListStdout)
    expect(result).toEqual({
      state: "ok",
      accounts: [
        { ref: "codex-2", readiness: "credentials_revoked" },
        { ref: "codex-b7d4438c", readiness: "credentials_revoked" },
      ],
    })
  })

  test("non-JSON and shape-less payloads degrade to unavailable", () => {
    expect(parseAccountsListJson("not json").state).toBe("unavailable")
    expect(parseAccountsListJson("{}").state).toBe("unavailable")
  })
})

describe("bridge contract schemas (main side; must agree with renderer decode)", () => {
  test("status round trip: main decode and renderer decode accept the same payloads", () => {
    const payloads = [
      { state: "idle" },
      { state: "starting" },
      { state: "awaiting_browser", url: "https://auth.openai.com/codex/device", code: "8260-DUG55" },
      { state: "connected", ref: "codex-4" },
      { state: "failed", reason: "device_auth_timeout" },
    ]
    for (const payload of payloads) {
      expect(decodeCodexConnectStatus(payload)).toEqual(payload as never)
      expect(decodeConnectStatusView(payload)).toEqual(payload as never)
    }
  })

  test("both sides reject a non-https verification URL identically", () => {
    const bad = { state: "awaiting_browser", url: "http://evil.example", code: "8260-DUG55" }
    expect(decodeCodexConnectStatus(bad)).toEqual({
      state: "failed",
      reason: "invalid_verification_url",
    })
    expect(decodeConnectStatusView(bad)).toEqual({
      state: "failed",
      reason: "invalid_verification_url",
    })
  })

  test("accounts round trip agrees across the bridge", () => {
    const payload = {
      state: "ok",
      accounts: [{ ref: "codex-2", readiness: "credentials_revoked" }],
    }
    expect(decodeCodexAccountsResult(payload)).toEqual(payload as never)
    expect(decodeAccountsView(payload)).toEqual({
      state: "loaded",
      accounts: payload.accounts,
    })
    expect(decodeCodexAccountsResult(null).state).toBe("unavailable")
  })
})

describe("makeCodexConnectService (fake children)", () => {
  test("fixture spawn: listAccounts projects, start reaches awaiting_browser, single-flight holds", async () => {
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: makeFixtureSpawnPylon(),
    })
    expect(await service.listAccounts()).toEqual({
      state: "ok",
      accounts: [
        { ref: "codex-2", readiness: "credentials_revoked" },
        { ref: "codex-b7d4438c", readiness: "credentials_revoked" },
      ],
    })
    expect(service.start().state).toBe("starting")
    await new Promise((resolve) => setTimeout(resolve, 10))
    const status = service.status()
    expect(status).toEqual({
      state: "awaiting_browser",
      url: "https://auth.openai.com/codex/device",
      code: "1234-ABCDE",
    })
    // second start while live returns the in-flight status (no second child)
    expect(service.start()).toEqual(status)
    service.dispose()
  })

  test("openVerification opens ONLY the main-held URL and only while awaiting the browser", async () => {
    const openedUrls: Array<string> = []
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: makeFixtureSpawnPylon(),
      openExternal: async (url) => {
        openedUrls.push(url)
      },
    })
    expect(await service.openVerification()).toBe(false)
    service.start()
    await new Promise((resolve) => setTimeout(resolve, 10))
    expect(await service.openVerification()).toBe(true)
    expect(openedUrls).toEqual(["https://auth.openai.com/codex/device"])
    service.dispose()
  })

  test("unavailable pylon runtime yields typed unavailable/failed, never a throw", async () => {
    const service = makeCodexConnectService("/nonexistent", { spawnPylon: () => null })
    expect((await service.listAccounts()).state).toBe("unavailable")
    expect(service.start()).toEqual({ state: "failed", reason: "pylon_runtime_unavailable" })
  })

  test("dispose settles and kills list/device children once and is terminal", async () => {
    const listChild = makeFakeChild()
    const deviceChild = makeFakeChild()
    let spawnCount = 0
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: args => {
        spawnCount++
        return args.includes("list") ? listChild : deviceChild
      },
      listTimeoutMs: 60_000,
      connectTimeoutMs: 60_000,
    })
    const listed = service.listAccounts()
    expect(service.start()).toEqual({ state: "starting" })

    service.dispose()
    service.dispose()
    expect((await listed).state).toBe("unavailable")
    expect(listChild.killCount).toBe(1)
    expect(deviceChild.killCount).toBe(1)
    expect(service.status()).toEqual({ state: "failed", reason: "pylon_runtime_unavailable" })
    expect(service.start()).toEqual({ state: "failed", reason: "pylon_runtime_unavailable" })
    expect((await service.listAccounts()).state).toBe("unavailable")
    expect(spawnCount).toBe(2)
  })

  test("fixture device-auth stdout matches the real CLI prompt format", () => {
    const parser = createDeviceAuthStdoutParser()
    expect(parser.feed(fixtureDeviceAuthStdout)).toEqual([
      {
        kind: "awaiting_browser",
        url: "https://auth.openai.com/codex/device",
        code: "1234-ABCDE",
      },
    ])
  })

  test("ready isolated registration wins over earlier generic stderr failure and close ordering", () => {
    const child = makeFakeChild()
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: () => child,
    })

    expect(service.start()).toEqual({ state: "starting" })
    child.emitStdout("✓ Linked Codex account: owner@example.com (codex-4)")
    child.emitStderr("Pylon auth failed: stale generic wrapper warning\n")
    child.emitClose(1)

    const status = service.status()
    expect(status).toEqual({ state: "connected", ref: "codex-4" })
    expect(JSON.stringify(status)).not.toContain("owner@example.com")
  })
})

describe("installed package Codex custody", () => {
  test("connects and lists a named isolated account without a source checkout or Bun CLI", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-installed-codex-"))
    const spawns: Array<Readonly<{
      executable: string
      args: ReadonlyArray<string>
      env: Record<string, string | undefined>
    }>> = []
    try {
      const custody = makeInstalledCodexCustody({
        env: { PYLON_HOME: root, PATH: "" },
        resolveCodex: () => "/Applications/OpenAgents.app/Contents/Resources/app.asar.unpacked/node_modules/@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex",
        spawnCodex: input => {
          spawns.push(input)
          const child = makeFakeChild()
          queueMicrotask(() => {
            const home = input.env.CODEX_HOME!
            mkdirSync(home, { recursive: true })
            writeFileSync(path.join(home, "auth.json"), "{}")
            child.emitStdout("Open this URL: https://auth.openai.com/codex/device\n")
            child.emitStdout("Enter \u001b[94m8260-DUG55\u001b[0m\n")
            child.emitClose(0)
          })
          return child
        },
      })
      const service = makeCodexConnectService(
        "/Applications/OpenAgents.app/Contents/Resources/app.asar/dist",
        { installedCustody: custody },
      )
      expect(await service.listAccounts()).toEqual({ state: "ok", accounts: [] })
      expect(service.start()).toEqual({ state: "starting" })
      await new Promise(resolve => setTimeout(resolve, 20))
      expect(service.status()).toEqual({ state: "connected", ref: "codex" })
      expect(await service.listAccounts()).toEqual({
        state: "ok",
        accounts: [{ ref: "codex", readiness: "ready" }],
      })
      expect(spawns).toHaveLength(1)
      expect(spawns[0]?.args).toEqual(["login", "--device-auth"])
      expect(spawns[0]?.executable).toContain("app.asar.unpacked/node_modules/@openai/codex-darwin-arm64")
      expect(spawns[0]?.env.CODEX_HOME).toBe(path.join(root, "accounts", "codex", "codex"))
      expect(spawns[0]?.env.CODEX_HOME).not.toBe(path.join(homedir(), ".codex"))
      const config = readFileSync(path.join(root, "config.json"), "utf8")
      expect(config).toContain('"ref": "codex"')
      expect(config).toContain(path.join(root, "accounts", "codex", "codex"))
      expect(config).not.toContain("apps/pylon/src/index.ts")
      service.dispose()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("the installed default fails explicitly when the packaged Codex executable is absent", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-installed-codex-missing-"))
    try {
      const service = makeCodexConnectService("/Applications/OpenAgents.app/Contents/Resources/app.asar/dist", {
        installedCustody: makeInstalledCodexCustody({
          env: { PYLON_HOME: root },
          resolveCodex: () => null,
        }),
      })
      expect(service.start()).toEqual({ state: "starting" })
      await new Promise(resolve => setTimeout(resolve, 20))
      expect(service.status()).toMatchObject({ state: "failed" })
      expect(JSON.stringify(service.status())).not.toContain("apps/pylon")
      service.dispose()
    } finally {
      rmSync(root, { recursive: true, force: true })
    }
  })
})

describe("pylon-auth failure detail (EP250: the bare token hid the real reason)", () => {
  test("captures the bounded public-safe detail after `Pylon auth failed:`", () => {
    const parser = createDeviceAuthStdoutParser()
    const events = parser.feed(
      "Pylon auth failed: Unable to connect. Is the computer able to access the url?\n",
    )
    expect(events).toEqual([
      {
        kind: "failed",
        reason: "pylon_auth_failed: Unable to connect. Is the computer able to access the url?",
      },
    ])
  })

  test("redacts emails, home paths, and token-like material from the detail", () => {
    const parser = createDeviceAuthStdoutParser()
    const events = parser.feed(
      "Pylon auth failed: owner@example.com at /Users/owner/.openagents/pylon token oa_agent_abc123\n",
    )
    expect(events).toHaveLength(1)
    const failed = events[0] as { kind: "failed"; reason: string }
    expect(failed.reason).not.toContain("owner@example.com")
    expect(failed.reason).not.toContain("/Users/owner")
    expect(failed.reason).not.toContain("oa_agent_abc123")
    expect(failed.reason).toContain("<email>")
    expect(failed.reason).toContain("<path>")
    expect(failed.reason).toContain("<redacted>")
  })

  test("a detail-less line degrades to the bare typed token", () => {
    const parser = createDeviceAuthStdoutParser()
    expect(parser.feed("Pylon auth failed\n")).toEqual([
      { kind: "failed", reason: "pylon_auth_failed" },
    ])
  })

  test("publicSafeFailureDetail bounds output to 100 chars", () => {
    expect(publicSafeFailureDetail("x".repeat(500)).length).toBe(100)
  })
})

describe("startReconnect (EP250: UI-owned per-ref re-auth)", () => {
  test("spawns the receipted per-ref recipe against a ref main itself listed", async () => {
    const spawnedArgs: Array<ReadonlyArray<string>> = []
    const fixture = makeFixtureSpawnPylon()
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: args => {
        spawnedArgs.push(args)
        return fixture(args)
      },
    })
    // Main must list first: reconnect only accepts refs from its own read.
    await service.listAccounts()
    const started = service.startReconnect("codex-2")
    expect(started.state).toBe("starting")
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(service.status()).toEqual({
      state: "awaiting_browser",
      url: "https://auth.openai.com/codex/device",
      code: "1234-ABCDE",
    })
    // Receipt: pylon auth codex --account <ref> --force-device-login re-auths
    // the SAME ref into its existing isolated home (apps/pylon/src/auth.ts).
    expect(spawnedArgs).toEqual([
      ["codex", "accounts", "list", "--json"],
      ["auth", "codex", "--account", "codex-2", "--force-device-login"],
    ])
    service.dispose()
  })

  test("refuses a ref main never listed and a malformed ref, typed", async () => {
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: makeFixtureSpawnPylon(),
    })
    await service.listAccounts()
    expect(service.startReconnect("codex-999")).toEqual({
      state: "failed",
      reason: "unknown_account_ref",
    })
    expect(service.startReconnect("../escape")).toEqual({
      state: "failed",
      reason: "invalid_account_ref",
    })
    service.dispose()
  })

  test("single flight holds across connect and reconnect", async () => {
    const service = makeCodexConnectService("/nonexistent", {
      spawnPylon: makeFixtureSpawnPylon(),
    })
    await service.listAccounts()
    service.startReconnect("codex-2")
    await new Promise(resolve => setTimeout(resolve, 10))
    const live = service.status()
    expect(live.state).toBe("awaiting_browser")
    expect(service.start()).toEqual(live)
    expect(service.startReconnect("codex-b7d4438c")).toEqual(live)
    service.dispose()
  })

  test("re-authenticated CLI success line yields connected for the SAME ref", () => {
    const parser = createDeviceAuthStdoutParser()
    expect(parser.feed("\u2713 Re-authenticated Codex account: owner@example.com (codex-2)\n")).toEqual([
      { kind: "connected", ref: "codex-2" },
    ])
  })
})
