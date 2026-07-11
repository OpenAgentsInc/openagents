/**
 * Main-process provider-accounts host adapter tests (#8712): the accounts-list
 * and usage JSON projections against a checked-in public-safe fixture, the
 * typed failure paths (never a throw across IPC), the bridge contract schemas
 * (main side and renderer side must agree), and the service with fake
 * children.
 */
import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
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
  parseProviderAccountsListJson,
  parseProviderAccountUsageJson,
  projectReadinessState,
} from "../src/provider-accounts.ts"
import {
  decodeFleetAccountsProjection,
  decodeFleetUsageEntry,
} from "../src/renderer/fleet-workspace.ts"
import type { ProviderAccountsServiceDependencies } from "../src/provider-accounts.ts"

const fixtureListStdout = readFileSync(
  path.join(import.meta.dir, "fixtures", "provider-accounts", "accounts-list.json"),
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

describe("parseProviderAccountUsageJson (pylon accounts usage --refresh --json)", () => {
  test("projects bounded token totals for the requested ref only", () => {
    const result = parseProviderAccountUsageJson(fixtureProviderAccountUsageStdout, "codex", generatedAt)
    expect(result).toEqual({
      ok: true,
      ref: "codex",
      refreshedAt: generatedAt,
      summary: { inputTokens: 1200, outputTokens: 340, totalTokens: 1540 },
    })
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
    })
    expect(decodeFleetUsageEntry(payload, "codex-2")).toEqual({
      state: "failed",
      reason: "invalid_bridge_payload",
    })
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
  test("fixture spawn: list projects the public-safe fleet and usage returns bounded totals", async () => {
    const service = makeProviderAccountsService("/nonexistent", {
      spawnPylon: makeFixtureProviderAccountsSpawn(),
      now: () => new Date(generatedAt),
    })
    const list = await service.listProviderAccounts()
    expect(list).toEqual({
      ok: true,
      generatedAt,
      accounts: [
        { ref: "codex", provider: "codex", email: null, readiness: "ready" },
        { ref: "codex-2", provider: "codex", email: null, readiness: "credentials-missing" },
        { ref: "claude-pylon-3", provider: "claude_agent", email: null, readiness: "ready" },
      ],
    })
    const usage = await service.fetchProviderAccountUsage("codex")
    expect(usage).toEqual({
      ok: true,
      ref: "codex",
      refreshedAt: generatedAt,
      summary: { inputTokens: 1200, outputTokens: 340, totalTokens: 1540 },
    })
    service.dispose()
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
