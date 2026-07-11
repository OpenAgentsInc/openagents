/**
 * Main-process-only provider-neutral accounts host adapter (#8712 Fleet
 * overview). Read-only over the pylon CLI from the repo root:
 *
 *   bun apps/pylon/src/index.ts accounts list --json
 *   bun apps/pylon/src/index.ts accounts usage --account <ref> --refresh --json
 *
 * SAFETY (repo law, same as ./codex-connect.ts): this service NEVER sets or
 * touches CODEX_HOME or any default provider home. It spawns the read-only
 * pylon projections and forwards only public-safe fields (refs, provider
 * names, the projection's own email field, a closed readiness set, and
 * bounded usage token totals). Raw payloads, credential material, and local
 * paths never leave this module; failures return typed `{ ok: false, reason }`
 * values with public-safe reasons — never a throw across IPC.
 */
import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import path from "node:path"

import {
  providerAccountRefPattern,
  type ProviderAccountEntry,
  type ProviderAccountReadiness,
  type ProviderAccountsListResult,
  type ProviderAccountUsageResult,
  unavailableProviderAccountsListResult,
  unavailableProviderAccountUsageResult,
} from "./provider-accounts-contract.ts"

/**
 * Projects a pylon readiness state string into the closed renderer set.
 * Anything the desktop cannot positively classify degrades to "unknown" —
 * the Fleet view renders that as explicit missing evidence, never as lit.
 */
export const projectReadinessState = (state: unknown): ProviderAccountReadiness => {
  if (state === "ready") return "ready"
  if (typeof state === "string" && state.startsWith("credentials")) return "credentials-missing"
  return "unknown"
}

const projectEmail = (value: unknown): string | null =>
  typeof value === "string" && value.includes("@") && value.length <= 120 ? value : null

/** Public-safe list projection from pylon's accounts_list JSON. */
export const parseProviderAccountsListJson = (
  stdout: string,
  generatedAt: string,
): ProviderAccountsListResult => {
  try {
    const parsed = JSON.parse(stdout) as {
      accounts?: Array<{
        provider?: unknown
        accountRef?: unknown
        email?: unknown
        readiness?: { state?: unknown }
      }>
    }
    if (!Array.isArray(parsed.accounts)) {
      return unavailableProviderAccountsListResult("accounts_projection_invalid")
    }
    const accounts: Array<ProviderAccountEntry> = []
    for (const account of parsed.accounts) {
      if (typeof account.accountRef !== "string") continue
      if (!providerAccountRefPattern.test(account.accountRef)) continue
      accounts.push({
        ref: account.accountRef,
        provider: typeof account.provider === "string" ? account.provider.slice(0, 40) : "unknown",
        email: projectEmail(account.email),
        readiness: projectReadinessState(account.readiness?.state),
      })
    }
    return { ok: true, generatedAt, accounts }
  } catch {
    return unavailableProviderAccountsListResult("accounts_projection_invalid")
  }
}

const boundedTokenTotal = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) && value >= 0 ? Math.floor(value) : null

/** Public-safe usage projection from pylon's accounts_usage JSON. */
export const parseProviderAccountUsageJson = (
  stdout: string,
  ref: string,
  refreshedAt: string,
): ProviderAccountUsageResult => {
  try {
    const parsed = JSON.parse(stdout) as {
      accounts?: Array<{
        accountRef?: unknown
        truth?: { localSession?: { usage?: { inputTokens?: unknown; outputTokens?: unknown; totalTokens?: unknown } | null } }
      }>
    }
    if (!Array.isArray(parsed.accounts)) {
      return unavailableProviderAccountUsageResult(ref, "usage_projection_invalid")
    }
    const account = parsed.accounts.find((entry) => entry.accountRef === ref)
    if (account === undefined) {
      return unavailableProviderAccountUsageResult(ref, "account_not_found")
    }
    const usage = account.truth?.localSession?.usage ?? null
    return {
      ok: true,
      ref,
      refreshedAt,
      summary: {
        inputTokens: boundedTokenTotal(usage?.inputTokens),
        outputTokens: boundedTokenTotal(usage?.outputTokens),
        totalTokens: boundedTokenTotal(usage?.totalTokens),
      },
    }
  } catch {
    return unavailableProviderAccountUsageResult(ref, "usage_projection_invalid")
  }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

type ChildLike = {
  stdout: NodeJS.ReadableStream | null
  stderr: NodeJS.ReadableStream | null
  on: (event: "close" | "error", listener: (...args: unknown[]) => void) => unknown
  kill: (signal?: NodeJS.Signals) => boolean
  killed: boolean
  exitCode: number | null
}

export type ProviderAccountsServiceDependencies = Readonly<{
  /** Spawn a pylon CLI invocation; overridable for tests/fixture mode. */
  spawnPylon?: (args: ReadonlyArray<string>) => ChildLike | null
  listTimeoutMs?: number
  usageTimeoutMs?: number
  now?: () => Date
}>

const repoRootFromHere = (here: string): string | null => {
  // dist/main.js -> apps/openagents-desktop -> repo root
  const root = path.resolve(here, "..", "..", "..")
  return existsSync(path.join(root, "apps", "pylon", "src", "index.ts")) ? root : null
}

const defaultSpawnPylon = (here: string) => (args: ReadonlyArray<string>): ChildLike | null => {
  const root = repoRootFromHere(here)
  if (root === null) return null
  // No provider-home env injection — pylon owns credential custody and its
  // isolated per-account homes. Inherited env only.
  return spawn("bun", ["apps/pylon/src/index.ts", ...args], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
  }) as unknown as ChildLike
}

const collectStream = (stream: NodeJS.ReadableStream | null, onChunk: (text: string) => void): void => {
  if (stream === null) return
  stream.on("data", (chunk: Buffer | string) => {
    onChunk(typeof chunk === "string" ? chunk : chunk.toString("utf8"))
  })
}

export type ProviderAccountsService = Readonly<{
  listProviderAccounts: () => Promise<ProviderAccountsListResult>
  fetchProviderAccountUsage: (ref: string) => Promise<ProviderAccountUsageResult>
  dispose: () => void
}>

export const makeProviderAccountsService = (
  here: string,
  dependencies: ProviderAccountsServiceDependencies = {},
): ProviderAccountsService => {
  const spawnPylon = dependencies.spawnPylon ?? defaultSpawnPylon(here)
  const listTimeoutMs = dependencies.listTimeoutMs ?? 120_000
  const usageTimeoutMs = dependencies.usageTimeoutMs ?? 30_000
  const now = dependencies.now ?? (() => new Date())
  let disposed = false
  const operations = new Set<Readonly<{ child: ChildLike; cancel: () => void }>>()

  const runProjection = <Result>(input: Readonly<{
    args: ReadonlyArray<string>
    timeoutMs: number
    parse: (stdout: string) => Result
    unavailable: (reason: string) => Result
  }>): Promise<Result> =>
    new Promise((resolve) => {
      if (disposed) {
        resolve(input.unavailable("pylon_runtime_unavailable"))
        return
      }
      const child = spawnPylon(input.args)
      if (child === null) {
        resolve(input.unavailable("pylon_runtime_unavailable"))
        return
      }
      let stdout = ""
      let done = false
      let timer: ReturnType<typeof setTimeout> | null = null
      let operation: Readonly<{ child: ChildLike; cancel: () => void }> | null = null
      const finish = (result: Result): void => {
        if (done) return
        done = true
        if (timer !== null) clearTimeout(timer)
        if (operation !== null) operations.delete(operation)
        resolve(result)
      }
      operation = { child, cancel: () => finish(input.unavailable("pylon_runtime_unavailable")) }
      operations.add(operation)
      timer = setTimeout(() => {
        child.kill("SIGTERM")
        finish(input.unavailable("projection_timeout"))
      }, input.timeoutMs)
      collectStream(child.stdout, (text) => {
        stdout += text
      })
      collectStream(child.stderr, () => {})
      child.on("error", () => {
        finish(input.unavailable("pylon_runtime_unavailable"))
      })
      child.on("close", (...args: unknown[]) => {
        const exitCode = typeof args[0] === "number" ? args[0] : null
        if (exitCode !== 0) {
          finish(input.unavailable("accounts_command_failed"))
          return
        }
        finish(input.parse(stdout))
      })
    })

  return {
    listProviderAccounts: () =>
      runProjection({
        args: ["accounts", "list", "--json"],
        timeoutMs: listTimeoutMs,
        parse: (stdout) => parseProviderAccountsListJson(stdout, now().toISOString()),
        unavailable: unavailableProviderAccountsListResult,
      }),
    fetchProviderAccountUsage: (ref) => {
      if (!providerAccountRefPattern.test(ref)) {
        return Promise.resolve(unavailableProviderAccountUsageResult(ref, "invalid_account_ref"))
      }
      return runProjection({
        args: ["accounts", "usage", "--account", ref, "--refresh", "--json"],
        timeoutMs: usageTimeoutMs,
        parse: (stdout) => parseProviderAccountUsageJson(stdout, ref, now().toISOString()),
        unavailable: (reason) => unavailableProviderAccountUsageResult(ref, reason),
      })
    },
    dispose: () => {
      if (disposed) return
      disposed = true
      for (const operation of [...operations]) {
        if (!operation.child.killed) operation.child.kill("SIGTERM")
        operation.cancel()
      }
    },
  }
}

// ---------------------------------------------------------------------------
// Smoke fixture (honest: used ONLY when the smoke harness asks for it; logged
// loudly from main). It never spawns anything — scripted projections so the
// headless smoke can never touch a real pylon home.
// ---------------------------------------------------------------------------

export const fixtureProviderAccountsListStdout = JSON.stringify({
  schema: "openagents.pylon.accounts_list.v0.3",
  observedAt: "2026-07-11T00:00:00.000Z",
  accounts: [
    {
      provider: "codex",
      accountRef: "codex",
      readiness: { state: "ready" },
    },
    {
      provider: "codex",
      accountRef: "codex-2",
      readiness: { state: "credentials_revoked" },
    },
    {
      provider: "claude_agent",
      accountRef: "claude-pylon-3",
      readiness: { state: "ready" },
    },
  ],
  blockerRefs: [],
})

export const fixtureProviderAccountUsageStdout = JSON.stringify({
  schema: "openagents.pylon.accounts_usage.v0.3",
  observedAt: "2026-07-11T00:00:00.000Z",
  accounts: [
    {
      provider: "codex",
      accountRef: "codex",
      truth: {
        localSession: {
          usage: { inputTokens: 1200, outputTokens: 340, totalTokens: 1540 },
        },
      },
    },
  ],
  blockerRefs: [],
})

export const makeFixtureProviderAccountsSpawn = (): NonNullable<
  ProviderAccountsServiceDependencies["spawnPylon"]
> => {
  return (args) => {
    const isUsage = args.includes("usage")
    const listeners = new Map<string, Array<(...values: unknown[]) => void>>()
    const stdoutHandlers: Array<(chunk: string) => void> = []
    const child: ChildLike = {
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
      kill: () => true,
      killed: false,
      exitCode: null,
    }
    queueMicrotask(() => {
      for (const handler of stdoutHandlers) {
        handler(isUsage ? fixtureProviderAccountUsageStdout : fixtureProviderAccountsListStdout)
      }
      for (const listener of listeners.get("close") ?? []) listener(0)
    })
    return child
  }
}
