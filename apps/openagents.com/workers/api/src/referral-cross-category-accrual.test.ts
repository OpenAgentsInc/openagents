import { describe, expect, test } from 'vitest'

import { makeSiteReferralPayoutAdapter } from './site-referral-payout-adapter'
import { projectMdkPayoutModeGate } from './mdk-payout-mode-gate'
import { dispatchReferralPayoutSettlement } from './site-referral-payout-dispatch'
import { accrueCrossCategoryReferral } from './referral-cross-category-accrual'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

// In-memory D1 mock for the payout ledger + the consumed-attribution joins, both
// the user table (`user_referral_attributions`) and the agent table
// (`agent_referral_attributions`). Tests NEVER touch a real D1 or wallet.

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

type AttributionRow = Readonly<{
  kind: 'user' | 'agent'
  principal_user_id: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
}>

class Store {
  rows: Array<StoredPayoutEntry> = []
  attributions: Array<AttributionRow> = []
}

class Statement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: Store,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values
    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM agent_referral_attributions')) {
      return Promise.resolve(this.findAttribution('agent') as T | null)
    }

    if (this.query.includes('FROM user_referral_attributions')) {
      return Promise.resolve(this.findAttribution('user') as T | null)
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
          ['eligible', 'approved', 'dispatched', 'settled'].includes(entry.state),
      )
      return Promise.resolve({
        payout_count: rows.length,
        payout_sats: rows.reduce((total, row) => total + row.amount_sats, 0),
      } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  private findAttribution(kind: 'user' | 'agent'): AttributionRow | null {
    const principal = String(this.values[0])
    const row =
      this.store.attributions.find(
        entry => entry.kind === kind && entry.principal_user_id === principal,
      ) ?? null
    return row === null
      ? null
      : ({
          referral_attribution_id: row.referral_attribution_id,
          referral_invite_id: row.referral_invite_id,
          referral_source_id: row.referral_source_id,
          referrer_user_id: row.referrer_user_id,
        } as unknown as AttributionRow)
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

const db = (store: Store): D1Database => ({
  batch: () => Promise.reject(new Error('no batch')),
  dump: () => Promise.reject(new Error('no dump')),
  exec: () => Promise.reject(new Error('no exec')),
  prepare: query => new Statement(query, store),
  withSession: () => {
    throw new Error('no session')
  },
})

const seedUser = (store: Store, principal: string, referrer = 'github:referrer') => {
  store.attributions.push({
    kind: 'user',
    principal_user_id: principal,
    referral_attribution_id: `attr_${principal}`,
    referral_invite_id: null,
    referral_source_id: 'src_1',
    referrer_user_id: referrer,
  })
}

const seedAgent = (store: Store, principal: string, referrer = 'github:referrer') => {
  store.attributions.push({
    kind: 'agent',
    principal_user_id: principal,
    referral_attribution_id: `attr_agent_${principal}`,
    referral_invite_id: null,
    referral_source_id: 'src_1',
    referrer_user_id: referrer,
  })
}

const NOW = () => '2026-06-19T12:00:00.000Z'

describe('cross-category referral accrual (#5513)', () => {
  test('a referred user paying in the sites category accrues the referrer cut', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const result = await accrueCrossCategoryReferral(db(store), {
      category: 'sites',
      eventId: 'order_1',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:buyer' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin',
    })

    expect(result._tag).toBe('recorded')
    if (result._tag !== 'recorded') throw new Error('expected recorded')
    expect(result.entry).toMatchObject({
      amountSats: 125,
      payoutRef: 'referral.sites.payout.order_1',
      referredUserId: 'github:buyer',
      referrerUserId: 'github:referrer',
      state: 'eligible',
    })
  })

  test('the SAME referred party accrues across a SECOND category (refer-once-earn-forever, cross-category)', async () => {
    const store = new Store()
    // Permanent binding: the buyer is an agent referred by github:referrer.
    seedAgent(store, 'github:buyer')

    const sites = await accrueCrossCategoryReferral(db(store), {
      category: 'sites',
      eventId: 'order_1',
      nowIso: NOW,
      principal: { kind: 'agent', userId: 'github:buyer' },
      qualifyingAmountSats: 2000,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin',
    })
    const marketplace = await accrueCrossCategoryReferral(db(store), {
      category: 'marketplace',
      eventId: 'listing_9',
      nowIso: NOW,
      principal: { kind: 'agent', userId: 'github:buyer' },
      qualifyingAmountSats: 2000,
      qualifyingEventKind: 'marketplace_paid_purchase',
      revenueAsset: 'bitcoin',
    })

    expect(sites._tag).toBe('recorded')
    expect(marketplace._tag).toBe('recorded')
    if (sites._tag !== 'recorded' || marketplace._tag !== 'recorded') {
      throw new Error('expected recorded')
    }
    // Both categories accrue to the SAME referrer from ONE permanent binding,
    // each as its own independently dispatchable payout row.
    expect(sites.entry.referrerUserId).toBe('github:referrer')
    expect(marketplace.entry.referrerUserId).toBe('github:referrer')
    expect(sites.entry.payoutRef).not.toBe(marketplace.entry.payoutRef)
    expect(store.rows).toHaveLength(2)
  })

  test('idempotent per (category, event): a replay records one row', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')
    const database = db(store)

    const input = {
      category: 'sites' as const,
      eventId: 'order_1',
      nowIso: NOW,
      principal: { kind: 'user' as const, userId: 'github:buyer' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin' as const,
    }
    await accrueCrossCategoryReferral(database, input)
    await accrueCrossCategoryReferral(database, input)

    expect(store.rows).toHaveLength(1)
  })

  test('honest scope: a credit/USD-funded purchase accrues credit revshare, never a withdrawable Bitcoin liability', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    // Credit revenue accrues a CREDIT revshare eligibility (the boundary allows
    // credit->credit), but dispatch (below) refuses to move Bitcoin for it.
    const result = await accrueCrossCategoryReferral(db(store), {
      category: 'fine_tuning',
      eventId: 'ft_1',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:buyer' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'fine_tuning_paid_run',
      revenueAsset: 'usd',
    })

    expect(result._tag).toBe('recorded')
    if (result._tag !== 'recorded') throw new Error('expected recorded')

    // The credit-funded eligibility row may NOT move Bitcoin on dispatch.
    const { adapter, calls } = (() => {
      const c: Array<unknown> = []
      return {
        adapter: makeSiteReferralPayoutAdapter({
          client: {
            programmaticPayout: input => {
              c.push(input)
              return Promise.resolve({ paymentId: 'p', status: 'SUCCESS' as const })
            },
          },
          resolveDestination: async () => 'lno1reusable',
        }),
        calls: c,
      }
    })()

    const outcome = await dispatchReferralPayoutSettlement(
      db(store),
      {
        adapter,
        nowIso: () => '2026-06-19T12:05:00.000Z',
        readReadiness: () =>
          Promise.resolve(
            projectMdkPayoutModeGate({
              hostedFundedKeyVerified: true,
              hostedProgrammaticPayoutsEnabled: true,
              requestedMode: 'hosted_mdk_direct_payout',
            }),
          ),
      },
      { payoutRef: result.entry.payoutRef, revenueAsset: 'usd' },
    )

    expect(outcome._tag).toBe('refused')
    if (outcome._tag !== 'refused') throw new Error('expected refused')
    expect(outcome.reasonRef).toBe(
      'reason.public.asset_boundary.credit_revenue_no_bitcoin_share',
    )
    // No money moved.
    expect(calls).toHaveLength(0)
  })

  test('no attribution -> no row; self-attribution -> no row', async () => {
    const store = new Store()
    seedUser(store, 'github:self', 'github:self')

    const unreferred = await accrueCrossCategoryReferral(db(store), {
      category: 'sites',
      eventId: 'e1',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:nobody' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin',
    })
    const selfRef = await accrueCrossCategoryReferral(db(store), {
      category: 'sites',
      eventId: 'e2',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:self' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin',
    })

    expect(unreferred._tag).toBe('no_attribution')
    expect(selfRef._tag).toBe('self_attribution')
    expect(store.rows).toHaveLength(0)
  })

  test('usage-funded only: a zero/below-1-sat-cut event accrues nothing (not an error)', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const zero = await accrueCrossCategoryReferral(db(store), {
      category: 'sites',
      eventId: 'e0',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:buyer' },
      qualifyingAmountSats: 0,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin',
    })

    expect(zero._tag).toBe('zero_referrer_share')
    expect(store.rows).toHaveLength(0)
  })

  test('bounded fields: a malformed category is rejected', async () => {
    const store = new Store()
    const result = await accrueCrossCategoryReferral(db(store), {
      category: 'Not A Category',
      eventId: 'e1',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:buyer' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'k',
      revenueAsset: 'bitcoin',
    })
    expect(result._tag).toBe('invalid_input')
  })

  test('end-to-end: cross-category Bitcoin accrual settles through the armed adapter (staging-style proof)', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const accrued = await accrueCrossCategoryReferral(db(store), {
      category: 'sites',
      eventId: 'order_42',
      nowIso: NOW,
      principal: { kind: 'user', userId: 'github:buyer' },
      qualifyingAmountSats: 2500,
      qualifyingEventKind: 'sites_paid_order',
      revenueAsset: 'bitcoin',
    })
    expect(accrued._tag).toBe('recorded')
    if (accrued._tag !== 'recorded') throw new Error('expected recorded')

    const calls: Array<{ amountSats: number; idempotencyKey: string }> = []
    const adapter = makeSiteReferralPayoutAdapter({
      client: {
        programmaticPayout: input => {
          calls.push({ amountSats: input.amountSats, idempotencyKey: input.idempotencyKey })
          return Promise.resolve({
            paymentHash: 'staginghash',
            paymentId: 'staging_pay',
            status: 'SUCCESS' as const,
          })
        },
      },
      resolveDestination: async () => 'lno1reusablestaging',
    })

    const outcome = await dispatchReferralPayoutSettlement(
      db(store),
      {
        adapter,
        nowIso: () => '2026-06-19T12:05:00.000Z',
        readReadiness: () =>
          Promise.resolve(
            projectMdkPayoutModeGate({
              hostedFundedKeyVerified: true,
              hostedProgrammaticPayoutsEnabled: true,
              requestedMode: 'hosted_mdk_direct_payout',
            }),
          ),
      },
      { payoutRef: accrued.entry.payoutRef, revenueAsset: 'bitcoin' },
    )

    expect(outcome._tag).toBe('settled')
    if (outcome._tag !== 'settled') throw new Error('expected settled')
    expect(outcome.entry.state).toBe('settled')
    expect(outcome.receiptRef).toMatch(/^receipt\.site_referral_payout\.hosted_mdk\./)
    // Exactly one real payout call for the 5% cut of the qualifying amount.
    expect(calls).toHaveLength(1)
    expect(calls[0]?.amountSats).toBe(125)
    // The settled evidence is the redacted receipt, never raw payment material.
    expect(outcome.entry.evidenceRefs).toContain(outcome.receiptRef)
    expect(outcome.entry.evidenceRefs.join(' ')).not.toContain('staginghash')
  })
})
