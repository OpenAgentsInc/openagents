#!/usr/bin/env bun
/**
 * Concurrent local coding-session runner (#4869).
 *
 * Reads a JSON plan, materializes one isolated workspace per session, then
 * runs the retained dev-proof path in bounded parallel child processes.
 * Raw child output, local paths, prompts, and credential homes are never
 * retained in the run summary; per-session retained proof artifacts carry
 * the public objective and hashed account refs through dev-proof-run.
 *
 * Usage:
 *   bun apps/pylon/scripts/multi-session-run.ts \
 *     --plan plan.json --proofs-dir .pylon-proofs --pylon-home .pylon \
 *     [--concurrency 2] [--run-id run.local]
 */
import { createHash } from "node:crypto"
import { readFileSync } from "node:fs"
import { appendFile, mkdir, stat, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"
import {
  publicPylonAccountSelection,
  resolvePylonAccountSelection,
  type PublicPylonAccountSelection,
  type PylonAccountProvider,
  type ResolvedPylonAccountSelection,
} from "../src/account-registry"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import type { PylonComposerAdapter } from "../src/codex-agent"
import { classifySessionError } from "../src/session-error-class"
import { assertPublicProjectionSafe } from "../src/state"
import { scanProofSerialization } from "../src/proof-redaction"
import {
  materializeGitCheckoutWorkspaceWithLease,
  type GitCheckoutWorkspace,
} from "../src/workspace-materializer"
import { classifyQuotaSignal } from "../src/account-quota"
import {
  isAccountAvailable,
  loadQuotaRecord,
  recordQuotaBlock,
} from "../src/account-quota-ledger"

export const MULTI_SESSION_PLAN_SCHEMA = "openagents.pylon.multi_session_plan.v0.1"
export const MULTI_SESSION_HEARTBEAT_SCHEMA = "openagents.pylon.multi_session_heartbeat.v0.1"
export const MULTI_SESSION_SUMMARY_SCHEMA = "openagents.pylon.multi_session_summary.v0.1"
export const MULTI_SESSION_FAILURE_SCHEMA = "openagents.pylon.multi_session_failure.v0.1"

type MultiSessionRepositoryRef = GitCheckoutWorkspace["repository"]

export const MULTI_SESSION_ALL_ACCOUNTS_EXHAUSTED_DEVIATION =
  "deviation.pylon.multi_session.all_accounts_exhausted"

/**
 * One account selector in a session's ordered failover pool. Each member uses
 * the same selector vocabulary as a single-account session entry.
 */
export type MultiSessionAccountSelector = {
  accountRef?: string
  accountHome?: string
  codexHome?: string
  claudeConfigDir?: string
}

export type MultiSessionPlanEntry = {
  id?: string
  adapter: PylonComposerAdapter
  accountRef?: string
  accountHome?: string
  codexHome?: string
  claudeConfigDir?: string
  /**
   * Ordered failover pool (#4884). When present and non-empty, the runner tries
   * each available account in order; on a detected quota block it records the
   * block and advances to the next available account. When absent, the entry's
   * own single selector (accountRef/accountHome/codexHome/claudeConfigDir) is
   * used, preserving the prior single-account behavior.
   */
  accountPool?: MultiSessionAccountSelector[]
  repoRef?: MultiSessionRepositoryRef
  worktreePath?: string
  objective: string
  verify: string[]
  timeoutSeconds?: number
}

export type PylonRoutingReason = "succeeded" | "quota_block" | "skipped_unavailable" | "failed"

/** One account attempt within a session, recorded refs-only. */
export type PylonRoutingAttempt = {
  accountHash: string | null
  reason: PylonRoutingReason
  retryAtIso: string | null
}

export type MultiSessionArgs = {
  plan: MultiSessionPlanEntry[]
  proofsDir: string
  pylonHome: string
  concurrency: number
  runId: string
}

export type ProofChildInput = {
  adapter: PylonComposerAdapter
  account: ResolvedPylonAccountSelection | null
  accountHome: string | null
  accountRef: string | null
  cwd: string
  env: Record<string, string | undefined>
  issueRefs: string[]
  objective: string
  proofOutput: string
  timeoutSeconds: number
  verify: string[]
}

export type ProofChildResult = {
  exitCode: number
  stdout: string
  stderr: string
}

export type MultiSessionOutcome = {
  sessionIndex: number
  sessionRef: string
  adapter: PylonComposerAdapter
  account: PublicPylonAccountSelection | null
  workspaceRef: string
  state: "completed" | "failed"
  artifactFile: string | null
  resultRef: string | null
  errorClass: string | null
  errorDigestRef: string | null
  startedAt: string
  completedAt: string
  durationMs: number
  routingReason: PylonRoutingReason
  attempts: PylonRoutingAttempt[]
  retryAtIso: string | null
}

export type MultiSessionSummary = {
  schema: typeof MULTI_SESSION_SUMMARY_SCHEMA
  runRef: string
  runIdRef: string
  generatedAt: string
  concurrency: number
  totalSessions: number
  completedCount: number
  failedCount: number
  totalDurationMs: number
  totalTokens: number
  artifactRefs: string[]
  heartbeatRef: string
  outcomes: MultiSessionOutcome[]
  deviations: string[]
}

type WorkspaceSelection = {
  workspaceRef: string
  workingDirectory: string
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function nowIso() {
  return new Date().toISOString()
}

function earliestIso(a: string | null, b: string | null): string | null {
  if (a === null) return b
  if (b === null) return a
  return Date.parse(a) <= Date.parse(b) ? a : b
}

function boundedInt(value: string, min: number, max: number, usage: string): number {
  const parsed = Number(value)
  if (!Number.isFinite(parsed) || parsed < min || parsed > max) throw new Error(usage)
  return Math.floor(parsed)
}

function parseCliArgs(argv: string[]): MultiSessionArgs {
  const usage =
    "usage: multi-session-run.ts --plan <plan.json> --proofs-dir <dir> --pylon-home <dir> " +
    "[--concurrency <n>] [--run-id <id>]"
  let planPath: string | null = null
  let proofsDir: string | null = null
  let pylonHome: string | null = null
  let concurrency = 2
  let runId = `multi-session-${Date.now().toString(36)}`
  for (let index = 0; index < argv.length; index += 2) {
    const arg = argv[index]
    const value = argv[index + 1]
    if (typeof value !== "string") throw new Error(usage)
    if (arg === "--plan") planPath = resolve(value)
    else if (arg === "--proofs-dir") proofsDir = resolve(value)
    else if (arg === "--pylon-home") pylonHome = resolve(value)
    else if (arg === "--concurrency") concurrency = boundedInt(value, 1, 32, usage)
    else if (arg === "--run-id") runId = value
    else throw new Error(usage)
  }
  if (planPath === null || proofsDir === null || pylonHome === null) throw new Error(usage)
  return {
    plan: parsePlanJson(JSON.parse(readFileSyncText(planPath))),
    proofsDir,
    pylonHome,
    concurrency,
    runId,
  }
}

function readFileSyncText(path: string): string {
  return readFileSync(path, "utf8")
}

function providerForAdapter(adapter: PylonComposerAdapter): PylonAccountProvider {
  return adapter === "codex" ? "codex" : "claude_agent"
}

function safeId(value: string | undefined, fallback: string): string {
  if (!value) return fallback
  const normalized = value.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "")
  return normalized || fallback
}

function stringArray(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.length === 0) return null
  const entries = value.filter((entry): entry is string => typeof entry === "string" && entry.length > 0)
  return entries.length === value.length ? entries : null
}

function repositoryRefFrom(value: unknown): MultiSessionRepositoryRef | null {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (record.provider !== "github" || record.visibility !== "public") return null
  if (typeof record.fullName !== "string" || typeof record.branch !== "string") return null
  if (typeof record.commitSha !== "string" || !/^[a-f0-9]{40}$/i.test(record.commitSha)) return null
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(record.fullName)) return null
  return {
    provider: "github",
    visibility: "public",
    fullName: record.fullName,
    branch: record.branch,
    commitSha: record.commitSha,
  }
}

function accountPoolFrom(value: unknown, index: number): MultiSessionAccountSelector[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`multi-session plan entry ${index} accountPool must be a non-empty array`)
  }
  return value.map((member, memberIndex) => {
    if (member === null || typeof member !== "object") {
      throw new Error(`multi-session plan entry ${index} accountPool member ${memberIndex} is not an object`)
    }
    const record = member as Record<string, unknown>
    const selector: MultiSessionAccountSelector = {
      ...(typeof record.accountRef === "string" ? { accountRef: record.accountRef } : {}),
      ...(typeof record.accountHome === "string" ? { accountHome: record.accountHome } : {}),
      ...(typeof record.codexHome === "string" ? { codexHome: record.codexHome } : {}),
      ...(typeof record.claudeConfigDir === "string" ? { claudeConfigDir: record.claudeConfigDir } : {}),
    }
    return selector
  })
}

export function parsePlanJson(raw: unknown): MultiSessionPlanEntry[] {
  const entries = Array.isArray(raw)
    ? raw
    : raw !== null && typeof raw === "object" && Array.isArray((raw as { sessions?: unknown }).sessions)
      ? (raw as { sessions: unknown[] }).sessions
      : null
  if (entries === null) throw new Error("multi-session plan must be an array or { sessions }")
  return entries.map((entry, index) => {
    if (entry === null || typeof entry !== "object") {
      throw new Error(`multi-session plan entry ${index} is not an object`)
    }
    const record = entry as Record<string, unknown>
    const adapter = record.adapter
    if (adapter !== "codex" && adapter !== "claude_agent") {
      throw new Error(`multi-session plan entry ${index} has invalid adapter`)
    }
    if (typeof record.objective !== "string" || record.objective.trim().length === 0) {
      throw new Error(`multi-session plan entry ${index} has invalid objective`)
    }
    const verify = stringArray(record.verify)
    if (verify === null) throw new Error(`multi-session plan entry ${index} has invalid verify argv`)
    const repoRef = record.repoRef === undefined ? undefined : repositoryRefFrom(record.repoRef)
    if (record.repoRef !== undefined && repoRef === null) {
      throw new Error(`multi-session plan entry ${index} has invalid repoRef`)
    }
    const worktreePath = typeof record.worktreePath === "string" ? record.worktreePath : undefined
    if (repoRef === undefined && worktreePath === undefined) {
      throw new Error(`multi-session plan entry ${index} needs repoRef or worktreePath`)
    }
    if (repoRef !== undefined && worktreePath !== undefined) {
      throw new Error(`multi-session plan entry ${index} must use only one workspace selector`)
    }
    const accountPool =
      record.accountPool === undefined ? undefined : accountPoolFrom(record.accountPool, index)
    return {
      adapter,
      ...(typeof record.id === "string" ? { id: record.id } : {}),
      ...(typeof record.accountRef === "string" ? { accountRef: record.accountRef } : {}),
      ...(typeof record.accountHome === "string" ? { accountHome: record.accountHome } : {}),
      ...(typeof record.codexHome === "string" ? { codexHome: record.codexHome } : {}),
      ...(typeof record.claudeConfigDir === "string" ? { claudeConfigDir: record.claudeConfigDir } : {}),
      ...(accountPool === undefined ? {} : { accountPool }),
      ...(repoRef === undefined ? {} : { repoRef }),
      ...(worktreePath === undefined ? {} : { worktreePath }),
      objective: record.objective,
      verify,
      ...(typeof record.timeoutSeconds === "number" && Number.isFinite(record.timeoutSeconds)
        ? { timeoutSeconds: Math.max(1, Math.min(1200, Math.floor(record.timeoutSeconds))) }
        : {}),
    }
  })
}

async function workspaceForSession(input: {
  args: MultiSessionArgs
  entry: MultiSessionPlanEntry
  index: number
}): Promise<WorkspaceSelection> {
  if (input.entry.worktreePath !== undefined) {
    const workingDirectory = resolve(input.entry.worktreePath)
    try {
      const info = await stat(workingDirectory)
      if (!info.isDirectory()) throw new Error("not a directory")
    } catch {
      throw new Error("worktree_path_missing")
    }
    return {
      workingDirectory,
      workspaceRef: stableRef("workspace.pylon.multi_session.injected", workingDirectory),
    }
  }

  const repoRef = input.entry.repoRef
  if (repoRef === undefined) throw new Error("workspace_selector_missing")
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
    ...Bun.env,
    PYLON_HOME: input.args.pylonHome,
  })
  const checkout: GitCheckoutWorkspace = {
    kind: "git_checkout",
    repository: repoRef,
    verificationCommand: {
      args: input.entry.verify,
      commandRef: stableRef("command.pylon.multi_session.verify", input.entry.verify.join("\0")),
    },
  }
  const leaseRef = stableRef(
    "lease.pylon.multi_session.workspace",
    `${input.args.runId}:${input.index}:${repoRef.fullName}:${repoRef.commitSha}`,
  )
  const materialized = await materializeGitCheckoutWorkspaceWithLease({
    cacheRoot: join(summary.paths.cache, "multi-session-worktrees"),
    checkout,
    leaseRef,
    refPrefix: "workspace.pylon.multi_session",
    repositoryCacheRoot: join(summary.paths.cache, "workspace-git-cache"),
    workspaceStateRoot: join(summary.paths.cache, "workspace-leases"),
  })
  return {
    workingDirectory: materialized.workingDirectory,
    workspaceRef: materialized.workspaceRef,
  }
}

async function defaultProofChildRunner(input: ProofChildInput): Promise<ProofChildResult> {
  const script = resolve(dirname(fileURLToPath(import.meta.url)), "dev-proof-run.ts")
  const args = [
    "bun",
    script,
    "--adapter",
    input.adapter,
    "--objective",
    input.objective,
    "--proof-output",
    input.proofOutput,
    "--timeout-seconds",
    String(input.timeoutSeconds),
    ...input.issueRefs.flatMap(ref => ["--issue", ref]),
    ...(input.accountRef === null ? [] : ["--account-ref", input.accountRef]),
    ...(input.adapter === "codex" && input.accountHome !== null ? ["--codex-home", input.accountHome] : []),
    ...(input.adapter === "claude_agent" && input.accountHome !== null
      ? ["--claude-config-dir", input.accountHome]
      : []),
    "--",
    ...input.verify,
  ]
  const proc = Bun.spawn(args, {
    cwd: input.cwd,
    env: input.env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  return { exitCode, stdout, stderr }
}

async function appendHeartbeat(path: string, payload: unknown) {
  const serialized = JSON.stringify(payload)
  const violations = scanProofSerialization(serialized)
  if (violations.length > 0) return
  await appendFile(path, `${serialized}\n`, "utf8")
}

async function writeFailure(path: string, payload: unknown) {
  const serialized = JSON.stringify(payload, null, 2)
  const violations = scanProofSerialization(serialized)
  if (violations.length > 0) {
    await writeFile(
      path,
      `${JSON.stringify(
        {
          schema: MULTI_SESSION_FAILURE_SCHEMA,
          state: "quarantined",
          generatedAt: nowIso(),
          violationRefs: violations,
          artifactDigestRef: stableRef("digest.pylon.multi_session.quarantine", serialized),
        },
        null,
        2,
      )}\n`,
    )
    return
  }
  await writeFile(path, `${serialized}\n`)
}

async function runOneSession(input: {
  args: MultiSessionArgs
  entry: MultiSessionPlanEntry
  index: number
  heartbeatPath: string
  proofRunner: (child: ProofChildInput) => Promise<ProofChildResult>
}): Promise<MultiSessionOutcome> {
  const startedAt = nowIso()
  const runRef = stableRef("run.pylon.multi_session", input.args.runId)
  const sessionRef = stableRef(
    "session.pylon.multi_session",
    `${input.args.runId}:${input.index}:${input.entry.adapter}:${input.entry.objective}`,
  )
  const adapter = input.entry.adapter
  const provider = providerForAdapter(adapter)
  const artifactFile = `${safeId(input.entry.id, `session-${input.index}`)}-${adapter}-proof.json`
  const failureFile = `${safeId(input.entry.id, `session-${input.index}`)}-failure.json`
  const attempts: PylonRoutingAttempt[] = []

  const emitAttempt = (reason: PylonRoutingReason, accountHash: string | null, retryAtIso: string | null) =>
    appendHeartbeat(input.heartbeatPath, {
      schema: MULTI_SESSION_HEARTBEAT_SCHEMA,
      runRef,
      sessionRef,
      observedAt: nowIso(),
      phase: "attempt",
      sessionIndex: input.index,
      routingReason: reason,
      accountHash,
      retryAtIso,
    })

  await appendHeartbeat(input.heartbeatPath, {
    schema: MULTI_SESSION_HEARTBEAT_SCHEMA,
    runRef,
    sessionRef,
    observedAt: startedAt,
    phase: "started",
    sessionIndex: input.index,
  })

  // The ordered pool is the explicit accountPool when present; otherwise a
  // single member derived from the entry's own selector (prior behavior).
  const pool: MultiSessionAccountSelector[] =
    input.entry.accountPool && input.entry.accountPool.length > 0
      ? input.entry.accountPool
      : [
          {
            ...(input.entry.accountRef === undefined ? {} : { accountRef: input.entry.accountRef }),
            ...(input.entry.accountHome === undefined ? {} : { accountHome: input.entry.accountHome }),
            ...(input.entry.codexHome === undefined ? {} : { codexHome: input.entry.codexHome }),
            ...(input.entry.claudeConfigDir === undefined
              ? {}
              : { claudeConfigDir: input.entry.claudeConfigDir }),
          },
        ]

  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
      ...Bun.env,
      PYLON_HOME: input.args.pylonHome,
    })
    const workspace = await workspaceForSession(input)
    const proofOutput = join(input.args.proofsDir, artifactFile)

    let earliestRetryAtIso: string | null = null
    let lastAccount: ResolvedPylonAccountSelection | null = null
    let lastFailure: { errorClass: string; errorDigestRef: string } | null = null

    for (const member of pool) {
      const accountHome =
        member.accountHome ??
        (adapter === "codex" ? member.codexHome : member.claudeConfigDir) ??
        undefined
      const account = await resolvePylonAccountSelection(summary, {
        provider,
        ...(member.accountRef === undefined ? {} : { accountRef: member.accountRef }),
        ...(accountHome === undefined ? {} : { accountHome }),
      })
      lastAccount = account
      const accountHash = account?.accountRefHash ?? null

      // Skip an account the ledger still marks unavailable (cooldown / known limit).
      if (accountHash !== null) {
        const record = await loadQuotaRecord(summary, accountHash)
        if (!isAccountAvailable(record, new Date())) {
          const retryAtIso = record?.retryAtIso ?? null
          earliestRetryAtIso = earliestIso(earliestRetryAtIso, retryAtIso)
          attempts.push({ accountHash, reason: "skipped_unavailable", retryAtIso })
          await emitAttempt("skipped_unavailable", accountHash, retryAtIso)
          continue
        }
      }

      const child = await input.proofRunner({
        adapter,
        account,
        accountHome: account?.accountRef === null ? account.home : null,
        accountRef: account?.accountRef ?? null,
        cwd: workspace.workingDirectory,
        env: { ...Bun.env, PYLON_HOME: input.args.pylonHome },
        issueRefs: ["OpenAgentsInc/openagents#4884"],
        objective: input.entry.objective,
        proofOutput,
        timeoutSeconds: input.entry.timeoutSeconds ?? 600,
        verify: input.entry.verify,
      })

      if (child.exitCode === 0) {
        attempts.push({ accountHash, reason: "succeeded", retryAtIso: null })
        await emitAttempt("succeeded", accountHash, null)
        const completedAt = nowIso()
        return {
          sessionIndex: input.index,
          sessionRef,
          adapter,
          account: publicPylonAccountSelection(account),
          workspaceRef: workspace.workspaceRef,
          state: "completed",
          artifactFile,
          resultRef: stableRef("proof.pylon.multi_session", `${sessionRef}:${artifactFile}`),
          errorClass: null,
          errorDigestRef: null,
          startedAt,
          completedAt,
          durationMs: new Date(completedAt).getTime() - new Date(startedAt).getTime(),
          routingReason: "succeeded",
          attempts,
          retryAtIso: null,
        }
      }

      // A quota block records the reset and routes on to the next account; any
      // other failure is terminal so we do not burn the rest of the pool.
      const combined = `${child.stderr}\n${child.stdout}`
      const quota = classifyQuotaSignal(combined, adapter)
      if (quota.exhausted) {
        if (accountHash !== null) {
          await recordQuotaBlock(summary, {
            accountRefHash: accountHash,
            provider,
            retryAtIso: quota.retryAtIso,
            sourceDigestRef: quota.sourceDigestRef,
            now: new Date(),
          })
        }
        earliestRetryAtIso = earliestIso(earliestRetryAtIso, quota.retryAtIso)
        attempts.push({ accountHash, reason: "quota_block", retryAtIso: quota.retryAtIso })
        await emitAttempt("quota_block", accountHash, quota.retryAtIso)
        lastFailure = { errorClass: "account_quota_exhausted", errorDigestRef: quota.sourceDigestRef }
        continue
      }

      const failure = classifySessionError(combined)
      attempts.push({ accountHash, reason: "failed", retryAtIso: null })
      await emitAttempt("failed", accountHash, null)
      const completedAt = nowIso()
      await writeFailure(join(input.args.proofsDir, failureFile), {
        schema: MULTI_SESSION_FAILURE_SCHEMA,
        sessionRef,
        sessionIndex: input.index,
        adapter,
        account: publicPylonAccountSelection(account),
        workspaceRef: workspace.workspaceRef,
        generatedAt: completedAt,
        errorClass: failure.errorClass,
        errorDigestRef: failure.errorDigestRef,
        routingReason: "failed",
        attempts,
      })
      return {
        sessionIndex: input.index,
        sessionRef,
        adapter,
        account: publicPylonAccountSelection(account),
        workspaceRef: workspace.workspaceRef,
        state: "failed",
        artifactFile: failureFile,
        resultRef: null,
        errorClass: failure.errorClass,
        errorDigestRef: failure.errorDigestRef,
        startedAt,
        completedAt,
        durationMs: new Date(completedAt).getTime() - new Date(startedAt).getTime(),
        routingReason: "failed",
        attempts,
        retryAtIso: null,
      }
    }

    // Pool exhausted with no success: every member was quota-blocked or skipped.
    const completedAt = nowIso()
    const failure = lastFailure ?? classifySessionError("all accounts exhausted")
    await writeFailure(join(input.args.proofsDir, failureFile), {
      schema: MULTI_SESSION_FAILURE_SCHEMA,
      sessionRef,
      sessionIndex: input.index,
      adapter,
      account: publicPylonAccountSelection(lastAccount),
      workspaceRef: workspace.workspaceRef,
      generatedAt: completedAt,
      errorClass: failure.errorClass,
      errorDigestRef: failure.errorDigestRef,
      routingReason: "quota_block",
      attempts,
      retryAtIso: earliestRetryAtIso,
    })
    return {
      sessionIndex: input.index,
      sessionRef,
      adapter,
      account: publicPylonAccountSelection(lastAccount),
      workspaceRef: workspace.workspaceRef,
      state: "failed",
      artifactFile: failureFile,
      resultRef: null,
      errorClass: failure.errorClass,
      errorDigestRef: failure.errorDigestRef,
      startedAt,
      completedAt,
      durationMs: new Date(completedAt).getTime() - new Date(startedAt).getTime(),
      routingReason: "quota_block",
      attempts,
      retryAtIso: earliestRetryAtIso,
    }
  } catch (error) {
    const completedAt = nowIso()
    const failure = classifySessionError(error)
    await writeFailure(join(input.args.proofsDir, failureFile), {
      schema: MULTI_SESSION_FAILURE_SCHEMA,
      sessionRef,
      sessionIndex: input.index,
      adapter,
      generatedAt: completedAt,
      errorClass: failure.errorClass,
      errorDigestRef: failure.errorDigestRef,
      routingReason: "failed",
      attempts,
    })
    return {
      sessionIndex: input.index,
      sessionRef,
      adapter,
      account: null,
      workspaceRef: stableRef("workspace.pylon.multi_session.failed", sessionRef),
      state: "failed",
      artifactFile: failureFile,
      resultRef: null,
      errorClass: failure.errorClass,
      errorDigestRef: failure.errorDigestRef,
      startedAt,
      completedAt,
      durationMs: new Date(completedAt).getTime() - new Date(startedAt).getTime(),
      routingReason: "failed",
      attempts,
      retryAtIso: null,
    }
  } finally {
    await appendHeartbeat(input.heartbeatPath, {
      schema: MULTI_SESSION_HEARTBEAT_SCHEMA,
      runRef,
      sessionRef,
      observedAt: nowIso(),
      phase: "completed",
      sessionIndex: input.index,
    })
  }
}

async function runBounded<T>(
  total: number,
  concurrency: number,
  worker: (index: number) => Promise<T>,
): Promise<T[]> {
  const results: T[] = new Array(total)
  let cursor = 0
  const runners = Array.from({ length: Math.min(concurrency, total) }, async () => {
    while (true) {
      const index = cursor
      cursor += 1
      if (index >= total) return
      results[index] = await worker(index)
    }
  })
  await Promise.all(runners)
  return results
}

export async function runMultiSessionPlan(
  args: MultiSessionArgs,
  options: {
    proofRunner?: (child: ProofChildInput) => Promise<ProofChildResult>
  } = {},
): Promise<MultiSessionSummary> {
  await mkdir(args.proofsDir, { recursive: true })
  const runRef = stableRef("run.pylon.multi_session", args.runId)
  const heartbeatPath = join(args.proofsDir, "heartbeats.jsonl")
  const proofRunner = options.proofRunner ?? defaultProofChildRunner
  await appendHeartbeat(heartbeatPath, {
    schema: MULTI_SESSION_HEARTBEAT_SCHEMA,
    runRef,
    observedAt: nowIso(),
    phase: "run_started",
    sessionIndex: null,
  })
  const outcomes = await runBounded(args.plan.length, args.concurrency, index =>
    runOneSession({
      args,
      entry: args.plan[index]!,
      heartbeatPath,
      index,
      proofRunner,
    }),
  )
  await appendHeartbeat(heartbeatPath, {
    schema: MULTI_SESSION_HEARTBEAT_SCHEMA,
    runRef,
    observedAt: nowIso(),
    phase: "run_completed",
    sessionIndex: null,
  })
  const completedCount = outcomes.filter(outcome => outcome.state === "completed").length
  const failedCount = outcomes.length - completedCount
  const totalDurationMs = outcomes.reduce((sum, outcome) => sum + outcome.durationMs, 0)
  let totalTokens = 0
  for (const outcome of outcomes) {
    if (outcome.state !== "completed" || outcome.artifactFile === null) continue
    try {
      const proof = JSON.parse(readFileSyncText(join(args.proofsDir, outcome.artifactFile)))
      const tokens = proof?.executor?.totalTokens
      if (typeof tokens === "number" && Number.isFinite(tokens)) totalTokens += tokens
    } catch {
      // Missing or unparseable artifact contributes 0 tokens.
    }
  }
  const summary: MultiSessionSummary = {
    schema: MULTI_SESSION_SUMMARY_SCHEMA,
    runRef,
    runIdRef: stableRef("ref.pylon.multi_session.run_id", args.runId),
    generatedAt: nowIso(),
    concurrency: args.concurrency,
    totalSessions: outcomes.length,
    completedCount,
    failedCount,
    totalDurationMs,
    totalTokens,
    artifactRefs: outcomes
      .map(outcome => outcome.artifactFile)
      .filter((file): file is string => file !== null)
      .map(file => stableRef("artifact.pylon.multi_session.file", file)),
    heartbeatRef: stableRef("artifact.pylon.multi_session.heartbeats", `${runRef}:heartbeats.jsonl`),
    outcomes,
    deviations: [
      ...(failedCount === 0 ? [] : ["deviation.pylon.multi_session.some_sessions_failed"]),
      ...(outcomes.some(outcome => outcome.state === "failed" && outcome.routingReason === "quota_block")
        ? [MULTI_SESSION_ALL_ACCOUNTS_EXHAUSTED_DEVIATION]
        : []),
    ],
  }
  assertPublicProjectionSafe(summary)
  const serialized = JSON.stringify(summary, null, 2)
  const violations = scanProofSerialization(serialized)
  if (violations.length > 0) {
    throw new Error(`multi-session summary failed redaction scan: ${violations.join(", ")}`)
  }
  await writeFile(join(args.proofsDir, "multi-session-summary.json"), `${serialized}\n`)
  return summary
}

async function main() {
  const args = parseCliArgs(Bun.argv.slice(2))
  const summary = await runMultiSessionPlan(args)
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`)
  if (summary.failedCount > 0) process.exitCode = 1
}

if (import.meta.main) {
  await main()
}
