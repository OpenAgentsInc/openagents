import { spawnProcess } from "./proc.js"
import { listFleetAccounts, type KhalaFleetAccount, type KhalaFleetStatus } from "./fleet.js"
import { DEFAULT_BASE_URL } from "./types.js"

export const KHALA_FLEET_RUN_SCHEMA = "openagents.khala.fleet_run.v0.1"

export type KhalaFleetRunMode = "dry_run" | "once" | "supervise"

export type KhalaFleetRunPlan = {
  readonly schema: typeof KHALA_FLEET_RUN_SCHEMA
  readonly mode: KhalaFleetRunMode
  readonly baseUrl: string
  readonly branch: string
  readonly commit: string
  readonly issues: readonly number[]
  readonly maxSlots: number
  readonly perAccount: number
  readonly pylonRef: string | null
  readonly readyAccounts: readonly string[]
  readonly repo: string
  readonly targetSlots: number
  readonly verify: string
}

export type KhalaFleetRunResult = {
  readonly schema: typeof KHALA_FLEET_RUN_SCHEMA
  readonly mode: KhalaFleetRunMode
  readonly plan: KhalaFleetRunPlan
  readonly rounds: readonly KhalaFleetRunRound[]
  readonly status: "planned" | "completed"
}

export type KhalaFleetRunRound = {
  readonly accountRef: string
  readonly assignmentRef?: string | undefined
  readonly dedupeKey?: string | undefined
  readonly issue: number | null
  readonly ok: boolean
  readonly objective: string
  readonly slot: number
  readonly status: "dry_run" | "accepted" | "refused" | "failed"
  readonly workKind: "issue" | "replenishment"
}

export type KhalaFleetRunOptions = {
  readonly baseUrl?: string | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly dryRun?: boolean | undefined
  readonly env?: Record<string, string | undefined> | undefined
  readonly issues: readonly number[]
  readonly maxSlots?: number | undefined
  readonly once?: boolean | undefined
  readonly perAccount?: number | undefined
  readonly pylonCommand?: readonly string[] | undefined
  readonly pylonRef?: string | undefined
  readonly repo: string
  readonly status?: KhalaFleetStatus | undefined
  readonly token?: string | undefined
  readonly verify: string
}

const DEFAULT_PER_ACCOUNT = 1
const DEFAULT_MAX_SLOTS = 8
const SUPERVISOR_IDLE_MS = 2_000
const REFUSED_BACKOFF_MS = 15_000
const MAX_REFUSED_BACKOFF_MS = 120_000
const LOCKOUT_REPLENISH_AFTER_ROUNDS = 2

type FleetRunSlot = Omit<KhalaFleetRunRound, "ok" | "status" | "assignmentRef">

const REPLENISHMENT_OBJECTIVES = [
  {
    dedupeKey: "gepa-dspy-6707",
    issue: 6707,
    objective:
      "Advance the standing GEPA/DSPy continual-learning loop from public issue #6707 over recent public-safe traces. Keep the work bounded, avoid duplicate open PRs, and run the named verification.",
  },
  {
    dedupeKey: "bounded-codebase-audit",
    issue: null,
    objective:
      "Run a bounded codebase audit/review over apps/pylon, clients, and apps/openagents.com/workers/api using real source files. Fix one concrete issue if found, avoid duplicate open PRs, and run the named verification.",
  },
  {
    dedupeKey: "test-lint-typecheck-sweep",
    issue: null,
    objective:
      "Run a bounded test, lint, and typecheck sweep for the public checkout. Fix one concrete failure if found, avoid duplicate open PRs, and run the named verification.",
  },
] as const

export function parseFleetIssueList(raw: string): readonly number[] {
  const issues = raw
    .split(/[,\s]+/)
    .map(part => part.trim())
    .filter(part => part.length > 0)
    .map(part => {
      const normalized = part.startsWith("#") ? part.slice(1) : part
      const parsed = Number.parseInt(normalized, 10)
      if (!/^\d+$/.test(normalized) || !Number.isSafeInteger(parsed) || parsed <= 0) {
        throw new Error("khala fleet run --issues must contain public GitHub issue numbers")
      }
      return parsed
    })
  if (issues.length === 0) {
    throw new Error("khala fleet run requires --issues <numbers>")
  }
  return [...new Set(issues)]
}

export function validateFleetRunRepo(repo: string): string {
  const value = repo.trim()
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value)) {
    throw new Error("khala fleet run --repo must be owner/repo")
  }
  return value
}

export function validateFleetRunVerify(verify: string): string {
  const value = verify.trim()
  if (value.length < 3 || value.length > 500) {
    throw new Error("khala fleet run --verify must be 3-500 characters")
  }
  if (/(\.\.|~\/|\/Users\/|\/home\/|auth\.json|OPENAGENTS_AGENT_TOKEN|API_KEY|SECRET|TOKEN=)/i.test(value)) {
    throw new Error("khala fleet run --verify must be public-safe and must not include local paths or secrets")
  }
  return value
}

export function buildFleetRunPlan(input: {
  readonly baseUrl?: string | undefined
  readonly branch?: string | undefined
  readonly commit: string
  readonly issues: readonly number[]
  readonly maxSlots?: number | undefined
  readonly mode: KhalaFleetRunMode
  readonly perAccount?: number | undefined
  readonly pylonRef?: string | undefined
  readonly readyAccounts: readonly string[]
  readonly repo: string
  readonly verify: string
}): KhalaFleetRunPlan {
  const perAccount = positiveBounded(input.perAccount ?? DEFAULT_PER_ACCOUNT, "--per-account", 16)
  const maxSlots = positiveBounded(input.maxSlots ?? DEFAULT_MAX_SLOTS, "--max-slots", 64)
  const readyAccounts = input.readyAccounts.filter(ref => ref.trim().length > 0)
  const targetSlots = Math.min(maxSlots, readyAccounts.length * perAccount)
  return {
    schema: KHALA_FLEET_RUN_SCHEMA,
    baseUrl: input.baseUrl ?? DEFAULT_BASE_URL,
    branch: input.branch?.trim() || "main",
    commit: validateCommit(input.commit),
    issues: input.issues,
    maxSlots,
    mode: input.mode,
    perAccount,
    pylonRef: input.pylonRef?.trim() || null,
    readyAccounts,
    repo: validateFleetRunRepo(input.repo),
    targetSlots,
    verify: validateFleetRunVerify(input.verify),
  }
}

export async function runKhalaFleetSupervisor(options: KhalaFleetRunOptions): Promise<KhalaFleetRunResult> {
  const env = options.env ?? process.env
  const commandEnv = {
    ...env,
    PYLON_OPENAGENTS_BASE_URL: options.baseUrl ?? env.PYLON_OPENAGENTS_BASE_URL ?? DEFAULT_BASE_URL,
  }
  const status = options.status ?? await listFleetAccounts({ env: commandEnv })
  const readyAccounts = status.accounts
    .filter(account => account.readiness === "ready")
    .map(account => account.accountRef)
  const mode: KhalaFleetRunMode = options.dryRun ? "dry_run" : options.once ? "once" : "supervise"
  const pylonCommand = options.pylonCommand ?? pylonCommandFromEnv(env)
  const pylonRef = options.pylonRef?.trim() || (options.dryRun ? undefined : await resolvePylonRef({ env: commandEnv, pylonCommand }))
  const commit = options.commit?.trim() || (options.dryRun ? "0000000000000000000000000000000000000000" : await resolveGitHeadCommit())
  const plan = buildFleetRunPlan({
    baseUrl: options.baseUrl,
    branch: options.branch,
    commit,
    issues: options.issues,
    maxSlots: options.maxSlots,
    mode,
    perAccount: options.perAccount,
    pylonRef,
    readyAccounts,
    repo: options.repo,
    verify: options.verify,
  })

  if (plan.readyAccounts.length === 0) {
    throw new Error("No ready Codex accounts. Run `khala fleet connect`, then `khala fleet status`.")
  }
  if (plan.targetSlots === 0) {
    throw new Error("khala fleet run resolved zero target slots")
  }
  if (plan.pylonRef === null && !options.dryRun) {
    throw new Error("Could not resolve a local Pylon ref. Run `pylon provider go-online --json` or pass --pylon-ref.")
  }

  if (options.dryRun) {
    return {
      schema: KHALA_FLEET_RUN_SCHEMA,
      mode,
      plan,
      rounds: plannedRounds(plan).map(round => ({ ...round, ok: true, status: "dry_run" })),
      status: "planned",
    }
  }

  await runPylonCommand(pylonCommand, ["provider", "go-online", "--json"], envWithToken(commandEnv, options.token))
  await publishHeartbeat({ env: commandEnv, plan, pylonCommand, token: options.token })
  const rounds = options.once
    ? await runOneRound({ env: commandEnv, plan, pylonCommand, token: options.token })
    : await runSupervisorLoop({ env: commandEnv, plan, pylonCommand, token: options.token })
  return { schema: KHALA_FLEET_RUN_SCHEMA, mode, plan, rounds, status: "completed" }
}

function plannedRounds(plan: KhalaFleetRunPlan): ReadonlyArray<FleetRunSlot> {
  return Array.from({ length: plan.targetSlots }, (_, slot) => ({
    accountRef: plan.readyAccounts[slot % plan.readyAccounts.length] ?? "codex",
    issue: plan.issues[slot % plan.issues.length] ?? plan.issues[0]!,
    objective: `Implement public issue #${plan.issues[slot % plan.issues.length] ?? plan.issues[0]!} and run the named verification.`,
    slot,
    workKind: "issue",
  }))
}

export function plannedReplenishmentRounds(
  plan: KhalaFleetRunPlan,
  alreadyDispatchedKeys: ReadonlySet<string> = new Set(),
): ReadonlyArray<FleetRunSlot> {
  const available = REPLENISHMENT_OBJECTIVES.filter(task => !alreadyDispatchedKeys.has(task.dedupeKey))
  return available.slice(0, plan.targetSlots).map((task, slot) => ({
    accountRef: plan.readyAccounts[slot % plan.readyAccounts.length] ?? "codex",
    dedupeKey: task.dedupeKey,
    issue: task.issue,
    objective: task.objective,
    slot,
    workKind: "replenishment",
  }))
}

async function runSupervisorLoop(input: {
  readonly env: Record<string, string | undefined>
  readonly plan: KhalaFleetRunPlan
  readonly pylonCommand: readonly string[]
  readonly token?: string | undefined
}): Promise<readonly KhalaFleetRunRound[]> {
  const rounds: KhalaFleetRunRound[] = []
  let issueOffset = 0
  let refusedBackoffMs = REFUSED_BACKOFF_MS
  let consecutiveLockoutRounds = 0
  const dispatchedReplenishmentKeys = new Set<string>()
  for (;;) {
    await publishHeartbeat(input)
    const roundPlan = plannedRounds({
      ...input.plan,
      issues: rotateIssues(input.plan.issues, issueOffset),
    })
    const round = await Promise.all(roundPlan.map(slot => dispatchFleetSlot({ ...input, slot })))
    rounds.push(...round)
    issueOffset = (issueOffset + round.length) % input.plan.issues.length
    const lockout = round.length > 0 && round.every(item => item.status === "refused")
    consecutiveLockoutRounds = lockout ? consecutiveLockoutRounds + 1 : 0
    if (consecutiveLockoutRounds >= LOCKOUT_REPLENISH_AFTER_ROUNDS) {
      const replenishmentPlan = plannedReplenishmentRounds(input.plan, dispatchedReplenishmentKeys)
      for (const slot of replenishmentPlan) {
        if (slot.dedupeKey !== undefined) dispatchedReplenishmentKeys.add(slot.dedupeKey)
      }
      if (replenishmentPlan.length > 0) {
        const replenishmentRound = await Promise.all(replenishmentPlan.map(slot => dispatchFleetSlot({ ...input, slot })))
        rounds.push(...replenishmentRound)
        refusedBackoffMs = REFUSED_BACKOFF_MS
        consecutiveLockoutRounds = 0
        await delay(SUPERVISOR_IDLE_MS)
        continue
      }
    }
    if (round.some(item => item.status === "refused")) {
      await delay(refusedBackoffMs)
      refusedBackoffMs = Math.min(refusedBackoffMs * 2, MAX_REFUSED_BACKOFF_MS)
    } else {
      refusedBackoffMs = REFUSED_BACKOFF_MS
      await delay(SUPERVISOR_IDLE_MS)
    }
  }
}

async function runOneRound(input: {
  readonly env: Record<string, string | undefined>
  readonly plan: KhalaFleetRunPlan
  readonly pylonCommand: readonly string[]
  readonly token?: string | undefined
}): Promise<readonly KhalaFleetRunRound[]> {
  return await Promise.all(plannedRounds(input.plan).map(slot => dispatchFleetSlot({ ...input, slot })))
}

async function dispatchFleetSlot(input: {
  readonly env: Record<string, string | undefined>
  readonly plan: KhalaFleetRunPlan
  readonly pylonCommand: readonly string[]
  readonly slot: FleetRunSlot
  readonly token?: string | undefined
}): Promise<KhalaFleetRunRound> {
  const args = [
    "khala",
    "request",
    "--prompt",
    input.slot.objective,
    "--workflow",
    "codex_agent_task",
    "--pylon-ref",
    input.plan.pylonRef ?? "",
    "--account-ref",
    input.slot.accountRef,
    "--repo",
    input.plan.repo,
    "--branch",
    input.plan.branch,
    "--commit",
    input.plan.commit,
    "--verify",
    input.plan.verify,
    "--json",
  ]
  const result = await runPylonCommand(input.pylonCommand, args, envWithToken(input.env, input.token))
  const assignmentRef = readJsonStringField(result.stdout, "assignmentRef")
  if (result.exitCode === 0 && assignmentRef !== undefined) {
    return { ...input.slot, assignmentRef, ok: true, status: "accepted" }
  }
  const combined = `${result.stdout}\n${result.stderr}`
  if (/access token could not be refreshed|please sign in again|reauthenticate/i.test(combined)) {
    throw new Error("NEEDS-OWNER: Codex login needs reauthentication. `khala fleet run` will not run codex login or touch ~/.codex.")
  }
  if (/409|429|target_pylon_unavailable|rate.?limit|duplicate_active_assignment/i.test(combined)) {
    return { ...input.slot, ok: false, status: "refused" }
  }
  return { ...input.slot, ok: false, status: "failed" }
}

async function publishHeartbeat(input: {
  readonly env: Record<string, string | undefined>
  readonly plan: KhalaFleetRunPlan
  readonly pylonCommand: readonly string[]
  readonly token?: string | undefined
}): Promise<void> {
  await runPylonCommand(input.pylonCommand, ["presence", "heartbeat", "--json"], {
    ...envWithToken(input.env, input.token),
    OPENAGENTS_PYLON_CODEX_CONCURRENCY: String(input.plan.targetSlots),
    OPENAGENTS_PYLON_CODEX_BUSY: "0",
    OPENAGENTS_PYLON_CODEX_QUEUED: "0",
  })
}

async function resolvePylonRef(input: {
  readonly env: Record<string, string | undefined>
  readonly pylonCommand: readonly string[]
}): Promise<string> {
  const result = await runPylonCommand(input.pylonCommand, ["provider", "go-online", "--json"], input.env)
  const ref = readJsonStringField(result.stdout, "pylonRef")
  if (result.exitCode !== 0 || ref === undefined) {
    throw new Error("Could not auto-resolve Pylon ref from `pylon provider go-online --json`; pass --pylon-ref or check Pylon auth.")
  }
  return ref
}

async function resolveGitHeadCommit(): Promise<string> {
  const result = await runPylonCommand(["git"], ["rev-parse", "HEAD"], process.env)
  if (result.exitCode !== 0) {
    throw new Error("Could not resolve current git commit; pass --commit <40-char-sha>.")
  }
  return validateCommit(result.stdout.trim())
}

async function runPylonCommand(
  command: readonly string[],
  args: readonly string[],
  env: Record<string, string | undefined>,
): Promise<{ readonly exitCode: number; readonly stdout: string; readonly stderr: string }> {
  const child = spawnProcess([...command, ...args], {
    env: { ...process.env, ...env },
    stdout: "pipe",
    stderr: "pipe",
  })
  const [exitCode, stdout, stderr] = await Promise.all([child.exited, child.stdout, child.stderr])
  return { exitCode, stdout, stderr }
}

function pylonCommandFromEnv(env: Record<string, string | undefined>): readonly string[] {
  const raw = env.KHALA_PYLON_COMMAND?.trim()
  if (!raw) return ["pylon"]
  return raw.split(/\s+/).filter(part => part.length > 0)
}

function envWithToken(env: Record<string, string | undefined>, token: string | undefined): Record<string, string | undefined> {
  const clean = token?.trim()
  return clean ? { ...env, OPENAGENTS_AGENT_TOKEN: clean } : env
}

function rotateIssues(issues: readonly number[], offset: number): readonly number[] {
  if (issues.length === 0) return issues
  const normalized = offset % issues.length
  return [...issues.slice(normalized), ...issues.slice(0, normalized)]
}

function readJsonStringField(raw: string, field: string): string | undefined {
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>
    const value = parsed[field]
    return typeof value === "string" && value.trim().length > 0 ? value : undefined
  } catch {
    return undefined
  }
}

function validateCommit(commit: string): string {
  if (!/^[0-9a-f]{40}$/i.test(commit)) {
    throw new Error("khala fleet run --commit must be a pinned 40-character git SHA")
  }
  return commit
}

function positiveBounded(value: number, label: string, max: number): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`khala fleet run ${label} must be a positive integer`)
  }
  if (value > max) {
    throw new Error(`khala fleet run ${label} is capped at ${max}`)
  }
  return value
}

function delay(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

export function readyFleetAccountRefs(status: KhalaFleetStatus): readonly string[] {
  return status.accounts.filter(isReadyFleetAccount).map(account => account.accountRef)
}

function isReadyFleetAccount(account: KhalaFleetAccount): boolean {
  return account.readiness === "ready"
}
