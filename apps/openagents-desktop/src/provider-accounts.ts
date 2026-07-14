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
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { homedir } from "node:os"
import path from "node:path"
import { inspectProviderRuntimeCompatibility } from "./provider-runtime-host.ts"
import type { ProviderRuntimeCompatibility } from "./provider-runtime-compatibility.ts"

import {
  providerAccountRefPattern,
  providerAccountUsageWindowCap,
  type ProviderAccountEntry,
  type ProviderAccountReadiness,
  type ProviderAccountsListResult,
  type ProviderAccountUsageResult,
  type ProviderAccountUsageWindow,
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

const clampPercent = (value: number): number => Math.min(100, Math.max(0, value))

/**
 * One bounded rate-limit window from a pylon `truth.provider.snapshots[*]`
 * primary/secondary record (codex-rs RateLimitSnapshot lineage). `resetsAt`
 * arrives as a unix timestamp (seconds; defensively also accepts millis) and
 * is projected to a bounded ISO string. Anything unparseable is dropped —
 * the renderer then shows the honest grayed bar instead of a fake one.
 */
const boundedUsageWindow = (value: unknown): ProviderAccountUsageWindow | null => {
  if (typeof value !== "object" || value === null) return null
  const raw = value as {
    usedPercent?: unknown
    remainingPercent?: unknown
    windowMinutes?: unknown
    resetsAt?: unknown
    label?: unknown
  }
  if (typeof raw.usedPercent !== "number" || !Number.isFinite(raw.usedPercent)) return null
  const usedPercent = clampPercent(raw.usedPercent)
  const remainingPercent =
    typeof raw.remainingPercent === "number" && Number.isFinite(raw.remainingPercent)
      ? clampPercent(raw.remainingPercent)
      : clampPercent(100 - usedPercent)
  const windowMinutes =
    typeof raw.windowMinutes === "number" && Number.isFinite(raw.windowMinutes) && raw.windowMinutes >= 0
      ? Math.floor(raw.windowMinutes)
      : null
  let resetsAt: string | null = null
  if (typeof raw.resetsAt === "number" && Number.isFinite(raw.resetsAt) && raw.resetsAt > 0) {
    const epochMs = raw.resetsAt > 10_000_000_000 ? raw.resetsAt : raw.resetsAt * 1000
    const date = new Date(epochMs)
    resetsAt = Number.isNaN(date.getTime()) ? null : date.toISOString()
  }
  return {
    label: typeof raw.label === "string" && raw.label.length > 0 ? raw.label.slice(0, 20) : "usage",
    usedPercent,
    remainingPercent,
    windowMinutes,
    resetsAt,
  }
}

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
        truth?: {
          provider?: { snapshots?: Array<{ primary?: unknown; secondary?: unknown }> } | null
          localSession?: { usage?: { inputTokens?: unknown; outputTokens?: unknown; totalTokens?: unknown } | null }
        }
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
    // Rate-limit window truth (EP250 sidebar accounts box): pylon's provider
    // truth snapshots carry the codex 5h/weekly windows when the account's
    // provider reports them. Bounded and additive — absent snapshots simply
    // omit the field.
    const snapshots = account.truth?.provider?.snapshots
    const windows: Array<ProviderAccountUsageWindow> = []
    if (Array.isArray(snapshots)) {
      for (const snapshot of snapshots) {
        if (typeof snapshot !== "object" || snapshot === null) continue
        for (const candidate of [snapshot.primary, snapshot.secondary]) {
          const window = boundedUsageWindow(candidate)
          if (window !== null && windows.length < providerAccountUsageWindowCap) windows.push(window)
        }
      }
    }
    return {
      ok: true,
      ref,
      refreshedAt,
      summary: {
        inputTokens: boundedTokenTotal(usage?.inputTokens),
        outputTokens: boundedTokenTotal(usage?.outputTokens),
        totalTokens: boundedTokenTotal(usage?.totalTokens),
      },
      ...(windows.length > 0 ? { windows } : {}),
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
  inspectRuntimes?: () => Promise<ReadonlyArray<ProviderRuntimeCompatibility>>
  /** Electron's authoritative packaged-state signal; avoids ASAR path heuristics. */
  packaged?: boolean
  diagnostic?: (event: Readonly<Record<string, string | number | boolean | null>>) => void
  /**
   * Source-independent projection used by packaged Desktop builds. The CLI
   * adapter remains the development path; an installed app cannot derive an
   * `apps/pylon` checkout from `app.asar` and therefore reads the same typed
   * Pylon core directly instead.
   */
  packagedProjection?: Readonly<{
    list: () => Promise<string>
    usage: (ref: string) => Promise<string>
  }>
}>

const repoRootFromHere = (here: string): string | null => {
  // dist/main.js -> apps/openagents-desktop -> repo root
  const root = path.resolve(here, "..", "..", "..")
  return existsSync(path.join(root, "apps", "pylon", "src", "index.ts")) ? root : null
}

export const isPackagedAsarPath = (here: string): boolean =>
  here.split(path.sep).some(part => part === "app.asar" || part.startsWith("app.asar."))

const defaultSpawnPylon = (here: string) => (args: ReadonlyArray<string>): ChildLike | null => {
  const root = repoRootFromHere(here)
  if (root === null) return null
  // No provider-home env injection — pylon owns credential custody and its
  // isolated per-account homes. Inherited env only.
  return spawn("node", ["--import", "tsx", "apps/pylon/src/index.ts", ...args], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
  }) as unknown as ChildLike
}

type PackagedRegistryAccount = Readonly<{
  ref: string
  provider: string
  home: string
  paused: boolean
}>

const parsePackagedRegistry = (configJson: string): Array<PackagedRegistryAccount> => {
  const parsed = JSON.parse(configJson) as {
    dev?: { accounts?: Array<Record<string, unknown>> }
  }
  return (parsed.dev?.accounts ?? []).flatMap(account => {
    const ref = account.ref
    const provider = account.provider
    const accountHome = account.home
    if (typeof ref !== "string" || !providerAccountRefPattern.test(ref) ||
        typeof provider !== "string" || typeof accountHome !== "string" ||
        !path.isAbsolute(accountHome)) return []
    return [{ ref, provider: provider.slice(0, 40), home: accountHome, paused: account.paused === true }]
  })
}

export const packagedAccountsListJson = (
  configJson: string,
  authExists: (value: string) => boolean = existsSync,
): string => JSON.stringify({
  accounts: parsePackagedRegistry(configJson).map(account => ({
    provider: account.provider,
    accountRef: account.ref,
    readiness: {
      state: !account.paused && authExists(path.join(account.home, "auth.json"))
        ? "ready"
        : "credentials_missing",
    },
  })),
})

const defaultPackagedProjection = (): NonNullable<ProviderAccountsServiceDependencies["packagedProjection"]> => {
  const home = process.env.PYLON_HOME?.trim() || path.join(homedir(), ".openagents", "pylon")
  const configPath = path.join(home, "config.json")
  const usagePath = path.join(home, "account-usage.json")
  const registry = async (): Promise<Array<PackagedRegistryAccount>> =>
    parsePackagedRegistry(await readFile(configPath, "utf8"))
  return {
    list: async () => packagedAccountsListJson(await readFile(configPath, "utf8")),
    usage: async (ref) => {
      const account = (await registry()).find(candidate => candidate.ref === ref)
      if (account === undefined) return JSON.stringify({ accounts: [] })
      const parsed = JSON.parse(await readFile(usagePath, "utf8")) as {
        accounts?: Record<string, {
          providerTruth?: { snapshots?: unknown }
          localSessionTruth?: { usage?: unknown }
        }>
      }
      const hash = `account.pylon.${account.provider}.${createHash("sha256")
        .update(`${account.provider}:${account.ref}`).digest("hex").slice(0, 24)}`
      const entry = parsed.accounts?.[hash]
      return JSON.stringify({
        accounts: [{
          accountRef: ref,
          truth: {
            provider: { snapshots: entry?.providerTruth?.snapshots ?? [] },
            localSession: { usage: entry?.localSessionTruth?.usage ?? null },
          },
        }],
      })
    },
  }
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
  const inspectRuntimes = dependencies.inspectRuntimes ?? inspectProviderRuntimeCompatibility
  const packagedProjection = dependencies.packagedProjection ?? (
    dependencies.spawnPylon === undefined && (
      dependencies.packaged === true || isPackagedAsarPath(here) || repoRootFromHere(here) === null
    )
      ? defaultPackagedProjection()
      : null
  )
  dependencies.diagnostic?.({
    kind: "projection_selected",
    mode: packagedProjection === null ? "pylon_child" : "packaged_projection",
    packagedHint: dependencies.packaged === true,
    asarHint: isPackagedAsarPath(here),
    repoRootAvailable: repoRootFromHere(here) !== null,
  })
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
        dependencies.diagnostic?.({ kind: "spawn_unavailable", operation: input.args[1] ?? "unknown" })
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
      let stderrBytes = 0
      collectStream(child.stderr, text => { stderrBytes += Buffer.byteLength(text) })
      child.on("error", () => {
        dependencies.diagnostic?.({ kind: "child_error", operation: input.args[1] ?? "unknown", stderrBytes })
        finish(input.unavailable("pylon_runtime_unavailable"))
      })
      child.on("close", (...args: unknown[]) => {
        const exitCode = typeof args[0] === "number" ? args[0] : null
        dependencies.diagnostic?.({ kind: "child_close", operation: input.args[1] ?? "unknown", exitCode, stderrBytes })
        if (exitCode !== 0) {
          finish(input.unavailable("accounts_command_failed"))
          return
        }
        finish(input.parse(stdout))
      })
    })

  return {
    listProviderAccounts: async () => {
      const result = packagedProjection === null
        ? await runProjection({
            args: ["accounts", "list", "--json"],
            timeoutMs: listTimeoutMs,
            parse: (stdout) => parseProviderAccountsListJson(stdout, now().toISOString()),
            unavailable: unavailableProviderAccountsListResult,
          })
        : await packagedProjection.list()
            .then(stdout => parseProviderAccountsListJson(stdout, now().toISOString()))
            .catch(error => {
              dependencies.diagnostic?.({ kind: "packaged_projection_error", operation: "list", errorName: error instanceof Error ? error.name : "unknown" })
              return unavailableProviderAccountsListResult("accounts_command_failed")
            })
      if (!result.ok) return result
      const runtimes = await inspectRuntimes().catch(() => [])
      return runtimes.length === 0 ? result : { ...result, runtimes: runtimes.slice(0, 2) }
    },
    fetchProviderAccountUsage: (ref) => {
      if (!providerAccountRefPattern.test(ref)) {
        return Promise.resolve(unavailableProviderAccountUsageResult(ref, "invalid_account_ref"))
      }
      return packagedProjection === null
        ? runProjection({
            args: ["accounts", "usage", "--account", ref, "--refresh", "--json"],
            timeoutMs: usageTimeoutMs,
            parse: (stdout) => parseProviderAccountUsageJson(stdout, ref, now().toISOString()),
            unavailable: (reason) => unavailableProviderAccountUsageResult(ref, reason),
          })
        : packagedProjection.usage(ref)
            .then(stdout => parseProviderAccountUsageJson(stdout, ref, now().toISOString()))
            .catch(() => unavailableProviderAccountUsageResult(ref, "accounts_command_failed"))
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
      // The healthy codex account the composer's codex-local lane runs on
      // (FIXTURE_CODEX_LOCAL_ACCOUNT shares this ref). Listed FIRST so the
      // renderer's default exact-provider-target binding (#8701 CUT-21:
      // first READY fleet account per provider) selects it — "codex" below
      // keeps its child-observed reconnect-required fleet narrative and
      // "codex-2" stays credentials_revoked.
      provider: "codex",
      accountRef: "codex-3",
      readiness: { state: "ready" },
    },
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
        provider: {
          state: "available",
          observedAt: "2026-07-11T00:00:00.000Z",
          // codex-rs RateLimitSnapshot lineage: primary = the 5h window,
          // secondary = the weekly window (the ChatGPT app's two limit bars).
          snapshots: [
            {
              provider: "codex",
              limitId: "codex",
              limitName: null,
              primary: { usedPercent: 63, remainingPercent: 37, windowMinutes: 300, resetsAt: 1783738800, label: "5h" },
              secondary: { usedPercent: 18, remainingPercent: 82, windowMinutes: 10080, resetsAt: 1784073600, label: "weekly" },
              credits: null,
              planType: null,
              rateLimitReachedType: null,
            },
          ],
        },
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
