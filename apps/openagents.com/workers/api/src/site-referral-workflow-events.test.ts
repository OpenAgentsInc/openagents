import { describe, expect, test } from 'vitest'

import {
  ReferralWorkflowEventValidationError,
  listReferralWorkflowEventsByAttribution,
  listReferralWorkflowEventsByOrder,
  listReferralWorkflowEventsBySite,
  listReferralWorkflowEventsBySource,
  recordReferralWorkflowEvent,
  type RecordReferralWorkflowEventInput,
} from './site-referral-workflow-events'

type StoredReferralWorkflowEvent = Readonly<{
  accepted_work_ref: string | null
  amount: number
  archived_at: string | null
  asset: 'none' | 'credits' | 'sats' | 'usd'
  created_at: string
  entitlement_ref: string | null
  event_kind:
    | 'paid_usage'
    | 'site_checkout'
    | 'l402_redemption'
    | 'accepted_outcome'
    | 'refund'
    | 'reversal'
    | 'eligibility_hold'
    | 'dispute_hold'
    | 'operator_adjustment'
  id: string
  idempotency_key: string
  metadata_json: string
  occurred_at: string
  paid_action_id: string | null
  payment_event_id: string | null
  payment_evidence_ref: string | null
  policy_state:
    | 'recorded'
    | 'eligible'
    | 'held'
    | 'disputed'
    | 'refunded'
    | 'reversed'
    | 'ignored'
  product_id: string | null
  public_invite_ref: string | null
  public_receipt_ref: string
  public_source_ref: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  related_event_id: string | null
  site_id: string | null
  site_version_id: string | null
  software_order_id: string | null
}>

class ReferralWorkflowEventStore {
  rows: Array<StoredReferralWorkflowEvent> = []
}

class ReferralWorkflowEventStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ReferralWorkflowEventStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.rows.find(
          item =>
            item.idempotency_key === idempotencyKey && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO referral_workflow_events')) {
      const idempotencyKey = String(this.values[1])

      if (
        !this.store.rows.some(row => row.idempotency_key === idempotencyKey)
      ) {
        this.store.rows.push({
          accepted_work_ref: this.values[16] as string | null,
          amount: Number(this.values[20]),
          archived_at: this.values[25] as string | null,
          asset: this.values[21] as StoredReferralWorkflowEvent['asset'],
          created_at: String(this.values[24]),
          entitlement_ref: this.values[15] as string | null,
          event_kind: this.values[2] as StoredReferralWorkflowEvent['event_kind'],
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[22]),
          occurred_at: String(this.values[23]),
          paid_action_id: this.values[12] as string | null,
          payment_event_id: this.values[13] as string | null,
          payment_evidence_ref: this.values[14] as string | null,
          policy_state:
            this.values[19] as StoredReferralWorkflowEvent['policy_state'],
          product_id: this.values[11] as string | null,
          public_invite_ref: this.values[7] as string | null,
          public_receipt_ref: String(this.values[18]),
          public_source_ref: String(this.values[6]),
          referral_attribution_id: String(this.values[3]),
          referral_invite_id: this.values[5] as string | null,
          referral_source_id: String(this.values[4]),
          related_event_id: this.values[17] as string | null,
          site_id: this.values[9] as string | null,
          site_version_id: this.values[10] as string | null,
          software_order_id: this.values[8] as string | null,
        })
      }

      return Promise.resolve({
        meta: { changes: 1 },
        results: [],
        success: true,
      } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM referral_workflow_events')) {
      const value = String(this.values[0])
      const key = this.query.includes('referral_attribution_id = ?')
        ? 'referral_attribution_id'
        : this.query.includes('referral_source_id = ?')
          ? 'referral_source_id'
          : this.query.includes('software_order_id = ?')
            ? 'software_order_id'
            : 'site_id'
      const limit = Number(this.values[1] ?? 100)
      const rows = this.store.rows
        .filter(row => row.archived_at === null && row[key] === value)
        .slice(0, limit)

      return Promise.resolve({
        results: rows as unknown as ReadonlyArray<T>,
        success: true,
      } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
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
}

const referralWorkflowEventDb = (
  store: ReferralWorkflowEventStore,
): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ReferralWorkflowEventStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const baseInput = {
  amount: 15,
  asset: 'credits',
  eventKind: 'site_checkout',
  id: 'referral_workflow_event_1',
  idempotencyKey: 'referral-event:checkout:1',
  metadata: {
    paymentHashRef: 'redacted',
    source: 'site_checkout',
  },
  occurredAt: '2026-06-05T12:00:00.000Z',
  paymentEventId: 'site_payment_event_1',
  paymentEvidenceRef: 'mdk_payment_proof_12345678',
  policyState: 'eligible',
  productId: 'site_product_otec_report',
  publicReceiptRef: 'site_payment_receipt_1',
  publicSourceRef: 'src_otec',
  referralAttributionId: 'referral_attribution_otec',
  referralSourceId: 'site_referral_source_otec',
  siteId: 'site_project_otec',
  siteVersionId: 'site_version_otec_2',
  softwareOrderId: 'software_order_otec',
} satisfies RecordReferralWorkflowEventInput

describe('referral workflow event ledger', () => {
  test('records paid workflow events idempotently by idempotency key', async () => {
    const store = new ReferralWorkflowEventStore()
    const db = referralWorkflowEventDb(store)
    const first = await recordReferralWorkflowEvent(db, baseInput)
    const second = await recordReferralWorkflowEvent(db, {
      ...baseInput,
      amount: 99,
      id: 'referral_workflow_event_duplicate',
      publicReceiptRef: 'site_payment_receipt_duplicate',
    })

    expect(store.rows).toHaveLength(1)
    expect(first).toMatchObject({
      amount: 15,
      id: 'referral_workflow_event_1',
      policyState: 'eligible',
    })
    expect(second).toEqual(first)
  })

  test('lists events by attribution, source, order, and site', async () => {
    const store = new ReferralWorkflowEventStore()
    const db = referralWorkflowEventDb(store)
    const checkout = await recordReferralWorkflowEvent(db, baseInput)
    const refund = await recordReferralWorkflowEvent(db, {
      ...baseInput,
      amount: 15,
      eventKind: 'refund',
      id: 'referral_workflow_event_refund_1',
      idempotencyKey: 'referral-event:refund:1',
      policyState: 'refunded',
      publicReceiptRef: 'site_payment_receipt_refund_1',
      relatedEventId: checkout.id,
    })

    await expect(
      listReferralWorkflowEventsByAttribution(
        db,
        'referral_attribution_otec',
      ),
    ).resolves.toHaveLength(2)
    await expect(
      listReferralWorkflowEventsBySource(db, 'site_referral_source_otec'),
    ).resolves.toHaveLength(2)
    await expect(
      listReferralWorkflowEventsByOrder(db, 'software_order_otec'),
    ).resolves.toHaveLength(2)
    await expect(
      listReferralWorkflowEventsBySite(db, 'site_project_otec'),
    ).resolves.toEqual([
      expect.objectContaining({
        eventKind: 'site_checkout',
        relatedEventId: null,
      }),
      expect.objectContaining({
        eventKind: 'refund',
        relatedEventId: refund.relatedEventId,
      }),
    ])
  })

  test('requires refund and reversal events to link a related event', async () => {
    await expect(
      recordReferralWorkflowEvent(referralWorkflowEventDb(new ReferralWorkflowEventStore()), {
        ...baseInput,
        eventKind: 'reversal',
        idempotencyKey: 'referral-event:reversal:missing-related',
        policyState: 'reversed',
      }),
    ).rejects.toBeInstanceOf(ReferralWorkflowEventValidationError)
  })

  test('rejects raw invoices, preimages, wallet, and provider material', async () => {
    await expect(
      recordReferralWorkflowEvent(referralWorkflowEventDb(new ReferralWorkflowEventStore()), {
        ...baseInput,
        idempotencyKey: 'referral-event:unsafe-payment-proof',
        paymentEvidenceRef: 'lnbc1000n1rawinvoice',
      }),
    ).rejects.toMatchObject({
      reason:
        'paymentEvidenceRef must be a public-safe ref, not raw payment, wallet, provider, or secret material.',
    })

    await expect(
      recordReferralWorkflowEvent(referralWorkflowEventDb(new ReferralWorkflowEventStore()), {
        ...baseInput,
        idempotencyKey: 'referral-event:unsafe-metadata',
        metadata: {
          paymentPreimage: 'payment_preimage=abc123',
        },
      }),
    ).rejects.toBeInstanceOf(ReferralWorkflowEventValidationError)
  })
})
