import { describe, expect, test } from 'vitest'

import { projectMdkPayoutModeGate } from './mdk-payout-mode-gate'
import {
  type ReferralPayoutAdapter,
  dispatchReferralPayoutSettlement,
} from './site-referral-payout-dispatch'
import { recordReferralPayoutForPaidEvent } from './site-referral-payout-feed'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

// In-memory D1 mock mirroring site-referral-payout-ledger.test.ts, extended to
// support the feed's user_referral_attributions JOIN site_referral_sources read.
// Tests NEVER touch a real D1 or a real wallet: the payout adapter is a stub
// that records the call and returns a public-safe receipt ref WITHOUT moving
// money (money-safety: no real payout in dev).

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

class WireStore {
  rows: Array<StoredPayoutEntry> = []
  attributions: Array<AttributionJoinRow> = []
}

class WireStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: WireStore,
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

const wireDb = (store: WireStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new WireStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const seedAttribution = (
  store: WireStore,
  overrides: Partial<AttributionJoinRow> & Pick<AttributionJoinRow, 'user_id'>,
): void => {
  store.attributions.push({
    referral_attribution_id: 'referral_attribution_1',
    referral_invite_id: null,
    referral_source_id: 'site_referral_source_1',
    referrer_user_id: 'github:referrer',
    src_active: true,
    ura_active: true,
    ...overrides,
  })
}

// Stub adapter: records dispatch calls, returns a public-safe receipt ref, moves
// NO money. A real payout never happens in tests.
const makeStubAdapter = (): {
  adapter: ReferralPayoutAdapter
  calls: Array<{ amountSats: number; idempotencyKey: string; payoutRef: string }>
} => {
  const calls: Array<{
    amountSats: number
    idempotencyKey: string
    payoutRef: string
  }> = []

  return {
    adapter: {
      adapterKind: 'stub_mock',
      dispatch: input => {
        calls.push(input)

        return Promise.resolve({
          receiptRef: `receipt.mock.${input.payoutRef}`,
        })
      },
    },
    calls,
  }
}

const readyGate = () =>
  Promise.resolve(
    projectMdkPayoutModeGate({
      hostedFundedKeyVerified: true,
      hostedProgrammaticPayoutsEnabled: true,
      requestedMode: 'hosted_mdk_direct_payout',
    }),
  )

const blockedGate = () =>
  Promise.resolve(
    projectMdkPayoutModeGate({
      hostedFundedKeyVerified: false,
      hostedProgrammaticPayoutsEnabled: false,
      requestedMode: 'disabled',
    }),
  )

const bitcoinPaidEvent = (userId: string) => ({
  idempotencyKey: `site_referral_payout.btc.${userId}`,
  nowIso: '2026-06-18T12:00:00.000Z',
  periodKey: '2026-06',
  qualifyingAmountSats: 2500,
  qualifyingEventKind: 'forum_tip_paid',
  qualifyingEventRef: `evidence.btc_paid.${userId}`,
  revenueAsset: 'bitcoin' as const,
  userId,
})

describe('RL-1: referral payout feed', () => {
  test('a referred + paid event creates an eligible payout row', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })

    const result = await recordReferralPayoutForPaidEvent(
      wireDb(store),
      bitcoinPaidEvent('github:buyer'),
    )

    expect(result._tag).toBe('recorded')

    if (result._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    expect(result.entry).toMatchObject({
      amountSats: 125,
      referredUserId: 'github:buyer',
      referrerUserId: 'github:referrer',
      state: 'eligible',
    })
    expect(store.rows).toHaveLength(1)
  })

  test('a paid user with no consumed attribution records nothing', async () => {
    const store = new WireStore()

    const result = await recordReferralPayoutForPaidEvent(
      wireDb(store),
      bitcoinPaidEvent('github:unreferred'),
    )

    expect(result._tag).toBe('no_attribution')
    expect(store.rows).toHaveLength(0)
  })

  test('self-attribution is short-circuited (no row)', async () => {
    const store = new WireStore()
    seedAttribution(store, {
      referrer_user_id: 'github:buyer',
      user_id: 'github:buyer',
    })

    const result = await recordReferralPayoutForPaidEvent(
      wireDb(store),
      bitcoinPaidEvent('github:buyer'),
    )

    expect(result._tag).toBe('self_attribution')
    expect(store.rows).toHaveLength(0)
  })

  test('feed is idempotent per paid event (one row on replay)', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)

    await recordReferralPayoutForPaidEvent(db, bitcoinPaidEvent('github:buyer'))
    const replay = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    expect(replay._tag).toBe('recorded')
    expect(store.rows).toHaveLength(1)
  })
})

describe('RL-1: referral payout dispatch', () => {
  test('staged referred+paid event dispatches through the adapter and settles with a receipt ref', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)
    const { adapter, calls } = makeStubAdapter()

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const outcome = await dispatchReferralPayoutSettlement(
      db,
      { adapter, nowIso: () => '2026-06-18T12:05:00.000Z', readReadiness: readyGate },
      { payoutRef: fed.entry.payoutRef, revenueAsset: 'bitcoin' },
    )

    expect(outcome._tag).toBe('settled')

    if (outcome._tag !== 'settled') {
      throw new Error('expected settled')
    }

    expect(outcome.entry.state).toBe('settled')
    expect(outcome.receiptRef).toBe(`receipt.mock.${fed.entry.payoutRef}`)
    expect(outcome.entry.evidenceRefs).toContain(outcome.receiptRef)
    // Exactly one real payout call to the adapter.
    expect(calls).toHaveLength(1)
    expect(calls[0]?.amountSats).toBe(125)
  })

  test('dispatch is idempotent: a re-drive returns already_settled and does NOT re-pay', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)
    const { adapter, calls } = makeStubAdapter()

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const deps = {
      adapter,
      nowIso: () => '2026-06-18T12:05:00.000Z',
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
    // The adapter was called exactly once across both drives.
    expect(calls).toHaveLength(1)
  })

  test('dispatch is readiness-gated: a blocked target refuses without calling the adapter', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)
    const { adapter, calls } = makeStubAdapter()

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const outcome = await dispatchReferralPayoutSettlement(
      db,
      { adapter, nowIso: () => '2026-06-18T12:05:00.000Z', readReadiness: blockedGate },
      { payoutRef: fed.entry.payoutRef, revenueAsset: 'bitcoin' },
    )

    expect(outcome._tag).toBe('refused')

    if (outcome._tag !== 'refused') {
      throw new Error('expected refused')
    }

    expect(outcome.reasonRef).toBe(
      'reason.public.site_referral_payout.payout_target_not_ready',
    )
    expect(calls).toHaveLength(0)
  })

  test('rev-share invariant: credit/USD revenue refuses Bitcoin dispatch (no adapter call)', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)
    const { adapter, calls } = makeStubAdapter()

    // A Bitcoin-denominated eligible row exists, but the qualifying REVENUE is
    // credit/USD: the asset boundary must refuse moving Bitcoin for it.
    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const outcome = await dispatchReferralPayoutSettlement(
      db,
      { adapter, nowIso: () => '2026-06-18T12:05:00.000Z', readReadiness: readyGate },
      { payoutRef: fed.entry.payoutRef, revenueAsset: 'usd' },
    )

    expect(outcome._tag).toBe('refused')

    if (outcome._tag !== 'refused') {
      throw new Error('expected refused')
    }

    // RL-3 (#5460): the live dispatch now refuses via the SHARED credit<->Bitcoin
    // boundary guard, so the reason ref is the boundary's, not the old inline ref.
    expect(outcome.reasonRef).toBe(
      'reason.public.asset_boundary.credit_revenue_no_bitcoin_share',
    )
    expect(calls).toHaveLength(0)
  })

  test('RL-3 asset boundary: credit revenue refuses Bitcoin dispatch via the shared guard (no adapter call)', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)
    const { adapter, calls } = makeStubAdapter()

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const outcome = await dispatchReferralPayoutSettlement(
      db,
      { adapter, nowIso: () => '2026-06-18T12:05:00.000Z', readReadiness: readyGate },
      { payoutRef: fed.entry.payoutRef, revenueAsset: 'credit' },
    )

    expect(outcome._tag).toBe('refused')

    if (outcome._tag !== 'refused') {
      throw new Error('expected refused')
    }

    expect(outcome.reasonRef).toBe(
      'reason.public.asset_boundary.credit_revenue_no_bitcoin_share',
    )
    expect(calls).toHaveLength(0)
  })

  test('RL-3 asset boundary: Bitcoin revenue still settles through the live path (the valid crossing)', async () => {
    const store = new WireStore()
    seedAttribution(store, { user_id: 'github:buyer' })
    const db = wireDb(store)
    const { adapter, calls } = makeStubAdapter()

    const fed = await recordReferralPayoutForPaidEvent(
      db,
      bitcoinPaidEvent('github:buyer'),
    )

    if (fed._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    const outcome = await dispatchReferralPayoutSettlement(
      db,
      { adapter, nowIso: () => '2026-06-18T12:05:00.000Z', readReadiness: readyGate },
      { payoutRef: fed.entry.payoutRef, revenueAsset: 'bitcoin' },
    )

    // Bitcoin revenue -> Bitcoin share is the ALLOWED crossing: it settles and
    // calls the (mock) adapter exactly once.
    expect(outcome._tag).toBe('settled')
    expect(calls).toHaveLength(1)
  })

  test('unknown payout ref refuses cleanly', async () => {
    const store = new WireStore()
    const { adapter } = makeStubAdapter()

    const outcome = await dispatchReferralPayoutSettlement(
      wireDb(store),
      { adapter, nowIso: () => '2026-06-18T12:05:00.000Z', readReadiness: readyGate },
      { payoutRef: 'site_referral_payout_does_not_exist', revenueAsset: 'bitcoin' },
    )

    expect(outcome._tag).toBe('refused')

    if (outcome._tag !== 'refused') {
      throw new Error('expected refused')
    }

    expect(outcome.entry).toBeNull()
    expect(outcome.reasonRef).toBe(
      'reason.public.site_referral_payout.unknown_payout_ref',
    )
  })
})
