import { describe, expect, test } from 'vitest'

import { recordBusinessReferralEngagement } from './business-referral-engagement-feed'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

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

type BusinessAttributionJoinRow = Readonly<{
  business_signup_request_id: string
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  public_source_ref: string
  referrer_user_id: string
  bsra_active: boolean
  src_active: boolean
}>

type FunnelEventRow = Readonly<{
  event_ref: string
  stage: string
  source_kind: string
  source_ref: string | null
}>

class BusinessReferralStore {
  rows: Array<StoredPayoutEntry> = []
  attributions: Array<BusinessAttributionJoinRow> = []
  funnelEvents: Array<FunnelEventRow> = []
}

class BusinessReferralStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: BusinessReferralStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): BusinessReferralStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM business_signup_referral_attributions')) {
      const businessSignupId = String(this.values[0])
      const row =
        this.store.attributions.find(
          entry =>
            entry.business_signup_request_id === businessSignupId &&
            entry.bsra_active &&
            entry.src_active,
        ) ?? null

      return Promise.resolve(
        row === null
          ? null
          : ({
              referral_attribution_id: row.referral_attribution_id,
              referral_invite_id: row.referral_invite_id,
              referral_source_id: row.referral_source_id,
              public_source_ref: row.public_source_ref,
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
      const existing = this.store.rows.find(
        row => row.idempotency_key === String(this.values[2]),
      )
      if (existing === undefined) {
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
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO business_funnel_events')) {
      const eventRef = String(this.values[1])
      if (!this.store.funnelEvents.some(row => row.event_ref === eventRef)) {
        this.store.funnelEvents.push({
          event_ref: eventRef,
          source_kind: String(this.values[3]),
          source_ref: this.values[4] as string | null,
          stage: String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(): Promise<Array<T>> {
    return Promise.reject(new Error(`Unexpected raw: ${this.query}`))
  }
}

class BusinessReferralD1 {
  constructor(private readonly store: BusinessReferralStore) {}

  prepare(query: string): BusinessReferralStatement {
    return new BusinessReferralStatement(query, this.store)
  }

  batch<T = unknown>(): Promise<Array<D1Result<T>>> {
    return Promise.reject(new Error('Unexpected batch'))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.reject(new Error('Unexpected dump'))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.reject(new Error('Unexpected exec'))
  }
}

const dbFor = (store: BusinessReferralStore): D1Database =>
  new BusinessReferralD1(store) as unknown as D1Database

const seedAttribution = (
  store: BusinessReferralStore,
  overrides: Partial<BusinessAttributionJoinRow> = {},
) => {
  store.attributions.push({
    bsra_active: true,
    business_signup_request_id: 'business_signup_001',
    referral_attribution_id: 'referral_attr_001',
    referral_invite_id: null,
    referral_source_id: 'referral_source_001',
    public_source_ref: 'ref-code-001',
    referrer_user_id: 'github:referrer',
    src_active: true,
    ...overrides,
  })
}

const paidBusinessEngagement = (
  businessSignupId: string,
  overrides: Partial<Parameters<typeof recordBusinessReferralEngagement>[1]> = {},
) => ({
  businessSignupId,
  idempotencyKey: `business_referral_engagement.${businessSignupId}`,
  nowIso: '2026-07-03T12:00:00.000Z',
  periodKey: '2026-07',
  qualifyingAmountSats: 20_000,
  qualifyingEventKind: 'business_referred_engagement_paid',
  qualifyingEventRef: `evidence.business_referred_engagement.${businessSignupId}`,
  revenueAsset: 'bitcoin' as const,
  referredUserId: 'github:buyer',
  ...overrides,
})

describe('business referral engagement feed', () => {
  test('records an attributed engagement on the existing referral payout ledger', async () => {
    const store = new BusinessReferralStore()
    seedAttribution(store)

    const result = await recordBusinessReferralEngagement(
      dbFor(store),
      paidBusinessEngagement('business_signup_001'),
    )

    expect(result._tag).toBe('recorded')

    if (result._tag !== 'recorded') {
      throw new Error('expected recorded')
    }

    expect(result.payout).toMatchObject({
      amountSats: 1000,
      qualifyingEventKind: 'business_referred_engagement_paid',
      referredUserId: 'github:buyer',
      referralAttributionId: 'referral_attr_001',
      referrerUserId: 'github:referrer',
      state: 'eligible',
    })
    expect(result.funnelEvent).toMatchObject({
      eventRef:
        'business_referral_engagement:evidence.business_referred_engagement.business_signup_001',
      sourceKind: 'referral',
      // Bounded public-safe token derived from the referral source's
      // public_source_ref (signup-path mapping), never the internal
      // site_referral_sources.id.
      sourceRef: 'affiliate_ref-code-001',
      stage: 'referred_engagement',
    })
    expect(store.rows).toHaveLength(1)
    expect(store.funnelEvents).toHaveLength(1)
  })

  test('records nothing when a business signup has no referral attribution', async () => {
    const store = new BusinessReferralStore()

    const result = await recordBusinessReferralEngagement(
      dbFor(store),
      paidBusinessEngagement('business_signup_002'),
    )

    expect(result._tag).toBe('no_attribution')
    expect(store.rows).toHaveLength(0)
    expect(store.funnelEvents).toHaveLength(0)
  })

  test('keeps checkout retries idempotent across payout and funnel rows', async () => {
    const store = new BusinessReferralStore()
    seedAttribution(store)
    const db = dbFor(store)
    const input = paidBusinessEngagement('business_signup_001')

    await recordBusinessReferralEngagement(db, input)
    await recordBusinessReferralEngagement(db, input)

    expect(store.rows).toHaveLength(1)
    expect(store.funnelEvents).toHaveLength(1)
  })
})
