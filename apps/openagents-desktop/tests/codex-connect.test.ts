/**
 * Main-process codex-connect unit tests (#8574, #8640 unblock): the
 * device-auth stdout parser against the exact pylon CLI output format, the
 * accounts-list JSON projection, the bridge contract schemas (main side and
 * renderer side must agree), and the service lifecycle with fake children.
 */
import { describe, expect, test } from "bun:test"

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
  parseAccountsListJson,
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
