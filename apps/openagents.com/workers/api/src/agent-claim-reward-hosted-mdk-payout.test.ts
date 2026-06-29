import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentClaimRewardHostedMdkClient,
  dispatchAgentClaimRewardHostedMdkPayout,
  projectHostedMdkClaimRewardReceipt,
  settleAgentClaimRewardHostedMdkPayout,
} from './agent-claim-reward-hosted-mdk-payout'
import {
  agentClaimRewardReceiptHasPrivateMaterial,
  createPendingAgentClaimRewardRecord,
} from './agent-claim-reward-ledger'

const approvedReward = {
  ...createPendingAgentClaimRewardRecord({
    agentClaimRef: 'agent_claim_verified_one',
    destinationKind: 'lightning_address',
    id: 'claim_reward_receipt_one',
    idempotencyKey: 'claim-reward-one',
    now: '2026-06-09T00:00:00.000Z',
    ownerRef: 'owner:github-owner-one',
    redactedDestinationRef: 'lightning_address:hash_public',
    tweetRef: 'x_tweet:100',
    xAccountRef: 'x:ownerone',
  }),
  state: 'approved' as const,
}

const readyGate = {
  hostedFundedKeyVerified: true,
  hostedProgrammaticPayoutsEnabled: true,
  requestedMode: 'hosted_mdk_direct_payout' as const,
}

describe('agent claim reward hosted MDK payout', () => {
  test('blocks dispatch when hosted MDK payout mode is not ready', async () => {
    const client: AgentClaimRewardHostedMdkClient = {
      programmaticPayout: () => Promise.reject(new Error('should not send')),
      waitForPayoutResult: () => Promise.reject(new Error('should not wait')),
    }
    const result = await Effect.runPromiseExit(
      dispatchAgentClaimRewardHostedMdkPayout(approvedReward, {
        client,
        gate: {
          hostedFundedKeyVerified: false,
          hostedProgrammaticPayoutsEnabled: false,
          requestedMode: 'hosted_mdk_direct_payout',
        },
        resolveDestination: () => Effect.succeed('owner@example.com'),
      }),
    )

    expect(result._tag).toBe('Failure')
  })

  test('dispatches exactly 1000 sats through hosted MDK with reward idempotency', async () => {
    const calls: Array<{
      amountSats: number
      destination: string
      idempotencyKey: string
    }> = []
    const client: AgentClaimRewardHostedMdkClient = {
      programmaticPayout: input => {
        calls.push(input)

        return Promise.resolve({
          paymentHash: 'payment_hash_private_123',
          paymentId: 'payment-id-1',
          status: 'REQUESTED',
        })
      },
      waitForPayoutResult: () => Promise.reject(new Error('not used')),
    }
    const dispatched = await Effect.runPromise(
      dispatchAgentClaimRewardHostedMdkPayout(approvedReward, {
        client,
        gate: readyGate,
        resolveDestination: () => Effect.succeed('owner@example.com'),
      }),
    )
    const receipt = projectHostedMdkClaimRewardReceipt(dispatched)

    expect(calls).toEqual([
      {
        amountSats: 1000,
        destination: 'owner@example.com',
        idempotencyKey: 'claim-reward-one',
      },
    ])
    expect(dispatched).toMatchObject({
      payoutIntentRef: 'claim_reward_payout_intent:claim_reward_receipt_one',
      state: 'dispatched',
    })
    expect(receipt.dispatchAttemptRef).toBe('hosted_mdk:payment-id-1')
    expect(agentClaimRewardReceiptHasPrivateMaterial(receipt)).toBe(false)
    expect(JSON.stringify(receipt)).not.toContain('payment_hash_private')
    expect(JSON.stringify(receipt)).not.toContain('owner@example.com')
  })

  test('settles dispatched hosted MDK reward only after success result', async () => {
    const client: AgentClaimRewardHostedMdkClient = {
      programmaticPayout: () => Promise.reject(new Error('not used')),
      waitForPayoutResult: input =>
        Promise.resolve({
          paymentHash: `hash-for-${input.idempotencyKey}`,
          paymentId: input.paymentId,
          status: 'SUCCESS',
        }),
    }
    const settled = await Effect.runPromise(
      settleAgentClaimRewardHostedMdkPayout(
        {
          ...approvedReward,
          dispatchAttemptRef: 'hosted_mdk:payment-id-1',
          payoutIntentRef:
            'claim_reward_payout_intent:claim_reward_receipt_one',
          state: 'dispatched',
        },
        {
          client,
          gate: readyGate,
          resolveDestination: () => Effect.succeed('owner@example.com'),
          waitTimeoutMs: 250,
        },
      ),
    )
    const receipt = projectHostedMdkClaimRewardReceipt(settled)

    expect(receipt.state).toBe('settled')
    expect(receipt.settlementRef).toBe('hosted_mdk:hash-for-claim-reward-one')
    expect(agentClaimRewardReceiptHasPrivateMaterial(receipt)).toBe(false)
  })
})
