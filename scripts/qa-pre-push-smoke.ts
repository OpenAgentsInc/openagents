#!/usr/bin/env bun
// Warning-only Tier 1 QA pre-push smoke (#6245).
//
// This script mirrors the desktop "run only when changed" gate, but keeps QA
// off the blocking path: it scopes to user-facing surfaces, runs the deterministic
// qa-runner --fake-model loop under a hard wall-clock bound, and exits non-zero
// only so the hook can print a warning while still letting check:deploy own the
// push decision.

import { execFileSync } from "node:child_process"
import { mkdirSync } from "node:fs"
import { join, resolve } from "node:path"

export const QA_PRE_PUSH_DEFAULT_TIMEOUT_MS = 60_000

export const USER_FACING_SURFACE_PREFIXES = [
  ".githooks/pre-push",
  "scripts/qa-pre-push-smoke",
  "apps/openagents.com/apps/web/",
  "apps/openagents.com/workers/api/src/",
  "apps/openagents.com/workers/api/migrations/",
  "apps/openagents.com/workers/api/wrangler",
  "apps/qa-runner/",
  "apps/forum/",
  "apps/openagents-world/",
  "apps/pylon/",
  "clients/khala-ios/",
  "packages/autopilot-ui/",
  "packages/design-tokens/",
  "packages/input-bindings/",
  "packages/ui/",
] as const

export type QaPrePushSmokeVerdict = Readonly<{
  artifactDir?: string
  changedFiles: ReadonlyArray<string>
  command?: ReadonlyArray<string>
  elapsedMs?: number
  exitCode: number
  reason?: string
  status: "skipped" | "passed" | "failed" | "incomplete"
  surfaceFiles: ReadonlyArray<string>
}>

const splitLines = (value: string): ReadonlyArray<string> =>
  value
    .split("\n")
    .map(line => line.trim())
    .filter(Boolean)

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  Array.from(new Set(values)).sort((a, b) => a.localeCompare(b))

export const isUserFacingSurfaceChange = (path: string): boolean =>
  USER_FACING_SURFACE_PREFIXES.some(prefix => path === prefix || path.startsWith(prefix))

export const selectUserFacingSurfaceFiles = (
  changedFiles: ReadonlyArray<string>,
): ReadonlyArray<string> => uniqueSorted(changedFiles.filter(isUserFacingSurfaceChange))

export const qaPrePushShouldRun = (
  input: Readonly<{
    changedFiles: ReadonlyArray<string>
    force?: boolean | undefined
  }>,
): Readonly<{ run: boolean; surfaceFiles: ReadonlyArray<string> }> => {
  const surfaceFiles = selectUserFacingSurfaceFiles(input.changedFiles)
  return {
    run: input.force === true || surfaceFiles.length > 0,
    surfaceFiles,
  }
}

const git = (
  root: string,
  args: ReadonlyArray<string>,
): string => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim()

export const collectChangedFiles = (
  root: string,
): ReadonlyArray<string> => {
  try {
    git(root, ["fetch", "origin", "main", "--quiet"])
  } catch {
    return ["apps/openagents.com/apps/web/__fetch_failed_run_conservatively__"]
  }

  const changed: Array<string> = []
  for (const range of ["origin/main...HEAD", "HEAD"]) {
    try {
      changed.push(...splitLines(git(root, ["diff", "--name-only", range])))
    } catch {
      // A missing ref should not make QA silently disappear; the caller can force
      // the smoke by treating the synthetic path as changed.
      changed.push("apps/openagents.com/apps/web/__diff_failed_run_conservatively__")
    }
  }

  return uniqueSorted(changed)
}

const positiveIntFromEnv = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number.parseInt(value ?? "", 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

export const resolveQaPrePushTimeoutMs = (
  env: Readonly<Record<string, string | undefined>> = process.env,
): number => positiveIntFromEnv(env.OA_QA_PRE_PUSH_TIMEOUT_MS, QA_PRE_PUSH_DEFAULT_TIMEOUT_MS)

export const buildQaPrePushCommand = (
  artifactDir: string,
): ReadonlyArray<string> => [
  "bun",
  "run",
  "--cwd",
  "apps/qa-runner",
  "qa",
  "run",
  "--fake-model",
  "--url",
  "https://example.test",
  "--goal",
  "pre-push smoke: verify the deterministic login scenario reaches a pass verdict",
  "--out",
  artifactDir,
  "--emit",
  join(artifactDir, "generated", "pre-push.e2e.test.ts"),
  "--max-turns",
  "8",
]

export const runBoundedCommand = async (
  input: Readonly<{
    command: ReadonlyArray<string>
    cwd: string
    timeoutMs: number
  }>,
): Promise<Readonly<{ elapsedMs: number; exitCode: number; timedOut: boolean }>> => {
  const startedAt = Date.now()
  const child = Bun.spawn({
    cmd: [...input.command],
    cwd: input.cwd,
    detached: true,
    stderr: "inherit",
    stdin: "inherit",
    stdout: "inherit",
  })

  let timedOut = false
  const killTree = (signal: NodeJS.Signals) => {
    try {
      process.kill(-child.pid, signal)
    } catch {
      try {
        child.kill(signal)
      } catch {
        // already exited
      }
    }
  }

  let killTimer: ReturnType<typeof setTimeout> | undefined
  const timer = setTimeout(() => {
    timedOut = true
    console.error(
      `[qa-pre-push] INCOMPLETE: smoke exceeded ${input.timeoutMs}ms; terminating child process group.`,
    )
    killTree("SIGTERM")
    killTimer = setTimeout(() => killTree("SIGKILL"), 5_000)
  }, input.timeoutMs)

  const exitCode = await child.exited
  clearTimeout(timer)
  if (killTimer !== undefined) clearTimeout(killTimer)

  return {
    elapsedMs: Date.now() - startedAt,
    exitCode: timedOut ? 124 : exitCode ?? 1,
    timedOut,
  }
}

export const runQaPrePushSmoke = async (
  root = resolve(process.cwd()),
  env: Readonly<Record<string, string | undefined>> = process.env,
): Promise<QaPrePushSmokeVerdict> => {
  const changedFiles = collectChangedFiles(root)
  const { run, surfaceFiles } = qaPrePushShouldRun({
    changedFiles,
    force: env.OA_FORCE_QA_PRE_PUSH === "1",
  })

  if (!run) {
    return {
      changedFiles,
      exitCode: 0,
      reason: "no user-facing surface changes vs origin/main",
      status: "skipped",
      surfaceFiles,
    }
  }

  const artifactDir = join(root, "var", "qa-pre-push-smoke", String(Date.now()))
  mkdirSync(artifactDir, { recursive: true })
  const command = buildQaPrePushCommand(artifactDir)
  const timeoutMs = resolveQaPrePushTimeoutMs(env)

  console.error(
    `[qa-pre-push] user-facing changes detected (${surfaceFiles.length}); running warning-only smoke with ${timeoutMs}ms timeout.`,
  )
  console.error(`[qa-pre-push] artifacts: ${artifactDir}`)

  const result = await runBoundedCommand({
    command,
    cwd: root,
    timeoutMs,
  })

  if (result.timedOut) {
    return {
      artifactDir,
      changedFiles,
      command,
      elapsedMs: result.elapsedMs,
      exitCode: 124,
      reason: "bounded smoke timed out; Tier 2 async QA remains authoritative",
      status: "incomplete",
      surfaceFiles,
    }
  }

  if (result.exitCode === 0) {
    return {
      artifactDir,
      changedFiles,
      command,
      elapsedMs: result.elapsedMs,
      exitCode: 0,
      status: "passed",
      surfaceFiles,
    }
  }

  return {
    artifactDir,
    changedFiles,
    command,
    elapsedMs: result.elapsedMs,
    exitCode: result.exitCode,
    reason: "qa-runner smoke returned a non-zero verdict",
    status: "failed",
    surfaceFiles,
  }
}

if (import.meta.main) {
  const verdict = await runQaPrePushSmoke()
  if (verdict.status === "skipped") {
    console.error(`[qa-pre-push] SKIPPED: ${verdict.reason}. Set OA_FORCE_QA_PRE_PUSH=1 to run anyway.`)
  } else if (verdict.status === "passed") {
    console.error(`[qa-pre-push] PASS: deterministic QA smoke completed in ${verdict.elapsedMs}ms.`)
  } else if (verdict.status === "incomplete") {
    console.error(`[qa-pre-push] INCOMPLETE: ${verdict.reason}. Push is allowed to continue by the hook.`)
  } else {
    console.error(`[qa-pre-push] WARNING: ${verdict.reason}. Push is allowed to continue by the hook.`)
  }

  process.exit(verdict.exitCode)
}
