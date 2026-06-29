import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { join } from "node:path"
import { parseLocalStateJsonEffect } from "@openagentsinc/effect-boundary"
import { Effect, Schema as S } from "effect"
import {
  assertPublicProjectionSafe,
  type PylonPaths,
} from "./state.js"

export type PylonCodingServiceRef = "claude" | "codex"

export type PylonActiveAssignmentRun = {
  schema: "openagents.pylon.active_assignment_run.v0.1"
  accountRefHash?: string
  assignmentRef: string
  leaseRef: string
  refreshedAt: string
  runRef: string
  service: PylonCodingServiceRef
  startedAt: string
}

export type PylonActiveCodingRunCounts = Partial<Record<PylonCodingServiceRef, number>>

// #6354: per-account active-run counts so the heartbeat can project each linked
// Codex account's own busy load. Keyed by service, then by the public-safe
// account-ref hash recorded on the active run. Runs without an account hash are
// folded into the `__unkeyed__` bucket so they still count toward pooled load.
export const UNKEYED_ACTIVE_RUN_ACCOUNT = "__unkeyed__"
export type PylonActiveCodingRunAccountCounts = Partial<
  Record<PylonCodingServiceRef, Record<string, number>>
>

export type PylonAssignmentLeaseLike = Readonly<{
  capabilityRefs?: ReadonlyArray<string>
  expiresAt?: string
}>

const PylonCodingServiceRef = S.Literals(["claude", "codex"])
const PylonActiveAssignmentRun = S.Struct({
  schema: S.Literal("openagents.pylon.active_assignment_run.v0.1"),
  accountRefHash: S.optional(S.String),
  assignmentRef: S.String,
  leaseRef: S.String,
  refreshedAt: S.String,
  runRef: S.String,
  service: PylonCodingServiceRef,
  startedAt: S.String,
})

const DEFAULT_ACTIVE_RUN_TTL_MS = 120_000
const CODEX_CAPABILITY_REF = "capability.pylon.local_codex"
const CLAUDE_CAPABILITY_REF = "capability.pylon.local_claude_agent"

const stableRef = (prefix: string, value: string) =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`

const activeRunFilename = (runRef: string) =>
  `${runRef.replace(/[^a-zA-Z0-9._-]+/g, "_")}.json`

const activeRunPath = (paths: PylonPaths, runRef: string) =>
  join(paths.activeAssignmentRuns, activeRunFilename(runRef))

const parseActiveRunFile = async (path: string): Promise<PylonActiveAssignmentRun | null> => {
  try {
    return await Effect.runPromise(
      parseLocalStateJsonEffect(
        PylonActiveAssignmentRun,
        await readFile(path, "utf8"),
        "pylon.active_assignment_run",
        "file.pylon.active_assignment_run",
      ),
    )
  } catch {
    return null
  }
}

const activeRunIsFresh = (
  run: PylonActiveAssignmentRun,
  now: Date,
  ttlMs: number,
) => now.getTime() - new Date(run.refreshedAt).getTime() <= ttlMs

export async function registerActiveCodingRun(
  paths: PylonPaths,
  input: {
    accountRefHash?: string
    assignmentRef: string
    leaseRef: string
    now?: Date
    service: PylonCodingServiceRef
  },
): Promise<PylonActiveAssignmentRun> {
  await mkdir(paths.activeAssignmentRuns, { recursive: true })
  const now = input.now ?? new Date()
  const run: PylonActiveAssignmentRun = {
    schema: "openagents.pylon.active_assignment_run.v0.1",
    ...(input.accountRefHash === undefined ? {} : { accountRefHash: input.accountRefHash }),
    assignmentRef: input.assignmentRef,
    leaseRef: input.leaseRef,
    refreshedAt: now.toISOString(),
    runRef: stableRef(
      "assignment_run.local",
      `${input.service}:${input.assignmentRef}:${input.leaseRef}:${randomUUID()}`,
    ),
    service: input.service,
    startedAt: now.toISOString(),
  }
  assertPublicProjectionSafe(run)
  await writeFile(activeRunPath(paths, run.runRef), `${JSON.stringify(run, null, 2)}\n`)
  return run
}

export async function refreshActiveCodingRun(
  paths: PylonPaths,
  runRef: string,
  input: { now?: Date } = {},
): Promise<void> {
  const path = activeRunPath(paths, runRef)
  if (!existsSync(path)) return
  const run = await parseActiveRunFile(path)
  if (run === null) return
  const next = {
    ...run,
    refreshedAt: (input.now ?? new Date()).toISOString(),
  }
  assertPublicProjectionSafe(next)
  await writeFile(path, `${JSON.stringify(next, null, 2)}\n`)
}

export async function finishActiveCodingRun(
  paths: PylonPaths,
  runRef: string,
): Promise<void> {
  await rm(activeRunPath(paths, runRef), { force: true })
}

export async function activeCodingRunCounts(
  paths: PylonPaths,
  input: { now?: Date; ttlMs?: number } = {},
): Promise<PylonActiveCodingRunCounts> {
  await mkdir(paths.activeAssignmentRuns, { recursive: true })
  const now = input.now ?? new Date()
  const ttlMs = input.ttlMs ?? DEFAULT_ACTIVE_RUN_TTL_MS
  const counts: PylonActiveCodingRunCounts = {}

  for (const filename of await readdir(paths.activeAssignmentRuns)) {
    if (!filename.endsWith(".json")) continue
    const path = join(paths.activeAssignmentRuns, filename)
    const run = await parseActiveRunFile(path)
    if (run === null || !activeRunIsFresh(run, now, ttlMs)) {
      await rm(path, { force: true })
      continue
    }
    counts[run.service] = (counts[run.service] ?? 0) + 1
  }

  return counts
}

// #6354: per-account busy load from fresh local active runs, keyed by the
// account-ref hash recorded when the run was registered. Stale runs are pruned
// like `activeCodingRunCounts`.
export async function activeCodingRunCountsByAccount(
  paths: PylonPaths,
  input: { now?: Date; ttlMs?: number } = {},
): Promise<PylonActiveCodingRunAccountCounts> {
  await mkdir(paths.activeAssignmentRuns, { recursive: true })
  const now = input.now ?? new Date()
  const ttlMs = input.ttlMs ?? DEFAULT_ACTIVE_RUN_TTL_MS
  const counts: PylonActiveCodingRunAccountCounts = {}

  for (const filename of await readdir(paths.activeAssignmentRuns)) {
    if (!filename.endsWith(".json")) continue
    const path = join(paths.activeAssignmentRuns, filename)
    const run = await parseActiveRunFile(path)
    if (run === null || !activeRunIsFresh(run, now, ttlMs)) {
      await rm(path, { force: true })
      continue
    }
    const accountKey =
      typeof run.accountRefHash === "string" && run.accountRefHash.trim() !== ""
        ? run.accountRefHash.trim()
        : UNKEYED_ACTIVE_RUN_ACCOUNT
    const byAccount = (counts[run.service] = counts[run.service] ?? {})
    byAccount[accountKey] = (byAccount[accountKey] ?? 0) + 1
  }

  return counts
}

const leaseIsUnexpired = (lease: PylonAssignmentLeaseLike, now: Date): boolean => {
  if (typeof lease.expiresAt !== "string") return false
  const expiresAt = Date.parse(lease.expiresAt)
  return Number.isFinite(expiresAt) && expiresAt > now.getTime()
}

export function activeCodingRunCountsFromAssignmentLeases(
  leases: ReadonlyArray<PylonAssignmentLeaseLike>,
  input: { now?: Date } = {},
): PylonActiveCodingRunCounts {
  const now = input.now ?? new Date()
  const counts: PylonActiveCodingRunCounts = {}

  for (const lease of leases) {
    if (!leaseIsUnexpired(lease, now)) continue
    const capabilityRefs = lease.capabilityRefs ?? []
    if (capabilityRefs.includes(CODEX_CAPABILITY_REF)) {
      counts.codex = (counts.codex ?? 0) + 1
    }
    if (capabilityRefs.includes(CLAUDE_CAPABILITY_REF)) {
      counts.claude = (counts.claude ?? 0) + 1
    }
  }

  return counts
}

export function maxActiveCodingRunCounts(
  ...counts: ReadonlyArray<PylonActiveCodingRunCounts | undefined>
): PylonActiveCodingRunCounts {
  const maxCounts: PylonActiveCodingRunCounts = {}

  for (const count of counts) {
    if (count === undefined) continue
    for (const service of ["claude", "codex"] as const) {
      maxCounts[service] = Math.max(maxCounts[service] ?? 0, count[service] ?? 0)
    }
  }

  return maxCounts
}
