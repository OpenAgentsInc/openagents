import { describe, expect, test } from 'vitest'

import {
  SiteReferralPolicyValidationError,
  evaluateSiteReferralPolicy,
  listSiteReferralPolicyEventsByAttribution,
  listSiteReferralPolicyEventsBySource,
  listSiteReferralPolicyEventsByWorkflowEvent,
  publicSiteReferralPolicyDecision,
  recordOperatorSiteReferralPolicyOverride,
  recordSiteReferralPolicyEvent,
  type RecordSiteReferralPolicyEventInput,
} from './site-referral-policy'

type StoredPolicyEvent = Readonly<{
  archived_at: string | null
  created_at: string
  customer_status: 'active' | 'under_review' | 'not_eligible' | 'expired'
  decided_at: string
  decision_state:
    | 'pending'
    | 'active'
    | 'held'
    | 'disputed'
    | 'capped'
    | 'reversed'
    | 'expired'
    | 'archived'
    | 'operator_overridden'
  eligibility: 'eligible' | 'not_eligible' | 'manual_review'
  id: string
  idempotency_key: string
  metadata_json: string
  operator_actor_user_id: string | null
  operator_note_ref: string | null
  policy_reason:
    | 'eligible'
    | 'self_referral'
    | 'duplicate_account'
    | 'collusion_risk'
    | 'chargeback_refund'
    | 'sanctions_compliance'
    | 'expired'
    | 'cap_exceeded'
    | 'clawback'
    | 'operator_override'
    | 'refund_or_reversal'
    | 'first_verified_wins'
    | 'manual_review'
  previous_state: string | null
  referral_attribution_id: string | null
  referral_invite_id: string | null
  referral_source_id: string | null
  referral_workflow_event_id: string | null
  site_id: string | null
  software_order_id: string | null
  subject_kind:
    | 'referral_source'
    | 'referral_invite'
    | 'referral_attribution'
    | 'user_attribution'
    | 'order_attribution'
    | 'agent_attribution'
    | 'workflow_event'
  subject_ref: string
}>

class PolicyEventStore {
  rows: Array<StoredPolicyEvent> = []
}

class PolicyEventStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: PolicyEventStore,
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

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO site_referral_policy_events')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.rows.every(row => row.idempotency_key !== idempotencyKey)
      ) {
        this.store.rows.push({
          archived_at: null,
          created_at: String(this.values[19]),
          customer_status:
            this.values[14] as StoredPolicyEvent['customer_status'],
          decided_at: String(this.values[18]),
          decision_state:
            this.values[11] as StoredPolicyEvent['decision_state'],
          eligibility: this.values[13] as StoredPolicyEvent['eligibility'],
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[17]),
          operator_actor_user_id: this.values[15] as string | null,
          operator_note_ref: this.values[16] as string | null,
          policy_reason: this.values[12] as StoredPolicyEvent['policy_reason'],
          previous_state: this.values[10] as string | null,
          referral_attribution_id: this.values[4] as string | null,
          referral_invite_id: this.values[6] as string | null,
          referral_source_id: this.values[5] as string | null,
          referral_workflow_event_id: this.values[7] as string | null,
          site_id: this.values[9] as string | null,
          software_order_id: this.values[8] as string | null,
          subject_kind: this.values[2] as StoredPolicyEvent['subject_kind'],
          subject_ref: String(this.values[3]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_referral_policy_events')) {
      const value = String(this.values[0])
      const key = this.query.includes('referral_attribution_id = ?')
        ? 'referral_attribution_id'
        : this.query.includes('referral_source_id = ?')
          ? 'referral_source_id'
          : 'referral_workflow_event_id'
      const limit = Number(this.values[1] ?? 100)
      const rows = this.store.rows
        .filter(row => row.archived_at === null && row[key] === value)
        .slice(0, limit)

      return Promise.resolve({
        results: rows as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
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
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }
}

const policyEventDb = (store: PolicyEventStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new PolicyEventStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const baseEvent = {
  customerStatus: 'under_review',
  decisionState: 'held',
  eligibility: 'manual_review',
  id: 'site_referral_policy_event_1',
  idempotencyKey: 'site-referral-policy:self-referral:1',
  metadata: {
    evidenceRef: 'redacted_policy_signal_1',
  },
  policyReason: 'self_referral',
  previousState: 'pending',
  referralAttributionId: 'referral_attribution_otec',
  referralSourceId: 'site_referral_source_otec',
  referralWorkflowEventId: 'referral_workflow_event_checkout_1',
  siteId: 'site_project_otec',
  softwareOrderId: 'software_order_otec',
  subjectKind: 'referral_attribution',
  subjectRef: 'referral_attribution_otec',
} satisfies RecordSiteReferralPolicyEventInput

describe('Site referral policy decisions', () => {
  test('blocks self-referral before future reward eligibility', () => {
    expect(
      evaluateSiteReferralPolicy({
        attributionExpiresAt: '2026-07-05T20:00:00.000Z',
        attributionPolicyState: 'claimed',
        nowIso: '2026-06-05T20:00:00.000Z',
        referredUserId: 'github:owner',
        referrerUserId: 'github:owner',
        referralAttributionId: 'referral_attribution_otec',
        sourcePolicyState: 'active',
      }),
    ).toEqual({
      customerStatus: 'under_review',
      decisionState: 'held',
      eligibility: 'manual_review',
      eligibleForFutureReward: false,
      reason: 'self_referral',
    })
  })

  test('keeps first-verified attribution from being replaced by duplicates', () => {
    expect(
      evaluateSiteReferralPolicy({
        attributionExpiresAt: '2026-07-05T20:00:00.000Z',
        attributionPolicyState: 'claimed',
        existingUserAttributionId: 'referral_attribution_first',
        nowIso: '2026-06-05T20:00:00.000Z',
        referredUserId: 'github:buyer',
        referrerUserId: 'github:owner',
        referralAttributionId: 'referral_attribution_second',
        sourcePolicyState: 'active',
      }),
    ).toMatchObject({
      customerStatus: 'not_eligible',
      decisionState: 'held',
      eligibility: 'not_eligible',
      reason: 'first_verified_wins',
    })
  })

  test('classifies expired, capped, disputed, and reversed workflows', () => {
    const base = {
      attributionExpiresAt: '2026-07-05T20:00:00.000Z',
      attributionPolicyState: 'claimed' as const,
      nowIso: '2026-06-05T20:00:00.000Z',
      referredUserId: 'github:buyer',
      referrerUserId: 'github:owner',
      referralAttributionId: 'referral_attribution_otec',
      sourcePolicyState: 'active' as const,
    }

    expect(
      evaluateSiteReferralPolicy({
        ...base,
        attributionExpiresAt: '2026-06-01T20:00:00.000Z',
      }),
    ).toMatchObject({ decisionState: 'expired', reason: 'expired' })
    expect(
      evaluateSiteReferralPolicy({
        ...base,
        caps: { maxEligibleWorkflowEvents: 1 },
        workflowEvents: [
          {
            amount: 10,
            eventKind: 'site_checkout',
            id: 'referral_workflow_event_checkout_1',
            policyState: 'eligible',
          },
        ],
      }),
    ).toMatchObject({ decisionState: 'capped', reason: 'cap_exceeded' })
    expect(
      evaluateSiteReferralPolicy({
        ...base,
        signals: { collusionRisk: true },
      }),
    ).toMatchObject({ decisionState: 'disputed', reason: 'collusion_risk' })
    expect(
      evaluateSiteReferralPolicy({
        ...base,
        workflowEvents: [
          {
            amount: 10,
            eventKind: 'refund',
            id: 'referral_workflow_event_refund_1',
            policyState: 'refunded',
          },
        ],
      }),
    ).toMatchObject({
      decisionState: 'reversed',
      reason: 'refund_or_reversal',
    })
  })

  test('customer projection hides abuse reasons and operator-only detail', () => {
    const policyDecision = evaluateSiteReferralPolicy({
      attributionExpiresAt: '2026-07-05T20:00:00.000Z',
      attributionPolicyState: 'claimed',
      nowIso: '2026-06-05T20:00:00.000Z',
      referredUserId: 'github:buyer',
      referrerUserId: 'github:owner',
      referralAttributionId: 'referral_attribution_otec',
      signals: { sanctionsOrComplianceHold: true },
      sourcePolicyState: 'active',
    })

    expect(publicSiteReferralPolicyDecision(policyDecision)).toEqual({
      customerStatus: 'under_review',
      decisionState: 'held',
      eligibleForFutureReward: false,
    })
    expect(JSON.stringify(publicSiteReferralPolicyDecision(policyDecision))).not.toMatch(
      /sanctions|compliance|reason|operator/i,
    )
  })
})

describe('Site referral policy event ledger', () => {
  test('records policy events idempotently and lists by source, attribution, and workflow event', async () => {
    const store = new PolicyEventStore()
    const db = policyEventDb(store)
    const first = await recordSiteReferralPolicyEvent(db, baseEvent)
    const second = await recordSiteReferralPolicyEvent(db, {
      ...baseEvent,
      decisionState: 'active',
      id: 'site_referral_policy_event_duplicate',
    })

    expect(store.rows).toHaveLength(1)
    expect(second).toEqual(first)
    await expect(
      listSiteReferralPolicyEventsBySource(db, 'site_referral_source_otec'),
    ).resolves.toHaveLength(1)
    await expect(
      listSiteReferralPolicyEventsByAttribution(
        db,
        'referral_attribution_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listSiteReferralPolicyEventsByWorkflowEvent(
        db,
        'referral_workflow_event_checkout_1',
      ),
    ).resolves.toHaveLength(1)
  })

  test('records operator override audit refs without storing notes inline', async () => {
    const store = new PolicyEventStore()
    const event = await recordOperatorSiteReferralPolicyOverride(
      policyEventDb(store),
      {
        idempotencyKey: 'site-referral-policy:operator-override:1',
        override: {
          actorUserId: 'github:operator',
          decisionState: 'held',
          eligibility: 'manual_review',
          noteRef: 'operator_note_referral_policy_1',
        },
        previousState: 'active',
        referralAttributionId: 'referral_attribution_otec',
        referralSourceId: 'site_referral_source_otec',
        siteId: 'site_project_otec',
        subjectKind: 'referral_attribution',
        subjectRef: 'referral_attribution_otec',
      },
    )

    expect(event).toMatchObject({
      decisionState: 'operator_overridden',
      operatorActorUserId: 'github:operator',
      operatorNoteRef: 'operator_note_referral_policy_1',
      policyReason: 'operator_override',
    })
    expect(JSON.stringify(event)).not.toContain('actual operator note')
  })

  test('rejects private payment, provider, and abuse-detail material', async () => {
    await expect(
      recordSiteReferralPolicyEvent(policyEventDb(new PolicyEventStore()), {
        ...baseEvent,
        idempotencyKey: 'site-referral-policy:unsafe-payment:1',
        metadata: {
          invoice: 'lnbc1000n1rawinvoice',
        },
      }),
    ).rejects.toBeInstanceOf(SiteReferralPolicyValidationError)

    await expect(
      recordSiteReferralPolicyEvent(policyEventDb(new PolicyEventStore()), {
        ...baseEvent,
        idempotencyKey: 'site-referral-policy:unsafe-provider:1',
        subjectRef: 'gho_rawgithubtoken',
      }),
    ).rejects.toMatchObject({
      reason:
        'subjectRef must be a public-safe ref, not private payment, wallet, provider, or abuse-detail material.',
    })
  })
})
