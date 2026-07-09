import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { join } from "node:path"
import { parseJsonEffect } from "@openagentsinc/effect-boundary"
import { Effect, Schema as S } from "effect"
import {
  assertPublicProjectionSafe,
  type PylonPaths,
} from "../shared/state.js"

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
  codingAssignment?: unknown
  expiresAt?: string
}>

const DEFAULT_ACTIVE_RUN_TTL_MS = 120_000
const CODEX_CAPABILITY_REF = "capability.pylon.local_codex"
const CLAUDE_CAPABILITY_REF = "capability.pylon.local_claude_agent"
const PylonCodingServiceRefSchema = S.Literals(["claude", "codex"])
const PylonActiveAssignmentRunSchema = S.Struct({
  schema: S.Literal("openagents.pylon.active_assignment_run.v0.1"),
  accountRefHash: S.optional(S.String),
  assignmentRef: S.String,
  leaseRef: S.String,
  refreshedAt: S.String,
  runRef: S.String,
  service: PylonCodingServiceRefSchema,
  startedAt: S.String,
})

const stableRef = (prefix: string, value: string) =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`

const activeRunFilename = (runRef: string) =>
  `${runRef.replace(/[^a-zA-Z0-9._-]+/g, "_")}.json`

const activeRunPath = (paths: PylonPaths, runRef: string) =>
  join(paths.activeAssignmentRuns, activeRunFilename(runRef))

const readActiveRunFile = async (
  path: string,
): Promise<PylonActiveAssignmentRun | null> =>
  Effect.runPromise(
    parseJsonEffect(
      PylonActiveAssignmentRunSchema,
      await readFile(path, "utf8"),
      "pylon.active_assignment_run.file",
    ).pipe(Effect.catch(() => Effect.succeed(null))),
  )

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
  const run = await readActiveRunFile(path)
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
  const runs = await activeCodingRuns(paths, input)
  const counts: PylonActiveCodingRunCounts = {}

  for (const run of runs) {
    counts[run.service] = (counts[run.service] ?? 0) + 1
  }

  return counts
}

export async function activeCodingRuns(
  paths: PylonPaths,
  input: { now?: Date; ttlMs?: number } = {},
): Promise<PylonActiveAssignmentRun[]> {
  await mkdir(paths.activeAssignmentRuns, { recursive: true })
  const now = input.now ?? new Date()
  const ttlMs = input.ttlMs ?? DEFAULT_ACTIVE_RUN_TTL_MS
  const runs: PylonActiveAssignmentRun[] = []

  for (const filename of await readdir(paths.activeAssignmentRuns)) {
    if (!filename.endsWith(".json")) continue
    const path = join(paths.activeAssignmentRuns, filename)
    let run: PylonActiveAssignmentRun | null = null
    try {
      run = await readActiveRunFile(path)
    } catch {
      run = null
    }
    if (run === null || !activeRunIsFresh(run, now, ttlMs)) {
      await rm(path, { force: true })
      continue
    }
    runs.push(run)
  }

  return runs.sort((left, right) => left.startedAt.localeCompare(right.startedAt))
}

// #6354: per-account busy load from fresh local active runs, keyed by the
// account-ref hash recorded when the run was registered. Stale runs are pruned
// like `activeCodingRunCounts`.
export async function activeCodingRunCountsByAccount(
  paths: PylonPaths,
  input: { now?: Date; ttlMs?: number } = {},
): Promise<PylonActiveCodingRunAccountCounts> {
  const runs = await activeCodingRuns(paths, input)
  const counts: PylonActiveCodingRunAccountCounts = {}

  for (const run of runs) {
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

const accountRefHashFromCodingPayload = (value: unknown): string | null => {
  if (value === null || typeof value !== "object") return null
  const accountRefHash = (value as { accountRefHash?: unknown }).accountRefHash
  return typeof accountRefHash === "string" && accountRefHash.trim() !== ""
    ? accountRefHash.trim()
    : null
}

const accountRefHashForLeaseService = (
  lease: PylonAssignmentLeaseLike,
  service: PylonCodingServiceRef,
): string => {
  const codingAssignment = lease.codingAssignment
  if (codingAssignment !== null && typeof codingAssignment === "object") {
    const payload =
      service === "codex"
        ? (codingAssignment as { codex?: unknown }).codex
        : (codingAssignment as { claudeAgent?: unknown }).claudeAgent
    const accountRefHash = accountRefHashFromCodingPayload(payload)
    if (accountRefHash !== null) return accountRefHash
  }
  return UNKEYED_ACTIVE_RUN_ACCOUNT
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

export function activeCodingRunCountsByAccountFromAssignmentLeases(
  leases: ReadonlyArray<PylonAssignmentLeaseLike>,
  input: { now?: Date } = {},
): PylonActiveCodingRunAccountCounts {
  const now = input.now ?? new Date()
  const counts: PylonActiveCodingRunAccountCounts = {}

  for (const lease of leases) {
    if (!leaseIsUnexpired(lease, now)) continue
    const capabilityRefs = lease.capabilityRefs ?? []
    for (const service of ["codex", "claude"] as const) {
      const capabilityRef =
        service === "codex" ? CODEX_CAPABILITY_REF : CLAUDE_CAPABILITY_REF
      if (!capabilityRefs.includes(capabilityRef)) continue
      const accountKey = accountRefHashForLeaseService(lease, service)
      const byAccount = (counts[service] = counts[service] ?? {})
      byAccount[accountKey] = (byAccount[accountKey] ?? 0) + 1
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

export function maxActiveCodingRunAccountCounts(
  ...counts: ReadonlyArray<PylonActiveCodingRunAccountCounts | undefined>
): PylonActiveCodingRunAccountCounts {
  const maxCounts: PylonActiveCodingRunAccountCounts = {}

  for (const count of counts) {
    if (count === undefined) continue
    for (const service of ["claude", "codex"] as const) {
      const byAccount = count[service]
      if (byAccount === undefined) continue
      const maxByAccount = (maxCounts[service] = maxCounts[service] ?? {})
      for (const [accountRefHash, value] of Object.entries(byAccount)) {
        maxByAccount[accountRefHash] = Math.max(
          maxByAccount[accountRefHash] ?? 0,
          Math.max(0, value),
        )
      }
    }
  }

  return maxCounts
}
