import { ARTANIS_ADMIN_DISPATCH_PER_DAY } from './artanis-administrator-tick'
import { parseJsonRecord } from './json-boundary'

// Public Artanis administrator-tick monitor. Clears
// blocker.product_promises.artanis_public_tick_monitor_missing on
// artanis.tassadar_evolution_loop.v1: the tick ledger becomes a
// public-safe, read-only surface so anyone can see what the
// administrator decided (dispatched / no_action / blocked /
// dispatch_failed) without operator access. Pre-mind skips
// (disabled, mind unconfigured, daily bound, no eligible Pylons) are
// not persisted rows by design - an empty day here plus online Pylons
// means the tick is skipping before the mind runs.
//
// This module is projection-only: the HTTP Response is built by the
// index route surface, and time arrives injected.

export type ArtanisTickDecisionRow = Readonly<{
  id: unknown
  state: unknown
  action_json: unknown
  assignment_ref: unknown
  created_at: unknown
}>

export type ArtanisTickMonitorEntry = Readonly<{
  decisionRef: string
  state: 'dispatched' | 'no_action' | 'blocked' | 'dispatch_failed'
  assignmentRef: string | null
  reason: string
  createdAt: string
}>

export type ArtanisTickMonitor = Readonly<{
  kind: 'artanis_admin_tick_monitor'
  publicSafe: true
  authorityBoundary: string
  dailyDispatchBound: number
  dispatchedToday: number
  countsByState: Readonly<Record<string, number>>
  decisions: ReadonlyArray<ArtanisTickMonitorEntry>
  generatedAt: string
  notes: ReadonlyArray<string>
}>

const VALID_STATES = new Set([
  'blocked',
  'dispatch_failed',
  'dispatched',
  'no_action',
])

const unsafeReasonPattern =
  /(\/Users\/|\/home\/|bearer\s+|sk-[a-z0-9]|lnbc|lntb|lno1|mnemonic|preimage|private[_-]?key|secret|api[_-]?key|token|xprv|password)/i

const sanitizeReason = (actionJson: unknown): string => {
  const parsed = parseJsonRecord(String(actionJson ?? '{}'))
  if (parsed === undefined) return 'reason.unparseable'
  const raw =
    typeof parsed.reason === 'string'
      ? parsed.reason
      : typeof parsed.rationale === 'string'
        ? parsed.rationale
        : ''
  const trimmed = raw.trim().slice(0, 200)
  if (trimmed.length === 0) return 'reason.not_recorded'
  if (unsafeReasonPattern.test(trimmed)) return 'reason.redacted'
  return trimmed
}

export const projectArtanisTickMonitor = (
  rows: ReadonlyArray<ArtanisTickDecisionRow>,
  nowIso: string,
): ArtanisTickMonitor => {
  const decisions: ArtanisTickMonitorEntry[] = []
  const countsByState: Record<string, number> = {}
  const today = nowIso.slice(0, 10)
  let dispatchedToday = 0

  for (const row of rows) {
    const state = String(row.state ?? '')
    if (!VALID_STATES.has(state)) continue
    const createdAt = String(row.created_at ?? '')
    countsByState[state] = (countsByState[state] ?? 0) + 1
    if (state === 'dispatched' && createdAt.startsWith(today)) {
      dispatchedToday += 1
    }
    decisions.push({
      assignmentRef:
        row.assignment_ref === null || row.assignment_ref === undefined
          ? null
          : String(row.assignment_ref),
      createdAt,
      decisionRef: `tick_decision.${String(row.id ?? 'unknown')}`,
      reason: sanitizeReason(row.action_json),
      state: state as ArtanisTickMonitorEntry['state'],
    })
  }

  return {
    authorityBoundary:
      'Read-only tick ledger projection. Grants no dispatch, spend, assignment, settlement, or registry authority; reasons are truncated and redaction-scanned; raw mind output is never projected.',
    countsByState,
    dailyDispatchBound: ARTANIS_ADMIN_DISPATCH_PER_DAY,
    decisions,
    dispatchedToday,
    generatedAt: nowIso,
    kind: 'artanis_admin_tick_monitor',
    notes: [
      'Pre-mind skips (runner disabled, mind unconfigured, daily bound reached, no eligible online Pylons) are not persisted decisions; an empty window with online Pylons means the tick is skipping before the mind runs.',
    ],
    publicSafe: true,
  }
}

export const ARTANIS_TICK_MONITOR_MAX_LIMIT = 50

export const boundedTickMonitorLimit = (raw: string | null): number => {
  const parsed = Number(raw ?? '20')
  if (!Number.isFinite(parsed)) return 20
  return Math.min(
    Math.max(1, Math.trunc(parsed)),
    ARTANIS_TICK_MONITOR_MAX_LIMIT,
  )
}

export const readArtanisTickMonitor = async (
  db: D1Database,
  input: Readonly<{ limit: number; nowIso: string }>,
): Promise<ArtanisTickMonitor> => {
  const limit = boundedTickMonitorLimit(String(input.limit))
  const result = await db
    .prepare(
      `SELECT id, state, action_json, assignment_ref, created_at
         FROM artanis_admin_tick_decisions
        ORDER BY created_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all()

  return projectArtanisTickMonitor(
    (result.results ?? []) as unknown as ReadonlyArray<ArtanisTickDecisionRow>,
    input.nowIso,
  )
}
