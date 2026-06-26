import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises"
import { existsSync } from "node:fs"
import { createHash, randomUUID } from "node:crypto"
import { join } from "node:path"
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

const DEFAULT_ACTIVE_RUN_TTL_MS = 120_000

const stableRef = (prefix: string, value: string) =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`

const activeRunFilename = (runRef: string) =>
  `${runRef.replace(/[^a-zA-Z0-9._-]+/g, "_")}.json`

const activeRunPath = (paths: PylonPaths, runRef: string) =>
  join(paths.activeAssignmentRuns, activeRunFilename(runRef))

const activeRunFromUnknown = (value: unknown): PylonActiveAssignmentRun | null => {
  const record = value as PylonActiveAssignmentRun
  return record?.schema === "openagents.pylon.active_assignment_run.v0.1" &&
    (record.service === "codex" || record.service === "claude") &&
    typeof record.assignmentRef === "string" &&
    typeof record.leaseRef === "string" &&
    typeof record.refreshedAt === "string" &&
    typeof record.runRef === "string" &&
    typeof record.startedAt === "string"
    ? record
    : null
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
  const run = activeRunFromUnknown(JSON.parse(await readFile(path, "utf8")) as unknown)
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
    let run: PylonActiveAssignmentRun | null = null
    try {
      run = activeRunFromUnknown(JSON.parse(await readFile(path, "utf8")) as unknown)
    } catch {
      run = null
    }
    if (run === null || !activeRunIsFresh(run, now, ttlMs)) {
      await rm(path, { force: true })
      continue
    }
    counts[run.service] = (counts[run.service] ?? 0) + 1
  }

  return counts
}
