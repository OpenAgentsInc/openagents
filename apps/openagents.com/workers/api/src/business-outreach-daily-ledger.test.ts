import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  computeDailySalesLedger,
  dateRangeInclusive,
  DailySalesLedgerValidationError,
  DAILY_SALES_LEDGER_SEGMENT_REFS,
} from './business-outreach-daily-ledger'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0278_business_commitment_ledger.sql'))
  db.exec(migration('0294_business_pipeline_queue.sql'))
  db.exec('ALTER TABLE business_pipeline_rows ADD COLUMN business_signup_request_id TEXT;')
  db.exec(migration('0299_business_pipeline_partner_routing.sql'))
  db.exec(migration('0296_business_outreach_sequences.sql'))
  db.exec(migration('0026_email_ledger.sql'))
  db.exec(migration('0063_email_campaign_records.sql'))
  db.exec(migration('0218_crm_contacts.sql'))
  db.exec(migration('0270_business_funnel_events.sql'))
  db.exec(migration('0310_crm_command_batches_and_replies.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

const insertReplyEvent = async (
  db: D1Database,
  id: string,
  createdAt: string,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO crm_reply_events (
        id, tenant_ref, contact_id, from_email, provider, routed_to, created_at
      ) VALUES (?, 'tenant.openagents', NULL, 'prospect@example.com',
                'inbound_webhook', 'operator_notification', ?)`,
    )
    .bind(id, createdAt)
    .run()
}

const insertFunnelEvent = async (
  db: D1Database,
  eventRef: string,
  occurredAt: string,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO business_funnel_events (
        id, event_ref, stage, source_kind, source_ref, occurred_at, observed_at
      ) VALUES (?, ?, 'visit', 'outbound', 'source.test', ?, ?)`,
    )
    .bind(`funnel-${eventRef}`, eventRef, occurredAt, occurredAt)
    .run()
}

/** Canned khala-sync Postgres fake for the Sarah conversations read. */
const fakeSarahTurnStore = (
  rows: ReadonlyArray<Record<string, unknown>>,
  observed?: { queries: Array<{ text: string; params: ReadonlyArray<string> }>; ended: number },
) => ({
  binding: { connectionString: 'postgres://fake' },
  makeSqlClient: async (_connectionString: string) => ({
    end: async () => {
      if (observed) observed.ended += 1
    },
    query: async (text: string, params: ReadonlyArray<string>) => {
      observed?.queries.push({ params, text })
      return rows
    },
  }),
})

const insertPipelineRow = async (
  db: D1Database,
  overrides: Partial<{
    pipelineRef: string
    vertical: string
    stage: string
    quotedMaxUsdCents: number
    createdAt: string
    stageUpdatedAt: string
  }> = {},
): Promise<void> => {
  const row = {
    pipelineRef: 'biz-pipe-001',
    vertical: 'commerce',
    stage: 'intake_received',
    quotedMaxUsdCents: 0,
    createdAt: '2026-07-01T12:00:00.000Z',
    stageUpdatedAt: '2026-07-01T12:00:00.000Z',
    ...overrides,
  }
  await db
    .prepare(
      `INSERT INTO business_pipeline_rows (
        pipeline_ref, vertical, source_ref, stage,
        quoted_min_usd_cents, quoted_max_usd_cents, quoted_band_label,
        owner_role, receipt_refs_json, partner_route_flag,
        created_at, updated_at, stage_updated_at
      ) VALUES (?, ?, 'apollo:test', ?, 0, ?, 'unquoted', 'operator', '[]', 0, ?, ?, ?)`,
    )
    .bind(
      row.pipelineRef,
      row.vertical,
      row.stage,
      row.quotedMaxUsdCents,
      row.createdAt,
      row.stageUpdatedAt,
      row.stageUpdatedAt,
    )
    .run()
}

describe('dateRangeInclusive', () => {
  test('produces an inclusive list of ISO dates', () => {
    expect(dateRangeInclusive('2026-07-01', '2026-07-03')).toEqual([
      '2026-07-01',
      '2026-07-02',
      '2026-07-03',
    ])
  })

  test('rejects since after until', () => {
    expect(() => dateRangeInclusive('2026-07-05', '2026-07-01')).toThrow(
      DailySalesLedgerValidationError,
    )
  })

  test('rejects a malformed date', () => {
    expect(() => dateRangeInclusive('not-a-date', '2026-07-01')).toThrow(
      DailySalesLedgerValidationError,
    )
  })

  test('rejects a window larger than the max', () => {
    expect(() => dateRangeInclusive('2026-01-01', '2026-12-31')).toThrow(
      DailySalesLedgerValidationError,
    )
  })
})

describe('computeDailySalesLedger', () => {
  test('returns a zero-filled ledger for every segment and day when there is no data', async () => {
    const db = makeDb()

    const ledger = await computeDailySalesLedger(db, {
      nowIso: () => '2026-07-03T00:00:00.000Z',
      since: '2026-07-01',
      until: '2026-07-02',
    })

    expect(ledger.segmentRefs).toEqual(DAILY_SALES_LEDGER_SEGMENT_REFS)
    // 2 days x N segments
    expect(ledger.segmentDays).toHaveLength(2 * DAILY_SALES_LEDGER_SEGMENT_REFS.length)
    expect(ledger.segmentDays.every(row => row.sourced === 0 && row.sent === 0)).toBe(true)
    expect(ledger.deliverabilityDays).toHaveLength(2)
    expect(ledger.deliverabilityDays[0]?.health).toBe('not_measured')
    expect(ledger.totals.sourced).toBe(0)
    // Known gaps stay honest, never silently zeroed.
    expect(
      ledger.segmentDays.every(
        row =>
          row.replies.status === 'not_measured' &&
          row.reportClicks.status === 'not_measured' &&
          row.conversations.status === 'not_measured',
      ),
    ).toBe(true)
    expect(ledger.notMeasured.map(entry => entry.field).sort()).toEqual([
      'conversations',
      'conversations.perSegment',
      'operatorMinutes',
      'replies.perSegment',
      'reportClicks.perSegment',
    ])
    // Replies/report clicks are measured per day (their event tables exist
    // and are simply empty); conversations are not (no khala-sync source).
    expect(ledger.engagementDays).toHaveLength(2)
    expect(
      ledger.engagementDays.every(
        day =>
          day.replies.status === 'measured' &&
          day.replies.count === 0 &&
          day.reportClicks.status === 'measured' &&
          day.reportClicks.count === 0 &&
          day.conversations.status === 'not_measured',
      ),
    ).toBe(true)
    const conversations = ledger.engagementDays[0]?.conversations
    expect(conversations?.status === 'not_measured' && conversations.reasonRef).toBe(
      'reason.ob6.khala_sync_db_binding_absent',
    )
    expect(ledger.operatorMinutes).toEqual({
      reasonRef: 'reason.ob6.operator_minutes_pending_ob4_batch_timing',
      status: 'not_measured',
    })
  })

  test('aggregates a sourced pipeline row into the correct day and segment', async () => {
    const db = makeDb()
    await insertPipelineRow(db, {
      createdAt: '2026-07-01T09:00:00.000Z',
      pipelineRef: 'biz-pipe-ecom-1',
      stageUpdatedAt: '2026-07-01T09:00:00.000Z',
      vertical: 'commerce',
    })

    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-01',
    })

    const row = ledger.segmentDays.find(
      entry => entry.segmentRef === 'agent_readiness_ecommerce',
    )
    expect(row?.sourced).toBe(1)
    expect(ledger.totals.sourced).toBe(1)
  })

  test('counts quoted and closed_won pipeline rows on their stage_updated_at day', async () => {
    const db = makeDb()
    await insertPipelineRow(db, {
      pipelineRef: 'biz-pipe-closed-1',
      quotedMaxUsdCents: 500000,
      stage: 'closed_won',
      stageUpdatedAt: '2026-07-02T10:00:00.000Z',
      vertical: 'saas',
    })

    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-02',
    })

    const day1 = ledger.segmentDays.find(
      row => row.date === '2026-07-02' && row.segmentRef === 'agent_readiness_saas',
    )
    expect(day1?.quoted).toBe(1)
    expect(day1?.closedWon).toBe(1)
    expect(day1?.closedLost).toBe(0)
    expect(ledger.totals.closedWon).toBe(1)
  })

  test('aggregates drafts, template approvals, and sends by segment and day', async () => {
    const db = makeDb()
    await insertPipelineRow(db, { pipelineRef: 'biz-pipe-agency-1', vertical: 'agency' })
    await db
      .prepare(
        `INSERT INTO business_outreach_drafts (
          draft_ref, pipeline_ref, subject_ref, template_version_ref, segment_ref,
          audit_report_ref, finding_refs_json, body_text, claim_lint_refs_json,
          source_ref, state, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, '[]', ?, '[]', ?, 'draft', ?)`,
      )
      .bind(
        'draft-1',
        'biz-pipe-agency-1',
        'prospect.001',
        'business.outreach.agent_readiness_agency.report_led.v1',
        'agent_readiness_agency',
        'audit.001',
        'body text ok',
        'source.001',
        '2026-07-01T10:00:00.000Z',
      )
      .run()
    await db
      .prepare(
        `INSERT INTO business_outreach_template_approvals (
          approval_receipt_ref, template_version_ref, approved_by_ref, source_ref, created_at
        ) VALUES (?, ?, 'owner', 'source.001', ?)`,
      )
      .bind(
        'receipt.approval.001',
        'business.outreach.agent_readiness_agency.report_led.v1',
        '2026-07-01T11:00:00.000Z',
      )
      .run()
    await db
      .prepare(
        `INSERT INTO business_outreach_sends (
          send_ref, pipeline_ref, draft_ref, subject_ref, template_version_ref,
          mailbox_ref, channel, source_ref, approval_receipt_ref, send_receipt_ref,
          sent_at, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, 'apollo_sequence', ?, ?, ?, ?, ?)`,
      )
      .bind(
        'send-1',
        'biz-pipe-agency-1',
        'draft-1',
        'prospect.001',
        'business.outreach.agent_readiness_agency.report_led.v1',
        'mailbox.001',
        'source.001',
        'receipt.approval.001',
        'receipt.send.001',
        '2026-07-01T12:00:00.000Z',
        '2026-07-01T12:00:00.000Z',
      )
      .run()

    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-01',
    })
    const row = ledger.segmentDays.find(entry => entry.segmentRef === 'agent_readiness_agency')
    expect(row?.drafted).toBe(1)
    expect(row?.approved).toBe(1)
    expect(row?.sent).toBe(1)
  })

  test('computes deliverability rates and health from email_provider_events, and opt-outs from email_suppression_entries', async () => {
    const db = makeDb()
    const insertEvent = (eventType: string, id: string) =>
      db
        .prepare(
          `INSERT INTO email_provider_events (
            id, provider, provider_event_id, event_type, email,
            email_message_id, provider_message_id, occurred_at,
            payload_summary_json, source_authority_ref, created_at
          ) VALUES (?, 'resend', ?, ?, 'prospect@example.com', NULL, 'msg-1', ?, '{}', 'resend.webhook', ?)`,
        )
        .bind(id, id, eventType, '2026-07-01T10:00:00.000Z', '2026-07-01T10:00:00.000Z')
        .run()

    // 10 delivered, 1 bounced -> bounce rate 1/11 ~= 9.09% -> breach (>5%)
    for (let index = 0; index < 10; index += 1) {
      await insertEvent('email.delivered', `evt-delivered-${index}`)
    }
    await insertEvent('email.bounced', 'evt-bounced-0')

    await db
      .prepare(
        `INSERT INTO email_suppression_entries (
          id, email, reason, scope, active, source_authority_ref, created_at, updated_at
        ) VALUES (?, ?, 'unsubscribe', 'marketing', 1, 'test', ?, ?)`,
      )
      .bind(
        'suppression-1',
        'prospect@example.com',
        '2026-07-01T10:05:00.000Z',
        '2026-07-01T10:05:00.000Z',
      )
      .run()

    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-01',
    })

    const day = ledger.deliverabilityDays[0]
    expect(day?.delivered).toBe(10)
    expect(day?.bounced).toBe(1)
    expect(day?.optOuts).toBe(1)
    expect(day?.bounceRatePct.status).toBe('measured')
    if (day?.bounceRatePct.status === 'measured') {
      expect(day.bounceRatePct.valuePct).toBeCloseTo((1 / 11) * 100, 2)
    }
    expect(day?.health).toBe('breach')
    expect(ledger.totals.optOuts).toBe(1)
  })

  test('counts inbound replies per day from crm_reply_events', async () => {
    const db = makeDb()
    await insertReplyEvent(db, 'reply-1', '2026-07-01T10:00:00.000Z')
    await insertReplyEvent(db, 'reply-2', '2026-07-01T18:30:00.000Z')
    await insertReplyEvent(db, 'reply-3', '2026-07-02T08:00:00.000Z')

    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-02',
    })

    expect(ledger.engagementDays.map(day => day.replies)).toEqual([
      { count: 2, status: 'measured' },
      { count: 1, status: 'measured' },
    ])
    // Per-segment replies stay honest: the event rows carry no segment ref.
    expect(
      ledger.segmentDays.every(row => row.replies.status === 'not_measured'),
    ).toBe(true)
  })

  test('counts only readiness-report click funnel events, per day', async () => {
    const db = makeDb()
    await insertFunnelEvent(
      db,
      'agent_readiness_report_click_tok123_1',
      '2026-07-01T09:00:00.000Z',
    )
    await insertFunnelEvent(
      db,
      'agent_readiness_report_click_tok123_2',
      '2026-07-02T09:00:00.000Z',
    )
    // A non-click funnel receipt must NOT count as a report click.
    await insertFunnelEvent(db, 'business_signup_visit_001', '2026-07-01T09:30:00.000Z')

    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-02',
    })

    expect(ledger.engagementDays.map(day => day.reportClicks)).toEqual([
      { count: 1, status: 'measured' },
      { count: 1, status: 'measured' },
    ])
  })

  test('counts Sarah conversations per day from the khala-sync turn store', async () => {
    const db = makeDb()
    const observed: {
      ended: number
      queries: Array<{ text: string; params: ReadonlyArray<string> }>
    } = { ended: 0, queries: [] }
    const ledger = await computeDailySalesLedger(db, {
      sarahTurnStore: fakeSarahTurnStore(
        [
          { count: 3, day: '2026-07-01' },
          // postgres.js may hand counts back as strings; both must parse.
          { count: '1', day: '2026-07-02' },
        ],
        observed,
      ),
      since: '2026-07-01',
      until: '2026-07-02',
    })

    expect(ledger.engagementDays.map(day => day.conversations)).toEqual([
      { count: 3, status: 'measured' },
      { count: 1, status: 'measured' },
    ])
    expect(ledger.notMeasured.map(entry => entry.field)).not.toContain('conversations')
    expect(observed.queries).toHaveLength(1)
    expect(observed.queries[0]?.text).toContain('sarah_transcript_turns')
    expect(observed.queries[0]?.text).toContain("role = 'user'")
    expect(observed.queries[0]?.params).toEqual(['2026-07-01', '2026-07-02'])
    expect(observed.ended).toBe(1)
  })

  test('reports conversations as not_measured when the turn-store read fails', async () => {
    const db = makeDb()
    const ledger = await computeDailySalesLedger(db, {
      sarahTurnStore: {
        binding: { connectionString: 'postgres://fake' },
        makeSqlClient: async () => ({
          end: async () => {},
          query: async () => {
            throw new Error('connection refused')
          },
        }),
      },
      since: '2026-07-01',
      until: '2026-07-01',
    })

    const conversations = ledger.engagementDays[0]?.conversations
    expect(conversations).toEqual({
      reasonRef: 'reason.ob6.sarah_turn_store_unreachable',
      status: 'not_measured',
    })
    expect(
      ledger.notMeasured.find(entry => entry.field === 'conversations')?.reasonRef,
    ).toBe('reason.ob6.sarah_turn_store_unreachable')
  })

  test('renders a one-line digest for the latest day in the window', async () => {
    const db = makeDb()
    await insertPipelineRow(db, {
      createdAt: '2026-07-02T09:00:00.000Z',
      pipelineRef: 'biz-pipe-marketplace-1',
      stageUpdatedAt: '2026-07-02T09:00:00.000Z',
      vertical: 'marketplace',
    })

    await insertReplyEvent(db, 'reply-digest-1', '2026-07-02T10:00:00.000Z')
    await insertFunnelEvent(
      db,
      'agent_readiness_report_click_tokd_1',
      '2026-07-02T11:00:00.000Z',
    )

    const ledger = await computeDailySalesLedger(db, {
      sarahTurnStore: fakeSarahTurnStore([{ count: 2, day: '2026-07-02' }]),
      since: '2026-07-01',
      until: '2026-07-02',
    })

    expect(ledger.digestLine).toContain('2026-07-02 sales ledger:')
    expect(ledger.digestLine).toContain('sourced 1')
    expect(ledger.digestLine).toContain('replies 1')
    expect(ledger.digestLine).toContain('report clicks 1')
    expect(ledger.digestLine).toContain('conversations 2')
  })

  test('digest marks engagement metrics n/m when conversations are not measured', async () => {
    const db = makeDb()
    const ledger = await computeDailySalesLedger(db, {
      since: '2026-07-01',
      until: '2026-07-01',
    })
    expect(ledger.digestLine).toContain('conversations n/m')
    expect(ledger.digestLine).toContain('replies 0')
  })
})
