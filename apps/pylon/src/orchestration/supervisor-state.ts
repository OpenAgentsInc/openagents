#!/usr/bin/env bun

import { Database } from "bun:sqlite"
import { createHash } from "node:crypto"
import { mkdir } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"
import {
  createPylonOrchestrationStore,
  type FleetRunWorkerKind,
  type OrchestrationRunnerKind,
} from "./store.js"

const args = Bun.argv.slice(2)

const fail = (message: string, code = 2): never => {
  console.error(message)
  process.exit(code)
}

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`

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
  if (!Number.isInteger(value) || value < 0) fail(`${name} must be a non-negative integer`)
  return value
}

const required = (name: string): string => {
  const value = option(name)
  if (value === undefined || value.trim() === "") fail(`${name} is required`)
  return value!
}

const print = (value: unknown): void => {
  process.stdout.write(`${JSON.stringify(value)}\n`)
}

const sleepSync = (ms: number): void => {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms)
}

const isSqliteBusy = (error: unknown): boolean =>
  error instanceof Error &&
  (error.message.includes("SQLITE_BUSY") || error.message.includes("database is locked"))

const withSqliteBusyRetry = <T>(fn: () => T): T => {
  let lastError: unknown
  for (let attempt = 0; attempt < 4; attempt += 1) {
    try {
      return fn()
    } catch (error) {
      if (!isSqliteBusy(error) || attempt === 3) throw error
      lastError = error
      sleepSync(25 * (attempt + 1))
    }
  }
  throw lastError
}

const pylonHome = option("--pylon-home") ?? Bun.env.PYLON_HOME ?? join(homedir(), ".pylon")
const dbPath = option("--db", join(pylonHome, "orchestration.sqlite"))!
await mkdir(pylonHome, { recursive: true })
const db = new Database(dbPath)
withSqliteBusyRetry(() => {
  db.exec("PRAGMA journal_mode = WAL")
  db.exec("PRAGMA busy_timeout = 5000")
})
const store = withSqliteBusyRetry(() => createPylonOrchestrationStore(db))

const supervisor = required("--supervisor")
const kind = required("--kind") as FleetRunWorkerKind
if (kind !== "codex" && kind !== "claude" && kind !== "auto") fail("--kind must be codex, claude, or auto")
const runnerKind: OrchestrationRunnerKind = kind === "claude" ? "claude_agent" : kind === "auto" ? "generic" : "codex"
const runRef = option("--run-ref", stableRef("fleet-run.pylon.supervisor", supervisor))!
const now = new Date()

const ensureRun = () =>
  store.getFleetRun(runRef) ??
  store.createFleetRun({
    runRef,
    objective: `${kind} supervisor process launcher`,
    workSource: "github_backlog",
    targetConcurrency: Math.max(1, intOption("--target-concurrency", 1)),
    workerKind: kind,
    state: "running",
    dispatchKind: "supervised_dispatch",
    now,
  })

const ensureContext = (accountRef: string, slot?: string) => {
  const contextId = stableRef("dispatch-context.pylon.supervisor", `${supervisor}:${kind}:${accountRef}:${slot ?? "slot"}`)
  const existing = store.getDispatchContext(contextId)
  if (existing !== null) return existing
  const accountRefHash = accountRef.startsWith("account.pylon.")
    ? accountRef
    : stableRef(`account.pylon.${runnerKind}`, accountRef)
  return store.createDispatchContext({
    id: contextId,
    assigneeHandle: accountRef,
    accountRefHash,
    lane: runnerKind,
    runnerKind,
    maxConcurrentSlots: 1,
    lastHeartbeatAt: now,
    now,
  })
}

const COMMANDS = [
  "sync",
  "desired-slots",
  "pause",
  "resume",
  "heartbeat",
  "live-claim",
  "try-claim",
  "refresh-claim",
  "release-claim",
  "dispatch-attempt",
  "worker-done",
  "reconcile",
] as const

const command = args.find((arg): arg is typeof COMMANDS[number] =>
  (COMMANDS as readonly string[]).includes(arg),
) ?? fail("command is required")

try {
  withSqliteBusyRetry(() => {
    switch (command) {
      case "sync": {
        const run = ensureRun()
        const desiredSlots = intOption("--desired-slots", run.targetConcurrency)
        const pausedFlag = option("--paused")
        const updated = store.upsertFleetRun({
          ...run,
          state: pausedFlag === "1" ? "paused" : pausedFlag === "0" ? "running" : run.state,
          ...(pausedFlag === "1" || pausedFlag === "0" ? { stateSource: "operator" as const } : {}),
          targetConcurrency: Math.max(1, desiredSlots),
          counters: {
            ...run.counters,
            activeAssignments: store.listLiveWorkClaims(now).filter((claim) => claim.runRef === runRef).length,
          },
          updatedAt: now.toISOString(),
        })
        print({ ok: true, dbPath, run: updated, desiredSlots })
        break
      }
      case "desired-slots": {
        const run = ensureRun()
        print({ ok: true, desiredSlots: run.state === "paused" ? 0 : run.targetConcurrency, state: run.state })
        break
      }
      case "pause": {
        const run = ensureRun()
        const updated = store.updateFleetRunState(run.runRef, "paused", now)
        print({ ok: true, run: updated })
        break
      }
      case "resume": {
        const run = ensureRun()
        const updated = store.updateFleetRunState(run.runRef, "running", now)
        print({ ok: true, run: updated })
        break
      }
      case "heartbeat": {
        ensureRun()
        const accountRef = required("--account-ref")
        const slot = option("--slot")
        const context = ensureContext(accountRef, slot)
        const updated = store.recordHeartbeat(context.id, { at: now, status: "idle" })
        print({ ok: true, context: updated })
        break
      }
      case "try-claim": {
        const run = ensureRun()
        const issue = required("--issue")
        const accountRef = required("--account-ref")
        const slot = option("--slot")
        const workUnitRef = stableRef("work-unit.github-issue", `${kind}:${issue}`)
        const claim = store.tryClaimWorkUnit({
          claimRef: stableRef("claim.pylon.supervisor", `${runRef}:${workUnitRef}:${accountRef}:${slot ?? ""}:${now.toISOString()}`),
          workUnitRef,
          runRef: run.runRef,
          workerAccountRef: accountRef,
          ttl: intOption("--ttl-ms", intOption("--ttl-secs", 1800) * 1000),
          now,
        })
        if (claim === null) {
          print({ ok: false, claimed: false, workUnitRef })
        } else {
          ensureContext(accountRef, slot)
          print({ ok: true, claimed: true, claim })
        }
        break
      }
      case "live-claim": {
        ensureRun()
        const issue = required("--issue")
        const workUnitRef = stableRef("work-unit.github-issue", `${kind}:${issue}`)
        const claim = store.getLiveWorkClaim(workUnitRef, now)
        print({ ok: true, active: claim !== null, workUnitRef, claim: claim ?? null })
        break
      }
      case "refresh-claim": {
        ensureRun()
        const issue = required("--issue")
        const claim = store.refreshLiveWorkClaim(stableRef("work-unit.github-issue", `${kind}:${issue}`), now)
        print({ ok: true, refreshed: claim !== null, claim: claim ?? null })
        break
      }
      case "release-claim": {
        ensureRun()
        const issue = required("--issue")
        const claim = store.releaseLiveWorkClaim(stableRef("work-unit.github-issue", `${kind}:${issue}`), now)
        print({ ok: true, released: claim !== null, claim: claim ?? null })
        break
      }
      case "dispatch-attempt": {
        ensureRun()
        const accountRef = required("--account-ref")
        const slot = option("--slot")
        const issue = option("--issue")
        const context = ensureContext(accountRef, slot)
        const breaker = store.getActiveDispatchBreakerForContext(context, now)
        if (breaker !== null) {
          print({
            ok: false,
            reason: breaker.failureKind === "permanent"
              ? "dispatch_breaker_permanent"
              : "dispatch_breaker_cooling_down",
            breaker,
          })
          break
        }
        const taskId = issue === undefined ? null : stableRef("task.pylon.supervisor", `${runRef}:${kind}:${issue}`)
        if (taskId !== null && store.getTask(taskId) === null) {
          store.createTask({
            id: taskId,
            spec: {
              title: `${kind} supervisor issue #${issue}`,
              prompt: `Implement public issue #${issue}.`,
              runnerKind,
              issueRef: `#${issue}`,
              fleetRunRef: runRef,
            },
            now,
          })
        }
        if (taskId !== null) store.markDispatched(taskId, context.id, now)
        print({ ok: true, contextId: context.id, taskId })
        break
      }
      case "worker-done": {
        ensureRun()
        const accountRef = required("--account-ref")
        const slot = option("--slot")
        const issue = option("--issue")
        const status = option("--status", "completed")
        const failureText = option("--failure") ?? option("--error") ?? option("--result") ?? status
        const context = ensureContext(accountRef, slot)
        const taskId = issue === undefined ? context.currentTaskId : stableRef("task.pylon.supervisor", `${runRef}:${kind}:${issue}`)
        if (taskId !== null && store.getTask(taskId) !== null && context.currentTaskId === taskId) {
          store.recordWorkerDone({
            contextId: context.id,
            taskId,
            status: status === "completed" ? "completed" : status === "blocked" ? "blocked" : "failed",
            ...(status === "completed" || status === "blocked"
              ? {}
              : { failure: { error: failureText, status } }),
            result: option("--result") ?? null,
            now,
          })
        } else {
          store.releaseDispatchContext(context.id, status === "completed" ? "completed" : "failed", now)
        }
        print({ ok: true, contextId: context.id, taskId })
        break
      }
      case "reconcile": {
        const run = ensureRun()
        const claims = store.reconcileWorkClaims({ now })
        const reconciled = store.reconcileFleetRun(run.runRef, now)
        print({ ok: true, claims, run: reconciled })
        break
      }
    }
  })
} finally {
  db.close()
}
