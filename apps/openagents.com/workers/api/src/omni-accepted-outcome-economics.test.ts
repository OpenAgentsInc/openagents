import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniAcceptedOutcomeEconomicsContractNotFound,
  OmniAcceptedOutcomeEconomicsValidationError,
  OmniAcceptedOutcomeEconomicsWorkroomNotFound,
  operatorOmniAcceptedOutcomeEconomicsProjection,
  publicOmniAcceptedOutcomeEconomicsProjection,
  recordOmniAcceptedOutcomeEconomics,
} from './omni-accepted-outcome-economics'

type RefRow = Readonly<{ archived_at: string | null; id: string }>
type WorkroomRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind:
    | 'site'
    | 'coding'
    | 'adjustment'
    | 'existing_project_import'
    | 'business'
    | 'legal_sensitive'
}>

type EconomicsRow = Readonly<{
  accepted_outcome_contract_id: string | null
  accepted_value_cents: number
  archived_at: string | null
  artifact_cost_cents: number
  buyer_price_asset: 'none' | 'usd' | 'credits' | 'sats'
  buyer_price_cents: number
  created_at: string
  credits_charged: number
  funding_mode:
    | 'free_beta'
    | 'credit_funded'
    | 'sats_funded'
    | 'internal_only'
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
  work_kind: WorkroomRow['work_kind']
  workroom_id: string
}>

class EconomicsStore {
  contracts: Array<RefRow> = [
    { archived_at: null, id: 'omni_accepted_outcome_contract_1' },
  ]
  economics: Array<EconomicsRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
  ]
}

const runtime = {
  makeEconomicsId: () => 'omni_outcome_economics_generated',
  nowIso: () => '2026-06-06T01:45:00.000Z',
}

class EconomicsStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: EconomicsStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_accepted_outcome_economics')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.economics.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM omni_workrooms')) {
      return Promise.resolve(this.findWorkroom() as T | null)
    }

    if (this.query.includes('FROM omni_accepted_outcome_contracts')) {
      const id = String(this.values[0])
      const row =
        this.store.contracts.find(
          item => item.id === id && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO omni_accepted_outcome_economics')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.economics.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.economics.push({
          accepted_outcome_contract_id: this.values[3] as string | null,
          accepted_value_cents: Number(this.values[17]),
          archived_at: null,
          artifact_cost_cents: Number(this.values[15]),
          buyer_price_asset: this.values[6] as EconomicsRow['buyer_price_asset'],
          buyer_price_cents: Number(this.values[7]),
          created_at: String(this.values[23]),
          credits_charged: Number(this.values[8]),
          funding_mode: this.values[5] as EconomicsRow['funding_mode'],
          gross_margin_cents: Number(this.values[18]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          internal_caveat_ref: this.values[20] as string | null,
          metadata_json: String(this.values[22]),
          no_settlement_implication: Number(this.values[21]),
          provider_cost_cents: Number(this.values[11]),
          public_caveat_ref: String(this.values[19]),
          retry_cost_cents: Number(this.values[12]),
          review_cost_cents: Number(this.values[14]),
          review_minutes: Number(this.values[13]),
          runner_cost_cents: Number(this.values[10]),
          sats_charged: Number(this.values[9]),
          total_cost_cents: Number(this.values[16]),
          updated_at: String(this.values[24]),
          work_kind: this.values[4] as EconomicsRow['work_kind'],
          workroom_id: String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
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
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }

  private findWorkroom(): WorkroomRow | null {
    const id = String(this.values[0])

    return (
      this.store.workrooms.find(
        item => item.id === id && item.archived_at === null,
      ) ?? null
    )
  }
}

const economicsDb = (store: EconomicsStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new EconomicsStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const recordEconomics = (
  store: EconomicsStore,
  overrides: Partial<
    Parameters<typeof recordOmniAcceptedOutcomeEconomics>[1]
  > = {},
) =>
  Effect.runPromise(
    recordOmniAcceptedOutcomeEconomics(
      economicsDb(store),
      {
        acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
        acceptedValueCents: 5000,
        artifactCostCents: 100,
        buyerPriceAsset: 'none',
        fundingMode: 'free_beta',
        id: 'omni_outcome_economics_1',
        idempotencyKey: 'omni-economics:site-1',
        providerCostCents: 750,
        publicCaveatRef: 'economics_public_caveat_internal_only',
        retryCostCents: 50,
        reviewCostCents: 600,
        reviewMinutes: 12,
        runnerCostCents: 900,
        workKind: 'site',
        workroomId: 'omni_workroom_site_1',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni accepted outcome economics', () => {
  test('records free-beta economics idempotently with derived cost and margin', async () => {
    const store = new EconomicsStore()
    const economics = await recordEconomics(store)
    const replay = await recordEconomics(store, {
      acceptedValueCents: 100,
    })
    const publicProjection = publicOmniAcceptedOutcomeEconomicsProjection(
      economics,
    )
    const operator = operatorOmniAcceptedOutcomeEconomicsProjection(economics)

    expect(replay.acceptedValueCents).toBe(5000)
    expect(economics.totalCostCents).toBe(2400)
    expect(economics.grossMarginCents).toBe(2600)
    expect(publicProjection).toEqual({
      fundingMode: 'free_beta',
      noSettlementImplication: true,
      publicCaveatRef: 'economics_public_caveat_internal_only',
      workKind: 'site',
      workroomId: 'omni_workroom_site_1',
    })
    expect(operator.providerCostCents).toBe(750)
  })

  test('records credit-funded and sats-funded accepted outcomes', async () => {
    await expect(
      recordEconomics(new EconomicsStore(), {
        buyerPriceAsset: 'credits',
        buyerPriceCents: 2500,
        creditsCharged: 2500,
        fundingMode: 'credit_funded',
        id: 'omni_outcome_economics_credit_1',
        idempotencyKey: 'omni-economics:credit-1',
      }),
    ).resolves.toMatchObject({
      creditsCharged: 2500,
      fundingMode: 'credit_funded',
      satsCharged: 0,
    })

    await expect(
      recordEconomics(new EconomicsStore(), {
        buyerPriceAsset: 'sats',
        buyerPriceCents: 0,
        fundingMode: 'sats_funded',
        id: 'omni_outcome_economics_sats_1',
        idempotencyKey: 'omni-economics:sats-1',
        satsCharged: 10000,
        workKind: 'coding',
        workroomId: 'omni_workroom_coding_1',
      }),
    ).resolves.toMatchObject({
      fundingMode: 'sats_funded',
      satsCharged: 10000,
      workKind: 'coding',
    })
  })

  test('rejects invalid funding modes and negative or non-integer math', async () => {
    await expect(
      recordEconomics(new EconomicsStore(), {
        buyerPriceAsset: 'usd',
        buyerPriceCents: 1,
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsValidationError)

    await expect(
      recordEconomics(new EconomicsStore(), {
        fundingMode: 'credit_funded',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsValidationError)

    await expect(
      recordEconomics(new EconomicsStore(), {
        providerCostCents: -1,
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsValidationError)
  })

  test('rejects missing linked refs, work-kind mismatches, and settlement material', async () => {
    await expect(
      recordEconomics(new EconomicsStore(), {
        workroomId: 'omni_workroom_missing',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsWorkroomNotFound)

    await expect(
      recordEconomics(new EconomicsStore(), {
        acceptedOutcomeContractId: 'omni_contract_missing',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsContractNotFound)

    await expect(
      recordEconomics(new EconomicsStore(), {
        workKind: 'coding',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsValidationError)

    await expect(
      recordEconomics(new EconomicsStore(), {
        metadata: { settlement: 'eligible_for_payout' },
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsValidationError)

    await expect(
      recordEconomics(new EconomicsStore(), {
        publicCaveatRef: 'payout_settlement_claim_1',
      }),
    ).rejects.toBeInstanceOf(OmniAcceptedOutcomeEconomicsValidationError)
  })
})
