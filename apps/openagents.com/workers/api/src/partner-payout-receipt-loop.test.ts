import { describe, expect, test } from 'vitest'

import { projectMdkPayoutModeGate } from './mdk-payout-mode-gate'
import { dispatchPartnerPayoutSettlement } from './partner-payout-dispatch'
import {
  createPartnerPayoutEligibility,
  type PartnerPayoutAsset,
  type PartnerPayoutRole,
  type PartnerPayoutState,
} from './partner-payout-ledger'
import { makeD1PartnerPayoutReceiptStore } from './partner-payout-receipts'
import {
  PartnerPayoutStagingAdapterError,
  makePartnerPayoutStagingAdapter,
  partnerPayoutStagingReceiptRef,
} from './partner-payout-staging-adapter'

type StoredPayoutEntry = Readonly<{
  amount: number
  archived_at: string | null
  asset: PartnerPayoutAsset
  beneficiary_user_id: string | null
  caveat_refs_json: string
  created_at: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  partner_ref: string
  partner_role: PartnerPayoutRole
  partner_user_id: string
  payout_ref: string
  period_key: string
  policy_refs_json: string
  previous_entry_id: string | null
  qualifying_amount: number
  qualifying_event_kind: string
  qualifying_event_ref: string
  reversal_of_entry_id: string | null
  state: PartnerPayoutState
  state_reason_ref: string | null
}>

class PayoutStore {
  rows: Array<StoredPayoutEntry> = []
}

const likeContains = (pattern: string): string => {
  const trimmed = pattern.startsWith('%') ? pattern.slice(1) : pattern
  return trimmed.endsWith('%') ? trimmed.slice(0, -1) : trimmed
}

class PayoutStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: PayoutStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const row =
        this.store.rows.find(
          entry =>
            entry.idempotency_key === String(this.values[0]) &&
            entry.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WHERE payout_ref = ?')) {
      const row =
        this.store.rows
          .filter(
            entry =>
              entry.payout_ref === String(this.values[0]) &&
              entry.archived_at === null,
          )
          .sort((left, right) =>
            right.created_at === left.created_at
              ? right.id.localeCompare(left.id)
              : right.created_at.localeCompare(left.created_at),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('COUNT(*) AS payout_count')) {
      const [partnerUserId, periodKey, asset] = this.values
      const rows = this.store.rows.filter(
        entry =>
          entry.partner_user_id === partnerUserId &&
          entry.period_key === periodKey &&
          entry.asset === asset &&
          entry.amount > 0 &&
          entry.archived_at === null &&
          ['eligible', 'approved', 'dispatched', 'settled'].includes(
            entry.state,
          ),
      )

      return Promise.resolve({
        payout_amount: rows.reduce((total, row) => total + row.amount, 0),
        payout_count: rows.length,
      } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO partner_payout_ledger_entries')) {
      this.store.rows.push({
        amount: Number(this.values[11]),
        archived_at: null,
        asset: this.values[7] as PartnerPayoutAsset,
        beneficiary_user_id: this.values[6] as string | null,
        caveat_refs_json: String(this.values[19]),
        created_at: String(this.values[20]),
        evidence_refs_json: String(this.values[17]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[2]),
        partner_ref: String(this.values[5]),
        partner_role: this.values[3] as PartnerPayoutRole,
        partner_user_id: String(this.values[4]),
        payout_ref: String(this.values[1]),
        period_key: String(this.values[12]),
        policy_refs_json: String(this.values[18]),
        previous_entry_id: this.values[15] as string | null,
        qualifying_amount: Number(this.values[10]),
        qualifying_event_kind: String(this.values[9]),
        qualifying_event_ref: String(this.values[8]),
        reversal_of_entry_id: this.values[16] as string | null,
        state: this.values[13] as PartnerPayoutState,
        state_reason_ref: this.values[14] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes('FROM partner_payout_ledger_entries') &&
      this.query.includes("state = 'settled'") &&
      this.query.includes('evidence_refs_json LIKE ?')
    ) {
      const needle = likeContains(String(this.values[0]))
      const results = this.store.rows
        .filter(
          entry =>
            entry.state === 'settled' &&
            entry.archived_at === null &&
            entry.evidence_refs_json.includes(needle),
        )
        .sort((left, right) =>
          right.created_at === left.created_at
            ? right.id.localeCompare(left.id)
            : right.created_at.localeCompare(left.created_at),
        )
        .slice(0, 10)
        .map(entry => ({
          amount: entry.amount,
          asset: entry.asset,
          caveat_refs_json: entry.caveat_refs_json,
          evidence_refs_json: entry.evidence_refs_json,
          policy_refs_json: entry.policy_refs_json,
          qualifying_event_kind: entry.qualifying_event_kind,
        }))

      return Promise.resolve({
        results: results as Array<T>,
        success: true,
      } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
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

const payoutDb = (store: PayoutStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new PayoutStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const readyGate = () =>
  Promise.resolve(
    projectMdkPayoutModeGate({
      hostedFundedKeyVerified: true,
      hostedProgrammaticPayoutsEnabled: true,
      requestedMode: 'hosted_mdk_direct_payout',
    }),
  )

const seedEligibleSatsPayout = (store: PayoutStore) =>
  createPartnerPayoutEligibility(payoutDb(store), {
    asset: 'sats',
    beneficiaryUserId: 'github:buyer',
    idempotencyKey: 'partner-payout:receipt-loop:eligible',
    nowIso: '2026-06-29T12:00:00.000Z',
    partnerRef: 'partner_agreement_receipt_loop',
    partnerRole: 'affiliate',
    partnerUserId: 'github:partner',
    payoutRef: 'partner_payout_receipt_loop',
    periodKey: '2026-06',
    qualifyingAmount: 5000,
    qualifyingEventKind: 'stripe_credit_purchase_paid',
    qualifyingEventRef: 'event.stripe_checkout.receipt_loop',
  })

describe('partner payout staging-test settlement-receipt closed loop (#7021)', () => {
  test('dispatch (staging adapter) -> settled D1 row -> public receipt dereferences', async () => {
    const store = new PayoutStore()
    const db = payoutDb(store)
    const eligible = await seedEligibleSatsPayout(store)
    const adapter = makePartnerPayoutStagingAdapter({ enabled: true })

    const outcome = await dispatchPartnerPayoutSettlement(
      db,
      {
        adapter,
        nowIso: () => '2026-06-29T12:05:00.000Z',
        readReadiness: readyGate,
      },
      { payoutRef: eligible.payoutRef },
    )

    if (outcome._tag !== 'settled') {
      throw new Error(`expected settled, got ${outcome._tag}`)
    }

    const expectedReceiptRef = await partnerPayoutStagingReceiptRef(
      eligible.payoutRef,
      eligible.amount,
    )
    expect(outcome.receiptRef).toBe(expectedReceiptRef)
    expect(outcome.receiptRef).toMatch(
      /^receipt\.partner_payout\.staging_test\./,
    )
    expect(outcome.entry.evidenceRefs).toContain(outcome.receiptRef)

    const receipt = await makeD1PartnerPayoutReceiptStore(
      db,
    ).readPartnerPayoutReceipt(outcome.receiptRef, '2026-06-29T12:06:00.000Z')

    expect(receipt).toMatchObject({
      amount: eligible.amount,
      asset: 'sats',
      qualifyingEventKind: 'stripe_credit_purchase_paid',
      receiptRef: outcome.receiptRef,
      resolution: {
        settlementRail: 'staging_test',
        state: 'settled',
        status: 'ok',
      },
      schemaVersion: 'openagents.partner_payout_receipt.v1',
    })
    expect(receipt?.evidenceRefs).toContain(outcome.receiptRef)

    const serialized = JSON.stringify(receipt).toLowerCase()
    for (const banned of [
      '"partnerref"',
      '"partneruserid"',
      '"beneficiaryuserid"',
      '"payoutref"',
      'lnbc',
      'preimage',
      'payment_hash',
      'private_key',
    ]) {
      expect(serialized).not.toContain(banned)
    }
  })

  test('idempotent: re-driving dispatch settles at most once and keeps the same receipt', async () => {
    const store = new PayoutStore()
    const db = payoutDb(store)
    const eligible = await seedEligibleSatsPayout(store)
    const deps = {
      adapter: makePartnerPayoutStagingAdapter({ enabled: true }),
      nowIso: () => '2026-06-29T12:05:00.000Z',
      readReadiness: readyGate,
    }

    const first = await dispatchPartnerPayoutSettlement(db, deps, {
      payoutRef: eligible.payoutRef,
    })
    const second = await dispatchPartnerPayoutSettlement(db, deps, {
      payoutRef: eligible.payoutRef,
    })

    expect(first._tag).toBe('settled')
    expect(second._tag).toBe('already_settled')
    expect(store.rows.filter(row => row.state === 'settled')).toHaveLength(1)

    const expectedReceiptRef = await partnerPayoutStagingReceiptRef(
      eligible.payoutRef,
      eligible.amount,
    )
    const receipt = await makeD1PartnerPayoutReceiptStore(
      db,
    ).readPartnerPayoutReceipt(expectedReceiptRef, '2026-06-29T12:06:00.000Z')

    expect(receipt?.receiptRef).toBe(expectedReceiptRef)
    expect(receipt?.resolution.state).toBe('settled')
  })

  test('fail-safe: disabled staging adapter records no settled state and no receipt', async () => {
    const store = new PayoutStore()
    const db = payoutDb(store)
    const eligible = await seedEligibleSatsPayout(store)

    await expect(
      dispatchPartnerPayoutSettlement(
        db,
        {
          adapter: makePartnerPayoutStagingAdapter({ enabled: false }),
          nowIso: () => '2026-06-29T12:05:00.000Z',
          readReadiness: readyGate,
        },
        { payoutRef: eligible.payoutRef },
      ),
    ).rejects.toMatchObject({ reason: 'partner_payout_adapter_dispatch_failed' })

    expect(store.rows.some(row => row.state === 'settled')).toBe(false)

    const wouldBeReceiptRef = await partnerPayoutStagingReceiptRef(
      eligible.payoutRef,
      eligible.amount,
    )
    const receipt = await makeD1PartnerPayoutReceiptStore(
      db,
    ).readPartnerPayoutReceipt(wouldBeReceiptRef, '2026-06-29T12:06:00.000Z')

    expect(receipt).toBeNull()
  })

  test('staging adapter throws tagged fail-closed error when disabled', async () => {
    const adapter = makePartnerPayoutStagingAdapter({ enabled: false })

    await expect(
      adapter.dispatch({
        amountSats: 500,
        idempotencyKey: 'partner_payout.adapter.test',
        payoutRef: 'partner_payout_receipt_loop',
      }),
    ).rejects.toBeInstanceOf(PartnerPayoutStagingAdapterError)
  })

  test('staging receipt ref is deterministic and public-safe', async () => {
    const a = await partnerPayoutStagingReceiptRef('partner_payout_x', 500)
    const b = await partnerPayoutStagingReceiptRef('partner_payout_x', 500)
    const c = await partnerPayoutStagingReceiptRef('partner_payout_x', 501)

    expect(a).toBe(b)
    expect(a).not.toBe(c)
    expect(a).toMatch(/^receipt\.partner_payout\.staging_test\.[a-f0-9]{32}$/)
  })
})
