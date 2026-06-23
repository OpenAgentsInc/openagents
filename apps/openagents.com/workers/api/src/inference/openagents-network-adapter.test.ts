import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeAdmittedOpenAgentsNetworkAdapter,
  makeOpenAgentsNetworkAdapter,
  type NetworkServedResult,
  NETWORK_PAID_TRAFFIC_RECEIPT_UNVERIFIED_KIND,
  NETWORK_DISPATCH_PENDING_REASON,
  NETWORK_DISPATCH_UNAVAILABLE_KIND,
  NETWORK_PYLON_ADMISSION_REFUSED_KIND,
  OPENAGENTS_NETWORK_ADAPTER_ID,
  openAgentsNetworkAdapter,
  servingReceiptClearsPaidTraffic,
  type ServingReceipt,
} from './openagents-network-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'
import type { PylonServingSnapshot } from './khala-pylon-admission'

const runResult = <A>(effect: Effect.Effect<A, InferenceAdapterError>) =>
  Effect.runPromise(Effect.result(effect))

const request = (model = 'kimi-k2p6'): InferenceRequest => ({
  messages: [{ content: 'hello fabric', role: 'user' }],
  model,
  passthroughParams: {},
  stream: false,
})

const wholeModelReceipt: ServingReceipt = {
  parityMode: 'exact_greedy_parity',
  parityVerified: true,
  servedModel: 'kimi-k2p6',
  sharded: false,
  servingRunRef: 'serve.run.abc',
  stages: [{ layerEnd: 32, layerStart: 0, nodeRef: 'pylon.alpha', role: 'stage' }],
}

const paidTrafficVerifiedReceipt: ServingReceipt = {
  ...wholeModelReceipt,
  paidTrafficVerification: {
    blockerRefs: [],
    canaryPassed: true,
    parityPassed: true,
    payoutEligible: true,
    replayPassed: true,
  },
}

const servedResult: NetworkServedResult = {
  receipt: wholeModelReceipt,
  result: {
    content: 'served by the fabric',
    finishReason: 'stop',
    servedModel: 'kimi-k2p6',
    usage: { completionTokens: 4, promptTokens: 2, totalTokens: 6 },
  },
}

const paidTrafficVerifiedServedResult: NetworkServedResult = {
  ...servedResult,
  receipt: paidTrafficVerifiedReceipt,
}

const NOW_MS = Date.parse('2026-06-23T16:00:00.000Z')
const REQUIRED_CAPABILITY = 'capability.serving.khala_mini.v1'

const admittedSnapshot = (
  overrides: Partial<PylonServingSnapshot> = {},
): PylonServingSnapshot => ({
  capabilityRefs: [REQUIRED_CAPABILITY],
  latestHeartbeatAt: '2026-06-23T15:59:30.000Z',
  latestHeartbeatStatus: 'ok',
  pylonRef: 'pylon.alpha',
  servingLaneRefs: ['lane.nip90.serving.v1'],
  sparkPayoutTargetRef: 'payout.spark.redacted',
  status: 'active',
  walletReady: true,
  ...overrides,
})

const admission = (snapshot = admittedSnapshot()) => () => ({
  nowMs: NOW_MS,
  requiredCapabilityRef: REQUIRED_CAPABILITY,
  snapshot,
})

describe('openagents-network adapter — inert (no fabric dispatch)', () => {
  test('has the canonical lane adapter id', () => {
    expect(openAgentsNetworkAdapter.id).toBe(OPENAGENTS_NETWORK_ADAPTER_ID)
    expect(OPENAGENTS_NETWORK_ADAPTER_ID).toBe('openagents-network')
  })

  test('complete typed-fails non-retryably with the pending-fabric reason', async () => {
    const outcome = await runResult(openAgentsNetworkAdapter.complete(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(NETWORK_DISPATCH_UNAVAILABLE_KIND)
      expect(outcome.failure.reason).toBe(NETWORK_DISPATCH_PENDING_REASON)
      // NON-retryable: routing must overflow to the next lane, not back off here.
      expect(outcome.failure.retryable).toBe(false)
      expect(outcome.failure.adapterId).toBe(OPENAGENTS_NETWORK_ADAPTER_ID)
    }
  })

  test('stream typed-fails the same way (never fabricates frames)', async () => {
    const outcome = await runResult(openAgentsNetworkAdapter.stream(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(NETWORK_DISPATCH_UNAVAILABLE_KIND)
    }
  })
})

describe('openagents-network adapter — live fabric dispatch seam', () => {
  test('complete returns the served result from an injected fabric dispatch', async () => {
    const adapter = makeOpenAgentsNetworkAdapter({
      dispatch: () => Effect.succeed(servedResult),
    })
    const outcome = await runResult(adapter.complete(request()))
    expect(outcome._tag).toBe('Success')
    if (outcome._tag === 'Success') {
      expect(outcome.success.content).toBe('served by the fabric')
      expect(outcome.success.usage.totalTokens).toBe(6)
    }
  })

  test('stream maps the served result into a content frame + terminal usage frame', async () => {
    const adapter = makeOpenAgentsNetworkAdapter({
      dispatch: () => Effect.succeed(servedResult),
    })
    const outcome = await runResult(adapter.stream(request()))
    expect(outcome._tag).toBe('Success')
    if (outcome._tag === 'Success') {
      expect(outcome.success).toHaveLength(2)
      expect(outcome.success[0]!.contentDelta).toBe('served by the fabric')
      // Terminal frame carries the receipt-first usage for metering.
      expect(outcome.success[1]!.usage?.totalTokens).toBe(6)
      expect(outcome.success[1]!.finishReason).toBe('stop')
    }
  })

  test('a fabric dispatch refusal surfaces as a typed adapter error', async () => {
    const adapter = makeOpenAgentsNetworkAdapter({
      dispatch: () =>
        Effect.fail(
          new InferenceAdapterError({
            adapterId: OPENAGENTS_NETWORK_ADAPTER_ID,
            kind: 'request_rejected',
            reason: 'no hardware-backed receipt for large-model serve',
            retryable: false,
          }),
        ),
    })
    const outcome = await runResult(adapter.complete(request()))
    expect(outcome._tag).toBe('Failure')
  })
})

describe('openagents-network adapter — admitted paid Pylon lane', () => {
  test('paid-traffic receipt gate requires parity, canary, replay, and payout eligibility', () => {
    expect(servingReceiptClearsPaidTraffic(wholeModelReceipt)).toBe(false)
    expect(servingReceiptClearsPaidTraffic(paidTrafficVerifiedReceipt)).toBe(true)
    expect(
      servingReceiptClearsPaidTraffic({
        ...paidTrafficVerifiedReceipt,
        paidTrafficVerification: {
          ...paidTrafficVerifiedReceipt.paidTrafficVerification!,
          replayPassed: false,
          blockerRefs: ['blocker.pylon.serving.replay_mismatch'],
        },
      }),
    ).toBe(false)
  })

  test('refuses before dispatch when the candidate Pylon is not admitted', async () => {
    let dispatchCount = 0
    const adapter = makeAdmittedOpenAgentsNetworkAdapter({
      admission: admission(admittedSnapshot({ walletReady: false })),
      dispatch: () => {
        dispatchCount += 1
        return Effect.succeed(paidTrafficVerifiedServedResult)
      },
    })

    const outcome = await runResult(adapter.complete(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(NETWORK_PYLON_ADMISSION_REFUSED_KIND)
      expect(outcome.failure.reason).toContain(
        'blocker.pylon_admission.wallet_not_ready',
      )
      expect(outcome.failure.retryable).toBe(false)
    }
    expect(dispatchCount).toBe(0)
  })

  test('refuses paid routing when dispatch returns only a parity receipt', async () => {
    const adapter = makeAdmittedOpenAgentsNetworkAdapter({
      admission: admission(),
      dispatch: () => Effect.succeed(servedResult),
    })

    const outcome = await runResult(adapter.complete(request()))
    expect(outcome._tag).toBe('Failure')
    if (outcome._tag === 'Failure') {
      expect(outcome.failure.kind).toBe(
        NETWORK_PAID_TRAFFIC_RECEIPT_UNVERIFIED_KIND,
      )
      expect(outcome.failure.retryable).toBe(false)
    }
  })

  test('serves only after admission and the full paid-traffic receipt pass', async () => {
    const adapter = makeAdmittedOpenAgentsNetworkAdapter({
      admission: admission(),
      dispatch: () => Effect.succeed(paidTrafficVerifiedServedResult),
    })

    const complete = await runResult(adapter.complete(request()))
    expect(complete._tag).toBe('Success')
    if (complete._tag === 'Success') {
      expect(complete.success.content).toBe('served by the fabric')
    }

    const stream = await runResult(adapter.stream(request()))
    expect(stream._tag).toBe('Success')
    if (stream._tag === 'Success') {
      expect(stream.success[1]!.usage?.totalTokens).toBe(6)
    }
  })
})
