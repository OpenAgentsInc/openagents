#!/usr/bin/env bun

import { Database } from "bun:sqlite"
import { mkdir } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"
import { loadPylonAccountRegistry, resolvePylonAccountSelection } from "../account-registry.js"
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

const pylonHome = option("--pylon-home") ?? Bun.env.PYLON_HOME ?? join(homedir(), ".pylon")
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
const registrySummary = { paths: { config: configPath } }

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
        limit,
        listCandidateAccounts: async () =>
          candidateAccountsFromRegistry(await loadPylonAccountRegistry(registrySummary)),
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
