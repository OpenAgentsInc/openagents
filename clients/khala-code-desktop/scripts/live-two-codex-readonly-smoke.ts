#!/usr/bin/env bun
import {
  runTwoCodexReadOnlySmoke,
  TWO_CODEX_READONLY_SMOKE_DEFAULT_TIMEOUT_MS,
  TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY,
  type TwoCodexReadOnlySmokeWork,
} from "../src/bun/khala-codex-live-smoke.js"

const liveAllowed =
  Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_SMOKE === "1" ||
  process.argv.includes("--allow-live")

if (!liveAllowed) {
  console.error([
    "Refusing to launch live Codex assignments without an explicit guard.",
    "Set KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_SMOKE=1 or pass --allow-live.",
  ].join("\n"))
  process.exit(2)
}

const startedAtMs = Date.now()
const timeoutMs = positiveInteger(
  Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_TIMEOUT_MS,
  TWO_CODEX_READONLY_SMOKE_DEFAULT_TIMEOUT_MS,
)
const maxAttempts = positiveInteger(
  Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_ATTEMPTS,
  3,
)
const fixture = process.argv.includes("--fixture")
const work = fixture
  ? { kind: "fixture" as const }
  : await repositoryWork()

console.error(`[live-smoke] launching two read-only Codex assignments in ${workLabel(work)}`)
console.error(`[live-smoke] timeout ${timeoutMs}ms`)
console.error(`[live-smoke] attempts ${maxAttempts}`)
if (work.kind === "repository") {
  console.error(`[live-smoke] read-only verification: ${work.verify ?? TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY}`)
}

const summary = await runWithTransientRetry(maxAttempts, async () =>
  runTwoCodexReadOnlySmoke({
    env: Bun.env,
    onProgress: payload => {
      const event = payload.events.at(-1)
      const label = [
        event?.assignmentRef ?? "assignment.pending",
        event?.event,
        event?.phase === undefined ? null : `phase=${event.phase}`,
        event?.status === undefined ? null : `status=${event.status}`,
      ].filter((value): value is string => value !== null && value !== undefined && value.length > 0)
        .join(" ")
      console.error(`[stream +${elapsedSeconds(startedAtMs)}s] ${label || "lifecycle"}`)
      for (const line of payload.lines.slice(-2)) {
        console.error(`  ${line}`)
      }
    },
    timeoutMs,
    work,
  })
)

console.log(JSON.stringify(summary, null, 2))

if (!summary.ok) {
  console.error(`[live-smoke] failed: ${summary.failures.join("; ")}`)
  for (const slotSummary of summary.slotSummaries) {
    console.error(`[live-smoke] ${slotSummary}`)
  }
  process.exit(1)
}

console.error(
  `[live-smoke] ok: accepted ${summary.acceptedCount}/${summary.requestedCount}, ` +
    `${summary.tokensVerified} verified tokens, ` +
    `${summary.progressEventCount} streamed lifecycle event(s)`,
)

async function repositoryWork(): Promise<TwoCodexReadOnlySmokeWork> {
  const repo = Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_REPO ?? "OpenAgentsInc/openagents"
  const branch = Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_BRANCH ?? "main"
  const commit = Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_COMMIT ?? await gitText(["rev-parse", "HEAD"])
  const verify = Bun.env.KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_VERIFY ??
    TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY
  return {
    branch,
    commit,
    kind: "repository",
    repo,
    verify,
  }
}

async function gitText(args: readonly string[]): Promise<string> {
  const proc = Bun.spawn(["git", ...args], {
    cwd: await gitRoot(),
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${stderr.trim() || `exit ${exitCode}`}`)
  }
  return stdout.trim()
}

async function gitRoot(): Promise<string> {
  const proc = Bun.spawn(["git", "rev-parse", "--show-toplevel"], {
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(`git rev-parse --show-toplevel failed: ${stderr.trim() || `exit ${exitCode}`}`)
  }
  return stdout.trim()
}

function positiveInteger(raw: string | undefined, fallback: number): number {
  if (raw === undefined || raw.trim().length === 0) return fallback
  const parsed = Number.parseInt(raw, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

function elapsedSeconds(startedAt: number): number {
  return Math.max(0, Math.round((Date.now() - startedAt) / 1000))
}

function workLabel(work: TwoCodexReadOnlySmokeWork): string {
  return work.kind === "fixture"
    ? "Pylon fixture mode"
    : `${work.repo}@${work.commit.slice(0, 12)} (${work.branch ?? "main"})`
}

async function runWithTransientRetry<T>(
  attempts: number,
  run: () => Promise<T>,
): Promise<T> {
  let lastError: unknown
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await run()
    } catch (error) {
      lastError = error
      if (attempt >= attempts || !isTransientSetupError(error)) break
      const delayMs = Math.min(10_000, 1_000 * attempt)
      console.error(
        `[live-smoke] transient setup failure on attempt ${attempt}/${attempts}: ${errorMessage(error)}`,
      )
      console.error(`[live-smoke] retrying in ${delayMs}ms`)
      await sleep(delayMs)
    }
  }
  throw lastError
}

function isTransientSetupError(error: unknown): boolean {
  return /500|overload|overloaded|queued for too long|D1_ERROR|capacity_probe_failed|presence request failed/iu
    .test(errorMessage(error))
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}
