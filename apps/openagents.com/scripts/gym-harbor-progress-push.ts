#!/usr/bin/env bun

/**
 * Push live Gym / Harbor run progress to the Worker ingest endpoint (#6271).
 *
 * The Cloudflare Worker cannot read a Harbor job's local `result.json`, so this
 * pusher runs ALONGSIDE the job (locally or on Hydralisk), reads `result.json`
 * on a poll cadence, projects a PUBLIC-SAFE `openagents.gym.run_progress.v1`
 * snapshot (counts / official denominator / pass-rate-over-completed / token
 * counts / public-safe profile refs / freshness — NEVER prompts, responses,
 * logs, trajectories, keys, or private endpoints), and POSTs it to
 * `POST /api/operator/gym/run-progress` with the operator bearer. The Worker
 * RE-VALIDATES and re-asserts public-safety before storing, so this script is
 * the producer of the snapshot, not the trusted authority for its safety.
 *
 * Redaction discipline: this script only ever projects COUNTS and the typed run
 * context (runRef/jobRef/configId/profileRef/agent/phase) into the snapshot. It
 * does not copy any per-trial prompt, command, response, log, or trajectory
 * field out of `result.json`. Token COUNTS are summed when present and omitted
 * (null = not_measured) otherwise. The operator bearer is read from the
 * environment and NEVER printed.
 *
 * Usage:
 *   bun scripts/gym-harbor-progress-push.ts \
 *     --result <path/to/result.json> \
 *     --run-ref run.gym.terminal_bench.<id> \
 *     --job-ref job.gym.harbor_terminal_bench.<id> \
 *     --config-id gym.terminal_bench.<id> \
 *     --profile-ref khala-public-heuristic \
 *     --agent opencode \
 *     [--official-denominator 89] \
 *     [--publication local_only|web_authorized] \
 *     [--phase queued|running|completed|cancelled|errored] \
 *     [--poll-seconds 15] \
 *     [--base-url https://openagents.com] \
 *     [--once] [--dry-run] [--json]
 *
 * Environment:
 *   OPENAGENTS_BASE_URL          Optional base URL (default https://openagents.com).
 *   OPENAGENTS_ADMIN_API_TOKEN   Operator bearer. Required unless --dry-run.
 *   GYM_HARBOR_RESULT            Optional default for --result.
 *
 * Most flags also accept an env fallback (GYM_HARBOR_RUN_REF, GYM_HARBOR_JOB_REF,
 * GYM_HARBOR_CONFIG_ID, GYM_HARBOR_PROFILE_REF, GYM_HARBOR_AGENT,
 * GYM_HARBOR_OFFICIAL_DENOMINATOR, GYM_HARBOR_PUBLICATION).
 */
import { readFile } from 'node:fs/promises'

export const DEFAULT_BASE_URL = 'https://openagents.com'
export const INGEST_PATH = '/api/operator/gym/run-progress'
export const DEFAULT_OFFICIAL_DENOMINATOR = 89
export const DEFAULT_POLL_SECONDS = 15

type Phase = 'queued' | 'running' | 'completed' | 'cancelled' | 'errored'

// The public-safe ingest snapshot shape (mirrors GymRunProgressInput). Counts +
// typed context only — no raw benchmark content, ever.
export type GymRunProgressSnapshot = Readonly<{
  runRef: string
  jobRef: string
  configId: string
  profileRef: string
  agent: string
  phase: Phase
  publication: 'local_only' | 'web_authorized'
  officialDenominator: number
  completedPassed: number
  completedFailed: number
  running: number
  pending: number
  error: number
  cancelled: number
  promptTokens: number | null
  completionTokens: number | null
  elapsedMs: number | null
  lastUpdatedAt: string
  caveatRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

export type SnapshotContext = Readonly<{
  runRef: string
  jobRef: string
  configId: string
  profileRef: string
  agent: string
  officialDenominator: number
  publication: 'local_only' | 'web_authorized'
  phase?: Phase
  nowIso?: () => string
}>

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const numberOrNull = (value: unknown): number | null =>
  typeof value === 'number' && Number.isFinite(value) ? value : null

// A non-negative integer count, or 0 when absent/invalid (counts are never null).
const countOrZero = (value: unknown): number => {
  const parsed = numberOrNull(value)
  return parsed !== null && parsed >= 0 ? Math.floor(parsed) : 0
}

// Pull an array of per-trial entries from a Harbor `result.json`. Harbor has
// emitted the trials under a few keys across versions; we accept the common
// shapes and otherwise treat the top-level value as the trial array.
const trialArray = (result: unknown): ReadonlyArray<Record<string, unknown>> => {
  if (Array.isArray(result)) {
    return result.filter(isRecord)
  }
  if (isRecord(result)) {
    for (const key of ['trials', 'results', 'tasks']) {
      const candidate = result[key]
      if (Array.isArray(candidate)) {
        return candidate.filter(isRecord)
      }
    }
  }
  return []
}

// A trial's pass/fail verdict from `verifier_result.rewards` (or common
// fallbacks). A reward strictly > 0 (or an explicit boolean pass / "resolved")
// counts as passed. Returns undefined when the trial has no verdict yet
// (still running / pending) so it is NOT counted as completed.
const trialVerdict = (trial: Record<string, unknown>): boolean | undefined => {
  const verifier = trial['verifier_result']
  if (isRecord(verifier)) {
    const rewards = verifier['rewards']
    if (Array.isArray(rewards) && rewards.length > 0) {
      const numeric = rewards.filter(
        (value): value is number =>
          typeof value === 'number' && Number.isFinite(value),
      )
      if (numeric.length > 0) {
        return numeric.reduce((sum, value) => sum + value, 0) / numeric.length > 0
      }
    }
    const reward = numberOrNull(verifier['reward'])
    if (reward !== null) {
      return reward > 0
    }
  }
  const directReward = numberOrNull(trial['reward'])
  if (directReward !== null) {
    return directReward > 0
  }
  const resolved = trial['is_resolved'] ?? trial['resolved'] ?? trial['passed']
  if (typeof resolved === 'boolean') {
    return resolved
  }
  return undefined
}

const trialStatus = (trial: Record<string, unknown>): string | undefined => {
  const status = trial['status'] ?? trial['state']
  return typeof status === 'string' ? status.toLowerCase() : undefined
}

const sumTokens = (
  trials: ReadonlyArray<Record<string, unknown>>,
  key: 'prompt_tokens' | 'completion_tokens',
): number | null => {
  const values = trials.flatMap(trial => {
    const metrics = trial['metrics'] ?? trial['usage'] ?? trial
    if (!isRecord(metrics)) {
      return []
    }
    const value = numberOrNull(metrics[key])
    return value === null ? [] : [value]
  })
  return values.length === 0
    ? null
    : values.reduce((sum, value) => sum + value, 0)
}

const elapsedMsFromTrials = (
  trials: ReadonlyArray<Record<string, unknown>>,
): number | null => {
  const values = trials.flatMap(trial => {
    const metrics = trial['metrics'] ?? trial
    if (!isRecord(metrics)) {
      return []
    }
    const value =
      numberOrNull(metrics['elapsed_ms']) ??
      numberOrNull(metrics['duration_ms'])
    return value === null ? [] : [value]
  })
  return values.length === 0
    ? null
    : values.reduce((sum, value) => sum + value, 0)
}

// Derive the phase from OBSERVED trial activity (not the synthetic
// remaining-denominator pending): nothing observed yet => queued; anything still
// in flight (observed-running or denominator not yet fully accounted) => running;
// every official task accounted with none running => completed.
const derivePhase = (
  observed: Readonly<{
    completedPassed: number
    completedFailed: number
    running: number
    pending: number
    error: number
    cancelled: number
  }>,
  officialDenominator: number,
  explicit?: Phase,
): Phase => {
  if (explicit !== undefined) {
    return explicit
  }
  const observedCount =
    observed.completedPassed +
    observed.completedFailed +
    observed.running +
    observed.pending +
    observed.error +
    observed.cancelled
  if (observedCount === 0) {
    return 'queued'
  }
  const terminalCount =
    observed.completedPassed +
    observed.completedFailed +
    observed.error +
    observed.cancelled
  if (observed.running > 0 || terminalCount < officialDenominator) {
    return 'running'
  }
  return 'completed'
}

// ---------------------------------------------------------------------------
// Real Terminal-Bench 2.0 Harbor `result.json` shape: a `.stats` SUMMARY with
// NO per-trial array.
// ---------------------------------------------------------------------------
//
// The live TB2.0 / Harbor result carries `n_total_trials` plus a `.stats`
// object with `n_completed_trials`/`n_running_trials`/`n_pending_trials`/
// `n_errored_trials`/`n_cancelled_trials`, summed token counts, and a per-eval
// pass/fail breakdown under `.stats.evals[<evalKey>].reward_stats.reward`. That
// reward object is keyed by reward VALUE (e.g. "1.0", "0.0") with arrays of task
// ids as values. We count ONLY the list LENGTHS (passed = ids under reward > 0,
// failed = ids under reward <= 0), never the ids themselves, summed across all
// eval keys. In TB2.0 an errored trial still counts as a completed reward-0.0
// failure (Harbor's own `metrics[0].mean` = passed / n_completed confirms this),
// so `n_errored_trials` is reported as a SUBSET of completed, not an additive
// disjoint bucket.

// Sum, across all eval keys, the count of task ids whose reward key passes the
// predicate. PURE: only list lengths leave this function — never the ids.
const sumRewardListLengths = (
  evals: Record<string, unknown>,
  rewardPredicate: (reward: number) => boolean,
): number =>
  Object.values(evals).reduce((total, evalValue) => {
    if (!isRecord(evalValue)) {
      return total
    }
    const rewardStats = evalValue['reward_stats']
    if (!isRecord(rewardStats)) {
      return total
    }
    const reward = rewardStats['reward']
    if (!isRecord(reward)) {
      return total
    }
    const matched = Object.entries(reward).reduce((sum, [key, ids]) => {
      const rewardValue = Number(key)
      if (!Number.isFinite(rewardValue) || !rewardPredicate(rewardValue)) {
        return sum
      }
      return sum + (Array.isArray(ids) ? ids.length : 0)
    }, 0)
    return total + matched
  }, 0)

// Project the real TB2.0 `.stats` summary into a public-safe snapshot. Returns
// undefined when the result is not the stats shape so the caller falls back to
// the per-trial-array parser. PURE: counts / denominator / tokens only.
const projectStatsToSnapshot = (
  result: Record<string, unknown>,
  context: SnapshotContext,
  now: string,
): GymRunProgressSnapshot | undefined => {
  const stats = result['stats']
  if (!isRecord(stats)) {
    return undefined
  }

  const evals = isRecord(stats['evals']) ? stats['evals'] : {}
  const completedPassed = sumRewardListLengths(evals, reward => reward > 0)
  const completedFailed = sumRewardListLengths(evals, reward => reward <= 0)
  const running = countOrZero(stats['n_running_trials'])
  const pending = countOrZero(stats['n_pending_trials'])
  const error = countOrZero(stats['n_errored_trials'])
  const cancelled = countOrZero(stats['n_cancelled_trials'])

  // Prefer the official total from the file; fall back to the configured one.
  const officialDenominator =
    countOrZero(result['n_total_trials']) || context.officialDenominator

  const observed = {
    completedPassed,
    completedFailed,
    running,
    pending,
    error,
    cancelled,
  }

  // `finished_at` is the authoritative completion marker for the whole job.
  const finished =
    typeof result['finished_at'] === 'string' &&
    result['finished_at'].trim() !== ''
  const phase: Phase =
    context.phase ??
    (finished
      ? 'completed'
      : derivePhase(observed, officialDenominator, undefined))

  return {
    runRef: context.runRef,
    jobRef: context.jobRef,
    configId: context.configId,
    profileRef: context.profileRef,
    agent: context.agent,
    publication: context.publication,
    officialDenominator,
    completedPassed,
    completedFailed,
    running,
    pending,
    error,
    cancelled,
    promptTokens: numberOrNull(stats['n_input_tokens']),
    completionTokens: numberOrNull(stats['n_output_tokens']),
    elapsedMs: null,
    lastUpdatedAt: now,
    caveatRefs: [],
    blockerRefs: [],
    phase,
  }
}

/**
 * Project a parsed Harbor `result.json` into a public-safe run-progress
 * snapshot. PURE: counts + typed context only. Never copies a prompt, command,
 * response, log, or trajectory field out of the trials.
 *
 * Prefers the real Terminal-Bench 2.0 `.stats` summary shape; falls back to the
 * per-trial-array parser for other/older Harbor result shapes.
 */
export const projectHarborResultToSnapshot = (
  result: unknown,
  context: SnapshotContext,
): GymRunProgressSnapshot => {
  const now = (context.nowIso ?? (() => new Date().toISOString()))()

  if (isRecord(result)) {
    const fromStats = projectStatsToSnapshot(result, context, now)
    if (fromStats !== undefined) {
      return fromStats
    }
  }

  const trials = trialArray(result)

  const tally = trials.reduce(
    (acc, trial) => {
      const status = trialStatus(trial)
      if (status === 'cancelled' || status === 'canceled') {
        return { ...acc, cancelled: acc.cancelled + 1 }
      }
      if (status === 'error' || status === 'errored' || status === 'failed_to_run') {
        return { ...acc, error: acc.error + 1 }
      }
      const verdict = trialVerdict(trial)
      if (verdict === true) {
        return { ...acc, completedPassed: acc.completedPassed + 1 }
      }
      if (verdict === false) {
        return { ...acc, completedFailed: acc.completedFailed + 1 }
      }
      if (status === 'running' || status === 'in_progress') {
        return { ...acc, running: acc.running + 1 }
      }
      return { ...acc, pending: acc.pending + 1 }
    },
    {
      completedPassed: 0,
      completedFailed: 0,
      running: 0,
      pending: 0,
      error: 0,
      cancelled: 0,
    },
  )

  const accountedFor =
    tally.completedPassed +
    tally.completedFailed +
    tally.running +
    tally.error +
    tally.cancelled
  const remaining = Math.max(0, context.officialDenominator - accountedFor)
  // Pending is "official denominator not yet observed" plus any observed-pending.
  const pending = remaining + tally.pending

  const base: Omit<GymRunProgressSnapshot, 'phase'> = {
    runRef: context.runRef,
    jobRef: context.jobRef,
    configId: context.configId,
    profileRef: context.profileRef,
    agent: context.agent,
    publication: context.publication,
    officialDenominator: context.officialDenominator,
    completedPassed: tally.completedPassed,
    completedFailed: tally.completedFailed,
    running: tally.running,
    pending,
    error: tally.error,
    cancelled: tally.cancelled,
    promptTokens: sumTokens(trials, 'prompt_tokens'),
    completionTokens: sumTokens(trials, 'completion_tokens'),
    elapsedMs: elapsedMsFromTrials(trials),
    lastUpdatedAt: now,
    caveatRefs: [],
    blockerRefs: [],
  }

  return {
    ...base,
    phase: derivePhase(tally, context.officialDenominator, context.phase),
  }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const usage = () => `Usage:
  bun scripts/gym-harbor-progress-push.ts --result <path> --run-ref <ref> \\
    --job-ref <ref> --config-id <id> --profile-ref <ref> --agent <agent> \\
    [--official-denominator 89] [--publication local_only|web_authorized] \\
    [--phase queued|running|completed|cancelled|errored] [--poll-seconds 15] \\
    [--base-url https://openagents.com] [--once] [--dry-run] [--json]

Environment:
  OPENAGENTS_BASE_URL          Optional base URL (default ${DEFAULT_BASE_URL}).
  OPENAGENTS_ADMIN_API_TOKEN   Operator bearer. Required unless --dry-run.
  GYM_HARBOR_RESULT            Optional default for --result.
  GYM_HARBOR_RUN_REF / _JOB_REF / _CONFIG_ID / _PROFILE_REF / _AGENT /
  GYM_HARBOR_OFFICIAL_DENOMINATOR / GYM_HARBOR_PUBLICATION   Optional defaults.

Projects COUNTS + typed run context only. Never reads or sends a prompt,
response, log, trajectory, key, or private endpoint. The bearer is never
printed. The Worker re-validates and re-asserts public-safety before storing.`

type Flags = Map<string, string | true>

const parseArgs = (argv: ReadonlyArray<string>): Flags => {
  const flags: Flags = new Map()
  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index]!
    if (!raw.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${raw}`)
    }
    const name = raw.slice(2)
    const next = argv[index + 1]
    if (
      ['once', 'dry-run', 'json', 'help', 'h'].includes(name) ||
      next === undefined ||
      next.startsWith('--')
    ) {
      flags.set(name, true)
      continue
    }
    flags.set(name, next)
    index += 1
  }
  return flags
}

const flagString = (flags: Flags, name: string): string | undefined => {
  const value = flags.get(name)
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim()
    : undefined
}

const requireString = (
  flags: Flags,
  name: string,
  envName: string,
): string => {
  const value = flagString(flags, name) ?? process.env[envName]
  if (value === undefined || value.trim() === '') {
    throw new Error(`Missing required --${name} (or ${envName}).`)
  }
  return value.trim()
}

const readContext = (flags: Flags): SnapshotContext => {
  const publication = (flagString(flags, 'publication') ??
    process.env['GYM_HARBOR_PUBLICATION'] ??
    'local_only') as 'local_only' | 'web_authorized'
  if (publication !== 'local_only' && publication !== 'web_authorized') {
    throw new Error('--publication must be local_only or web_authorized.')
  }
  const phaseFlag = flagString(flags, 'phase') as Phase | undefined
  const denominator =
    flagString(flags, 'official-denominator') ??
    process.env['GYM_HARBOR_OFFICIAL_DENOMINATOR']
  return {
    runRef: requireString(flags, 'run-ref', 'GYM_HARBOR_RUN_REF'),
    jobRef: requireString(flags, 'job-ref', 'GYM_HARBOR_JOB_REF'),
    configId: requireString(flags, 'config-id', 'GYM_HARBOR_CONFIG_ID'),
    profileRef: requireString(flags, 'profile-ref', 'GYM_HARBOR_PROFILE_REF'),
    agent: requireString(flags, 'agent', 'GYM_HARBOR_AGENT'),
    officialDenominator:
      denominator === undefined
        ? DEFAULT_OFFICIAL_DENOMINATOR
        : Number(denominator),
    publication,
    ...(phaseFlag === undefined ? {} : { phase: phaseFlag }),
  }
}

const isTerminalPhase = (phase: Phase): boolean =>
  phase === 'completed' || phase === 'cancelled' || phase === 'errored'

const postSnapshot = async (
  baseUrl: string,
  token: string,
  snapshot: GymRunProgressSnapshot,
): Promise<{ status: number; body: unknown }> => {
  const response = await fetch(`${baseUrl}${INGEST_PATH}`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${token}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify(snapshot),
  })
  const body = await response.json().catch(() => ({}))
  return { status: response.status, body }
}

const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  const flags = parseArgs(argv)
  if (flags.has('help') || flags.has('h')) {
    process.stdout.write(`${usage()}\n`)
    return 0
  }

  const resultPath =
    flagString(flags, 'result') ?? process.env['GYM_HARBOR_RESULT']
  if (resultPath === undefined) {
    throw new Error('Missing required --result (or GYM_HARBOR_RESULT).')
  }
  const context = readContext(flags)
  const baseUrl =
    flagString(flags, 'base-url') ??
    process.env['OPENAGENTS_BASE_URL'] ??
    DEFAULT_BASE_URL
  const dryRun = flags.get('dry-run') === true
  const printJson = flags.get('json') === true
  const once = flags.get('once') === true
  const pollSeconds = Number(
    flagString(flags, 'poll-seconds') ?? DEFAULT_POLL_SECONDS,
  )

  const token = process.env['OPENAGENTS_ADMIN_API_TOKEN']
  if (!dryRun && (token === undefined || token.trim() === '')) {
    throw new Error('OPENAGENTS_ADMIN_API_TOKEN is required (or use --dry-run).')
  }

  const pushOnce = async (): Promise<Phase> => {
    const raw = await readFile(resultPath, 'utf8')
    const snapshot = projectHarborResultToSnapshot(JSON.parse(raw), context)
    if (dryRun) {
      process.stdout.write(`${JSON.stringify(snapshot, null, 2)}\n`)
      return snapshot.phase
    }
    const { status, body } = await postSnapshot(baseUrl, token!, snapshot)
    if (printJson) {
      process.stdout.write(`${JSON.stringify(body, null, 2)}\n`)
    } else {
      process.stdout.write(
        `[${snapshot.lastUpdatedAt}] ${snapshot.runRef} phase=${snapshot.phase} ` +
          `completed=${snapshot.completedPassed + snapshot.completedFailed}/${snapshot.officialDenominator} ` +
          `passed=${snapshot.completedPassed} -> HTTP ${status}\n`,
      )
    }
    if (status >= 400) {
      throw new Error(`Ingest rejected with HTTP ${status}.`)
    }
    return snapshot.phase
  }

  if (once || dryRun) {
    await pushOnce()
    return 0
  }

  // Poll loop: stream snapshots until the run reaches a terminal phase.
  for (;;) {
    const phase = await pushOnce()
    if (isTerminalPhase(phase)) {
      return 0
    }
    await new Promise(resolve => setTimeout(resolve, pollSeconds * 1000))
  }
}

if (import.meta.main) {
  main(process.argv.slice(2))
    .then(code => process.exit(code))
    .catch((error: unknown) => {
      const message = error instanceof Error ? error.message : String(error)
      process.stderr.write(`gym-harbor-progress-push: ${message}\n`)
      process.exit(1)
    })
}
