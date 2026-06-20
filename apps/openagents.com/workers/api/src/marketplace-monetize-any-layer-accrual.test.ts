import { describe, expect, test } from 'vitest'

import {
  accrueMonetizeLayerReferral,
  monetizeLayerSpendMsatToQualifyingSats,
} from './marketplace-monetize-any-layer-accrual'
import { buildLayerMonetizationDefinition } from './marketplace-monetize-any-layer'
import type { LayerMonetizationDefinition } from './marketplace-monetize-any-layer'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

// In-memory D1 mock for the cross-category ledger + the consumed-attribution
// joins. Mirrors referral-cross-category-accrual.test.ts. Tests NEVER touch a
// real D1 or wallet.

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

const NOW = () => '2026-06-19T12:00:00.000Z'

// A simple, valid agentic-work Bitcoin offer (the unconditionally-authorizable
// monetization kind). 1_000_000 msat == 1000 sats qualifying spend.
const makeOffer = (
  overrides: Partial<{
    layer: Parameters<typeof buildLayerMonetizationDefinition>[0]['layer']
    monetizationKind: Parameters<
      typeof buildLayerMonetizationDefinition
    >[0]['monetizationKind']
    priceAsset: Parameters<typeof buildLayerMonetizationDefinition>[0]['priceAsset']
    referrerRef: string
    sellerRef: string
  }> = {},
): LayerMonetizationDefinition => {
  const built = buildLayerMonetizationDefinition({
    offerId: 'offer_1',
    sellerRef: overrides.sellerRef ?? 'github:seller',
    layer: overrides.layer ?? 'inference',
    monetizationKind: overrides.monetizationKind ?? 'agentic_work',
    unitPriceMsat: 1000,
    priceAsset: overrides.priceAsset ?? 'bitcoin',
    referralBps: 500,
    referrerRef: overrides.referrerRef ?? 'github:offer_referrer',
    createdAt: '2026-06-19T11:00:00.000Z',
  })
  if (!built.ok) throw new Error(`bad offer: ${built.error.reason}`)
  return built.definition
}

describe('monetizeLayerSpendMsatToQualifyingSats', () => {
  test('floors msat to whole sats (1 sat == 1000 msat)', () => {
    expect(monetizeLayerSpendMsatToQualifyingSats(1_000_000)).toBe(1000)
    expect(monetizeLayerSpendMsatToQualifyingSats(1999)).toBe(1)
    expect(monetizeLayerSpendMsatToQualifyingSats(999)).toBe(0)
    expect(monetizeLayerSpendMsatToQualifyingSats(0)).toBe(0)
    expect(monetizeLayerSpendMsatToQualifyingSats(-5)).toBe(0)
    expect(monetizeLayerSpendMsatToQualifyingSats(Number.NaN)).toBe(0)
  })
})

describe('monetize-any-layer -> cross-category referral accrual bridge (#5518/#5513)', () => {
  test('FLAG OFF (default): plans but touches NO ledger', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: false },
      {
        definition: makeOffer(),
        eventId: 'spend_1',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('disabled')
    if (result._tag !== 'disabled') throw new Error('expected disabled')
    // The plan is authorized (agentic-work is allowed) and computed, but no row.
    expect(result.plan.authorized).toBe(true)
    expect(result.plan.inert).toBe(true)
    expect(result.plan.promiseState).toBe('planned')
    expect(store.rows).toHaveLength(0)
  })

  test('FLAG ON: an authorized Bitcoin agentic-work spend accrues the referee-attributed referrer cut', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer', 'github:true_referrer')

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: true },
      {
        definition: makeOffer({ referrerRef: 'github:offer_referrer' }),
        eventId: 'spend_1',
        meteredSpendMsat: 1_000_000, // 1000 sats qualifying
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('accrued')
    if (result._tag !== 'accrued') throw new Error('expected accrued')
    expect(result.accrual._tag).toBe('recorded')
    if (result.accrual._tag !== 'recorded') throw new Error('expected recorded')
    // The LEDGER pays the principal's ATTRIBUTED referrer, NOT the offer's
    // seller-asserted referrerRef. Refer-once-earn-forever: the binding is the
    // referee's.
    expect(result.accrual.entry.referrerUserId).toBe('github:true_referrer')
    // 5% of 1000 sats == 50 sats (ledger policy, not the offer's bps).
    expect(result.accrual.entry.amountSats).toBe(50)
    // Layer-namespaced payout ref.
    expect(result.accrual.entry.payoutRef).toBe(
      'referral.monetize_inference.payout.spend_1',
    )
    expect(result.accrual.entry.qualifyingEventKind).toBe(
      'monetize_any_layer.agentic_work',
    )
    expect(store.rows).toHaveLength(1)
  })

  test('subscription resale is non-waivably blocked: unauthorized, no ledger row even with the flag ON', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: true },
      {
        definition: makeOffer({
          monetizationKind: 'subscription_capacity_resale',
        }),
        accountAuthMode: 'subscription',
        eventId: 'spend_1',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('unauthorized')
    if (result._tag !== 'unauthorized') throw new Error('expected unauthorized')
    expect(result.plan.authorized).toBe(false)
    expect(result.plan.blockerRefs).toContain(
      'blocker.inference_resale.subscription_resale_forbidden',
    )
    expect(store.rows).toHaveLength(0)
  })

  test('self-referral is blocked: unauthorized, no ledger row', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: true },
      {
        definition: makeOffer({
          referrerRef: 'github:seller',
          sellerRef: 'github:seller',
        }),
        eventId: 'spend_1',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('unauthorized')
    if (result._tag !== 'unauthorized') throw new Error('expected unauthorized')
    expect(result.plan.blockerRefs).toContain(
      'blocker.monetize_any_layer.self_referral',
    )
    expect(store.rows).toHaveLength(0)
  })

  test('an unreferred buyer accrues nothing (no_attribution), even when armed', async () => {
    const store = new Store()
    // No attribution seeded for github:buyer.

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: true },
      {
        definition: makeOffer(),
        eventId: 'spend_1',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('accrued')
    if (result._tag !== 'accrued') throw new Error('expected accrued')
    expect(result.accrual._tag).toBe('no_attribution')
    expect(store.rows).toHaveLength(0)
  })

  test('idempotent per (layer, event): a replay records ONE row', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')
    const database = db(store)

    const input = {
      definition: makeOffer(),
      eventId: 'spend_1',
      meteredSpendMsat: 1_000_000,
      nowIso: NOW,
      principal: { kind: 'user' as const, userId: 'github:buyer' },
    }
    await accrueMonetizeLayerReferral(database, { enabled: true }, input)
    await accrueMonetizeLayerReferral(database, { enabled: true }, input)

    expect(store.rows).toHaveLength(1)
  })

  test('sub-sat spend accrues nothing (zero_referrer_share), not an error', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: true },
      {
        definition: makeOffer(),
        eventId: 'spend_1',
        meteredSpendMsat: 500, // < 1000 msat == 0 sats qualifying
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('accrued')
    if (result._tag !== 'accrued') throw new Error('expected accrued')
    expect(result.accrual._tag).toBe('zero_referrer_share')
    expect(store.rows).toHaveLength(0)
  })

  test('different layers for the SAME event id do NOT collide (layer-namespaced)', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')
    const database = db(store)

    const inference = await accrueMonetizeLayerReferral(
      database,
      { enabled: true },
      {
        definition: makeOffer({ layer: 'inference' }),
        eventId: 'shared_event',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )
    const sandbox = await accrueMonetizeLayerReferral(
      database,
      { enabled: true },
      {
        definition: makeOffer({ layer: 'sandbox' }),
        eventId: 'shared_event',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(inference._tag).toBe('accrued')
    expect(sandbox._tag).toBe('accrued')
    if (inference._tag !== 'accrued' || sandbox._tag !== 'accrued') {
      throw new Error('expected accrued')
    }
    if (
      inference.accrual._tag !== 'recorded' ||
      sandbox.accrual._tag !== 'recorded'
    ) {
      throw new Error('expected recorded')
    }
    expect(inference.accrual.entry.payoutRef).toBe(
      'referral.monetize_inference.payout.shared_event',
    )
    expect(sandbox.accrual.entry.payoutRef).toBe(
      'referral.monetize_sandbox.payout.shared_event',
    )
    expect(store.rows).toHaveLength(2)
  })

  test('credit-priced offer accrues a credit revshare (no withdrawable Bitcoin liability)', async () => {
    const store = new Store()
    seedUser(store, 'github:buyer')

    const result = await accrueMonetizeLayerReferral(
      db(store),
      { enabled: true },
      {
        definition: makeOffer({ priceAsset: 'credit' }),
        eventId: 'spend_1',
        meteredSpendMsat: 1_000_000,
        nowIso: NOW,
        principal: { kind: 'user', userId: 'github:buyer' },
      },
    )

    expect(result._tag).toBe('accrued')
    if (result._tag !== 'accrued') throw new Error('expected accrued')
    // credit -> credit revshare is allowed by the boundary at accrual; the row
    // records, and dispatch (tested in the cross-category suite) refuses to move
    // Bitcoin for a credit-funded row.
    expect(result.accrual._tag).toBe('recorded')
    expect(store.rows).toHaveLength(1)
  })
})
