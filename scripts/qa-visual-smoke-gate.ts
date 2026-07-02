#!/usr/bin/env bun
// Scoped Khala Code visual-smoke gate for desktop/UI-touching main pushes.

import { execFileSync } from "node:child_process"
import { mkdirSync } from "node:fs"
import { join, resolve } from "node:path"
import { runBoundedCommand } from "./qa-pre-push-smoke"

export const KHALA_VISUAL_SMOKE_GATE_SCHEMA = "openagents.khala_code.visual_smoke_gate.v1"
export const KHALA_VISUAL_SMOKE_GATE_HARD_FAIL_DATE = "2026-07-09"
export const KHALA_VISUAL_SMOKE_GATE_DEFAULT_TIMEOUT_MS = 295_000

export const KHALA_VISUAL_SMOKE_GATE_PREFIXES = [
  "clients/khala-code-desktop/",
  "packages/ui/",
] as const

export type KhalaVisualSmokeGateMode = "warning-only" | "hard-fail"
export type KhalaVisualSmokeGateStatus = "skipped" | "passed" | "failed" | "incomplete"
export type KhalaVisualSmokeGateStepId = "part2-ui" | "cockpit-visual" | "composer-visual"

export type KhalaVisualSmokeGateStep = Readonly<{
  command: ReadonlyArray<string>
  id: KhalaVisualSmokeGateStepId
  label: string
}>

export type KhalaVisualSmokeGateStepResult = Readonly<{
  elapsedMs: number
  exitCode: number
  id: KhalaVisualSmokeGateStepId
  timedOut: boolean
}>

export type KhalaVisualSmokeGateVerdict = Readonly<{
  artifactDir?: string
  changedFiles: ReadonlyArray<string>
  exitCode: number
  hardFailDate: string
  mode: KhalaVisualSmokeGateMode
  reason?: string
  schema: typeof KHALA_VISUAL_SMOKE_GATE_SCHEMA
  status: KhalaVisualSmokeGateStatus
  steps: ReadonlyArray<KhalaVisualSmokeGateStepResult>
  timeoutMs: number
  visualFiles: ReadonlyArray<string>
}>

export type KhalaVisualSmokeGateRunInput = Readonly<{
  changedFiles?: ReadonlyArray<string>
  env?: Readonly<Record<string, string | undefined>>
  root?: string
  runCommand?: typeof runBoundedCommand
}>

const ZERO_SHA = "0000000000000000000000000000000000000000"

const splitLines = (value: string): ReadonlyArray<string> =>
  value
    .split("\n")
    .map(line => line.trim())
    .filter(Boolean)

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  Array.from(new Set(values)).sort((a, b) => a.localeCompare(b))

const git = (
  root: string,
  args: ReadonlyArray<string>,
): string => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim()

const positiveIntFromEnv = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number.parseInt(value ?? "", 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const todayIsoDate = (
  env: Readonly<Record<string, string | undefined>>,
): string => env.OA_KHALA_VISUAL_SMOKE_GATE_TODAY ?? new Date().toISOString().slice(0, 10)

export const khalaVisualSmokeGateSteps = (): ReadonlyArray<KhalaVisualSmokeGateStep> => [
  {
    command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:part2-ui"],
    id: "part2-ui",
    label: "Part 2 UI fixture visual smoke",
  },
  {
    command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:cockpit-visual"],
    id: "cockpit-visual",
    label: "Fleet cockpit fixture visual smoke",
  },
  {
    command: ["bun", "run", "--cwd", "clients/khala-code-desktop", "smoke:composer-visual"],
    id: "composer-visual",
    label: "Composer fixture visual smoke",
  },
]

export const isKhalaVisualSmokeGateFile = (path: string): boolean =>
  KHALA_VISUAL_SMOKE_GATE_PREFIXES.some(prefix => {
    const directory = prefix.endsWith("/") ? prefix.slice(0, -1) : prefix
    return path === directory || path.startsWith(prefix)
  })

export const selectKhalaVisualSmokeGateFiles = (
  changedFiles: ReadonlyArray<string>,
): ReadonlyArray<string> => uniqueSorted(changedFiles.filter(isKhalaVisualSmokeGateFile))

export const khalaVisualSmokeGateShouldRun = (
  input: Readonly<{
    changedFiles: ReadonlyArray<string>
    force?: boolean | undefined
  }>,
): Readonly<{ run: boolean; visualFiles: ReadonlyArray<string> }> => {
  const visualFiles = selectKhalaVisualSmokeGateFiles(input.changedFiles)
  return {
    run: input.force === true || visualFiles.length > 0,
    visualFiles,
  }
}

export const resolveKhalaVisualSmokeGateMode = (
  env: Readonly<Record<string, string | undefined>> = process.env,
): KhalaVisualSmokeGateMode => {
  const override = env.OA_KHALA_VISUAL_SMOKE_GATE_MODE?.toLowerCase()
  if (override === "hard" || override === "hard-fail" || override === "enforce") return "hard-fail"
  if (override === "warn" || override === "warning" || override === "warning-only") return "warning-only"
  return todayIsoDate(env) >= KHALA_VISUAL_SMOKE_GATE_HARD_FAIL_DATE ? "hard-fail" : "warning-only"
}

export const resolveKhalaVisualSmokeGateTimeoutMs = (
  env: Readonly<Record<string, string | undefined>> = process.env,
): number => positiveIntFromEnv(env.OA_KHALA_VISUAL_SMOKE_GATE_TIMEOUT_MS, KHALA_VISUAL_SMOKE_GATE_DEFAULT_TIMEOUT_MS)

export const collectKhalaVisualSmokeGateChangedFiles = (
  root: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): ReadonlyArray<string> => {
  const localSha = env.OPENAGENTS_PRE_PUSH_LOCAL_SHA
  const remoteSha = env.OPENAGENTS_PRE_PUSH_REMOTE_SHA
  if (localSha !== undefined && localSha !== "" && localSha !== ZERO_SHA) {
    try {
      if (remoteSha !== undefined && remoteSha !== "" && remoteSha !== ZERO_SHA) {
        return uniqueSorted(splitLines(git(root, ["diff", "--name-only", `${remoteSha}..${localSha}`])))
      }
      return uniqueSorted(splitLines(git(root, ["diff-tree", "--no-commit-id", "--name-only", "-r", localSha])))
    } catch {
      return ["clients/khala-code-desktop/__pre_push_diff_failed_run_conservatively__"]
    }
  }

  try {
    git(root, ["fetch", "origin", "main", "--quiet"])
  } catch {
    return ["clients/khala-code-desktop/__fetch_failed_run_conservatively__"]
  }

  const changed: Array<string> = []
  for (const range of ["origin/main...HEAD", "HEAD"]) {
    try {
      changed.push(...splitLines(git(root, ["diff", "--name-only", range])))
    } catch {
      changed.push("clients/khala-code-desktop/__diff_failed_run_conservatively__")
    }
  }

  return uniqueSorted(changed)
}

const gateExitCode = (
  status: KhalaVisualSmokeGateStatus,
  mode: KhalaVisualSmokeGateMode,
): number => {
  if (status === "skipped" || status === "passed") return 0
  return mode === "hard-fail" ? 1 : 0
}

export const runKhalaVisualSmokeGate = async (
  input: KhalaVisualSmokeGateRunInput = {},
): Promise<KhalaVisualSmokeGateVerdict> => {
  const root = input.root ?? resolve(process.cwd())
  const env = input.env ?? process.env
  const mode = resolveKhalaVisualSmokeGateMode(env)
  const timeoutMs = resolveKhalaVisualSmokeGateTimeoutMs(env)
  const changedFiles = input.changedFiles ?? collectKhalaVisualSmokeGateChangedFiles(root, env)
  const { run, visualFiles } = khalaVisualSmokeGateShouldRun({
    changedFiles,
    force: env.OA_FORCE_KHALA_VISUAL_SMOKE_GATE === "1",
  })

  if (!run) {
    return {
      changedFiles,
      exitCode: 0,
      hardFailDate: KHALA_VISUAL_SMOKE_GATE_HARD_FAIL_DATE,
      mode,
      reason: "no Khala desktop or shared UI changes vs the pushed main range",
      schema: KHALA_VISUAL_SMOKE_GATE_SCHEMA,
      status: "skipped",
      steps: [],
      timeoutMs,
      visualFiles,
    }
  }

  const artifactDir = join(root, "var", "khala-code-visual-smoke-gate", String(Date.now()))
  mkdirSync(artifactDir, { recursive: true })

  const runCommand = input.runCommand ?? runBoundedCommand
  const startedAt = Date.now()
  const results: Array<KhalaVisualSmokeGateStepResult> = []

  for (const step of khalaVisualSmokeGateSteps()) {
    const elapsedTotalMs = Date.now() - startedAt
    const remainingMs = Math.max(1, timeoutMs - elapsedTotalMs)
    console.error(`[khala-visual-smoke-gate] running ${step.id}: ${step.label}`)
    const result = await runCommand({
      command: step.command,
      cwd: root,
      timeoutMs: remainingMs,
    })
    results.push({
      elapsedMs: result.elapsedMs,
      exitCode: result.exitCode,
      id: step.id,
      timedOut: result.timedOut,
    })

    if (result.timedOut || result.exitCode !== 0) break
  }

  const timedOut = results.some(result => result.timedOut)
  const failed = results.some(result => result.exitCode !== 0)
  const status: KhalaVisualSmokeGateStatus =
    timedOut ? "incomplete" : failed ? "failed" : "passed"
  const reason =
    status === "passed"
      ? undefined
      : timedOut
        ? `visual smoke gate exceeded ${timeoutMs}ms before all fixture smokes passed`
        : "one or more Khala Code fixture visual smokes failed"

  return {
    artifactDir,
    changedFiles,
    exitCode: gateExitCode(status, mode),
    hardFailDate: KHALA_VISUAL_SMOKE_GATE_HARD_FAIL_DATE,
    mode,
    reason,
    schema: KHALA_VISUAL_SMOKE_GATE_SCHEMA,
    status,
    steps: results,
    timeoutMs,
    visualFiles,
  }
}

if (import.meta.main) {
  const verdict = await runKhalaVisualSmokeGate()
  if (verdict.status === "skipped") {
    console.error(`[khala-visual-smoke-gate] SKIPPED: ${verdict.reason}.`)
  } else if (verdict.status === "passed") {
    console.error(
      `[khala-visual-smoke-gate] PASS: ${verdict.steps.length} fixture visual smokes passed in ${verdict.mode} mode.`,
    )
  } else if (verdict.mode === "warning-only") {
    console.error(
      `[khala-visual-smoke-gate] WARNING: ${verdict.reason}. Push remains allowed until hard-fail mode starts on ${verdict.hardFailDate}.`,
    )
  } else {
    console.error(`[khala-visual-smoke-gate] BLOCKED: ${verdict.reason}.`)
  }

  process.exit(verdict.exitCode)
}
