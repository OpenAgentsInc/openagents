import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TargetedSiteSalesRewardPolicyRelatedEventNotFound,
  TargetedSiteSalesRewardPolicyValidationError,
  projectTargetedSiteSalesRewardPolicy,
  publicTargetedSiteSalesRewardPolicyProjection,
  recordTargetedSiteSalesRewardPolicyEvent,
} from './targeted-site-sales-reward-policy'

type Campaign = Readonly<{ archived_at: string | null; id: string }>
type Prospect = Readonly<{
  archived_at: string | null
  campaign_id: string
  id: string
}>
type RewardEvent = Readonly<{
  accepted_work_ref: string | null
  agent_ref: string
  archived_at: string | null
  buyer_payment_ref: string | null
  campaign_id: string
  created_at: string
  dispute_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  occurred_at: string
  outcome_kind:
    | 'lead_proposed'
    | 'meeting_accepted'
    | 'customer_accepted'
    | 'reward_eligible'
    | 'payout_intent_created'
    | 'reward_held'
    | 'reward_disputed'
    | 'reward_reversed'
    | 'refund_recorded'
    | 'complaint_recorded'
    | 'settlement_caveat_recorded'
  payout_intent_ref: string | null
  policy_state:
    | 'proposed'
    | 'accepted'
    | 'held'
    | 'disputed'
    | 'reversed'
    | 'eligible'
  prospect_id: string | null
  public_receipt_ref: string
  referral_attribution_ref: string | null
  related_event_id: string | null
  reward_amount: number
  reward_asset: 'credits' | 'sats' | 'internal_payable'
  settlement_caveat_ref: string | null
}>

class SalesRewardStore {
  campaigns: Array<Campaign> = [
    { archived_at: null, id: 'targeted_site_campaign_1' },
    {
      archived_at: '2026-06-05T22:00:00.000Z',
      id: 'targeted_site_campaign_archived',
    },
  ]
  events: Array<RewardEvent> = []
  prospects: Array<Prospect> = [
    {
      archived_at: null,
      campaign_id: 'targeted_site_campaign_1',
      id: 'targeted_site_prospect_1',
    },
    {
      archived_at: null,
      campaign_id: 'targeted_site_campaign_other',
      id: 'targeted_site_prospect_other',
    },
  ]
}

const runtime = {
  makeEventId: () => 'targeted_site_sales_reward_policy_generated',
  nowIso: () => '2026-06-05T22:10:00.000Z',
}

class SalesRewardStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: SalesRewardStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM targeted_site_campaigns')) {
      const campaignId = String(this.values[0])
      const campaign =
        this.store.campaigns.find(
          item => item.id === campaignId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(campaign as T | null)
    }

    if (this.query.includes('FROM targeted_site_prospects')) {
      const prospectId = String(this.values[0])
      const campaignId = String(this.values[1])
      const prospect =
        this.store.prospects.find(
          item =>
            item.id === prospectId &&
            item.campaign_id === campaignId &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(prospect as T | null)
    }

    if (this.query.includes('FROM targeted_site_sales_reward_policy_events')) {
      return Promise.resolve(this.findEvent() as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_sales_reward_policy_events')) {
      const rows = this.eventsForProjection() as ReadonlyArray<T>

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes(
        'INSERT OR IGNORE INTO targeted_site_sales_reward_policy_events',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.events.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.events.push({
          accepted_work_ref: this.values[11] as string | null,
          agent_ref: String(this.values[3]),
          archived_at: null,
          buyer_payment_ref: this.values[9] as string | null,
          campaign_id: String(this.values[2]),
          created_at: String(this.values[19]),
          dispute_ref: this.values[14] as string | null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[17]),
          occurred_at: String(this.values[18]),
          outcome_kind: this.values[5] as RewardEvent['outcome_kind'],
          payout_intent_ref: this.values[12] as string | null,
          policy_state: this.values[6] as RewardEvent['policy_state'],
          prospect_id: this.values[4] as string | null,
          public_receipt_ref: String(this.values[15]),
          referral_attribution_ref: this.values[10] as string | null,
          related_event_id: this.values[16] as string | null,
          reward_amount: Number(this.values[8]),
          reward_asset: this.values[7] as RewardEvent['reward_asset'],
          settlement_caveat_ref: this.values[13] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
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

  private findEvent(): RewardEvent | null {
    if (this.query.includes('idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])

      return (
        this.store.events.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null
      )
    }

    const eventId = String(this.values[0])

    return (
      this.store.events.find(
        item => item.id === eventId && item.archived_at === null,
      ) ?? null
    )
  }

  private eventsForProjection(): ReadonlyArray<RewardEvent> {
    const campaignId = String(this.values[0])
    const agentRef = String(this.values[1])
    const prospectId = this.values[2] === undefined ? undefined : String(this.values[2])

    return this.store.events
      .filter(
        item =>
          item.campaign_id === campaignId &&
          item.agent_ref === agentRef &&
          item.archived_at === null &&
          (prospectId === undefined || item.prospect_id === prospectId),
      )
      .sort((left, right) =>
        `${left.occurred_at}:${left.created_at}`.localeCompare(
          `${right.occurred_at}:${right.created_at}`,
        ),
      )
  }
}

const salesRewardDb = (store: SalesRewardStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new SalesRewardStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const recordEvent = (
  store: SalesRewardStore,
  input: Parameters<typeof recordTargetedSiteSalesRewardPolicyEvent>[1],
) =>
  Effect.runPromise(
    recordTargetedSiteSalesRewardPolicyEvent(
      salesRewardDb(store),
      input,
      runtime,
    ),
  )

describe('targeted Site sales reward policy', () => {
  test('records proposed leads idempotently and projects public-safe state', async () => {
    const store = new SalesRewardStore()
    const event = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      id: 'targeted_site_sales_reward_event_1',
      idempotencyKey: 'reward:lead:1',
      outcomeKind: 'lead_proposed',
      prospectId: 'targeted_site_prospect_1',
      referralAttributionRef: 'referral_source_site_sales_1',
    })
    const replay = await recordEvent(store, {
      agentRef: 'agent_site_sales_changed',
      campaignId: 'targeted_site_campaign_1',
      idempotencyKey: 'reward:lead:1',
      outcomeKind: 'lead_proposed',
    })
    const projection = await Effect.runPromise(
      projectTargetedSiteSalesRewardPolicy(salesRewardDb(store), {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        prospectId: 'targeted_site_prospect_1',
      }),
    )
    const publicProjection =
      publicTargetedSiteSalesRewardPolicyProjection(projection)

    expect(event).toMatchObject({
      policyState: 'proposed',
      publicReceiptRef: 'targeted_site_sales_reward:lead_proposed:reward:lead:1',
      rewardAmount: 0,
      rewardAsset: 'credits',
    })
    expect(replay.agentRef).toBe('agent_site_sales_1')
    expect(publicProjection).toEqual({
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      eventCount: 1,
      latestEventAt: '2026-06-05T22:10:00.000Z',
      latestOutcomeKind: 'lead_proposed',
      policyState: 'proposed',
      prospectId: 'targeted_site_prospect_1',
      publicReceiptRef: 'targeted_site_sales_reward:lead_proposed:reward:lead:1',
      rewardAmount: 0,
      rewardAsset: 'credits',
      settlementPosture: 'no_settlement_claim',
    })
    expect(publicProjection).not.toHaveProperty('buyerPaymentRef')
    expect(publicProjection).not.toHaveProperty('metadata')
  })

  test('keeps accepted outcome, eligibility, payout intent, and settlement separate', async () => {
    const store = new SalesRewardStore()
    const customerAccepted = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      buyerPaymentRef: 'site_checkout_payment_receipt_1',
      campaignId: 'targeted_site_campaign_1',
      id: 'targeted_site_sales_reward_event_customer',
      idempotencyKey: 'reward:customer:1',
      outcomeKind: 'customer_accepted',
      prospectId: 'targeted_site_prospect_1',
      publicReceiptRef: 'sales_reward_customer_public_receipt_1',
      rewardAmount: 0,
      rewardAsset: 'credits',
    })
    const eligible = await recordEvent(store, {
      acceptedWorkRef: 'accepted_work_site_sales_1',
      agentRef: 'agent_site_sales_1',
      buyerPaymentRef: 'site_checkout_payment_receipt_1',
      campaignId: 'targeted_site_campaign_1',
      id: 'targeted_site_sales_reward_event_eligible',
      idempotencyKey: 'reward:eligible:1',
      outcomeKind: 'reward_eligible',
      prospectId: 'targeted_site_prospect_1',
      relatedEventId: customerAccepted.id,
      rewardAmount: 2500,
      rewardAsset: 'sats',
    })
    const payoutIntent = await recordEvent(store, {
      acceptedWorkRef: 'accepted_work_site_sales_1',
      agentRef: 'agent_site_sales_1',
      buyerPaymentRef: 'site_checkout_payment_receipt_1',
      campaignId: 'targeted_site_campaign_1',
      id: 'targeted_site_sales_reward_event_payout',
      idempotencyKey: 'reward:payout:1',
      outcomeKind: 'payout_intent_created',
      payoutIntentRef: 'payout_intent_site_sales_1',
      prospectId: 'targeted_site_prospect_1',
      relatedEventId: eligible.id,
      rewardAmount: 2500,
      rewardAsset: 'sats',
    })
    const projection = await Effect.runPromise(
      projectTargetedSiteSalesRewardPolicy(salesRewardDb(store), {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        prospectId: 'targeted_site_prospect_1',
      }),
    )

    expect(customerAccepted.policyState).toBe('accepted')
    expect(eligible).toMatchObject({
      policyState: 'eligible',
      settlementCaveatRef: null,
    })
    expect(payoutIntent).toMatchObject({
      payoutIntentRef: 'payout_intent_site_sales_1',
      policyState: 'eligible',
      settlementCaveatRef: null,
    })
    expect(projection).toMatchObject({
      acceptedWorkRef: 'accepted_work_site_sales_1',
      buyerPaymentRef: 'site_checkout_payment_receipt_1',
      eventCount: 3,
      payoutIntentRef: 'payout_intent_site_sales_1',
      policyState: 'eligible',
      rewardAmount: 2500,
      rewardAsset: 'sats',
      settlementPosture: 'payout_intent_not_settled',
    })
  })

  test('records hold, dispute, refund, complaint, reversal, and settlement caveat states', async () => {
    const store = new SalesRewardStore()
    const lead = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      id: 'targeted_site_sales_reward_event_hold_root',
      idempotencyKey: 'reward:hold-root:1',
      outcomeKind: 'lead_proposed',
    })
    const held = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      idempotencyKey: 'reward:held:1',
      outcomeKind: 'reward_held',
      relatedEventId: lead.id,
    })
    const disputed = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      disputeRef: 'dispute_ref_site_sales_1',
      idempotencyKey: 'reward:dispute:1',
      outcomeKind: 'reward_disputed',
      relatedEventId: held.id,
    })
    const complaint = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      disputeRef: 'complaint_ref_site_sales_1',
      idempotencyKey: 'reward:complaint:1',
      outcomeKind: 'complaint_recorded',
      relatedEventId: disputed.id,
    })
    const refund = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      idempotencyKey: 'reward:refund:1',
      outcomeKind: 'refund_recorded',
      relatedEventId: complaint.id,
    })
    const reversed = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      idempotencyKey: 'reward:reversed:1',
      outcomeKind: 'reward_reversed',
      relatedEventId: refund.id,
    })
    const caveat = await recordEvent(store, {
      agentRef: 'agent_site_sales_1',
      campaignId: 'targeted_site_campaign_1',
      idempotencyKey: 'reward:caveat:1',
      outcomeKind: 'settlement_caveat_recorded',
      relatedEventId: reversed.id,
      settlementCaveatRef: 'settlement_caveat_policy_hold_1',
    })

    expect(held.policyState).toBe('held')
    expect(disputed.policyState).toBe('disputed')
    expect(complaint.policyState).toBe('disputed')
    expect(refund.policyState).toBe('reversed')
    expect(reversed.policyState).toBe('reversed')
    expect(caveat).toMatchObject({
      policyState: 'held',
      settlementCaveatRef: 'settlement_caveat_policy_hold_1',
    })
  })

  test('validates transition preconditions and related event existence', async () => {
    const store = new SalesRewardStore()
    await expect(
      recordEvent(store, {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        idempotencyKey: 'reward:eligible:invalid',
        outcomeKind: 'reward_eligible',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteSalesRewardPolicyValidationError)

    await expect(
      recordEvent(store, {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        idempotencyKey: 'reward:payout:missing-related',
        outcomeKind: 'payout_intent_created',
        payoutIntentRef: 'payout_intent_site_sales_1',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteSalesRewardPolicyValidationError)

    await expect(
      recordEvent(store, {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        idempotencyKey: 'reward:dispute:missing',
        outcomeKind: 'reward_disputed',
        relatedEventId: 'targeted_site_sales_reward_missing',
        disputeRef: 'dispute_ref_site_sales_1',
      }),
    ).rejects.toBeInstanceOf(
      TargetedSiteSalesRewardPolicyRelatedEventNotFound,
    )
  })

  test('rejects private customer, email, payment, wallet, and provider material', async () => {
    await expect(
      recordEvent(new SalesRewardStore(), {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        idempotencyKey: 'reward:private-material',
        metadata: { customerEmail: 'ben@example.com' },
        outcomeKind: 'lead_proposed',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteSalesRewardPolicyValidationError)

    await expect(
      recordEvent(new SalesRewardStore(), {
        agentRef: 'agent_site_sales_1',
        campaignId: 'targeted_site_campaign_1',
        idempotencyKey: 'reward:raw-payment',
        outcomeKind: 'lead_proposed',
        publicReceiptRef: 'lnbc1rawinvoice',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteSalesRewardPolicyValidationError)
  })
})
