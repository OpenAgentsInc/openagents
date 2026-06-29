import { describe, expect, test } from 'vitest'

import { projectMdkPayoutModeGate } from './mdk-payout-mode-gate'
import { dispatchReferralPayoutSettlement } from './site-referral-payout-dispatch'
import { recordReferralPayoutForPaidEvent } from './site-referral-payout-feed'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'
import { makeD1SiteReferralPayoutReceiptStore } from './site-referral-payout-receipts'
import {
  SiteReferralPayoutStagingAdapterError,
  makeSiteReferralPayoutStagingAdapter,
  stagingTestReceiptRef,
} from './site-referral-payout-staging-adapter'

// CLOSED-LOOP end-to-end proof for #5524 / DE-1 (sites.referral_bitcoin_stream.v1).
//
// The pre-existing wire test (site-referral-payout-wire.test.ts) proves the
// dispatcher settles a row through a MOCK adapter, and the public receipt-route
// test (public-site-referral-payout-receipt-routes.test.ts) proves the route
// serves a settled receipt from a MOCK store. NEITHER connects the two: there is
// no test that drives feed -> dispatch -> a real settled D1 row -> and then
// dereferences the receipt the dispatch produced through the REAL public receipt
// store. That gap is exactly the `referral_settlement_receipts_missing` blocker:
// the settlement-receipt surface had never been proven dereferenceable against a
// real settled row produced by the real dispatch path.
//
// This suite closes that loop in STAGING/TEST mode using the staging-test
// adapter, which produces a real-shaped, public-safe `staging_test` receipt ref
// and moves NO money. It proves the receipt the dispatch settled with is
// genuinely dereferenceable through `makeD1SiteReferralPayoutReceiptStore`,
// while the real owner-armed Bitcoin payout (`referral_first_real_payout_pending`)
// stays out of scope and unarmed.

type StoredPayoutEntry = Readonly<{
  amount_sats: number
  archived_at: string | null
  caveat_refs_json: string
  created_at: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  payout_ref: string
  period_key: string
  policy_refs_json: string
  previous_entry_id: string | null
  qualifying_amount_sats: number
  qualifying_event_kind: string
  qualifying_event_ref: string
  referred_user_id: string | null
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
  reversal_of_entry_id: string | null
  state: SiteReferralPayoutState
  state_reason_ref: string | null
}>

type AttributionJoinRow = Readonly<{
  user_id: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
  ura_active: boolean
  src_active: boolean
}>

class LoopStore {
  rows: Array<StoredPayoutEntry> = []
  attributions: Array<AttributionJoinRow> = []
}

const likeContains = (pattern: string): string => {
  // The receipt store binds `%${receiptRef}%`. Strip the surrounding wildcards
  // to get the substring the LIKE is testing for.
  const trimmed = pattern.startsWith('%') ? pattern.slice(1) : pattern
  return trimmed.endsWith('%') ? trimmed.slice(0, -1) : trimmed
}

class LoopStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: LoopStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM user_referral_attributions')) {
      const userId = String(this.values[0])
      const row =
        this.store.attributions.find(
          entry =>
            entry.user_id === userId && entry.ura_active && entry.src_active,
        ) ?? null

      return Promise.resolve(
        row === null
          ? null
          : ({
              referral_attribution_id: row.referral_attribution_id,
              referral_invite_id: row.referral_invite_id,
              referral_source_id: row.referral_source_id,
              referrer_user_id: row.referrer_user_id,
            } as T),
      )
    }

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
      const [referrerUserId, periodKey] = this.values
      const rows = this.store.rows.filter(
        entry =>
          entry.referrer_user_id === referrerUserId &&
          entry.period_key === periodKey &&
          entry.amount_sats > 0 &&
          entry.archived_at === null &&
          ['eligible', 'approved', 'dispatched', 'settled'].includes(
            entry.state,
          ),
      )

      return Promise.resolve({
        payout_count: rows.length,
        payout_sats: rows.reduce((total, row) => total + row.amount_sats, 0),
      } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO site_referral_payout_ledger_entries')) {
      this.store.rows.push({
        amount_sats: Number(this.values[11]),
        archived_at: null,
        caveat_refs_json: String(this.values[19]),
        created_at: String(this.values[20]),
        evidence_refs_json: String(this.values[17]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[2]),
        payout_ref: String(this.values[1]),
        period_key: String(this.values[12]),
        policy_refs_json: String(this.values[18]),
        previous_entry_id: this.values[15] as string | null,
        qualifying_amount_sats: Number(this.values[10]),
        qualifying_event_kind: String(this.values[9]),
        qualifying_event_ref: String(this.values[8]),
        referred_user_id: this.values[7] as string | null,
        referral_attribution_id: String(this.values[3]),
        referral_invite_id: this.values[5] as string | null,
        referral_source_id: String(this.values[4]),
        referrer_user_id: String(this.values[6]),
        reversal_of_entry_id: this.values[16] as string | null,
        state: this.values[13] as SiteReferralPayoutState,
        state_reason_ref: this.values[14] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    // The receipt store's settled-row read:
    //   SELECT ... FROM site_referral_payout_ledger_entries
    //    WHERE state = 'settled' AND archived_at IS NULL
    //      AND evidence_refs_json LIKE ? ORDER BY created_at DESC, id DESC LIMIT 10
    if (
      this.query.includes('FROM site_referral_payout_ledger_entries') &&
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
          amount_sats: entry.amount_sats,
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

const loopDb = (store: LoopStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new LoopStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const seedAttribution = (store: LoopStore, userId: string): void => {
  store.attributions.push({
    referral_attribution_id: 'referral_attribution_loop_1',
    referral_invite_id: null,
    referral_source_id: 'site_referral_source_loop_1',
    referrer_user_id: 'github:loop-referrer',
    src_active: true,
    ura_active: true,
    user_id: userId,
  })
}

const readyGate = () =>
  Promise.resolve(
    projectMdkPayoutModeGate({
      hostedFundedKeyVerified: true,
      hostedProgrammaticPayoutsEnabled: true,
      requestedMode: 'hosted_mdk_direct_payout',
    }),
  )

const bitcoinPaidEvent = (userId: string) => ({
  idempotencyKey: `site_referral_payout.loop.${userId}`,
  nowIso: '2026-06-22T12:00:00.000Z',
  periodKey: '2026-06',
  qualifyingAmountSats: 2500,
  qualifyingEventKind: 'forum_tip_paid',
  qualifyingEventRef: `evidence.btc_paid.loop.${userId}`,
  revenueAsset: 'bitcoin' as const,
  userId,
})

describe('RL-1 staging-test settlement-receipt closed loop (#5524)', () => {
  test('feed -> dispatch (staging adapter) -> settled D1 row -> public receipt dereferences', async () => {
    const store = new LoopStore()
    seedAttribution(store, 'github:loop-buyer')
    const db = loopDb(store)

    // 1. A referred + paid Bitcoin event records an eligible payout row.
    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:loop-buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected the paid event to record an eligible row')
    }
    expect(fed.entry.state).toBe('eligible')

    // 2. Dispatch through the STAGING-TEST adapter (moves no money) with the
    //    readiness gate armed for staging. The row walks
    //    approved -> dispatched -> settled and records the staging receipt ref.
    const adapter = makeSiteReferralPayoutStagingAdapter({ enabled: true })
    const outcome = await dispatchReferralPayoutSettlement(
      db,
      {
        adapter,
        nowIso: () => '2026-06-22T12:05:00.000Z',
        readReadiness: readyGate,
      },
      { payoutRef: fed.entry.payoutRef, revenueAsset: 'bitcoin' },
    )

    if (outcome._tag !== 'settled') {
      throw new Error(`expected settled, got ${outcome._tag}`)
    }
    expect(outcome.entry.state).toBe('settled')

    const expectedReceiptRef = await stagingTestReceiptRef(
      fed.entry.payoutRef,
      fed.entry.amountSats,
    )
    expect(outcome.receiptRef).toBe(expectedReceiptRef)
    expect(outcome.receiptRef).toMatch(
      /^receipt\.site_referral_payout\.staging_test\./,
    )
    expect(outcome.entry.evidenceRefs).toContain(outcome.receiptRef)

    // 3. THE LOOP CLOSES: dereference the receipt the dispatch settled with
    //    through the REAL public receipt store. This is the dereferenceable
    //    settlement receipt that clears referral_settlement_receipts_missing.
    const receiptStore = makeD1SiteReferralPayoutReceiptStore(db)
    const receipt = await receiptStore.readSiteReferralPayoutReceipt(
      outcome.receiptRef,
      '2026-06-22T12:06:00.000Z',
    )

    expect(receipt).not.toBeNull()
    if (receipt === null) {
      throw new Error('expected the settled receipt to dereference')
    }

    expect(receipt).toMatchObject({
      amountSats: fed.entry.amountSats,
      attributionLinked: true,
      proofChain: {
        attribution: {
          linked: true,
          source: 'consume_once_referral_attribution',
        },
        eligibility: {
          source: 'site_referral_payout_ledger_entries',
          state: 'recorded',
        },
        paidEvent: {
          kind: 'forum_tip_paid',
          source: 'qualifying_event_kind',
        },
        settlement: {
          receiptRef: outcome.receiptRef,
          state: 'settled',
        },
      },
      qualifyingEventKind: 'forum_tip_paid',
      receiptRef: outcome.receiptRef,
      resolution: {
        settlementRail: 'staging_test',
        state: 'settled',
        status: 'ok',
      },
      schemaVersion: 'openagents.site_referral_payout_receipt.v1',
    })
    expect(receipt.evidenceRefs).toContain(outcome.receiptRef)
    expect(receipt.generatedAt).toBe('2026-06-22T12:06:00.000Z')

    // No private material crosses the public boundary.
    const serialized = JSON.stringify(receipt).toLowerCase()
    for (const banned of [
      '"payoutref"',
      '"referreruserid"',
      '"referreduserid"',
      'lnbc',
      'preimage',
      'payment_hash',
      'private_key',
    ]) {
      expect(serialized).not.toContain(banned)
    }
  })

  test('idempotent: re-driving the staging dispatch settles at most once and the SAME receipt dereferences', async () => {
    const store = new LoopStore()
    seedAttribution(store, 'github:loop-buyer')
    const db = loopDb(store)

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:loop-buyer'),
    )
    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const adapter = makeSiteReferralPayoutStagingAdapter({ enabled: true })
    const deps = {
      adapter,
      nowIso: () => '2026-06-22T12:05:00.000Z',
      readReadiness: readyGate,
    }
    const input = {
      payoutRef: fed.entry.payoutRef,
      revenueAsset: 'bitcoin' as const,
    }

    const first = await dispatchReferralPayoutSettlement(db, deps, input)
    const second = await dispatchReferralPayoutSettlement(db, deps, input)

    expect(first._tag).toBe('settled')
    expect(second._tag).toBe('already_settled')

    // Exactly one settled row exists, and the receipt still dereferences.
    const settledRows = store.rows.filter(row => row.state === 'settled')
    expect(settledRows).toHaveLength(1)

    const expectedReceiptRef = await stagingTestReceiptRef(
      fed.entry.payoutRef,
      fed.entry.amountSats,
    )
    const receipt = await makeD1SiteReferralPayoutReceiptStore(
      db,
    ).readSiteReferralPayoutReceipt(expectedReceiptRef, '2026-06-22T12:06:00.000Z')

    expect(receipt?.resolution.state).toBe('settled')
    expect(receipt?.receiptRef).toBe(expectedReceiptRef)
  })

  test('fail-safe: a DISABLED staging adapter never settles and records no settled state (no receipt to dereference)', async () => {
    const store = new LoopStore()
    seedAttribution(store, 'github:loop-buyer')
    const db = loopDb(store)

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:loop-buyer'),
    )
    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    // Default-OFF posture: the adapter fails closed (throws), the dispatcher
    // records NO settled state, and nothing is dereferenceable.
    const adapter = makeSiteReferralPayoutStagingAdapter({ enabled: false })

    await expect(
      dispatchReferralPayoutSettlement(
        db,
        {
          adapter,
          nowIso: () => '2026-06-22T12:05:00.000Z',
          readReadiness: readyGate,
        },
        { payoutRef: fed.entry.payoutRef, revenueAsset: 'bitcoin' },
      ),
    ).rejects.toMatchObject({ reason: 'site_referral_payout_adapter_dispatch_failed' })

    // No settled row, and the would-be receipt ref does not resolve.
    expect(store.rows.some(row => row.state === 'settled')).toBe(false)

    const wouldBeReceiptRef = await stagingTestReceiptRef(
      fed.entry.payoutRef,
      fed.entry.amountSats,
    )
    const receipt = await makeD1SiteReferralPayoutReceiptStore(
      db,
    ).readSiteReferralPayoutReceipt(wouldBeReceiptRef, '2026-06-22T12:06:00.000Z')
    expect(receipt).toBeNull()
  })

  test('staging adapter throws the tagged fail-closed error when disabled', async () => {
    const adapter = makeSiteReferralPayoutStagingAdapter({ enabled: false })

    await expect(
      adapter.dispatch({
        amountSats: 125,
        idempotencyKey: 'k',
        payoutRef: 'site_referral_payout_x',
      }),
    ).rejects.toBeInstanceOf(SiteReferralPayoutStagingAdapterError)
  })

  test('staging receipt ref is deterministic and public-safe', async () => {
    const a = await stagingTestReceiptRef('site_referral_payout_x', 125)
    const b = await stagingTestReceiptRef('site_referral_payout_x', 125)
    const c = await stagingTestReceiptRef('site_referral_payout_x', 126)

    expect(a).toBe(b)
    expect(a).not.toBe(c)
    expect(a).toMatch(
      /^receipt\.site_referral_payout\.staging_test\.[a-f0-9]{32}$/,
    )
  })
})
