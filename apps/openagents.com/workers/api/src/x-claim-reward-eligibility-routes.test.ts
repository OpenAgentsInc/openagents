// x_claim_reward eligibility read path (issue #4754): the verification
// write records eligibility; this surface serves it publicly with the
// promise's four-state separation, generatedAt + staleness honesty, and
// digest-redacted identity refs.
import { describe, expect, test } from 'vitest'

import type {
  XClaimRewardRecord,
  XClaimRewardState,
} from './agent-owner-claim-routes'
import {
  X_CLAIM_REWARD_ELIGIBILITY_PROJECTION_CONTRACT,
  X_CLAIM_REWARD_LIFECYCLE,
  xClaimRewardEligibilityListResponse,
  xClaimRewardEligibilityProjection,
  xClaimRewardEligibilityStatusResponse,
  xClaimRewardLifecycleStage,
} from './x-claim-reward-eligibility-routes'

const OWNER_USER_ID = 'github:eligible-owner-7'
const AGENT_USER_ID = 'agent:orrery-owner-claimed'
const X_ACCOUNT_REF = 'x:eligible_owner_handle'

const reward = (
  overrides: Partial<XClaimRewardRecord> = {},
): XClaimRewardRecord => ({
  agentUserId: AGENT_USER_ID,
  amountSats: 1000,
  challengeId: 'challenge-1',
  claimId: 'claim-1',
  createdAt: '2026-06-11T01:00:00.000Z',
  evidenceRefs: ['receipt.x_claim.challenge-1'],
  id: 'x_claim_reward_audit_case',
  ownerUserId: OWNER_USER_ID,
  receiptRef: 'x_claim_reward_receipt_x_claim_reward_audit_case',
  state: 'eligible',
  stateReasonRef: null,
  treasuryPaymentId: null,
  updatedAt: '2026-06-11T01:00:00.000Z',
  xAccountRef: X_ACCOUNT_REF,
  ...overrides,
})

const makeStore = (rewards: ReadonlyArray<XClaimRewardRecord>) => ({
  listXClaimRewards: (limit: number) =>
    Promise.resolve(rewards.slice(0, limit)),
  readXClaimRewardById: (rewardId: string) =>
    Promise.resolve(rewards.find(item => item.id === rewardId)),
  readXClaimRewardByReceiptRef: (receiptRef: string) =>
    Promise.resolve(rewards.find(item => item.receiptRef === receiptRef)),
})

const NOW = '2026-06-11T03:00:00.000Z'

const dependencies = (rewards: ReadonlyArray<XClaimRewardRecord>) => ({
  nowIso: () => NOW,
  store: makeStore(rewards),
})

const getRequest = (path: string): Request =>
  new Request(`https://openagents.com${path}`)

describe('x_claim_reward lifecycle mapping (#4754)', () => {
  test('maps the ledger states onto the promise four-state separation', () => {
    const stages: Record<XClaimRewardState, string> = {
      dispatch_requested: 'operator_approved',
      dispatched: 'dispatched',
      eligible: 'eligible',
      failed: 'failed',
      refused: 'refused',
      settled: 'settled',
    }
    for (const [state, stage] of Object.entries(stages)) {
      expect(xClaimRewardLifecycleStage(state as XClaimRewardState)).toBe(
        stage,
      )
    }
    expect(X_CLAIM_REWARD_LIFECYCLE).toEqual([
      'eligible',
      'operator_approved',
      'dispatched',
      'settled',
    ])
  })

  test('redacts owner, agent, and X-account identity to digest refs while keeping the citable receipt ref', async () => {
    const projection = await xClaimRewardEligibilityProjection(reward())
    const json = JSON.stringify(projection)

    expect(projection.receiptRef).toBe(
      'x_claim_reward_receipt_x_claim_reward_audit_case',
    )
    expect(projection.ownerRef).toMatch(/^owner\.sha256\.[0-9a-f]{16}$/)
    expect(projection.agentRef).toMatch(/^agent\.sha256\.[0-9a-f]{16}$/)
    expect(projection.xAccountRef).toMatch(/^x_account\.sha256\.[0-9a-f]{16}$/)
    expect(json).not.toContain(OWNER_USER_ID)
    expect(json).not.toContain(AGENT_USER_ID)
    expect(json).not.toContain(X_ACCOUNT_REF)
    expect(json).not.toContain('eligible_owner_handle')
    // Evidence refs stay private; only their count is public.
    expect(projection.evidenceRefCount).toBe(1)
    expect(json).not.toContain('receipt.x_claim.challenge-1')
    expect(projection.nonSpendableCaveat).toContain(
      'not Forum tip settlement, accepted-work payout, Treasury authority, or spendable balance',
    )
  })
})

describe('x_claim_reward eligibility routes (#4754)', () => {
  test('serves the audit eligible case from a public surface by reward id and by receipt ref', async () => {
    const deps = dependencies([reward()])

    for (const ref of [
      'x_claim_reward_audit_case',
      'x_claim_reward_receipt_x_claim_reward_audit_case',
    ]) {
      const response = await xClaimRewardEligibilityStatusResponse(
        deps,
        getRequest(`/api/agents/claims/rewards/${ref}`),
        ref,
      )
      const body = (await response.json()) as Record<string, unknown>

      expect(response.status).toBe(200)
      expect(body).toMatchObject({
        contractVersion: X_CLAIM_REWARD_ELIGIBILITY_PROJECTION_CONTRACT,
        generatedAt: NOW,
        reward: {
          amountSats: 1000,
          lifecycleStage: 'eligible',
          receiptRef: 'x_claim_reward_receipt_x_claim_reward_audit_case',
          state: 'eligible',
          treasuryPaymentAttached: false,
        },
        staleness: {
          composition: 'live_at_read',
          maxStalenessSeconds: 0,
          rebuildsOn: ['x_claim_reward_state_transition'],
        },
      })
    }
  })

  test('a state transition is immediately visible: the dispatch smoke can cite operator approval and settlement', async () => {
    const approved = reward({
      state: 'dispatch_requested',
      stateReasonRef: 'reason.public.x_claim_reward_treasury_dispatch_started',
      updatedAt: '2026-06-11T02:00:00.000Z',
    })
    const response = await xClaimRewardEligibilityStatusResponse(
      dependencies([approved]),
      getRequest('/api/agents/claims/rewards/x_claim_reward_audit_case'),
      'x_claim_reward_audit_case',
    )
    const body = (await response.json()) as { reward: Record<string, unknown> }

    expect(body.reward).toMatchObject({
      lifecycleStage: 'operator_approved',
      state: 'dispatch_requested',
      stateReasonRef:
        'reason.public.x_claim_reward_treasury_dispatch_started',
    })

    const settled = reward({
      evidenceRefs: [
        'receipt.x_claim.challenge-1',
        'evidence.public.x_claim_reward.settlement',
      ],
      state: 'settled',
      treasuryPaymentId: 'treasury-payment-redacted-id',
      updatedAt: '2026-06-11T02:30:00.000Z',
    })
    const settledResponse = await xClaimRewardEligibilityStatusResponse(
      dependencies([settled]),
      getRequest('/api/agents/claims/rewards/x_claim_reward_audit_case'),
      'x_claim_reward_audit_case',
    )
    const settledBody = (await settledResponse.json()) as {
      reward: Record<string, unknown>
    }

    expect(settledBody.reward).toMatchObject({
      lifecycleStage: 'settled',
      state: 'settled',
      treasuryPaymentAttached: true,
    })
    // The treasury payment id itself never leaves the ledger.
    expect(JSON.stringify(settledBody)).not.toContain(
      'treasury-payment-redacted-id',
    )
  })

  test('lists the campaign ledger with per-stage counts, generatedAt, and the staleness contract', async () => {
    const rewards = [
      reward(),
      reward({
        challengeId: 'challenge-2',
        id: 'x_claim_reward_second',
        ownerUserId: 'github:owner-2',
        receiptRef: 'x_claim_reward_receipt_x_claim_reward_second',
        state: 'dispatch_requested',
        xAccountRef: 'x:second_handle',
      }),
      reward({
        challengeId: 'challenge-3',
        id: 'x_claim_reward_third',
        ownerUserId: 'github:owner-3',
        receiptRef: 'x_claim_reward_receipt_x_claim_reward_third',
        state: 'refused',
        stateReasonRef:
          'reason.public.x_claim_reward_campaign_budget_exhausted',
        xAccountRef: 'x:third_handle',
      }),
    ]
    const response = await xClaimRewardEligibilityListResponse(
      dependencies(rewards),
      getRequest('/api/agents/claims/rewards'),
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      contractVersion: X_CLAIM_REWARD_ELIGIBILITY_PROJECTION_CONTRACT,
      counts: {
        dispatched: 0,
        eligible: 1,
        failed: 0,
        operator_approved: 1,
        refused: 1,
        settled: 0,
      },
      generatedAt: NOW,
      lifecycle: ['eligible', 'operator_approved', 'dispatched', 'settled'],
      staleness: { composition: 'live_at_read', maxStalenessSeconds: 0 },
    })
    const json = JSON.stringify(body)
    expect(json).not.toContain('github:owner')
    expect(json).not.toContain('second_handle')
    expect(json).not.toContain('third_handle')
  })

  test('refuses non-GET methods and unknown refs honestly', async () => {
    const deps = dependencies([reward()])

    const postResponse = await xClaimRewardEligibilityListResponse(
      deps,
      new Request('https://openagents.com/api/agents/claims/rewards', {
        method: 'POST',
      }),
    )
    expect(postResponse.status).toBe(405)

    const missing = await xClaimRewardEligibilityStatusResponse(
      deps,
      getRequest('/api/agents/claims/rewards/x_claim_reward_unknown'),
      'x_claim_reward_unknown',
    )
    expect(missing.status).toBe(404)
  })
})
