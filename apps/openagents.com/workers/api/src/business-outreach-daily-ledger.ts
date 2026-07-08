// OB-6 (P1 Track C, #8563): the daily sales ledger — funnel + deliverability
// metrics gating the ramp.
//
// SCOPE (read the issue before touching this file): this module is the
// METRICS SURFACE only. It aggregates exact rows from real event sources
// that OB-2/OB-3/OB-4's already-shipped machinery writes:
//
//   - `business_pipeline_rows`      (business-pipeline-queue.ts)   -> sourced, quotes, closes
//   - `business_outreach_drafts`    (business-outreach.ts)         -> drafted
//   - `business_outreach_template_approvals` (business-outreach.ts) -> approved
//   - `business_outreach_sends`     (business-outreach.ts)         -> sent
//   - `email_provider_events`       (resend-webhooks.ts)           -> delivered/bounced/complained
//   - `email_suppression_entries`   (0063_email_campaign_records.sql) -> opt-outs
//
// It does NOT create a new raw event table, and it does NOT enforce
// anything: no send is ever blocked from here. OB-1 (#8558) owns the
// warm-up ramp config and the actual cap-freeze decision once it ships;
// the `health` label this module computes is informational only, sized
// off common ESP-safety literature, and clearly marked as such.
//
// HONESTY DISCIPLINE (same convention as admin-ops-routes.ts): every
// number here is either a real read from an exact source table, or an
// explicit `not_measured` sentinel with a `reasonRef` — never a fabricated
// or guessed value. As of this writing OB-1/OB-4/OB-5 (Sarah's sending
// identity, the batch-approval/reply loop, and the Stripe-close
// conversation surface) are still open, so `replies`, `reportClicks`, and
// `conversations` have no wired source yet and are reported as
// `not_measured` rather than silently zeroed.
//
// KNOWN LIMITATIONS (see the OB-6 closing note for the full write-up):
//  - `quotes`/`closes` read `business_pipeline_rows` as a point-in-time
//    snapshot keyed on `stage_updated_at` (the last time ANY column on the
//    row changed while it happened to be in that stage), not a true
//    stage-transition event log. For a terminal stage (`closed_won`,
//    `closed_lost`) this is normally accurate; a row that is re-touched
//    after closing (e.g. metadata backfill) would shift the day it counts
//    on. A real append-only stage-history table would remove this caveat.
//  - deliverability (`delivered`/`bounced`/`complained`) is READ GLOBALLY
//    per day, not per segment: `email_provider_events` has no
//    pipeline/segment linkage today (Resend gives us `email` + provider
//    message ids, not our internal refs), and outbound sends made through
//    Apollo's own sending infrastructure (`channel: 'apollo_sequence'`)
//    never touch Resend at all. Once OB-1's dedicated sending subdomain
//    routes outbound mail through Resend with message-id linkage back to
//    `business_outreach_sends`, this can become exact and segment-scoped.

import {
  BUSINESS_OUTREACH_TEMPLATE_VERSIONS,
  segmentFromVertical,
  type BusinessOutreachSegmentRef,
} from './business-outreach'

export const DAILY_SALES_LEDGER_SEGMENT_REFS: ReadonlyArray<BusinessOutreachSegmentRef> =
  [...new Set(BUSINESS_OUTREACH_TEMPLATE_VERSIONS.map(template => template.segmentRef))]

const segmentRefForTemplateVersion = new Map<string, BusinessOutreachSegmentRef>(
  BUSINESS_OUTREACH_TEMPLATE_VERSIONS.map(template => [
    template.templateVersionRef,
    template.segmentRef,
  ]),
)

// A same-key identity map is a cast-free way to narrow the raw `segment_ref`
// TEXT column read back from `business_outreach_drafts` into the typed
// `BusinessOutreachSegmentRef` union (`.get` returns `undefined` for any
// value outside the known set, so an unrecognized value is skipped rather
// than silently miscounted).
const knownSegmentRef = new Map<string, BusinessOutreachSegmentRef>(
  DAILY_SALES_LEDGER_SEGMENT_REFS.map(segmentRef => [segmentRef, segmentRef]),
)

/** Informational-only display thresholds (NOT enforcement — see header). */
export const DAILY_SALES_LEDGER_BOUNCE_RATE_AT_RISK_PCT = 2
export const DAILY_SALES_LEDGER_BOUNCE_RATE_BREACH_PCT = 5
export const DAILY_SALES_LEDGER_COMPLAINT_RATE_AT_RISK_PCT = 0.05
export const DAILY_SALES_LEDGER_COMPLAINT_RATE_BREACH_PCT = 0.1

export const DAILY_SALES_LEDGER_MAX_WINDOW_DAYS = 92

export type NotMeasured = Readonly<{ status: 'not_measured'; reasonRef: string }>

const NOT_MEASURED_REPLIES: NotMeasured = {
  reasonRef: 'reason.ob6.reply_capture_pending_ob4',
  status: 'not_measured',
}
const NOT_MEASURED_REPORT_CLICKS: NotMeasured = {
  reasonRef: 'reason.ob6.report_click_tracking_pending_ob3',
  status: 'not_measured',
}
const NOT_MEASURED_CONVERSATIONS: NotMeasured = {
  reasonRef: 'reason.ob6.conversation_tracking_pending_ob5',
  status: 'not_measured',
}

export type DailySalesLedgerSegmentDay = Readonly<{
  date: string
  segmentRef: BusinessOutreachSegmentRef
  sourced: number
  drafted: number
  approved: number
  sent: number
  quoted: number
  closedWon: number
  closedLost: number
  replies: NotMeasured
  reportClicks: NotMeasured
  conversations: NotMeasured
}>

export type DailySalesLedgerDeliverabilityHealth =
  | 'healthy'
  | 'at_risk'
  | 'breach'
  | 'not_measured'

export type DailySalesLedgerRateMetric =
  | Readonly<{ status: 'measured'; valuePct: number }>
  | NotMeasured

export type DailySalesLedgerDeliverabilityDay = Readonly<{
  date: string
  delivered: number
  bounced: number
  complained: number
  failed: number
  optOuts: number
  bounceRatePct: DailySalesLedgerRateMetric
  complaintRatePct: DailySalesLedgerRateMetric
  health: DailySalesLedgerDeliverabilityHealth
}>

export type DailySalesLedgerTotals = Readonly<{
  sourced: number
  drafted: number
  approved: number
  sent: number
  delivered: number
  bounced: number
  complained: number
  optOuts: number
  quoted: number
  closedWon: number
  closedLost: number
}>

export type DailySalesLedger = Readonly<{
  since: string
  until: string
  generatedAt: string
  segmentRefs: ReadonlyArray<BusinessOutreachSegmentRef>
  segmentDays: ReadonlyArray<DailySalesLedgerSegmentDay>
  deliverabilityDays: ReadonlyArray<DailySalesLedgerDeliverabilityDay>
  totals: DailySalesLedgerTotals
  digestLine: string
  notMeasured: ReadonlyArray<
    Readonly<{ field: 'replies' | 'reportClicks' | 'conversations'; reasonRef: string }>
  >
}>

export class DailySalesLedgerValidationError extends Error {}

const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/

const assertDate = (field: string, value: string): void => {
  if (!DATE_PATTERN.test(value)) {
    throw new DailySalesLedgerValidationError(`${field} must be an ISO date (YYYY-MM-DD)`)
  }
}

/** Inclusive list of every YYYY-MM-DD date from `since` to `until`. */
export const dateRangeInclusive = (since: string, until: string): ReadonlyArray<string> => {
  assertDate('since', since)
  assertDate('until', until)
  if (since > until) {
    throw new DailySalesLedgerValidationError('since must not be after until')
  }
  const days: Array<string> = []
  let cursor = new Date(`${since}T00:00:00.000Z`)
  const end = new Date(`${until}T00:00:00.000Z`)
  while (cursor.getTime() <= end.getTime()) {
    days.push(cursor.toISOString().slice(0, 10))
    if (days.length > DAILY_SALES_LEDGER_MAX_WINDOW_DAYS) {
      throw new DailySalesLedgerValidationError(
        `window exceeds DAILY_SALES_LEDGER_MAX_WINDOW_DAYS (${DAILY_SALES_LEDGER_MAX_WINDOW_DAYS})`,
      )
    }
    cursor = new Date(cursor.getTime() + 24 * 60 * 60 * 1000)
  }
  return days
}

type CountByDaySegmentRow = Readonly<{ day: string; segment_key: string; count: number }>
type CountByDayRow = Readonly<{ day: string; count: number }>
type PipelineSnapshotRow = Readonly<{
  day: string
  vertical: string
  stage: string
  quoted_flag: number
  count: number
}>
type ProviderEventRow = Readonly<{ day: string; event_type: string; count: number }>

const emptySegmentDay = (
  date: string,
  segmentRef: BusinessOutreachSegmentRef,
): { -readonly [K in keyof DailySalesLedgerSegmentDay]: DailySalesLedgerSegmentDay[K] } => ({
  approved: 0,
  closedLost: 0,
  closedWon: 0,
  conversations: NOT_MEASURED_CONVERSATIONS,
  date,
  drafted: 0,
  quoted: 0,
  replies: NOT_MEASURED_REPLIES,
  reportClicks: NOT_MEASURED_REPORT_CLICKS,
  segmentRef,
  sent: 0,
  sourced: 0,
})

const rateMetric = (numerator: number, denominator: number): DailySalesLedgerRateMetric =>
  denominator === 0
    ? { reasonRef: 'reason.ob6.no_resolved_deliverability_outcomes_yet', status: 'not_measured' }
    : { status: 'measured', valuePct: Number(((numerator / denominator) * 100).toFixed(3)) }

const healthFromRates = (
  bounceRate: DailySalesLedgerRateMetric,
  complaintRate: DailySalesLedgerRateMetric,
): DailySalesLedgerDeliverabilityHealth => {
  if (bounceRate.status === 'not_measured' && complaintRate.status === 'not_measured') {
    return 'not_measured'
  }
  const bouncePct = bounceRate.status === 'measured' ? bounceRate.valuePct : 0
  const complaintPct = complaintRate.status === 'measured' ? complaintRate.valuePct : 0
  if (
    bouncePct > DAILY_SALES_LEDGER_BOUNCE_RATE_BREACH_PCT ||
    complaintPct > DAILY_SALES_LEDGER_COMPLAINT_RATE_BREACH_PCT
  ) {
    return 'breach'
  }
  if (
    bouncePct > DAILY_SALES_LEDGER_BOUNCE_RATE_AT_RISK_PCT ||
    complaintPct > DAILY_SALES_LEDGER_COMPLAINT_RATE_AT_RISK_PCT
  ) {
    return 'at_risk'
  }
  return 'healthy'
}

const renderDigestLine = (ledger: Omit<DailySalesLedger, 'digestLine'>): string => {
  const latestDate = ledger.deliverabilityDays.at(-1)?.date ?? ledger.until
  const latestSegmentDays = ledger.segmentDays.filter(row => row.date === latestDate)
  const latestDeliverability = ledger.deliverabilityDays.find(row => row.date === latestDate)
  const sourced = latestSegmentDays.reduce((sum, row) => sum + row.sourced, 0)
  const drafted = latestSegmentDays.reduce((sum, row) => sum + row.drafted, 0)
  const sent = latestSegmentDays.reduce((sum, row) => sum + row.sent, 0)
  const quoted = latestSegmentDays.reduce((sum, row) => sum + row.quoted, 0)
  const closes = latestSegmentDays.reduce(
    (sum, row) => sum + row.closedWon + row.closedLost,
    0,
  )
  const health = latestDeliverability?.health ?? 'not_measured'
  const delivered = latestDeliverability?.delivered ?? 0
  const bounced = latestDeliverability?.bounced ?? 0
  const complained = latestDeliverability?.complained ?? 0

  return (
    `${latestDate} sales ledger: sourced ${sourced}, drafted ${drafted}, sent ${sent}, ` +
    `delivered ${delivered}, bounced ${bounced}, complained ${complained} ` +
    `(deliverability: ${health}), quoted ${quoted}, closes ${closes}.`
  )
}

/**
 * Computes the daily sales ledger over `[since, until]` (inclusive ISO
 * dates) from the real D1 event tables listed in the module header. Pure
 * w.r.t. the returned shape — every field is either an exact aggregate or
 * an explicit `not_measured` sentinel.
 */
export const computeDailySalesLedger = async (
  db: D1Database,
  input: Readonly<{ since: string; until: string; nowIso?: () => string }>,
): Promise<DailySalesLedger> => {
  const since = input.since
  const until = input.until
  const dates = dateRangeInclusive(since, until)
  const nowIso = input.nowIso?.() ?? new Date().toISOString()

  const [sourcedRows, draftedRows, approvedRows, sentRows, pipelineSnapshotRows, providerRows, optOutRows] =
    await Promise.all([
      db
        .prepare(
          `SELECT substr(created_at, 1, 10) AS day, vertical AS segment_key, COUNT(*) AS count
             FROM business_pipeline_rows
            WHERE substr(created_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day, segment_key`,
        )
        .bind(since, until)
        .all<CountByDaySegmentRow>(),
      db
        .prepare(
          `SELECT substr(created_at, 1, 10) AS day, segment_ref AS segment_key, COUNT(*) AS count
             FROM business_outreach_drafts
            WHERE substr(created_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day, segment_key`,
        )
        .bind(since, until)
        .all<CountByDaySegmentRow>(),
      db
        .prepare(
          `SELECT substr(created_at, 1, 10) AS day, template_version_ref AS segment_key, COUNT(*) AS count
             FROM business_outreach_template_approvals
            WHERE substr(created_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day, segment_key`,
        )
        .bind(since, until)
        .all<CountByDaySegmentRow>(),
      db
        .prepare(
          `SELECT substr(sent_at, 1, 10) AS day, template_version_ref AS segment_key, COUNT(*) AS count
             FROM business_outreach_sends
            WHERE substr(sent_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day, segment_key`,
        )
        .bind(since, until)
        .all<CountByDaySegmentRow>(),
      db
        .prepare(
          `SELECT substr(stage_updated_at, 1, 10) AS day, vertical, stage,
                  CASE WHEN quoted_max_usd_cents > 0 THEN 1 ELSE 0 END AS quoted_flag,
                  COUNT(*) AS count
             FROM business_pipeline_rows
            WHERE substr(stage_updated_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day, vertical, stage, quoted_flag`,
        )
        .bind(since, until)
        .all<PipelineSnapshotRow>(),
      db
        .prepare(
          `SELECT substr(COALESCE(NULLIF(TRIM(occurred_at), ''), created_at), 1, 10) AS day,
                  event_type, COUNT(*) AS count
             FROM email_provider_events
            WHERE substr(COALESCE(NULLIF(TRIM(occurred_at), ''), created_at), 1, 10) BETWEEN ? AND ?
            GROUP BY day, event_type`,
        )
        .bind(since, until)
        .all<ProviderEventRow>(),
      db
        .prepare(
          `SELECT substr(created_at, 1, 10) AS day, COUNT(*) AS count
             FROM email_suppression_entries
            WHERE reason = 'unsubscribe' AND substr(created_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day`,
        )
        .bind(since, until)
        .all<CountByDayRow>(),
    ])

  const segmentDayMap = new Map<
    string,
    { -readonly [K in keyof DailySalesLedgerSegmentDay]: DailySalesLedgerSegmentDay[K] }
  >()
  const key = (date: string, segmentRef: BusinessOutreachSegmentRef) => `${date}|${segmentRef}`
  for (const date of dates) {
    for (const segmentRef of DAILY_SALES_LEDGER_SEGMENT_REFS) {
      segmentDayMap.set(key(date, segmentRef), emptySegmentDay(date, segmentRef))
    }
  }
  const rowFor = (date: string, segmentRef: BusinessOutreachSegmentRef) => {
    const existing = segmentDayMap.get(key(date, segmentRef))
    if (existing !== undefined) return existing
    const created = emptySegmentDay(date, segmentRef)
    segmentDayMap.set(key(date, segmentRef), created)
    return created
  }

  for (const row of sourcedRows.results ?? []) {
    rowFor(row.day, segmentFromVertical(row.segment_key)).sourced += Number(row.count)
  }
  for (const row of draftedRows.results ?? []) {
    const segmentRef = knownSegmentRef.get(row.segment_key)
    if (segmentRef === undefined) continue
    rowFor(row.day, segmentRef).drafted += Number(row.count)
  }
  for (const row of approvedRows.results ?? []) {
    const segmentRef = segmentRefForTemplateVersion.get(row.segment_key)
    if (segmentRef === undefined) continue
    rowFor(row.day, segmentRef).approved += Number(row.count)
  }
  for (const row of sentRows.results ?? []) {
    const segmentRef = segmentRefForTemplateVersion.get(row.segment_key)
    if (segmentRef === undefined) continue
    rowFor(row.day, segmentRef).sent += Number(row.count)
  }
  for (const row of pipelineSnapshotRows.results ?? []) {
    const segmentDay = rowFor(row.day, segmentFromVertical(row.vertical))
    const count = Number(row.count)
    if (row.quoted_flag === 1) segmentDay.quoted += count
    if (row.stage === 'closed_won') segmentDay.closedWon += count
    if (row.stage === 'closed_lost') segmentDay.closedLost += count
  }

  const deliverabilityByDay = new Map<
    string,
    { delivered: number; bounced: number; complained: number; failed: number; optOuts: number }
  >()
  for (const date of dates) {
    deliverabilityByDay.set(date, {
      bounced: 0,
      complained: 0,
      delivered: 0,
      failed: 0,
      optOuts: 0,
    })
  }
  for (const row of providerRows.results ?? []) {
    const bucket = deliverabilityByDay.get(row.day)
    if (bucket === undefined) continue
    const count = Number(row.count)
    if (row.event_type === 'email.delivered') bucket.delivered += count
    else if (row.event_type === 'email.bounced') bucket.bounced += count
    else if (row.event_type === 'email.complained') bucket.complained += count
    else if (row.event_type === 'email.failed') bucket.failed += count
  }
  for (const row of optOutRows.results ?? []) {
    const bucket = deliverabilityByDay.get(row.day)
    if (bucket === undefined) continue
    bucket.optOuts += Number(row.count)
  }

  const deliverabilityDays: ReadonlyArray<DailySalesLedgerDeliverabilityDay> = dates.map(date => {
    const bucket = deliverabilityByDay.get(date) ?? {
      bounced: 0,
      complained: 0,
      delivered: 0,
      failed: 0,
      optOuts: 0,
    }
    const resolvedOutcomes = bucket.delivered + bucket.bounced + bucket.complained
    const bounceRatePct = rateMetric(bucket.bounced, resolvedOutcomes)
    const complaintRatePct = rateMetric(bucket.complained, resolvedOutcomes)
    return {
      bounceRatePct,
      bounced: bucket.bounced,
      complained: bucket.complained,
      complaintRatePct,
      date,
      delivered: bucket.delivered,
      failed: bucket.failed,
      health: healthFromRates(bounceRatePct, complaintRatePct),
      optOuts: bucket.optOuts,
    }
  })

  const segmentDays: ReadonlyArray<DailySalesLedgerSegmentDay> = dates.flatMap(date =>
    DAILY_SALES_LEDGER_SEGMENT_REFS.map(segmentRef => rowFor(date, segmentRef)),
  )

  const totals: DailySalesLedgerTotals = {
    approved: segmentDays.reduce((sum, row) => sum + row.approved, 0),
    bounced: deliverabilityDays.reduce((sum, row) => sum + row.bounced, 0),
    closedLost: segmentDays.reduce((sum, row) => sum + row.closedLost, 0),
    closedWon: segmentDays.reduce((sum, row) => sum + row.closedWon, 0),
    complained: deliverabilityDays.reduce((sum, row) => sum + row.complained, 0),
    delivered: deliverabilityDays.reduce((sum, row) => sum + row.delivered, 0),
    drafted: segmentDays.reduce((sum, row) => sum + row.drafted, 0),
    optOuts: deliverabilityDays.reduce((sum, row) => sum + row.optOuts, 0),
    quoted: segmentDays.reduce((sum, row) => sum + row.quoted, 0),
    sent: segmentDays.reduce((sum, row) => sum + row.sent, 0),
    sourced: segmentDays.reduce((sum, row) => sum + row.sourced, 0),
  }

  const ledgerWithoutDigest: Omit<DailySalesLedger, 'digestLine'> = {
    deliverabilityDays,
    generatedAt: nowIso,
    notMeasured: [
      { field: 'replies', reasonRef: NOT_MEASURED_REPLIES.reasonRef },
      { field: 'reportClicks', reasonRef: NOT_MEASURED_REPORT_CLICKS.reasonRef },
      { field: 'conversations', reasonRef: NOT_MEASURED_CONVERSATIONS.reasonRef },
    ],
    segmentDays,
    segmentRefs: DAILY_SALES_LEDGER_SEGMENT_REFS,
    since,
    totals,
    until,
  }

  return { ...ledgerWithoutDigest, digestLine: renderDigestLine(ledgerWithoutDigest) }
}
