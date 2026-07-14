/**
 * Main-process provider-accounts host adapter tests (#8712): the accounts-list
 * and usage JSON projections against a checked-in public-safe fixture, the
 * typed failure paths (never a throw across IPC), the bridge contract schemas
 * (main side and renderer side must agree), and the service with fake
 * children.
 */
import { describe, expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  decodeProviderAccountUsageRequest,
  decodeProviderAccountsListResult,
  decodeProviderAccountUsageResult,
  unavailableProviderAccountsListResult,
} from "../src/provider-accounts-contract.ts"
import {
  fixtureProviderAccountsListStdout,
  fixtureProviderAccountUsageStdout,
  makeFixtureProviderAccountsSpawn,
  makeProviderAccountsService,
  isPackagedAsarPath,
  packagedAccountsListJson,
  parseProviderAccountsListJson,
  parseProviderAccountUsageJson,
  projectReadinessState,
} from "../src/provider-accounts.ts"
import {
  decodeFleetAccountsProjection,
  decodeFleetUsageEntry,
} from "../src/renderer/fleet-workspace.ts"
import type { ProviderAccountsServiceDependencies } from "../src/provider-accounts.ts"
import { classifyProviderRuntimeCompatibility } from "../src/provider-runtime-compatibility.ts"

const fixtureListStdout = readFileSync(
  path.join(import.meta.dirname, "fixtures", "provider-accounts", "accounts-list.json"),
  "utf8",
)

const generatedAt = "2026-07-11T12:00:00.000Z"

describe("parseProviderAccountsListJson (pylon accounts list --json)", () => {
  test("projects refs, providers, email, and the closed readiness set from the fixture", () => {
    const result = parseProviderAccountsListJson(fixtureListStdout, generatedAt)
    expect(result).toEqual({
      ok: true,
      generatedAt,
      accounts: [
        { ref: "codex", provider: "codex", email: "owner@example.com", readiness: "ready" },
        { ref: "codex-2", provider: "codex", email: null, readiness: "credentials-missing" },
        { ref: "claude-pylon-3", provider: "claude_agent", email: null, readiness: "unknown" },
      ],
    })
  })

  test("unknown readiness states and missing emails degrade instead of throwing", () => {
    expect(projectReadinessState("ready")).toBe("ready")
    expect(projectReadinessState("credentials_revoked")).toBe("credentials-missing")
    expect(projectReadinessState("credentials_missing")).toBe("credentials-missing")
    expect(projectReadinessState("account_exhausted")).toBe("unknown")
    expect(projectReadinessState(undefined)).toBe("unknown")
  })

  test("non-JSON and shape-less payloads degrade to a typed public-safe failure", () => {
    const broken = parseProviderAccountsListJson("not json", generatedAt)
    expect(broken).toEqual({ ok: false, reason: "accounts_projection_invalid" })
    expect(parseProviderAccountsListJson("{}", generatedAt).ok).toBe(false)
    expect(JSON.stringify(broken)).not.toContain("/")
  })
})

/** The fixture's provider rate-limit windows after bounded projection. */
const fixtureUsageWindows = [
  { label: "5h", usedPercent: 63, remainingPercent: 37, windowMinutes: 300, resetsAt: "2026-07-11T03:00:00.000Z" },
  { label: "weekly", usedPercent: 18, remainingPercent: 82, windowMinutes: 10080, resetsAt: "2026-07-15T00:00:00.000Z" },
] as const

describe("parseProviderAccountUsageJson (pylon accounts usage --refresh --json)", () => {
  test("projects bounded token totals and rate-limit windows for the requested ref only", () => {
    const result = parseProviderAccountUsageJson(fixtureProviderAccountUsageStdout, "codex", generatedAt)
    expect(result).toEqual({
      ok: true,
      ref: "codex",
      refreshedAt: generatedAt,
      summary: { inputTokens: 1200, outputTokens: 340, totalTokens: 1540 },
      windows: fixtureUsageWindows,
    })
  })

  test("rate-limit windows are bounded, clamped, second/milli epoch tolerant, and omitted when absent", () => {
    // usedPercent-only windows infer remaining; out-of-range percents clamp;
    // garbage windows drop; more than four windows cap at four.
    const stdout = JSON.stringify({
      accounts: [{
        accountRef: "codex",
        truth: {
          provider: {
            snapshots: [
              {
                primary: { usedPercent: 120, windowMinutes: 300, resetsAt: 1783738800, label: "5h" },
                secondary: { usedPercent: -5, windowMinutes: 10080, resetsAt: 1783738800_000, label: "weekly" },
              },
              { primary: { usedPercent: 50 }, secondary: { usedPercent: "not-a-number" } },
              { primary: { usedPercent: 10, label: "monthly" } },
            ],
          },
          localSession: { usage: null },
        },
      }],
    })
    const result = parseProviderAccountUsageJson(stdout, "codex", generatedAt)
    if (!result.ok) throw new Error("expected ok usage projection")
    expect(result.windows).toEqual([
      { label: "5h", usedPercent: 100, remainingPercent: 0, windowMinutes: 300, resetsAt: "2026-07-11T03:00:00.000Z" },
      { label: "weekly", usedPercent: 0, remainingPercent: 100, windowMinutes: 10080, resetsAt: "2026-07-11T03:00:00.000Z" },
      { label: "usage", usedPercent: 50, remainingPercent: 50, windowMinutes: null, resetsAt: null },
      { label: "monthly", usedPercent: 10, remainingPercent: 90, windowMinutes: null, resetsAt: null },
    ])
    // No snapshots -> the key is omitted entirely (never an empty fake array).
    const bare = parseProviderAccountUsageJson(
      JSON.stringify({ accounts: [{ accountRef: "codex", truth: { localSession: { usage: null } } }] }),
      "codex",
      generatedAt,
    )
    if (!bare.ok) throw new Error("expected ok usage projection")
    expect("windows" in bare).toBe(false)
  })

  test("a ref absent from the projection is a typed failure", () => {
    expect(parseProviderAccountUsageJson(fixtureProviderAccountUsageStdout, "codex-9", generatedAt)).toEqual({
      ok: false,
      ref: "codex-9",
      reason: "account_not_found",
    })
  })

  test("missing usage truth degrades to null totals, not a throw", () => {
    const stdout = JSON.stringify({ accounts: [{ accountRef: "codex", truth: { localSession: { usage: null } } }] })
    expect(parseProviderAccountUsageJson(stdout, "codex", generatedAt)).toEqual({
      ok: true,
      ref: "codex",
      refreshedAt: generatedAt,
      summary: { inputTokens: null, outputTokens: null, totalTokens: null },
    })
    expect(parseProviderAccountUsageJson("not json", "codex", generatedAt)).toEqual({
      ok: false,
      ref: "codex",
      reason: "usage_projection_invalid",
    })
  })
})

describe("bridge contract schemas (main side; must agree with renderer decode)", () => {
  test("list round trip agrees across the bridge", () => {
    const payload = parseProviderAccountsListJson(fixtureListStdout, generatedAt)
    expect(decodeProviderAccountsListResult(payload)).toEqual(payload)
    const rendererSide = decodeFleetAccountsProjection(payload)
    expect(rendererSide).toEqual(payload as never)
    expect(decodeProviderAccountsListResult(null)).toEqual({ ok: false, reason: "invalid_bridge_payload" })
    expect(decodeFleetAccountsProjection(null)).toEqual({ ok: false, reason: "invalid_bridge_payload" })
  })

  test("usage round trip agrees across the bridge and pins the ref", () => {
    const payload = parseProviderAccountUsageJson(fixtureProviderAccountUsageStdout, "codex", generatedAt)
    expect(decodeProviderAccountUsageResult(payload, "codex")).toEqual(payload)
    expect(decodeFleetUsageEntry(payload, "codex")).toEqual({
      state: "checked",
      refreshedAt: generatedAt,
      inputTokens: 1200,
      outputTokens: 340,
      totalTokens: 1540,
      windows: fixtureUsageWindows,
    })
    expect(decodeFleetUsageEntry(payload, "codex-2")).toEqual({
      state: "failed",
      reason: "invalid_bridge_payload",
    })
    // A windowless payload keeps the pre-EP250 checked shape (no windows key).
    const windowless = {
      ok: true,
      ref: "codex",
      refreshedAt: generatedAt,
      summary: { inputTokens: 1, outputTokens: 2, totalTokens: 3 },
    }
    const entry = decodeFleetUsageEntry(windowless, "codex")
    expect(entry).toEqual({
      state: "checked",
      refreshedAt: generatedAt,
      inputTokens: 1,
      outputTokens: 2,
      totalTokens: 3,
    })
    expect("windows" in entry).toBe(false)
  })

  test("usage requests only pass pylon-grammar refs", () => {
    expect(decodeProviderAccountUsageRequest({ ref: "codex-2" })).toEqual({ ref: "codex-2" })
    expect(decodeProviderAccountUsageRequest({ ref: "../etc/passwd" })).toBeNull()
    expect(decodeProviderAccountUsageRequest({ ref: "" })).toBeNull()
    expect(decodeProviderAccountUsageRequest("codex-2")).toBeNull()
  })
})

type FakeChild = NonNullable<ReturnType<NonNullable<ProviderAccountsServiceDependencies["spawnPylon"]>>>

const makeFakeChild = (): FakeChild & {
  emitStdout: (chunk: string) => void
  emitClose: (code: number | null) => void
  emitError: () => void
  killCount: number
} => {
  const listeners = new Map<string, Array<(...values: unknown[]) => void>>()
  const stdoutHandlers: Array<(chunk: string) => void> = []
  const child: FakeChild & {
    emitStdout: (chunk: string) => void
    emitClose: (code: number | null) => void
    emitError: () => void
    killCount: number
  } = {
    stdout: {
      on: (event: string, listener: (chunk: string) => void) => {
        if (event === "data") stdoutHandlers.push(listener)
      },
    } as unknown as NodeJS.ReadableStream,
    stderr: null,
    on: (event, listener) => {
      const existing = listeners.get(event) ?? []
      listeners.set(event, [...existing, listener])
      return child
    },
    kill: () => {
      child.killCount++
      child.killed = true
      return true
    },
    killed: false,
    exitCode: null,
    killCount: 0,
    emitStdout: (chunk) => {
      for (const handler of stdoutHandlers) handler(chunk)
    },
    emitClose: (code) => {
      child.exitCode = code
      for (const listener of listeners.get("close") ?? []) listener(code)
    },
    emitError: () => {
      for (const listener of listeners.get("error") ?? []) listener(new Error("spawn failed"))
    },
  }
  return child
}

describe("makeProviderAccountsService", () => {
  test("recognizes Electron archive paths even when the archive exposes packaged source files", () => {
    expect(isPackagedAsarPath("/Applications/OpenAgents.app/Contents/Resources/app.asar/dist/main")).toBe(true)
    expect(isPackagedAsarPath("/tmp/OpenAgents.app/Contents/Resources/app.asar.unpacked/bin")).toBe(true)
    expect(isPackagedAsarPath("/Users/me/work/openagents/apps/openagents-desktop/out/main")).toBe(false)
  })

  test("packaged config projection admits only valid registered refs with local auth presence", () => {
    const projected = parseProviderAccountsListJson(packagedAccountsListJson(JSON.stringify({
      dev: { accounts: [
        { ref: "codex-2", provider: "codex", home: "/private/accounts/codex-2" },
        { ref: "codex-paused", provider: "codex", home: "/private/accounts/paused", paused: true },
        { ref: "../escape", provider: "codex", home: "/private/accounts/escape" },
        { ref: "relative", provider: "codex", home: "relative/home" },
      ] },
    }), value => value === "/private/accounts/codex-2/auth.json"), generatedAt)
    expect(projected).toEqual({
      ok: true,
      generatedAt,
      accounts: [
        { ref: "codex-2", provider: "codex", email: null, readiness: "ready" },
        { ref: "codex-paused", provider: "codex", email: null, readiness: "credentials-missing" },
      ],
    })
    expect(JSON.stringify(projected)).not.toContain("/private")
  })

  test("fixture spawn: list projects the public-safe fleet and usage returns bounded totals", async () => {
    const service = makeProviderAccountsService("/nonexistent", {
      spawnPylon: makeFixtureProviderAccountsSpawn(),
      now: () => new Date(generatedAt),
      inspectRuntimes: async () => [
        classifyProviderRuntimeCompatibility("codex_cli", "codex-cli 0.144.1"),
        classifyProviderRuntimeCompatibility("claude_agent_sdk", "0.3.172"),
      ],
    })
    const list = await service.listProviderAccounts()
    expect(list).toEqual({
      ok: true,
      generatedAt,
      accounts: [
        { ref: "codex-3", provider: "codex", email: null, readiness: "ready" },
        { ref: "codex", provider: "codex", email: null, readiness: "ready" },
        { ref: "codex-2", provider: "codex", email: null, readiness: "credentials-missing" },
        { ref: "claude-pylon-3", provider: "claude_agent", email: null, readiness: "ready" },
      ],
      runtimes: [
        { kind: "codex_cli", state: "compatible", expectedVersion: "0.144.1", observedVersion: "0.144.1", reason: "verified" },
        { kind: "claude_agent_sdk", state: "compatible", expectedVersion: "0.3.172", observedVersion: "0.3.172", reason: "verified" },
      ],
    })
    const usage = await service.fetchProviderAccountUsage("codex")
    expect(usage).toEqual({
      ok: true,
      ref: "codex",
      refreshedAt: generatedAt,
      summary: { inputTokens: 1200, outputTokens: 340, totalTokens: 1540 },
      windows: fixtureUsageWindows,
    })
    service.dispose()
  })

  test("packaged app.asar path uses the source-independent Pylon core projection", async () => {
    let spawnCount = 0
    const service = makeProviderAccountsService(
      "/Applications/OpenAgents.app/Contents/Resources/app.asar/dist",
      {
        spawnPylon: () => {
          spawnCount++
          return null
        },
        packagedProjection: {
          list: async () => fixtureProviderAccountsListStdout,
          usage: async () => fixtureProviderAccountUsageStdout,
        },
        now: () => new Date(generatedAt),
        inspectRuntimes: async () => [],
      },
    )

    expect(await service.listProviderAccounts()).toMatchObject({
      ok: true,
      accounts: [
        { ref: "codex-3", provider: "codex", readiness: "ready" },
        { ref: "codex", provider: "codex", readiness: "ready" },
        { ref: "codex-2", provider: "codex", readiness: "credentials-missing" },
        { ref: "claude-pylon-3", provider: "claude_agent", readiness: "ready" },
      ],
    })
    expect(await service.fetchProviderAccountUsage("codex")).toMatchObject({
      ok: true,
      ref: "codex",
      summary: { totalTokens: 1540 },
    })
    expect(spawnCount).toBe(0)
    service.dispose()
  })

  test("explicit Electron packaged truth wins when the bundled dirname is not an ASAR path", async () => {
    const service = makeProviderAccountsService("/Applications/OpenAgents.app/Contents/Resources", {
      packaged: true,
      spawnPylon: undefined,
      packagedProjection: {
        list: async () => fixtureProviderAccountsListStdout,
        usage: async () => fixtureProviderAccountUsageStdout,
      },
      inspectRuntimes: async () => [],
    })
    const result = await service.listProviderAccounts()
    expect(result.ok).toBe(true)
    service.dispose()
  })

  test("headless packaged bootstrap reads the isolated Pylon registry without a child runtime", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "provider-accounts-bootstrap-"))
    const pylonHome = path.join(root, "pylon")
    const accountHome = path.join(root, "account")
    mkdirSync(pylonHome, { recursive: true })
    mkdirSync(accountHome, { recursive: true })
    writeFileSync(path.join(accountHome, "auth.json"), "{}", { mode: 0o600 })
    writeFileSync(path.join(pylonHome, "config.json"), JSON.stringify({
      dev: { accounts: [{ ref: "codex-2", provider: "codex", home: accountHome }] },
    }))
    const previous = process.env.PYLON_HOME
    process.env.PYLON_HOME = pylonHome
    try {
      const diagnostics: Array<Readonly<Record<string, string | number | boolean | null>>> = []
      const service = makeProviderAccountsService("/opaque/bundled/main", {
        packaged: true,
        inspectRuntimes: async () => [],
        diagnostic: event => diagnostics.push(event),
      })
      expect(await service.listProviderAccounts()).toMatchObject({
        ok: true,
        accounts: [{ ref: "codex-2", provider: "codex", readiness: "ready" }],
      })
      expect(diagnostics[0]).toMatchObject({ mode: "packaged_projection", packagedHint: true })
      service.dispose()
    } finally {
      if (previous === undefined) delete process.env.PYLON_HOME
      else process.env.PYLON_HOME = previous
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("unavailable pylon runtime yields typed failures with no paths, never a throw", async () => {
    const service = makeProviderAccountsService("/nonexistent", { spawnPylon: () => null })
    const list = await service.listProviderAccounts()
    expect(list).toEqual(unavailableProviderAccountsListResult())
    const usage = await service.fetchProviderAccountUsage("codex")
    expect(usage).toEqual({ ok: false, ref: "codex", reason: "pylon_runtime_unavailable" })
    expect(JSON.stringify([list, usage])).not.toContain("/nonexistent")
  })

  test("an invalid ref never reaches a spawn", async () => {
    let spawnCount = 0
    const service = makeProviderAccountsService("/nonexistent", {
      spawnPylon: () => {
        spawnCount++
        return null
      },
    })
    expect(await service.fetchProviderAccountUsage("../escape")).toEqual({
      ok: false,
      ref: "../escape",
      reason: "invalid_account_ref",
    })
    expect(spawnCount).toBe(0)
  })

  test("non-zero exits and child errors settle as typed failures", async () => {
    const failing = makeFakeChild()
    const service = makeProviderAccountsService("/nonexistent", { spawnPylon: () => failing })
    const pending = service.listProviderAccounts()
    failing.emitStdout("Pylon accounts failed\n")
    failing.emitClose(1)
    expect(await pending).toEqual({ ok: false, reason: "accounts_command_failed" })

    const erroring = makeFakeChild()
    const errorService = makeProviderAccountsService("/nonexistent", { spawnPylon: () => erroring })
    const pendingError = errorService.listProviderAccounts()
    erroring.emitError()
    expect(await pendingError).toEqual({ ok: false, reason: "pylon_runtime_unavailable" })
  })

  test("a hung usage child is killed at the bounded timeout", async () => {
    const hung = makeFakeChild()
    const service = makeProviderAccountsService("/nonexistent", {
      spawnPylon: () => hung,
      usageTimeoutMs: 5,
    })
    expect(await service.fetchProviderAccountUsage("codex")).toEqual({
      ok: false,
      ref: "codex",
      reason: "projection_timeout",
    })
    expect(hung.killCount).toBe(1)
  })

  test("dispose settles in-flight projections and is terminal", async () => {
    const child = makeFakeChild()
    const service = makeProviderAccountsService("/nonexistent", { spawnPylon: () => child })
    const pending = service.listProviderAccounts()
    service.dispose()
    service.dispose()
    expect(await pending).toEqual(unavailableProviderAccountsListResult())
    expect(child.killCount).toBe(1)
    expect(await service.listProviderAccounts()).toEqual(unavailableProviderAccountsListResult())
  })

  test("fixture stdout parses through the same projection as the real CLI output", () => {
    const result = parseProviderAccountsListJson(fixtureProviderAccountsListStdout, generatedAt)
    expect(result.ok).toBe(true)
  })
})
