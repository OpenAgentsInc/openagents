import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { XClaimRewardRecord } from './agent-owner-claim-routes'
import { handleOperatorTreasuryStatusApi } from './treasury-routes'
import {
  type XClaimRewardTreasuryClient,
  type XClaimRewardTreasuryDispatchConfig,
  type XClaimRewardTreasuryDispatchStats,
  type XClaimRewardTreasuryDispatchStore,
  evaluateXClaimRewardSmokePreflight,
  makeD1XClaimRewardRecipientResolver,
  readXClaimRewardTreasuryDispatchConfig,
  runXClaimRewardOperatorDispatchSmoke,
  runXClaimRewardTreasuryDispatch,
  xClaimRewardDispatchDayStartIso,
} from './x-claim-reward-treasury-dispatcher'

const nowIso = '2026-06-10T12:00:00.000Z'
const safeOffer =
  'lno1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq'

const config = (
  overrides: Partial<XClaimRewardTreasuryDispatchConfig> = {},
): XClaimRewardTreasuryDispatchConfig => ({
  dailySatsCap: 5000,
  enabled: true,
  liquidityBufferSats: 11,
  nowIso,
  paymentTimeoutSecs: 50,
  perRunRewardCap: 1,
  ...overrides,
})

const rewardRecord = (
  overrides: Partial<XClaimRewardRecord> = {},
): XClaimRewardRecord => ({
  agentUserId: 'user_agent_1',
  amountSats: 1000,
  challengeId: 'x_challenge_1',
  claimId: 'agent_claim_1',
  createdAt: '2026-06-10T10:00:00.000Z',
  evidenceRefs: ['receipt.public.x_claim.1'],
  id: 'x_claim_reward_1',
  ownerUserId: 'user_owner_1',
  receiptRef: 'x_claim_reward_receipt_1',
  state: 'dispatch_requested',
  stateReasonRef: null,
  treasuryPaymentId: null,
  updatedAt: '2026-06-10T10:00:00.000Z',
  xAccountRef: 'x_account.public.owner_1',
  ...overrides,
})

class MemoryDispatchStore implements XClaimRewardTreasuryDispatchStore {
  readonly rewards = new Map<string, XClaimRewardRecord>()

  constructor(rewards: ReadonlyArray<XClaimRewardRecord>) {
    rewards.forEach(reward => this.rewards.set(reward.id, reward))
  }

  attachTreasuryPayment = async (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    paymentId: string
    rewardId: string
    stateReasonRef: string | null
  }) => {
    const reward = this.rewards.get(input.rewardId)

    if (
      reward === undefined ||
      reward.state !== 'dispatched' ||
      reward.treasuryPaymentId !== null
    ) {
      return undefined
    }

    return this.put({
      ...reward,
      evidenceRefs: input.evidenceRefs,
      stateReasonRef: input.stateReasonRef,
      treasuryPaymentId: input.paymentId,
      updatedAt: input.nowIso,
    })
  }

  claimDispatchRequestedReward = async (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    rewardId: string
    stateReasonRef: string
  }) => {
    const reward = this.rewards.get(input.rewardId)

    if (
      reward === undefined ||
      reward.state !== 'dispatch_requested' ||
      reward.treasuryPaymentId !== null
    ) {
      return undefined
    }

    return this.put({
      ...reward,
      evidenceRefs: input.evidenceRefs,
      state: 'dispatched',
      stateReasonRef: input.stateReasonRef,
      updatedAt: input.nowIso,
    })
  }

  countTodayReservedSats = async (dayStartIso: string) =>
    Array.from(this.rewards.values())
      .filter(
        reward =>
          reward.updatedAt >= dayStartIso &&
          reward.treasuryPaymentId !== null &&
          (reward.state === 'dispatched' || reward.state === 'settled'),
      )
      .reduce((total, reward) => total + reward.amountSats, 0)

  failReward = async (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    reasonRef: string
    rewardId: string
  }) => {
    const reward = this.rewards.get(input.rewardId)

    return reward === undefined
      ? undefined
      : this.put({
          ...reward,
          evidenceRefs: input.evidenceRefs,
          state: 'failed',
          stateReasonRef: input.reasonRef,
          updatedAt: input.nowIso,
        })
  }

  listDispatchRequestedRewards = async (limit: number) =>
    this.sortedRewards()
      .filter(reward => reward.state === 'dispatch_requested')
      .slice(0, limit)

  listPendingTreasuryPaymentRewards = async (limit: number) =>
    this.sortedRewards()
      .filter(
        reward =>
          reward.state === 'dispatched' && reward.treasuryPaymentId !== null,
      )
      .slice(0, limit)

  readDispatchStats = async (
    dayStartIso: string,
    dispatchConfig: XClaimRewardTreasuryDispatchConfig,
  ) => ({
    dailySatsCap: dispatchConfig.dailySatsCap,
    enabled: dispatchConfig.enabled,
    liquidityBufferSats: dispatchConfig.liquidityBufferSats,
    pendingPaymentCount: this.sortedRewards().filter(
      reward => reward.state === 'dispatched' && reward.treasuryPaymentId !== null,
    ).length,
    perRunRewardCap: dispatchConfig.perRunRewardCap,
    requestedDispatchCount: this.sortedRewards().filter(
      reward => reward.state === 'dispatch_requested',
    ).length,
    todayReservedSats: await this.countTodayReservedSats(dayStartIso),
  })

  readRewardById = async (rewardId: string) => this.rewards.get(rewardId)

  settleReward = async (input: {
    evidenceRefs: ReadonlyArray<string>
    nowIso: string
    rewardId: string
  }) => {
    const reward = this.rewards.get(input.rewardId)

    return reward === undefined
      ? undefined
      : this.put({
          ...reward,
          evidenceRefs: input.evidenceRefs,
          state: 'settled',
          stateReasonRef: null,
          updatedAt: input.nowIso,
        })
  }

  private put(reward: XClaimRewardRecord): XClaimRewardRecord {
    this.rewards.set(reward.id, reward)

    return reward
  }

  private sortedRewards(): ReadonlyArray<XClaimRewardRecord> {
    return Array.from(this.rewards.values()).sort((left, right) =>
      left.updatedAt.localeCompare(right.updatedAt),
    )
  }
}

class FakeTreasury implements XClaimRewardTreasuryClient {
  readonly paidDestinations: Array<string> = []
  maxSendableSat: number | null = 2500
  nextPay:
    | Awaited<ReturnType<XClaimRewardTreasuryClient['pay']>>
    | undefined
  payments = new Map<string, Awaited<ReturnType<XClaimRewardTreasuryClient['readPayment']>>>()

  pay = async (input: {
    amountSat: number
    destination: string
    timeoutSecs: number
  }) => {
    this.paidDestinations.push(input.destination)

    return (
      this.nextPay ?? {
        kind: 'succeeded' as const,
        paymentId: 'payment_secret_1',
      }
    )
  }

  readBalance = async () => ({
    kind: 'ok' as const,
    maxSendableSat: this.maxSendableSat,
  })

  readPayment = async (paymentId: string) =>
    this.payments.get(paymentId) ?? {
      kind: 'pending' as const,
      paymentId,
    }
}

const recipientDb = (
  row: Readonly<{
    bolt12_offer: string | null
    lightning_address?: string | null
    wallet_ref: string
  }> | null,
): D1Database =>
  ({
    prepare: (query: string) => ({
      bind: () => ({
        first: async () =>
          query.includes('bolt12_offer IS NOT NULL') &&
          row?.bolt12_offer === null
            ? null
            : row,
      }),
    }),
  }) as unknown as D1Database

const runDispatch = (
  store: MemoryDispatchStore,
  treasury: XClaimRewardTreasuryClient | null,
  dispatchConfig = config(),
) =>
  runXClaimRewardTreasuryDispatch({
    config: dispatchConfig,
    resolveRecipient: async reward =>
      reward.agentUserId === null
        ? null
        : {
            destination: safeOffer,
            destinationSourceRef: `wallet.public.${reward.agentUserId}.redacted`,
          },
    store,
    treasury,
  })

describe('X claim reward treasury dispatcher', () => {
  test('defaults to disabled unless TREASURY_DISPATCH_ENABLED is true', () => {
    expect(readXClaimRewardTreasuryDispatchConfig({}, nowIso).enabled).toBe(false)
    expect(
      readXClaimRewardTreasuryDispatchConfig(
        { TREASURY_DISPATCH_ENABLED: 'true' },
        nowIso,
      ).enabled,
    ).toBe(true)
  })

  test('settles one approved reward through the treasury binding without leaking payment material', async () => {
    const store = new MemoryDispatchStore([rewardRecord()])
    const treasury = new FakeTreasury()
    const summary = await runDispatch(store, treasury)
    const reward = store.rewards.get('x_claim_reward_1')
    const serialized = JSON.stringify(summary)

    expect(summary.requested).toBe(1)
    expect(summary.settled).toBe(1)
    expect(treasury.paidDestinations).toEqual([safeOffer])
    expect(reward?.state).toBe('settled')
    expect(reward?.treasuryPaymentId).toBe('payment_secret_1')
    expect(reward?.evidenceRefs).toContain(
      'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
    )
    expect(serialized).not.toContain(safeOffer)
    expect(serialized).not.toContain('payment_secret_1')
  })

  test('polls pending treasury payments on later ticks without paying again', async () => {
    const store = new MemoryDispatchStore([rewardRecord()])
    const treasury = new FakeTreasury()
    treasury.nextPay = { kind: 'pending', paymentId: 'payment_secret_pending' }

    const first = await runDispatch(store, treasury)
    treasury.payments.set('payment_secret_pending', {
      kind: 'succeeded',
      paymentId: 'payment_secret_pending',
    })
    const second = await runDispatch(store, treasury)
    const reward = store.rewards.get('x_claim_reward_1')

    expect(first.pending).toBe(1)
    expect(second.polled).toBe(1)
    expect(second.settled).toBe(1)
    expect(treasury.paidDestinations).toHaveLength(1)
    expect(reward?.state).toBe('settled')
  })

  test('records terminal treasury failures as failed reward rows', async () => {
    const store = new MemoryDispatchStore([rewardRecord()])
    const treasury = new FakeTreasury()
    treasury.nextPay = {
      kind: 'failed',
      paymentId: null,
      reasonRef: 'reason.public.x_claim_reward_treasury_pay_failed',
    }

    const summary = await runDispatch(store, treasury)
    const reward = store.rewards.get('x_claim_reward_1')

    expect(summary.failed).toBe(1)
    expect(reward?.state).toBe('failed')
    expect(reward?.stateReasonRef).toBe(
      'reason.public.x_claim_reward_treasury_pay_failed',
    )
  })

  test('refuses rewards without registered recipient wallet identity', async () => {
    const store = new MemoryDispatchStore([
      rewardRecord({ agentUserId: null, id: 'x_claim_reward_missing_wallet' }),
    ])
    const treasury = new FakeTreasury()

    const summary = await runDispatch(store, treasury)
    const reward = store.rewards.get('x_claim_reward_missing_wallet')

    expect(summary.failed).toBe(1)
    expect(treasury.paidDestinations).toHaveLength(0)
    expect(reward?.stateReasonRef).toBe(
      'reason.public.x_claim_reward_recipient_wallet_missing',
    )
  })

  test('resolves only a registered BOLT12 recipient for live reward dispatch', async () => {
    const bolt12Recipient = await makeD1XClaimRewardRecipientResolver(
      recipientDb({
        bolt12_offer: safeOffer,
        lightning_address: 'owner@example.com',
        wallet_ref: 'wallet.public.agent.bolt12',
      }),
    )(rewardRecord())
    const lightningOnlyRecipient = await makeD1XClaimRewardRecipientResolver(
      recipientDb({
        bolt12_offer: null,
        lightning_address: 'owner@example.com',
        wallet_ref: 'wallet.public.agent.lightning_address_only',
      }),
    )(rewardRecord())

    expect(bolt12Recipient).toEqual({
      destination: safeOffer,
      destinationSourceRef: 'wallet.public.agent.bolt12',
    })
    expect(lightningOnlyRecipient).toBeNull()
  })

  test('enforces per-run, per-day, and liquidity caps before new sends', async () => {
    const first = rewardRecord({ id: 'x_claim_reward_1' })
    const second = rewardRecord({
      id: 'x_claim_reward_2',
      updatedAt: '2026-06-10T10:01:00.000Z',
    })
    const store = new MemoryDispatchStore([first, second])
    const treasury = new FakeTreasury()

    const cappedRun = await runDispatch(store, treasury, config({ perRunRewardCap: 1 }))
    treasury.maxSendableSat = 1000
    const liquiditySkipped = await runDispatch(store, treasury)
    const dailySkipped = await runDispatch(
      new MemoryDispatchStore([rewardRecord()]),
      new FakeTreasury(),
      config({ dailySatsCap: 999 }),
    )

    expect(cappedRun.requested).toBe(1)
    expect(store.rewards.get('x_claim_reward_2')?.state).toBe(
      'dispatch_requested',
    )
    expect(liquiditySkipped.skippedReasonRefs).toContain(
      'reason.public.x_claim_reward_treasury_liquidity_insufficient',
    )
    expect(dailySkipped.skippedReasonRefs).toContain(
      'reason.public.x_claim_reward_treasury_daily_cap_reached',
    )
  })

  test('double ticks race through one ledger claim and pay at most once', async () => {
    const store = new MemoryDispatchStore([rewardRecord()])
    const treasury = new FakeTreasury()
    const [first, second] = await Promise.all([
      runDispatch(store, treasury),
      runDispatch(store, treasury),
    ])

    expect(first.requested + second.requested).toBe(1)
    expect(treasury.paidDestinations).toHaveLength(1)
    expect(store.rewards.get('x_claim_reward_1')?.state).toBe('settled')
  })

  describe('live smoke preflight', () => {
    const readyStats = (
      overrides: Partial<XClaimRewardTreasuryDispatchStats> = {},
    ): XClaimRewardTreasuryDispatchStats => ({
      dailySatsCap: 5000,
      enabled: true,
      liquidityBufferSats: 11,
      pendingPaymentCount: 0,
      perRunRewardCap: 1,
      requestedDispatchCount: 1,
      todayReservedSats: 0,
      ...overrides,
    })

    test('reports ready when one approved reward and bounded liquidity are present', () => {
      const report = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 2500,
        stats: readyStats(),
      })

      expect(report.ready).toBe(true)
      expect(report.blockingReasonRefs).toEqual([])
      expect(report.checks.every(check => check.ok)).toBe(true)
    })

    test('blocks when the dispatch flag is still off', () => {
      const report = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 2500,
        stats: readyStats({ enabled: false }),
      })

      expect(report.ready).toBe(false)
      expect(report.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_treasury_dispatch_disabled',
      )
    })

    test('blocks unless exactly one reward is approved for the first smoke', () => {
      const none = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 2500,
        stats: readyStats({ requestedDispatchCount: 0 }),
      })
      const many = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 2500,
        stats: readyStats({ requestedDispatchCount: 2 }),
      })

      expect(none.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_no_approved_reward',
      )
      expect(many.ready).toBe(false)
    })

    test('blocks when a pending payment is still in flight', () => {
      const report = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 2500,
        stats: readyStats({ pendingPaymentCount: 1 }),
      })

      expect(report.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_treasury_payment_pending',
      )
    })

    test('blocks on insufficient or unavailable treasury liquidity', () => {
      const thin = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 1000,
        stats: readyStats(),
      })
      const unavailable = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: null,
        stats: readyStats(),
      })

      expect(thin.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_treasury_liquidity_insufficient',
      )
      expect(unavailable.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_treasury_balance_unavailable',
      )
    })

    test('blocks when the daily sats cap leaves no headroom for one reward', () => {
      const report = evaluateXClaimRewardSmokePreflight({
        balanceMaxSendableSat: 2500,
        stats: readyStats({ dailySatsCap: 5000, todayReservedSats: 4500 }),
      })

      expect(report.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_treasury_daily_cap_reached',
      )
    })

    test('preflight report carries no payment material', () => {
      const serialized = JSON.stringify(
        evaluateXClaimRewardSmokePreflight({
          balanceMaxSendableSat: 2500,
          stats: readyStats(),
        }),
      )

      expect(serialized).not.toContain(safeOffer)
      expect(serialized).not.toContain('payment_secret')
    })
  })

  test('operator treasury status includes aggregate dispatch stats only', async () => {
    const store = new MemoryDispatchStore([
      rewardRecord({
        id: 'x_claim_reward_status',
        state: 'dispatched',
        treasuryPaymentId: 'payment_secret_status',
      }),
    ])
    const dispatchConfig = config()
    const response = await Effect.runPromise(
      handleOperatorTreasuryStatusApi(
        new Request('https://openagents.com/api/operator/treasury/status'),
        {
          fetchTreasury: path =>
            Promise.resolve(
              path === '/balance'
                ? new Response(JSON.stringify({ maxSendableSat: 2500 }), {
                    status: 200,
                  })
                : new Response(
                    JSON.stringify({
                      accessTokenConfigured: true,
                      mnemonicConfigured: true,
                      serviceTokenConfigured: true,
                    }),
                    { status: 200 },
                  ),
            ),
          readRewardDispatchStats: () =>
            store.readDispatchStats(
              xClaimRewardDispatchDayStartIso(nowIso),
              dispatchConfig,
            ),
          requireAdminApiToken: () => Promise.resolve(true),
        },
      ),
    )
    const body = (await response.json()) as {
      rewardDispatch: { pendingPaymentCount: number }
    }
    const serialized = JSON.stringify(body)

    expect(body.rewardDispatch.pendingPaymentCount).toBe(1)
    expect(serialized).not.toContain('payment_secret_status')
    expect(serialized).not.toContain(safeOffer)
  })

  describe('operator dispatch smoke harness', () => {
    test('runs one armed reward through dispatch and returns public-safe receipt evidence', async () => {
      const store = new MemoryDispatchStore([rewardRecord()])
      const treasury = new FakeTreasury()
      const report = await runXClaimRewardOperatorDispatchSmoke({
        config: config(),
        resolveRecipient: async reward =>
          reward.agentUserId === null
            ? null
            : {
                destination: safeOffer,
                destinationSourceRef: `wallet.public.${reward.agentUserId}.redacted`,
              },
        store,
        treasury,
      })
      const serialized = JSON.stringify(report)

      expect(report.ready).toBe(true)
      expect(report.preflight.ready).toBe(true)
      expect(report.summary?.requested).toBe(1)
      expect(report.summary?.settled).toBe(1)
      expect(report.rewardRef).toBe('x_claim_reward_receipt_1')
      expect(report.completion?.transitionRequest).toEqual({
        evidenceRefs: [
          'x_claim_reward_receipt_1',
          'settlement_evidence.public.mdk_treasury.x_claim_reward_x_claim_reward_1',
        ],
        promiseId: 'agents.x_claim_reward.v1',
        toState: 'green',
      })
      expect(treasury.paidDestinations).toEqual([safeOffer])
      expect(serialized).not.toContain(safeOffer)
      expect(serialized).not.toContain('payment_secret_1')
    })

    test('stays no-op when the dispatch flag is not armed', async () => {
      const store = new MemoryDispatchStore([rewardRecord()])
      const treasury = new FakeTreasury()
      const report = await runXClaimRewardOperatorDispatchSmoke({
        config: config({ enabled: false }),
        resolveRecipient: async () => ({
          destination: safeOffer,
          destinationSourceRef: 'wallet.public.agent.redacted',
        }),
        store,
        treasury,
      })

      expect(report.ready).toBe(false)
      expect(report.summary).toBeNull()
      expect(report.completion).toBeNull()
      expect(report.blockingReasonRefs).toContain(
        'reason.public.x_claim_reward_treasury_dispatch_disabled',
      )
      expect(treasury.paidDestinations).toEqual([])
      expect(store.rewards.get('x_claim_reward_1')?.state).toBe(
        'dispatch_requested',
      )
    })
  })
})
