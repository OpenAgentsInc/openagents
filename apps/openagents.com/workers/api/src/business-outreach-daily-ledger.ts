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
//   - `crm_reply_events`            (crm-reply.ts, OB-4 0310)      -> replies (per day)
//   - `business_funnel_events`      (agent-readiness-public-report-routes.ts,
//                                    OB-3 report-click receipts)   -> report clicks (per day)
//   - `sarah_transcript_turns`      (apps/sarah turn-store.ts, khala-sync
//                                    Postgres via KHALA_SYNC_DB)   -> conversations (per day)
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
// or guessed value. `replies`, `reportClicks`, and `conversations` are now
// measured exactly PER DAY (their event sources landed with OB-3/OB-4/OB-5)
// but stay `not_measured` PER SEGMENT because none of those sources carries
// a segment linkage — see the sentinel comments below. `operatorMinutes`
// (the OB-4 agency-trap metric) has no exact source yet and is a sentinel.
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

// Per-SEGMENT engagement stays not_measured even though the same metrics are
// now measured exactly per DAY (see `engagementDays`): none of the three
// event sources carries a segment linkage today. `crm_reply_events` has
// tenant/contact refs only; `business_funnel_events` report-click rows carry
// an LG-6 source_ref (not a segment); `sarah_transcript_turns` keys on an
// opaque prospect_ref. Splitting them per segment would require joins that
// do not exist yet — reporting a per-segment number would be synthesis.
const NOT_MEASURED_REPLIES: NotMeasured = {
  reasonRef: 'reason.ob6.reply_events_lack_segment_linkage',
  status: 'not_measured',
}
const NOT_MEASURED_REPORT_CLICKS: NotMeasured = {
  reasonRef: 'reason.ob6.report_click_events_lack_segment_linkage',
  status: 'not_measured',
}
const NOT_MEASURED_CONVERSATIONS: NotMeasured = {
  reasonRef: 'reason.ob6.sarah_turns_lack_segment_linkage',
  status: 'not_measured',
}
// The OB-4 agency-trap metric (operator-minutes per approved batch): the
// batch receipt table (`crm_command_batches`, migration 0310) records WHICH
// commands were batch-approved but not how long the operator spent reviewing
// them, so there is nothing exact to count yet.
const NOT_MEASURED_OPERATOR_MINUTES: NotMeasured = {
  reasonRef: 'reason.ob6.operator_minutes_pending_ob4_batch_timing',
  status: 'not_measured',
}
const NOT_MEASURED_CONVERSATIONS_NO_BINDING: NotMeasured = {
  reasonRef: 'reason.ob6.khala_sync_db_binding_absent',
  status: 'not_measured',
}
const NOT_MEASURED_CONVERSATIONS_UNREACHABLE: NotMeasured = {
  reasonRef: 'reason.ob6.sarah_turn_store_unreachable',
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

export type DailySalesLedgerCountMetric =
  | Readonly<{ status: 'measured'; count: number }>
  | NotMeasured

/**
 * Cross-segment (global) per-day engagement counts from the event sources
 * that landed after the first OB-6 cut:
 *  - `replies`       — exact rows from `crm_reply_events` (migration 0310,
 *                      OB-4 inbound-reply plumbing), keyed on `created_at`.
 *  - `reportClicks`  — exact rows from `business_funnel_events` whose
 *                      `event_ref` is the OB-3 readiness-report click
 *                      convention (`agent_readiness_report_click_<token>_<n>`,
 *                      written by agent-readiness-public-report-routes.ts),
 *                      keyed on `occurred_at`.
 *  - `conversations` — distinct Sarah prospects with at least one prospect
 *                      (`role = 'user'`) turn that day in
 *                      `sarah_transcript_turns`, read from the khala-sync
 *                      Postgres the Sarah service writes to (OB-5/KHS;
 *                      apps/sarah turn-store falls back to
 *                      KHALA_SYNC_DATABASE_URL). `not_measured` when the
 *                      KHALA_SYNC_DB binding is absent or the read fails —
 *                      never zero-faked.
 */
export type DailySalesLedgerEngagementDay = Readonly<{
  date: string
  replies: DailySalesLedgerCountMetric
  reportClicks: DailySalesLedgerCountMetric
  conversations: DailySalesLedgerCountMetric
}>

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
  engagementDays: ReadonlyArray<DailySalesLedgerEngagementDay>
  totals: DailySalesLedgerTotals
  /** OB-4 agency-trap metric — not measurable until batches carry timing. */
  operatorMinutes: NotMeasured
  digestLine: string
  notMeasured: ReadonlyArray<Readonly<{ field: string; reasonRef: string }>>
}>

/**
 * Minimal Postgres reader used for the Sarah conversations count. Matches
 * the `KhalaSyncSmokeSqlClient` contract from khala-sync-db-smoke-routes.ts
 * (query + end), so the deployed wiring can reuse `defaultMakeSqlClient`
 * while tests inject a canned fake.
 */
export type SarahTurnStoreSqlClient = Readonly<{
  query: (
    text: string,
    params: ReadonlyArray<string>,
  ) => Promise<ReadonlyArray<Record<string, unknown>>>
  end: () => Promise<void>
}>

export type SarahTurnStoreSource = Readonly<{
  /** `env.KHALA_SYNC_DB` — absent until the binding is deployed. */
  binding: Readonly<{ connectionString: string }> | undefined
  makeSqlClient: (connectionString: string) => Promise<SarahTurnStoreSqlClient>
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

/**
 * Distinct Sarah prospects with at least one prospect turn per UTC day,
 * read from the khala-sync Postgres. Returns a by-day map on success, or a
 * `NotMeasured` sentinel (binding absent / query failed) — never a guess.
 */
const readSarahConversationDays = async (
  source: SarahTurnStoreSource | undefined,
  since: string,
  until: string,
): Promise<ReadonlyMap<string, number> | NotMeasured> => {
  if (
    source === undefined ||
    source.binding === undefined ||
    typeof source.binding.connectionString !== 'string' ||
    source.binding.connectionString.length === 0
  ) {
    return NOT_MEASURED_CONVERSATIONS_NO_BINDING
  }
  let sql: SarahTurnStoreSqlClient | undefined
  try {
    sql = await source.makeSqlClient(source.binding.connectionString)
    const rows = await sql.query(
      `SELECT to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD') AS day,
              COUNT(DISTINCT prospect_ref)::int AS count
         FROM sarah_transcript_turns
        WHERE role = 'user'
          AND recorded_at >= ($1 || 'T00:00:00Z')::timestamptz
          AND recorded_at < (($2 || 'T00:00:00Z')::timestamptz + INTERVAL '1 day')
        GROUP BY day`,
      [since, until],
    )
    const byDay = new Map<string, number>()
    for (const row of rows) {
      const day = typeof row.day === 'string' ? row.day : undefined
      const count =
        typeof row.count === 'number'
          ? row.count
          : typeof row.count === 'string'
            ? Number(row.count)
            : Number.NaN
      if (day === undefined || !Number.isFinite(count)) continue
      byDay.set(day, count)
    }
    return byDay
  } catch {
    return NOT_MEASURED_CONVERSATIONS_UNREACHABLE
  } finally {
    if (sql !== undefined) {
      try {
        await sql.end()
      } catch {
        // already reported (or succeeded) above; releasing is best-effort
      }
    }
  }
}

const renderCountForDigest = (metric: DailySalesLedgerCountMetric): string =>
  metric.status === 'measured' ? String(metric.count) : 'n/m'

const renderDigestLine = (ledger: Omit<DailySalesLedger, 'digestLine'>): string => {
  const latestDate = ledger.deliverabilityDays.at(-1)?.date ?? ledger.until
  const latestSegmentDays = ledger.segmentDays.filter(row => row.date === latestDate)
  const latestDeliverability = ledger.deliverabilityDays.find(row => row.date === latestDate)
  const latestEngagement = ledger.engagementDays.find(row => row.date === latestDate)
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

  const replies = renderCountForDigest(latestEngagement?.replies ?? NOT_MEASURED_REPLIES)
  const reportClicks = renderCountForDigest(
    latestEngagement?.reportClicks ?? NOT_MEASURED_REPORT_CLICKS,
  )
  const conversations = renderCountForDigest(
    latestEngagement?.conversations ?? NOT_MEASURED_CONVERSATIONS_NO_BINDING,
  )

  return (
    `${latestDate} sales ledger: sourced ${sourced}, drafted ${drafted}, sent ${sent}, ` +
    `delivered ${delivered}, bounced ${bounced}, complained ${complained} ` +
    `(deliverability: ${health}), replies ${replies}, report clicks ${reportClicks}, ` +
    `conversations ${conversations}, quoted ${quoted}, closes ${closes}.`
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
  input: Readonly<{
    since: string
    until: string
    nowIso?: () => string
    /**
     * Optional khala-sync Postgres source for the Sarah conversations
     * count. Omitted (or binding absent) => conversations stay
     * `not_measured` with an exact reasonRef.
     */
    sarahTurnStore?: SarahTurnStoreSource
  }>,
): Promise<DailySalesLedger> => {
  const since = input.since
  const until = input.until
  const dates = dateRangeInclusive(since, until)
  const nowIso = input.nowIso?.() ?? new Date().toISOString()

  const [
    sourcedRows,
    draftedRows,
    approvedRows,
    sentRows,
    pipelineSnapshotRows,
    providerRows,
    optOutRows,
    replyRows,
    reportClickRows,
    conversationDays,
  ] = await Promise.all([
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
      // Inbound replies: exact rows from OB-4's reply plumbing (0310).
      db
        .prepare(
          `SELECT substr(created_at, 1, 10) AS day, COUNT(*) AS count
             FROM crm_reply_events
            WHERE substr(created_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day`,
        )
        .bind(since, until)
        .all<CountByDayRow>(),
      // Readiness-report clicks: exact OB-3 funnel receipts. The event_ref
      // prefix is the convention written by defaultRecordClick in
      // agent-readiness-public-report-routes.ts.
      db
        .prepare(
          `SELECT substr(occurred_at, 1, 10) AS day, COUNT(*) AS count
             FROM business_funnel_events
            WHERE event_ref LIKE 'agent_readiness_report_click_%'
              AND substr(occurred_at, 1, 10) BETWEEN ? AND ?
            GROUP BY day`,
        )
        .bind(since, until)
        .all<CountByDayRow>(),
      readSarahConversationDays(input.sarahTurnStore, since, until),
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

  const repliesByDay = new Map<string, number>()
  for (const row of replyRows.results ?? []) {
    repliesByDay.set(row.day, (repliesByDay.get(row.day) ?? 0) + Number(row.count))
  }
  const reportClicksByDay = new Map<string, number>()
  for (const row of reportClickRows.results ?? []) {
    reportClicksByDay.set(row.day, (reportClicksByDay.get(row.day) ?? 0) + Number(row.count))
  }

  const engagementDays: ReadonlyArray<DailySalesLedgerEngagementDay> = dates.map(date => ({
    conversations:
      'status' in conversationDays
        ? conversationDays
        : { count: conversationDays.get(date) ?? 0, status: 'measured' },
    date,
    replies: { count: repliesByDay.get(date) ?? 0, status: 'measured' },
    reportClicks: { count: reportClicksByDay.get(date) ?? 0, status: 'measured' },
  }))

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

  const notMeasured: Array<{ field: string; reasonRef: string }> = [
    { field: 'replies.perSegment', reasonRef: NOT_MEASURED_REPLIES.reasonRef },
    { field: 'reportClicks.perSegment', reasonRef: NOT_MEASURED_REPORT_CLICKS.reasonRef },
    { field: 'conversations.perSegment', reasonRef: NOT_MEASURED_CONVERSATIONS.reasonRef },
    { field: 'operatorMinutes', reasonRef: NOT_MEASURED_OPERATOR_MINUTES.reasonRef },
  ]
  if ('status' in conversationDays) {
    notMeasured.push({ field: 'conversations', reasonRef: conversationDays.reasonRef })
  }

  const ledgerWithoutDigest: Omit<DailySalesLedger, 'digestLine'> = {
    deliverabilityDays,
    engagementDays,
    generatedAt: nowIso,
    notMeasured,
    operatorMinutes: NOT_MEASURED_OPERATOR_MINUTES,
    segmentDays,
    segmentRefs: DAILY_SALES_LEDGER_SEGMENT_REFS,
    since,
    totals,
    until,
  }

  return { ...ledgerWithoutDigest, digestLine: renderDigestLine(ledgerWithoutDigest) }
}
