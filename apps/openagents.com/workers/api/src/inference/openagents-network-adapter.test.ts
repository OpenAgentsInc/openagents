import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  makeOpenAgentsNetworkAdapter,
  type NetworkServedResult,
  NETWORK_DISPATCH_PENDING_REASON,
  NETWORK_DISPATCH_UNAVAILABLE_KIND,
  OPENAGENTS_NETWORK_ADAPTER_ID,
  openAgentsNetworkAdapter,
  type ServingReceipt,
} from './openagents-network-adapter'
import {
  InferenceAdapterError,
  type InferenceRequest,
} from './provider-adapter'

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

const servedResult: NetworkServedResult = {
  receipt: wholeModelReceipt,
  result: {
    content: 'served by the fabric',
    finishReason: 'stop',
    servedModel: 'kimi-k2p6',
    usage: { completionTokens: 4, promptTokens: 2, totalTokens: 6 },
  },
}

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
