import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  BUSINESS_FACTORY_METRICS_SQL,
  selectBusinessFactoryMetrics,
} from './business-factory-metrics'

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

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
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
  db.exec('PRAGMA foreign_keys = OFF')
  db.exec(migration('0091_omni_accepted_outcome_contracts.sql'))
  db.exec(migration('0095_omni_accepted_outcome_economics.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

describe('business factory metrics', () => {
  test('commits the BF-7.2 query pack with the locked metric refs', () => {
    expect(BUSINESS_FACTORY_METRICS_SQL).toContain(
      'business_factory.throughput.accepted_outcomes.v1',
    )
    expect(BUSINESS_FACTORY_METRICS_SQL).toContain(
      'business_factory.cycle_time.accepted_minutes.v1',
    )
    expect(BUSINESS_FACTORY_METRICS_SQL).toContain(
      'business_factory.pass_rate.terminal_outcomes_bps.v1',
    )
    expect(BUSINESS_FACTORY_METRICS_SQL).toContain(
      'business_factory.review_minutes.v1',
    )
    expect(BUSINESS_FACTORY_METRICS_SQL).toContain(
      'business_engagement.operator_minutes.review_ledger_floor.v1',
    )
  })

  test('returns measured rows from auditable outcome and economics ledgers', async () => {
    const raw = new DatabaseSync(':memory:')
    raw.exec('PRAGMA foreign_keys = OFF')
    raw.exec(migration('0091_omni_accepted_outcome_contracts.sql'))
    raw.exec(migration('0095_omni_accepted_outcome_economics.sql'))
    raw.prepare(
      `INSERT INTO omni_accepted_outcome_contracts (
        id, idempotency_key, work_kind, subject_ref, customer_ref,
        expected_artifacts_json, review_policy, acceptance_state, proof_policy,
        economic_state, closeout_requirements_json, legal_sensitive,
        public_receipt_ref, metadata_json, created_at, updated_at, archived_at
      ) VALUES (?, ?, ?, ?, ?, '[]', 'operator_review', ?, 'public_safe_proof',
        'paid_required', '[]', 0, ?, '{}', ?, ?, NULL)`,
    ).run(
      'contract_1',
      'idem_contract_1',
      'business',
      'business_engagement.opaque.1',
      'business_engagement.opaque.1',
      'accepted',
      'receipt.public.contract_1',
      '2026-07-03T10:00:00.000Z',
      '2026-07-03T11:30:00.000Z',
    )
    raw.prepare(
      `INSERT INTO omni_accepted_outcome_contracts (
        id, idempotency_key, work_kind, subject_ref, customer_ref,
        expected_artifacts_json, review_policy, acceptance_state, proof_policy,
        economic_state, closeout_requirements_json, legal_sensitive,
        public_receipt_ref, metadata_json, created_at, updated_at, archived_at
      ) VALUES (?, ?, ?, ?, ?, '[]', 'operator_review', ?, 'public_safe_proof',
        'paid_required', '[]', 0, ?, '{}', ?, ?, NULL)`,
    ).run(
      'contract_2',
      'idem_contract_2',
      'business',
      'business_engagement.opaque.2',
      'business_engagement.opaque.2',
      'rejected',
      'receipt.public.contract_2',
      '2026-07-03T10:20:00.000Z',
      '2026-07-03T12:00:00.000Z',
    )
    raw.prepare(
      `INSERT INTO omni_accepted_outcome_economics (
        id, idempotency_key, workroom_id, accepted_outcome_contract_id,
        work_kind, funding_mode, buyer_price_asset, buyer_price_cents,
        credits_charged, sats_charged, runner_cost_cents,
        provider_cost_cents, retry_cost_cents, review_minutes,
        review_cost_cents, artifact_cost_cents, total_cost_cents,
        accepted_value_cents, gross_margin_cents, public_caveat_ref,
        internal_caveat_ref, no_settlement_implication, metadata_json,
        created_at, updated_at, archived_at
      ) VALUES (?, ?, ?, ?, 'business', 'credit_funded', 'usd', 10000,
        0, 0, 1000, 2000, 0, 17, 3000, 0, 6000, 10000, 4000,
        'caveat.public.fixture', NULL, 1, '{}', ?, ?, NULL)`,
    ).run(
      'economics_1',
      'idem_economics_1',
      'workroom_1',
      'contract_1',
      '2026-07-03T11:40:00.000Z',
      '2026-07-03T11:45:00.000Z',
    )

    const rows = await selectBusinessFactoryMetrics(
      new SqliteD1(raw) as unknown as D1Database,
      {
        windowStart: '2026-07-03T00:00:00.000Z',
        windowEnd: '2026-07-04T00:00:00.000Z',
      },
    )

    const businessThroughput = rows.find(
      row =>
        row.metric_ref === 'business_factory.throughput.accepted_outcomes.v1'
        && row.work_kind === 'business',
    )
    const businessCycleTime = rows.find(
      row =>
        row.metric_ref === 'business_factory.cycle_time.accepted_minutes.v1'
        && row.work_kind === 'business',
    )
    const businessPassRate = rows.find(
      row =>
        row.metric_ref === 'business_factory.pass_rate.terminal_outcomes_bps.v1'
        && row.work_kind === 'business',
    )
    const businessReviewMinutes = rows.find(
      row =>
        row.metric_ref === 'business_factory.review_minutes.v1'
        && row.work_kind === 'business',
    )
    const engagementOperatorMinutes = rows.find(
      row =>
        row.metric_ref ===
          'business_engagement.operator_minutes.review_ledger_floor.v1'
        && row.engagement_ref === 'business_engagement.opaque.1',
    )

    expect(businessThroughput).toMatchObject({
      value: 1,
      unit: 'outcomes',
      measurement_state: 'measured',
    })
    expect(businessCycleTime).toMatchObject({
      value: 90,
      unit: 'minutes',
      measurement_state: 'measured',
    })
    expect(businessPassRate).toMatchObject({
      numerator: 1,
      denominator: 2,
      value: 5000,
      unit: 'basis_points',
      measurement_state: 'measured',
    })
    expect(businessReviewMinutes).toMatchObject({
      value: 17,
      unit: 'minutes',
      measurement_state: 'measured',
    })
    expect(engagementOperatorMinutes).toMatchObject({
      value: 17,
      caveat_refs_json:
        '["caveat.business_metrics.operator_minutes_review_only_until_labor_ledger"]',
    })
    expect(JSON.stringify(rows)).not.toContain('lead@example.com')
  })

  test('marks empty rate windows as not measured instead of fake zero rates', async () => {
    const rows = await selectBusinessFactoryMetrics(makeDb(), {
      windowStart: '2026-07-03T00:00:00.000Z',
      windowEnd: '2026-07-04T00:00:00.000Z',
    })

    const siteCycleTime = rows.find(
      row =>
        row.metric_ref === 'business_factory.cycle_time.accepted_minutes.v1'
        && row.work_kind === 'site',
    )
    const sitePassRate = rows.find(
      row =>
        row.metric_ref === 'business_factory.pass_rate.terminal_outcomes_bps.v1'
        && row.work_kind === 'site',
    )
    const siteThroughput = rows.find(
      row =>
        row.metric_ref === 'business_factory.throughput.accepted_outcomes.v1'
        && row.work_kind === 'site',
    )

    expect(siteThroughput).toMatchObject({
      value: 0,
      measurement_state: 'measured',
    })
    expect(siteCycleTime).toMatchObject({
      value: null,
      measurement_state: 'not_measured',
      caveat_refs_json:
        '["caveat.business_metrics.no_accepted_outcomes_in_window"]',
    })
    expect(sitePassRate).toMatchObject({
      value: null,
      measurement_state: 'not_measured',
      caveat_refs_json:
        '["caveat.business_metrics.no_terminal_outcomes_in_window"]',
    })
  })
})
