#!/usr/bin/env bun

import { Database } from "bun:sqlite"
import { mkdir } from "node:fs/promises"
import { join } from "node:path"
import {
  discoverPylonSiblingAccountHomes,
  loadPylonAccountRegistry,
  resolvePylonAccountSelection,
  type PylonAccountRegistryEntry,
} from "../account-registry.js"
import { resolvePylonHome } from "../bootstrap.js"
import {
  candidateAccountsFromRegistry,
  enforcePendingRuntimeIntents,
  type ActiveRuntimeTurns,
} from "./runtime-intent-enforcement.js"
import { createPylonOrchestrationStore } from "./store.js"

/**
 * Runtime control-intent dispatch consumer — standalone process (#8388).
 *
 * WHY THIS IS ITS OWN LONG-RUNNING PROCESS, NOT a `supervisor-state.ts`
 * one-shot command run in a shell loop (like `enforce-intents` for fleet
 * intents, see `../orchestration/fleet-intent-enforcement.ts` +
 * `codex-supervisor.sh`): a `turn.start` dispatch launches a REAL Codex
 * turn in the background (fire-and-forget — see
 * `runtime-intent-enforcement.ts`'s module doc) so the SAME process's next
 * tick can observe a `turn.interrupt` for an already-running turn. If this
 * ran as a one-shot CLI invocation re-exec'd every few seconds by an outer
 * shell loop (the fleet-intents pattern), the process would exit at the
 * end of every tick and kill any in-flight background Codex dispatch with
 * it. So this script owns its own `while (true)` loop with a real
 * non-blocking `setTimeout`-based sleep between ticks (NEVER the
 * synchronous `Atomics.wait` sleep `supervisor-state.ts` uses for its
 * one-shot commands — that would freeze the event loop and stall every
 * in-flight Codex dispatch too).
 *
 * Run it directly:
 *
 *   OPENAGENTS_ADMIN_API_TOKEN=... OPENAGENTS_AGENT_TOKEN=... \
 *     bun apps/pylon/src/orchestration/runtime-intent-supervisor.ts \
 *     --pylon-home ~/.pylon --owner-user-id <linked-user-id>
 *
 * Stop it with SIGINT/SIGTERM (Ctrl-C) — the loop checks a `stopping` flag
 * between ticks and exits cleanly rather than mid-tick.
 */

const args = Bun.argv.slice(2)

const option = (name: string, fallback?: string): string | undefined => {
  const prefix = `${name}=`
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i]
    if (arg === name) return args[i + 1]
    if (arg?.startsWith(prefix)) return arg.slice(prefix.length)
  }
  return fallback
}

const intOption = (name: string, fallback: number): number => {
  const raw = option(name)
  if (raw === undefined || raw === "") return fallback
  const value = Number(raw)
  return Number.isFinite(value) && value >= 0 ? value : fallback
}

const required = (name: string, envValue: string | undefined): string => {
  const value = option(name) ?? envValue
  if (value === undefined || value.trim() === "") {
    console.error(`${name} is required (flag or environment variable)`)
    process.exit(2)
  }
  return value
}

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${Bun.hash(value).toString(16).padStart(16, "0")}`

// Bug found while testing #8425 cross-device dispatch: this used to default
// bare-unset-PYLON_HOME straight to `join(homedir(), ".pylon")`, reintroducing
// the exact "Orwell report" bug `bootstrap.ts`'s `selectPylonHomeResolution`
// was written to fix (an operator with a real seed/registry under
// `~/.openagents/pylon` silently got routed to an empty `~/.pylon` instead).
// An explicit `--pylon-home`/`PYLON_HOME` still always wins; only the
// bare-unset fallback changes, to the same smart resolution the rest of
// Pylon already uses.
const pylonHome = option("--pylon-home") ?? Bun.env.PYLON_HOME ?? resolvePylonHome(process.env).home
const dbPath = option("--db", join(pylonHome, "orchestration.sqlite"))!
const configPath = option("--config", join(pylonHome, "config.json"))!
const workspaceRoot = option("--workspace-root", join(pylonHome, "cache", "runtime-turns"))!
const baseUrl = option("--base-url") ?? Bun.env.OPENAGENTS_BASE_URL ?? Bun.env.PYLON_OPENAGENTS_BASE_URL ?? "https://openagents.com"
const adminToken = required("--admin-token", Bun.env.OPENAGENTS_ADMIN_API_TOKEN)
const agentToken = required("--agent-token", Bun.env.OPENAGENTS_AGENT_TOKEN)
const ownerUserId = option("--owner-user-id") ?? Bun.env.OPENAGENTS_RUNTIME_OWNER_USER_ID
const pylonRef = option("--pylon-ref") ?? stableRef("pylon.runtime_supervisor", pylonHome)
const pollIntervalMs = intOption("--poll-interval-ms", 3_000)
const limit = intOption("--limit", 20)

await mkdir(pylonHome, { recursive: true })
await mkdir(workspaceRoot, { recursive: true })

const db = new Database(dbPath)
db.exec("PRAGMA journal_mode = WAL")
db.exec("PRAGMA busy_timeout = 5000")
const store = createPylonOrchestrationStore(db)

const activeTurns: ActiveRuntimeTurns = new Map()
/** Persisted across ticks (like `activeTurns`) so `selectDispatchAccount`'s
 * round-robin tie-break actually has dispatch history to work from across
 * separate `turn.start` intents for the same thread — passing a FRESH Map
 * per call (or omitting this option) would make every dispatch resolve to
 * the same deterministic lowest-`accountRefHash` account every time. */
const lastDispatchedAccountByThread = new Map<string, string>()
const registrySummary = { paths: { config: configPath } }
/**
 * The fuller `BootstrapSummary`-shaped `paths` object real per-account
 * readiness checking needs (#8410 follow-up — see
 * `candidateAccountsFromRegistry`'s `summary` option): unlike
 * `registrySummary` above (only `config`, enough for
 * `loadPylonAccountRegistry`/`resolvePylonAccountSelection`),
 * `readinessForTarget` also reads the codex-account-health and quota ledgers
 * under `paths.home` — the SAME `<pylon home>` this supervisor and the
 * fleet-assignment executor both already write real health/quota records
 * into, so this reuses that history rather than starting a second one.
 */
const readinessSummary = {
  paths: {
    cache: join(pylonHome, "cache"),
    config: configPath,
    home: pylonHome,
    releases: join(pylonHome, "releases"),
  },
}

/**
 * `candidateAccountsFromRegistry` only projects `loadPylonAccountRegistry`'s
 * explicit `dev.accounts` entries — real, but NOT the same account set `pylon
 * accounts list` shows an operator (that CLI path also calls
 * `discoverPylonSiblingAccountHomes`, which finds real dispatch-ready
 * accounts living as sibling home directories, e.g. `~/.claude-pylon-2`,
 * that were never added to `dev.accounts`). Found while testing #8425
 * cross-device Claude dispatch from mobile: this Mac has zero explicit
 * `claude_agent` registry entries but real, ready, pooled-OAuth-token
 * sibling Claude homes — so every `claude_pylon` turn.start intent failed
 * with "no dispatch-ready local Claude account available" even though `pylon
 * accounts list --json` reported those same accounts `ready`. Merging
 * sibling-discovered accounts into the registry array here (deduped by
 * provider+home) gets this supervisor back to parity with what the CLI
 * already considers a valid dispatch target, without changing
 * `candidateAccountsFromRegistry`'s own contract or tests. */
const registryWithSiblingAccounts = async (): Promise<ReadonlyArray<PylonAccountRegistryEntry>> => {
  const registry = await loadPylonAccountRegistry(registrySummary)
  const seen = new Set(registry.map((entry) => `${entry.provider}:${entry.home}`))
  const siblings = await discoverPylonSiblingAccountHomes(Bun.env as Record<string, string | undefined>)
  const siblingEntries: PylonAccountRegistryEntry[] = siblings
    .filter((sibling) => !seen.has(`${sibling.provider}:${sibling.home}`))
    .map((sibling) => ({
      provider: sibling.provider,
      ref: sibling.ref,
      home: sibling.home,
      openAgentsProviderAccountRef: null,
      hourlyCap: null,
      weeklyCap: null,
      manualResetsRemaining: null,
    }))
  return [...registry, ...siblingEntries]
}

let stopping = false
const requestStop = (signal: string) => {
  console.error(`runtime-intent-supervisor: received ${signal}, stopping after the current tick`)
  stopping = true
}
process.on("SIGINT", () => requestStop("SIGINT"))
process.on("SIGTERM", () => requestStop("SIGTERM"))

const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

console.error(
  `runtime-intent-supervisor: starting (pylonRef=${pylonRef} owner=${ownerUserId ?? "ALL"} ` +
    `baseUrl=${baseUrl} pollIntervalMs=${pollIntervalMs})`,
)

try {
  while (!stopping) {
    try {
      const result = await enforcePendingRuntimeIntents(store, {
        activeTurns,
        adminToken,
        agentToken,
        baseUrl,
        ensureWorkspace: async (threadId) => {
          const dir = join(workspaceRoot, threadId)
          await mkdir(dir, { recursive: true })
          return dir
        },
        lastDispatchedAccountByThread,
        limit,
        listCandidateAccounts: async () =>
          candidateAccountsFromRegistry(await registryWithSiblingAccounts(), {
            env: Bun.env as Record<string, string | undefined>,
            summary: readinessSummary,
          }),
        log: (line) => console.error(`runtime-intent-supervisor: ${line}`),
        ...(ownerUserId === undefined ? {} : { ownerUserId }),
        pylonRef,
        resolveAccountSelection: (entry) =>
          resolvePylonAccountSelection(registrySummary, { accountRef: entry.ref, provider: entry.provider }),
        workspaceRoot,
      })
      if (!result.ok) {
        console.error(
          `runtime-intent-supervisor: poll failed error=${result.error} status=${result.status ?? "none"}`,
        )
      }
    } catch (error) {
      console.error(
        `runtime-intent-supervisor: tick threw unexpectedly (this should never happen — ` +
          `enforcePendingRuntimeIntents is documented never to throw): ` +
          `${error instanceof Error ? error.message : "unknown"}`,
      )
    }
    if (!stopping) await sleep(pollIntervalMs)
  }
} finally {
  console.error(`runtime-intent-supervisor: stopped (${activeTurns.size} turn(s) still running locally)`)
  db.close()
}
