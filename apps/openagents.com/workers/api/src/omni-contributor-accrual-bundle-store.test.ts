import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  dereferenceOmniContributorAccrualBundle,
  OmniContributorAccrualBundleDereferenceError,
} from './omni-contributor-accrual-bundle-store'

type EconomicsRow = Readonly<{
  accepted_outcome_contract_id: string | null
  accepted_value_cents: number
  archived_at: string | null
  artifact_cost_cents: number
  buyer_price_asset: 'none' | 'usd' | 'credits' | 'sats'
  buyer_price_cents: number
  created_at: string
  credits_charged: number
  funding_mode: 'free_beta' | 'credit_funded' | 'sats_funded' | 'internal_only'
  gross_margin_cents: number
  id: string
  idempotency_key: string
  internal_caveat_ref: string | null
  metadata_json: string
  no_settlement_implication: number
  provider_cost_cents: number
  public_caveat_ref: string
  retry_cost_cents: number
  review_cost_cents: number
  review_minutes: number
  runner_cost_cents: number
  sats_charged: number
  total_cost_cents: number
  updated_at: string
  work_kind: 'site' | 'coding'
  workroom_id: string
}>

const makeRow = (overrides: Partial<EconomicsRow> = {}): EconomicsRow => ({
  accepted_outcome_contract_id: null,
  accepted_value_cents: 5000,
  archived_at: null,
  artifact_cost_cents: 0,
  buyer_price_asset: 'none',
  buyer_price_cents: 0,
  created_at: '2026-06-20T00:00:00.000Z',
  credits_charged: 0,
  funding_mode: 'free_beta',
  gross_margin_cents: 2600,
  id: 'omni_outcome_economics_1',
  idempotency_key: 'omni-economics:site-1',
  internal_caveat_ref: null,
  metadata_json: JSON.stringify({
    contributors: { reviewerId: 'reviewer_1', runnerId: 'runner_1' },
  }),
  no_settlement_implication: 1,
  provider_cost_cents: 0,
  public_caveat_ref: 'economics_public_caveat_internal_only',
  retry_cost_cents: 0,
  review_cost_cents: 0,
  review_minutes: 0,
  runner_cost_cents: 2400,
  sats_charged: 0,
  total_cost_cents: 2400,
  updated_at: '2026-06-20T00:00:00.000Z',
  work_kind: 'site',
  workroom_id: 'omni_workroom_site_1',
  ...overrides,
})

class EconomicsByIdStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly rows: ReadonlyArray<EconomicsRow>,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (
      this.query.includes('FROM omni_accepted_outcome_economics') &&
      this.query.includes('WHERE id = ?')
    ) {
      const id = String(this.values[0])
      const row =
        this.rows.find(
          item => item.id === id && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error('run should not be used'))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const dbWith = (rows: ReadonlyArray<EconomicsRow>): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new EconomicsByIdStatement(query, rows),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const dereference = (rows: ReadonlyArray<EconomicsRow>, id: string) =>
  Effect.runPromise(dereferenceOmniContributorAccrualBundle(dbWith(rows), id))

describe('dereferenceOmniContributorAccrualBundle', () => {
  test('dereferences a stored record into a reconciled bundle by id', async () => {
    const bundle = await dereference([makeRow()], 'omni_outcome_economics_1')

    expect(bundle).not.toBeNull()
    expect(bundle?.economicsId).toBe('omni_outcome_economics_1')
    // Receipt and ledger reconcile to the same single derived gross margin.
    expect(bundle?.reconciledGrossMarginCents).toBe(2600)
    expect(bundle?.contributorAccrualLedger.grossMarginCents).toBe(2600)
    // No margin invented or lost across the attribution.
    expect(bundle?.contributorAccrualLedger.totalAccruedCents).toBe(2600)
    expect(bundle?.contributorAccrualLedger.distributableMarginCents).toBe(2600)
    // The sourced parties became attributed contributors.
    const contributorIds =
      bundle?.contributorAccrualLedger.entries.map(
        entry => entry.contributorId,
      ) ?? []
    expect(contributorIds).toContain('runner_1')
    expect(contributorIds).toContain('reviewer_1')
  })

  test('keeps settlement disclaimed across both halves of the bundle', async () => {
    const bundle = await dereference([makeRow()], 'omni_outcome_economics_1')

    expect(bundle?.grossMarginReceipt.noSettlementImplication).toBe(true)
    expect(bundle?.contributorAccrualLedger.noSettlementImplication).toBe(true)
    for (const entry of bundle?.contributorAccrualLedger.entries ?? []) {
      expect(entry.payableEvidenceState).toBe('not_yet_evidenced')
      expect(entry.settlementEvidenceState).toBe('not_yet_evidenced')
    }
  })

  test('returns null for an unknown accepted-outcome id', async () => {
    const bundle = await dereference([makeRow()], 'omni_outcome_economics_missing')

    expect(bundle).toBeNull()
  })

  test('returns null for an archived record (read-only, archived excluded)', async () => {
    const bundle = await dereference(
      [makeRow({ archived_at: '2026-06-20T01:00:00.000Z' })],
      'omni_outcome_economics_1',
    )

    expect(bundle).toBeNull()
  })

  test('fails honestly when a stored record names no contributor parties', async () => {
    await expect(
      dereference(
        [makeRow({ metadata_json: JSON.stringify({}) })],
        'omni_outcome_economics_1',
      ),
    ).rejects.toBeInstanceOf(OmniContributorAccrualBundleDereferenceError)
  })

  test('is deterministic: same stored record yields an identical bundle', async () => {
    const rows = [makeRow()]
    const first = await dereference(rows, 'omni_outcome_economics_1')
    const second = await dereference(rows, 'omni_outcome_economics_1')

    expect(first).toEqual(second)
  })
})
