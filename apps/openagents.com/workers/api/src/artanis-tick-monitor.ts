import { ARTANIS_ADMIN_DISPATCH_PER_DAY } from './artanis-administrator-tick'
import {
  artanisAdminCloseoutReceiptRecordFromRow,
  artanisAdminCloseoutReceiptRef,
} from './artanis-admin-closeout-receipts'
import { parseJsonRecord } from './json-boundary'
import { publicScannerSafeRefs } from './public-ref-scanner-safety'
import { liveAtReadStaleness } from './public-projection-staleness'
import type { PublicProjectionStalenessContract } from './public-projection-staleness'
import { TASSADAR_TICK_CLOSURE_CONTRACT_VERSION } from './tassadar-trace-factory/tick-closure'

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
  accepted_work_refs_json?: unknown
  artifact_refs_json?: unknown
  assignment_created_at?: unknown
  id: unknown
  state: unknown
  action_json: unknown
  assignment_ref: unknown
  assignment_state?: unknown
  assignment_updated_at?: unknown
  closeout_refs_json?: unknown
  created_at: unknown
  job_kind?: unknown
  proof_refs_json?: unknown
  pylon_ref?: unknown
  verdict_accept_state?: unknown
  verdict_created_at?: unknown
  verdict_outcome?: unknown
  verdict_trace_digest_prefix?: unknown
}>

export type ArtanisClosedTickFaceRefs = Readonly<{
  evaluationRefs: ReadonlyArray<string>
  executionRefs: ReadonlyArray<string>
  intentRefs: ReadonlyArray<string>
  stateDeltaRefs: ReadonlyArray<string>
}>

export type ArtanisClosedTickReceipt = Readonly<{
  assignmentRef: string
  caveatRefs: ReadonlyArray<string>
  closureContractVersion: typeof TASSADAR_TICK_CLOSURE_CONTRACT_VERSION
  closeoutReceiptRef: string
  faceRefs: ArtanisClosedTickFaceRefs
  pylonRef: string
  provenanceLabel: string
  receiptKind: 'artanis_tetrahedron_closed_tick'
  receiptRef: string
  verdictOutcome: 'verified'
  verdictRef: string
}>

export type ArtanisUnattendedTickStreak = Readonly<{
  authorityBoundary: string
  blockerRefs: ReadonlyArray<string>
  closedTickReceiptRefs: ReadonlyArray<string>
  currentConsecutiveClosedTicks: number
  decisionRefs: ReadonlyArray<string>
  kind: 'artanis_unattended_tick_streak'
  longestConsecutiveClosedTicks: number
  provenanceLabel: string
  receiptRef: string | null
  requiredConsecutiveClosedTicks: 10
  satisfied: boolean
  staleness: PublicProjectionStalenessContract
}>

export type ArtanisTickMonitorEntry = Readonly<{
  decisionRef: string
  state: 'dispatched' | 'no_action' | 'blocked' | 'dispatch_failed'
  assignmentRef: string | null
  closedTickReceiptRef: string | null
  closeoutReceiptRef: string | null
  closureState:
    | 'closed_verified'
    | 'open_no_assignment'
    | 'open_no_closeout'
    | 'open_no_verified_replay'
  reason: string
  createdAt: string
}>

export type ArtanisTickMonitor = Readonly<{
  kind: 'artanis_admin_tick_monitor'
  publicSafe: true
  authorityBoundary: string
  closedTickReceiptRefs: ReadonlyArray<string>
  closedTickReceipts: ReadonlyArray<ArtanisClosedTickReceipt>
  closedTickStaleness: PublicProjectionStalenessContract
  dailyDispatchBound: number
  dispatchedToday: number
  countsByState: Readonly<Record<string, number>>
  decisions: ReadonlyArray<ArtanisTickMonitorEntry>
  generatedAt: string
  notes: ReadonlyArray<string>
  unattendedTickStreak: ArtanisUnattendedTickStreak
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

const closedTickReceiptRef = (assignmentRef: string): string =>
  `receipt.public.artanis.tetrahedron_closed_tick.${assignmentRef}`

const verdictRef = (outcome: string | null): string | null =>
  outcome === null ? null : `verdict.artanis_closeout.${outcome}`

const safeRefs = (
  scope: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => publicScannerSafeRefs(scope, refs)

const maybeClosedTickReceipt = (
  row: ArtanisTickDecisionRow,
): ArtanisClosedTickReceipt | null => {
  const record = artanisAdminCloseoutReceiptRecordFromRow(
    row as Record<string, unknown>,
  )
  const decisionState = record.decisionState ?? String(row.state ?? '')

  if (
    record.assignmentRef === '' ||
    record.pylonRef === '' ||
    decisionState !== 'dispatched' ||
    record.assignmentState !== 'accepted_work' ||
    record.verdictOutcome !== 'verified' ||
    record.verdictAcceptState !== 'accepted'
  ) {
    return null
  }

  const replayVerdictRef = verdictRef(record.verdictOutcome)
  if (replayVerdictRef === null) {
    return null
  }

  const closeoutReceiptRef = artanisAdminCloseoutReceiptRef(record.assignmentRef)
  const expectationRefs =
    record.claimedTraceDigestPrefix === null
      ? []
      : [
          `expectation.tassadar_poc.trace_digest.${record.claimedTraceDigestPrefix}`,
        ]

  return {
    assignmentRef: record.assignmentRef,
    caveatRefs: [
      'caveat.public.artanis.closed_tick_not_payout_settlement',
      'caveat.public.artanis.closed_tick_not_model_capability',
      'caveat.public.artanis.closed_tick_omits_private_runner_logs',
    ],
    closureContractVersion: TASSADAR_TICK_CLOSURE_CONTRACT_VERSION,
    closeoutReceiptRef,
    faceRefs: {
      evaluationRefs: safeRefs('evaluation.public.artanis.closed_tick', [
        replayVerdictRef,
        ...expectationRefs,
      ]),
      executionRefs: safeRefs('execution.public.artanis.closed_tick', [
        ...record.artifactRefs,
        ...record.closeoutRefs,
      ]),
      intentRefs: safeRefs('intent.public.artanis.closed_tick', [
        record.assignmentRef,
        `tick_decision.${String(row.id ?? 'unknown')}`,
      ]),
      stateDeltaRefs: safeRefs('state_delta.public.artanis.closed_tick', [
        ...record.acceptedWorkRefs,
        closeoutReceiptRef,
      ]),
    },
    pylonRef: record.pylonRef,
    provenanceLabel:
      'Tetrahedron-closed Artanis executor tick: persisted dispatch intent, Pylon closeout execution refs, accepted-work state delta, and verified exact-replay evaluation are all present. This is operational evidence only; it is not payout settlement or trained-model capability.',
    receiptKind: 'artanis_tetrahedron_closed_tick',
    receiptRef: closedTickReceiptRef(record.assignmentRef),
    verdictOutcome: 'verified',
    verdictRef: replayVerdictRef,
  }
}

const closureStateForRow = (
  row: ArtanisTickDecisionRow,
  receipt: ArtanisClosedTickReceipt | null,
): ArtanisTickMonitorEntry['closureState'] => {
  if (receipt !== null) {
    return 'closed_verified'
  }

  if (row.assignment_ref === null || row.assignment_ref === undefined) {
    return 'open_no_assignment'
  }

  const record = artanisAdminCloseoutReceiptRecordFromRow(
    row as Record<string, unknown>,
  )

  if (record.closeoutRefs.length === 0 && record.artifactRefs.length === 0) {
    return 'open_no_closeout'
  }

  return 'open_no_verified_replay'
}

const streakStaleness = (): PublicProjectionStalenessContract =>
  liveAtReadStaleness([
    'artanis_admin_tick_decision_recorded',
    'pylon_assignment_closeout_submitted',
    'artanis_closeout_verdict_recorded',
  ])

const unattendedStreakReceiptRef = (
  entries: ReadonlyArray<ArtanisTickMonitorEntry>,
): string | null => {
  const latestDecisionRef = entries[0]?.decisionRef
  if (latestDecisionRef === undefined) return null
  const decisionFragment =
    safeRefs('receipt.public.artanis.unattended_tick_streak.decision', [
      latestDecisionRef.replace(/^tick_decision\./, ''),
    ])[0] ?? 'unknown'
  return `receipt.public.artanis.unattended_tick_streak.${decisionFragment}.x${entries.length}`
}

const longestClosedRun = (
  decisions: ReadonlyArray<ArtanisTickMonitorEntry>,
): ReadonlyArray<ArtanisTickMonitorEntry> => {
  let currentRun: ArtanisTickMonitorEntry[] = []
  let longestRun: ArtanisTickMonitorEntry[] = []

  for (const decision of decisions) {
    if (decision.closureState === 'closed_verified') {
      currentRun = [...currentRun, decision]
      if (currentRun.length > longestRun.length) {
        longestRun = currentRun
      }
      continue
    }
    currentRun = []
  }

  return longestRun
}

const currentClosedRunLength = (
  decisions: ReadonlyArray<ArtanisTickMonitorEntry>,
): number => {
  let length = 0
  for (const decision of decisions) {
    if (decision.closureState !== 'closed_verified') break
    length += 1
  }
  return length
}

const projectUnattendedTickStreak = (
  decisions: ReadonlyArray<ArtanisTickMonitorEntry>,
): ArtanisUnattendedTickStreak => {
  const longestRun = longestClosedRun(decisions)
  const satisfied = longestRun.length >= 10
  const receiptRef = satisfied ? unattendedStreakReceiptRef(longestRun) : null
  return {
    authorityBoundary:
      'Read-only consecutive closed-tick projection. Grants no dispatch, spend, assignment, publication, promise-transition, model-capability, payout, or settlement authority.',
    blockerRefs: satisfied
      ? []
      : ['blocker.product_promises.artanis_unattended_tick_streak_missing'],
    closedTickReceiptRefs: safeRefs(
      'receipt.public.artanis.unattended_tick_streak.closed_ticks',
      longestRun.flatMap(decision =>
        decision.closedTickReceiptRef === null
          ? []
          : [decision.closedTickReceiptRef],
      ),
    ),
    currentConsecutiveClosedTicks: currentClosedRunLength(decisions),
    decisionRefs: safeRefs(
      'receipt.public.artanis.unattended_tick_streak.decisions',
      longestRun.map(decision => decision.decisionRef),
    ),
    kind: 'artanis_unattended_tick_streak',
    longestConsecutiveClosedTicks: longestRun.length,
    provenanceLabel:
      'Unattended Artanis tick-streak projection derived only from persisted dispatch decisions that have accepted Pylon closeouts and accepted exact-replay verdicts. A receipt is emitted only after ten consecutive closed ticks are visible in this public-safe read model.',
    receiptRef,
    requiredConsecutiveClosedTicks: 10,
    satisfied,
    staleness: streakStaleness(),
  }
}

export const projectArtanisTickMonitor = (
  rows: ReadonlyArray<ArtanisTickDecisionRow>,
  nowIso: string,
): ArtanisTickMonitor => {
  const decisions: ArtanisTickMonitorEntry[] = []
  const closedTickReceipts: ArtanisClosedTickReceipt[] = []
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
    const closedTickReceipt = maybeClosedTickReceipt(row)
    if (closedTickReceipt !== null) {
      closedTickReceipts.push(closedTickReceipt)
    }
    decisions.push({
      assignmentRef:
        row.assignment_ref === null || row.assignment_ref === undefined
          ? null
          : String(row.assignment_ref),
      closedTickReceiptRef: closedTickReceipt?.receiptRef ?? null,
      closeoutReceiptRef:
        row.assignment_ref === null || row.assignment_ref === undefined
          ? null
          : artanisAdminCloseoutReceiptRef(String(row.assignment_ref)),
      closureState: closureStateForRow(row, closedTickReceipt),
      createdAt,
      decisionRef: `tick_decision.${String(row.id ?? 'unknown')}`,
      reason: sanitizeReason(row.action_json),
      state: state as ArtanisTickMonitorEntry['state'],
    })
  }

  return {
    authorityBoundary:
      'Read-only tick ledger projection. Grants no dispatch, spend, assignment, settlement, or registry authority; reasons are truncated and redaction-scanned; raw mind output is never projected.',
    closedTickReceiptRefs: closedTickReceipts.map(receipt => receipt.receiptRef),
    closedTickReceipts,
    closedTickStaleness: liveAtReadStaleness([
      'artanis_admin_tick_decision_recorded',
      'pylon_assignment_closeout_submitted',
      'artanis_closeout_verdict_recorded',
    ]),
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
    unattendedTickStreak: projectUnattendedTickStreak(decisions),
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
      `SELECT d.id,
              d.state,
              d.action_json,
              d.assignment_ref,
              d.created_at,
              a.pylon_ref,
              a.job_kind,
              a.state AS assignment_state,
              a.accepted_work_refs_json,
              a.artifact_refs_json,
              a.proof_refs_json,
              a.closeout_refs_json,
              a.created_at AS assignment_created_at,
              a.updated_at AS assignment_updated_at,
              d.id AS decision_id,
              d.state AS decision_state,
              d.created_at AS decision_created_at,
              v.outcome AS verdict_outcome,
              v.claimed_trace_digest_prefix AS verdict_trace_digest_prefix,
              v.accept_state AS verdict_accept_state,
              v.created_at AS verdict_created_at
         FROM artanis_admin_tick_decisions d
    LEFT JOIN pylon_api_assignments a
           ON a.assignment_ref = d.assignment_ref
          AND a.archived_at IS NULL
    LEFT JOIN artanis_closeout_verdicts v
           ON v.assignment_ref = d.assignment_ref
        ORDER BY d.created_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all()

  return projectArtanisTickMonitor(
    (result.results ?? []) as unknown as ReadonlyArray<ArtanisTickDecisionRow>,
    input.nowIso,
  )
}
