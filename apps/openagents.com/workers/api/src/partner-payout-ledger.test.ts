import { describe, expect, test } from 'vitest'

import {
  calculatePartnerPayoutAmount,
  createPartnerPayoutEligibility,
  projectPartnerPayout,
  transitionPartnerPayout,
  type PartnerPayoutAsset,
  type PartnerPayoutRole,
  type PartnerPayoutState,
} from './partner-payout-ledger'

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

const baseDesignPartner = {
  asset: 'usd' as const,
  beneficiaryUserId: 'github:client',
  id: 'partner_payout_entry_1',
  idempotencyKey: 'partner-payout:design:eligible:1',
  nowIso: '2026-06-10T09:00:00.000Z',
  partnerRef: 'design_partner_acme',
  partnerRole: 'design_partner' as const,
  partnerUserId: 'github:acme_agency',
  payoutRef: 'partner_payout_ref_1',
  periodKey: '2026-06',
  qualifyingAmount: 10000,
  qualifyingEventKind: 'design_partner_engagement',
  qualifyingEventRef: 'partner_event_engagement_1',
}

describe('Partner payout amount policy', () => {
  test('applies per-role percentage with per-event ceiling', () => {
    // design_partner: 20% of 10000 = 2000, under maxEventAmount 100000.
    expect(calculatePartnerPayoutAmount('design_partner', 10000)).toBe(2000)
    // referral: 5% of 2500 = 125, matches legacy referral lane.
    expect(calculatePartnerPayoutAmount('referral', 2500)).toBe(125)
    // affiliate: 10% of 1000 = 100.
    expect(calculatePartnerPayoutAmount('affiliate', 1000)).toBe(100)
    // affiliate per-event ceiling at 5000.
    expect(calculatePartnerPayoutAmount('affiliate', 1_000_000)).toBe(5000)
    // zero / negative qualifying amounts yield 0.
    expect(calculatePartnerPayoutAmount('referral', 0)).toBe(0)
    expect(calculatePartnerPayoutAmount('referral', -5)).toBe(0)
  })
})

describe('Partner payout ledger eligibility', () => {
  test('creates eligible design-partner usd payout with role percentage', async () => {
    const store = new PayoutStore()
    const entry = await createPartnerPayoutEligibility(
      payoutDb(store),
      baseDesignPartner,
    )

    expect(entry).toMatchObject({
      amount: 2000,
      asset: 'usd',
      partnerRef: 'design_partner_acme',
      partnerRole: 'design_partner',
      payoutRef: 'partner_payout_ref_1',
      state: 'eligible',
    })
    expect(entry.policyRefs).toContain('policy.partner_payout.v1')
    expect(entry.caveatRefs).toContain(
      'caveat.partner_payout.settlement_evidence_required',
    )
    expect(store.rows).toHaveLength(1)
  })

  test('supports referral and affiliate roles across credits and sats assets', async () => {
    const referralStore = new PayoutStore()
    const referral = await createPartnerPayoutEligibility(payoutDb(referralStore), {
      ...baseDesignPartner,
      asset: 'sats',
      idempotencyKey: 'partner-payout:referral:1',
      partnerRef: 'site_referral_source_1',
      partnerRole: 'referral',
      payoutRef: 'partner_payout_referral_1',
      qualifyingAmount: 2500,
    })

    expect(referral).toMatchObject({
      amount: 125,
      asset: 'sats',
      partnerRole: 'referral',
      state: 'eligible',
    })

    const affiliateStore = new PayoutStore()
    const affiliate = await createPartnerPayoutEligibility(payoutDb(affiliateStore), {
      ...baseDesignPartner,
      asset: 'credits',
      idempotencyKey: 'partner-payout:affiliate:1',
      partnerRef: 'affiliate_code_xyz',
      partnerRole: 'affiliate',
      payoutRef: 'partner_payout_affiliate_1',
      qualifyingAmount: 1000,
    })

    expect(affiliate).toMatchObject({
      amount: 100,
      asset: 'credits',
      partnerRole: 'affiliate',
      state: 'eligible',
    })
  })

  test('refuses self-payouts and partner period cap overflow', async () => {
    const selfStore = new PayoutStore()
    const self = await createPartnerPayoutEligibility(payoutDb(selfStore), {
      ...baseDesignPartner,
      beneficiaryUserId: 'github:acme_agency',
      idempotencyKey: 'partner-payout:self',
    })

    expect(self).toMatchObject({
      amount: 0,
      state: 'refused',
      stateReasonRef: 'reason.public.partner_payout.self_payout',
    })

    const cappedStore = new PayoutStore()
    cappedStore.rows.push({
      ...selfStore.rows[0]!,
      amount: 1_000_000,
      id: 'existing',
      idempotency_key: 'existing',
      payout_ref: 'existing',
      beneficiary_user_id: 'github:other',
      state: 'settled',
      state_reason_ref: null,
    })
    const capped = await createPartnerPayoutEligibility(payoutDb(cappedStore), {
      ...baseDesignPartner,
      idempotencyKey: 'partner-payout:capped',
      payoutRef: 'partner_payout_ref_capped',
    })

    expect(capped).toMatchObject({
      amount: 0,
      state: 'refused',
      stateReasonRef: 'reason.public.partner_payout.partner_period_cap_exceeded',
    })
  })

  test('period caps are scoped per asset', async () => {
    const store = new PayoutStore()
    // A large settled USD payout should NOT cap a sats payout in same period.
    await createPartnerPayoutEligibility(payoutDb(store), {
      ...baseDesignPartner,
      idempotencyKey: 'partner-payout:usd-big',
    })
    const sats = await createPartnerPayoutEligibility(payoutDb(store), {
      ...baseDesignPartner,
      asset: 'sats',
      idempotencyKey: 'partner-payout:sats-ok',
      payoutRef: 'partner_payout_ref_sats',
      qualifyingAmount: 2500,
    })

    expect(sats.state).toBe('eligible')
  })

  test('persists the attribution basis refs, deduped, with required refs', async () => {
    const store = new PayoutStore()
    const entry = await createPartnerPayoutEligibility(payoutDb(store), {
      ...baseDesignPartner,
      evidenceRefs: [
        'partner_agreement_acme',
        // a duplicate of the qualifying event ref must not be doubled up
        'partner_event_engagement_1',
      ],
      policyRefs: ['policy.partner_attribution.v1'],
    })

    // qualifying event ref stays first, the agreement ref is appended, and the
    // duplicate qualifying ref is collapsed.
    expect(entry.evidenceRefs).toEqual([
      'partner_event_engagement_1',
      'partner_agreement_acme',
    ])
    // required payout policy ref is always present alongside the attribution one.
    expect(entry.policyRefs).toEqual([
      'policy.partner_payout.v1',
      'policy.partner_attribution.v1',
    ])
  })

  test('rejects an unsafe attribution evidence ref', async () => {
    const store = new PayoutStore()

    await expect(
      createPartnerPayoutEligibility(payoutDb(store), {
        ...baseDesignPartner,
        evidenceRefs: ['payment_preimage_should_be_refused'],
      }),
    ).rejects.toMatchObject({ _tag: 'PartnerPayoutLedgerValidationError' })
    expect(store.rows).toHaveLength(0)
  })

  test('is idempotent on repeated eligibility writes', async () => {
    const store = new PayoutStore()
    const first = await createPartnerPayoutEligibility(
      payoutDb(store),
      baseDesignPartner,
    )
    const second = await createPartnerPayoutEligibility(
      payoutDb(store),
      baseDesignPartner,
    )

    expect(second.id).toBe(first.id)
    expect(store.rows).toHaveLength(1)
  })
})

describe('Partner payout ledger transitions', () => {
  test('walks eligible -> approved -> dispatched -> settled -> reversed append-only', async () => {
    const store = new PayoutStore()
    await createPartnerPayoutEligibility(payoutDb(store), baseDesignPartner)
    await transitionPartnerPayout(payoutDb(store), {
      action: 'approve_dispatch',
      id: 'partner_payout_entry_2',
      idempotencyKey: 'partner-payout:approve:1',
      nowIso: '2026-06-10T09:01:00.000Z',
      payoutRef: 'partner_payout_ref_1',
    })
    await transitionPartnerPayout(payoutDb(store), {
      action: 'mark_dispatched',
      id: 'partner_payout_entry_3',
      idempotencyKey: 'partner-payout:dispatch:1',
      nowIso: '2026-06-10T09:02:00.000Z',
      payoutRef: 'partner_payout_ref_1',
    })

    await expect(
      transitionPartnerPayout(payoutDb(store), {
        action: 'mark_settled',
        idempotencyKey: 'partner-payout:settle-missing-evidence:1',
        nowIso: '2026-06-10T09:03:00.000Z',
        payoutRef: 'partner_payout_ref_1',
      }),
    ).rejects.toMatchObject({
      _tag: 'PartnerPayoutLedgerValidationError',
    })

    await transitionPartnerPayout(payoutDb(store), {
      action: 'mark_settled',
      evidenceRefs: ['settlement_evidence.public.partner_payout.test'],
      id: 'partner_payout_entry_4',
      idempotencyKey: 'partner-payout:settle:1',
      nowIso: '2026-06-10T09:04:00.000Z',
      payoutRef: 'partner_payout_ref_1',
    })
    const reversed = await transitionPartnerPayout(payoutDb(store), {
      action: 'reverse',
      id: 'partner_payout_entry_5',
      idempotencyKey: 'partner-payout:reverse:1',
      nowIso: '2026-06-10T09:05:00.000Z',
      payoutRef: 'partner_payout_ref_1',
      stateReasonRef: 'reason.public.partner_payout.refund_or_abuse',
    })

    expect(store.rows.map(row => row.state)).toEqual([
      'eligible',
      'approved',
      'dispatched',
      'settled',
      'reversed',
    ])
    expect(reversed).toMatchObject({
      amount: -2000,
      reversalOfEntryId: 'partner_payout_entry_4',
      state: 'reversed',
    })
  })

  test('rejects illegal transitions from the current state', async () => {
    const store = new PayoutStore()
    await createPartnerPayoutEligibility(payoutDb(store), baseDesignPartner)

    // Cannot mark_settled directly from eligible.
    await expect(
      transitionPartnerPayout(payoutDb(store), {
        action: 'mark_settled',
        evidenceRefs: ['settlement_evidence.public.partner_payout.test'],
        idempotencyKey: 'partner-payout:bad-settle',
        nowIso: '2026-06-10T09:01:00.000Z',
        payoutRef: 'partner_payout_ref_1',
      }),
    ).rejects.toMatchObject({
      _tag: 'PartnerPayoutLedgerValidationError',
    })
  })

  test('rejects transitions against an unknown payout ref', async () => {
    const store = new PayoutStore()

    await expect(
      transitionPartnerPayout(payoutDb(store), {
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:unknown',
        nowIso: '2026-06-10T09:01:00.000Z',
        payoutRef: 'partner_payout_ref_missing',
      }),
    ).rejects.toMatchObject({
      _tag: 'PartnerPayoutLedgerValidationError',
    })
  })

  test('transitions are idempotent and do not append duplicate rows', async () => {
    const store = new PayoutStore()
    await createPartnerPayoutEligibility(payoutDb(store), baseDesignPartner)
    const first = await transitionPartnerPayout(payoutDb(store), {
      action: 'approve_dispatch',
      id: 'partner_payout_entry_2',
      idempotencyKey: 'partner-payout:approve:dup',
      nowIso: '2026-06-10T09:01:00.000Z',
      payoutRef: 'partner_payout_ref_1',
    })
    const repeat = await transitionPartnerPayout(payoutDb(store), {
      action: 'approve_dispatch',
      id: 'partner_payout_entry_should_not_insert',
      idempotencyKey: 'partner-payout:approve:dup',
      nowIso: '2026-06-10T09:02:00.000Z',
      payoutRef: 'partner_payout_ref_1',
    })

    expect(repeat.id).toBe(first.id)
    expect(store.rows.map(row => row.state)).toEqual(['eligible', 'approved'])
  })

  test('projection exposes the authority boundary and current state', async () => {
    const store = new PayoutStore()
    const entry = await createPartnerPayoutEligibility(
      payoutDb(store),
      baseDesignPartner,
    )
    const projection = projectPartnerPayout(entry)

    expect(projection).toMatchObject({
      amount: 2000,
      asset: 'usd',
      currentEntryId: entry.id,
      partnerRole: 'design_partner',
      state: 'eligible',
    })
    expect(projection.authorityBoundary).toContain('not spendable')
  })
})
