/**
 * Blueprint Signature — `fleet-liveness` (#6646)
 *
 * Wedge detection for the codex-supervisor dispatch loop. The supervisor can be
 * ALIVE and heartbeating yet WEDGED: an external `gh`/network call in the
 * dispatch loop hung with no timeout and silently stalled async dispatch while
 * the independent heartbeat kept firing. This is the #1 token-burn failure mode
 * (it can cost ~all of a day's burn); see
 * `docs/afteraction/2026-06-26-khala-pylon-codex-delegation-afteraction.md` and
 * the burn runbook `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`.
 *
 * The timeout guards in the supervisor scripts (lockout.sh +
 * supervisor-task-pool.sh + codex-supervisor.sh, via `sup_run_timeout`) make a
 * single hung call no longer able to stall the loop. This module is the
 * defense-in-depth layer on top: the tested source of truth for "is the
 * supervisor wedged?". Given the supervisor pid liveness plus the most recent
 * dispatch-ATTEMPT timestamp (written by the supervisor to
 * `$SUP_STATE_DIR/last_dispatch_time`), it returns `wedged` when the process is
 * alive but no dispatch has been attempted within the wedge threshold (default
 * 10 min). `launch.sh wedge-watch` consumes the CLI entry below to force-kill +
 * restart a wedged supervisor.
 *
 * Pure core (`evaluateFleetLiveness`) + a thin I/O CLI. Aligns with the other
 * blueprint-gates. It is intentionally NOT run from inside the supervisor
 * itself: a wedged process cannot reliably restart itself, so an external
 * watcher owns the restart decision.
 */

import { readdir } from "node:fs/promises"

export const FLEET_LIVENESS_STATES = ["healthy", "wedged", "unknown"] as const
export const FLEET_LIVENESS_GATE_STATES = [
  "BLOCKED",
  "DISPATCH_ATTEMPT_SEEN",
  "QUOTA_CHECKED",
  "PROVEN_ALIVE",
] as const

export type FleetLivenessStatus = (typeof FLEET_LIVENESS_STATES)[number]
export type FleetLivenessGateState =
  (typeof FLEET_LIVENESS_GATE_STATES)[number]

export const FLEET_LIVENESS_EVIDENCE = {
  lastDispatchAttempt: "evidence://supervisor/last-dispatch-attempt",
  quotaLedgerSnapshot: "evidence://supervisor/quota-ledger-snapshot",
  heartbeatPayload: "evidence://supervisor/heartbeat-payload",
} as const

export type FleetLivenessEvidenceRef =
  (typeof FLEET_LIVENESS_EVIDENCE)[keyof typeof FLEET_LIVENESS_EVIDENCE]

/** Default wedge threshold: alive but no dispatch attempt in 10 minutes. */
export const DEFAULT_WEDGE_THRESHOLD_MS = 10 * 60 * 1000

/** Process exit codes for the CLI: stable so a watcher can branch on them. */
export const FLEET_LIVENESS_EXIT = {
  healthy: 0,
  wedged: 3,
  unknown: 4,
} as const

export interface FleetLivenessInputs {
  /** Is the supervisor process currently alive? */
  readonly pidAlive: boolean
  /** Epoch ms of the most recent dispatch ATTEMPT, or null if none recorded. */
  readonly lastDispatchTime: number | null
  /** Current time, epoch ms. */
  readonly now: number
  /** Override the wedge threshold (ms). Non-positive values are ignored. */
  readonly wedgeThresholdMs?: number
  /** evidence://supervisor/quota-ledger-snapshot */
  readonly quotaLedgerSnapshot?: unknown
  /** evidence://supervisor/heartbeat-payload */
  readonly heartbeatPayload?: unknown
}

export interface FleetLivenessResult {
  readonly status: FleetLivenessStatus
  readonly gateState: FleetLivenessGateState
  readonly canReportHealthy: boolean
  /** True only when alive AND last dispatch attempt is older than threshold. */
  readonly wedged: boolean
  /** True only when alive AND last dispatch attempt is within threshold. */
  readonly healthy: boolean
  readonly satisfiedEvidence: ReadonlyArray<FleetLivenessEvidenceRef>
  readonly missingEvidence: ReadonlyArray<FleetLivenessEvidenceRef>
  /** Age of the last dispatch attempt in ms (null when unknown). */
  readonly ageMs: number | null
  readonly wedgeThresholdMs: number
  readonly reason: string
}

/**
 * Evaluate supervisor liveness. Pure function.
 *
 * - not alive          -> `unknown` (a dead/stopped process is the watcher's
 *                          ordinary start path, not a wedge)
 * - no dispatch yet    -> `unknown`
 * - alive, fresh       -> `healthy` (age <= threshold)
 * - alive, stale       -> `wedged`  (age >  threshold)
 */
export function evaluateFleetLiveness(
  inputs: FleetLivenessInputs,
): FleetLivenessResult {
  const threshold =
    typeof inputs.wedgeThresholdMs === "number" && inputs.wedgeThresholdMs > 0
      ? inputs.wedgeThresholdMs
      : DEFAULT_WEDGE_THRESHOLD_MS

  const satisfied: Array<FleetLivenessEvidenceRef> = []
  const missing: Array<FleetLivenessEvidenceRef> = []

  const finish = (partial: {
    status: FleetLivenessStatus
    gateState: FleetLivenessGateState
    wedged: boolean
    healthy: boolean
    ageMs: number | null
    reason: string
  }): FleetLivenessResult => ({
    ...partial,
    canReportHealthy: partial.gateState === "PROVEN_ALIVE",
    satisfiedEvidence: satisfied,
    missingEvidence: missing,
    wedgeThresholdMs: threshold,
  })

  if (!inputs.pidAlive) {
    missing.push(
      FLEET_LIVENESS_EVIDENCE.lastDispatchAttempt,
      FLEET_LIVENESS_EVIDENCE.quotaLedgerSnapshot,
      FLEET_LIVENESS_EVIDENCE.heartbeatPayload,
    )
    return finish({
      status: "unknown",
      gateState: "BLOCKED",
      wedged: false,
      healthy: false,
      ageMs: null,
      reason: "supervisor process is not alive",
    })
  }

  if (
    inputs.lastDispatchTime === null ||
    !Number.isFinite(inputs.lastDispatchTime)
  ) {
    missing.push(
      FLEET_LIVENESS_EVIDENCE.lastDispatchAttempt,
      FLEET_LIVENESS_EVIDENCE.quotaLedgerSnapshot,
      FLEET_LIVENESS_EVIDENCE.heartbeatPayload,
    )
    return finish({
      status: "unknown",
      gateState: "BLOCKED",
      wedged: false,
      healthy: false,
      ageMs: null,
      reason: "no dispatch attempt recorded yet",
    })
  }

  const ageMs = inputs.now - inputs.lastDispatchTime
  satisfied.push(FLEET_LIVENESS_EVIDENCE.lastDispatchAttempt)

  if (ageMs > threshold) {
    missing.push(
      FLEET_LIVENESS_EVIDENCE.quotaLedgerSnapshot,
      FLEET_LIVENESS_EVIDENCE.heartbeatPayload,
    )
    return finish({
      status: "wedged",
      gateState: "DISPATCH_ATTEMPT_SEEN",
      wedged: true,
      healthy: false,
      ageMs,
      reason: `alive but last dispatch attempt was ${ageMs}ms ago (> ${threshold}ms threshold)`,
    })
  }

  if (inputs.quotaLedgerSnapshot === undefined || inputs.quotaLedgerSnapshot === null) {
    missing.push(
      FLEET_LIVENESS_EVIDENCE.quotaLedgerSnapshot,
      FLEET_LIVENESS_EVIDENCE.heartbeatPayload,
    )
    return finish({
      status: "unknown",
      gateState: "DISPATCH_ATTEMPT_SEEN",
      wedged: false,
      healthy: false,
      ageMs,
      reason: "dispatch attempt is fresh but quota ledger snapshot evidence is missing",
    })
  }
  satisfied.push(FLEET_LIVENESS_EVIDENCE.quotaLedgerSnapshot)

  if (inputs.heartbeatPayload === undefined || inputs.heartbeatPayload === null) {
    missing.push(FLEET_LIVENESS_EVIDENCE.heartbeatPayload)
    return finish({
      status: "unknown",
      gateState: "QUOTA_CHECKED",
      wedged: false,
      healthy: false,
      ageMs,
      reason: "dispatch attempt and quota evidence exist but heartbeat payload evidence is missing",
    })
  }
  satisfied.push(FLEET_LIVENESS_EVIDENCE.heartbeatPayload)

  return finish({
    status: "healthy",
    gateState: "PROVEN_ALIVE",
    wedged: false,
    healthy: true,
    ageMs,
    reason: `alive and last dispatch attempt was ${ageMs}ms ago (<= ${threshold}ms threshold)`,
  })
}

/**
 * Parse a `last_dispatch_time` file payload: epoch ms (digits) or an ISO-8601
 * timestamp. Returns epoch ms, or null when empty/unparseable.
 */
export function parseLastDispatchTime(
  raw: string | null | undefined,
): number | null {
  if (typeof raw !== "string") {
    return null
  }
  const trimmed = raw.trim()
  if (trimmed.length === 0) {
    return null
  }
  if (/^[0-9]+$/.test(trimmed)) {
    const n = Number.parseInt(trimmed, 10)
    return Number.isFinite(n) ? n : null
  }
  const parsed = Date.parse(trimmed)
  return Number.isFinite(parsed) ? parsed : null
}

function parsePositiveInt(raw: string | undefined): number | null {
  if (typeof raw !== "string") {
    return null
  }
  const n = Number.parseInt(raw.trim(), 10)
  return Number.isFinite(n) && n > 0 ? n : null
}

async function readTextOrNull(path: string): Promise<string | null> {
  try {
    return await Bun.file(path).text()
  } catch {
    return null
  }
}

async function readJsonOrNull(path: string): Promise<unknown | null> {
  const raw = await readTextOrNull(path)
  if (raw === null) {
    return null
  }
  try {
    return JSON.parse(raw)
  } catch {
    return null
  }
}

async function readQuotaLedgerSnapshot(home: string): Promise<unknown | null> {
  if (home.length === 0) {
    return null
  }
  const quotaDir = `${home}/.pylon/account-quota`
  try {
    const names = await readdir(quotaDir)
    const entries: Array<unknown> = []
    for (const name of names.sort()) {
      if (!name.endsWith(".json")) {
        continue
      }
      const parsed = await readJsonOrNull(`${quotaDir}/${name}`)
      if (parsed !== null) {
        entries.push(parsed)
      }
    }
    return { quotaDir, entries }
  } catch {
    return null
  }
}

/**
 * CLI: read the supervisor pid + last_dispatch_time from `$SUP_STATE_DIR`,
 * evaluate liveness, print a JSON verdict, and exit with FLEET_LIVENESS_EXIT.
 * Consumed by `launch.sh wedge-watch` / `wedge-check`.
 */
export async function runFleetLivenessCli(): Promise<number> {
  const home = process.env.HOME ?? ""
  const stateDir =
    typeof process.env.SUP_STATE_DIR === "string" &&
    process.env.SUP_STATE_DIR.length > 0
      ? process.env.SUP_STATE_DIR
      : `${home}/.codex-supervisor`
  const pidPath = `${stateDir}/supervisor.pid`
  const lastDispatchPath = `${stateDir}/last_dispatch_time`
  const heartbeatPayloadPath = `${stateDir}/heartbeat_payload.json`
  const thresholdMs =
    parsePositiveInt(process.env.SUP_WEDGE_THRESHOLD_MS) ??
    DEFAULT_WEDGE_THRESHOLD_MS

  let pid: number | null = null
  let pidAlive = false
  const pidRaw = await readTextOrNull(pidPath)
  if (pidRaw !== null) {
    const parsed = Number.parseInt(pidRaw.trim(), 10)
    if (Number.isFinite(parsed) && parsed > 0) {
      pid = parsed
      try {
        // Signal 0 = liveness probe (no signal delivered).
        process.kill(parsed, 0)
        pidAlive = true
      } catch (err) {
        // EPERM means the process exists but is owned by another user -> alive.
        const code = (err as { code?: string } | null)?.code
        pidAlive = code === "EPERM"
      }
    }
  }

  const lastDispatchTime = parseLastDispatchTime(
    await readTextOrNull(lastDispatchPath),
  )
  const quotaLedgerSnapshot = await readQuotaLedgerSnapshot(home)
  const heartbeatPayload = await readJsonOrNull(heartbeatPayloadPath)

  const result = evaluateFleetLiveness({
    pidAlive,
    lastDispatchTime,
    now: Date.now(),
    wedgeThresholdMs: thresholdMs,
    quotaLedgerSnapshot,
    heartbeatPayload,
  })

  console.log(
    JSON.stringify({
      pid,
      pidAlive,
      lastDispatchTime,
      stateDir,
      ...result,
    }),
  )

  return FLEET_LIVENESS_EXIT[result.status]
}

if (import.meta.main) {
  runFleetLivenessCli()
    .then((code) => process.exit(code))
    .catch((err) => {
      console.error(
        `fleet-liveness CLI error: ${err instanceof Error ? err.message : String(err)}`,
      )
      process.exit(FLEET_LIVENESS_EXIT.unknown)
    })
}
