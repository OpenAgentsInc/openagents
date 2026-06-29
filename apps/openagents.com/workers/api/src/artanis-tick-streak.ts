// Public Artanis unattended tick-streak projection. This is the missing
// piece for the artanis_unattended_tick_streak_missing blocker on
// artanis.tassadar_evolution_loop.v1: the registry verification text
// requires "at least ten consecutive unattended ticks whose receipts
// include executor dispatch and exact-replay verdicts". The tick monitor
// (artanis-tick-monitor.ts) projects individual decisions and counts by
// state, but nothing computes the CONSECUTIVE streak of verified
// dispatch-execute-verify ticks. This module is that counter/projection.
//
// A "qualifying" tick is a `dispatched` admin-tick decision whose
// admin-tick assignment carries an exact-replay closeout verdict with
// outcome = 'verified' (the byte-identical replay accepted the closeout).
// The current streak is the run of consecutive qualifying ticks counted
// backwards from the most recent tick decision; it BREAKS the moment a
// tick decision is encountered that is not a verified dispatch (a
// no_action, blocked, dispatch_failed, or a dispatched tick whose
// closeout did not verify / is not yet verified). The longest streak is
// the maximum such run anywhere in the projected window.
//
// This module is projection-only: it joins the tick-decision ledger and
// the closeout verdict ledger that the runner already persists. It grants
// no dispatch, spend, assignment, settlement, or registry authority, and
// it never fabricates a tick or a verdict - an unverified or pending tick
// can only ever shorten the streak, never lengthen it. The projection
// exposes structure, not free text, so no raw mind output can leak here.

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

// Staleness contract (epic #4751). This projection is composed live from
// the tick-decision and closeout-verdict tables at read time, so it can
// never be older than the request: live_at_read, maxStalenessSeconds 0.
// rebuildsOn names the write transitions whose rows it reads, so the
// invalidation set is explicit.
export const ARTANIS_TICK_STREAK_STALENESS: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'artanis_admin_tick_decisions.insert',
    'artanis_closeout_verdicts.insert',
    'artanis_closeout_verdicts.update',
  ])

// The streak length the registry verification gate calls for before the
// unattended-tick-streak dimension of artanis.tassadar_evolution_loop.v1
// can clear. Code-anchored so the projection and any future verifier read
// the same threshold.
export const ARTANIS_UNATTENDED_TICK_STREAK_TARGET = 10

export type ArtanisTickStreakDecisionRow = Readonly<{
  id: unknown
  state: unknown
  assignment_ref: unknown
  created_at: unknown
  // LEFT JOINed from artanis_closeout_verdicts on assignment_ref.
  verdict_outcome: unknown
  verdict_accept_state: unknown
  verdict_trace_digest_prefix: unknown
  verdict_created_at: unknown
}>

export type ArtanisTickStreakEntry = Readonly<{
  decisionRef: string
  qualifies: boolean
  state: 'dispatched' | 'no_action' | 'blocked' | 'dispatch_failed'
  assignmentRef: string | null
  verdictOutcome: 'verified' | 'rejected' | 'unreadable' | null
  verdictAcceptState: string | null
  traceDigestPrefix: string | null
  closeoutReceiptRef: string | null
  createdAt: string
}>

export type ArtanisTickStreak = Readonly<{
  kind: 'artanis_unattended_tick_streak'
  publicSafe: true
  authorityBoundary: string
  // Staleness contract (epic #4751): live_at_read, never older than the request.
  staleness: PublicProjectionStalenessContract
  // The streak length the gate requires before the streak dimension clears.
  streakTarget: number
  // Consecutive qualifying ticks counted backwards from the latest tick.
  currentStreak: number
  // Longest consecutive qualifying run anywhere in the projected window.
  longestStreak: number
  // True once longestStreak reaches the target on dereferenceable receipts.
  targetReached: boolean
  // Total qualifying ticks projected (not necessarily consecutive).
  verifiedTickCount: number
  // The ordered window the streak was computed over (newest first).
  ticks: ReadonlyArray<ArtanisTickStreakEntry>
  // The assignment refs of the current streak, newest first, so a reader
  // can dereference each closeout receipt and re-verify the replay verdict.
  currentStreakAssignmentRefs: ReadonlyArray<string>
  generatedAt: string
  notes: ReadonlyArray<string>
}>

const VALID_STATES = new Set([
  'blocked',
  'dispatch_failed',
  'dispatched',
  'no_action',
])

const VALID_OUTCOMES = new Set(['verified', 'rejected', 'unreadable'])

// A digest prefix is the only verdict-derived string projected; scan it so
// the public surface can never leak an unexpected value smuggled into the
// column.
const safeDigestPrefixPattern = /^[a-f0-9]{1,32}$/

const closeoutReceiptRef = (assignmentRef: string): string =>
  `receipt.nexus_pylon.artanis_admin_closeout.${assignmentRef}`

const normalizeOutcome = (
  value: unknown,
): 'verified' | 'rejected' | 'unreadable' | null => {
  const text = String(value ?? '')
  return VALID_OUTCOMES.has(text)
    ? (text as 'verified' | 'rejected' | 'unreadable')
    : null
}

// A tick qualifies for the streak iff it is a dispatched decision with a
// real admin-tick assignment ref, and that assignment's exact-replay
// closeout verdict both VERIFIED and was ACCEPTED. Nothing else counts.
const tickQualifies = (entry: ArtanisTickStreakEntry): boolean =>
  entry.state === 'dispatched' &&
  entry.assignmentRef !== null &&
  entry.verdictOutcome === 'verified' &&
  entry.verdictAcceptState === 'accepted'

export const projectArtanisTickStreak = (
  rows: ReadonlyArray<ArtanisTickStreakDecisionRow>,
  nowIso: string,
): ArtanisTickStreak => {
  const ticks: ArtanisTickStreakEntry[] = []

  for (const row of rows) {
    const state = String(row.state ?? '')
    if (!VALID_STATES.has(state)) continue
    const assignmentRef =
      row.assignment_ref === null || row.assignment_ref === undefined
        ? null
        : String(row.assignment_ref)
    const rawPrefix =
      row.verdict_trace_digest_prefix === null ||
      row.verdict_trace_digest_prefix === undefined
        ? null
        : String(row.verdict_trace_digest_prefix)
    const traceDigestPrefix =
      rawPrefix !== null && safeDigestPrefixPattern.test(rawPrefix)
        ? rawPrefix
        : null
    const verdictAcceptStateRaw =
      row.verdict_accept_state === null ||
      row.verdict_accept_state === undefined
        ? null
        : String(row.verdict_accept_state)

    const entry: ArtanisTickStreakEntry = {
      assignmentRef,
      closeoutReceiptRef:
        state === 'dispatched' && assignmentRef !== null
          ? closeoutReceiptRef(assignmentRef)
          : null,
      createdAt: String(row.created_at ?? ''),
      decisionRef: `tick_decision.${String(row.id ?? 'unknown')}`,
      qualifies: false,
      state: state as ArtanisTickStreakEntry['state'],
      traceDigestPrefix,
      verdictAcceptState: verdictAcceptStateRaw,
      verdictOutcome: normalizeOutcome(row.verdict_outcome),
    }
    ticks.push({ ...entry, qualifies: tickQualifies(entry) })
  }

  // Rows arrive newest-first (ORDER BY created_at DESC). The current streak
  // is the run of qualifying ticks from the head; the longest streak is the
  // max run anywhere.
  let currentStreak = 0
  let currentDone = false
  let longestStreak = 0
  let run = 0
  let verifiedTickCount = 0
  const currentStreakAssignmentRefs: string[] = []

  for (const tick of ticks) {
    if (tick.qualifies) {
      verifiedTickCount += 1
      run += 1
      if (run > longestStreak) longestStreak = run
      if (!currentDone) {
        currentStreak += 1
        if (tick.assignmentRef !== null) {
          currentStreakAssignmentRefs.push(tick.assignmentRef)
        }
      }
    } else {
      run = 0
      currentDone = true
    }
  }

  const targetReached = longestStreak >= ARTANIS_UNATTENDED_TICK_STREAK_TARGET

  return {
    authorityBoundary:
      'Read-only streak projection over the tick-decision and exact-replay closeout-verdict ledgers. Grants no dispatch, spend, assignment, settlement, or registry authority. A tick only counts when it is a dispatched decision whose admin-tick assignment carries an accepted exact-replay verdict; pending or unverified ticks can only shorten the streak, never lengthen it. The projection cannot create a tick or a verdict.',
    currentStreak,
    currentStreakAssignmentRefs,
    generatedAt: nowIso,
    kind: 'artanis_unattended_tick_streak',
    longestStreak,
    staleness: ARTANIS_TICK_STREAK_STALENESS,
    notes: [
      `A qualifying tick is a dispatched admin-tick decision whose assignment carries an accepted exact-replay closeout verdict (outcome=verified, accept_state=accepted). The gate requires ${ARTANIS_UNATTENDED_TICK_STREAK_TARGET} consecutive qualifying ticks.`,
      'Each currentStreakAssignmentRefs entry is dereferenceable at /api/public/nexus-pylon/receipts/receipt.nexus_pylon.artanis_admin_closeout.<assignmentRef> for independent replay-verdict inspection.',
      'Pre-mind skips (runner disabled, mind unconfigured, daily bound, no eligible Pylons) are not persisted decisions and never appear here; a dispatched tick whose closeout is not yet verified breaks the current streak until its verdict lands.',
    ],
    publicSafe: true,
    streakTarget: ARTANIS_UNATTENDED_TICK_STREAK_TARGET,
    targetReached,
    ticks,
    verifiedTickCount,
  }
}

export const ARTANIS_TICK_STREAK_MAX_LIMIT = 200

export const boundedTickStreakLimit = (raw: string | null): number => {
  const parsed = Number(raw ?? '100')
  if (!Number.isFinite(parsed)) return 100
  return Math.min(
    Math.max(1, Math.trunc(parsed)),
    ARTANIS_TICK_STREAK_MAX_LIMIT,
  )
}

export const readArtanisTickStreak = async (
  db: D1Database,
  input: Readonly<{ limit: number; nowIso: string }>,
): Promise<ArtanisTickStreak> => {
  const limit = boundedTickStreakLimit(String(input.limit))
  const result = await db
    .prepare(
      `SELECT d.id AS id,
              d.state AS state,
              d.assignment_ref AS assignment_ref,
              d.created_at AS created_at,
              v.outcome AS verdict_outcome,
              v.accept_state AS verdict_accept_state,
              v.claimed_trace_digest_prefix AS verdict_trace_digest_prefix,
              v.created_at AS verdict_created_at
         FROM artanis_admin_tick_decisions d
    LEFT JOIN artanis_closeout_verdicts v
           ON v.assignment_ref = d.assignment_ref
        ORDER BY d.created_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all()

  return projectArtanisTickStreak(
    (result.results ?? []) as unknown as ReadonlyArray<ArtanisTickStreakDecisionRow>,
    input.nowIso,
  )
}

// Kept exported for any future server-side verifier that wants to assert
// the gate predicate without re-parsing the projection JSON.
export const artanisTickStreakMeetsGate = (
  streak: ArtanisTickStreak,
): boolean => streak.targetReached
