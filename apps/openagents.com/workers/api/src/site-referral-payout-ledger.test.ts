import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  hostedMdkDirectPayoutDisabledGate,
  projectMdkPayoutModeGate,
} from './mdk-payout-mode-gate'
import {
  createReferralPayoutEligibility,
  transitionReferralPayout,
  type SiteReferralPayoutState,
} from './site-referral-payout-ledger'
import { makeSiteReferralPayoutLedgerRoutes } from './site-referral-payout-ledger-routes'

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

class PayoutStore {
  rows: Array<StoredPayoutEntry> = []
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
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
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

const baseEligibility = {
  id: 'site_referral_payout_entry_1',
  idempotencyKey: 'site-referral-payout:eligible:1',
  nowIso: '2026-06-10T09:00:00.000Z',
  payoutRef: 'site_referral_payout_ref_1',
  periodKey: '2026-06',
  qualifyingAmountSats: 2500,
  qualifyingEventKind: 'site_checkout',
  qualifyingEventRef: 'referral_workflow_event_checkout_1',
  referredUserId: 'github:buyer',
  referralAttributionId: 'referral_attribution_1',
  referralInviteId: null,
  referralSourceId: 'site_referral_source_1',
  referrerUserId: 'github:referrer',
}

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

const makeLedgerRoute = (
  overrides: Partial<{
    admin: boolean
    dispatch: (input: {
      amountSats: number
      idempotencyKey: string
      payoutRef: string
    }) => Promise<{ receiptRef: string }>
    livePayoutClaimAllowed: boolean
  }> = {},
) =>
  makeSiteReferralPayoutLedgerRoutes({
    dispatchDependencies: {
      adapter: {
        adapterKind: 'test',
        dispatch:
          overrides.dispatch ??
          (async () => ({ receiptRef: 'receipt.site_referral_payout.test' })),
      },
      nowIso: () => '2026-06-10T09:01:00.000Z',
      readReadiness: async () =>
        overrides.livePayoutClaimAllowed === true
          ? projectMdkPayoutModeGate({
              hostedFundedKeyVerified: true,
              hostedProgrammaticPayoutsEnabled: true,
              requestedMode: 'hosted_mdk_direct_payout',
            })
          : hostedMdkDirectPayoutDisabledGate(),
    },
    nowIso: () => '2026-06-10T09:01:00.000Z',
    requireAdminApiToken: () => Promise.resolve(overrides.admin ?? true),
  })

describe('Site referral payout ledger', () => {
  test('creates eligible payout rows with capped percentage amount', async () => {
    const store = new PayoutStore()
    const entry = await createReferralPayoutEligibility(
      payoutDb(store),
      baseEligibility,
    )

    expect(entry).toMatchObject({
      amountSats: 125,
      payoutRef: 'site_referral_payout_ref_1',
      state: 'eligible',
    })
    expect(store.rows).toHaveLength(1)
  })

  test('refuses self-referrals and referrer period cap overflow', async () => {
    const selfReferralStore = new PayoutStore()
    const self = await createReferralPayoutEligibility(payoutDb(selfReferralStore), {
      ...baseEligibility,
      idempotencyKey: 'site-referral-payout:self',
      referredUserId: 'github:referrer',
    })

    expect(self).toMatchObject({
      amountSats: 0,
      state: 'refused',
      stateReasonRef: 'reason.public.site_referral_payout.self_referral',
    })

    const cappedStore = new PayoutStore()
    cappedStore.rows.push({
      ...selfReferralStore.rows[0]!,
      amount_sats: 5000,
      id: 'existing',
      idempotency_key: 'existing',
      payout_ref: 'existing',
      referred_user_id: 'github:other',
      state: 'settled',
      state_reason_ref: null,
    })
    const capped = await createReferralPayoutEligibility(payoutDb(cappedStore), {
      ...baseEligibility,
      idempotencyKey: 'site-referral-payout:capped',
      payoutRef: 'site_referral_payout_ref_capped',
    })

    expect(capped).toMatchObject({
      amountSats: 0,
      state: 'refused',
      stateReasonRef:
        'reason.public.site_referral_payout.referrer_period_cap_exceeded',
    })
  })

  test('uses append-only transition and reversal rows', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    await transitionReferralPayout(payoutDb(store), {
      action: 'approve_dispatch',
      id: 'site_referral_payout_entry_2',
      idempotencyKey: 'site-referral-payout:approve:1',
      nowIso: '2026-06-10T09:01:00.000Z',
      payoutRef: 'site_referral_payout_ref_1',
    })
    await transitionReferralPayout(payoutDb(store), {
      action: 'mark_dispatched',
      id: 'site_referral_payout_entry_3',
      idempotencyKey: 'site-referral-payout:dispatch:1',
      nowIso: '2026-06-10T09:02:00.000Z',
      payoutRef: 'site_referral_payout_ref_1',
    })
    await expect(
      transitionReferralPayout(payoutDb(store), {
        action: 'mark_settled',
        idempotencyKey: 'site-referral-payout:settle-missing-evidence:1',
        nowIso: '2026-06-10T09:03:00.000Z',
        payoutRef: 'site_referral_payout_ref_1',
      }),
    ).rejects.toMatchObject({
      _tag: 'SiteReferralPayoutLedgerValidationError',
    })
    await transitionReferralPayout(payoutDb(store), {
      action: 'mark_settled',
      evidenceRefs: ['settlement_evidence.public.referral_payout.test'],
      id: 'site_referral_payout_entry_4',
      idempotencyKey: 'site-referral-payout:settle:1',
      nowIso: '2026-06-10T09:04:00.000Z',
      payoutRef: 'site_referral_payout_ref_1',
    })
    const reversed = await transitionReferralPayout(payoutDb(store), {
      action: 'reverse',
      id: 'site_referral_payout_entry_5',
      idempotencyKey: 'site-referral-payout:reverse:1',
      nowIso: '2026-06-10T09:05:00.000Z',
      payoutRef: 'site_referral_payout_ref_1',
      stateReasonRef: 'reason.public.site_referral_payout.refund_or_abuse',
    })

    expect(store.rows.map(row => row.state)).toEqual([
      'eligible',
      'approved',
      'dispatched',
      'settled',
      'reversed',
    ])
    expect(reversed).toMatchObject({
      amountSats: -125,
      reversalOfEntryId: 'site_referral_payout_entry_4',
      state: 'reversed',
    })
  })
})

describe('Site referral payout ledger routes', () => {
  test('requires admin token for operator transitions', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    const route = makeLedgerRoute({ admin: false })
    const response = await Effect.runPromise(
      route.routeSiteReferralPayoutLedgerRequest(
        new Request(
          'https://openagents.com/api/operator/sites/referrals/payout-ledger/site_referral_payout_ref_1/transitions',
          {
            body: JSON.stringify({
              action: 'approve_dispatch',
              idempotencyKey: 'site-referral-payout:route-denied',
            }),
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: payoutDb(store) },
        executionContext(),
      )!,
    )

    expect(response.status).toBe(401)
    expect(store.rows).toHaveLength(1)
  })

  test('operator route records approved transition without mutating prior row', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    const route = makeLedgerRoute()
    const response = await Effect.runPromise(
      route.routeSiteReferralPayoutLedgerRequest(
        new Request(
          'https://openagents.com/api/operator/sites/referrals/payout-ledger/site_referral_payout_ref_1/transitions',
          {
            body: JSON.stringify({
              action: 'approve_dispatch',
              idempotencyKey: 'site-referral-payout:route-approve',
            }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: payoutDb(store) },
        executionContext(),
      )!,
    )
    const body = (await response.json()) as {
      payout: { state: string }
    }

    expect(response.status).toBe(200)
    expect(body.payout.state).toBe('approved')
    expect(store.rows.map(row => row.state)).toEqual(['eligible', 'approved'])
  })

  test('requires admin token for operator dispatch', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    const route = makeLedgerRoute({ admin: false })
    const response = await Effect.runPromise(
      route.routeSiteReferralPayoutLedgerRequest(
        new Request(
          'https://openagents.com/api/operator/sites/referrals/payout-ledger/site_referral_payout_ref_1/dispatch',
          {
            body: JSON.stringify({ revenueAsset: 'bitcoin' }),
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: payoutDb(store) },
        executionContext(),
      )!,
    )

    expect(response.status).toBe(401)
    expect(store.rows).toHaveLength(1)
  })

  test('operator dispatch refuses while owner-armed payout mode is disabled', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    const dispatchCalls: Array<unknown> = []
    const route = makeLedgerRoute({
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.site_referral_payout.should_not_dispatch' }
      },
      livePayoutClaimAllowed: false,
    })
    const response = await Effect.runPromise(
      route.routeSiteReferralPayoutLedgerRequest(
        new Request(
          'https://openagents.com/api/operator/sites/referrals/payout-ledger/site_referral_payout_ref_1/dispatch',
          {
            body: JSON.stringify({ revenueAsset: 'bitcoin' }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: payoutDb(store) },
        executionContext(),
      )!,
    )
    const body = (await response.json()) as {
      dispatch: { _tag: string; reasonRef?: string; state?: string }
    }

    expect(response.status).toBe(200)
    expect(body.dispatch).toMatchObject({
      _tag: 'refused',
      reasonRef: 'reason.public.site_referral_payout.payout_target_not_ready',
      state: 'eligible',
    })
    expect(dispatchCalls).toHaveLength(0)
    expect(store.rows.map(row => row.state)).toEqual(['eligible'])
  })

  test('operator dispatch refuses usd and credit revenue before adapter call', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    const dispatchCalls: Array<unknown> = []
    const route = makeLedgerRoute({
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.site_referral_payout.should_not_dispatch' }
      },
      livePayoutClaimAllowed: true,
    })

    for (const revenueAsset of ['usd', 'credit'] as const) {
      const response = await Effect.runPromise(
        route.routeSiteReferralPayoutLedgerRequest(
          new Request(
            'https://openagents.com/api/operator/sites/referrals/payout-ledger/site_referral_payout_ref_1/dispatch',
            {
              body: JSON.stringify({ revenueAsset }),
              headers: { 'content-type': 'application/json' },
              method: 'POST',
            },
          ),
          { OPENAGENTS_DB: payoutDb(store) },
          executionContext(),
        )!,
      )
      const body = (await response.json()) as {
        dispatch: { _tag: string; reasonRef?: string }
      }

      expect(response.status).toBe(200)
      expect(body.dispatch).toMatchObject({
        _tag: 'refused',
        reasonRef:
          'reason.public.asset_boundary.credit_revenue_no_bitcoin_share',
      })
    }

    expect(dispatchCalls).toHaveLength(0)
    expect(store.rows.map(row => row.state)).toEqual(['eligible'])
  })

  test('operator dispatch settles through an armed adapter and redacted receipt', async () => {
    const store = new PayoutStore()
    await createReferralPayoutEligibility(payoutDb(store), baseEligibility)
    const dispatchCalls: Array<{
      amountSats: number
      idempotencyKey: string
      payoutRef: string
    }> = []
    const route = makeLedgerRoute({
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.site_referral_payout.hosted_mdk.abc123' }
      },
      livePayoutClaimAllowed: true,
    })
    const response = await Effect.runPromise(
      route.routeSiteReferralPayoutLedgerRequest(
        new Request(
          'https://openagents.com/api/operator/sites/referrals/payout-ledger/site_referral_payout_ref_1/dispatch',
          {
            body: JSON.stringify({ revenueAsset: 'bitcoin' }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        { OPENAGENTS_DB: payoutDb(store) },
        executionContext(),
      )!,
    )
    const body = (await response.json()) as {
      dispatch: {
        _tag: string
        amountSats: number
        payoutRef: string
        receiptRef: string
        state: string
      }
    }

    expect(response.status).toBe(200)
    expect(body.dispatch).toMatchObject({
      _tag: 'settled',
      amountSats: 125,
      payoutRef: 'site_referral_payout_ref_1',
      receiptRef: 'receipt.site_referral_payout.hosted_mdk.abc123',
      state: 'settled',
    })
    expect(dispatchCalls).toEqual([
      {
        amountSats: 125,
        idempotencyKey:
          'site_referral_payout.adapter.site_referral_payout_ref_1',
        payoutRef: 'site_referral_payout_ref_1',
      },
    ])
    expect(store.rows.map(row => row.state)).toEqual([
      'eligible',
      'approved',
      'dispatched',
      'settled',
    ])
    expect(store.rows.at(-1)?.evidence_refs_json).toContain(
      'receipt.site_referral_payout.hosted_mdk.abc123',
    )
  })
})
