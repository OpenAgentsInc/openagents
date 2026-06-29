import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  listAdjutantUsageReceiptsForAssignment,
  listCustomerAdjutantUsageReceiptsForOrder,
  recordAdjutantUsageReceipt,
  summarizeAdjutantUsageReceipts,
} from './adjutant-usage-receipts'

type StoredUsageReceipt = Readonly<{
  adjustment_id: string | null
  assignment_id: string
  billing_ledger_entry_id: string | null
  billing_mode: 'public_beta_free' | 'paid_credits'
  category: 'generation' | 'build' | 'hosting' | 'storage' | 'adjustment'
  created_at: string
  credits_charged_cents: number
  currency: string
  id: string
  idempotency_key: string
  public_receipt_json: string
  quantity: number
  run_id: string | null
  site_id: string | null
  software_order_id: string | null
  summary: string
  team_receipt_json: string
  unit: string
  visibility: 'private' | 'team' | 'public'
}>

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: {} as D1Meta & Record<string, unknown>,
  results,
  success: true,
})

class UsageReceiptStore {
  receipts: Array<StoredUsageReceipt> = []
}

class UsageReceiptStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: UsageReceiptStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM adjutant_usage_receipts')) {
      const [idempotencyKey] = this.values
      const receipt = this.store.receipts.find(
        item => item.idempotency_key === idempotencyKey,
      )

      return Promise.resolve((receipt as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO adjutant_usage_receipts')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        adjustmentId,
        runId,
        category,
        visibility,
        billingMode,
        summary,
        quantity,
        unit,
        creditsChargedCents,
        currency,
        billingLedgerEntryId,
        publicReceiptJson,
        teamReceiptJson,
        idempotencyKey,
        createdAt,
      ] = this.values

      if (
        typeof idempotencyKey === 'string' &&
        !this.store.receipts.some(
          receipt => receipt.idempotency_key === idempotencyKey,
        )
      ) {
        this.store.receipts.push({
          adjustment_id: typeof adjustmentId === 'string' ? adjustmentId : null,
          assignment_id: String(assignmentId),
          billing_ledger_entry_id:
            typeof billingLedgerEntryId === 'string'
              ? billingLedgerEntryId
              : null,
          billing_mode:
            billingMode === 'paid_credits'
              ? 'paid_credits'
              : 'public_beta_free',
          category: category as StoredUsageReceipt['category'],
          created_at: String(createdAt),
          credits_charged_cents: Number(creditsChargedCents ?? 0),
          currency: String(currency),
          id: String(id),
          idempotency_key: idempotencyKey,
          public_receipt_json: String(publicReceiptJson),
          quantity: Number(quantity ?? 0),
          run_id: typeof runId === 'string' ? runId : null,
          site_id: typeof siteId === 'string' ? siteId : null,
          software_order_id:
            typeof softwareOrderId === 'string' ? softwareOrderId : null,
          summary: String(summary),
          team_receipt_json: String(teamReceiptJson),
          unit: String(unit),
          visibility: visibility as StoredUsageReceipt['visibility'],
        })
      }

      return Promise.resolve(makeResult<T>())
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes('FROM adjutant_usage_receipts') &&
      this.query.includes('assignment_id = ?')
    ) {
      const [assignmentId, limit] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.receipts
            .filter(receipt => receipt.assignment_id === assignmentId)
            .slice(0, Number(limit ?? 50)) as Array<T>,
        ),
      )
    }

    if (
      this.query.includes('FROM adjutant_usage_receipts') &&
      this.query.includes('software_order_id = ?')
    ) {
      const [softwareOrderId, limit] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.receipts
            .filter(
              receipt =>
                receipt.software_order_id === softwareOrderId &&
                receipt.visibility === 'public',
            )
            .slice(0, Number(limit ?? 50)) as Array<T>,
        ),
      )
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

const usageDb = (store: UsageReceiptStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new UsageReceiptStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime = {
  makeReceiptId: () => 'adjutant_usage_receipt_static',
  nowIso: () => '2026-06-05T12:00:00.000Z',
}

describe('Adjutant usage receipts', () => {
  test('records public beta receipts idempotently and summarizes categories', async () => {
    const store = new UsageReceiptStore()
    const db = usageDb(store)
    const input = {
      assignmentId: 'adjutant_assignment_1',
      category: 'generation' as const,
      idempotencyKey: 'adjutant_usage:one',
      publicDetails: {
        billingNote: 'Public beta Site generation is free.',
      },
      quantity: 1,
      runId: 'agent_run_internal',
      siteId: 'site_project_1',
      softwareOrderId: 'software_order_1',
      summary: 'Autopilot Site generation run was queued.',
      teamDetails: {
        runId: 'agent_run_internal',
      },
      unit: 'run',
      visibility: 'public' as const,
    }

    const first = await Effect.runPromise(
      recordAdjutantUsageReceipt(db, input, runtime),
    )
    const retry = await Effect.runPromise(
      recordAdjutantUsageReceipt(db, input, runtime),
    )
    const receipts = await Effect.runPromise(
      listAdjutantUsageReceiptsForAssignment(db, 'adjutant_assignment_1'),
    )
    const customerReceipts = await Effect.runPromise(
      listCustomerAdjutantUsageReceiptsForOrder(db, 'software_order_1'),
    )

    expect(retry.id).toBe(first.id)
    expect(receipts).toHaveLength(1)
    expect(customerReceipts).toEqual([
      expect.objectContaining({
        billingMode: 'public_beta_free',
        category: 'generation',
        creditsChargedFormatted: '$0.00',
        details: {
          billingNote: 'Public beta Site generation is free.',
        },
      }),
    ])
    expect(JSON.stringify(customerReceipts)).not.toContain('agent_run_internal')
    expect(summarizeAdjutantUsageReceipts(customerReceipts)).toMatchObject({
      billingMode: 'public_beta_free',
      categories: [
        expect.objectContaining({
          category: 'generation',
          quantity: 1,
          unit: 'run',
        }),
      ],
      totalCreditsChargedCents: 0,
    })
  })

  test('requires billing ledger linkage for paid credits and forbids public beta charges', async () => {
    const db = usageDb(new UsageReceiptStore())
    const baseInput = {
      assignmentId: 'adjutant_assignment_1',
      category: 'hosting' as const,
      idempotencyKey: 'adjutant_usage:hosting',
      quantity: 1,
      summary: 'Autopilot activated public Site hosting.',
      unit: 'deployment',
      visibility: 'public' as const,
    }

    await expect(
      Effect.runPromise(
        recordAdjutantUsageReceipt(
          db,
          {
            ...baseInput,
            billingMode: 'public_beta_free',
            creditsChargedCents: 1,
          },
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantUsageReceiptUnsafe',
    })

    await expect(
      Effect.runPromise(
        recordAdjutantUsageReceipt(
          db,
          {
            ...baseInput,
            billingMode: 'paid_credits',
            creditsChargedCents: 100,
            idempotencyKey: 'adjutant_usage:paid',
          },
          runtime,
        ),
      ),
    ).rejects.toMatchObject({
      _tag: 'AdjutantUsageReceiptUnsafe',
    })
  })
})
