import { describe, expect, test } from 'vitest'

import {
  AGENT_CLAIM_X_REWARD_AMOUNT_SATS,
  AGENT_CLAIM_X_REWARD_CAMPAIGN_REF,
  type AgentClaimRewardLedgerRecord,
  agentClaimRewardReceiptHasPrivateMaterial,
  createPendingAgentClaimRewardRecord,
  projectAgentClaimRewardPublicReceipt,
} from './agent-claim-reward-ledger'

class MemoryAgentClaimRewardLedgerStore {
  readonly rewards = new Map<string, AgentClaimRewardLedgerRecord>()

  createPendingReward(
    input: Parameters<typeof createPendingAgentClaimRewardRecord>[0],
  ): AgentClaimRewardLedgerRecord {
    const existing = Array.from(this.rewards.values()).find(
      reward => reward.idempotencyKey === input.idempotencyKey,
    )

    if (existing !== undefined) {
      return existing
    }

    const duplicate = Array.from(this.rewards.values()).find(
      reward =>
        reward.campaignRef === AGENT_CLAIM_X_REWARD_CAMPAIGN_REF &&
        reward.state !== 'rejected' &&
        reward.state !== 'reversed' &&
        (reward.xAccountRef === input.xAccountRef ||
          reward.ownerRef === input.ownerRef ||
          reward.agentClaimRef === input.agentClaimRef),
    )

    if (duplicate !== undefined) {
      throw new Error('UNIQUE constraint failed: agent claim reward policy')
    }

    const record = createPendingAgentClaimRewardRecord(input)
    this.rewards.set(record.id, record)

    return record
  }
}

const rewardInput = {
  agentClaimRef: 'agent_claim_verified_one',
  id: 'claim_reward_receipt_one',
  idempotencyKey: 'claim-reward-one',
  now: '2026-06-09T00:00:00.000Z',
  ownerRef: 'owner:github-owner-one',
  redactedDestinationRef: 'lightning_address:hash_abcd',
  destinationKind: 'lightning_address' as const,
  tweetRef: 'x_tweet:100',
  xAccountRef: 'x:ownerone',
}

describe('agent claim reward ledger', () => {
  test('creates a fixed 1000 sats verified campaign reward', () => {
    const record = createPendingAgentClaimRewardRecord(rewardInput)

    expect(record).toMatchObject({
      amountSats: AGENT_CLAIM_X_REWARD_AMOUNT_SATS,
      campaignRef: AGENT_CLAIM_X_REWARD_CAMPAIGN_REF,
      state: 'verified',
    })
    expect(record.amountSats).toBe(1000)
  })

  test('idempotency key replays the same reward record', () => {
    const store = new MemoryAgentClaimRewardLedgerStore()
    const first = store.createPendingReward(rewardInput)
    const replay = store.createPendingReward({
      ...rewardInput,
      id: 'claim_reward_receipt_replay_attempt',
      tweetRef: 'x_tweet:999',
    })

    expect(replay).toBe(first)
    expect(store.rewards).toHaveLength(1)
  })

  test('duplicate X account, owner, or agent claim cannot mint repeated rewards', () => {
    const store = new MemoryAgentClaimRewardLedgerStore()
    store.createPendingReward(rewardInput)

    expect(() =>
      store.createPendingReward({
        ...rewardInput,
        agentClaimRef: 'agent_claim_second',
        id: 'claim_reward_receipt_two',
        idempotencyKey: 'claim-reward-two',
        ownerRef: 'owner:github-owner-two',
      }),
    ).toThrow('UNIQUE constraint failed')
    expect(() =>
      store.createPendingReward({
        ...rewardInput,
        id: 'claim_reward_receipt_three',
        idempotencyKey: 'claim-reward-three',
        xAccountRef: 'x:ownerthree',
      }),
    ).toThrow('UNIQUE constraint failed')
  })

  test('public receipt redacts private payment and identity material', () => {
    const record = createPendingAgentClaimRewardRecord({
      ...rewardInput,
      redactedDestinationRef: 'lightning_address:hash_public',
    })
    const receipt = projectAgentClaimRewardPublicReceipt(record)

    expect(receipt).toMatchObject({
      amountSats: 1000,
      destinationKind: 'lightning_address',
      redactedDestinationRef: 'lightning_address:hash_public',
      state: 'verified',
    })
    expect(agentClaimRewardReceiptHasPrivateMaterial(receipt)).toBe(false)
    expect(JSON.stringify(receipt)).not.toContain('lnbc')
    expect(JSON.stringify(receipt)).not.toContain('preimage')
    expect(JSON.stringify(receipt)).not.toContain('oauth')
  })

  test('settled copy is blocked until settlement refs exist', () => {
    const record: AgentClaimRewardLedgerRecord = {
      ...createPendingAgentClaimRewardRecord(rewardInput),
      dispatchAttemptRef: 'dispatch.agent_claim_reward.one',
      payoutIntentRef: 'payout_intent.agent_claim_reward.one',
      settlementRef: null,
      state: 'settled',
    }
    const receipt = projectAgentClaimRewardPublicReceipt(record)

    expect(receipt.state).toBe('dispatched')
    expect(receipt.settlementRef).toBeNull()
  })
})
