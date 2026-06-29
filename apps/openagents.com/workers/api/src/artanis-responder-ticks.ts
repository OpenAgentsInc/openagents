import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const ARTANIS_RESPONDER_TICK_TARGET = 10
export const ARTANIS_RESPONDER_UNATTENDED_TICKS_BLOCKER =
  'blocker.product_promises.ten_unattended_responder_ticks_unaccrued'

export const ARTANIS_RESPONDER_TICK_STALENESS: PublicProjectionStalenessContract =
  liveAtReadStaleness([
    'artanis_responder_ticks.insert',
    'artanis_responder_ticks.update',
    'artanis_responder_actions.update',
  ])

export type ArtanisResponderScanTickOutcome = Readonly<{
  scanned: number
  proposed: number
  blocked: number
  skipped: number
  skippedReason: string | null
}>

export type ArtanisResponderComposeTickOutcome = Readonly<{
  considered: number
  responded: number
  blocked: number
  tipped: number
  skippedReason: string | null
}>

export type ArtanisResponderTickRow = Readonly<{
  tick_ref: unknown
  scheduled_at: unknown
  scan_state: unknown
  scan_scanned: unknown
  scan_proposed: unknown
  scan_blocked: unknown
  scan_skipped: unknown
  scan_skipped_reason: unknown
  compose_state: unknown
  compose_considered: unknown
  compose_responded: unknown
  compose_blocked: unknown
  compose_tipped: unknown
  compose_skipped_reason: unknown
}>

export type ArtanisResponderTickActionRow = Readonly<{
  id: unknown
  topic_id: unknown
  asker_provenance: unknown
  reply_post_id: unknown
  replied_at: unknown
  state: unknown
}>

export type ArtanisResponderTickWindow = Readonly<{
  tickRef: string
  scheduledAt: string
  nextScheduledAt: string | null
  scanScanned: number
  scanProposed: number
  composeResponded: number
  composeTipped: number
  qualifiesUnattendedResponderTick: boolean
  externalContributorAnsweredInWindow: boolean
  replyPostRefs: ReadonlyArray<string>
}>

export type ArtanisResponderTickReadinessProjection = Readonly<{
  kind: 'artanis_pylon_support_responder_tick_readiness'
  publicSafe: true
  authorityBoundary: string
  staleness: PublicProjectionStalenessContract
  blockerRefs: ReadonlyArray<string>
  tickTarget: number
  qualifyingUnattendedResponderTickCount: number
  unattendedResponderTicksProven: boolean
  externalContributorAnsweredWithinTickWindow: boolean
  tickWindows: ReadonlyArray<ArtanisResponderTickWindow>
  notes: ReadonlyArray<string>
}>

const safeRefPattern = /^[A-Za-z0-9._:-]{1,200}$/

const safeRef = (value: unknown): string | null => {
  const text = value === null || value === undefined ? '' : String(value)
  return text !== '' && safeRefPattern.test(text) ? text : null
}

const safeInt = (value: unknown): number => {
  const parsed = Number(value ?? 0)
  return Number.isFinite(parsed) ? Math.max(0, Math.trunc(parsed)) : 0
}

export const artanisResponderTickRef = (scheduledAt: string): string =>
  `receipt.artanis_responder.tick.${scheduledAt
    .replace(/[^0-9A-Za-z]/g, '')
    .slice(0, 32)}`

const outcomeState = (
  skippedReason: string | null,
): 'ran' | 'skipped' | 'error' => {
  if (skippedReason === null) {
    return 'ran'
  }
  return skippedReason.includes('error') ? 'error' : 'skipped'
}

export const recordArtanisResponderScanTick = async (
  db: D1Database,
  input: Readonly<{ nowIso: string; outcome: ArtanisResponderScanTickOutcome }>,
): Promise<void> => {
  const tickRef = artanisResponderTickRef(input.nowIso)
  await db
    .prepare(
      `INSERT INTO artanis_responder_ticks
       (tick_ref, scheduled_at, scan_state, scan_scanned, scan_proposed,
        scan_blocked, scan_skipped, scan_skipped_reason, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(scheduled_at) DO UPDATE SET
         scan_state = excluded.scan_state,
         scan_scanned = excluded.scan_scanned,
         scan_proposed = excluded.scan_proposed,
         scan_blocked = excluded.scan_blocked,
         scan_skipped = excluded.scan_skipped,
         scan_skipped_reason = excluded.scan_skipped_reason,
         updated_at = excluded.updated_at`,
    )
    .bind(
      tickRef,
      input.nowIso,
      outcomeState(input.outcome.skippedReason),
      input.outcome.scanned,
      input.outcome.proposed,
      input.outcome.blocked,
      input.outcome.skipped,
      input.outcome.skippedReason,
      input.nowIso,
      input.nowIso,
    )
    .run()
}

export const recordArtanisResponderComposeTick = async (
  db: D1Database,
  input: Readonly<{
    nowIso: string
    outcome: ArtanisResponderComposeTickOutcome
  }>,
): Promise<void> => {
  const tickRef = artanisResponderTickRef(input.nowIso)
  await db
    .prepare(
      `INSERT INTO artanis_responder_ticks
       (tick_ref, scheduled_at, compose_state, compose_considered,
        compose_responded, compose_blocked, compose_tipped,
        compose_skipped_reason, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(scheduled_at) DO UPDATE SET
         compose_state = excluded.compose_state,
         compose_considered = excluded.compose_considered,
         compose_responded = excluded.compose_responded,
         compose_blocked = excluded.compose_blocked,
         compose_tipped = excluded.compose_tipped,
         compose_skipped_reason = excluded.compose_skipped_reason,
         updated_at = excluded.updated_at`,
    )
    .bind(
      tickRef,
      input.nowIso,
      outcomeState(input.outcome.skippedReason),
      input.outcome.considered,
      input.outcome.responded,
      input.outcome.blocked,
      input.outcome.tipped,
      input.outcome.skippedReason,
      input.nowIso,
      input.nowIso,
    )
    .run()
}

export const projectArtanisResponderTickReadiness = (
  tickRows: ReadonlyArray<ArtanisResponderTickRow>,
  actionRows: ReadonlyArray<ArtanisResponderTickActionRow>,
): ArtanisResponderTickReadinessProjection => {
  const orderedTicks = tickRows
    .map(row => {
      const scheduledAt = String(row.scheduled_at ?? '')
      return {
        composeResponded: safeInt(row.compose_responded),
        composeState: String(row.compose_state ?? ''),
        composeTipped: safeInt(row.compose_tipped),
        scheduledAt,
        scanProposed: safeInt(row.scan_proposed),
        scanScanned: safeInt(row.scan_scanned),
        scanState: String(row.scan_state ?? ''),
        tickRef: safeRef(row.tick_ref) ?? artanisResponderTickRef(scheduledAt),
      }
    })
    .filter(row => row.scheduledAt !== '')
    .sort((a, b) => a.scheduledAt.localeCompare(b.scheduledAt))

  const answeredActions = actionRows
    .map(row => ({
      askerProvenance: String(row.asker_provenance ?? ''),
      repliedAt: String(row.replied_at ?? ''),
      replyPostRef: safeRef(row.reply_post_id),
      state: String(row.state ?? ''),
    }))
    .filter(
      row =>
        (row.state === 'responded' || row.state === 'tipped') &&
        row.repliedAt !== '' &&
        row.replyPostRef !== null,
    )

  const tickWindows = orderedTicks.map((tick, index) => {
    const nextScheduledAt = orderedTicks[index + 1]?.scheduledAt ?? null
    const actionsInWindow = answeredActions.filter(
      action =>
        action.repliedAt >= tick.scheduledAt &&
        (nextScheduledAt === null || action.repliedAt < nextScheduledAt),
    )
    const replyPostRefs = actionsInWindow
      .map(action => action.replyPostRef)
      .filter((value): value is string => value !== null)
    const externalContributorAnsweredInWindow = actionsInWindow.some(
      action => action.askerProvenance === 'external_contributor',
    )
    const qualifiesUnattendedResponderTick =
      tick.scanState === 'ran' &&
      tick.composeState === 'ran' &&
      tick.scanScanned > 0 &&
      tick.scanProposed > 0 &&
      tick.composeResponded > 0 &&
      replyPostRefs.length > 0

    return {
      composeResponded: tick.composeResponded,
      composeTipped: tick.composeTipped,
      externalContributorAnsweredInWindow,
      nextScheduledAt,
      qualifiesUnattendedResponderTick,
      replyPostRefs,
      scanProposed: tick.scanProposed,
      scanScanned: tick.scanScanned,
      scheduledAt: tick.scheduledAt,
      tickRef: tick.tickRef,
    } satisfies ArtanisResponderTickWindow
  })

  const qualifyingUnattendedResponderTickCount = tickWindows.filter(
    tick => tick.qualifiesUnattendedResponderTick,
  ).length

  return {
    authorityBoundary:
      'Read-only projection over Artanis responder scheduled tick receipts plus answered responder-action refs. Grants no dispatch, spend, assignment, settlement, moderation, Forum-write, or registry-transition authority and cannot create a tick, classify a question, post a reply, or tip.',
    blockerRefs: [ARTANIS_RESPONDER_UNATTENDED_TICKS_BLOCKER],
    externalContributorAnsweredWithinTickWindow: tickWindows.some(
      tick => tick.externalContributorAnsweredInWindow,
    ),
    kind: 'artanis_pylon_support_responder_tick_readiness',
    notes: [
      'A qualifying unattended responder tick requires the scheduled scan and compose stages to have run, at least one candidate scanned and proposed, at least one reply posted, and a dereferenceable reply-post ref inside that tick window.',
      'The ten-tick target is a mechanical evidence predicate for blocker tracking only; the promise state remains owner-signed and receipt-first.',
      'externalContributorAnsweredWithinTickWindow is true only when an answered action in a scheduled tick window carries asker_provenance external_contributor.',
    ],
    publicSafe: true,
    qualifyingUnattendedResponderTickCount,
    staleness: ARTANIS_RESPONDER_TICK_STALENESS,
    tickTarget: ARTANIS_RESPONDER_TICK_TARGET,
    tickWindows: tickWindows.slice(-ARTANIS_RESPONDER_TICK_TARGET).reverse(),
    unattendedResponderTicksProven:
      qualifyingUnattendedResponderTickCount >= ARTANIS_RESPONDER_TICK_TARGET,
  }
}

export const readArtanisResponderTickReadiness = async (
  db: D1Database,
  input: Readonly<{ limit: number }>,
): Promise<ArtanisResponderTickReadinessProjection> => {
  const limit = Math.max(1, Math.min(200, Math.trunc(input.limit)))
  const tickResult = await db
    .prepare(
      `SELECT tick_ref,
              scheduled_at,
              scan_state,
              scan_scanned,
              scan_proposed,
              scan_blocked,
              scan_skipped,
              scan_skipped_reason,
              compose_state,
              compose_considered,
              compose_responded,
              compose_blocked,
              compose_tipped,
              compose_skipped_reason
         FROM artanis_responder_ticks
        ORDER BY scheduled_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all()

  const actionResult = await db
    .prepare(
      `SELECT id, topic_id, asker_provenance, reply_post_id, replied_at, state
         FROM artanis_responder_actions
        WHERE state IN ('responded', 'tipped')
          AND reply_post_id IS NOT NULL
        ORDER BY replied_at DESC
        LIMIT ?`,
    )
    .bind(Math.max(1, Math.min(500, Math.trunc(limit * 5))))
    .all()

  return projectArtanisResponderTickReadiness(
    (tickResult.results ?? []) as unknown as ReadonlyArray<ArtanisResponderTickRow>,
    (actionResult.results ?? []) as unknown as ReadonlyArray<ArtanisResponderTickActionRow>,
  )
}
