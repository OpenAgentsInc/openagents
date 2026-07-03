import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
  readBusinessCommitmentWeeklyReview,
} from './business-commitment-ledger'

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
  db.exec(migration('0275_business_commitment_ledger.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

describe('business commitment ledger', () => {
  test('seeds the two owed make-goods as tracked weekly-review commitments', async () => {
    const review = await readBusinessCommitmentWeeklyReview(
      makeDb(),
      '2026-07-03T12:00:00.000Z',
    )

    expect(review).toMatchObject({
      schemaVersion: 'openagents.business_commitment_weekly_review.v1',
      generatedAt: '2026-07-03T12:00:00.000Z',
      weeklyReviewRef: BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
      totals: {
        commitmentCount: 2,
        dueCount: 2,
        blockedCount: 0,
        shippedCount: 0,
        parkedCount: 0,
        untrackedOwedCommitmentCount: 0,
      },
    })
    expect(review.owedMakeGoodRefs).toEqual([
      'business.commitment.owed.ecommerce_make_good.20260702',
      'business.commitment.owed.settlement_make_good.20260702',
    ])
    expect(review.commitments.map(commitment => commitment.engagementRef)).toEqual([
      'business.engagement.opaque.ecommerce_make_good',
      'business.engagement.opaque.settlement_make_good',
    ])
    expect(
      review.commitments.every(
        commitment =>
          commitment.ownerRef !== '' &&
          commitment.dueAt !== '' &&
          commitment.weeklyReviewRef === BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
      ),
    ).toBe(true)
    expect(JSON.stringify(review.commitments)).not.toMatch(
      /@|client_name|client_email|customer|contact_email|raw_crm|raw_email/i,
    )
  })

  test('includes new promised sends and shipped evidence in weekly review order', async () => {
    const db = makeDb()
    await db
      .prepare(
        `INSERT INTO business_commitment_ledger (
          id,
          commitment_ref,
          engagement_ref,
          owner_ref,
          vertical_ref,
          promised_object_ref,
          commitment_kind,
          due_state,
          due_at,
          shipped_at,
          weekly_review_ref,
          source_refs_json,
          blocker_refs_json,
          evidence_refs_json,
          created_at,
          updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, 'send', 'shipped', ?, ?, ?, ?, '[]', ?, ?, ?)`,
      )
      .bind(
        'business_commitment_sent_weekly_update_20260703',
        'business.commitment.sent.weekly_update.20260703',
        'business.engagement.opaque.ops_test',
        'owner.business.ops',
        'vertical.business_ops',
        'send.business.weekly_pipeline_update',
        '2026-07-03T17:00:00.000Z',
        '2026-07-03T18:00:00.000Z',
        BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
        JSON.stringify(['docs/fable/ROADMAP_BIZ.md#BF-9.1']),
        JSON.stringify(['receipt.business.send.weekly_update.20260703']),
        '2026-07-03T17:00:00.000Z',
        '2026-07-03T18:00:00.000Z',
      )
      .run()

    const review = await readBusinessCommitmentWeeklyReview(
      db,
      '2026-07-03T19:00:00.000Z',
    )

    expect(review.totals).toMatchObject({
      commitmentCount: 3,
      dueCount: 2,
      shippedCount: 1,
      untrackedOwedCommitmentCount: 0,
    })
    expect(review.commitments.at(-1)).toMatchObject({
      commitmentKind: 'send',
      commitmentRef: 'business.commitment.sent.weekly_update.20260703',
      dueState: 'shipped',
      evidenceRefs: ['receipt.business.send.weekly_update.20260703'],
    })
  })
})
