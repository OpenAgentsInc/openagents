/**
 * Fleet "never silently stalls" watchdog — server-side detection + recovery
 * (openagents#6408).
 *
 * Owner mandate: nothing must stop the own-capacity Codex burn fleet; if it ever
 * stalls, the system must auto-detect and auto-recover within minutes. This is
 * the Cloudflare-native server half (a 1-minute Worker cron tick). The local
 * half is the launchd-managed supervisors + their in-loop self-heal.
 *
 * Detection: measure the live own-capacity burn rate from `token_usage_events`
 * (provider `pylon-codex-own-capacity`, demand `own_capacity` /
 * `khala_coding_delegation`) over a rolling window, and read whether there are
 * active coding leases. The classifier deliberately distinguishes:
 *   - healthy       — burn at/above threshold, fleet is producing tokens.
 *   - idle_no_work  — burn below threshold BUT no active leases. A genuinely
 *                     idle fleet with no work is NOT an alarm.
 *   - stalled       — burn below threshold WHILE active leases exist. This is
 *                     the failure mode: work is leased but no tokens are
 *                     flowing (e.g. abandoned leases poisoning the dispatch gate
 *                     into `duplicate_active_assignment`).
 *
 * Recovery (on stall only): force-flush stale/abandoned active leases for the
 * configured owner pylon(s) so the dispatch gate stops refusing fresh work, and
 * record every action taken for audit in `fleet_alerts`. This module does NOT
 * touch the dispatch-gate dedup logic itself — a separate lane owns that
 * hotfix; here we only clear the poisoned rows it trips on.
 *
 * No third-party services: the alert sink is a durable D1 row plus a loud
 * structured log line, per the prefer-cloudflare-primitives invariant.
 */

import {
  epochMillisToIsoTimestamp,
  isoTimestampAfterIso,
  randomUuid,
} from '../runtime-primitives'

/** Rolling window (minutes) used to measure the burn rate. */
export const FLEET_BURN_STALL_WINDOW_MINUTES = 5

/**
 * Stall threshold: minimum own-capacity tokens that must burn within the rolling
 * window for the fleet to be considered "producing". Below this WITH active work
 * is a stall. Roughly ~1M tokens / 5-min window (the runbook's "burn < ~1M
 * tokens per rolling 5-min window" definition).
 */
export const FLEET_BURN_STALL_THRESHOLD_TOKENS = 1_000_000

/**
 * Coding-assignment states that hold a dispatch slot (mirror of the dispatch
 * gate's `duplicateBlockingAssignmentStates`). Kept as a local constant so the
 * detector's "is there active work" question matches what the gate counts,
 * without importing or mutating the gate logic.
 */
export const FLEET_ACTIVE_CODING_ASSIGNMENT_STATES = [
  'accepted',
  'blocked',
  'offered',
  'proof_submitted',
  'running',
] as const

export type FleetBurnStatus = 'healthy' | 'idle_no_work' | 'stalled'

export const FLEET_BURN_HEALTHY_REASON_REF = 'fleet.burn.healthy'
export const FLEET_BURN_IDLE_REASON_REF = 'fleet.burn.idle_no_work'
export const FLEET_BURN_STALLED_REASON_REF =
  'blocker.public.fleet.burn_stalled_with_active_work'

export type FleetBurnClassificationInput = Readonly<{
  burnTokensInWindow: number
  activeCodingAssignments: number
  queuedCodingAssignments: number
  stallThresholdTokens: number
}>

export type FleetBurnClassification = Readonly<{
  status: FleetBurnStatus
  reasonRef: string
  burnTokensInWindow: number
  activeCodingAssignments: number
  queuedCodingAssignments: number
  hasActiveWork: boolean
  belowThreshold: boolean
  stallThresholdTokens: number
}>

const nonNegativeInt = (value: number): number => Math.max(0, Math.trunc(value))

/**
 * Pure stall classifier. Unit-tested exhaustively: stalled-with-work alerts,
 * idle-no-work does not, healthy does not. No I/O, no clock.
 */
export const classifyFleetBurnState = (
  input: FleetBurnClassificationInput,
): FleetBurnClassification => {
  const burnTokensInWindow = nonNegativeInt(input.burnTokensInWindow)
  const activeCodingAssignments = nonNegativeInt(input.activeCodingAssignments)
  const queuedCodingAssignments = nonNegativeInt(input.queuedCodingAssignments)
  const stallThresholdTokens = Math.max(1, nonNegativeInt(input.stallThresholdTokens))
  const hasActiveWork =
    activeCodingAssignments + queuedCodingAssignments > 0
  const belowThreshold = burnTokensInWindow < stallThresholdTokens

  const status: FleetBurnStatus = !belowThreshold
    ? 'healthy'
    : hasActiveWork
      ? 'stalled'
      : 'idle_no_work'

  const reasonRef =
    status === 'healthy'
      ? FLEET_BURN_HEALTHY_REASON_REF
      : status === 'idle_no_work'
        ? FLEET_BURN_IDLE_REASON_REF
        : FLEET_BURN_STALLED_REASON_REF

  return {
    activeCodingAssignments,
    belowThreshold,
    burnTokensInWindow,
    hasActiveWork,
    queuedCodingAssignments,
    reasonRef,
    stallThresholdTokens,
    status,
  }
}

export type FleetWatchdogConfig = Readonly<{
  enabled: boolean
  recoveryEnabled: boolean
  windowMinutes: number
  stallThresholdTokens: number
  /** Owner pylon refs whose abandoned leases may be auto-flushed on a stall. */
  ownerPylonRefs: ReadonlyArray<string>
  /**
   * A lease is "abandoned" only if it has held the slot for at least this many
   * minutes without progressing — protects an in-flight just-leased turn from a
   * premature flush.
   */
  staleLeaseMinAgeMinutes: number
}>

const parseBool = (value: unknown, fallback: boolean): boolean => {
  if (typeof value !== 'string' || value.trim() === '') {
    return fallback
  }
  return value.trim().toLowerCase() === 'true'
}

const parsePositiveInt = (value: unknown, fallback: number): number => {
  if (typeof value !== 'string' || value.trim() === '') {
    return fallback
  }
  const parsed = Number.parseInt(value.trim(), 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback
}

const parseRefList = (value: unknown): ReadonlyArray<string> => {
  if (typeof value !== 'string') {
    return []
  }
  return [
    ...new Set(
      value
        .split(/[\s,]+/)
        .map(ref => ref.trim())
        .filter(ref => ref !== ''),
    ),
  ]
}

/**
 * Resolve watchdog config from the Worker env. Detection defaults ON (so a live
 * stall is flagged immediately); recovery defaults ON but is a no-op unless
 * owner pylon refs are configured (a safety scoping so the watchdog never
 * flushes a pylon it was not told to own).
 */
export const readFleetWatchdogConfig = (env: unknown): FleetWatchdogConfig => {
  const record = (env ?? {}) as Record<string, unknown>
  return {
    enabled: parseBool(record.FLEET_WATCHDOG_ENABLED, true),
    ownerPylonRefs: parseRefList(record.FLEET_WATCHDOG_OWNER_PYLON_REFS),
    recoveryEnabled: parseBool(record.FLEET_WATCHDOG_RECOVERY_ENABLED, true),
    staleLeaseMinAgeMinutes: parsePositiveInt(
      record.FLEET_WATCHDOG_STALE_LEASE_MIN_AGE_MINUTES,
      10,
    ),
    stallThresholdTokens: parsePositiveInt(
      record.FLEET_WATCHDOG_STALL_THRESHOLD_TOKENS,
      FLEET_BURN_STALL_THRESHOLD_TOKENS,
    ),
    windowMinutes: parsePositiveInt(
      record.FLEET_WATCHDOG_WINDOW_MINUTES,
      FLEET_BURN_STALL_WINDOW_MINUTES,
    ),
  }
}

export type FleetAlertRecord = Readonly<{
  id: string
  alertRef: string
  detectedAt: string
  classification: FleetBurnStatus
  reasonRef: string
  burnTokensWindow: number
  windowMinutes: number
  stallThresholdTokens: number
  activeAssignments: number
  queuedAssignments: number
  recoveryActions: ReadonlyArray<string>
  recoveredLeaseCount: number
  createdAt: string
}>

export type FleetWatchdogStore = Readonly<{
  /** Sum own-capacity coding-delegation tokens with observed_at >= sinceIso. */
  sumOwnCapacityBurnSince: (sinceIso: string) => Promise<number>
  /**
   * Count active (slot-holding, non-expired) coding leases. When pylonRefs is
   * non-empty, scope to those pylons; otherwise count fleet-wide. Returns split
   * active / queued (queued = the `offered` subset).
   */
  countActiveCodingAssignments: (
    pylonRefs: ReadonlyArray<string>,
    nowIso: string,
  ) => Promise<Readonly<{ active: number; queued: number }>>
  /**
   * Force-flush abandoned active leases (slot-holding, non-expired, idle past
   * the stale-age cutoff) for the given pylons by marking them terminal
   * (`cancelled`) and expiring the lease so they stop poisoning the dispatch
   * gate. Returns the flushed assignment refs. Does NOT modify gate logic.
   */
  flushAbandonedLeases: (
    pylonRefs: ReadonlyArray<string>,
    nowIso: string,
    staleBeforeIso: string,
  ) => Promise<ReadonlyArray<string>>
  insertAlert: (record: FleetAlertRecord) => Promise<void>
}>

const isoMinutesBefore = (nowIso: string, minutes: number): string =>
  isoTimestampAfterIso(nowIso, -minutes * 60_000)

export type FleetBurnStallTickResult = Readonly<{
  ranAt: string
  enabled: boolean
  classification: FleetBurnClassification
  alertRef: string | null
  recoveryActions: ReadonlyArray<string>
  recoveredLeaseRefs: ReadonlyArray<string>
}>

/**
 * One watchdog tick. Measures burn + active work, classifies, and on a stall
 * writes a loud alert row + log line and (when recovery is enabled and owner
 * pylon refs are configured) force-flushes abandoned leases, recording every
 * action. Healthy / idle ticks are silent (no row, no recovery).
 */
export const runFleetBurnStallDetectorTick = async (
  input: Readonly<{
    store: FleetWatchdogStore
    config: FleetWatchdogConfig
    nowIso: string
    makeId: () => string
    log?: (line: string, fields: Record<string, unknown>) => void
  }>,
): Promise<FleetBurnStallTickResult> => {
  const { config, makeId, nowIso, store } = input
  const log = input.log ?? (() => {})

  if (!config.enabled) {
    return {
      alertRef: null,
      classification: classifyFleetBurnState({
        activeCodingAssignments: 0,
        burnTokensInWindow: 0,
        queuedCodingAssignments: 0,
        stallThresholdTokens: config.stallThresholdTokens,
      }),
      enabled: false,
      ranAt: nowIso,
      recoveredLeaseRefs: [],
      recoveryActions: [],
    }
  }

  const windowStartIso = isoMinutesBefore(nowIso, config.windowMinutes)
  const burnTokensInWindow = await store.sumOwnCapacityBurnSince(windowStartIso)
  const { active, queued } = await store.countActiveCodingAssignments(
    config.ownerPylonRefs,
    nowIso,
  )

  const classification = classifyFleetBurnState({
    activeCodingAssignments: active,
    burnTokensInWindow,
    queuedCodingAssignments: queued,
    stallThresholdTokens: config.stallThresholdTokens,
  })

  if (classification.status !== 'stalled') {
    return {
      alertRef: null,
      classification,
      enabled: true,
      ranAt: nowIso,
      recoveredLeaseRefs: [],
      recoveryActions: [],
    }
  }

  // --- Stall: loud alert + auto-recovery. ---
  const recoveryActions: string[] = []
  let recoveredLeaseRefs: ReadonlyArray<string> = []

  if (!config.recoveryEnabled) {
    recoveryActions.push('recovery.skipped.disabled')
  } else if (config.ownerPylonRefs.length === 0) {
    recoveryActions.push('recovery.skipped.no_owner_pylon_refs_configured')
  } else {
    const staleBeforeIso = isoMinutesBefore(
      nowIso,
      config.staleLeaseMinAgeMinutes,
    )
    try {
      recoveredLeaseRefs = await store.flushAbandonedLeases(
        config.ownerPylonRefs,
        nowIso,
        staleBeforeIso,
      )
      recoveryActions.push(
        `recovery.flushed_abandoned_leases.count=${recoveredLeaseRefs.length}`,
      )
      if (recoveredLeaseRefs.length === 0) {
        recoveryActions.push('recovery.no_abandoned_leases_found')
      }
    } catch (error) {
      recoveryActions.push(
        `recovery.flush_failed.${error instanceof Error ? error.name : 'unknown'}`,
      )
    }
  }

  const alertRef = `fleet_alert.${nowIso}.${makeId().slice(0, 8)}`
  const record: FleetAlertRecord = {
    activeAssignments: classification.activeCodingAssignments,
    alertRef,
    burnTokensWindow: classification.burnTokensInWindow,
    classification: 'stalled',
    createdAt: nowIso,
    detectedAt: nowIso,
    id: makeId(),
    queuedAssignments: classification.queuedCodingAssignments,
    reasonRef: classification.reasonRef,
    recoveredLeaseCount: recoveredLeaseRefs.length,
    recoveryActions,
    stallThresholdTokens: classification.stallThresholdTokens,
    windowMinutes: config.windowMinutes,
  }

  // Loud, structured alert line (the Cloudflare-native notification surface
  // alongside the durable D1 row). Public-safe refs only.
  log('FLEET-STALL detected by watchdog cron', {
    activeAssignments: classification.activeCodingAssignments,
    alertRef,
    blockerRef: FLEET_BURN_STALLED_REASON_REF,
    burnTokensWindow: classification.burnTokensInWindow,
    queuedAssignments: classification.queuedCodingAssignments,
    recoveredLeaseCount: recoveredLeaseRefs.length,
    recoveryActions: recoveryActions.join(','),
    stallThresholdTokens: classification.stallThresholdTokens,
    windowMinutes: config.windowMinutes,
  })

  try {
    await store.insertAlert(record)
  } catch (error) {
    log('FLEET-STALL alert persistence failed', {
      alertRef,
      errorName: error instanceof Error ? error.name : 'unknown',
    })
  }

  return {
    alertRef,
    classification,
    enabled: true,
    ranAt: nowIso,
    recoveredLeaseRefs,
    recoveryActions,
  }
}

const sqlPlaceholders = (count: number): string =>
  new Array(count).fill('?').join(', ')

const ACTIVE_STATE_PLACEHOLDERS = sqlPlaceholders(
  FLEET_ACTIVE_CODING_ASSIGNMENT_STATES.length,
)

/**
 * D1-backed watchdog store. The burn query is scoped to the own-capacity Codex
 * coding-delegation lane (the lane the daily token target rides and the one the
 * gate regression stalls), matching the canonical `token_usage_events` filter.
 */
export const makeD1FleetWatchdogStore = (db: D1Database): FleetWatchdogStore => ({
  countActiveCodingAssignments: async (pylonRefs, nowIso) => {
    const scoped = pylonRefs.length > 0
    const result = await db
      .prepare(
        `SELECT
            COUNT(*) AS active,
            COALESCE(SUM(CASE WHEN state = 'offered' THEN 1 ELSE 0 END), 0) AS queued
          FROM pylon_api_assignments
          WHERE state IN (${ACTIVE_STATE_PLACEHOLDERS})
            AND lease_expires_at > ?
            ${scoped ? `AND pylon_ref IN (${sqlPlaceholders(pylonRefs.length)})` : ''}`,
      )
      .bind(
        ...FLEET_ACTIVE_CODING_ASSIGNMENT_STATES,
        nowIso,
        ...(scoped ? pylonRefs : []),
      )
      .first<{ active: number | null; queued: number | null }>()
    return {
      active: Math.max(0, Math.trunc(result?.active ?? 0)),
      queued: Math.max(0, Math.trunc(result?.queued ?? 0)),
    }
  },

  flushAbandonedLeases: async (pylonRefs, nowIso, staleBeforeIso) => {
    if (pylonRefs.length === 0) {
      return []
    }
    const pylonPlaceholders = sqlPlaceholders(pylonRefs.length)
    const selectResult = await db
      .prepare(
        `SELECT assignment_ref FROM pylon_api_assignments
          WHERE pylon_ref IN (${pylonPlaceholders})
            AND state IN (${ACTIVE_STATE_PLACEHOLDERS})
            AND lease_expires_at > ?
            AND updated_at <= ?`,
      )
      .bind(
        ...pylonRefs,
        ...FLEET_ACTIVE_CODING_ASSIGNMENT_STATES,
        nowIso,
        staleBeforeIso,
      )
      .all<{ assignment_ref: string }>()
    const refs = (selectResult.results ?? [])
      .map(row => row.assignment_ref)
      .filter((ref): ref is string => typeof ref === 'string')
    if (refs.length === 0) {
      return []
    }
    // Mark terminal + expire the lease so it stops counting toward the gate's
    // active-lease tally. State is set to the existing terminal 'cancelled'
    // literal; no gate logic is changed.
    await db
      .prepare(
        `UPDATE pylon_api_assignments
            SET state = 'cancelled', lease_expires_at = ?, updated_at = ?
          WHERE pylon_ref IN (${pylonPlaceholders})
            AND state IN (${ACTIVE_STATE_PLACEHOLDERS})
            AND lease_expires_at > ?
            AND updated_at <= ?`,
      )
      .bind(
        nowIso,
        nowIso,
        ...pylonRefs,
        ...FLEET_ACTIVE_CODING_ASSIGNMENT_STATES,
        nowIso,
        staleBeforeIso,
      )
      .run()
    return refs
  },

  insertAlert: async record => {
    await db
      .prepare(
        `INSERT INTO fleet_alerts
          (id, alert_ref, detected_at, classification, reason_ref,
           burn_tokens_window, window_minutes, stall_threshold_tokens,
           active_assignments, queued_assignments, recovery_actions_json,
           recovered_lease_count, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        record.id,
        record.alertRef,
        record.detectedAt,
        record.classification,
        record.reasonRef,
        record.burnTokensWindow,
        record.windowMinutes,
        record.stallThresholdTokens,
        record.activeAssignments,
        record.queuedAssignments,
        JSON.stringify(record.recoveryActions),
        record.recoveredLeaseCount,
        record.createdAt,
      )
      .run()
  },

  sumOwnCapacityBurnSince: async sinceIso => {
    const result = await db
      .prepare(
        `SELECT COALESCE(SUM(total_tokens), 0) AS burn
           FROM token_usage_events
          WHERE demand_kind = 'own_capacity'
            AND demand_source = 'khala_coding_delegation'
            AND observed_at >= ?`,
      )
      .bind(sinceIso)
      .first<{ burn: number | null }>()
    return Math.max(0, Math.trunc(result?.burn ?? 0))
  },
})

/**
 * Scheduled (1-minute cron) entrypoint. Resolves config from env, builds the
 * D1 store, and runs one watchdog tick. Fail-soft: any error is caught and
 * logged so the watchdog never throws out of the shared scheduled handler. The
 * caller supplies the log sink (the Worker's redacted Effect observability
 * helper), so this module emits no raw console output.
 */
export const runFleetBurnStallDetectorScheduled = async (
  db: D1Database,
  env: unknown,
  input: Readonly<{ scheduledTimeMs: number }>,
  log: (line: string, fields: Record<string, unknown>) => void,
): Promise<FleetBurnStallTickResult | null> => {
  const config = readFleetWatchdogConfig(env)
  const nowIso = epochMillisToIsoTimestamp(input.scheduledTimeMs)
  try {
    return await runFleetBurnStallDetectorTick({
      config,
      log,
      makeId: () => randomUuid(),
      nowIso,
      store: makeD1FleetWatchdogStore(db),
    })
  } catch (error) {
    log('FleetBurnStallDetector tick failed', {
      errorName: error instanceof Error ? error.name : 'unknown',
    })
    return null
  }
}
